//! Stack resolution orchestration and exhaustive primitive dispatch.
//!
//! Adding a primitive requires one exhaustive `SpellEffectKind` arm below and one implementation
//! in the best-fit domain module. Dispatch arms contain delegation only, so variant coverage stays
//! compiler-checked while resolution logic remains grouped by domain.

use super::events::{color_string, ev_log, object_display_name};
use super::targeting::{
    battlefield_objects_matching, compute_spell_targets, effect_has_legal_target_at_resolution,
    object_matches_mass_filter, spell_effect_kind_needs_target, target_filter_legal_at_resolution,
};
use super::*;
use rand::seq::SliceRandom;
use rand::SeedableRng;

mod damage;
/// `pub(super)` so the combat damage step can reach `life::apply_life_gain` — lifelink is the one
/// life-gain edge outside stack resolution, and it must go through the same funnel.
pub(super) mod life;
mod mass;
mod misc;
mod pump_counters;
mod stack_ops;
mod tokens;
mod zones;

/// Shared resolution context for one primitive effect.
struct EffectCx<'a> {
    engine: &'a mut GameEngine,
    events: &'a mut Vec<rv1::RuledEvent>,
    targets: &'a [ObjectId],
    target_damage: &'a [u32],
    top: &'a StackItem,
    controller: PlayerId,
    /// The player an untargeted, player-scoped effect acts on. Equals `controller` for spells,
    /// activated abilities, and every trigger that doesn't name another player; differs only when
    /// a triggered ability says "**that player** …" ([`StackItem::trigger_player`] — Howling Mine).
    /// Effects that act on the *controller* by rule (Brainstorm's draw, a self-pump) keep using
    /// `controller`.
    affected_player: PlayerId,
    spell_label: &'a str,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum EffectOutcome {
    Continue,
    Suspended,
}

/// One entry of a resolving stack item's flattened effect list: the primitive, the targets it was
/// cast with, and the per-target damage split. A modal spell contributes one entry per effect of
/// each chosen mode, so targets are per-entry rather than per-item.
type ResolutionEffect = (SpellEffectKind, Vec<ObjectId>, Vec<u32>);

