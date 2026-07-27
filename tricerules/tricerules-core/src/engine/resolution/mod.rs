//! Stack resolution orchestration and exhaustive primitive dispatch.
//!
//! Adding a primitive requires one exhaustive `SpellEffectKind` arm below and one implementation
//! in the best-fit domain module. Dispatch arms contain delegation only, so variant coverage stays
//! compiler-checked while resolution logic remains grouped by domain.

use super::events::{color_string, ev_log, object_display_name};
use super::targeting::{
    battlefield_objects_matching, compute_spell_targets, object_matches_mass_filter,
    spell_has_no_legal_targets_at_resolution,
};
use super::*;
use rand::seq::SliceRandom;
use rand::SeedableRng;

mod damage;
mod life;
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
    top: &'a StackItem,
    controller: PlayerId,
    spell_label: &'a str,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum EffectOutcome {
    Continue,
    Suspended,
}

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
            let resolves_to_battlefield_raw = self
                .registry
                .get(&card_id)
                .and_then(|d| d.face(top.face_index))
                .map(|f| f.is_permanent())
                .unwrap_or(false);
            // CR 303.4f: an aura whose enchant target is no longer on the battlefield at resolution
            // is countered (goes to owner's graveyard) rather than entering the battlefield orphaned.
            let is_aura = resolves_to_battlefield_raw
                && self
                    .registry
                    .get(&card_id)
                    .map(|d| d.is_aura)
                    .unwrap_or(false);
            let aura_target_valid = !is_aura
                || targets.first().is_some_and(|&tid| {
                    self.state
                        .objects
                        .get(&tid)
                        .map(|o| o.zone == Zone::Battlefield)
                        .unwrap_or(false)
                });
            let resolves_to_battlefield = resolves_to_battlefield_raw && aura_target_valid;
            let destination = if resolves_to_battlefield {
                rv1::StackResolveDestination::Battlefield as i32
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
                } else {
                    Zone::Graveyard
                },
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

        // Determine effects: for spells use spell_effect (Vec); for abilities wrap the single
        // effect. Triggered and activated abilities are now uniform — both carry a plain
        // `SpellEffectKind` (self-referencing effects use a `Self_` target filter, bound below).
        let (effects, spell_label): (Vec<SpellEffectKind>, String) = if is_ability {
            let ability_index = top.ability_index.unwrap_or(0);
            let def = self.registry.get(&card_id);
            let name = def
                .map(|d| d.name.clone())
                .unwrap_or_else(|| "Ability".into());
            let abilities = if top.is_triggered {
                def.map(|d| &d.triggered_abilities[..])
                    .and_then(|a| a.get(ability_index))
                    .map(|a| a.effect.clone())
            } else {
                def.and_then(|d| d.activated_abilities.get(ability_index))
                    .map(|a| a.effect.clone())
            };
            (vec![abilities.unwrap_or(SpellEffectKind::None)], name)
        } else {
            // CR 709/712/715: resolve the cast face's effects and show its name.
            let face = self
                .registry
                .get(&card_id)
                .and_then(|d| d.face(top.face_index));
            let effects = face.map(|f| f.spell_effect.to_vec()).unwrap_or_default();
            let name = face
                .map(|f| f.name.to_string())
                .unwrap_or_else(|| "Spell".into());
            (effects, name)
        };

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
                .and_then(|f| f.custom_effect.map(str::to_string));
            if let Some(custom_key) = custom_key {
                return self.begin_custom_resolution(top, custom_key, events);
            }
        }

        let fizzle = spell_has_no_legal_targets_at_resolution(
            &self.state,
            self.registry,
            &effects,
            &targets,
            controller,
        );
        if fizzle {
            events.push(ev_log(format!("{spell_label} fizzles (no legal targets).")));
            return Ok(());
        }

        for effect in effects {
            let outcome = {
                let mut cx = EffectCx {
                    engine: self,
                    events,
                    targets: &targets,
                    top: &top,
                    controller,
                    spell_label: &spell_label,
                };
                match effect {
                    effect @ SpellEffectKind::DamageTarget { .. } => {
                        damage::damage_target(&mut cx, effect)?
                    }
                    effect @ SpellEffectKind::DamageTargets { .. } => {
                        damage::damage_targets(&mut cx, effect)?
                    }
                    effect @ SpellEffectKind::Draw { .. } => zones::draw(&mut cx, effect)?,
                    effect @ SpellEffectKind::PumpTarget { .. } => {
                        pump_counters::pump_target(&mut cx, effect)?
                    }
                    effect @ SpellEffectKind::PumpAll { .. } => {
                        pump_counters::pump_all(&mut cx, effect)?
                    }
                    effect @ SpellEffectKind::GrantKeywordsAll { .. } => {
                        pump_counters::grant_keywords_all(&mut cx, effect)?
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
        let is_creature = def.is_creature;
        let power = def.power;
        let toughness = def.toughness;
        let types = def.types.clone();
        let keywords: Vec<String> = def
            .keywords
            .iter()
            .map(|k| k.as_str().to_string())
            .collect();
        let color = color_string(&def.colors());
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
                        owner: pid,
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
    rv1::RuledEvent {
        ev: Some(rv1::ruled_event::Ev::PermanentMoved(rv1::PermanentMoved {
            object_id: oid,
            owner_player_id,
            destination: destination as i32,
            card_id,
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

pub(crate) fn move_object_to_zone(
    state: &mut GameState,
    oid: ObjectId,
    z: Zone,
) -> Result<(), EngineError> {
    let owner = state
        .objects
        .get(&oid)
        .map(|o| o.owner)
        .ok_or(EngineError::Illegal("no object"))?;

    // CR 400.7: a zone change creates a new game object. Remove any Single-target continuous
    // effects on this object so they don't apply if the same ObjectId is reused later.
    // CR 604.3 / 611.3: also drain any `WhileSourceOnBattlefield` effects this permanent was the
    // source of (anthems) — a static ability stops applying the moment its source leaves (LTB).
    // One-shot `UntilEndOfTurn` effects (Giant Growth, firebreathing) are deliberately NOT drained
    // here: once created they are independent of their source (CR 611.2g) and only end at cleanup.
    let leaving_battlefield = state.objects.get(&oid).map(|o| o.zone) == Some(Zone::Battlefield);
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
            o.tapped = false;
            o.counters.clear();
            o.attached_to = None;
            o.regeneration_shields = 0;
        }
    }

    let idx = state.player_idx(owner).unwrap();
    let p = &mut state.players[idx];
    p.library.retain(|&x| x != oid);
    p.hand.retain(|&x| x != oid);
    p.battlefield.retain(|&x| x != oid);
    p.graveyard.retain(|&x| x != oid);
    p.exile.retain(|&x| x != oid);
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
    move_object_to_zone(state, oid, Zone::Graveyard)
}

/// Sacrifice a permanent (CR 701.17). Unlike destroy, sacrifice bypasses indestructible and
/// regeneration — it is always a cost, never a triggered or replacement effect that can be
/// redirected.
pub(super) fn sacrifice_permanent(state: &mut GameState, oid: ObjectId) -> Result<(), EngineError> {
    move_object_to_zone(state, oid, Zone::Graveyard)
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
/// Uses the card's derived type flags from `CardDefinition`.
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
        SpellTypeFilter::Instant => def.is_instant,
        SpellTypeFilter::Sorcery => def.is_sorcery,
        SpellTypeFilter::InstantOrSorcery => def.is_instant || def.is_sorcery,
        SpellTypeFilter::Creature => def.is_creature,
        SpellTypeFilter::Artifact => def.is_artifact,
        SpellTypeFilter::Enchantment => def.is_enchantment,
        SpellTypeFilter::Noncreature => !def.is_creature,
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
    if let Some(o) = state.objects.get_mut(&oid) {
        o.regeneration_shields -= 1;
        o.damage = 0;
        o.deathtouch_damage = false;
        o.tapped = true;
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
