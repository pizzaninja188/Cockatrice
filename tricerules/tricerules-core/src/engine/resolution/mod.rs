//! Stack resolution orchestration and exhaustive primitive dispatch.
//!
//! Adding a primitive requires one exhaustive `SpellEffectKind` arm below and one implementation
//! in the best-fit domain module. Dispatch arms contain delegation only, so variant coverage stays
//! compiler-checked while resolution logic remains grouped by domain.

use super::events::{color_string, ev_log, object_display_name};
use super::targeting::{
    attachment_recipient_for_target, battlefield_objects_matching, compute_spell_targets,
    effect_has_legal_target_at_resolution, graveyard_target_legal, object_matches_mass_filter,
    object_matches_scoped_mass_filter, stack_target_identity_is_current,
    target_filter_legal_at_resolution, target_role_legal_at_resolution, target_schema,
    TargetSourceIdentity,
};
use super::*;
use tricerules_cards::primitives::{ManaRetention, TargetRole, TargetingDef};

mod choices;
pub(super) use choices::resolution_branch_is_live;
pub(in crate::engine) use choices::{card_result_characteristic_sum, card_result_count};
mod amass;
mod blight;
mod damage;
/// `pub(super)` so the combat damage step can reach `life::apply_life_gain` — lifelink is the one
/// life-gain edge outside stack resolution, and it must go through the same funnel.
pub(super) mod life;
mod mass;
mod misc;
mod pump_counters;
mod restrictions;
mod stack_ops;
pub(super) use stack_ops::{counter_stack_object_ref, counter_stack_spell};
mod tokens;
pub(in crate::engine) mod zones;

/// Shared resolution context for one primitive effect.
struct EffectCx<'a> {
    engine: &'a mut GameEngine,
    events: &'a mut Vec<rv1::RuledEvent>,
    targets: &'a [ObjectId],
    targets_by_role: &'a [Vec<ObjectId>],
    target_damage: &'a [u32],
    target_group_indices: &'a [u32],
    top: &'a StackItem,
    controller: PlayerId,
    /// The player an untargeted, player-scoped effect acts on. Equals `controller` for spells,
    /// activated abilities, and every trigger that doesn't name another player; differs only when
    /// a triggered ability says "**that player** …"
    /// ([`StackItem::trigger_context`] — Howling Mine).
    /// Effects that act on the *controller* by rule (Brainstorm's draw, a self-pump) keep using
    /// `controller`.
    affected_player: PlayerId,
    spell_label: &'a str,
    previous_effect_result: &'a EffectResult,
    effect_result: &'a mut EffectResult,
    effect_index: u32,
}

impl EffectCx<'_> {
    fn previous_battlefield_object(&self) -> Option<ObjectId> {
        let selected = self.previous_effect_result.selected_objects.first()?;
        let generation = self
            .engine
            .state
            .zone_change_generation
            .get(&selected.object_id)
            .copied()
            .unwrap_or(0);
        (generation == selected.zone_change_generation
            && self
                .engine
                .state
                .objects
                .get(&selected.object_id)
                .is_some_and(|object| object.zone == Zone::Battlefield))
        .then_some(selected.object_id)
    }

    fn resolve_battlefield_subject(&self, subject: &EffectSubject) -> Option<ObjectId> {
        match subject {
            EffectSubject::PreviousEffectObject => self.previous_battlefield_object(),
            EffectSubject::Chosen(filter) => self.targets.first().copied().filter(|object_id| {
                target_filter_legal_at_resolution(
                    self.engine,
                    filter,
                    *object_id,
                    self.controller,
                    TargetSourceIdentity::for_stack_item(self.engine, self.top),
                    self.top.trigger_context,
                )
            }),
            _ => resolve_effect_subject(self.engine, self.top, self.targets, subject),
        }
    }

    fn resolve_battlefield_subjects(&self, subject: &EffectSubject) -> Vec<ObjectId> {
        match subject {
            EffectSubject::Chosen(filter) => self
                .targets
                .iter()
                .copied()
                .filter(|object_id| {
                    target_filter_legal_at_resolution(
                        self.engine,
                        filter,
                        *object_id,
                        self.controller,
                        TargetSourceIdentity::for_stack_item(self.engine, self.top),
                        self.top.trigger_context,
                    )
                })
                .collect(),
            _ => self
                .resolve_battlefield_subject(subject)
                .into_iter()
                .collect(),
        }
    }

    fn resolve_continuous_subject(
        &self,
        subject: &EffectSubject,
    ) -> Option<(ObjectId, Option<ObjectId>)> {
        let object_id = self.resolve_battlefield_subject(subject)?;
        let source_id = match subject {
            EffectSubject::Source | EffectSubject::AttachedObject => self.top.source_permanent_id,
            EffectSubject::Chosen(_)
            | EffectSubject::TriggerObject
            | EffectSubject::PreviousEffectObject => Some(self.top.id),
        };
        Some((object_id, source_id))
    }
}

impl GameEngine {
    /// CR 707.10f / 608.3f: a resolving permanent spell copy becomes a token. This is
    /// not a token-creation instruction: use normal entry replacements, never creation multipliers.
    fn materialize_permanent_spell_copy(&mut self, item: &StackItem) -> bool {
        if !item.is_copy || item.ability_text.is_some() {
            return false;
        }
        let Some(definition) = self.registry.get(&item.card_id) else {
            return false;
        };
        let Some(face) = definition
            .face(item.face_index)
            .filter(|face| face.is_permanent())
        else {
            return false;
        };
        let values = CopiableValues {
            source_card_id: item.card_id.clone(),
            source_face_index: item.face_index,
            face: face.clone(),
            room_faces: (definition.layout == Layout::Room).then(|| definition.faces.clone()),
            display_name: face.name.clone(),
        };
        let mut object =
            new_object_from_card(item.id, item.controller, &item.card_id, Zone::Stack, face);
        object.face_up_index = item.face_index;
        object.token_origin = Some(values);
        self.state.objects.insert(item.id, object);
        true
    }

    pub(in crate::engine) fn finish_permanent_spell_entry(
        &mut self,
        item: &StackItem,
        events: &mut Vec<RuledEvent>,
    ) {
        self.register_warp_entry(item, item.id);
        if item.is_copy {
            if let Some(object) = self
                .state
                .objects
                .get(&item.id)
                .filter(|o| o.zone == Zone::Battlefield && o.is_token())
            {
                if let Some(values) = self.copiable_values_for(item.id) {
                    events.push(RuledEvent {
                        ev: Some(rv1::ruled_event::Ev::TokenCreated(rv1::TokenCreated {
                            object_id: item.id,
                            controller_player_id: object.controller,
                            card_id: item.card_id.clone(),
                            identity: Some(token_identity(&values)),
                            enters_tapped: object.tapped,
                        })),
                    });
                }
            }
        }
        if let Some(assignment) = item
            .sneak_attack
            .and_then(|assignment| self.add_sneak_attacker(item.id, assignment))
        {
            events.push(RuledEvent {
                ev: Some(rv1::ruled_event::Ev::AttackersAdded(rv1::AttackersAdded {
                    assignments: vec![assignment],
                })),
            });
        }
    }

    pub(in crate::engine) fn grant_exile_play_permission(
        &mut self,
        player_id: PlayerId,
        object_id: ObjectId,
        source_label: &str,
        grant: crate::state::ExilePlayPermissionGrant,
    ) -> Result<u64, EngineError> {
        self.grant_exile_play_permission_group(player_id, &[object_id], source_label, grant)
    }

    pub(in crate::engine) fn grant_exile_play_permission_group(
        &mut self,
        player_id: PlayerId,
        object_ids: &[ObjectId],
        source_label: &str,
        grant: crate::state::ExilePlayPermissionGrant,
    ) -> Result<u64, EngineError> {
        if object_ids.is_empty() {
            return Err(EngineError::Illegal("exile permission cohort is empty"));
        }
        let mut seen = HashSet::new();
        for object_id in object_ids {
            let object = self
                .state
                .objects
                .get(object_id)
                .ok_or(EngineError::Illegal("unknown exile permission object"))?;
            if object.zone != Zone::Exile || !seen.insert(*object_id) {
                return Err(EngineError::Illegal(
                    "permission cohort requires distinct objects in exile",
                ));
            }
        }
        let player_index = self
            .state
            .player_idx(player_id)
            .ok_or(EngineError::UnknownPlayer(player_id))?;
        let expires_at_cleanup_turn_instance = grant.until_end_of_next_turn.then(|| {
            let player_count = self.state.players.len() as u64;
            let active = self.state.active_player_idx;
            let offset = if player_index == active {
                player_count
            } else {
                ((player_index + self.state.players.len() - active) % self.state.players.len())
                    as u64
            };
            self.state.turn_instance.saturating_add(offset)
        });
        let group_id = self.state.next_exile_play_permission_group_id;
        self.state.next_exile_play_permission_group_id = group_id.saturating_add(1);
        let permissions = object_ids
            .iter()
            .map(|object_id| ActiveExilePlayPermission {
                group_id,
                player_id,
                source_label: source_label.to_string(),
                object_id: *object_id,
                zone_change_generation: self
                    .state
                    .zone_change_generation
                    .get(object_id)
                    .copied()
                    .unwrap_or(0),
                scope: grant.scope,
                cast_cost: grant.cast_cost.clone(),
                origin: grant.origin,
                available_after_turn_instance: grant.available_after_turn_instance,
                expires_at_cleanup_turn_instance,
            });
        self.state.active_exile_play_permissions.extend(permissions);
        Ok(group_id)
    }
}

fn target_roles_by_group<'a>(
    effects: &'a [SpellEffectKind],
    targeting: Option<&TargetingDef>,
) -> Vec<Vec<TargetRole<'a>>> {
    target_schema(effects, targeting)
        .groups
        .into_iter()
        .map(|group| {
            group
                .bindings
                .into_iter()
                .map(|binding| binding.role)
                .collect()
        })
        .collect()
}

pub(super) struct TokenCreationRequest<'a> {
    token_id: &'a str,
    values: Option<&'a CopiableValues>,
    count: u32,
    recipients: Vec<PlayerId>,
    spell_label: &'a str,
    item: &'a StackItem,
}