impl GameEngine {
    pub(super) fn resolve_top_of_stack(
        &mut self,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<(), EngineError> {
        let top = self
            .state
            .stack
            .pop()
            .ok_or(EngineError::Illegal("empty stack"))?;
        let controller = top.controller;
        let card_id = top.card_id.clone();
        let targets = top.targets.clone();

        // Abilities — and spell copies (CR 707.10d) — leave no object behind when they resolve;
        // only a genuinely cast spell has a backing card that moves to a zone. A copy has no
        // `GameObject` in `objects`, so it must take the same no-zone-move path as an ability.
        let is_ability = top.ability_text.is_some();
        let leaves_no_object = is_ability || top.is_copy;
        if leaves_no_object {
            events.push(rv1::RuledEvent {
                ev: Some(rv1::ruled_event::Ev::StackResolved(rv1::StackResolved {
                    object_id: top.id,
                    // Abilities cease to exist on resolution; graveyard tells the C++ server
                    // not to expect a permanent to land.
                    destination: rv1::StackResolveDestination::Graveyard as i32,
                })),
            });
        } else {
            // CR 709/712/715: permanence is the *cast face's* (Ice resolves to graveyard; an MDFC
            // permanent face resolves to the battlefield as that face).
            let resolving_face = self
                .registry
                .get(&card_id)
                .and_then(|d| d.face(top.face_index));
            let resolves_to_battlefield_raw =
                resolving_face.map(|f| f.is_permanent()).unwrap_or(false);
            // CR 303.4f: an aura whose enchant target is no longer on the battlefield at resolution
            // is countered (goes to owner's graveyard) rather than entering the battlefield orphaned.
            let is_aura =
                resolves_to_battlefield_raw && resolving_face.map(|f| f.is_aura).unwrap_or(false);
            let aura_target_valid = !is_aura
                || targets.first().is_some_and(|&tid| {
                    self.state
                        .objects
                        .get(&tid)
                        .map(|o| o.zone == Zone::Battlefield)
                        .unwrap_or(false)
                });
            // CR 702.34a: a spell cast with flashback is exiled instead of being put into its
            // owner's graveyard as it leaves the stack, regardless of whether it would normally
            // be a permanent spell.
            let resolves_to_battlefield =
                !top.flashback && resolves_to_battlefield_raw && aura_target_valid;
            let destination = if resolves_to_battlefield {
                rv1::StackResolveDestination::Battlefield as i32
            } else if top.flashback {
                rv1::StackResolveDestination::Exile as i32
            } else {
                rv1::StackResolveDestination::Graveyard as i32
            };
            events.push(rv1::RuledEvent {
                ev: Some(rv1::ruled_event::Ev::StackResolved(rv1::StackResolved {
                    object_id: top.id,
                    destination,
                })),
            });
            move_object_to_zone(
                &mut self.state,
                top.id,
                if resolves_to_battlefield {
                    Zone::Battlefield
                } else if top.flashback {
                    Zone::Exile
                } else {
                    Zone::Graveyard
                },
                // CR 110.2a: a permanent spell's controller becomes the permanent's controller.
                // Identical to the owner for a spell cast from its owner's hand, which is every
                // case today, but it is the spell's controller that is authoritative.
                Some(top.controller),
            )?;
            if resolves_to_battlefield {
                // CR 712.4: a permanent enters the battlefield showing the face that was cast.
                // Set face_up_index from the stack item so characteristic queries (types, keywords,
                // P/T, mana abilities) read from the correct face while on the battlefield.
                if let Some(o) = self.state.objects.get_mut(&top.id) {
                    o.face_up_index = top.face_index;
                }
                // Set attached_to before ETB triggers so emit_static_abilities_on_enter can read it.
                if is_aura {
                    if let (Some(aura_obj), Some(&enchanted_oid)) =
                        (self.state.objects.get_mut(&top.id), targets.first())
                    {
                        aura_obj.attached_to = Some(enchanted_oid);
                    }
                }
                self.fire_triggers(GameEvent::EntersBattlefield { object_id: top.id }, events);
            } else if is_aura {
                let aura_name = self
                    .registry
                    .get(&card_id)
                    .map(|d| d.name.as_str())
                    .unwrap_or("Aura");
                events.push(ev_log(format!(
                    "{aura_name} fizzles (enchant target left the battlefield)."
                )));
                return Ok(());
            }
        }

        // Tier-3 (CR 608): a custom effect owns this spell's resolution. The spell card has
        // already moved to its zone (graveyard/battlefield above); hand off the algorithm to the
        // registered `CardEffect`, which either completes now or parks awaiting a player choice.
        // A copy is excluded: the resumable custom machinery (`begin_custom_resolution`) expects the
        // spell's backing `GameObject`, which a copy lacks. Copying a tier-3 spell is a documented
        // limitation (the copy resolves its non-custom effects only, if any).
        if !is_ability && !top.is_copy {
            let custom_key = self
                .registry
                .get(&card_id)
                .and_then(|d| d.face(top.face_index))
                .and_then(|f| f.custom_effect.clone());
            if let Some(custom_key) = custom_key {
                return self.begin_custom_resolution(top, custom_key, events);
            }
        }

        let (resolution_effects, spell_label) = self.build_resolution_effects(&top);

        // CR 603.4, second of the two checks: a triggered ability with an intervening-"if" clause
        // does nothing if the clause is false as it resolves, even though it was true when the
        // ability triggered (Howling Mine tapped in response to its own trigger).
        if top.is_triggered {
            let clause = self
                .registry
                .get(&card_id)
                .and_then(|d| d.face(top.face_index))
                .and_then(|f| f.triggered_abilities.get(top.ability_index.unwrap_or(0)))
                .and_then(|ta| ta.intervening_if);
            let source_id = top.source_permanent_id.unwrap_or(top.id);
            let holds = if top.source_permanent_id.is_some() {
                self.intervening_if_holds_at_generation(
                    source_id,
                    clause,
                    Some(top.source_zone_change),
                )
            } else {
                self.intervening_if_holds(source_id, clause)
            };
            if !holds {
                events.push(ev_log(format!(
                    "{spell_label} does nothing (its \"if\" condition is no longer true, CR 603.4)."
                )));
                return Ok(());
            }
        }

        // CR 608.2b: targets are checked once, at the start of resolution — not again on resume.
        let targeted_effects: Vec<_> = resolution_effects
            .iter()
            .filter(|(effect, _, _)| spell_effect_kind_needs_target(effect))
            .collect();
        let fizzle = !targeted_effects.is_empty()
            && targeted_effects.iter().all(|(effect, mode_targets, _)| {
                !effect_has_legal_target_at_resolution(self, effect, mode_targets, controller)
            });
        if fizzle {
            events.push(ev_log(format!("{spell_label} fizzles (no legal targets).")));
            return Ok(());
        }

        self.run_effect_list(&top, &spell_label, resolution_effects, 0, events)
    }

    /// Rebuild a stack item's primitive effect list and display label.
    ///
    /// Pure function of the [`StackItem`] plus the registry, which is what lets a parked
    /// resolution resume its tail: nothing about the list has to be stored across the park, only
    /// the index to restart from (`PendingResolution::resume_effect_index`).
    pub(super) fn build_resolution_effects(
        &self,
        top: &StackItem,
    ) -> (Vec<ResolutionEffect>, String) {
        let is_ability = top.ability_text.is_some();
        let card_id: &str = &top.card_id;

        // Determine effects. Spells, triggered abilities and activated abilities are uniform:
        // every one of them carries a `Vec<SpellEffectKind>` resolved in written order (CR 608.2).
        // Self-referencing effects use a `Self_` target filter, bound below.
        let (effects, spell_label): (Vec<SpellEffectKind>, String) = if is_ability {
            let ability_index = top.ability_index.unwrap_or(0);
            let def = self.registry.get(card_id);
            let name = def
                .map(|d| d.name.clone())
                .unwrap_or_else(|| "Ability".into());
            // Ability indices are relative to the face recorded on the stack item, which is `0`
            // for abilities (see `StackItem::face_index`) — the same face `activate_ability`
            // and the trigger scan read them from.
            let face = def.and_then(|d| d.face(top.face_index));
            let abilities = if top.is_triggered {
                face.and_then(|f| f.triggered_abilities.get(ability_index))
                    .map(|a| a.effect.clone())
            } else {
                face.and_then(|f| f.activated_abilities.get(ability_index))
                    .map(|a| a.effect.clone())
            };
            (
                abilities.unwrap_or_else(|| vec![SpellEffectKind::None]),
                name,
            )
        } else {
            // CR 709/712/715: resolve the cast face's effects and show its name.
            let face = self
                .registry
                .get(card_id)
                .and_then(|d| d.face(top.face_index));
            let effects = face.map(|f| f.spell_effect.to_vec()).unwrap_or_default();
            let name = face
                .map(|f| f.name.to_string())
                .unwrap_or_else(|| "Spell".into());
            (effects, name)
        };

        let mut resolution_effects: Vec<ResolutionEffect> = Vec::new();
        if !is_ability && !top.chosen_modes.is_empty() {
            if let Some(modal) = self
                .registry
                .get(card_id)
                .and_then(|definition| definition.face(top.face_index))
                .and_then(|face| face.modal_spell.as_ref())
            {
                for chosen in &top.chosen_modes {
                    if let Some(mode) = modal.modes.get(chosen.mode_index) {
                        for effect in &mode.effects {
                            resolution_effects.push((
                                effect.clone(),
                                chosen.targets.clone(),
                                chosen.target_damage.clone(),
                            ));
                        }
                    }
                }
            }
        } else {
            resolution_effects.extend(
                effects
                    .into_iter()
                    .map(|effect| (effect, top.targets.clone(), top.target_damage.clone())),
            );
        }

        (resolution_effects, spell_label)
    }

    /// Run a stack item's primitive effects from `start` onwards, then close the resolution.
    ///
    /// Entered twice for a spell whose effect suspends: once from `resolve_top_of_stack` at index
    /// 0, and again from `complete_parked_resolution` at the index stamped below, once the player
    /// has answered. CR 608.2: the whole list runs, so an effect that parks for a choice must not
    /// swallow the effects after it (this is what `docs/issues.md` #36 tracked).
    pub(super) fn run_effect_list(
        &mut self,
        top: &StackItem,
        spell_label: &str,
        resolution_effects: Vec<ResolutionEffect>,
        start: usize,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<(), EngineError> {
        let controller = top.controller;
        for (index, (effect, effect_targets, effect_target_damage)) in
            resolution_effects.into_iter().enumerate().skip(start)
        {
            if spell_effect_kind_needs_target(&effect)
                && !effect_has_legal_target_at_resolution(
                    self,
                    &effect,
                    &effect_targets,
                    controller,
                )
            {
                continue;
            }
            let outcome = {
                let mut cx = EffectCx {
                    engine: self,
                    events,
                    targets: &effect_targets,
                    target_damage: &effect_target_damage,
                    top,
                    controller,
                    affected_player: top.trigger_player.unwrap_or(controller),
                    spell_label,
                };
                match effect {
                    effect @ SpellEffectKind::DamageTarget { .. } => {
                        damage::damage_target(&mut cx, effect)?
                    }
                    effect @ SpellEffectKind::DamageTargets { .. } => {
                        damage::damage_targets(&mut cx, effect)?
                    }
                    effect @ SpellEffectKind::Draw { .. } => zones::draw(&mut cx, effect)?,
                    effect @ SpellEffectKind::Scry { .. } => zones::scry(&mut cx, effect)?,
                    effect @ SpellEffectKind::PumpTarget { .. } => {
                        pump_counters::pump_target(&mut cx, effect)?
                    }
                    effect @ SpellEffectKind::PumpAll { .. } => {
                        pump_counters::pump_all(&mut cx, effect)?
                    }
                    effect @ SpellEffectKind::GrantKeywordsAll { .. } => {
                        pump_counters::grant_keywords_all(&mut cx, effect)?
                    }
                    effect @ SpellEffectKind::GrantKeywordsTarget { .. } => {
                        pump_counters::grant_keywords_target(&mut cx, effect)?
                    }
                    effect @ SpellEffectKind::GrantKeywordsAllPermanents { .. } => {
                        pump_counters::grant_keywords_all_permanents(&mut cx, effect)?
                    }
                    effect @ SpellEffectKind::PutCounters { .. } => {
                        pump_counters::put_counters(&mut cx, effect)?
                    }
                    effect @ SpellEffectKind::DestroyTarget { .. } => {
                        misc::destroy_target(&mut cx, effect)?
                    }
                    effect @ SpellEffectKind::CounterTargetSpell { .. } => {
                        stack_ops::counter_target_spell(&mut cx, effect)?
                    }
                    effect @ SpellEffectKind::CopyTargetSpell { .. } => {
                        stack_ops::copy_target_spell(&mut cx, effect)?
                    }
                    effect @ SpellEffectKind::GainLife { .. } => life::gain_life(&mut cx, effect)?,
                    effect @ SpellEffectKind::LoseLife { .. } => life::lose_life(&mut cx, effect)?,
                    effect @ SpellEffectKind::TargetPlayerGainsLife { .. } => {
                        life::target_player_gains_life(&mut cx, effect)?
                    }
                    effect @ SpellEffectKind::TargetPlayerLosesLife { .. } => {
                        life::target_player_loses_life(&mut cx, effect)?
                    }
                    effect @ SpellEffectKind::EachOpponentLosesLifeYouGainEqual { .. } => {
                        life::each_opponent_loses_life_you_gain_equal(&mut cx, effect)?
                    }
                    effect @ SpellEffectKind::DrainTarget { .. } => {
                        life::drain_target(&mut cx, effect)?
                    }
                    effect @ SpellEffectKind::ExileTarget => zones::exile_target(&mut cx, effect)?,
                    effect @ SpellEffectKind::ExileTargetGainLifeEqualToPower => {
                        zones::exile_target_gain_life_equal_to_power(&mut cx, effect)?
                    }
                    effect @ (SpellEffectKind::ReturnTargetCreatureToHand
                    | SpellEffectKind::ReturnTargetPermanentToHand) => {
                        zones::return_target_to_hand(&mut cx, effect)?
                    }
                    effect @ SpellEffectKind::DiscardCards { .. } => {
                        zones::discard_cards(&mut cx, effect)?
                    }
                    effect @ SpellEffectKind::MillTargetPlayer { .. } => {
                        zones::mill_target_player(&mut cx, effect)?
                    }
                    effect @ SpellEffectKind::TargetPlayerSacrifices { .. } => {
                        zones::target_player_sacrifices(&mut cx, effect)?
                    }
                    effect @ SpellEffectKind::TapTarget { .. } => {
                        misc::tap_target(&mut cx, effect)?
                    }
                    effect @ SpellEffectKind::UntapTarget { .. } => {
                        misc::untap_target(&mut cx, effect)?
                    }
                    effect @ SpellEffectKind::TapAllCreatures { .. } => {
                        misc::tap_all_creatures(&mut cx, effect)?
                    }
                    effect @ SpellEffectKind::UntapAll { .. } => mass::untap_all(&mut cx, effect)?,
                    effect @ SpellEffectKind::DestroyAll { .. } => {
                        mass::destroy_all(&mut cx, effect)?
                    }
                    effect @ SpellEffectKind::DamageAll { .. } => {
                        mass::damage_all(&mut cx, effect)?
                    }
                    effect @ SpellEffectKind::CreateTokens { .. } => {
                        tokens::create_tokens(&mut cx, effect)?
                    }
                    effect @ SpellEffectKind::Equip { .. } => misc::equip(&mut cx, effect)?,
                    effect @ SpellEffectKind::PreventNextDamage { .. } => {
                        misc::prevent_next_damage(&mut cx, effect)?
                    }
                    effect @ SpellEffectKind::PreventAllCombatDamageTurn => {
                        misc::prevent_all_combat_damage_turn(&mut cx, effect)?
                    }
                    effect @ SpellEffectKind::ReturnFromGraveyard { .. } => {
                        zones::return_from_graveyard(&mut cx, effect)?
                    }
                    effect @ SpellEffectKind::ProduceMana { .. } => {
                        misc::produce_mana(&mut cx, effect)?
                    }
                    effect @ SpellEffectKind::SearchLibrary { .. } => {
                        zones::search_library(&mut cx, effect)?
                    }
                    effect @ SpellEffectKind::Regenerate { .. } => {
                        misc::regenerate(&mut cx, effect)?
                    }
                    effect @ SpellEffectKind::None => misc::none(&mut cx, effect)?,
                    effect @ SpellEffectKind::AuraAttach { .. } => {
                        misc::aura_attach(&mut cx, effect)?
                    }
                }
            };
            if outcome == EffectOutcome::Suspended {
                // The handler parked a `PendingResolution` for a player choice; stamp where to
                // pick this list back up so `complete_parked_resolution` runs the tail (CR 608.2)
                // rather than ending the resolution here. Handlers do not set this themselves —
                // they have no idea which list they are a member of, or at what index.
                //
                // `if let` because `search_library`'s degenerate empty-library branch reports
                // `Suspended` without parking anything; there is then nothing to stamp.
                if let Some(pending) = self.state.pending_resolution.as_mut() {
                    pending.resume_effect_index = Some(index as u32 + 1);
                }
                return Ok(());
            }
        }
        events.push(ev_log(format!("{spell_label} resolves.")));
        // CR 608.2m: the spell lands in its owner's graveyard *after* its effects, so it sits
        // beneath anything those effects put there (e.g. a self-targeted Tome Scour's five cards).
        seat_resolved_spell_last_in_graveyard(&mut self.state, top.id);
        Ok(())
    }

    /// CR 111: mint `count` tokens of `token_id` for each recipient and put them onto the
    /// battlefield. Each minted token is a fresh [`GameObject`] whose characteristics come from
    /// the token's [`CardDefinition`] (via the registry's token namespace), so combat, P/T, and
    /// keyword queries treat it exactly like any other permanent. Entering tokens fire ETB
    /// triggers (CR 603.6) through the same hook as a resolved creature spell, so Soul Warden et al.
    /// see them. A [`TokenCreated`](rv1::TokenCreated) event carries the self-describing identity
    /// the relay needs (tokens have no deck card / Oracle entry).
    pub(super) fn create_tokens(
        &mut self,
        token_id: &str,
        count: u32,
        who: TokenController,
        spell_controller: PlayerId,
        spell_label: &str,
        events: &mut Vec<rv1::RuledEvent>,
    ) {
        let registry = self.registry;
        let Some(def) = registry.get(token_id) else {
            // Registry load validates every CreateTokens reference, so this is unreachable;
            // fail safe by doing nothing rather than panicking (server-authoritative).
            events.push(ev_log(format!(
                "{spell_label} could not create unknown token '{token_id}'."
            )));
            return;
        };
        let name = def.name.clone();
        // A token definition is always single-face (CR 111.4 identity is one characteristic tuple).
        let face = def.primary_face();
        let is_creature = face.is_creature;
        let power = face.power;
        let toughness = face.toughness;
        let types = face.types.to_vec();
        let keywords: Vec<String> = face
            .keywords
            .iter()
            .map(|k| k.as_str().to_string())
            .collect();
        let color = color_string(&face.colors());
        let pt = if is_creature {
            format!("{}/{}", power.unwrap_or(0), toughness.unwrap_or(0))
        } else {
            String::new()
        };

        let recipients: Vec<PlayerId> = match who {
            TokenController::Controller => vec![spell_controller],
            // CR 111.3: each token's owner/controller is the player it is created under.
            TokenController::EachPlayer => self
                .state
                .players
                .iter()
                .filter(|p| !p.has_lost)
                .map(|p| p.id)
                .collect(),
        };

        for pid in recipients {
            let Some(pidx) = self.state.player_idx(pid) else {
                continue;
            };
            for _ in 0..count {
                let oid = self.state.next_object_id;
                self.state.next_object_id += 1;
                self.state.objects.insert(
                    oid,
                    GameObject {
                        id: oid,
                        // CR 111.3: a token's owner is the player who controlled the effect that
                        // created it, so owner and controller coincide at creation.
                        owner: pid,
                        controller: pid,
                        card_id: token_id.to_string(),
                        zone: Zone::Battlefield,
                        tapped: false,
                        summoning_sick: is_creature,
                        power,
                        toughness,
                        damage: 0,
                        deathtouch_damage: false,
                        counters: BTreeMap::new(),
                        attached_to: None,
                        regeneration_shields: 0,
                        must_attack_if_able: false,
                        must_block_if_able: false,
                        face_up_index: 0,
                    },
                );
                self.state.players[pidx].battlefield.push(oid);
                events.push(rv1::RuledEvent {
                    ev: Some(rv1::ruled_event::Ev::TokenCreated(rv1::TokenCreated {
                        object_id: oid,
                        controller_player_id: pid,
                        card_id: token_id.to_string(),
                        identity: Some(rv1::TokenIdentity {
                            name: name.clone(),
                            pt: pt.clone(),
                            color: color.clone(),
                            types: types.clone(),
                            is_creature,
                            keywords: keywords.clone(),
                        }),
                    })),
                });
                // CR 603.6: a token entering the battlefield triggers ETB watchers.
                self.fire_triggers(GameEvent::EntersBattlefield { object_id: oid }, events);
            }
            let noun = if count == 1 { "token" } else { "tokens" };
            events.push(ev_log(format!(
                "P{pid} creates {count} {name} {noun} ({spell_label})."
            )));
        }
    }
}

/// The `(card_id, display name)` pair for each of `oids`, in order — the two parallel candidate
/// arrays a [`rv1::ResolutionChoiceRequired`] carries. Names come from the tricerules registry,
/// never Oracle.
pub(crate) fn candidate_identities(
    engine: &GameEngine,
    oids: &[ObjectId],
) -> (Vec<String>, Vec<String>) {
    let card_ids: Vec<String> = oids
        .iter()
        .map(|&oid| {
            engine
                .state
                .objects
                .get(&oid)
                .map(|o| o.card_id.clone())
                .unwrap_or_default()
        })
        .collect();
    let names = card_ids
        .iter()
        .map(|cid| {
            engine
                .registry
                .get(cid)
                .map(|d| d.name.clone())
                .unwrap_or_else(|| cid.clone())
        })
        .collect();
    (card_ids, names)
}

pub(super) fn draw_card(
    p: &mut PlayerState,
    objects: &mut HashMap<ObjectId, GameObject>,
) -> Result<(), EngineError> {
    let oid = p
        .library
        .pop_front()
        .ok_or(EngineError::Illegal("library empty"))?;
    p.hand.push(oid);
    if let Some(o) = objects.get_mut(&oid) {
        o.zone = Zone::Hand;
    }
    Ok(())
}

/// Build a `PermanentMoved` event, stamping the tricerules `card_id` from the object so
/// servers can resolve cards that have no engine-oid mapping (e.g. milled library cards).
pub(crate) fn permanent_moved_event(
    state: &GameState,
    oid: ObjectId,
    owner_player_id: PlayerId,
    destination: rv1::permanent_moved::Destination,
) -> rv1::RuledEvent {
    let card_id = state
        .objects
        .get(&oid)
        .map(|o| o.card_id.clone())
        .unwrap_or_default();
    // Callers emit this *after* the move, so the object already carries its post-move controller:
    // the new controller for a battlefield entry, and the owner again everywhere else (CR 400.7).
    // Always populated — proto3 scalars have no presence and player id 0 is valid, so a defaulted
    // 0 would be indistinguishable from "player 0 controls it".
    let controller_player_id = state
        .objects
        .get(&oid)
        .map(|o| o.controller)
        .unwrap_or(owner_player_id);
    rv1::RuledEvent {
        ev: Some(rv1::ruled_event::Ev::PermanentMoved(rv1::PermanentMoved {
            object_id: oid,
            owner_player_id,
            destination: destination as i32,
            card_id,
            controller_player_id,
        })),
    }
}

/// Resolve an [`AnthemFilter`] into an [`AffectedScope`] for a continuous effect, given the
/// effect's `controller` and the source permanent `source`. Shared by static anthems
/// (`AnthemPt`, source on the battlefield) and one-shot mass pumps (`PumpAll`, `source` is the
/// resolving spell). `exclude_self` only applies when the source persists, so a spell passes a
/// `source` that is harmlessly never on the battlefield as a creature.
pub(super) fn resolve_anthem_scope(
    filter: &AnthemFilter,
    controller: PlayerId,
    source: ObjectId,
) -> AffectedScope {
    AffectedScope::CreaturesMatching {
        controller: filter
            .controller
            .map(|AnthemController::YouControl| controller),
        subtype: filter.subtype.clone(),
        color: filter.color,
        exclude: if filter.exclude_self {
            Some(source)
        } else {
            None
        },
    }
}

/// Move `oid` into zone `z`, maintaining every zone list and the CR 400.7 new-object resets.
///
/// `controller` names the player the permanent enters the battlefield **under** (CR 110.2);
/// `None` means "its owner controls it", which is what every non-control-changing caller passes.
/// It is ignored for non-battlefield zones — those belong to the owner (CR 400.3).
pub(crate) fn move_object_to_zone(
    state: &mut GameState,
    oid: ObjectId,
    z: Zone,
    controller: Option<PlayerId>,
) -> Result<(), EngineError> {
    let owner = state
        .objects
        .get(&oid)
        .map(|o| o.owner)
        .ok_or(EngineError::Illegal("no object"))?;
    let old_zone = state.objects.get(&oid).map(|o| o.zone);
    if old_zone != Some(z) {
        *state.zone_change_generation.entry(oid).or_insert(0) += 1;
    }

    // CR 400.7: a zone change creates a new game object. Remove any Single-target continuous
    // effects on this object so they don't apply if the same ObjectId is reused later.
    // CR 604.3 / 611.3: also drain any `WhileSourceOnBattlefield` effects this permanent was the
    // source of (anthems) — a static ability stops applying the moment its source leaves (LTB).
    // One-shot `UntilEndOfTurn` effects (Giant Growth, firebreathing) are deliberately NOT drained
    // here: once created they are independent of their source (CR 611.2g) and only end at cleanup.
    let leaving_battlefield = old_zone == Some(Zone::Battlefield);
    if leaving_battlefield && z != Zone::Battlefield {
        state.continuous_effects.retain(|e| {
            let single_on_this = matches!(&e.affected, AffectedScope::Single(id) if *id == oid);
            let static_from_this =
                e.source_id == Some(oid) && e.duration == EffectDuration::WhileSourceOnBattlefield;
            !single_on_this && !static_from_this
        });
        // CR 400.7 / 121.2: a zone change makes this a new game object — transient
        // battlefield-only state (marked damage, deathtouch marking, tap status, regeneration
        // shields) and all counters do not carry over. Centralized here so every leave path
        // (SBA destroy, sacrifice, bounce, discard, mill, exile) is correct by construction.
        if let Some(o) = state.objects.get_mut(&oid) {
            o.damage = 0;
            o.deathtouch_damage = false;
            // CR 608.2h: snapshot before clearing — an ability still on the stack that asks about
            // this permanent's tap status gets its last known information, not the reset value.
            let was_tapped = o.tapped;
            o.tapped = false;
            o.counters.clear();
            o.attached_to = None;
            o.regeneration_shields = 0;
            let generation = state
                .zone_change_generation
                .get(&oid)
                .copied()
                .unwrap_or(0)
                .saturating_sub(1);
            state
                .last_known_tapped_by_generation
                .insert((oid, generation), was_tapped);
            state.last_known_tapped.insert(oid, was_tapped);
        }
    }

    // Remove from *every* player's lists, not just the owner's: `battlefield` is keyed by
    // controller, so a permanent under someone else's control lives in their vec. Scoping this to
    // the owner would strand a ghost oid that still blocks, still gets SBA-checked, and desyncs
    // the zone-view size check in the relay's `applyRuledEngineZoneView`.
    for p in &mut state.players {
        p.library.retain(|&x| x != oid);
        p.hand.retain(|&x| x != oid);
        p.battlefield.retain(|&x| x != oid);
        p.graveyard.retain(|&x| x != oid);
        p.exile.retain(|&x| x != oid);
    }
    // CR 400.3: the battlefield is entered under a *controller*; every other zone belongs to the
    // card's owner, so that is where a permanent goes when it leaves.
    let holder = if z == Zone::Battlefield {
        controller.unwrap_or(owner)
    } else {
        owner
    };
    let idx = state
        .player_idx(holder)
        .ok_or(EngineError::Illegal("no such player"))?;
    let p = &mut state.players[idx];
    match z {
        Zone::Graveyard => p.graveyard.push(oid),
        Zone::Hand => p.hand.push(oid),
        Zone::Battlefield => p.battlefield.push(oid),
        Zone::Library => p.library.push_back(oid),
        Zone::Exile => p.exile.push(oid),
        Zone::Stack => {}
    }
    if let Some(o) = state.objects.get_mut(&oid) {
        o.zone = z;
        // CR 110.2 / 400.7: control is a battlefield-only property, and a zone change makes this a
        // new object — so entering sets the new controller and leaving resets it to the owner.
        o.controller = if z == Zone::Battlefield {
            holder
        } else {
            o.owner
        };
        // CR 302.6: a permanent entering the battlefield has not been controlled continuously
        // since its controller's most recent turn began, so it is summoning sick. Assert this on
        // entry rather than trusting a persisted flag — a prior bounce/leave clears transient
        // state, so a creature returned to hand and recast (or reanimated/flickered) the same turn
        // must still be sick. Haste exempts the *use* of this (checked at attack/tap time).
        if z == Zone::Battlefield {
            o.summoning_sick = true;
        }
    }
    Ok(())
}

pub(super) fn destroy_permanent(state: &mut GameState, oid: ObjectId) -> Result<(), EngineError> {
    move_object_to_zone(state, oid, Zone::Graveyard, None)
}

/// Sacrifice a permanent (CR 701.17). Unlike destroy, sacrifice bypasses indestructible and
/// regeneration — it is always a cost, never a triggered or replacement effect that can be
/// redirected.
pub(super) fn sacrifice_permanent(state: &mut GameState, oid: ObjectId) -> Result<(), EngineError> {
    move_object_to_zone(state, oid, Zone::Graveyard, None)
}

/// CR 608.2m: "As the final part of an instant or sorcery spell's resolution, the spell is put
/// into its owner's graveyard" — that is, *after* its own effects have been applied. A
/// self-targeted Tome Scour must therefore end up beneath the five cards it milled, not on top
/// of them.
///
/// The spell object is moved out of the stack up front, before its effects run, and that is
/// deliberately left alone: resolution can suspend on a player choice at several points (tier-3
/// custom effects, copy-target, legend-keep, library search), and deferring the move would strand
/// an already-popped stack item in a zone-less limbo on every one of those paths. Instead this
/// re-seats the already-moved card at the back of its owner's graveyard once resolution finishes,
/// which is what graveyard-order-sensitive cards actually read.
///
/// Intentional simplification: the placement *timing* is still early, so an effect that scans its
/// own controller's graveyard mid-resolution can see the resolving spell. No card in the registry
/// does that today; revisit if one lands.
///
/// A no-op unless `oid` is currently in a graveyard and not already last, so it is safe to call
/// on any resolution path, including ones that end with the spell on the battlefield.
pub(super) fn seat_resolved_spell_last_in_graveyard(state: &mut GameState, oid: ObjectId) {
    let Some(owner) = state.objects.get(&oid).map(|o| o.owner) else {
        return;
    };
    let Some(idx) = state.player_idx(owner) else {
        return;
    };
    let graveyard = &mut state.players[idx].graveyard;
    if graveyard.last() == Some(&oid) || !graveyard.contains(&oid) {
        return;
    }
    graveyard.retain(|&x| x != oid);
    graveyard.push(oid);
}

fn counter_label(kind: CounterKind) -> &'static str {
    match kind {
        CounterKind::PlusOnePlusOne => "+1/+1",
        CounterKind::MinusOneMinusOne => "-1/-1",
    }
}

/// Return true if the library card `oid` satisfies `filter` (None = any card).
/// A card in the library shows no face, so any face may satisfy the filter (see
/// [`CardDefinition::any_face`](tricerules_cards::CardDefinition::any_face)).
pub(super) fn library_card_matches_filter(
    state: &GameState,
    registry: &'static CardRegistry,
    oid: ObjectId,
    filter: Option<&SpellTypeFilter>,
) -> bool {
    let Some(filter) = filter else {
        return true;
    };
    let Some(obj) = state.objects.get(&oid) else {
        return false;
    };
    let Some(def) = registry.get(&obj.card_id) else {
        return false;
    };
    match filter {
        SpellTypeFilter::Instant => def.any_face(|f| f.is_instant),
        SpellTypeFilter::Sorcery => def.any_face(|f| f.is_sorcery),
        SpellTypeFilter::InstantOrSorcery => def.any_face(|f| f.is_instant || f.is_sorcery),
        SpellTypeFilter::Creature => def.any_face(|f| f.is_creature),
        SpellTypeFilter::Artifact => def.any_face(|f| f.is_artifact),
        SpellTypeFilter::Enchantment => def.any_face(|f| f.is_enchantment),
        SpellTypeFilter::Noncreature => !def.any_face(|f| f.is_creature),
    }
}

/// Human-readable description of a [`SpellTypeFilter`] for prompt text.
fn spell_type_filter_desc(f: &SpellTypeFilter) -> &'static str {
    match f {
        SpellTypeFilter::Instant => "instant",
        SpellTypeFilter::Sorcery => "sorcery",
        SpellTypeFilter::InstantOrSorcery => "instant or sorcery",
        SpellTypeFilter::Creature => "creature",
        SpellTypeFilter::Artifact => "artifact",
        SpellTypeFilter::Enchantment => "enchantment",
        SpellTypeFilter::Noncreature => "noncreature",
    }
}