pub(super) fn token_identity(values: &CopiableValues) -> rv1::TokenIdentity {
    let face = &values.face;
    rv1::TokenIdentity {
        name: values.display_name.clone(),
        pt: if face.is_creature {
            format!(
                "{}/{}",
                face.power.unwrap_or(0),
                face.toughness.unwrap_or(0)
            )
        } else {
            String::new()
        },
        color: color_string(&face.colors()),
        types: face.types.clone(),
        is_creature: face.is_creature,
        keywords: face
            .keywords
            .iter()
            .map(|keyword| keyword.as_str().to_string())
            .collect(),
        ability_texts: face
            .activated_abilities
            .iter()
            .map(|ability| ability.fallback_text(&values.display_name))
            .chain(
                face.triggered_abilities
                    .iter()
                    .map(|ability| ability.fallback_text(&values.display_name)),
            )
            .collect(),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum EffectOutcome {
    Continue,
    Blighted(crate::state::BlightReceipt),
    Suspended,
    RestartResolutionBranch(Option<usize>),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DeferredStackExit {
    Resolved,
    DidNotResolve,
}

fn simple_player_recipients(
    state: &GameState,
    controller: PlayerId,
    affected_player: PlayerId,
    trigger_object_controller: Option<PlayerId>,
    source_controller: Option<PlayerId>,
    who: PlayerRecipient,
) -> Vec<PlayerId> {
    match who {
        PlayerRecipient::Controller => vec![controller],
        PlayerRecipient::AffectedPlayer => vec![affected_player],
        PlayerRecipient::TriggerObjectController => trigger_object_controller.into_iter().collect(),
        PlayerRecipient::SourceController => source_controller.into_iter().collect(),
        PlayerRecipient::ControllerOfTargetGroup { .. }
        | PlayerRecipient::DefendingPlayer
        | PlayerRecipient::AttackingOpponentsOfDefendingPlayer => Vec::new(),
        PlayerRecipient::EachOpponent => {
            let mut players = state
                .players
                .iter()
                .filter(|player| state.are_opponents(player.id, controller) && !player.has_lost)
                .map(|player| player.id)
                .collect::<Vec<_>>();
            players.sort_by_key(|player| state.apnap_rank(*player));
            players
        }
        PlayerRecipient::EachPlayer => {
            let mut players = state
                .players
                .iter()
                .filter(|player| !player.has_lost)
                .map(|player| player.id)
                .collect::<Vec<_>>();
            players.sort_by_key(|player| state.apnap_rank(*player));
            players
        }
    }
}

#[cfg(test)]
mod player_recipient_order_tests {
    use super::*;

    #[test]
    fn player_sets_follow_apnap_order_instead_of_storage_order() {
        let mut engine =
            GameEngine::new(121_001, &[10, 20], 20, None, true).expect("two-player engine");
        engine.state.players.push(PlayerState::new(30, 20));
        engine.state.active_player_idx = 1;

        assert_eq!(
            simple_player_recipients(
                &engine.state,
                10,
                10,
                None,
                None,
                PlayerRecipient::EachPlayer,
            ),
            [20, 30, 10]
        );
        assert_eq!(
            simple_player_recipients(
                &engine.state,
                20,
                20,
                None,
                None,
                PlayerRecipient::EachOpponent,
            ),
            [30, 10]
        );
    }
}

fn player_recipients(cx: &EffectCx<'_>, who: PlayerRecipient) -> Vec<PlayerId> {
    match who {
        PlayerRecipient::ControllerOfTargetGroup { group_index } => cx
            .targets
            .iter()
            .zip(cx.target_group_indices)
            .find_map(|(&object_id, &target_group)| {
                (target_group == group_index)
                    .then(|| cx.engine.controller_of(object_id))
                    .flatten()
            })
            .into_iter()
            .collect(),
        PlayerRecipient::DefendingPlayer => cx
            .top
            .trigger_context
            .defending_player
            .filter(|player_id| {
                cx.engine
                    .state
                    .player_idx(*player_id)
                    .is_some_and(|idx| !cx.engine.state.players[idx].has_lost)
            })
            .into_iter()
            .collect(),
        PlayerRecipient::AttackingOpponentsOfDefendingPlayer => {
            let Some((attacking_player, defending_player)) = cx
                .top
                .trigger_context
                .attacking_player
                .zip(cx.top.trigger_context.defending_player)
            else {
                return Vec::new();
            };
            let attacker_is_eligible = cx
                .engine
                .state
                .player_idx(attacking_player)
                .is_some_and(|idx| !cx.engine.state.players[idx].has_lost)
                && cx
                    .engine
                    .state
                    .are_opponents(attacking_player, defending_player)
                && cx.engine.state.combat.as_ref().is_some_and(|combat| {
                    combat.attacking.iter().any(|&object_id| {
                        cx.engine
                            .state
                            .objects
                            .get(&object_id)
                            .is_some_and(|object| object.zone == Zone::Battlefield)
                            && cx.engine.controller_of(object_id) == Some(attacking_player)
                    })
                });
            attacker_is_eligible
                .then_some(attacking_player)
                .into_iter()
                .collect()
        }
        _ => simple_player_recipients(
            &cx.engine.state,
            cx.controller,
            cx.affected_player,
            trigger_object_controller(cx.engine, cx.top),
            source_controller(cx.engine, cx.top),
            who,
        ),
    }
}

fn source_controller(engine: &GameEngine, top: &StackItem) -> Option<PlayerId> {
    let source_id = top.source_permanent_id?;
    if engine.source_is_current_object(top) {
        engine.controller_of(source_id)
    } else {
        engine
            .state
            .last_known_controller_by_generation
            .get(&(source_id, top.source_zone_change))
            .copied()
    }
}

fn trigger_object_controller(engine: &GameEngine, top: &StackItem) -> Option<PlayerId> {
    let trigger_object = top.trigger_context.observed_object?;
    let is_current = engine
        .state
        .zone_change_generation
        .get(&trigger_object.object_id)
        .copied()
        .unwrap_or(0)
        == trigger_object.zone_change_generation
        && engine
            .state
            .objects
            .get(&trigger_object.object_id)
            .is_some_and(|object| object.zone == Zone::Battlefield);
    if is_current {
        engine.controller_of(trigger_object.object_id)
    } else {
        // Control can change between the observed event and departure. CR 608.2h reads the
        // last controller of that occurrence, never a new occurrence with the same ObjectId.
        Some(
            engine
                .state
                .last_known_controller_by_generation
                .get(&(
                    trigger_object.object_id,
                    trigger_object.zone_change_generation,
                ))
                .copied()
                .unwrap_or(trigger_object.controller_at_event),
        )
    }
}

/// Resolve a permanent-valued effect subject without turning source or attachment references
/// into CR 115 targets. Attachment references read the source's current relation when possible;
/// if the source left, CR 608.2h/113.7a last-known information preserves the old relation while
/// both source and attached-object generations prevent leave-and-return identity leaks.
fn resolve_effect_subject(
    engine: &GameEngine,
    top: &StackItem,
    targets: &[ObjectId],
    subject: &EffectSubject,
) -> Option<ObjectId> {
    match subject {
        EffectSubject::Source => top
            .source_permanent_id
            .filter(|_| engine.source_is_current_object(top)),
        EffectSubject::Chosen(_) => targets.first().copied(),
        EffectSubject::PreviousEffectObject => None,
        EffectSubject::TriggerObject => {
            let trigger_object = top.trigger_context.observed_object?;
            let current_generation = engine
                .state
                .zone_change_generation
                .get(&trigger_object.object_id)
                .copied()
                .unwrap_or(0);
            (current_generation == trigger_object.zone_change_generation
                && engine
                    .state
                    .objects
                    .get(&trigger_object.object_id)
                    .is_some_and(|object| object.zone == Zone::Battlefield))
            .then_some(trigger_object.object_id)
        }
        EffectSubject::AttachedObject => {
            let source_oid = top.source_permanent_id?;
            let (target_oid, expected_generation) = super::targeting::attached_object_identity(
                &engine.state,
                source_oid,
                top.source_zone_change,
            )?;
            let current_generation = engine
                .state
                .zone_change_generation
                .get(&target_oid)
                .copied()
                .unwrap_or(0);
            (current_generation == expected_generation
                && engine
                    .state
                    .objects
                    .get(&target_oid)
                    .is_some_and(|object| object.zone == Zone::Battlefield))
            .then_some(target_oid)
        }
    }
}

/// Resolve a subject for a zone move. Source-bound zone actions may originate from a declared
/// public nonbattlefield zone, but must still match the exact generation captured on the stack.
/// Other effect families keep their existing battlefield-only Source contract.
fn resolve_zone_effect_subject(
    engine: &GameEngine,
    top: &StackItem,
    targets: &[ObjectId],
    subject: &EffectSubject,
) -> Option<ObjectId> {
    if !matches!(subject, EffectSubject::Source) {
        return resolve_effect_subject(engine, top, targets, subject);
    }
    let source_id = top.source_permanent_id?;
    let expected_zone = match top
        .activated_ability
        .as_ref()
        .map(|ability| ability.source_zone)
        .unwrap_or_default()
    {
        AbilitySourceZone::Battlefield => Zone::Battlefield,
        AbilitySourceZone::Hand => Zone::Hand,
        AbilitySourceZone::Graveyard => Zone::Graveyard,
    };
    (engine
        .state
        .zone_change_generation
        .get(&source_id)
        .copied()
        .unwrap_or(0)
        == top.source_zone_change
        && engine
            .state
            .objects
            .get(&source_id)
            .is_some_and(|object| object.zone == expected_zone))
    .then_some(source_id)
}

/// One entry of a resolving stack item's flattened effect list. Target group identities stay
/// attached so one primitive can consume multiple independently filtered roles.
pub(super) struct ResolutionEffect {
    effect: SpellEffectKind,
    targets: Vec<ObjectId>,
    target_damage: Vec<u32>,
    target_group_indices: Vec<u32>,
    /// Absolute authored group index for each target-filter role, ordered by filter index.
    role_group_indices: Vec<u32>,
}

fn resolving_damage_source_id(item: &StackItem) -> ObjectId {
    item.source_permanent_id.unwrap_or(item.id)
}

impl GameEngine {
    /// Whether the resolving spell or ability's damage source has `keyword` now, or had it as
    /// last known information before leaving the battlefield. Kept generic so all future
    /// source-characteristic damage results (lifelink, infect, wither) share this identity path.
    pub(super) fn resolving_source_has_keyword(&self, top: &StackItem, keyword: Keyword) -> bool {
        let Some(source_id) = top.source_permanent_id else {
            // A spell (and a copy of one) uses the characteristics of the selected face on the
            // stack. Copies have no backing GameObject, so the card definition is authoritative.
            return self
                .registry
                .get(&top.card_id)
                .and_then(|definition| definition.face(top.face_index))
                .is_some_and(|face| face.keywords.contains(&keyword));
        };

        let current_generation = self
            .state
            .zone_change_generation
            .get(&source_id)
            .copied()
            .unwrap_or(0);
        let source_is_same_battlefield_object = current_generation == top.source_zone_change
            && self
                .state
                .objects
                .get(&source_id)
                .is_some_and(|object| object.zone == Zone::Battlefield);
        if source_is_same_battlefield_object {
            return self.effective_has_keyword(source_id, keyword);
        }

        self.state
            .last_known_keywords_by_generation
            .get(&(source_id, top.source_zone_change))
            .is_some_and(|keywords| keywords.contains(&keyword))
    }

    /// CR 608.2b target snapshot. Writing the filtered grouped records back to this local stack
    /// item also preserves the snapshot if a primitive parks and later resumes its effect tail.
    fn snapshot_resolution_targets(&self, top: &mut StackItem) {
        let source = TargetSourceIdentity::for_stack_item(self, top);
        let controller = top.controller;
        let legal = |requirements: &[TargetRole<'_>], target: &StackTarget| {
            stack_target_identity_is_current(self, target)
                && requirements.iter().all(|&role| {
                    target_role_legal_at_resolution(
                        self,
                        role,
                        target.object_id,
                        controller,
                        source,
                        top.trigger_context,
                    )
                })
        };

        let face = self
            .registry
            .get(&top.card_id)
            .and_then(|definition| definition.face(top.face_index));
        if !top.chosen_modes.is_empty() {
            let Some(modal) = top
                .triggered_ability
                .as_ref()
                .and_then(|ability| ability.modal.as_ref())
                .or_else(|| face.and_then(|face| face.modal_spell.as_ref()))
            else {
                return;
            };
            for chosen in &mut top.chosen_modes {
                let Some(mode) = modal.mode_by_id(&chosen.mode_id) else {
                    continue;
                };
                let requirements = target_roles_by_group(&mode.effects, mode.targeting.as_ref());
                chosen.targets.retain(|target| {
                    requirements
                        .get(target.group_index as usize)
                        .is_some_and(|group| legal(group, target))
                });
            }
            return;
        }

        let (effects, targeting) = if top.ability_text.is_some() {
            if top.is_triggered {
                let Some(ability) = top.triggered_ability.as_ref().or_else(|| {
                    face.and_then(|face| {
                        face.triggered_abilities.get(top.ability_index.unwrap_or(0))
                    })
                }) else {
                    return;
                };
                (&ability.effect[..], ability.targeting.as_ref())
            } else {
                let Some(ability) = top.activated_ability.as_ref().or_else(|| {
                    face.and_then(|face| {
                        face.activated_abilities.get(top.ability_index.unwrap_or(0))
                    })
                }) else {
                    return;
                };
                (&ability.effect[..], ability.targeting.as_ref())
            }
        } else {
            let Some(face) = face else {
                return;
            };
            (&face.spell_effect[..], face.targeting.as_ref())
        };
        let requirements = target_roles_by_group(effects, targeting);
        top.targets.retain(|target| {
            requirements
                .get(target.group_index as usize)
                .is_some_and(|group| legal(group, target))
        });
    }

    pub(super) fn resolve_top_of_stack(
        &mut self,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<(), EngineError> {
        let mut top = self
            .state
            .stack
            .pop()
            .ok_or(EngineError::Illegal("empty stack"))?;
        // Preserve whether the caster actually chose any targets before CR 608.2b removes
        // targets that have become illegal. This distinguishes an optional-target spell cast
        // with zero targets (which resolves normally) from one whose chosen targets all became
        // illegal (which does not resolve).
        let had_chosen_targets =
            !top.targets.is_empty() || top.chosen_modes.iter().any(|mode| !mode.targets.is_empty());
        self.snapshot_resolution_targets(&mut top);
        let controller = top.controller;
        let card_id = top.card_id.clone();
        let targets = top
            .targets
            .iter()
            .map(|target| target.object_id)
            .collect::<Vec<_>>();

        // Abilities — and spell copies (CR 707.10d) — leave no object behind when they resolve;
        // only a genuinely cast spell has a backing card that moves to a zone. A copy has no
        // `GameObject` in `objects`, so it must take the same no-zone-move path as an ability.
        let is_ability = top.ability_text.is_some();
        let permanent_copy = self.materialize_permanent_spell_copy(&top);
        let leaves_no_object = is_ability || (top.is_copy && !permanent_copy);
        let is_omen_spell = self
            .registry
            .get(&card_id)
            .is_some_and(|definition| definition.layout == Layout::Omen && top.face_index == 1);
        let custom_key = (!is_ability && !top.is_copy)
            .then(|| {
                self.registry
                    .get(&card_id)
                    .and_then(|definition| definition.face(top.face_index))
                    .and_then(|face| face.custom_effect.clone())
            })
            .flatten();
        let defer_soft_counter_exit = self.build_resolution_effects(&top).0.iter().any(|entry| {
            matches!(
                entry.effect,
                SpellEffectKind::CounterTargetSpell {
                    unless_controller_pays: Some(_),
                    ..
                } | SpellEffectKind::CounterTargetSpell {
                    unless_controller_pays_by_cast_cost: Some(_),
                    ..
                } | SpellEffectKind::CounterTriggeringStackObjectUnlessPays { .. }
            )
        });
        if leaves_no_object && !defer_soft_counter_exit && !is_omen_spell {
            events.push(rv1::RuledEvent {
                ev: Some(rv1::ruled_event::Ev::StackResolved(rv1::StackResolved {
                    object_id: top.id,
                    // Abilities cease to exist on resolution; graveyard tells the C++ server
                    // not to expect a permanent to land.
                    destination: rv1::StackResolveDestination::Graveyard as i32,
                    owner_player_id: None,
                })),
            });
        } else if !leaves_no_object {
            // CR 709/712/715: permanence is the *cast face's* (Ice resolves to graveyard; an MDFC
            // permanent face resolves to the battlefield as that face).
            let resolving_face = self
                .registry
                .get(&card_id)
                .and_then(|d| d.face(top.face_index));
            let is_adventure_spell = self.registry.get(&card_id).is_some_and(|definition| {
                definition.layout == Layout::Adventure && top.face_index == 1
            });
            // CR 715.3d applies only when the Adventure actually resolves. The ordinary fizzle
            // check occurs later for every spell; preflight it here as well because destination is
            // chosen before effects run and an all-illegal-target Adventure must go to graveyard.
            let adventure_fizzles = if is_adventure_spell {
                let (effects, _) = self.build_resolution_effects(&top);
                let targeted: Vec<_> = effects
                    .iter()
                    .filter(|entry| !entry.role_group_indices.is_empty())
                    .collect();
                !targeted.is_empty() && targeted.iter().all(|entry| entry.targets.is_empty())
            } else {
                false
            };
            let adventure_resolves_to_exile = is_adventure_spell && !adventure_fizzles;
            let resolves_to_battlefield_raw =
                resolving_face.map(|f| f.is_permanent()).unwrap_or(false);
            // A data-driven instant or sorcery may suspend for a mid-resolution choice. Keep its
            // physical card on the stack until the whole effect list completes; immediate
            // resolutions still publish the same final batch, while parked resolutions now keep
            // the visible stack faithful to CR 608.2m. Adventure and tier-3 custom spells retain
            // their specialized exit ownership below.
            let defer_authored_nonpermanent_exit =
                !resolves_to_battlefield_raw && !is_adventure_spell && custom_key.is_none();
            // CR 303.4f: an aura whose enchant target is no longer on the battlefield at resolution
            // is countered (goes to owner's graveyard) rather than entering the battlefield orphaned.
            let is_aura =
                resolves_to_battlefield_raw && resolving_face.map(|f| f.is_aura).unwrap_or(false);
            let aura_filter = is_aura
                .then(|| {
                    resolving_face.and_then(|face| {
                        face.spell_effect.iter().find_map(|effect| match effect {
                            SpellEffectKind::AuraAttach { target } => Some(target),
                            _ => None,
                        })
                    })
                })
                .flatten();
            let aura_recipient = aura_filter.and_then(|filter| {
                targets.first().copied().and_then(|target| {
                    effect_has_legal_target_at_resolution(
                        self,
                        &SpellEffectKind::AuraAttach {
                            target: filter.clone(),
                        },
                        &targets,
                        controller,
                        TargetSourceIdentity::for_stack_item(self, &top),
                    )
                    .then(|| attachment_recipient_for_target(filter, target))
                    .flatten()
                })
            });
            let aura_target_valid = !is_aura || aura_recipient.is_some();
            // CR 702.34a: a spell cast with flashback is exiled instead of being put into its
            // owner's graveyard as it leaves the stack, regardless of whether it would normally
            // be a permanent spell.
            let resolves_to_battlefield = !top.cast_method.exiles_on_leave_stack()
                && resolves_to_battlefield_raw
                && aura_target_valid;
            let destination = if resolves_to_battlefield {
                rv1::StackResolveDestination::Battlefield as i32
            } else if top.cast_method.exiles_on_leave_stack() || adventure_resolves_to_exile {
                rv1::StackResolveDestination::Exile as i32
            } else {
                rv1::StackResolveDestination::Graveyard as i32
            };
            if resolves_to_battlefield {
                let attached_to = aura_recipient;
                match self.begin_battlefield_entry(
                    top.clone(),
                    BattlefieldEntryEvent {
                        object_id: top.id,
                        deciding_player: top.controller,
                        destination_controller: top.controller,
                        battle_protector: None,
                        face_index: top.face_index,
                        unlock_room_door: Some(top.face_index),
                        chosen_x: top.chosen_x,
                        cast_cost_receipts: top.cast_cost_receipts.clone(),
                        player_life_snapshot: self.player_life_snapshot(),
                        tapped: top.cast_method == SpellCastMethod::Sneak,
                        entry_counters: BTreeMap::new(),
                        applied_effects: Vec::new(),
                    },
                    BattlefieldEntryCompletion::PermanentSpell { attached_to },
                    events,
                ) {
                    super::replacement::BattlefieldEntryProgress::Parked => return Ok(()),
                    super::replacement::BattlefieldEntryProgress::Ready(entry) => {
                        events.push(rv1::RuledEvent {
                            ev: Some(rv1::ruled_event::Ev::StackResolved(rv1::StackResolved {
                                object_id: top.id,
                                destination,
                                owner_player_id: self
                                    .state
                                    .objects
                                    .get(&top.id)
                                    .map(|object| object.owner),
                            })),
                        });
                        self.commit_battlefield_entry(entry, attached_to)?;
                        self.finish_permanent_spell_entry(&top, events);
                    }
                }
            } else if !defer_soft_counter_exit && !defer_authored_nonpermanent_exit {
                events.push(rv1::RuledEvent {
                    ev: Some(rv1::ruled_event::Ev::StackResolved(rv1::StackResolved {
                        object_id: top.id,
                        destination,
                        owner_player_id: self.state.objects.get(&top.id).map(|object| object.owner),
                    })),
                });
                let zone = if top.cast_method.exiles_on_leave_stack() || adventure_resolves_to_exile
                {
                    Zone::Exile
                } else {
                    Zone::Graveyard
                };
                if custom_key.is_some() {
                    let occurrence = crate::state::StackObjectRef {
                        object_id: top.id,
                        zone_change_generation: Some(
                            self.state
                                .zone_change_generation
                                .get(&top.id)
                                .copied()
                                .unwrap_or(0),
                        ),
                    };
                    let fact = move_object_to_zone_with_entry_receipt(
                        &mut self.state,
                        self.registry,
                        top.id,
                        zone,
                        None,
                    )?;
                    self.state.deferred_graveyard_entry = fact.map(|entry| (occurrence, entry));
                } else {
                    move_object_to_zone(&mut self.state, self.registry, top.id, zone, None)?;
                }
            }
            if adventure_resolves_to_exile {
                let source_label = self
                    .registry
                    .get(&card_id)
                    .map(|definition| definition.name.clone())
                    .unwrap_or_else(|| "Adventure".to_string());
                self.grant_exile_play_permission(
                    top.controller,
                    top.id,
                    &source_label,
                    crate::state::ExilePlayPermissionGrant::printed(
                        ExilePlayPermissionScope::CastFace(0),
                        false,
                    ),
                )?;
            }
            if !resolves_to_battlefield && is_aura {
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
            if let Some(custom_key) = custom_key {
                return self.begin_custom_resolution(top, custom_key, events);
            }
        }

        let (resolution_effects, spell_label) = self.build_resolution_effects(&top);

        // CR 603.4, second of the two checks: a triggered ability with an intervening-"if" clause
        // does nothing if the clause is false as it resolves, even though it was true when the
        // ability triggered (Howling Mine tapped in response to its own trigger).
        if top.is_triggered {
            let clause = top
                .triggered_ability
                .as_ref()
                .or_else(|| {
                    self.registry
                        .get(&card_id)
                        .and_then(|d| d.face(top.face_index))
                        .and_then(|f| f.triggered_abilities.get(top.ability_index.unwrap_or(0)))
                })
                .and_then(|ta| ta.intervening_if.as_ref());
            let source_id = top.source_permanent_id.unwrap_or(top.id);
            let holds = if top.source_permanent_id.is_some() {
                self.intervening_if_holds_at_generation(
                    source_id,
                    top.controller,
                    clause,
                    Some(top.source_zone_change),
                    Some(&top.trigger_context),
                )
            } else {
                self.intervening_if_holds_at_generation(
                    source_id,
                    top.controller,
                    clause,
                    None,
                    Some(&top.trigger_context),
                )
            };
            if !holds {
                events.push(ev_log(format!(
                    "{spell_label} does nothing (its \"if\" condition is no longer true, CR 603.4)."
                )));
                self.finish_deferred_stack_exit(&top, DeferredStackExit::DidNotResolve, events)?;
                return Ok(());
            }
        }

        // CR 608.2b: targets are checked once, at the start of resolution — not again on resume.
        let targeted_effects: Vec<_> = resolution_effects
            .iter()
            .filter(|entry| !entry.role_group_indices.is_empty())
            .collect();
        let fizzle = had_chosen_targets
            && !targeted_effects.is_empty()
            && targeted_effects
                .iter()
                .all(|entry| entry.targets.is_empty());
        if fizzle {
            events.push(ev_log(format!("{spell_label} fizzles (no legal targets).")));
            self.finish_deferred_stack_exit(&top, DeferredStackExit::DidNotResolve, events)?;
            return Ok(());
        }

        self.run_effect_list(&top, &spell_label, resolution_effects, 0, events)
    }

    /// Complete a stack exit deferred until the data-driven effect list finishes. This keeps an
    /// instant or sorcery visible on the physical stack throughout any resolution-time choice.
    /// Permanent, Adventure, and tier-3 custom spells retain their specialized exit paths and
    /// make this a no-op after they have already moved.
    fn finish_deferred_stack_exit(
        &mut self,
        top: &StackItem,
        exit: DeferredStackExit,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<(), EngineError> {
        let has_stack_item = self.state.stack.iter().any(|item| item.id == top.id);
        let has_stack_object = self
            .state
            .objects
            .get(&top.id)
            .is_some_and(|object| object.zone == Zone::Stack);
        let is_omen_spell = self
            .registry
            .get(&top.card_id)
            .is_some_and(|definition| definition.layout == Layout::Omen && top.face_index == 1);
        // A spell copy has neither a backing object nor a second stack entry after
        // `resolve_top_of_stack` pops it. Omen copies still need the typed Library resolution
        // event and their controller's deterministic shuffle, so they are the sole no-object
        // deferred exit that reaches the completion path.
        let has_deferred_exit =
            has_stack_item || has_stack_object || (top.is_copy && is_omen_spell);
        if !has_deferred_exit {
            return Ok(());
        }

        self.state.stack.retain(|item| item.id != top.id);
        let is_resolved_omen = exit == DeferredStackExit::Resolved && is_omen_spell;
        let physical_owner = self.state.objects.get(&top.id).map(|object| object.owner);
        let shuffle_player = physical_owner.unwrap_or(top.controller);
        let destination = if is_resolved_omen {
            rv1::StackResolveDestination::Library
        } else if top.cast_method.exiles_on_leave_stack() {
            rv1::StackResolveDestination::Exile
        } else {
            rv1::StackResolveDestination::Graveyard
        };
        events.push(rv1::RuledEvent {
            ev: Some(rv1::ruled_event::Ev::StackResolved(rv1::StackResolved {
                object_id: top.id,
                destination: destination as i32,
                owner_player_id: physical_owner,
            })),
        });
        if has_stack_object {
            move_object_to_zone(
                &mut self.state,
                self.registry,
                top.id,
                if is_resolved_omen {
                    Zone::Library
                } else if top.cast_method.exiles_on_leave_stack() {
                    Zone::Exile
                } else {
                    Zone::Graveyard
                },
                None,
            )?;
        }
        if is_resolved_omen {
            shuffle_player_library_for_current_command(&mut self.state, shuffle_player);
            events.push(ev_log(format!(
                "P{} shuffles the resolving Omen into P{}'s library.",
                top.controller, shuffle_player
            )));
        }
        Ok(())
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
        // Self-referencing effects use `EffectSubject::Source` and bind during effect dispatch.
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
                top.triggered_ability
                    .as_ref()
                    .or_else(|| face.and_then(|f| f.triggered_abilities.get(ability_index)))
                    .map(|a| a.effect.clone())
            } else {
                top.activated_ability
                    .as_ref()
                    .or_else(|| face.and_then(|f| f.activated_abilities.get(ability_index)))
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

        let build_entries = |effects: &[SpellEffectKind],
                             targeting: Option<&TargetingDef>,
                             chosen_targets: &[StackTarget]| {
            let schema = target_schema(effects, targeting);
            effects
                .iter()
                .cloned()
                .enumerate()
                .map(|(effect_index, effect)| {
                    let mut role_groups = schema
                        .groups
                        .iter()
                        .enumerate()
                        .flat_map(|(group_index, group)| {
                            group.bindings.iter().filter_map(move |binding| {
                                (binding.effect_index == effect_index)
                                    .then_some((binding.role_index, group_index as u32))
                            })
                        })
                        .collect::<Vec<_>>();
                    role_groups.sort_by_key(|(filter_index, _)| *filter_index);
                    let role_group_indices = role_groups
                        .into_iter()
                        .map(|(_, group_index)| group_index)
                        .collect::<Vec<_>>();
                    let selected = chosen_targets
                        .iter()
                        .filter(|target| {
                            role_group_indices.is_empty()
                                || role_group_indices.contains(&target.group_index)
                        })
                        .collect::<Vec<_>>();
                    ResolutionEffect {
                        effect,
                        targets: selected.iter().map(|target| target.object_id).collect(),
                        target_damage: selected.iter().map(|target| target.damage_amount).collect(),
                        target_group_indices: selected
                            .iter()
                            .map(|target| target.group_index)
                            .collect(),
                        role_group_indices,
                    }
                })
                .collect::<Vec<_>>()
        };

        let mut resolution_effects: Vec<ResolutionEffect> = Vec::new();
        if !top.chosen_modes.is_empty() {
            let modal = if top.is_triggered {
                top.triggered_ability
                    .as_ref()
                    .and_then(|ability| ability.modal.as_ref())
            } else if !is_ability {
                self.registry
                    .get(card_id)
                    .and_then(|definition| definition.face(top.face_index))
                    .and_then(|face| face.modal_spell.as_ref())
            } else {
                None
            };
            if let Some(modal) = modal {
                for chosen in &top.chosen_modes {
                    if let Some(mode) = modal.mode_by_id(&chosen.mode_id) {
                        resolution_effects.extend(build_entries(
                            &mode.effects,
                            mode.targeting.as_ref(),
                            &chosen.targets,
                        ));
                    }
                }
            }
        } else {
            let targeting = if is_ability {
                let face = self
                    .registry
                    .get(card_id)
                    .and_then(|definition| definition.face(top.face_index));
                if top.is_triggered {
                    top.triggered_ability
                        .as_ref()
                        .or_else(|| {
                            face.and_then(|face| {
                                face.triggered_abilities.get(top.ability_index.unwrap_or(0))
                            })
                        })
                        .and_then(|ability| ability.targeting.as_ref())
                } else {
                    top.activated_ability
                        .as_ref()
                        .or_else(|| {
                            face.and_then(|face| {
                                face.activated_abilities.get(top.ability_index.unwrap_or(0))
                            })
                        })
                        .and_then(|ability| ability.targeting.as_ref())
                }
            } else {
                self.registry
                    .get(card_id)
                    .and_then(|definition| definition.face(top.face_index))
                    .and_then(|face| face.targeting.as_ref())
            };
            resolution_effects.extend(build_entries(&effects, targeting, &top.targets));
        }

        if !top.resolution_branch_choices.is_empty() {
            let mut expanded = Vec::new();
            for (effect_index, entry) in resolution_effects.into_iter().enumerate() {
                if let SpellEffectKind::ChooseResolutionBranch { branches, .. } = &entry.effect {
                    if let Some(choice) = top.resolution_branch_choices.get(&(effect_index as u32))
                    {
                        if let Some(branch_index) = choice {
                            if let Some(branch) = branches.get(*branch_index) {
                                expanded.extend(build_entries(&branch.effects, None, &[]));
                            }
                        }
                        continue;
                    }
                }
                expanded.push(entry);
            }
            resolution_effects = expanded;
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
        self.run_effect_list_with_previous(
            top,
            spell_label,
            resolution_effects,
            start,
            EffectResult::default(),
            events,
        )
    }

    pub(super) fn run_effect_list_with_previous(
        &mut self,
        top: &StackItem,
        spell_label: &str,
        resolution_effects: Vec<ResolutionEffect>,
        start: usize,
        mut previous_effect_result: EffectResult,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<(), EngineError> {
        let controller = top.controller;
        for (index, entry) in resolution_effects.into_iter().enumerate().skip(start) {
            let ResolutionEffect {
                effect,
                targets: effect_targets,
                target_damage: effect_target_damage,
                target_group_indices,
                role_group_indices,
            } = entry;
            if !role_group_indices.is_empty() && effect_targets.is_empty() {
                previous_effect_result = EffectResult::default();
                continue;
            }
            let targets_by_role = role_group_indices
                .iter()
                .map(|group_index| {
                    effect_targets
                        .iter()
                        .zip(&target_group_indices)
                        .filter_map(|(&target, target_group)| {
                            (target_group == group_index).then_some(target)
                        })
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>();
            let mut effect_result = EffectResult::default();
            let outcome = {
                let mut cx = EffectCx {
                    engine: self,
                    events,
                    targets: &effect_targets,
                    targets_by_role: &targets_by_role,
                    target_damage: &effect_target_damage,
                    target_group_indices: &target_group_indices,
                    top,
                    controller,
                    affected_player: top.trigger_context.affected_player.unwrap_or(controller),
                    spell_label,
                    previous_effect_result: &previous_effect_result,
                    effect_result: &mut effect_result,
                    effect_index: index as u32,
                };
                match effect {
                    SpellEffectKind::Conditional { condition, effect } => {
                        if !cx
                            .engine
                            .condition_holds(&condition, ConditionContext::for_stack_item(cx.top))
                        {
                            EffectOutcome::Continue
                        } else {
                            match *effect {
                                effect @ SpellEffectKind::Destroy { .. } => {
                                    misc::destroy(&mut cx, effect)?
                                }
                                effect @ SpellEffectKind::GrantKeywords { .. } => {
                                    pump_counters::grant_keywords(&mut cx, effect)?
                                }
                                effect @ SpellEffectKind::ChoosePermanents { .. } => {
                                    choices::choose_permanents(&mut cx, effect)?
                                }
                                effect @ SpellEffectKind::Draw { .. } => {
                                    zones::draw(&mut cx, effect)?
                                }
                                _ => {
                                    return Err(EngineError::Illegal(
                                        "unsupported conditional inner effect",
                                    ));
                                }
                            }
                        }
                    }
                    SpellEffectKind::ConditionalCastCost { condition, effect } => {
                        if !cx.top.cast_cost_condition_matches(&condition) {
                            EffectOutcome::Continue
                        } else {
                            match *effect {
                                effect @ SpellEffectKind::PumpTarget { .. } => {
                                    pump_counters::pump_target(&mut cx, effect)?
                                }
                                effect @ SpellEffectKind::GainLife { .. } => {
                                    life::gain_life(&mut cx, effect)?
                                }
                                _ => {
                                    return Err(EngineError::Illegal(
                                        "unsupported cast-cost conditional inner effect",
                                    ));
                                }
                            }
                        }
                    }
                    effect @ SpellEffectKind::DamageTarget { .. } => {
                        damage::damage_target(&mut cx, effect)?
                    }
                    effect @ SpellEffectKind::ExileIfWouldDieThisTurn { .. } => {
                        zones::exile_if_would_die_this_turn(&mut cx, effect)?
                    }
                    effect @ SpellEffectKind::CreatureDealsDamageEqualToPower { .. } => {
                        damage::creature_deals_damage_equal_to_power(&mut cx, effect)?
                    }
                    effect @ SpellEffectKind::Fight { .. } => damage::fight(&mut cx, effect)?,
                    effect @ SpellEffectKind::DamageTargets { .. } => {
                        damage::damage_targets(&mut cx, effect)?
                    }
                    effect @ SpellEffectKind::DamagePlayer { .. } => {
                        damage::damage_player(&mut cx, effect)?
                    }
                    effect @ SpellEffectKind::DamageAttackedPlayerOrPlaneswalker { .. } => {
                        damage::damage_attacked_player_or_planeswalker(&mut cx, effect)?
                    }
                    effect @ SpellEffectKind::Draw { .. } => zones::draw(&mut cx, effect)?,
                    effect @ SpellEffectKind::TargetPlayerDraws { .. } => {
                        zones::target_player_draws(&mut cx, effect)?
                    }
                    effect @ SpellEffectKind::Discard { .. } => zones::discard(&mut cx, effect)?,
                    effect @ SpellEffectKind::DrawDiscard { .. } => {
                        zones::draw_discard(&mut cx, effect)?
                    }
                    effect @ SpellEffectKind::Scry { .. } => zones::scry(&mut cx, effect)?,
                    effect @ SpellEffectKind::LibraryPartition { .. } => {
                        zones::library_partition(&mut cx, effect)?
                    }
                    SpellEffectKind::ManifestDread => zones::manifest_dread(&mut cx)?,
                    effect @ SpellEffectKind::LookChooseToHand { .. } => {
                        zones::look_choose_to_hand(&mut cx, effect)?
                    }
                    effect @ SpellEffectKind::PumpTarget { .. } => {
                        pump_counters::pump_target(&mut cx, effect)?
                    }
                    effect @ SpellEffectKind::SetBasePowerToughness { .. } => {
                        pump_counters::set_base_power_toughness(&mut cx, effect)?
                    }
                    effect @ SpellEffectKind::PumpAll { .. } => {
                        pump_counters::pump_all(&mut cx, effect)?
                    }
                    effect @ SpellEffectKind::GrantKeywordsAll { .. } => {
                        pump_counters::grant_keywords_all(&mut cx, effect)?
                    }
                    effect @ SpellEffectKind::RemoveAbilitiesAll { .. } => {
                        pump_counters::remove_abilities_all(&mut cx, effect)?
                    }
                    effect @ SpellEffectKind::GrantKeywords { .. } => {
                        pump_counters::grant_keywords(&mut cx, effect)?
                    }
                    effect @ SpellEffectKind::GrantProtection { .. } => {
                        pump_counters::grant_protection(&mut cx, effect)?
                    }
                    effect @ SpellEffectKind::GrantKeywordChoice { .. } => {
                        pump_counters::grant_keyword_choice(&mut cx, effect)?
                    }
                    effect @ SpellEffectKind::GrantTriggeredAbility { .. } => {
                        pump_counters::grant_triggered_ability(&mut cx, effect)?
                    }
                    effect @ SpellEffectKind::AddTypes { .. } => {
                        pump_counters::add_types(&mut cx, effect)?
                    }
                    effect @ SpellEffectKind::GrantKeywordsAllPermanents { .. } => {
                        pump_counters::grant_keywords_all_permanents(&mut cx, effect)?
                    }
                    effect @ SpellEffectKind::ApplyCombatRestriction { .. } => {
                        restrictions::apply_combat_restriction(&mut cx, effect)?
                    }
                    SpellEffectKind::Blight { count } => blight::blight(&mut cx, count)?,
                    effect @ (SpellEffectKind::RemoveCounters { .. }
                    | SpellEffectKind::PutCounterSnapshot { .. }) => {
                        pump_counters::change_counters(&mut cx, effect)?
                    }
                    effect @ SpellEffectKind::PutCounters { .. } => {
                        pump_counters::put_counters(&mut cx, effect)?
                    }
                    effect @ SpellEffectKind::Destroy { .. } => misc::destroy(&mut cx, effect)?,
                    effect @ SpellEffectKind::Sacrifice { .. } => misc::sacrifice(&mut cx, effect)?,
                    effect @ SpellEffectKind::DestroyAttached { .. } => {
                        mass::destroy_attached(&mut cx, effect)?
                    }
                    effect @ SpellEffectKind::CounterTargetSpell { .. } => {
                        stack_ops::counter_target_spell(&mut cx, effect)?
                    }
                    effect @ SpellEffectKind::CounterTriggeringStackObjectUnlessPays { .. } => {
                        stack_ops::counter_triggering_stack_object_unless_pays(&mut cx, effect)?
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
                    effect @ SpellEffectKind::Exile { .. } => zones::exile(&mut cx, effect)?,
                    effect @ SpellEffectKind::ExileWithOwnerCastPermission { .. } => {
                        zones::exile_with_owner_cast_permission(&mut cx, effect)?
                    }
                    effect @ SpellEffectKind::ExileTargetGainLifeEqualToPower => {
                        zones::exile_target_gain_life_equal_to_power(&mut cx, effect)?
                    }
                    effect @ SpellEffectKind::ExileTopWithPlayPermission { .. } => {
                        zones::exile_top_with_play_permission(&mut cx, effect)?
                    }
                    effect @ SpellEffectKind::ReturnToOwnersHand { .. } => {
                        zones::return_to_owners_hand(&mut cx, effect)?
                    }
                    effect @ SpellEffectKind::PutInOwnersLibrary { .. } => {
                        zones::put_in_owners_library(&mut cx, effect)?
                    }
                    effect @ SpellEffectKind::ShufflePermanentsIntoOwnersLibraries { .. } => {
                        zones::shuffle_permanents_into_owners_libraries(&mut cx, effect)?
                    }
                    effect @ SpellEffectKind::DiscardCards { .. } => {
                        zones::discard_cards(&mut cx, effect)?
                    }
                    effect @ SpellEffectKind::ExileCardsFromHand { .. } => {
                        zones::exile_cards_from_hand(&mut cx, effect)?
                    }
                    effect @ SpellEffectKind::MillTargetPlayer { .. } => {
                        zones::mill_target_player(&mut cx, effect)?
                    }
                    effect @ SpellEffectKind::Mill { .. } => zones::mill(&mut cx, effect)?,
                    effect @ SpellEffectKind::TargetPlayerSacrifices { .. } => {
                        zones::target_player_sacrifices(&mut cx, effect)?
                    }
                    effect @ SpellEffectKind::Tap { .. } => misc::tap(&mut cx, effect)?,
                    effect @ SpellEffectKind::SkipNextUntap { .. } => {
                        misc::skip_next_untap(&mut cx, effect)?
                    }
                    effect @ SpellEffectKind::Untap { .. } => misc::untap(&mut cx, effect)?,
                    effect @ SpellEffectKind::GainControlUntilEndOfTurn { .. } => {
                        misc::gain_control_until_end_of_turn(&mut cx, effect)?
                    }
                    effect @ SpellEffectKind::CreateDelayedTrigger { .. } => {
                        misc::create_delayed_trigger(&mut cx, effect)?
                    }
                    effect @ SpellEffectKind::ExileUntilSourceLeaves { .. } => {
                        zones::exile_until_source_leaves(&mut cx, effect)?
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
                    effect @ SpellEffectKind::CreateTokenCopies { .. } => {
                        tokens::create_token_copies(&mut cx, effect)?
                    }
                    SpellEffectKind::Populate => tokens::populate(&mut cx)?,
                    effect @ SpellEffectKind::Amass { .. } => amass::amass(&mut cx, effect)?,
                    effect @ SpellEffectKind::CreateAttackingTokens { .. } => {
                        tokens::create_attacking_tokens(&mut cx, effect)?
                    }
                    effect @ SpellEffectKind::SacrificeObservedObjects => {
                        tokens::sacrifice_observed_objects(&mut cx, effect)?
                    }
                    SpellEffectKind::ExileWarpedObject => {
                        cx.engine.resolve_warp_exile(cx.top, cx.events)?;
                        EffectOutcome::Continue
                    }
                    effect @ SpellEffectKind::AttachSource { .. } => {
                        misc::attach_source(&mut cx, effect)?
                    }
                    effect @ SpellEffectKind::AttachEquipment { .. } => {
                        misc::attach_equipment(&mut cx, effect)?
                    }
                    effect @ SpellEffectKind::Equip { .. } => misc::equip(&mut cx, effect)?,
                    effect @ SpellEffectKind::PreventNextDamage { .. } => {
                        misc::prevent_next_damage(&mut cx, effect)?
                    }
                    effect @ SpellEffectKind::PreventAllCombatDamageToTargetTurn { .. } => {
                        misc::prevent_all_combat_damage_to_target_turn(&mut cx, effect)?
                    }
                    effect @ SpellEffectKind::PreventAllCombatDamageTurn => {
                        misc::prevent_all_combat_damage_turn(&mut cx, effect)?
                    }
                    effect @ SpellEffectKind::DamageCantBePreventedThisTurn => {
                        misc::damage_cant_be_prevented_this_turn(&mut cx, effect)?
                    }
                    effect @ SpellEffectKind::MoveGraveyardCards { .. } => {
                        zones::move_graveyard_cards(&mut cx, effect)?
                    }
                    effect @ SpellEffectKind::ChooseGraveyardCard { .. } => {
                        zones::choose_graveyard_card(&mut cx, effect)?
                    }
                    SpellEffectKind::Earthbend { count } => {
                        pump_counters::earthbend(&mut cx, count)?
                    }
                    effect @ SpellEffectKind::AnimateSelf { .. } => {
                        pump_counters::animate_self(&mut cx, effect)?
                    }
                    effect @ SpellEffectKind::ReturnTriggeredCard { .. } => {
                        zones::return_triggered_card(&mut cx, effect)?
                    }
                    effect @ SpellEffectKind::ProduceMana { .. } => {
                        misc::produce_mana(&mut cx, effect)?
                    }
                    effect @ SpellEffectKind::AddMana { .. } => misc::add_mana(&mut cx, effect)?,
                    effect @ SpellEffectKind::SearchLibrary { .. } => {
                        zones::search_library(&mut cx, effect)?
                    }
                    effect @ SpellEffectKind::Regenerate { .. } => {
                        misc::regenerate(&mut cx, effect)?
                    }
                    effect @ SpellEffectKind::ChangeSourceFace { .. } => {
                        misc::change_source_face(&mut cx, effect)?
                    }
                    SpellEffectKind::SiegeDefeat => zones::siege_defeat(&mut cx)?,
                    effect @ SpellEffectKind::None => misc::none(&mut cx, effect)?,
                    effect @ SpellEffectKind::AuraAttach { .. } => {
                        misc::aura_attach(&mut cx, effect)?
                    }
                    effect @ SpellEffectKind::ChooseResolutionBranch { .. } => {
                        choices::choose_resolution_branch(&mut cx, effect)?
                    }
                    effect @ SpellEffectKind::ChoosePermanents { .. } => {
                        choices::choose_permanents(&mut cx, effect)?
                    }
                    effect @ SpellEffectKind::CreateReflexiveTrigger { .. } => {
                        choices::create_reflexive_trigger(&mut cx, effect)?
                    }
                }
            };
            let mut completed_item = top.clone();
            if let EffectOutcome::Blighted(receipt) = outcome {
                completed_item.blight_receipts.push(receipt);
            }
            self.refresh_enduring_story_designations();
            let mut observer_stack = ParkedStackResolution::new(completed_item.clone());
            observer_stack.resume_effect_index = Some(index as u32 + 1);
            observer_stack.previous_result = effect_result.clone();
            if self.drain_immediate_observer_actions(Some(observer_stack), events)? {
                return Ok(());
            }
            match outcome {
                EffectOutcome::Blighted(_) => {
                    let (effects, label) = self.build_resolution_effects(&completed_item);
                    return self.run_effect_list_with_previous(
                        &completed_item,
                        &label,
                        effects,
                        index + 1,
                        effect_result,
                        events,
                    );
                }
                EffectOutcome::Suspended => {
                    // The handler parked a `PendingResolution` for a player choice; stamp where to
                    // pick this list back up so `complete_parked_resolution` runs the tail (CR
                    // 608.2) rather than ending the resolution here. Handlers do not set this
                    // themselves — they have no idea which list they are a member of, or at what
                    // index.
                    //
                    // `if let` because `search_library`'s degenerate empty-library branch reports
                    // `Suspended` without parking anything; there is then nothing to stamp.
                    if let Some(pending) = self.state.pending_resolution.as_mut() {
                        if let Some(stack) = pending.continuation.stack_mut() {
                            stack.resume_effect_index = Some(index as u32 + 1);
                            stack.previous_result = effect_result;
                        }
                    }
                    return Ok(());
                }
                EffectOutcome::RestartResolutionBranch(branch_index) => {
                    let mut item = top.clone();
                    item.resolution_branch_choices
                        .insert(index as u32, branch_index);
                    let (effects, label) = self.build_resolution_effects(&item);
                    return self.run_effect_list_with_previous(
                        &item,
                        &label,
                        effects,
                        index,
                        previous_effect_result,
                        events,
                    );
                }
                EffectOutcome::Continue => {}
            }
            previous_effect_result = effect_result;
        }
        self.finish_deferred_stack_exit(top, DeferredStackExit::Resolved, events)?;
        events.push(ev_log(format!("{spell_label} resolves.")));
        // CR 608.2m: the spell lands in its owner's graveyard *after* its effects, so it sits
        // beneath anything those effects put there (e.g. a self-targeted Tome Scour's five cards).
        seat_resolved_spell_last_in_graveyard(&mut self.state, top.id);
        Ok(())
    }

    /// Complete CR 610.3 paired one-shot work discovered by a committed zone transition. This is
    /// called between effect instructions and again at the command boundary, so the return is
    /// immediate rather than a trigger that uses the stack. All currently-ready objects enter as
    /// one event cohort, preserving simultaneous returns when several sources leave together.
    pub(super) fn drain_immediate_observer_actions(
        &mut self,
        resume_stack: Option<ParkedStackResolution>,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<bool, EngineError> {
        let mut actions = VecDeque::from(std::mem::take(
            &mut self.state.pending_immediate_observer_actions,
        ));
        let mut entries = Vec::new();
        while let Some(action) = actions.pop_front() {
            let ImmediateObserverAction::ReturnExiledObject { exiled } = action;
            let generation = self
                .state
                .zone_change_generation
                .get(&exiled.object_id)
                .copied()
                .unwrap_or(0);
            let Some(object) = self.state.objects.get(&exiled.object_id) else {
                continue;
            };
            if object.zone != Zone::Exile || generation != exiled.zone_change_generation {
                continue;
            }
            // CR 111.7: a token that left the battlefield has ceased to exist at the preceding
            // SBA check. Do not recreate one if a synthetic test reaches this boundary earlier.
            if object.is_token() {
                continue;
            }
            let owner = object.owner;
            let label = object_display_name(&self.state, self.registry, exiled.object_id);
            let aura_filter = self.effective_face(exiled.object_id).and_then(|face| {
                face.spell_effect.iter().find_map(|effect| match effect {
                    SpellEffectKind::AuraAttach { target } => Some(target.clone()),
                    _ => None,
                })
            });
            if let Some(filter) = aura_filter {
                let (choice_kind, candidates) = if filter.is_player() {
                    (
                        rv1::ChoiceKind::AuraPlayer,
                        self.state
                            .players
                            .iter()
                            .filter(|player| !player.has_lost)
                            .map(|player| player.id as ObjectId)
                            .filter(|player_id| {
                                super::targeting::attachment_filter_legal(
                                    self,
                                    &filter,
                                    AttachmentRecipient::Player(*player_id as PlayerId),
                                    exiled.object_id,
                                    owner,
                                )
                            })
                            .collect::<Vec<_>>(),
                    )
                } else {
                    let mut candidates = self
                        .state
                        .objects
                        .keys()
                        .copied()
                        .filter(|object_id| {
                            super::targeting::attachment_filter_legal(
                                self,
                                &filter,
                                AttachmentRecipient::Object(*object_id),
                                exiled.object_id,
                                owner,
                            )
                        })
                        .collect::<Vec<_>>();
                    candidates.sort_unstable();
                    (rv1::ChoiceKind::AuraPermanent, candidates)
                };
                if candidates.is_empty() {
                    continue;
                }
                // Preserve the rest of a simultaneous return cohort. The accepted choice drains
                // it before resuming the original stack instruction.
                self.state
                    .pending_immediate_observer_actions
                    .extend(actions);
                let prompt = format!("Choose what {label} will enchant as it returns.");
                events.push(rv1::RuledEvent {
                    ev: Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(
                        rv1::ResolutionChoiceRequired {
                            deciding_player_id: owner,
                            source_object_id: exiled.object_id,
                            prompt_text: prompt.clone(),
                            choice_kind: choice_kind as i32,
                            candidate_object_ids: candidates.clone(),
                            candidate_card_ids: candidates
                                .iter()
                                .map(|candidate| {
                                    self.state
                                        .objects
                                        .get(candidate)
                                        .map(|object| object.card_id.clone())
                                        .unwrap_or_default()
                                })
                                .collect(),
                            min: 1,
                            max: 1,
                            ordered: false,
                            candidate_names: candidates
                                .iter()
                                .map(|candidate| {
                                    self.state
                                        .objects
                                        .get(candidate)
                                        .map(|_| {
                                            object_display_name(
                                                &self.state,
                                                self.registry,
                                                *candidate,
                                            )
                                        })
                                        .unwrap_or_else(|| format!("P{candidate}"))
                                })
                                .collect(),
                            candidate_server_card_ids: Vec::new(),
                            unique_names: false,
                            generic_mana_cost: 0,
                            payment_currently_legal: false,
                            resolution_branches: Vec::new(),
                            mana_cost: String::new(),
                            candidate_selectable: Vec::new(),
                            reveal_audience: 0,
                            revealed_zone_owner_player_id: None,
                            candidate_source_zones: Vec::new(),
                            combat_defender_options: Vec::new(),
                            waterbend: false,
                            selection_slots: Vec::new(),
                        },
                    )),
                });
                events.push(ev_log(prompt.clone()));
                self.state.pending_resolution = Some(PendingResolution {
                    deciding_player: owner,
                    presentation: PendingResolutionPresentation {
                        source_object_id: exiled.object_id,
                        candidates,
                        min: 1,
                        max: 1,
                        ordered: false,
                        prompt,
                        choice_kind,
                        unique_names: false,
                    },
                    continuation: ResolutionContinuation::AuraReturn {
                        stack: resume_stack,
                        exiled,
                    },
                });
                return Ok(true);
            }
            let entry = BattlefieldEntryEvent {
                object_id: exiled.object_id,
                deciding_player: owner,
                destination_controller: owner,
                battle_protector: None,
                face_index: 0,
                unlock_room_door: None,
                chosen_x: 0,
                cast_cost_receipts: Vec::new(),
                player_life_snapshot: self.player_life_snapshot(),
                tapped: false,
                entry_counters: BTreeMap::new(),
                applied_effects: Vec::new(),
            };
            let resume_original_stack = resume_stack.is_some();
            let item = resume_stack
                .as_ref()
                .map(|stack| stack.item.clone())
                .unwrap_or_else(|| self.observer_return_item(exiled.object_id, owner));
            match self.begin_battlefield_entry(
                item,
                entry,
                BattlefieldEntryCompletion::ObserverReturn {
                    owner,
                    object_label: label.clone(),
                    attached_to: None,
                    resume_original_stack,
                },
                events,
            ) {
                super::replacement::BattlefieldEntryProgress::Parked => {
                    if let (Some(resume), Some(pending)) = (
                        resume_stack.as_ref(),
                        self.state.pending_resolution.as_mut(),
                    ) {
                        if let Some(parked) = pending.continuation.stack_mut() {
                            parked.resume_effect_index = resume.resume_effect_index;
                            parked.previous_result = resume.previous_result.clone();
                        }
                    }
                    self.state
                        .pending_immediate_observer_actions
                        .extend(actions);
                    return Ok(true);
                }
                super::replacement::BattlefieldEntryProgress::Ready(entry) => {
                    entries.push((entry, owner, label));
                }
            }
        }

        let mut trigger_events = Vec::new();
        for (entry, owner, label) in entries {
            let object_id = entry.object_id;
            let chosen_x = entry.chosen_x;
            // Observer returns are independent one-shot effects. Entry replacement ordering is
            // added in the Aura/choice increment; the base path still uses the canonical commit
            // reset and static-registration machinery.
            let door_event = self.commit_battlefield_entry_state(entry, None)?;
            trigger_events.push(GameEvent::EntersBattlefield {
                object_id,
                chosen_x,
            });
            trigger_events.extend(door_event);
            events.push(permanent_moved_event(
                &self.state,
                object_id,
                owner,
                rv1::permanent_moved::Destination::Battlefield,
            ));
            events.push(ev_log(format!(
                "{label} returns to the battlefield under its owner's control."
            )));
        }
        self.fire_triggers(&trigger_events);
        Ok(false)
    }

    pub(super) fn observer_return_item(
        &self,
        object_id: ObjectId,
        controller: PlayerId,
    ) -> StackItem {
        let card_id = self
            .state
            .objects
            .get(&object_id)
            .map(|object| object.card_id.clone())
            .unwrap_or_default();
        StackItem {
            id: object_id,
            controller,
            card_id,
            targets: Vec::new(),
            ability_text: Some("paired one-shot return".into()),
            source_permanent_id: None,
            source_owner: None,
            source_zone_change: 0,
            source_face_change: 0,
            ability_index: None,
            activated_ability: None,
            triggered_ability: None,
            is_triggered: false,
            is_copy: true,
            face_index: 0,
            cast_method: SpellCastMethod::Normal,
            sneak_attack: None,
            chosen_x: 0,
            chosen_modes: Vec::new(),
            cast_condition_results: Vec::new(),
            cast_occurrence: None,
            cast_cost_receipts: Vec::new(),
            payment_result: CardResultCohort::default(),
            resolution_branch_choices: BTreeMap::new(),
            blight_receipts: Vec::new(),
            trigger_context: TriggerContext::default(),
        }
    }

    /// CR 111: mint `count` tokens of `token_id` for each recipient and put them onto the
    /// battlefield. Each minted token is a fresh [`GameObject`] whose characteristics come from
    /// the token's [`CardDefinition`] (via the registry's token namespace), so combat, P/T, and
    /// keyword queries treat it exactly like any other permanent. Entering tokens fire ETB
    /// triggers (CR 603.6) through the same hook as a resolved creature spell, so Soul Warden et al.
    /// see them. A [`TokenCreated`](rv1::TokenCreated) event carries the self-describing identity
    /// the relay needs (tokens have no deck card / Oracle entry).
    fn create_tokens(
        &mut self,
        request: TokenCreationRequest<'_>,
        enters_tapped: bool,
        delayed_sacrifice: Option<DelayedTokenSacrificeTiming>,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<bool, EngineError> {
        let item = request.item.clone();
        let (entries, logs) = self.prepare_token_entries(request, enters_tapped)?;
        // CR 603.6: one token-making instruction puts all of its tokens onto the battlefield
        // simultaneously, so every entrant exists before their ETB triggers are collected.
        self.begin_token_entry_batch(
            item,
            entries,
            logs,
            TokenEntryBatchOptions {
                delayed_sacrifice,
                ..Default::default()
            },
            events,
        )
    }

    pub(super) fn prepare_token_entries(
        &mut self,
        request: TokenCreationRequest<'_>,
        enters_tapped: bool,
    ) -> Result<(Vec<TokenBattlefieldEntry>, Vec<String>), EngineError> {
        let TokenCreationRequest {
            token_id,
            values,
            count,
            recipients,
            spell_label,
            item: _,
        } = request;
        let registry = self.registry;
        let values = if let Some(values) = values {
            values.clone()
        } else {
            let def = registry
                .get(token_id)
                .ok_or_else(|| EngineError::MissingCard(token_id.to_string()))?;
            CopiableValues {
                source_card_id: token_id.to_string(),
                source_face_index: 0,
                face: def.primary_face().clone(),
                display_name: def.name.clone(),
                room_faces: None,
            }
        };
        let name = values.display_name.clone();
        let face = &values.face;
        let is_creature = face.is_creature;
        let power = face.power;
        let toughness = face.toughness;

        let player_life_snapshot = self.player_life_snapshot();
        let mut entries = Vec::new();
        let mut logs = Vec::new();
        for pid in recipients {
            if self.state.player_idx(pid).is_none() {
                continue;
            }
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
                        base_controller: pid,
                        controller: pid,
                        card_id: token_id.to_string(),
                        token_origin: Some(values.clone()),
                        copiable_values: None,
                        copy_revision: 0,
                        // Proposed tokens live in no player's zone until entry replacements finish.
                        zone: Zone::Stack,
                        tapped: enters_tapped,
                        summoning_sick: is_creature,
                        power,
                        toughness,
                        damage: 0,
                        deathtouch_damage: false,
                        counters: BTreeMap::new(),
                        counter_timestamps: BTreeMap::new(),
                        attached_to: None,
                        regeneration_shields: 0,
                        must_attack_if_able: false,
                        must_block_if_able: false,
                        face_up_index: 0,
                        face_down: false,
                    },
                );
                let created = rv1::TokenCreated {
                    object_id: oid,
                    controller_player_id: pid,
                    card_id: token_id.to_string(),
                    identity: Some(token_identity(&values)),
                    enters_tapped,
                };
                entries.push(TokenBattlefieldEntry {
                    event: BattlefieldEntryEvent {
                        object_id: oid,
                        deciding_player: pid,
                        destination_controller: pid,
                        battle_protector: None,
                        face_index: 0,
                        unlock_room_door: None,
                        chosen_x: 0,
                        cast_cost_receipts: Vec::new(),
                        player_life_snapshot: player_life_snapshot.clone(),
                        tapped: enters_tapped,
                        entry_counters: BTreeMap::new(),
                        applied_effects: Vec::new(),
                    },
                    created,
                });
            }
            let noun = if count == 1 { "token" } else { "tokens" };
            logs.push(format!(
                "P{pid} creates {count} {name} {noun} ({spell_label})."
            ));
        }
        Ok((entries, logs))
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
    mut destination: rv1::permanent_moved::Destination,
) -> rv1::RuledEvent {
    if destination == rv1::permanent_moved::Destination::Graveyard
        && state
            .objects
            .get(&oid)
            .is_some_and(|object| object.zone == Zone::Exile)
    {
        destination = rv1::permanent_moved::Destination::Exile;
    }
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
            face_down: state
                .objects
                .get(&oid)
                .is_some_and(|object| object.zone == Zone::Battlefield && object.face_down),
            source_library_position: None,
        })),
    }
}

/// Resolve a [`CreatureScopeFilter`] into a dynamic [`AffectedScope`] for a static continuous effect,
/// given the effect's `controller` and the source permanent `source`.
pub(super) fn resolve_creature_scope(
    filter: &CreatureScopeFilter,
    controller: PlayerId,
    source: ObjectId,
) -> AffectedScope {
    AffectedScope::CreaturesMatching {
        reference_player: controller,
        filter: filter.clone(),
        exclude: if filter.exclude_self {
            Some(source)
        } else {
            None
        },
    }
}

/// Snapshot the creatures matched by a resolving one-shot team effect (CR 611.2c).
///
/// Unlike a static anthem, a resolving spell or triggered ability fixes its affected objects when
/// it resolves. Glorious Charge and Inspiring Captain both use this path; a creature entering
/// later in the turn must not inherit their pump.
pub(super) fn snapshot_creature_scope(
    engine: &GameEngine,
    filter: &CreatureScopeFilter,
    controller: PlayerId,
    source: ObjectId,
) -> Vec<ObjectId> {
    engine
        .state
        .players
        .iter()
        .flat_map(|player| player.battlefield.iter().copied())
        .filter_map(|oid| engine.characteristics(oid).map(|value| (oid, value)))
        .filter(|(oid, value)| {
            super::characteristics::creature_matches_scope(
                &engine.state,
                engine.registry,
                filter,
                controller,
                filter.exclude_self.then_some(source),
                *oid,
                value,
            )
        })
        .map(|(oid, _)| oid)
        .collect()
}

/// The semantic discard seam. Authored discard effects, discard costs, and cleanup discards all
/// pass through here; generic hand-to-zone movement does not. Future discard occurrences and
/// replacement effects attach here rather than inferring discard from a graveyard destination.
pub(crate) fn perform_discard(
    state: &mut GameState,
    registry: &'static CardRegistry,
    affected_player: PlayerId,
    object_id: ObjectId,
) -> Result<(String, rv1::RuledEvent), EngineError> {
    let object = state
        .objects
        .get(&object_id)
        .ok_or(EngineError::Illegal("discarded card object not found"))?;
    if object.owner != affected_player || object.zone != Zone::Hand {
        return Err(EngineError::Illegal(
            "discarded card is not in its owner's hand",
        ));
    }
    let card_name = object_display_name(state, registry, object_id);
    move_object_to_zone(state, registry, object_id, Zone::Graveyard, None)?;
    let moved = permanent_moved_event(
        state,
        object_id,
        affected_player,
        rv1::permanent_moved::Destination::Graveyard,
    );
    Ok((card_name, moved))
}

/// Move `oid` into zone `z`, maintaining every zone list and the CR 400.7 new-object resets.
///
/// `controller` names the player the permanent enters the battlefield **under** (CR 110.2);
/// `None` means "its owner controls it", which is what every non-control-changing caller passes.
/// It is ignored for non-battlefield zones — those belong to the owner (CR 400.3).
pub(crate) fn move_object_to_zone(
    state: &mut GameState,
    registry: &'static CardRegistry,
    oid: ObjectId,
    z: Zone,
    controller: Option<PlayerId>,
) -> Result<(), EngineError> {
    if let Some(fact) = move_object_to_zone_with_entry_receipt(state, registry, oid, z, controller)?
    {
        state
            .turn_history
            .current
            .permanent_cards_entered_graveyard
            .push(fact);
    }
    Ok(())
}

/// Ordinary moves commit the returned history receipt immediately. The sole exception is
/// early custom-resolution bookkeeping, whose receipt belongs to the resolution completion.
fn move_object_to_zone_with_entry_receipt(
    state: &mut GameState,
    registry: &'static CardRegistry,
    oid: ObjectId,
    mut z: Zone,
    controller: Option<PlayerId>,
) -> Result<Option<crate::state::PermanentHistoryFact>, EngineError> {
    let owner = state
        .objects
        .get(&oid)
        .map(|o| o.owner)
        .ok_or(EngineError::Illegal("no object"))?;
    let old_zone = state.objects.get(&oid).map(|o| o.zone);
    let leaving_battlefield = old_zone == Some(Zone::Battlefield) && z != Zone::Battlefield;
    let prior_generation = state.zone_change_generation.get(&oid).copied().unwrap_or(0);
    // CR 111.8: after leaving the battlefield a token cannot change zones again, even
    // while resolution defers SBAs. Proposed, not-yet-entered tokens have generation zero.
    if old_zone != Some(Zone::Battlefield)
        && prior_generation > 0
        && state
            .objects
            .get(&oid)
            .is_some_and(|object| object.token_origin.is_some())
    {
        return Ok(None);
    }
    if old_zone == Some(Zone::Battlefield)
        && z == Zone::Graveyard
        && state.death_replacement_effects.iter().any(|effect| {
            effect.object_id == oid && effect.zone_change_generation == prior_generation
        })
    {
        z = Zone::Exile;
    }
    if leaving_battlefield {
        state.death_replacement_effects.retain(|effect| {
            effect.object_id != oid || effect.zone_change_generation != prior_generation
        });
    }
    let last_known_characteristics = leaving_battlefield
        .then(|| super::characteristics::characteristics_from(state, registry, oid))
        .flatten();
    let last_known_attached_object = leaving_battlefield
        .then(|| {
            state
                .objects
                .get(&oid)
                .and_then(|object| object.attached_to)
        })
        .flatten()
        .and_then(|recipient| match recipient {
            AttachmentRecipient::Object(target_oid)
                if state
                    .objects
                    .get(&target_oid)
                    .is_some_and(|target| target.zone == Zone::Battlefield) =>
            {
                Some((
                    target_oid,
                    state
                        .zone_change_generation
                        .get(&target_oid)
                        .copied()
                        .unwrap_or(0),
                ))
            }
            AttachmentRecipient::Object(_) | AttachmentRecipient::Player(_) => None,
        });
    let front_face_values = leaving_battlefield
        .then(|| {
            state
                .objects
                .get(&oid)
                .and_then(|object| registry.get(&object.card_id))
                .map(|definition| {
                    let face = definition.primary_face();
                    (
                        face.power,
                        face.toughness,
                        face.must_attack_if_able,
                        face.must_block_if_able,
                    )
                })
        })
        .flatten();
    if leaving_battlefield {
        state.room_states.remove(&oid);
        state.battle_protectors.remove(&oid);
        if let Some(old_controller) = state.objects.get(&oid).map(|object| object.controller) {
            let object = TriggerObjectRef {
                object_id: oid,
                zone_change_generation: prior_generation,
                controller_at_event: old_controller,
            };
            let delayed = state.dispatch_event_observers(ObservedGameEvent::ControllerChanged {
                object,
                old_controller,
                new_controller: None,
            });
            state.stage_delayed_batch(delayed);
            let delayed =
                state.dispatch_event_observers(ObservedGameEvent::LeavesBattlefield(object));
            state.stage_delayed_batch(delayed);
        }
    }
    // A move to the same named zone is still a zone change and creates a new object (CR 400.7).
    // This matters for exile permissions: exiling an already-exiled card cannot preserve an old
    // Adventure or "play it" permission merely because the destination enum is unchanged.
    if old_zone.is_some() {
        state
            .warped_permanent_incarnations
            .retain(|&(object_id, generation)| object_id != oid || generation != prior_generation);
        *state.zone_change_generation.entry(oid).or_insert(0) += 1;
        state
            .active_exile_play_permissions
            .retain(|permission| permission.object_id != oid);
    }

    // CR 400.7: a zone change creates a new game object. Remove any Single-target continuous
    // effects on this object so they don't apply if the same ObjectId is reused later.
    // CR 604.3 / 611.3: also drain any `WhileSourceOnBattlefield` effects this permanent was the
    // source of (anthems) — a static ability stops applying the moment its source leaves (LTB).
    // One-shot `UntilEndOfTurn` effects (Giant Growth, firebreathing) are deliberately NOT drained
    // here: once created they are independent of their source (CR 611.2g) and only end at cleanup.
    if leaving_battlefield {
        if let Some(attached_object) = last_known_attached_object {
            state
                .last_known_attached_object_by_generation
                .insert((oid, prior_generation), attached_object);
        }
        state
            .skip_next_untap
            .retain(|&(object_id, _)| object_id != oid);
        state.continuous_effects.retain(|e| {
            let single_on_this = matches!(&e.affected, AffectedScope::Single(id) if *id == oid);
            let static_from_this =
                e.source_id == Some(oid) && e.duration == EffectDuration::WhileSourceOnBattlefield;
            !single_on_this && !static_from_this
        });
        state.damage_prevention_effects.retain(|effect| {
            let recipient_is_this_object = match effect.scope {
                DamagePreventionScope::Recipient(recipient) => recipient == oid,
                DamagePreventionScope::CombatRecipient { object_id, .. } => object_id == oid,
                DamagePreventionScope::Combat
                | DamagePreventionScope::OtherCreaturesYouControl { .. } => false,
            };
            let static_from_this = effect.source_id == Some(oid)
                && effect.duration == EffectDuration::WhileSourceOnBattlefield;
            !recipient_is_this_object && !static_from_this
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
            state
                .last_known_counters_by_generation
                .insert((oid, prior_generation), o.counters.clone());
            o.counters.clear();
            o.counter_timestamps.clear();
            o.attached_to = None;
            o.regeneration_shields = 0;
            o.face_up_index = 0;
            o.face_down = false;
            o.copiable_values = None;
            o.copy_revision = 0;
            if let Some((power, toughness, must_attack, must_block)) = front_face_values {
                o.power = power;
                o.toughness = toughness;
                o.must_attack_if_able = must_attack;
                o.must_block_if_able = must_block;
            }
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
        if let Some(characteristics) = last_known_characteristics {
            state.last_known_pt_by_generation.insert(
                (oid, prior_generation),
                (
                    characteristics.signed_power,
                    characteristics.signed_toughness,
                ),
            );
            state
                .last_known_controller_by_generation
                .insert((oid, prior_generation), characteristics.controller);
            state
                .last_known_keywords_by_generation
                .insert((oid, prior_generation), characteristics.keywords);
            state
                .last_known_colors_by_generation
                .insert((oid, prior_generation), characteristics.colors);
            state
                .last_known_types_by_generation
                .insert((oid, prior_generation), characteristics.types);
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
        let controller = if z == Zone::Battlefield {
            holder
        } else {
            o.owner
        };
        o.base_controller = controller;
        o.controller = controller;
        // CR 302.6: a permanent entering the battlefield has not been controlled continuously
        // since its controller's most recent turn began, so it is summoning sick. Assert this on
        // entry rather than trusting a persisted flag — a prior bounce/leave clears transient
        // state, so a creature returned to hand and recast (or reanimated/flickered) the same turn
        // must still be sick. Haste exempts the *use* of this (checked at attack/tap time).
        if z == Zone::Battlefield {
            o.summoning_sick = true;
        }
    }
    Ok((old_zone != Some(Zone::Graveyard))
        .then(|| super::history::graveyard_entry_fact(state, registry, oid))
        .flatten())
}

/// CR 608.2n: finish a custom spell's history without changing its physical projection.
pub(super) fn finish_deferred_graveyard_entry(state: &mut GameState, item: &StackItem) {
    if !state
        .deferred_graveyard_entry
        .as_ref()
        .is_some_and(|(occurrence, _)| {
            occurrence.object_id == item.id
                && item.cast_occurrence.is_none_or(|cast| cast == *occurrence)
        })
    {
        return;
    }
    let (_, fact) = state
        .deferred_graveyard_entry
        .take()
        .expect("matched receipt");
    if state
        .objects
        .get(&fact.object_id)
        .is_some_and(|object| object.zone == Zone::Graveyard)
        && state
            .zone_change_generation
            .get(&fact.object_id)
            .copied()
            .unwrap_or(0)
            == fact.zone_change_generation
    {
        state
            .turn_history
            .current
            .permanent_cards_entered_graveyard
            .push(fact);
    }
}

pub(super) fn destroy_permanent(
    state: &mut GameState,
    registry: &'static CardRegistry,
    oid: ObjectId,
) -> Result<bool, EngineError> {
    move_object_to_zone(state, registry, oid, Zone::Graveyard, None)?;
    Ok(state
        .objects
        .get(&oid)
        .is_some_and(|object| object.zone == Zone::Graveyard))
}

/// Move a permanent to its owner's graveyard without assigning a semantic cause. The legend
/// rule and other direct rule actions use this seam so they produce leave/death events but never
/// masquerade as sacrifices.
pub(super) fn put_permanent_in_graveyard(
    state: &mut GameState,
    registry: &'static CardRegistry,
    oid: ObjectId,
) -> Result<bool, EngineError> {
    move_object_to_zone(state, registry, oid, Zone::Graveyard, None)?;
    Ok(state
        .objects
        .get(&oid)
        .is_some_and(|object| object.zone == Zone::Graveyard))
}

/// Sacrifice a permanent (CR 701.21). Both costs and effects use this seam. Sacrifice bypasses
/// indestructible and regeneration, but a zone-change replacement can change its destination.
pub(super) fn sacrifice_permanent(
    state: &mut GameState,
    registry: &'static CardRegistry,
    oid: ObjectId,
) -> Result<bool, EngineError> {
    if !state
        .objects
        .get(&oid)
        .is_some_and(|object| object.zone == Zone::Battlefield)
    {
        return Err(EngineError::Illegal(
            "only a battlefield permanent can be sacrificed",
        ));
    }
    put_permanent_in_graveyard(state, registry, oid)
}

/// CR 608.2n: the spell enters its owner's graveyard after its effects have been applied. A
/// self-targeted Tome Scour must therefore end up beneath the five cards it milled, not on top
/// of them.
///
/// Authored spells defer their actual stack exit. Custom spells retain early physical movement;
/// this re-seats their already-moved card at the back of its owner's graveyard at completion.
///
/// Custom physical placement timing remains a simplification. History accounting is separately
/// deferred by `finish_deferred_graveyard_entry`; reseating must never manufacture another entry.
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

fn counter_label(kind: CounterKind) -> String {
    kind.label()
}

pub(crate) fn permanent_moved_event_with_library_position(
    state: &GameState,
    oid: ObjectId,
    owner_player_id: PlayerId,
    destination: rv1::permanent_moved::Destination,
    source_library_position: u32,
) -> rv1::RuledEvent {
    let mut event = permanent_moved_event(state, oid, owner_player_id, destination);
    if let Some(rv1::ruled_event::Ev::PermanentMoved(moved)) = event.ev.as_mut() {
        moved.source_library_position = Some(source_library_position);
    }
    event
}

/// Return true if the library card `oid` satisfies `filter` (None = any card). The definition
/// chooses the rules-correct characteristics for its physical layout in this non-stack zone.
pub(super) fn card_matches_type_filter(
    state: &GameState,
    registry: &'static CardRegistry,
    oid: ObjectId,
    filter: Option<&CardTypeFilter>,
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
    def.matches_card_type_outside_stack(*filter)
}

pub(super) fn library_card_matches_filter(
    state: &GameState,
    registry: &'static CardRegistry,
    oid: ObjectId,
    filter: Option<&ZoneCardFilter>,
) -> bool {
    let Some(filter) = filter else {
        return true;
    };
    let Some(def) = state
        .objects
        .get(&oid)
        .and_then(|object| registry.get(&object.card_id))
    else {
        return false;
    };
    if let Some(branches) = &filter.any_of {
        return branches
            .iter()
            .any(|branch| library_card_matches_filter(state, registry, oid, Some(branch)));
    }
    filter
        .exact_name
        .as_deref()
        .is_none_or(|name| def.name == name)
        && filter
            .card_type
            .is_none_or(|card_type| def.matches_card_type_outside_stack(card_type))
        && filter
            .subtype
            .as_deref()
            .is_none_or(|subtype| def.has_subtype_outside_stack(subtype))
        && filter.printed_power.is_none_or(|comparison| {
            def.primary_face()
                .power
                .is_some_and(|power| match comparison {
                    PowerComparison::AtLeast(minimum) => power >= minimum,
                    PowerComparison::AtMost(maximum) => power <= maximum,
                })
        })
}

#[cfg(test)]
mod exile_permission_generation_tests {
    use super::*;

    #[test]
    fn exile_to_exile_creates_a_new_generation_and_invalidates_permission() {
        let mut engine = GameEngine::new_with_default_decks(123_007, &[0, 1], 20).expect("engine");
        let object_id = engine.state.players[0].library[0];
        move_object_to_zone(
            &mut engine.state,
            engine.registry,
            object_id,
            Zone::Exile,
            None,
        )
        .expect("initial exile move");
        let generation = engine.state.zone_change_generation[&object_id];
        engine
            .state
            .active_exile_play_permissions
            .push(ActiveExilePlayPermission {
                group_id: 1,
                player_id: 0,
                source_label: "generation test".to_string(),
                origin: crate::state::ExilePlayPermissionOrigin::Effect,
                available_after_turn_instance: None,
                object_id,
                zone_change_generation: generation,
                scope: ExilePlayPermissionScope::PlayCard,
                cast_cost: crate::state::ExilePermissionCastCost::PrintedManaCost,
                expires_at_cleanup_turn_instance: None,
            });

        move_object_to_zone(
            &mut engine.state,
            engine.registry,
            object_id,
            Zone::Exile,
            None,
        )
        .expect("exile-to-exile move");

        assert_eq!(engine.state.objects[&object_id].zone, Zone::Exile);
        assert!(engine.state.zone_change_generation[&object_id] > generation);
        assert!(engine.state.active_exile_play_permissions.is_empty());
    }
}

#[cfg(test)]
mod zone_card_filter_tests {
    use super::*;

    #[test]
    fn multiface_cards_use_front_face_printed_power_outside_the_battlefield() {
        let decks = Some(vec![
            vec!["reckless_waif_merciless_predator".to_string(); 12],
            vec!["forest".to_string(); 12],
        ]);
        let engine = GameEngine::new(110_001, &[0, 1], 20, decks, true).expect("new game");
        let object_id = engine.state.players[0].library[0];
        let front_face_power = ZoneCardFilter {
            printed_power: Some(PowerComparison::AtMost(1)),
            ..Default::default()
        };
        let back_face_power = ZoneCardFilter {
            printed_power: Some(PowerComparison::AtLeast(3)),
            ..Default::default()
        };

        assert!(library_card_matches_filter(
            &engine.state,
            engine.registry,
            object_id,
            Some(&front_face_power),
        ));
        assert!(!library_card_matches_filter(
            &engine.state,
            engine.registry,
            object_id,
            Some(&back_face_power),
        ));
    }
}

/// CR 614.8 / 701.19: attempt to consume one regeneration shield from `oid`. If a shield is present,
/// taps the creature, removes it from combat, clears all marked damage, and returns `true` plus
/// the tap edge, if any. The caller is responsible for not destroying the creature. Returns
/// `false` if no shield exists.
/// Does NOT emit a zone-change event (the creature stays on the battlefield).
pub(super) fn consume_regen_shield(
    engine: &mut GameEngine,
    oid: ObjectId,
    events: &mut Vec<rv1::RuledEvent>,
) -> (bool, Option<GameEvent>) {
    let shields = engine
        .state
        .objects
        .get(&oid)
        .map(|o| o.regeneration_shields)
        .unwrap_or(0);
    if shields == 0 {
        return (false, None);
    }
    // CR 701.19a: regenerating taps the permanent — a real "becomes tapped" edge, so it goes
    // through the shared funnel rather than writing the flag inline.
    // CR 701.19: the permanent's controller taps it, not the shield's creator or destroyer.
    let actor = engine
        .characteristics(oid)
        .map(|c| c.controller)
        .unwrap_or(engine.state.objects[&oid].controller);
    let tap_event = engine.tap_permanents(actor, &[oid]).pop();
    let state = &mut engine.state;
    if let Some(o) = state.objects.get_mut(&oid) {
        o.regeneration_shields -= 1;
        o.damage = 0;
        o.deathtouch_damage = false;
    }
    // CR 701.19a: remove from combat (attacker/blocker lists). This mirrors what happens when
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
    (true, tap_event)
}

#[cfg(test)]
mod anthem_scope_tests {
    use super::*;

    fn add_creature(engine: &mut GameEngine, controller: PlayerId) -> ObjectId {
        let id = engine.state.next_object_id;
        engine.state.next_object_id += 1;
        engine.state.objects.insert(
            id,
            GameObject {
                id,
                owner: controller,
                base_controller: controller,
                controller,
                card_id: "grizzly_bears".to_string(),
                token_origin: None,
                copiable_values: None,
                copy_revision: 0,
                zone: Zone::Battlefield,
                tapped: false,
                summoning_sick: false,
                power: None,
                toughness: None,
                damage: 0,
                deathtouch_damage: false,
                counters: BTreeMap::new(),
                counter_timestamps: BTreeMap::new(),
                attached_to: None,
                regeneration_shields: 0,
                must_attack_if_able: false,
                must_block_if_able: false,
                face_up_index: 0,
                face_down: false,
            },
        );
        let player_index = engine.state.player_idx(controller).expect("controller");
        engine.state.players[player_index].battlefield.push(id);
        id
    }

    #[test]
    fn token_copy_cannot_reenter_before_the_next_sba() {
        let mut engine = GameEngine::new(4610, &[0, 1], 20, None, true).unwrap();
        let token = add_creature(&mut engine, 0);
        let values = engine.copiable_values_for(token).unwrap();
        engine.state.objects.get_mut(&token).unwrap().token_origin = Some(values);
        move_object_to_zone(&mut engine.state, engine.registry, token, Zone::Exile, None).unwrap();
        let generation = engine.state.zone_change_generation[&token];
        move_object_to_zone(
            &mut engine.state,
            engine.registry,
            token,
            Zone::Battlefield,
            None,
        )
        .unwrap();
        assert_eq!(engine.state.objects[&token].zone, Zone::Exile);
        assert_eq!(engine.state.zone_change_generation[&token], generation);
        assert!(!engine.state.players[0].battlefield.contains(&token));
    }

    #[test]
    fn issue_75_opponent_snapshot_is_player_set_generic() {
        let mut engine =
            GameEngine::new(75_004, &[10, 20], 20, None, true).expect("two-player engine");
        engine.state.players.push(PlayerState::new(30, 20));
        let mine = add_creature(&mut engine, 10);
        let first_opponent = add_creature(&mut engine, 20);
        let second_opponent = add_creature(&mut engine, 30);

        let affected = snapshot_creature_scope(
            &engine,
            &CreatureScopeFilter {
                controller: Some(CreatureScopeController::Opponents),
                ..CreatureScopeFilter::default()
            },
            10,
            u32::MAX,
        );

        assert_eq!(affected, [first_opponent, second_opponent]);
        assert!(!affected.contains(&mine));
    }
}

#[cfg(test)]
mod attached_subject_tests {
    use super::*;

    fn add_battlefield_object(
        engine: &mut GameEngine,
        controller: PlayerId,
        card_id: &str,
    ) -> ObjectId {
        let id = engine.state.next_object_id;
        engine.state.next_object_id += 1;
        engine.state.objects.insert(
            id,
            GameObject {
                id,
                owner: controller,
                base_controller: controller,
                controller,
                card_id: card_id.to_string(),
                token_origin: None,
                copiable_values: None,
                copy_revision: 0,
                zone: Zone::Battlefield,
                tapped: false,
                summoning_sick: false,
                power: None,
                toughness: None,
                damage: 0,
                deathtouch_damage: false,
                counters: BTreeMap::new(),
                counter_timestamps: BTreeMap::new(),
                attached_to: None,
                regeneration_shields: 0,
                must_attack_if_able: false,
                must_block_if_able: false,
                face_up_index: 0,
                face_down: false,
            },
        );
        let player_index = engine.state.player_idx(controller).expect("controller");
        engine.state.players[player_index].battlefield.push(id);
        id
    }

    fn triggered_item(source: ObjectId, generation: u64) -> StackItem {
        StackItem {
            id: source + 10_000,
            controller: 0,
            card_id: "capture_sphere".to_string(),
            targets: vec![],
            ability_text: Some("When this Aura enters, tap enchanted creature.".to_string()),
            source_permanent_id: Some(source),
            source_owner: Some(0),
            source_zone_change: generation,
            source_face_change: 0,
            ability_index: Some(0),
            activated_ability: None,
            triggered_ability: None,
            is_triggered: true,
            is_copy: false,
            face_index: 0,
            cast_method: SpellCastMethod::Normal,
            sneak_attack: None,
            chosen_x: 0,
            chosen_modes: vec![],
            cast_condition_results: Vec::new(),
            cast_occurrence: None,
            cast_cost_receipts: vec![],
            payment_result: CardResultCohort::default(),
            resolution_branch_choices: Default::default(),
            blight_receipts: Vec::new(),
            trigger_context: TriggerContext::default(),
        }
    }

    fn quantity_item(source: ObjectId, effects: Vec<SpellEffectKind>) -> StackItem {
        let mut item = triggered_item(source, 0);
        let mut ability = tricerules_cards::CardRegistry::global()
            .get("brambleguard_captain")
            .unwrap()
            .primary_face()
            .triggered_abilities[0]
            .clone();
        ability.effect = effects;
        item.triggered_ability = Some(ability);
        item
    }

    #[test]
    fn issue_157_blossombind_prohibition_precedes_stun_and_expires_on_departure() {
        let mut engine = GameEngine::new_with_default_decks(15705, &[0, 1], 20).unwrap();
        let target = add_battlefield_object(&mut engine, 0, "hill_giant");
        let aura = add_battlefield_object(&mut engine, 1, "blossombind");
        engine.state.objects.get_mut(&aura).unwrap().attached_to =
            Some(AttachmentRecipient::Object(target));
        engine.emit_static_abilities_on_enter(aura);
        let object = engine.state.objects.get_mut(&target).unwrap();
        object.tapped = true;
        object.add_counters(CounterKind::Stun, 1, 0);
        assert_eq!(attempt_untap(&mut engine, target), UntapOutcome::NoChange);
        assert_eq!(
            engine.state.objects[&target].counter_count(CounterKind::Stun),
            1
        );
        assert_eq!(
            engine.place_counters(target, CounterKind::PlusOnePlusOne, 1),
            0
        );
        assert_eq!(engine.remove_counters(target, CounterKind::Stun, 1), 1);
        assert_eq!(attempt_untap(&mut engine, target), UntapOutcome::NoChange);
        move_object_to_zone(
            &mut engine.state,
            engine.registry,
            aura,
            Zone::Graveyard,
            None,
        )
        .unwrap();
        assert_eq!(engine.place_counters(target, CounterKind::Stun, 1), 1);
        assert_eq!(
            attempt_untap(&mut engine, target),
            UntapOutcome::ReplacedByStun
        );
        assert_eq!(attempt_untap(&mut engine, target), UntapOutcome::Untapped);
    }

    #[test]
    fn issue_157_multiple_observers_recreate_the_same_departure_counter_bag() {
        use tricerules_cards::primitives::CounterSnapshotSource;
        let mut engine = GameEngine::new_with_default_decks(15709, &[0, 1], 20).unwrap();
        let departed = add_battlefield_object(&mut engine, 0, "dockworker_drone");
        let first = add_battlefield_object(&mut engine, 0, "hill_giant");
        let second = add_battlefield_object(&mut engine, 1, "hill_giant");
        let bag = BTreeMap::from([
            (CounterKind::PlusOnePlusOne, 2),
            (CounterKind::Stun, 3),
            (CounterKind::Keyword(Keyword::Flying), 1),
        ]);
        for (&kind, &count) in &bag {
            engine.place_counters(departed, kind, count);
        }
        move_object_to_zone(
            &mut engine.state,
            engine.registry,
            departed,
            Zone::Graveyard,
            None,
        )
        .unwrap();
        move_object_to_zone(
            &mut engine.state,
            engine.registry,
            departed,
            Zone::Battlefield,
            None,
        )
        .unwrap();
        for observer in [first, second] {
            let mut item = quantity_item(
                observer,
                vec![SpellEffectKind::PutCounterSnapshot {
                    from: CounterSnapshotSource::TriggerObject,
                    subject: EffectSubject::Source,
                }],
            );
            item.trigger_context.observed_object = Some(TriggerObjectRef {
                object_id: departed,
                zone_change_generation: 0,
                controller_at_event: 0,
            });
            let (effects, label) = engine.build_resolution_effects(&item);
            engine
                .run_effect_list(&item, &label, effects, 0, &mut Vec::new())
                .unwrap();
            assert_eq!(engine.state.objects[&observer].counters, bag);
        }
        assert_eq!(
            engine.state.last_known_counters_by_generation[&(departed, 0)],
            bag
        );
        assert!(engine.state.objects[&departed].counters.is_empty());
    }

    #[test]
    fn issue_167_custom_resolution_commits_graveyard_history_only_when_complete() {
        for (moved_again, initialized_generation) in
            [(false, false), (false, true), (true, false), (true, true)]
        {
            let mut engine = GameEngine::new_with_default_decks(167201, &[0, 1], 20).unwrap();
            // A permanent-front MDFC using the existing Brainstorm algorithm on its instant face
            // exercises destination card types and the custom park/resume boundary together.
            engine.registry = Box::leak(Box::new(
                CardRegistry::from_chunks_and_tokens(
                    &[
                        r#"(id: "brainstorm", name: "Brainstorm", layout: ModalDfc, faces: [
                (name: "Body", face_id: "body", types: ["Creature"], power: 2, toughness: 2),
                (name: "Thought", face_id: "thought", types: ["Instant"], custom_effect: "brainstorm")])"#,
                        include_str!("../../../../tricerules-cards/data/island.ron"),
                        include_str!("../../../../tricerules-cards/data/forest.ron"),
                    ],
                    &[],
                )
                .unwrap(),
            ));
            let oid = add_battlefield_object(&mut engine, 0, "brainstorm");
            move_object_to_zone(&mut engine.state, engine.registry, oid, Zone::Stack, None)
                .unwrap();
            if !initialized_generation {
                engine.state.zone_change_generation.remove(&oid);
            }
            let mut item = triggered_item(oid, 0);
            item.id = oid;
            item.card_id = "brainstorm".into();
            item.ability_text = None;
            item.is_triggered = false;
            item.source_permanent_id = None;
            item.face_index = 1;
            item.cast_occurrence = Some(crate::state::StackObjectRef {
                object_id: oid,
                zone_change_generation: Some(
                    engine
                        .state
                        .zone_change_generation
                        .get(&oid)
                        .copied()
                        .unwrap_or(0),
                ),
            });
            engine.state.stack.push(item);
            let mut events = Vec::new();
            engine.resolve_top_of_stack(&mut events).unwrap();
            assert!(engine.state.pending_resolution.is_some());
            assert_eq!(
                engine.state.objects[&oid].zone,
                Zone::Graveyard,
                "physical bookkeeping is unchanged"
            );
            assert!(
                engine
                    .state
                    .turn_history
                    .current
                    .permanent_cards_entered_graveyard
                    .is_empty(),
                "bookkeeping is not a committed rules event"
            );
            if moved_again {
                // The pending receipt must not attach to a new incarnation of the physical card.
                move_object_to_zone(&mut engine.state, engine.registry, oid, Zone::Hand, None)
                    .unwrap();
                move_object_to_zone(
                    &mut engine.state,
                    engine.registry,
                    oid,
                    Zone::Graveyard,
                    None,
                )
                .unwrap();
            }
            let chosen: Vec<_> = engine.state.players[0]
                .hand
                .iter()
                .take(2)
                .copied()
                .collect();
            let command = rv1::RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(
                    rv1::SubmitResolutionChoice {
                        chosen_object_ids: chosen,
                        ..Default::default()
                    },
                )),
            };
            let history = engine.state.turn_history.clone();
            let command_index = engine.state.command_index;
            assert!(engine.apply_command(1, &command).is_err());
            assert_eq!(engine.state.turn_history, history);
            assert_eq!(engine.state.command_index, command_index);
            engine.apply_command(0, &command).unwrap();
            assert!(engine.state.pending_resolution.is_none());
            assert!(engine.state.deferred_graveyard_entry.is_none());
            assert_eq!(
                engine
                    .state
                    .turn_history
                    .current
                    .permanent_cards_entered_graveyard
                    .len(),
                1
            );
            assert!(engine.apply_command(0, &command).is_err());
            assert_eq!(
                engine
                    .state
                    .turn_history
                    .current
                    .permanent_cards_entered_graveyard
                    .len(),
                1
            );
        }
    }

    #[test]
    fn issue_167_source_sacrifice_cannot_sacrifice_a_stolen_permanent() {
        let mut engine = GameEngine::new_with_default_decks(167202, &[0, 1], 20).unwrap();
        let oid = add_battlefield_object(&mut engine, 1, "grizzly_bears");
        let item = quantity_item(
            oid,
            vec![SpellEffectKind::Sacrifice {
                subject: EffectSubject::Source,
            }],
        );
        assert_eq!(item.controller, 0);
        let (effects, label) = engine.build_resolution_effects(&item);
        engine
            .run_effect_list(&item, &label, effects, 0, &mut Vec::new())
            .unwrap();
        assert_eq!(
            engine.state.objects[&oid].zone,
            Zone::Battlefield,
            "an old ability's controller cannot sacrifice the stolen source"
        );
        assert!(engine
            .state
            .turn_history
            .current
            .permanents_sacrificed
            .is_empty());
    }

    #[test]
    fn issue_167_mill_discard_and_countered_spell_routes_record_destination_cards() {
        let mut engine = GameEngine::new(
            167203,
            &[0, 1],
            20,
            Some(vec![vec!["island".into(); 30], vec!["forest".into(); 30]]),
            true,
        )
        .unwrap();
        let source = add_battlefield_object(&mut engine, 0, "grizzly_bears");
        let item = quantity_item(
            source,
            vec![SpellEffectKind::Mill {
                count: Amount::Fixed(2),
                who: PlayerRecipient::Controller,
            }],
        );
        let (effects, label) = engine.build_resolution_effects(&item);
        engine
            .run_effect_list(&item, &label, effects, 0, &mut Vec::new())
            .unwrap();
        assert_eq!(
            engine
                .state
                .turn_history
                .current
                .permanent_cards_entered_graveyard
                .len(),
            2
        );
        let discard = engine.state.players[0].hand[0];
        perform_discard(&mut engine.state, engine.registry, 0, discard).unwrap();
        assert_eq!(
            engine
                .state
                .turn_history
                .current
                .permanent_cards_entered_graveyard
                .len(),
            3
        );
        let history = engine.state.turn_history.clone();
        assert!(perform_discard(&mut engine.state, engine.registry, 0, discard).is_err());
        assert_eq!(engine.state.turn_history, history);
        for (card, face, method, count) in [
            ("grizzly_bears", 0, SpellCastMethod::Normal, 4),
            ("bonecrusher_giant_stomp", 1, SpellCastMethod::Normal, 5),
            ("bonecrusher_giant_stomp", 1, SpellCastMethod::Flashback, 5),
            ("shock", 0, SpellCastMethod::Normal, 5),
        ] {
            let oid = add_battlefield_object(&mut engine, 0, card);
            move_object_to_zone(&mut engine.state, engine.registry, oid, Zone::Stack, None)
                .unwrap();
            let mut spell = triggered_item(oid, 0);
            spell.id = oid;
            spell.card_id = card.into();
            spell.ability_text = None;
            spell.is_triggered = false;
            spell.face_index = face;
            spell.cast_method = method;
            engine.state.stack.push(spell);
            counter_stack_spell(&mut engine, oid, "test counter", &mut Vec::new()).unwrap();
            assert_eq!(
                engine
                    .state
                    .turn_history
                    .current
                    .permanent_cards_entered_graveyard
                    .len(),
                count,
                "{card}: {method:?}"
            );
        }
        assert!(engine
            .state
            .turn_history
            .current
            .permanents_sacrificed
            .is_empty());
        assert_eq!(engine.state.turn_history.current.creatures_died, 0);
    }

    #[test]
    fn issue_167_simultaneous_sacrifices_keep_predeparture_types() {
        let mut engine = GameEngine::new_with_default_decks(167204, &[0, 1], 20).unwrap();
        let first = add_battlefield_object(&mut engine, 0, "grizzly_bears");
        let second = add_battlefield_object(&mut engine, 0, "grizzly_bears");
        engine.state.continuous_effects.push(ContinuousEffect {
            source_id: Some(first),
            affected: AffectedScope::Single(second),
            kind: ContinuousEffectKind::Layer4AddTypes(tricerules_cards::TypeLineAddition {
                card_types: vec![PermanentTypeFilter::Artifact],
                creature_types: vec![],
            }),
            condition: None,
            duration: EffectDuration::WhileSourceOnBattlefield,
            timestamp: 1,
            trigger_grant_origin: None,
        });
        let refs: Vec<_> = [first, second]
            .into_iter()
            .map(|object_id| crate::state::TriggerObjectRef {
                object_id,
                zone_change_generation: 0,
                controller_at_event: 0,
            })
            .collect();
        let mut item = quantity_item(first, vec![SpellEffectKind::SacrificeObservedObjects]);
        item.trigger_context.observed_object = Some(refs[0]);
        engine
            .state
            .observed_object_cohorts
            .insert((first, 0), refs);
        let (effects, label) = engine.build_resolution_effects(&item);
        engine
            .run_effect_list(&item, &label, effects, 0, &mut Vec::new())
            .unwrap();
        let facts = &engine.state.turn_history.current.permanents_sacrificed;
        assert_eq!(facts.len(), 2);
        assert!(
            facts[1].types.iter().any(|kind| kind == "Artifact"),
            "the granting source left in the same instruction"
        );
        assert!(!engine
            .state
            .turn_history
            .current
            .permanent_cards_entered_graveyard[1]
            .types
            .iter()
            .any(|kind| kind == "Artifact"));
    }

    #[test]
    fn issue_165_dynamic_scry_freezes_private_candidates_and_resumes_tail_once() {
        fn run(power: u32) -> Vec<rv1::RuledEvent> {
            let mut engine = GameEngine::new_with_default_decks(165_201, &[0, 1], 20).unwrap();
            let source = add_battlefield_object(&mut engine, 0, "grizzly_bears");
            engine.state.objects.get_mut(&source).unwrap().power = Some(power);
            let item = quantity_item(
                source,
                vec![
                    SpellEffectKind::Scry {
                        count: Amount::Count(CountExpression::SourcePower),
                    },
                    SpellEffectKind::Draw {
                        count: Amount::Fixed(1),
                        who: PlayerRecipient::Controller,
                    },
                ],
            );
            let hand = engine.state.players[0].hand.len();
            let library: Vec<_> = engine.state.players[0].library.iter().copied().collect();
            let (effects, label) = engine.build_resolution_effects(&item);
            let mut events = Vec::new();
            engine
                .run_effect_list(&item, &label, effects, 0, &mut events)
                .unwrap();
            if power > 0 {
                let candidates = engine
                    .state
                    .pending_resolution
                    .as_ref()
                    .unwrap()
                    .presentation
                    .candidates
                    .clone();
                assert_eq!(candidates, library[..power as usize]);
                let choice = events
                    .iter()
                    .find_map(|event| match &event.ev {
                        Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(choice)) => {
                            Some(choice)
                        }
                        _ => None,
                    })
                    .unwrap();
                assert_eq!(choice.deciding_player_id, 0);
                assert_eq!(choice.reveal_audience, 0);
                assert_eq!(choice.candidate_object_ids, candidates);
                engine.state.objects.get_mut(&source).unwrap().power = Some(7);
                let answer = rv1::SubmitResolutionChoice {
                    chosen_object_ids: candidates,
                    ..Default::default()
                };
                assert!(engine.submit_resolution_choice(1, &answer).is_err());
                assert_eq!(
                    engine
                        .state
                        .pending_resolution
                        .as_ref()
                        .unwrap()
                        .presentation
                        .max,
                    power
                );
                events.extend(engine.submit_resolution_choice(0, &answer).unwrap().events);
                assert_eq!(
                    engine.state.players[0].hand.last(),
                    Some(&library[power as usize])
                );
            } else {
                assert!(
                    !events.iter().any(|event| matches!(&event.ev,
                    Some(rv1::ruled_event::Ev::Log(log)) if log.text.contains("scries"))),
                    "scry zero does nothing and must not claim the library was empty"
                );
            }
            assert_eq!(engine.state.players[0].hand.len(), hand + 1);
            assert!(engine.state.pending_resolution.is_none());
            events
        }
        for power in [0, 2] {
            assert_eq!(
                run(power),
                run(power),
                "same seed and choices reproduce the same events"
            );
        }
    }

    #[test]
    fn issue_165_dynamic_soft_counter_freezes_payment_and_allows_zero_or_decline() {
        for (power, decline, spend_treasures) in [
            (0, false, false),
            (0, true, false),
            (2, false, false),
            (2, false, true),
        ] {
            let mut engine = GameEngine::new_with_default_decks(165_202, &[0, 1], 20).unwrap();
            let source = add_battlefield_object(&mut engine, 0, "grizzly_bears");
            engine.state.objects.get_mut(&source).unwrap().power = Some(power);
            let mut target = triggered_item(source, 0);
            target.id += 1;
            target.controller = 1;
            let target_id = target.id;
            engine.state.stack.push(target);
            let treasures: Vec<_> = (0..if spend_treasures { power } else { 0 })
                .map(|_| add_battlefield_object(&mut engine, 1, "treasure"))
                .collect();
            let mut effect = engine
                .registry
                .get("convolute")
                .unwrap()
                .primary_face()
                .spell_effect[0]
                .clone();
            if let SpellEffectKind::CounterTargetSpell {
                unless_controller_pays,
                ..
            } = &mut effect
            {
                let quantity = if spend_treasures {
                    CountExpression::BattlefieldPermanents {
                        filter: BattlefieldPermanentFilter {
                            token: None,
                            controllers: RelativePlayerSet::Opponents,
                            card_type: Some(CardTypeFilter::Artifact),
                            any_of: None,
                            color: None,
                            name: None,
                            required_subtypes: vec![],
                            exclude_source: false,
                        },
                    }
                } else {
                    CountExpression::BattlefieldMaximum {
                        filter: BattlefieldPermanentFilter {
                            token: None,
                            controllers: RelativePlayerSet::Controller,
                            card_type: Some(CardTypeFilter::Creature),
                            any_of: None,
                            color: None,
                            name: None,
                            required_subtypes: vec![],
                            exclude_source: false,
                        },
                        characteristic: tricerules_cards::PowerToughnessCharacteristic::Power,
                    }
                };
                *unless_controller_pays = Some(Amount::Count(quantity));
            } else {
                panic!("Convolute soft counter");
            }
            let mut item = quantity_item(
                source,
                vec![
                    effect,
                    SpellEffectKind::GainLife {
                        amount: Amount::Fixed(1),
                    },
                ],
            );
            item.targets = vec![StackTarget {
                object_id: target_id,
                group_index: 0,
                damage_amount: 0,
                kind: 0,
                zone_change_generation: None,
            }];
            let (effects, label) = engine.build_resolution_effects(&item);
            let mut events = Vec::new();
            engine
                .run_effect_list(&item, &label, effects, 0, &mut events)
                .unwrap();
            assert_eq!(
                engine
                    .state
                    .pending_resolution
                    .as_ref()
                    .unwrap()
                    .continuation
                    .mana_payment()
                    .unwrap()
                    .generic_mana_cost,
                power
            );
            engine.state.objects.get_mut(&source).unwrap().power = Some(8);
            let mut answer = rv1::SubmitResolutionChoice {
                decision: if decline {
                    rv1::ResolutionChoiceDecision::Decline
                } else {
                    rv1::ResolutionChoiceDecision::PayMana
                } as i32,
                ..Default::default()
            };
            if power > 0 {
                assert!(engine.submit_resolution_choice(1, &answer).is_err());
                assert!(engine
                    .state
                    .stack
                    .iter()
                    .any(|object| object.id == target_id));
            }
            if spend_treasures {
                // This unit fixture runs the effect directly, bypassing the normal pass sequence.
                engine.state.priority_idx = 1;
                for (index, treasure) in treasures.iter().enumerate() {
                    engine
                        .apply_command(
                            1,
                            &RuledCommand {
                                cmd: Some(rv1::ruled_command::Cmd::ActivateAbility(
                                    rv1::ActivateAbility {
                                        source_object_id: *treasure,
                                        ..Default::default()
                                    },
                                )),
                            },
                        )
                        .unwrap();
                    assert!(!engine.state.players[1].battlefield.contains(treasure));
                    assert_eq!(
                        engine
                            .state
                            .pending_resolution
                            .as_ref()
                            .unwrap()
                            .continuation
                            .mana_payment()
                            .unwrap()
                            .generic_mana_cost,
                        power
                    );
                    if index == 0 {
                        assert!(
                            engine.submit_resolution_choice(1, &answer).is_err(),
                            "the cheaper live count must not replace the locked payment"
                        );
                    }
                }
            } else {
                engine.state.players[1].mana_pool.colorless = power;
            }
            if !decline {
                let preview = engine.preview_payment(
                    1,
                    &rv1::PreviewPayment {
                        transaction_id: 1,
                        revision: 1,
                        resolution_choice: Some(answer.clone()),
                        ..Default::default()
                    },
                );
                assert!(preview.valid, "{}", preview.error);
                answer.payment = preview.selection;
                let pool = engine.state.players[1].mana_pool;
                answer.payment.as_mut().unwrap().mana = Some(rv1::PaymentMana {
                    w: pool.white,
                    u: pool.blue,
                    b: pool.black,
                    r: pool.red,
                    g: pool.green,
                    c: pool.colorless,
                });
            }
            engine.submit_resolution_choice(1, &answer).unwrap();
            assert_eq!(engine.state.players[1].mana_pool.colorless, 0);
            assert_eq!(engine.state.players[1].mana_pool.white, 0);
            assert_eq!(
                engine
                    .state
                    .stack
                    .iter()
                    .any(|object| object.id == target_id),
                !decline
            );
            assert_eq!(engine.state.players[0].life, 21);
            assert!(engine.state.pending_resolution.is_none());
        }
    }

    #[test]
    fn issue_175_ferocidon_simultaneous_entry_uses_current_or_last_known_controller() {
        for departure in [0, 1, 2] {
            let mut engine = GameEngine::new_with_default_decks(175_301, &[0, 1], 20).unwrap();
            let ferocidon = add_battlefield_object(&mut engine, 1, "rampaging_ferocidon");
            let creature = add_battlefield_object(&mut engine, 0, "grizzly_bears");
            let triggers = engine.collect_event_triggers(&[
                GameEvent::EntersBattlefield {
                    object_id: ferocidon,
                    chosen_x: 0,
                },
                GameEvent::EntersBattlefield {
                    object_id: creature,
                    chosen_x: 0,
                },
            ]);
            assert_eq!(triggers.len(), 1, "another creature triggers even during simultaneous entry; Ferocidon excludes itself");
            let trigger = triggers.into_iter().next().unwrap();
            let mut item = triggered_item(ferocidon, 0);
            item.controller = 1;
            item.card_id = "rampaging_ferocidon".into();
            item.triggered_ability = Some(trigger.ability);
            item.trigger_context = trigger.trigger_context;
            engine.state.continuous_effects.push(ContinuousEffect {
                trigger_grant_origin: None,
                source_id: None,
                affected: AffectedScope::Single(creature),
                kind: ContinuousEffectKind::Layer2Control {
                    controller: tricerules_cards::ControllerReference::Fixed(1),
                },
                condition: None,
                duration: EffectDuration::UntilEndOfTurn,
                timestamp: engine.state.command_index,
            });
            let mut events = Vec::new();
            engine.apply_sbas(&mut events).unwrap();
            if departure > 0 {
                move_object_to_zone(
                    &mut engine.state,
                    engine.registry,
                    creature,
                    Zone::Hand,
                    None,
                )
                .unwrap();
            }
            if departure == 2 {
                move_object_to_zone(
                    &mut engine.state,
                    engine.registry,
                    creature,
                    Zone::Battlefield,
                    None,
                )
                .unwrap();
                assert_eq!(
                    engine.controller_of(creature),
                    Some(0),
                    "new occurrence belongs to its owner"
                );
            }
            let (effects, label) = engine.build_resolution_effects(&item);
            engine
                .run_effect_list(&item, &label, effects, 0, &mut events)
                .unwrap();
            assert_eq!(
                engine.state.players[0].life, 20,
                "event-time controller is not necessarily last known; departure={departure}"
            );
            assert_eq!(
                engine.state.players[1].life, 19,
                "current or last-known controller takes damage; departure={departure}"
            );
        }
    }

    #[test]
    fn issue_175_gain_routes_continue_the_effect_tail_and_preserve_loss() {
        let player_target = TargetFilter {
            kind: TargetKind::AnyPlayer,
            ..Default::default()
        };
        let cases = [
            (
                SpellEffectKind::GainLife {
                    amount: Amount::Fixed(3),
                },
                vec![],
                0,
            ),
            (
                SpellEffectKind::TargetPlayerGainsLife {
                    amount: 3,
                    target: player_target.clone(),
                },
                vec![0],
                0,
            ),
            (
                SpellEffectKind::DrainTarget {
                    amount: 3,
                    target: player_target,
                },
                vec![1],
                3,
            ),
            (
                SpellEffectKind::EachOpponentLosesLifeYouGainEqual { amount: 3 },
                vec![],
                3,
            ),
        ];
        for (effect, targets, lost) in cases {
            for prohibited in [false, true] {
                let mut engine = GameEngine::new_with_default_decks(175_302, &[0, 1], 20).unwrap();
                let source = add_battlefield_object(&mut engine, 0, "grizzly_bears");
                if prohibited {
                    add_battlefield_object(&mut engine, 1, "giant_cindermaw");
                }
                let item = triggered_item(source, 0);
                let library_before = engine.state.players[0].library.len();
                let mut events = Vec::new();
                let entries = [
                    effect.clone(),
                    SpellEffectKind::Draw {
                        count: Amount::Fixed(1),
                        who: PlayerRecipient::Controller,
                    },
                ]
                .into_iter()
                .map(|effect| ResolutionEffect {
                    effect,
                    targets: targets.clone(),
                    target_damage: vec![],
                    target_group_indices: vec![0; targets.len()],
                    role_group_indices: vec![],
                })
                .collect();
                engine
                    .run_effect_list(&item, "gain then draw", entries, 0, &mut events)
                    .unwrap();
                assert_eq!(
                    engine.state.players[0].life,
                    if prohibited { 20 } else { 23 },
                    "{effect:?}"
                );
                assert_eq!(engine.state.players[1].life, 20 - lost);
                assert_eq!(
                    engine.state.turn_history.current.player(0).life_gained,
                    if prohibited { 0 } else { 3 },
                    "{effect:?}"
                );
                assert_eq!(
                    engine.state.turn_history.current.player(1).life_lost,
                    lost as u64
                );
                assert_eq!(
                    engine.state.players[0].library.len(),
                    library_before - 1,
                    "the draw tail still resolves"
                );
                assert_eq!(events.iter().any(|event| matches!(event.ev.as_ref(), Some(rv1::ruled_event::Ev::LifeChanged(life)) if life.delta > 0)), !prohibited);
            }
        }
    }

    #[test]
    fn issue_170_direct_loss_effects_and_chosen_player_conditions_share_history() {
        for amount in [0, 2] {
            let mut engine = GameEngine::new_with_default_decks(170007, &[0, 1], 20).unwrap();
            let source = add_battlefield_object(&mut engine, 0, "grizzly_bears");
            let mut item = triggered_item(source, 0);
            item.targets = vec![super::super::targeting::capture_stack_target(
                &engine,
                &rv1::TargetRef {
                    object_id: 1,
                    group_index: 0,
                    ..Default::default()
                },
            )];
            let entries = [
                (
                    SpellEffectKind::LoseLife {
                        amount: LifeAmount::Fixed(amount),
                        who: PlayerRecipient::Controller,
                    },
                    vec![],
                ),
                (
                    SpellEffectKind::TargetPlayerLosesLife {
                        amount,
                        target: TargetFilter {
                            kind: TargetKind::AnyPlayer,
                            ..Default::default()
                        },
                    },
                    vec![1],
                ),
            ]
            .into_iter()
            .map(|(effect, targets)| ResolutionEffect {
                target_group_indices: vec![0; targets.len()],
                targets,
                effect,
                target_damage: vec![],
                role_group_indices: vec![],
            })
            .collect();
            engine
                .run_effect_list(&item, "life loss", entries, 0, &mut Vec::new())
                .unwrap();
            for player in [0, 1] {
                assert_eq!(
                    engine.state.turn_history.current.player(player).life_lost,
                    u64::from(amount)
                );
                assert_eq!(
                    engine.state.turn_history.current.player(player).life_gained,
                    0
                );
            }
            assert_eq!(
                engine.condition_holds(
                    &GameCondition::LifeChangedThisTurn {
                        players: ConditionPlayerSet::ChosenTarget {
                            group_index: 0,
                            target_index: 0
                        },
                        change: LifeChangeKind::Loss,
                        quantifier: PlayerQuantifier::Any,
                    },
                    ConditionContext::for_stack_item(&item)
                ),
                amount > 0
            );
        }
    }

    #[test]
    fn attached_subject_uses_generation_scoped_lki_after_source_exits() {
        let mut engine = GameEngine::new_with_default_decks(82_101, &[0, 1], 20).expect("engine");
        let source = add_battlefield_object(&mut engine, 0, "capture_sphere");
        let creature = add_battlefield_object(&mut engine, 1, "grizzly_bears");
        engine.state.objects.get_mut(&source).unwrap().attached_to =
            Some(AttachmentRecipient::Object(creature));
        engine.emit_static_abilities_on_enter(source);
        assert!(engine.doesnt_untap_during_untap_step(creature));

        let generation = engine
            .state
            .zone_change_generation
            .get(&source)
            .copied()
            .unwrap_or(0);
        let item = triggered_item(source, generation);
        move_object_to_zone(
            &mut engine.state,
            engine.registry,
            source,
            Zone::Graveyard,
            None,
        )
        .expect("move Aura");

        assert_eq!(
            resolve_effect_subject(&engine, &item, &[], &EffectSubject::AttachedObject),
            Some(creature),
            "the trigger remembers what the departed Aura enchanted"
        );
        assert!(
            !engine.doesnt_untap_during_untap_step(creature),
            "the departed Aura's static effect stops immediately"
        );
    }

    #[test]
    fn attached_subject_lki_rejects_an_attached_object_that_left_and_returned() {
        let mut engine = GameEngine::new_with_default_decks(82_102, &[0, 1], 20).expect("engine");
        let source = add_battlefield_object(&mut engine, 0, "capture_sphere");
        let creature = add_battlefield_object(&mut engine, 1, "grizzly_bears");
        engine.state.objects.get_mut(&source).unwrap().attached_to =
            Some(AttachmentRecipient::Object(creature));
        let item = triggered_item(
            source,
            engine
                .state
                .zone_change_generation
                .get(&source)
                .copied()
                .unwrap_or(0),
        );

        move_object_to_zone(
            &mut engine.state,
            engine.registry,
            source,
            Zone::Graveyard,
            None,
        )
        .expect("move Aura");
        move_object_to_zone(
            &mut engine.state,
            engine.registry,
            creature,
            Zone::Graveyard,
            None,
        )
        .expect("move creature out");
        move_object_to_zone(
            &mut engine.state,
            engine.registry,
            creature,
            Zone::Battlefield,
            None,
        )
        .expect("return creature");

        assert_eq!(
            resolve_effect_subject(&engine, &item, &[], &EffectSubject::AttachedObject),
            None,
            "CR 400.7 makes the returned creature a different object"
        );
    }

    #[test]
    fn current_attached_subject_is_empty_after_detachment() {
        let mut engine = GameEngine::new_with_default_decks(82_103, &[0, 1], 20).expect("engine");
        let source = add_battlefield_object(&mut engine, 0, "capture_sphere");
        let item = triggered_item(
            source,
            engine
                .state
                .zone_change_generation
                .get(&source)
                .copied()
                .unwrap_or(0),
        );

        assert_eq!(
            resolve_effect_subject(&engine, &item, &[], &EffectSubject::AttachedObject),
            None
        );
    }

    #[test]
    fn trigger_object_subject_rejects_a_leave_and_return_generation() {
        let mut engine = GameEngine::new_with_default_decks(82_104, &[0, 1], 20).expect("engine");
        let source = add_battlefield_object(&mut engine, 0, "grizzly_bears");
        let related = add_battlefield_object(&mut engine, 1, "grizzly_bears");
        let related_generation = engine
            .state
            .zone_change_generation
            .get(&related)
            .copied()
            .unwrap_or(0);
        let mut item = triggered_item(
            source,
            engine
                .state
                .zone_change_generation
                .get(&source)
                .copied()
                .unwrap_or(0),
        );
        item.trigger_context.observed_object = Some(TriggerObjectRef {
            object_id: related,
            zone_change_generation: related_generation,
            controller_at_event: 1,
        });

        assert_eq!(
            resolve_effect_subject(&engine, &item, &[], &EffectSubject::TriggerObject),
            Some(related)
        );
        move_object_to_zone(
            &mut engine.state,
            engine.registry,
            related,
            Zone::Graveyard,
            None,
        )
        .expect("move related object out");
        move_object_to_zone(
            &mut engine.state,
            engine.registry,
            related,
            Zone::Battlefield,
            None,
        )
        .expect("return related object");
        assert_eq!(
            resolve_effect_subject(&engine, &item, &[], &EffectSubject::TriggerObject),
            None,
            "CR 400.7 prevents the trigger from affecting the returned object"
        );
    }

    #[test]
    fn put_counters_applicability_accepts_a_current_noncreature_permanent() {
        let mut engine = GameEngine::new_with_default_decks(142_101, &[0, 1], 20).expect("engine");
        let source = add_battlefield_object(&mut engine, 0, "forest");
        let generation = engine
            .state
            .zone_change_generation
            .get(&source)
            .copied()
            .unwrap_or(0);
        let item = triggered_item(source, generation);

        assert!(pump_counters::can_put_counters(
            &engine,
            &item,
            &[],
            &EffectSubject::Source,
        ));
    }

    #[test]
    fn issue_153_tatterkite_cannot_pay_a_counter_placement() {
        let mut engine = GameEngine::new_with_default_decks(153_001, &[0, 1], 20).expect("engine");
        let source = add_battlefield_object(&mut engine, 0, "tatterkite");
        let generation = engine
            .state
            .zone_change_generation
            .get(&source)
            .copied()
            .unwrap_or(0);
        let item = triggered_item(source, generation);
        assert!(
            !pump_counters::can_put_counters(&engine, &item, &[], &EffectSubject::Source,),
            "Tatterkite cannot receive counters as an optional payment"
        );
        for kind in [
            CounterKind::MinusOneMinusOne,
            CounterKind::PlusOnePlusOne,
            CounterKind::Loyalty,
            CounterKind::Stun,
        ] {
            assert_eq!(engine.place_counters(source, kind, 2), 0);
            assert_eq!(engine.state.objects[&source].counter_count(kind), 0);
        }
        let item = quantity_item(
            source,
            vec![
                SpellEffectKind::PutCounters {
                    counter: CounterKind::PlusOnePlusOne,
                    count: Amount::Fixed(1),
                    subject: EffectSubject::Source,
                },
                SpellEffectKind::GainLife {
                    amount: Amount::Fixed(2),
                },
            ],
        );
        let (effects, label) = engine.build_resolution_effects(&item);
        engine
            .run_effect_list(&item, &label, effects, 0, &mut Vec::new())
            .unwrap();
        assert_eq!(
            engine.state.players[0].life, 22,
            "prohibition does not counter the effect tail"
        );
        assert_eq!(
            engine.state.objects[&source].counter_count(CounterKind::PlusOnePlusOne),
            0
        );
    }

    #[test]
    fn issue_153_counter_prohibition_tracks_attachment_and_ability_removal() {
        use tricerules_cards::primitives::CounterPlacementAffected;
        let mut engine = GameEngine::new_with_default_decks(153_002, &[0, 1], 20).unwrap();
        let aura = add_battlefield_object(&mut engine, 0, "pacifism");
        let first = add_battlefield_object(&mut engine, 0, "grizzly_bears");
        let second = add_battlefield_object(&mut engine, 1, "grizzly_bears");
        engine.place_counters(first, CounterKind::PlusOnePlusOne, 1);
        let mut values = engine.copiable_values_for(aura).unwrap();
        values.face.static_abilities = vec![tricerules_cards::IdentifiedAbility::fallback(
            "static_01",
            StaticAbilityDef::ProhibitCounters {
                affected: CounterPlacementAffected::AttachedPermanent,
            },
        )
        .unwrap()];
        engine.state.objects.get_mut(&aura).unwrap().copiable_values = Some(values);
        engine.state.objects.get_mut(&aura).unwrap().attached_to =
            Some(AttachmentRecipient::Object(first));
        assert!(!engine.can_receive_counters(first));
        assert_eq!(
            engine.state.objects[&first].counter_count(CounterKind::PlusOnePlusOne),
            1
        );
        engine.state.objects.get_mut(&aura).unwrap().attached_to =
            Some(AttachmentRecipient::Object(second));
        assert!(engine.can_receive_counters(first));
        assert!(!engine.can_receive_counters(second));
        engine.state.continuous_effects.push(ContinuousEffect {
            source_id: None,
            trigger_grant_origin: None,
            affected: AffectedScope::Single(aura),
            kind: ContinuousEffectKind::Layer6RemoveAllAbilities,
            condition: None,
            duration: EffectDuration::UntilEndOfTurn,
            timestamp: engine.state.command_index,
        });
        assert!(engine.can_receive_counters(second));
        engine.state.continuous_effects.clear();
        assert!(!engine.can_receive_counters(second));
        move_object_to_zone(
            &mut engine.state,
            engine.registry,
            aura,
            Zone::Graveyard,
            None,
        )
        .unwrap();
        assert!(engine.can_receive_counters(second));
    }

    #[test]
    fn issue_153_forced_impossible_blight_records_receipt_before_a_parked_tail() {
        let mut engine = GameEngine::new_with_default_decks(153_003, &[0, 1], 20).unwrap();
        let source = add_battlefield_object(&mut engine, 0, "tatterkite");
        let item = quantity_item(
            source,
            vec![
                SpellEffectKind::Blight { count: 2 },
                SpellEffectKind::Discard {
                    who: PlayerRecipient::EachOpponent,
                    count: 1,
                },
            ],
        );
        let (effects, label) = engine.build_resolution_effects(&item);
        engine
            .run_effect_list(&item, &label, effects, 0, &mut Vec::new())
            .unwrap();
        let pending = engine.state.pending_resolution.as_ref().unwrap();
        assert_eq!(pending.deciding_player, 1);
        assert_eq!(pending.presentation.choice_kind, rv1::ChoiceKind::HandCards);
        let receipts = &pending.continuation.stack().unwrap().item.blight_receipts;
        assert_eq!(
            receipts,
            &[crate::state::BlightReceipt {
                player: 0,
                count: 2,
                creature: None
            }]
        );
    }

    #[test]
    fn issue_211_failed_counter_placement_does_not_create_reflexive_trigger() {
        let mut engine = GameEngine::new_with_default_decks(211_001, &[0, 1], 20).unwrap();
        let source = add_battlefield_object(&mut engine, 0, "tatterkite");
        let item = quantity_item(
            source,
            vec![
                SpellEffectKind::PutCounters {
                    counter: CounterKind::PlusOnePlusOne,
                    count: Amount::Fixed(1),
                    subject: EffectSubject::Source,
                },
                SpellEffectKind::CreateReflexiveTrigger {
                    when: Some(
                        tricerules_cards::primitives::ResolutionReceiptCondition::CountersPlaced {
                            counter: CounterKind::PlusOnePlusOne,
                            object: tricerules_cards::primitives::ConditionObjectRef::Source,
                        },
                    ),
                    ability: Box::new(tricerules_cards::primitives::ReflexiveTriggeredAbilityDef {
                        ability_id: tricerules_cards::AbilityId::new("reflexive_01").unwrap(),
                        presentation: tricerules_cards::AbilityPresentation::Fallback,
                        effect: vec![SpellEffectKind::GainLife {
                            amount: Amount::Fixed(1),
                        }],
                        targeting: None,
                        intervening_if: None,
                    }),
                },
            ],
        );
        let (effects, label) = engine.build_resolution_effects(&item);

        engine
            .run_effect_list(&item, &label, effects, 0, &mut Vec::new())
            .unwrap();

        assert_eq!(
            engine.state.objects[&source].counter_count(CounterKind::PlusOnePlusOne),
            0
        );
        assert!(
            engine.state.staged_trigger_groups.is_empty(),
            "the reflexive trigger requires a successful counter-placement receipt"
        );
    }

    #[test]
    fn forced_resolution_branch_runs_its_tail_exactly_once() {
        let mut engine = GameEngine::new_with_default_decks(142_102, &[0, 1], 20).expect("engine");
        let source = add_battlefield_object(&mut engine, 0, "grizzly_bears");
        let generation = engine
            .state
            .zone_change_generation
            .get(&source)
            .copied()
            .unwrap_or(0);
        let mut item = triggered_item(source, generation);
        item.triggered_ability = Some(TriggeredAbilityDef {
            ability_id: tricerules_cards::AbilityId::new("triggered_01").unwrap(),
            presentation: tricerules_cards::AbilityPresentation::Fallback,
            trigger: TriggerCondition::WhenSelfEntersBattlefield,
            effect: vec![
                SpellEffectKind::ChooseResolutionBranch {
                    chooser: PlayerRecipient::Controller,
                    optional: false,
                    selection:
                        tricerules_cards::primitives::ResolutionBranchSelection::PlayerChoice,
                    branches: vec![ResolutionBranchDef {
                        branch_id: tricerules_cards::ChoiceId::new("branch_01").unwrap(),
                        presentation: tricerules_cards::AbilityPresentation::Fallback,
                        runtime_fallback: None,
                        cost: ResolutionCost::None,
                        requirement:
                            tricerules_cards::primitives::ResolutionBranchRequirement::Always,
                        effects: vec![SpellEffectKind::GainLife {
                            amount: Amount::Fixed(1),
                        }],
                    }],
                },
                SpellEffectKind::GainLife {
                    amount: Amount::Fixed(2),
                },
            ],
            modal: None,
            targeting: None,
            may: false,
            intervening_if: None,
            max_triggers_per_turn: None,
            triggers_only_once: false,
        });
        let (effects, label) = engine.build_resolution_effects(&item);
        let mut events = Vec::new();

        engine
            .run_effect_list(&item, &label, effects, 0, &mut events)
            .expect("resolve forced branch and tail");

        assert_eq!(engine.state.players[0].life, 23);
        assert!(engine.state.pending_resolution.is_none());
        assert_eq!(
            events
                .iter()
                .filter(|event| matches!(event.ev, Some(rv1::ruled_event::Ev::LifeChanged(_))))
                .count(),
            2,
        );
    }

    #[test]
    fn first_applicable_resolution_branch_uses_authored_order_and_runs_its_tail_once() {
        let mut engine = GameEngine::new_with_default_decks(116_102, &[0, 1], 20).expect("engine");
        let source = add_battlefield_object(&mut engine, 0, "grizzly_bears");
        let generation = engine
            .state
            .zone_change_generation
            .get(&source)
            .copied()
            .unwrap_or(0);
        let mut item = triggered_item(source, generation);
        item.triggered_ability = Some(TriggeredAbilityDef {
            ability_id: tricerules_cards::AbilityId::new("triggered_01").unwrap(),
            presentation: tricerules_cards::AbilityPresentation::Fallback,
            trigger: TriggerCondition::WhenSelfEntersBattlefield,
            effect: vec![
                SpellEffectKind::ChooseResolutionBranch {
                    chooser: PlayerRecipient::Controller,
                    optional: false,
                    selection:
                        tricerules_cards::primitives::ResolutionBranchSelection::FirstApplicable,
                    branches: vec![
                        ResolutionBranchDef {
                            branch_id: tricerules_cards::ChoiceId::new("branch_01").unwrap(),
                            presentation: tricerules_cards::AbilityPresentation::Fallback,
                            runtime_fallback: None,
                            cost: ResolutionCost::None,
                            requirement: tricerules_cards::primitives::ResolutionBranchRequirement::GameCondition(
                                GameCondition::ActivePlayer {
                                    players: RelativePlayerSet::Controller,
                                },
                            ),
                            effects: vec![SpellEffectKind::GainLife {
                                amount: Amount::Fixed(1),
                            }],
                        },
                        ResolutionBranchDef {
                            branch_id: tricerules_cards::ChoiceId::new("branch_02").unwrap(),
                            presentation: tricerules_cards::AbilityPresentation::Fallback,
                            runtime_fallback: None,
                            cost: ResolutionCost::None,
                            requirement:
                                tricerules_cards::primitives::ResolutionBranchRequirement::Always,
                            effects: vec![SpellEffectKind::GainLife {
                                amount: Amount::Fixed(5),
                            }],
                        },
                    ],
                },
                SpellEffectKind::GainLife {
                    amount: Amount::Fixed(2),
                },
            ],
            modal: None,
            targeting: None,
            may: false,
            intervening_if: None,
            max_triggers_per_turn: None,
            triggers_only_once: false,
        });
        let (effects, label) = engine.build_resolution_effects(&item);
        let mut events = Vec::new();

        engine
            .run_effect_list(&item, &label, effects, 0, &mut events)
            .expect("resolve first applicable branch and tail");

        assert_eq!(engine.state.players[0].life, 23);
        assert!(engine.state.pending_resolution.is_none());
        assert_eq!(
            events
                .iter()
                .filter(|event| matches!(event.ev, Some(rv1::ruled_event::Ev::LifeChanged(_))))
                .count(),
            2,
        );
    }

    #[test]
    fn optional_resolution_branch_with_no_legal_option_skips_to_the_tail() {
        let mut engine = GameEngine::new_with_default_decks(142_103, &[0, 1], 20).expect("engine");
        let source = add_battlefield_object(&mut engine, 0, "grizzly_bears");
        let generation = engine
            .state
            .zone_change_generation
            .get(&source)
            .copied()
            .unwrap_or(0);
        let mut item = triggered_item(source, generation.saturating_add(1));
        item.triggered_ability = Some(TriggeredAbilityDef {
            ability_id: tricerules_cards::AbilityId::new("triggered_01").unwrap(),
            presentation: tricerules_cards::AbilityPresentation::Fallback,
            trigger: TriggerCondition::WhenSelfEntersBattlefield,
            effect: vec![
                SpellEffectKind::ChooseResolutionBranch {
                    chooser: PlayerRecipient::Controller,
                    optional: true,
                    selection:
                        tricerules_cards::primitives::ResolutionBranchSelection::PlayerChoice,
                    branches: vec![ResolutionBranchDef {
                        branch_id: tricerules_cards::ChoiceId::new("branch_01").unwrap(),
                        presentation: tricerules_cards::AbilityPresentation::Fallback,
                        runtime_fallback: None,
                        cost: ResolutionCost::None,
                        requirement:
                            tricerules_cards::primitives::ResolutionBranchRequirement::EffectsApplicable,
                        effects: vec![SpellEffectKind::PutCounters {
                            counter: CounterKind::PlusOnePlusOne,
                            count: Amount::Fixed(1),
                            subject: EffectSubject::Source,
                        }],
                    }],
                },
                SpellEffectKind::GainLife {
                    amount: Amount::Fixed(2),
                },
            ],
            modal: None,
            targeting: None,
            may: false,
            intervening_if: None,
            max_triggers_per_turn: None,
            triggers_only_once: false,
        });
        let (effects, label) = engine.build_resolution_effects(&item);
        let mut events = Vec::new();

        engine
            .run_effect_list(&item, &label, effects, 0, &mut events)
            .expect("skip impossible optional branch and resolve tail");

        assert_eq!(engine.state.players[0].life, 22);
        assert!(engine.state.pending_resolution.is_none());
        assert_eq!(
            engine
                .state
                .objects
                .get(&source)
                .expect("source")
                .counter_count(CounterKind::PlusOnePlusOne),
            0,
        );
    }

    #[test]
    fn issue_86_attacking_recipient_is_player_set_generic_and_distinct_from_controller() {
        let mut engine =
            GameEngine::new(86_107, &[10, 20], 20, None, true).expect("two-player engine");
        engine.state.players.push(PlayerState::new(30, 20));
        let source = add_battlefield_object(&mut engine, 10, "capture_sphere");
        let attacker = add_battlefield_object(&mut engine, 30, "grizzly_bears");
        engine.state.combat = Some(CombatState {
            attacking: vec![attacker],
            attack_assignments: HashMap::new(),
            blockers: HashMap::new(),
            damage_assignments: HashMap::new(),
            trample_player_damage: HashMap::new(),
            damage_assignment_needed: false,
            attackers_declared: true,
            blockers_declared: false,
            assign_combat_damage_phase: false,
            first_strike_attackers: Vec::new(),
            first_strike_blockers: HashMap::new(),
            first_strike_damage_done: false,
        });
        let mut item = triggered_item(source, 0);
        item.controller = 10;
        item.trigger_context.attacking_player = Some(30);
        item.trigger_context.defending_player = Some(20);
        let mut events = Vec::new();
        let previous_effect_result = EffectResult::default();
        let mut effect_result = EffectResult::default();
        let cx = EffectCx {
            engine: &mut engine,
            events: &mut events,
            targets: &[],
            targets_by_role: &[],
            target_damage: &[],
            target_group_indices: &[],
            top: &item,
            controller: 10,
            affected_player: 10,
            spell_label: "Curse",
            previous_effect_result: &previous_effect_result,
            effect_result: &mut effect_result,
            effect_index: 0,
        };

        assert_eq!(player_recipients(&cx, PlayerRecipient::Controller), [10]);
        assert_eq!(
            player_recipients(&cx, PlayerRecipient::AttackingOpponentsOfDefendingPlayer),
            [30]
        );
        cx.engine
            .state
            .combat
            .as_mut()
            .expect("combat")
            .attacking
            .clear();
        assert!(
            player_recipients(&cx, PlayerRecipient::AttackingOpponentsOfDefendingPlayer).is_empty()
        );
    }
}

#[cfg(test)]
mod source_keyword_tests {
    use super::*;

    #[test]
    fn issue_157_wither_preserves_prevention_lifelink_deathtouch_and_source_lki() {
        use crate::engine::damage::{DamageEvent, DamageRecipient, DamageSpec};
        for left_and_returned in [false, true] {
            for prohibited in [false, true] {
                for prevented in [0, 1, 3] {
                    let mut engine =
                        GameEngine::new_with_default_decks(15704, &[0, 1], 20).unwrap();
                    let source = add_three_toughness_creature(&mut engine, 0);
                    let target = add_three_toughness_creature(&mut engine, 1);
                    engine.state.continuous_effects.push(ContinuousEffect {
                        trigger_grant_origin: None,
                        source_id: None,
                        affected: AffectedScope::Single(source),
                        kind: ContinuousEffectKind::Layer6AddKeyword(Keyword::Wither),
                        condition: None,
                        duration: EffectDuration::UntilEndOfTurn,
                        timestamp: 0,
                    });
                    let item = ability_item(source, 0);
                    if left_and_returned {
                        move_object_to_zone(
                            &mut engine.state,
                            engine.registry,
                            source,
                            Zone::Graveyard,
                            None,
                        )
                        .unwrap();
                        move_object_to_zone(
                            &mut engine.state,
                            engine.registry,
                            source,
                            Zone::Battlefield,
                            None,
                        )
                        .unwrap();
                        assert!(!engine.effective_has_keyword(source, Keyword::Wither));
                    }
                    if prohibited {
                        let aura = add_three_toughness_creature(&mut engine, 0);
                        let object = engine.state.objects.get_mut(&aura).unwrap();
                        object.card_id = "blossombind".into();
                        object.attached_to = Some(AttachmentRecipient::Object(target));
                    }
                    if prevented > 0 {
                        engine.add_damage_prevention(
                            None,
                            "shield",
                            DamagePreventionScope::Recipient(target),
                            DamagePreventionAmount::Remaining(prevented),
                        );
                    }
                    let mut events = Vec::new();
                    let completed = engine
                        .process_or_park_damage_batch(
                            &item,
                            vec![DamageSpec {
                                event: DamageEvent::noncombat(
                                    source,
                                    0,
                                    "Wither source",
                                    DamageRecipient::Permanent(target),
                                    3,
                                ),
                                source_has_deathtouch: true,
                                source_has_lifelink: true,
                            }],
                            &mut events,
                        )
                        .unwrap();
                    engine.commit_completed_damage_batch(&completed, &mut events);
                    let dealt = 3 - prevented;
                    let object = &engine.state.objects[&target];
                    assert_eq!(object.damage, 0);
                    assert_eq!(
                        object.counter_count(CounterKind::MinusOneMinusOne),
                        if prohibited { 0 } else { dealt }
                    );
                    assert_eq!(object.deathtouch_damage, dealt > 0);
                    assert_eq!(engine.state.players[0].life, 20 + dealt as i32);
                    assert_eq!(completed[0].result.prevented, prevented);
                }
            }
        }
    }

    fn ability_item(source: ObjectId, generation: u64) -> StackItem {
        StackItem {
            id: source + 1,
            controller: 0,
            card_id: "prodigal_sorcerer".to_string(),
            targets: vec![],
            ability_text: Some("ping".to_string()),
            source_permanent_id: Some(source),
            source_owner: Some(0),
            source_zone_change: generation,
            source_face_change: 0,
            ability_index: Some(0),
            activated_ability: None,
            triggered_ability: None,
            is_triggered: false,
            is_copy: false,
            face_index: 0,
            cast_method: SpellCastMethod::Normal,
            sneak_attack: None,
            chosen_x: 0,
            chosen_modes: vec![],
            cast_condition_results: Vec::new(),
            cast_occurrence: None,
            cast_cost_receipts: vec![],
            payment_result: CardResultCohort::default(),
            resolution_branch_choices: Default::default(),
            blight_receipts: Vec::new(),
            trigger_context: TriggerContext::default(),
        }
    }

    fn deathtouch_spell_item(chosen_x: u32) -> StackItem {
        StackItem {
            id: u32::MAX,
            controller: 0,
            card_id: "pharikas_chosen".to_string(),
            targets: vec![],
            ability_text: None,
            source_permanent_id: None,
            source_owner: None,
            source_zone_change: 0,
            source_face_change: 0,
            ability_index: None,
            activated_ability: None,
            triggered_ability: None,
            is_triggered: false,
            is_copy: false,
            face_index: 0,
            cast_method: SpellCastMethod::Normal,
            sneak_attack: None,
            chosen_x,
            chosen_modes: vec![],
            cast_condition_results: Vec::new(),
            cast_occurrence: None,
            cast_cost_receipts: vec![],
            payment_result: CardResultCohort::default(),
            resolution_branch_choices: Default::default(),
            blight_receipts: Vec::new(),
            trigger_context: TriggerContext::default(),
        }
    }

    fn add_three_toughness_creature(engine: &mut GameEngine, controller: PlayerId) -> ObjectId {
        let id = engine.state.next_object_id;
        engine.state.next_object_id += 1;
        engine.state.objects.insert(
            id,
            GameObject {
                id,
                owner: controller,
                base_controller: controller,
                controller,
                card_id: "hill_giant".to_string(),
                token_origin: None,
                copiable_values: None,
                copy_revision: 0,
                zone: Zone::Battlefield,
                tapped: false,
                summoning_sick: false,
                power: None,
                toughness: None,
                damage: 0,
                deathtouch_damage: false,
                counters: BTreeMap::new(),
                counter_timestamps: BTreeMap::new(),
                attached_to: None,
                regeneration_shields: 0,
                must_attack_if_able: false,
                must_block_if_able: false,
                face_up_index: 0,
                face_down: false,
            },
        );
        let player_index = engine.state.player_idx(controller).unwrap();
        engine.state.players[player_index].battlefield.push(id);
        id
    }

    #[test]
    fn source_keyword_lki_is_generation_scoped_across_leave_and_return() {
        let mut engine = GameEngine::new_with_default_decks(7022, &[0, 1], 20).expect("new engine");
        let source = engine.state.next_object_id;
        engine.state.next_object_id += 2;
        engine.state.objects.insert(
            source,
            GameObject {
                id: source,
                owner: 0,
                base_controller: 0,
                controller: 0,
                card_id: "prodigal_sorcerer".to_string(),
                token_origin: None,
                copiable_values: None,
                copy_revision: 0,
                zone: Zone::Battlefield,
                tapped: false,
                summoning_sick: false,
                power: None,
                toughness: None,
                damage: 0,
                deathtouch_damage: false,
                counters: BTreeMap::new(),
                counter_timestamps: BTreeMap::new(),
                attached_to: None,
                regeneration_shields: 0,
                must_attack_if_able: false,
                must_block_if_able: false,
                face_up_index: 0,
                face_down: false,
            },
        );
        engine.state.players[0].battlefield.push(source);
        engine.state.continuous_effects.push(ContinuousEffect {
            trigger_grant_origin: None,
            source_id: None,
            affected: AffectedScope::Single(source),
            kind: ContinuousEffectKind::Layer6AddKeyword(Keyword::Deathtouch),
            condition: None,
            duration: EffectDuration::UntilEndOfTurn,
            timestamp: 0,
        });
        let original_ability = ability_item(source, 0);
        assert!(engine.resolving_source_has_keyword(&original_ability, Keyword::Deathtouch));

        move_object_to_zone(
            &mut engine.state,
            engine.registry,
            source,
            Zone::Graveyard,
            None,
        )
        .unwrap();
        assert!(engine.resolving_source_has_keyword(&original_ability, Keyword::Deathtouch));

        move_object_to_zone(
            &mut engine.state,
            engine.registry,
            source,
            Zone::Battlefield,
            Some(0),
        )
        .unwrap();
        assert!(!engine.effective_has_keyword(source, Keyword::Deathtouch));
        assert!(engine.resolving_source_has_keyword(&original_ability, Keyword::Deathtouch));
        assert!(!engine.resolving_source_has_keyword(&ability_item(source, 2), Keyword::Deathtouch));
    }

    #[test]
    fn divided_damage_marks_every_damaged_creature_from_deathtouch_source() {
        let mut engine = GameEngine::new_with_default_decks(7023, &[0, 1], 20).expect("new engine");
        let first = add_three_toughness_creature(&mut engine, 0);
        let second = add_three_toughness_creature(&mut engine, 1);
        let targets = vec![first, second];
        let top = deathtouch_spell_item(2);
        let effect = engine
            .registry
            .get("fireball")
            .unwrap()
            .primary_face()
            .spell_effect[0]
            .clone();
        let mut events = vec![];
        let previous_effect_result = EffectResult::default();
        let mut effect_result = EffectResult::default();
        let mut cx = EffectCx {
            engine: &mut engine,
            events: &mut events,
            targets: &targets,
            targets_by_role: &[],
            target_damage: &[],
            target_group_indices: &[],
            top: &top,
            controller: 0,
            affected_player: 0,
            spell_label: "deathtouch source",
            previous_effect_result: &previous_effect_result,
            effect_result: &mut effect_result,
            effect_index: 0,
        };

        damage::damage_targets(&mut cx, effect).unwrap();

        for target in targets {
            let object = engine.state.objects.get(&target).unwrap();
            assert_eq!(object.damage, 1);
            assert!(object.deathtouch_damage);
        }
    }

    #[test]
    fn mass_damage_marks_every_damaged_creature_from_deathtouch_source() {
        let mut engine = GameEngine::new_with_default_decks(7024, &[0, 1], 20).expect("new engine");
        let first = add_three_toughness_creature(&mut engine, 0);
        let second = add_three_toughness_creature(&mut engine, 1);
        let top = deathtouch_spell_item(0);
        let effect = engine
            .registry
            .get("pyroclasm")
            .unwrap()
            .primary_face()
            .spell_effect[0]
            .clone();
        let mut events = vec![];
        let previous_effect_result = EffectResult::default();
        let mut effect_result = EffectResult::default();
        let mut cx = EffectCx {
            engine: &mut engine,
            events: &mut events,
            targets: &[],
            targets_by_role: &[],
            target_damage: &[],
            target_group_indices: &[],
            top: &top,
            controller: 0,
            affected_player: 0,
            spell_label: "deathtouch source",
            previous_effect_result: &previous_effect_result,
            effect_result: &mut effect_result,
            effect_index: 0,
        };

        mass::damage_all(&mut cx, effect).unwrap();

        for target in [first, second] {
            let object = engine.state.objects.get(&target).unwrap();
            assert_eq!(object.damage, 2);
            assert!(object.deathtouch_damage);
        }
    }
}