/// CR 701.15: attempt to consume one regeneration shield from `oid`. If a shield is present,
/// taps the creature, removes it from combat, clears all marked damage, and returns `true`.
/// The caller is responsible for not destroying the creature. Returns `false` if no shield exists.
/// Does NOT emit a zone-change event (the creature stays on the battlefield).
pub(super) fn consume_regen_shield(
    state: &mut GameState,
    oid: ObjectId,
    events: &mut Vec<rv1::RuledEvent>,
) -> bool {
    let shields = state
        .objects
        .get(&oid)
        .map(|o| o.regeneration_shields)
        .unwrap_or(0);
    if shields == 0 {
        return false;
    }
    // CR 701.15a: regenerating taps the permanent — a real "becomes tapped" edge, so it goes
    // through the shared funnel rather than writing the flag inline.
    super::set_tapped(state, oid, true);
    if let Some(o) = state.objects.get_mut(&oid) {
        o.regeneration_shields -= 1;
        o.damage = 0;
        o.deathtouch_damage = false;
    }
    // CR 701.15a: remove from combat (attacker/blocker lists). This mirrors what happens when
    // a creature is removed from combat by a tap effect.
    if let Some(combat) = state.combat.as_mut() {
        let was_in_combat = combat.attacking.contains(&oid)
            || combat.blockers.contains_key(&oid)
            || combat.blockers.values().any(|v| v.contains(&oid));
        combat.attacking.retain(|&id| id != oid);
        combat.blockers.remove(&oid);
        for v in combat.blockers.values_mut() {
            v.retain(|&id| id != oid);
        }
        if was_in_combat {
            events.push(rv1::RuledEvent {
                ev: Some(rv1::ruled_event::Ev::RemovedFromCombat(
                    rv1::CreaturesRemovedFromCombat {
                        object_ids: vec![oid],
                    },
                )),
            });
        }
    }
    true
}

/// CR 614.1a: consume damage from `shields` for target `tid`, returning the net damage after
/// prevention. Emits a log event for any prevented amount. `amount` is the incoming damage.
pub(super) fn apply_prevention_shield(
    shields: &mut std::collections::HashMap<crate::state::ObjectId, u32>,
    tid: crate::state::ObjectId,
    amount: u32,
    events: &mut Vec<rv1::RuledEvent>,
) -> u32 {
    if amount == 0 {
        return 0;
    }
    let shield = shields.get(&tid).copied().unwrap_or(0);
    if shield == 0 {
        return amount;
    }
    let prevented = amount.min(shield);
    let remaining = shield - prevented;
    if remaining == 0 {
        shields.remove(&tid);
    } else {
        shields.insert(tid, remaining);
    }
    if prevented > 0 {
        events.push(ev_log(format!("{prevented} damage prevented (shield).")));
    }
    amount - prevented
}
