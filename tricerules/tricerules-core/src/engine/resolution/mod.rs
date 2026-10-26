//! Stack resolution orchestration and exhaustive primitive dispatch.
//!
//! Adding a primitive requires one exhaustive `SpellEffectKind` arm in `dispatch` and one implementation
//! in the best-fit domain module. Dispatch arms contain delegation only, so variant coverage stays
//! compiler-checked while resolution logic remains grouped by domain.

use super::events::{color_string, ev_log, object_display_name};
use super::targeting::{
    ability_mode_program, attachment_recipient_for_target, battlefield_objects_matching,
    effect_has_legal_target_at_resolution, face_mode_program, face_program, graveyard_target_legal,
    object_matches_mass_filter, object_matches_scoped_mass_filter,
    stack_target_identity_is_current, target_role_legal_at_resolution, target_schema_with_program,
    TargetSourceIdentity,
};
use super::*;
use tricerules_card_model::primitives::{
    ManaRetention, ResolutionBranchDef, ResolutionBranchRequirement, ResolutionCost, ResultType,
    TargetRole, TargetingDef, TriggeredAbilityDef,
};
use tricerules_card_model::{AbilityPresentation, ChoiceId};

mod chaos_warp;
mod choices;
mod hand_library_return;
pub(super) use choices::{
    authored_resolution_branch_choice_event, authored_resolution_branch_has_selectable_option,
    direct_draw_effect_is_selectable, permanent_choice_prompt,
    reconcile_resolution_choice_delegate_slots, resolution_branch_is_selectable,
    resolution_choice_delegate_at_index, resolution_choice_delegate_candidates,
    resolution_choice_delegate_options, resolution_choice_selector,
    targeted_player_permanent_choice_event,
};
pub(in crate::engine) use choices::{
    card_result_characteristic_sum, card_result_count, card_result_count_for_player,
    card_result_maximum,
};
pub(in crate::engine) use choices::{
    current_targeted_player_control_cohort, current_targeted_player_permanent_candidates,
    targeted_player_control_cohort_candidates,
};

pub(super) fn optional_triggered_ability_branch(
    ability: &TriggeredAbilityDef,
) -> Option<ResolutionBranchDef> {
    if !ability.may
        || ability.program.is_some()
        || ability.modal.is_some()
        || target_schema_with_program(&ability.effect, ability.targeting.as_ref(), None)
            .has_targets()
    {
        return None;
    }

    Some(ResolutionBranchDef {
        branch_id: ChoiceId::new("apply_optional_effect").expect("static resolution branch id"),
        presentation: AbilityPresentation::Fallback,
        runtime_fallback: Some("Apply optional effect".into()),
        cost: ResolutionCost::None,
        requirement: ResolutionBranchRequirement::EffectsApplicable,
        effects: ability.effect.clone(),
    })
}

mod amass;
mod battlefield_returns;
mod blight;
mod boompile;
#[cfg(test)]
mod chosen_tap_tests;
#[cfg(test)]
mod chosen_untap_tests;
mod damage;
mod destruction;
#[cfg(test)]
mod destruction_tests;
mod dispatch;
mod join_forces;
pub(in crate::engine) mod library_cohorts;
/// `pub(super)` so the combat damage step can reach `life::apply_life_gain` — lifelink is the one
/// life-gain edge outside stack resolution, and it must go through the same funnel.
pub(super) mod life;
mod mass;
mod misc;
#[cfg(test)]
mod target_receipt_action_tests;
pub(in crate::engine) use misc::attach_equipment_ids;
#[cfg(test)]
mod portal_sacrifice_tests;
mod program;
mod proliferate;
mod pump_counters;
mod restrictions;
#[cfg(test)]
mod retained_exile_tests;
mod stack_ops;
pub(in crate::engine) use program::equipment_receipt_choice_event;
pub(super) use pump_counters::materialize_resolving_modifier;
pub(super) use stack_ops::{counter_stack_object, counter_stack_object_ref};
mod tokens;
pub(in crate::engine) mod zones;

/// Resolve the current attachable incarnation of the Program's exact `Source` Equipment object.
/// Object IDs are insufficient here because a source that leaves and returns is a new object.
pub(in crate::engine) fn current_equipment_receipt_source(
    engine: &GameEngine,
    object_id: ObjectId,
    expected_generation: u64,
) -> Option<crate::state::TriggerObjectRef> {
    if engine.state.zone_change_generation.get(&object_id).copied() != Some(expected_generation) {
        return None;
    }
    let object = engine.state.objects.get(&object_id)?;
    if object.zone != Zone::Battlefield {
        return None;
    }
    let characteristics = engine.characteristics(object_id)?;
    if !characteristics.has_type("Equipment")
        || characteristics.is_creature()
        || engine
            .state
            .player_idx(characteristics.controller)
            .is_none_or(|index| engine.state.players[index].has_lost)
    {
        return None;
    }
    Some(crate::state::TriggerObjectRef {
        object_id,
        zone_change_generation: expected_generation,
        controller_at_event: characteristics.controller,
    })
}

/// Refilter only the exact receipt members that remain the same battlefield creature
/// incarnations and are currently legal recipients for the retained Equipment incarnation.
pub(in crate::engine) fn current_equipment_receipt_candidates(
    engine: &GameEngine,
    equipment: crate::state::TriggerObjectRef,
    created_tokens: &[crate::state::TriggerObjectRef],
) -> Vec<(ObjectId, u64)> {
    if current_equipment_receipt_source(
        engine,
        equipment.object_id,
        equipment.zone_change_generation,
    )
    .is_none()
    {
        return Vec::new();
    }
    created_tokens
        .iter()
        .filter_map(|candidate| {
            let current_generation = engine
                .state
                .zone_change_generation
                .get(&candidate.object_id)
                .copied()
                .unwrap_or(0);
            let object = engine.state.objects.get(&candidate.object_id)?;
            (current_generation == candidate.zone_change_generation
                && object.zone == Zone::Battlefield
                && super::targeting::equipment_attachment_legal(
                    engine,
                    equipment.object_id,
                    candidate.object_id,
                ))
            .then_some((candidate.object_id, candidate.zone_change_generation))
        })
        .collect()
}

impl GameEngine {
    /// Keep begun ability Programs alive only for the compiled mandatory linked-Exile move
    /// contract approved for this runtime. Legacy linked play/battlefield choices keep their
    /// existing departure policy.
    pub(in crate::engine) fn is_mandatory_program_linked_owner_graveyard_choice(
        &self,
        stack: &ParkedStackResolution,
        key: &crate::state::LinkedExileKey,
        action: crate::state::PendingLinkedExileAction,
    ) -> bool {
        use tricerules_card_model::primitives::{LinkedExiledCardDestination, SpellEffectKind};

        if !matches!(
            action,
            crate::state::PendingLinkedExileAction::Move {
                destination: LinkedExiledCardDestination::OwnerGraveyard,
                optional: false,
            }
        ) {
            return false;
        }
        let Some(run) = stack.program.as_ref() else {
            return false;
        };
        if run.in_flight.is_none()
            || stack.item.source_permanent_id != Some(key.source_object_id)
            || stack.item.source_zone_change != key.source_zone_change
        {
            return false;
        }
        let tricerules_card_model::ProgramOwnerId::Ability(owner) = &run.owner else {
            return false;
        };
        if owner.card_id != key.producer.card_id || owner.face_id != key.producer.face_id {
            return false;
        }
        let Some(producer_ability_id) = key.producer.ability_path.last() else {
            return false;
        };
        matches!(
            self.current_program_perform(run),
            Ok(SpellEffectKind::ChooseLinkedExiledCard {
                producer_ability_id: current_producer,
                linked_exile_id,
                destination: LinkedExiledCardDestination::OwnerGraveyard,
                optional: false,
                ..
            }) if current_producer == *producer_ability_id
                && linked_exile_id == key.ability_link_id
        )
    }
}

/// Rebuild a linked-card or delegate offer from its private continuation after departure or
/// observer restoration. Existing LinkedExileCards and ResolutionBranch payloads are sufficient.
pub(in crate::engine) fn linked_exile_choice_event(engine: &GameEngine) -> Option<rv1::RuledEvent> {
    use crate::state::PendingLinkedExileChoiceStage;

    let pending = engine.state.pending_resolution.as_ref()?;
    let ResolutionContinuation::LinkedExileChoice {
        stack,
        key,
        action,
        stage,
        ..
    } = &pending.continuation
    else {
        return None;
    };
    if !engine.is_mandatory_program_linked_owner_graveyard_choice(stack, key, *action) {
        return None;
    }
    let (
        choice_kind,
        candidate_object_ids,
        candidate_card_ids,
        candidate_names,
        candidate_source_zones,
        resolution_branches,
        min,
        max,
    ) = match stage {
        PendingLinkedExileChoiceStage::ChoosingDelegate { candidates } => {
            let eligible = resolution_choice_delegate_candidates(
                &engine.state,
                stack.item.controller,
                stack.item.controller,
            );
            let options = candidates
                .iter()
                .enumerate()
                .map(|(index, candidate)| {
                    let selectable = candidate.is_some_and(|candidate| {
                        eligible.contains(&candidate)
                            && engine
                                .state
                                .player_idx(candidate)
                                .is_some_and(|seat| !engine.state.players[seat].has_lost)
                    });
                    let label = candidate.map_or_else(
                        || "Unavailable player".to_string(),
                        |candidate| format!("P{candidate} makes the choice"),
                    );
                    rv1::ResolutionBranchOption {
                        branch_index: index as u32,
                        label: label.clone(),
                        selectable,
                        presentation: Some(rv1::PresentationRef {
                            fallback_text: label,
                            ..Default::default()
                        }),
                        ..Default::default()
                    }
                })
                .collect();
            (
                rv1::ChoiceKind::ResolutionBranch,
                Vec::new(),
                Vec::new(),
                Vec::new(),
                Vec::new(),
                options,
                1,
                1,
            )
        }
        PendingLinkedExileChoiceStage::Selecting { .. } => {
            let candidate_object_ids = pending.presentation.candidates.clone();
            let (candidate_card_ids, candidate_names) =
                candidate_identities(engine, &candidate_object_ids);
            let candidate_source_zones =
                vec![rv1::ChoiceCandidateSourceZone::Exile as i32; candidate_object_ids.len()];
            (
                rv1::ChoiceKind::LinkedExileCards,
                candidate_object_ids,
                candidate_card_ids,
                candidate_names,
                candidate_source_zones,
                Vec::new(),
                pending.presentation.min,
                pending.presentation.max,
            )
        }
    };
    Some(rv1::RuledEvent {
        ev: Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(
            rv1::ResolutionChoiceRequired {
                resolution_prompt_id: pending.resolution_prompt_id.get(),
                accepted_permanent_selections: Vec::new(),
                variable_mana_contribution: false,
                candidate_token_identities: Vec::new(),
                candidate_player_ids: Vec::new(),
                deciding_player_id: pending.deciding_player,
                source_object_id: pending.presentation.source_object_id,
                prompt_text: pending.presentation.prompt.clone(),
                choice_kind: choice_kind as i32,
                candidate_object_ids,
                candidate_card_ids,
                min,
                max,
                ordered: false,
                candidate_names,
                candidate_server_card_ids: Vec::new(),
                unique_names: false,
                generic_mana_cost: 0,
                payment_currently_legal: false,
                resolution_branches,
                mana_cost: String::new(),
                candidate_selectable: Vec::new(),
                public_reveal: None,
                candidate_source_zones,
                combat_defender_options: Vec::new(),
                waterbend: false,
                selection_slots: Vec::new(),
                replacement_options: Vec::new(),
                selection_alternatives: Vec::new(),
            },
        )),
    })
}

/// Shared resolution context for one primitive effect.
struct EffectCx<'a> {
    engine: &'a mut GameEngine,
    events: &'a mut Vec<rv1::RuledEvent>,
    targets: &'a [ObjectId],
    targets_by_role: &'a [Vec<ObjectId>],
    target_receipts: &'a [StackTarget],
    target_role_group_indices: &'a [u32],
    top: &'a StackItem,
    controller: PlayerId,
    /// The player an untargeted, player-scoped effect acts on. Equals `controller` for spells,
    /// activated abilities, and every trigger that doesn't name another player; differs only when
    /// a triggered ability says "**that player** …"
    /// ([`StackItem::trigger_context`] — Howling Mine).
    /// Effects that act on the *controller* by rule (a spell program's draw, a self-pump) keep using
    /// `controller`.
    affected_player: PlayerId,
    spell_label: &'a str,
    previous_effect_result: &'a EffectResult,
    /// The result context to restore if this operation suspends. For authored programs this is
    /// the run-level context, which stays separate from the operation's explicit typed input.
    resume_previous_result: Option<&'a EffectResult>,
    effect_result: &'a mut EffectResult,
    effect_index: u32,
    program_state: Option<ProgramRunState>,
}

impl EffectCx<'_> {
    fn parked_stack(&self) -> ParkedStackResolution {
        let mut stack = ParkedStackResolution::new(self.top.clone());
        if let Some(previous_result) = self.resume_previous_result {
            stack.previous_result = previous_result.clone();
        }
        let mut program = self.program_state.clone();
        if let Some(run) = program.as_mut() {
            let token_batch_in_flight = run.in_flight.as_ref().is_some_and(|outputs| {
                self.engine
                    .registry
                    .compiled_program_for_owner(&run.owner)
                    .is_some_and(|compiled| {
                        outputs.iter().any(|slot_id| {
                            compiled.slot(*slot_id).is_some_and(|slot| {
                                matches!(slot.result_type, ResultType::TokenCopyBatch)
                            })
                        })
                    })
            });
            if token_batch_in_flight {
                run.in_flight_produced_object_provenance =
                    self.effect_result.produced_object_provenance;
            }
        }
        stack.program = program;
        stack
    }

    fn previous_battlefield_object(&self) -> Option<ObjectId> {
        let [selected] = self.previous_effect_result.produced_objects.as_slice() else {
            return None;
        };
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

    /// A resolution-start target remains usable only through the role that selected it and the
    /// same object incarnation. The filter itself was already checked at CR 608.2b.
    fn target_is_current_in_role(&self, object_id: ObjectId, role_index: usize) -> bool {
        let Some(role_targets) = self.targets_by_role.get(role_index) else {
            return false;
        };
        let Some(&group_index) = self.target_role_group_indices.get(role_index) else {
            return false;
        };
        target_receipt_is_current_in_role(
            self.engine,
            role_targets,
            self.target_receipts,
            group_index,
            object_id,
            None,
        )
    }

    /// Battlefield actions require an object receipt, so player targets cannot pass by sharing
    /// the ObjectId wire representation.
    fn battlefield_target_is_current_in_role(
        &self,
        object_id: ObjectId,
        role_index: usize,
    ) -> bool {
        self.target_in_zone_is_current_in_role(object_id, role_index, Zone::Battlefield)
    }

    fn target_in_zone_is_current_in_role(
        &self,
        object_id: ObjectId,
        role_index: usize,
        expected_zone: Zone,
    ) -> bool {
        let Some(role_targets) = self.targets_by_role.get(role_index) else {
            return false;
        };
        let Some(&group_index) = self.target_role_group_indices.get(role_index) else {
            return false;
        };
        target_receipt_is_current_in_role(
            self.engine,
            role_targets,
            self.target_receipts,
            group_index,
            object_id,
            Some(expected_zone),
        )
    }

    fn first_current_battlefield_target(&self, role_index: usize) -> Option<ObjectId> {
        self.targets_by_role
            .get(role_index)?
            .iter()
            .copied()
            .find(|&object_id| self.battlefield_target_is_current_in_role(object_id, role_index))
    }

    fn current_battlefield_targets(&self, role_index: usize) -> Vec<ObjectId> {
        self.targets_by_role
            .get(role_index)
            .into_iter()
            .flatten()
            .copied()
            .filter(|&object_id| self.battlefield_target_is_current_in_role(object_id, role_index))
            .collect()
    }

    fn first_current_graveyard_target(&self, role_index: usize) -> Option<ObjectId> {
        self.targets_by_role
            .get(role_index)?
            .iter()
            .copied()
            .find(|&object_id| {
                self.target_in_zone_is_current_in_role(object_id, role_index, Zone::Graveyard)
            })
    }

    fn target_token_copy_snapshot_in_role(
        &self,
        object_id: ObjectId,
        role_index: usize,
    ) -> Option<TokenCopySnapshot> {
        let role_targets = self.targets_by_role.get(role_index)?;
        let group_index = *self.target_role_group_indices.get(role_index)?;
        if !role_targets.contains(&object_id) {
            return None;
        }
        let receipt = self.target_receipts.iter().find(|target| {
            target.object_id == object_id
                && target.group_index == group_index
                && target.zone_change_generation.is_some()
        })?;
        super::copying::token_copy_snapshot_for_incarnation(
            &self.engine.state,
            self.engine.registry,
            object_id,
            receipt.zone_change_generation?,
        )
    }

    fn target_token_copy_snapshots_in_role(
        &self,
        role_index: usize,
    ) -> Vec<(ObjectId, TokenCopySnapshot)> {
        self.targets_by_role
            .get(role_index)
            .into_iter()
            .flatten()
            .copied()
            .filter_map(|object_id| {
                self.target_token_copy_snapshot_in_role(object_id, role_index)
                    .map(|snapshot| (object_id, snapshot))
            })
            .collect()
    }

    fn resolve_battlefield_subject(&self, subject: &EffectSubject) -> Option<ObjectId> {
        match subject {
            EffectSubject::PreviousEffectObject => self.previous_battlefield_object(),
            EffectSubject::Chosen(_) => self.first_current_battlefield_target(0),
            _ => resolve_effect_subject(self.engine, self.top, self.targets, subject),
        }
    }

    fn resolve_battlefield_subjects(&self, subject: &EffectSubject) -> Vec<ObjectId> {
        match subject {
            EffectSubject::Chosen(_) => self.current_battlefield_targets(0),
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
            EffectSubject::TargetAlias(_) => return None,
            EffectSubject::PreviousEffectObjects => return None,
        };
        Some((object_id, source_id))
    }
}

fn target_receipt_is_current_in_role(
    engine: &GameEngine,
    role_targets: &[ObjectId],
    receipts: &[StackTarget],
    group_index: u32,
    object_id: ObjectId,
    expected_zone: Option<Zone>,
) -> bool {
    role_targets.contains(&object_id)
        && receipts
            .iter()
            .find(|target| target.object_id == object_id && target.group_index == group_index)
            .is_some_and(|target| match target.zone_change_generation {
                Some(_) => {
                    stack_target_identity_is_current(engine, target)
                        && engine.state.objects.get(&object_id).is_some_and(|object| {
                            object.zone == expected_zone.unwrap_or(Zone::Battlefield)
                        })
                        && expected_zone.is_none_or(|zone| {
                            zone != Zone::Graveyard
                                || target.kind == rv1::TargetRefKind::Graveyard as i32
                        })
                }
                None if expected_zone.is_none() => engine
                    .state
                    .player_idx(object_id as PlayerId)
                    .is_some_and(|index| !engine.state.players[index].has_lost),
                None => false,
            })
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
            face: if item.prototyped {
                face.prototyped_face().unwrap_or_else(|| face.clone())
            } else {
                face.clone()
            },
            prototyped: item.prototyped,
            room_faces: (definition.layout == Layout::Room).then(|| definition.faces.clone()),
            display_name: face.name.clone(),
        };
        let mut object =
            new_object_from_card(item.id, item.controller, &item.card_id, Zone::Stack, face);
        object.face_up_index = item.face_index;
        object.prototyped = item.prototyped;
        object.token_origin = Some(values);
        // CR 707.10g: a copy of a double-faced permanent spell also has both faces.
        object.token_faces = copying::double_faced_values(definition);
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
            .returned_attacker_assignment
            .and_then(|assignment| self.add_returned_attacker(item.id, assignment))
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
                max_face_mana_value: None,
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
    program: Option<&'a tricerules_card_model::primitives::CompiledProgram>,
) -> Vec<Vec<TargetRole<'a>>> {
    target_schema_with_program(effects, targeting, program)
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
    pub(super) token_id: &'a str,
    pub(super) copy: Option<&'a TokenCopySnapshot>,
    pub(super) count: u32,
    pub(super) recipients: Vec<PlayerId>,
    pub(super) spell_label: &'a str,
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
            // Parameterized evasion is a printed keyword too. Project its MTG label into
            // display identity without changing the engine's typed combat authority.
            .chain(face.evasions.iter().map(|evasion| match evasion {
                tricerules_card_model::Evasion::Landwalk { land_subtype } => {
                    format!("{land_subtype}walk")
                }
            }))
            .collect(),
        ability_texts: face
            .activated_abilities
            .iter()
            // These strings identify artwork candidates, not ability labels. Keep the stable
            // identity fallback here: generated descriptions are not verbatim token Oracle text.
            // Legal actions and synthetic ability cards publish the readable fallback separately.
            .map(|ability| {
                tricerules_card_model::ability_fallback(
                    &values.display_name,
                    "activated ability",
                    std::slice::from_ref(&ability.ability_id),
                )
            })
            .chain(face.triggered_abilities.iter().map(|ability| {
                tricerules_card_model::ability_fallback(
                    &values.display_name,
                    "triggered ability",
                    std::slice::from_ref(&ability.ability_id),
                )
            }))
            .collect(),
        mana_cost: face.mana_cost.to_string(),
        mana_value: face.mana_cost.mana_value(),
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum EffectOutcome {
    Continue,
    GameEnded,
    Blighted(crate::state::BlightReceipt),
    RetainedExile(tricerules_card_model::ExiledCohortId, Vec<TriggerObjectRef>),
    ChaosWarpOwnerInstructions(PlayerId),
    ChaosWarpCommanderChoice,
    Suspended,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum DeferredStackExit {
    Resolved,
    DidNotResolve,
}

/// Resolution completion differs from parking and from CR 104.1 terminal victory.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ResolutionProgress {
    Completed,
    Parked,
    GameEnded,
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
        | PlayerRecipient::TargetedPlayer { .. }
        | PlayerRecipient::ProgramInputPlayer
        | PlayerRecipient::DefendingPlayer
        | PlayerRecipient::EachOtherPlayerThanAffectedPlayer
        | PlayerRecipient::AttackingOpponentsOfDefendingPlayer => Vec::new(),
        PlayerRecipient::TriggeringAttackingPlayer => Vec::new(),
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
        let mut engine = GameEngine::new(
            tricerules_cards::registry::global(),
            121_001,
            &[10, 20],
            20,
            None,
            true,
        )
        .expect("two-player engine");
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

    #[test]
    fn targeted_player_recipient_reads_only_its_authored_group() {
        let targets = [
            StackTarget {
                object_id: 30,
                group_index: 1,
                damage_amount: 0,
                kind: rv1::TargetRefKind::Player as i32,
                zone_change_generation: None,
                required_controller: None,
            },
            StackTarget {
                object_id: 20,
                group_index: 0,
                damage_amount: 0,
                kind: rv1::TargetRefKind::Player as i32,
                zone_change_generation: None,
                required_controller: None,
            },
        ];
        assert_eq!(target_player_from_group(&targets, 0), Some(20));
        assert_eq!(target_player_from_group(&targets, 2), None);
    }
}

fn player_recipients(cx: &EffectCx<'_>, who: PlayerRecipient) -> Vec<PlayerId> {
    match who {
        PlayerRecipient::TargetedPlayer { group_index, .. } => {
            target_player_from_group(cx.target_receipts, group_index)
                .into_iter()
                .collect()
        }
        PlayerRecipient::EachOtherPlayerThanAffectedPlayer => {
            let Some(caster) = cx.top.trigger_context.affected_player else {
                return Vec::new();
            };
            let mut players = cx
                .engine
                .state
                .players
                .iter()
                .filter(|player| !player.has_lost && player.id != caster)
                .map(|player| player.id)
                .collect::<Vec<_>>();
            players.sort_by_key(|player| cx.engine.state.apnap_rank(*player));
            players
        }
        PlayerRecipient::ProgramInputPlayer => {
            match cx.previous_effect_result.program_value.as_ref() {
                Some(crate::state::ProgramValue::TargetedSpellController(Some(player))) => {
                    vec![*player]
                }
                Some(crate::state::ProgramValue::TargetedSpellController(None)) | None => {
                    Vec::new()
                }
                _ => Vec::new(),
            }
        }
        PlayerRecipient::ControllerOfTargetGroup { group_index } => {
            let target = cx
                .target_receipts
                .iter()
                .find(|target| target.group_index == group_index);
            target
                .and_then(|target| {
                    let generation = target.zone_change_generation?;
                    let is_current = cx
                        .engine
                        .state
                        .zone_change_generation
                        .get(&target.object_id)
                        .copied()
                        .unwrap_or(0)
                        == generation
                        && cx
                            .engine
                            .state
                            .objects
                            .get(&target.object_id)
                            .is_some_and(|object| object.zone == Zone::Battlefield);
                    if is_current {
                        cx.engine.controller_of(target.object_id)
                    } else {
                        cx.engine
                            .state
                            .last_known_controller_by_generation
                            .get(&(target.object_id, generation))
                            .copied()
                    }
                })
                .into_iter()
                .collect()
        }
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
            let Some(defending_player) = cx.top.trigger_context.defending_player else {
                return Vec::new();
            };
            let defender_is_live = cx
                .engine
                .state
                .player_idx(defending_player)
                .is_some_and(|idx| !cx.engine.state.players[idx].has_lost);
            if !defender_is_live {
                return Vec::new();
            }
            let assignments = cx
                .engine
                .state
                .combat
                .as_ref()
                .map(|combat| {
                    combat
                        .attacking
                        .iter()
                        .filter_map(|attacker| {
                            combat
                                .attack_assignments
                                .get(attacker)
                                .copied()
                                .map(|assignment| (*attacker, assignment))
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            let mut recipients = Vec::new();
            for (attacker, assignment) in assignments {
                if assignment.attacker.object_id != attacker
                    || assignment.defending_player != defending_player
                    || assignment.defender != CombatDefenderTarget::Player(defending_player)
                    || cx
                        .engine
                        .state
                        .zone_change_generation
                        .get(&attacker)
                        .copied()
                        .unwrap_or(0)
                        != assignment.attacker.zone_change_generation
                    || !cx
                        .engine
                        .state
                        .objects
                        .get(&attacker)
                        .is_some_and(|object| object.zone == Zone::Battlefield)
                {
                    continue;
                }
                let Some(attacking_player) = cx.engine.controller_of(attacker) else {
                    continue;
                };
                let attacking_player_is_live = cx
                    .engine
                    .state
                    .player_idx(attacking_player)
                    .is_some_and(|idx| !cx.engine.state.players[idx].has_lost);
                if attacking_player_is_live
                    && cx
                        .engine
                        .state
                        .are_opponents(attacking_player, defending_player)
                    && !recipients.contains(&attacking_player)
                {
                    recipients.push(attacking_player);
                }
            }
            recipients.sort_by_key(|player| cx.engine.state.apnap_rank(*player));
            recipients
        }
        PlayerRecipient::TriggeringAttackingPlayer => {
            let Some((attacking_player, defending_player)) = cx
                .top
                .trigger_context
                .attacking_player
                .zip(cx.top.trigger_context.defending_player)
            else {
                return Vec::new();
            };
            let attacking_player_is_live = cx
                .engine
                .state
                .player_idx(attacking_player)
                .is_some_and(|idx| !cx.engine.state.players[idx].has_lost);
            (attacking_player_is_live
                && cx
                    .engine
                    .state
                    .are_opponents(attacking_player, defending_player))
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

/// The player id represented by one authored target group. Player targets share the engine's
/// ObjectId wire slot, but their group index is typed separately from permanent targets.
fn target_player_from_group(targets: &[StackTarget], group_index: u32) -> Option<PlayerId> {
    targets.iter().find_map(|target| {
        (target.group_index == group_index).then_some(target.object_id as PlayerId)
    })
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
        EffectSubject::TargetAlias(_) => None,
        EffectSubject::PreviousEffectObject => None,
        EffectSubject::PreviousEffectObjects => None,
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
    target_receipts: &[StackTarget],
    target_role_group_indices: &[u32],
    subject: &EffectSubject,
) -> Option<ObjectId> {
    if let EffectSubject::TriggerObject = subject {
        let observed = top.trigger_context.observed_object?;
        let current_generation = engine
            .state
            .zone_change_generation
            .get(&observed.object_id)
            .copied()
            .unwrap_or(0);
        let object = engine.state.objects.get(&observed.object_id)?;
        let is_observed_battlefield_object = current_generation == observed.zone_change_generation
            && object.zone == Zone::Battlefield;
        let is_died_graveyard_incarnation = observed
            .zone_change_generation
            .checked_add(1)
            .is_some_and(|graveyard_generation| {
                current_generation == graveyard_generation && object.zone == Zone::Graveyard
            });
        let is_discarded_graveyard_incarnation =
            top.triggered_ability.as_ref().is_some_and(|ability| {
                matches!(
                    &ability.trigger,
                    tricerules_card_model::TriggerCondition::WheneverPlayerDiscardsCard { .. }
                )
            }) && current_generation == observed.zone_change_generation
                && object.zone == Zone::Graveyard;
        return (is_observed_battlefield_object
            || is_died_graveyard_incarnation
            || is_discarded_graveyard_incarnation)
            .then_some(observed.object_id);
    }
    if matches!(subject, EffectSubject::Chosen(_)) {
        // Mode effects carry only the selected mode's role receipts here. Match its exact role
        // rather than searching the stack-wide target list, where a sibling mode can reuse the
        // same group index. Chosen zone effects target battlefield permanents; source and trigger
        // objects retain their separate nonbattlefield LKI handling above.
        let object_id = targets.first().copied()?;
        let group_index = *target_role_group_indices.first()?;
        return target_receipt_is_current_in_role(
            engine,
            targets,
            target_receipts,
            group_index,
            object_id,
            Some(Zone::Battlefield),
        )
        .then_some(object_id);
    }
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
    pub(super) effect: SpellEffectKind,
    targets: Vec<ObjectId>,
    target_receipts: Vec<StackTarget>,
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
    fn snapshot_resolution_targets(
        &self,
        top: &mut StackItem,
        program_owners: Option<&[tricerules_card_model::ProgramOwnerId]>,
    ) -> bool {
        let source = TargetSourceIdentity::for_stack_item(self, top);
        let controller = top.controller;
        let chosen_x = top.chosen_x;
        let trigger_context = top.trigger_context;
        let legal = |requirements: &[TargetRole<'_>], target: &StackTarget, x_bound: bool| {
            stack_target_identity_is_current(self, target)
                && target.required_controller.is_none_or(|player| {
                    self.characteristics(target.object_id)
                        .is_some_and(|value| value.controller == player)
                })
                && (!x_bound
                    || self
                        .characteristics(target.object_id)
                        .is_some_and(|value| value.mana_value == chosen_x))
                && requirements.iter().all(|&role| {
                    target_role_legal_at_resolution(
                        self,
                        role,
                        target.object_id,
                        controller,
                        source,
                        trigger_context,
                    )
                })
        };

        let face = self
            .registry
            .get(&top.card_id)
            .and_then(|definition| definition.face(top.face_index));
        let authored_target_groups = program_owners.is_some_and(|owners| {
            owners.iter().any(|owner| {
                self.registry
                    .compiled_program_for_owner(owner)
                    .is_some_and(|program| !program.target_bindings.is_empty())
            })
        });
        if !top.chosen_modes.is_empty() {
            let Some(modal) = top
                .triggered_ability
                .as_ref()
                .and_then(|ability| ability.modal.as_ref())
                .or_else(|| face.and_then(|face| face.modal_spell.as_ref()))
            else {
                return false;
            };
            let ability_program_owner = top.ability_program_owner();
            let is_triggered = top.is_triggered;
            let is_spell = top.ability_text.is_none();
            for chosen in &mut top.chosen_modes {
                let Some(mode) = modal.mode_by_id(&chosen.mode_id) else {
                    continue;
                };
                let program = if is_triggered {
                    ability_program_owner
                        .as_ref()
                        .and_then(|owner| match owner {
                            tricerules_card_model::ProgramOwnerId::Ability(definition) => {
                                ability_mode_program(self, definition, &mode.mode_id)
                            }
                            _ => None,
                        })
                } else if is_spell {
                    face.and_then(|face| {
                        face_mode_program(self, &top.card_id, &face.face_id, &mode.mode_id)
                    })
                } else {
                    None
                };
                let requirements =
                    target_roles_by_group(&mode.effects, mode.targeting.as_ref(), program);
                let x_bound_groups = super::targeting::x_target_group_indices(
                    &mode.effects,
                    mode.targeting.as_ref(),
                    program,
                );
                chosen.targets.retain(|target| {
                    requirements
                        .get(target.group_index as usize)
                        .is_some_and(|group| {
                            legal(
                                group,
                                target,
                                x_bound_groups.contains(&(target.group_index as usize)),
                            )
                        })
                });
            }
            return authored_target_groups
                && top
                    .chosen_modes
                    .iter()
                    .all(|chosen| chosen.targets.is_empty());
        }

        let (effects, targeting, program) = if top.ability_text.is_some() {
            if top.is_triggered {
                let Some(ability) = top.triggered_ability.as_ref().or_else(|| {
                    face.and_then(|face| {
                        face.triggered_abilities.get(top.ability_index.unwrap_or(0))
                    })
                }) else {
                    return false;
                };
                let program = top
                    .ability_program_owner()
                    .and_then(|owner| self.registry.compiled_program_for_owner(&owner));
                (&ability.effect[..], ability.targeting.as_ref(), program)
            } else {
                let Some(ability) = top.activated_ability.as_ref().or_else(|| {
                    face.and_then(|face| {
                        face.activated_abilities.get(top.ability_index.unwrap_or(0))
                    })
                }) else {
                    return false;
                };
                let program = top
                    .ability_program_owner()
                    .and_then(|owner| self.registry.compiled_program_for_owner(&owner));
                (&ability.effect[..], ability.targeting.as_ref(), program)
            }
        } else {
            let Some(face) = face else {
                return false;
            };
            (
                &face.spell_effect[..],
                face.targeting.as_ref(),
                face_program(self, &top.card_id, &face.face_id),
            )
        };
        let requirements = target_roles_by_group(effects, targeting, program);
        let x_bound_groups = super::targeting::x_target_group_indices(effects, targeting, program);
        top.targets.retain(|target| {
            requirements
                .get(target.group_index as usize)
                .is_some_and(|group| {
                    legal(
                        group,
                        target,
                        x_bound_groups.contains(&(target.group_index as usize)),
                    )
                })
        });
        authored_target_groups && top.targets.is_empty()
    }

    pub(super) fn resolve_top_of_stack(
        &mut self,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<ResolutionProgress, EngineError> {
        let mut top = self
            .state
            .stack
            .pop()
            .ok_or(EngineError::Illegal("empty stack"))?;
        // Bind authored program owners from this exact stack occurrence before target
        // revalidation or the stack-wide fizzle check can consume it through legacy resolution.
        let compiled_program_owners = self.compiled_resolution_program_owners(&top)?;
        // Preserve whether the caster actually chose any targets before CR 608.2b removes
        // targets that have become illegal. This distinguishes an optional-target spell cast
        // with zero targets (which resolves normally) from one whose chosen targets all became
        // illegal (which does not resolve).
        let had_chosen_targets =
            !top.targets.is_empty() || top.chosen_modes.iter().any(|mode| !mode.targets.is_empty());
        let authored_program_targets_all_illegal =
            self.snapshot_resolution_targets(&mut top, compiled_program_owners.as_deref());
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
        // Abilities and nonpermanent spell copies also retire only after their instructions.
        // A draw replacement can end the game while either is resolving (CR 104.1).
        if !leaves_no_object {
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
            // An authored instant or sorcery may suspend for a mid-resolution choice. Keep its
            // physical card on the stack until the whole effect list or finite program completes;
            // immediate resolutions still publish the same final batch, while parked resolutions
            // keep the visible stack faithful to the resolution boundary. This includes Adventure.
            let defer_authored_nonpermanent_exit = !resolves_to_battlefield_raw;
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
                        entry_reveal_receipts: Vec::new(),
                        mana_colors_spent_to_cast: top.mana_colors_spent_to_cast,
                        prepared: false,
                        object_id: top.id,
                        deciding_player: top.controller,
                        destination_controller: top.controller,
                        battle_protector: None,
                        face_index: top.face_index,
                        unlock_room_door: Some(top.face_index),
                        chosen_x: top.chosen_x,
                        cast_by: top
                            .cast_by
                            .filter(|_| !top.is_copy && top.cast_occurrence.is_some()),
                        cast_cost_receipts: top.cast_cost_receipts.clone(),
                        player_life_snapshot: self.player_life_snapshot(),
                        tapped: top.cast_method == SpellCastMethod::Sneak,
                        set_types: None,
                        chosen_basic_land_type: None,
                        chosen_opponents: Vec::new(),
                        chosen_creature_types: Vec::new(),
                        entry_counters: BTreeMap::new(),
                        entry_counter_placements: Vec::new(),
                        entry_counter_replacements_resolved: false,
                        entry_modifiers: Vec::new(),
                        entry_cause: None,
                        attached_to: None,
                        pending_copy_candidate: None,
                        pending_aura_recipient: None,
                        accepted_aura_recipient: None,
                        applied_effects: Vec::new(),
                    },
                    BattlefieldEntryCompletion::PermanentSpell { attached_to },
                    events,
                )? {
                    super::replacement::BattlefieldEntryProgress::Parked => {
                        return Ok(ResolutionProgress::Parked);
                    }
                    super::replacement::BattlefieldEntryProgress::Skipped(entry) => {
                        self.restore_skipped_battlefield_entry(&entry)?;
                        let owner = self.state.objects.get(&top.id).map(|object| object.owner);
                        events.push(rv1::RuledEvent {
                            ev: Some(rv1::ruled_event::Ev::StackResolved(rv1::StackResolved {
                                object_id: top.id,
                                destination: rv1::StackResolveDestination::Graveyard as i32,
                                owner_player_id: owner,
                            })),
                        });
                        move_object_to_zone(
                            &mut self.state,
                            self.registry,
                            top.id,
                            Zone::Graveyard,
                            None,
                        )?;
                    }
                    super::replacement::BattlefieldEntryProgress::Ready(entry) => {
                        let entry = *entry;
                        let Some(entry) = self.prepare_single_entry_counter_placements(
                            crate::state::ParkedStackResolution::new(top.clone()),
                            entry,
                            crate::state::BattlefieldEntryCompletion::PermanentSpell {
                                attached_to,
                            },
                            events,
                        )?
                        else {
                            return Ok(ResolutionProgress::Parked);
                        };
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
                        self.commit_battlefield_entry(entry, attached_to, events)?;
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
                move_object_to_zone(&mut self.state, self.registry, top.id, zone, None)?;
            }
            if adventure_resolves_to_exile
                && !defer_authored_nonpermanent_exit
                && !defer_soft_counter_exit
            {
                self.grant_adventure_play_permission(&top)?;
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
                return Ok(ResolutionProgress::Completed);
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
                return Ok(ResolutionProgress::Completed);
            }
        }

        // CR 608.2b: targets are checked once, at the start of resolution — not again on resume.
        let targeted_effects: Vec<_> = resolution_effects
            .iter()
            .filter(|entry| !entry.role_group_indices.is_empty())
            .collect();
        let fizzle = had_chosen_targets
            && (authored_program_targets_all_illegal
                || (!targeted_effects.is_empty()
                    && targeted_effects
                        .iter()
                        .all(|entry| entry.targets.is_empty())));
        if fizzle {
            events.push(ev_log(format!("{spell_label} fizzles (no legal targets).")));
            self.finish_deferred_stack_exit(&top, DeferredStackExit::DidNotResolve, events)?;
            return Ok(ResolutionProgress::Completed);
        }

        // CR 603.5: a targeted triggered ability chooses targets as it is put on the stack, then
        // asks whether to perform its optional effects only after the CR 603.4 and 608.2b checks.
        // Untargeted optional abilities continue to use their authored resolution branch.
        if top.is_triggered
            && top
                .triggered_ability
                .as_ref()
                .is_some_and(|ability| ability.may)
            && had_chosen_targets
            && !targeted_effects.is_empty()
        {
            if top.triggered_ability.as_ref().is_some_and(|ability| {
                direct_draw_effect_is_selectable(
                    self,
                    &top,
                    &crate::state::EffectResult::default(),
                    &ability.effect,
                ) == Some(false)
            }) {
                events.push(ev_log(format!(
                    "{spell_label} has no legal optional draw under the current draw limit."
                )));
                self.finish_deferred_stack_exit(&top, DeferredStackExit::Resolved, events)?;
                return Ok(ResolutionProgress::Completed);
            }
            return Ok(self.park_optional_triggered_ability_choice(top, &spell_label, events));
        }

        // Untargeted, nonmodal legacy "you may" abilities have no target-choice boundary to
        // carry their optional decision. Route them through the same public Apply/Decline
        // continuation, after CR 603.4/608.2b and the existing targeted-may route. Typed Programs
        // keep their own choice semantics and must never receive this legacy gate.
        if top.is_triggered && compiled_program_owners.is_none() {
            let ability = top.triggered_ability.as_ref().or_else(|| {
                self.registry
                    .get(&card_id)
                    .and_then(|definition| definition.face(top.face_index))
                    .and_then(|face| face.triggered_abilities.get(top.ability_index.unwrap_or(0)))
            });
            if let Some(branch) = ability.and_then(optional_triggered_ability_branch) {
                if !resolution_branch_is_selectable(
                    self,
                    &top,
                    &crate::state::EffectResult::default(),
                    top.controller,
                    top.controller,
                    &branch,
                    true,
                ) {
                    events.push(ev_log(format!(
                        "P{} declines the optional resolution choice.",
                        top.controller
                    )));
                    self.finish_deferred_stack_exit(&top, DeferredStackExit::Resolved, events)?;
                    events.push(ev_log(format!("{spell_label} resolves.")));
                    seat_resolved_spell_last_in_graveyard(&mut self.state, top.id);
                    return Ok(ResolutionProgress::Completed);
                }

                return Ok(self.park_optional_triggered_ability_choice(top, &spell_label, events));
            }
        }

        if let Some(owners) = compiled_program_owners {
            let program = self.begin_program_run(&top, owners)?;
            return self.run_program_with_previous(&top, program, EffectResult::default(), events);
        }
        self.run_effect_list(&top, &spell_label, resolution_effects, 0, events)
    }

    fn park_optional_triggered_ability_choice(
        &mut self,
        top: StackItem,
        spell_label: &str,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> ResolutionProgress {
        let prompt = format!("{spell_label}: apply the optional effect?");
        self.state.pending_resolution = Some(PendingResolution {
            resolution_prompt_id: crate::state::ResolutionPromptId::UNASSIGNED,
            resolution_prompt_semantics: None,
            deciding_player: top.controller,
            presentation: PendingResolutionPresentation {
                source_object_id: top.id,
                candidates: Vec::new(),
                min: 0,
                max: 1,
                ordered: false,
                prompt: prompt.clone(),
                choice_kind: rv1::ChoiceKind::ResolutionBranch,
                unique_names: false,
            },
            continuation: ResolutionContinuation::OptionalTriggeredAbility {
                stack: ParkedStackResolution::new(top),
                stage: crate::state::PendingOptionalTriggeredAbilityStage::Selecting,
            },
        });
        events.push(
            self.optional_triggered_ability_choice_event()
                .expect("optional trigger choice was parked"),
        );
        events.push(ev_log(prompt));
        ResolutionProgress::Parked
    }

    /// Complete a stack exit deferred until the data-driven effect list finishes. This keeps an
    /// instant or sorcery visible on the physical stack throughout any resolution-time choice.
    /// Permanent spells retain their specialized entry path and make this a no-op after they
    /// have already moved.
    pub(super) fn finish_deferred_stack_exit(
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
        // deferred exit with a Library destination. Ordinary abilities/copies retire here too.
        let has_deferred_exit = has_stack_item
            || has_stack_object
            || top.ability_text.is_some()
            || (top.is_copy && !self.state.objects.contains_key(&top.id));
        if !has_deferred_exit {
            return Ok(());
        }

        self.state.stack.retain(|item| item.id != top.id);
        let released_state_trigger = self
            .state
            .millennium_calendar_state_trigger_stack_keys
            .remove(&top.id)
            .is_some();
        let is_resolved_omen = exit == DeferredStackExit::Resolved && is_omen_spell;
        let is_resolved_adventure = exit == DeferredStackExit::Resolved
            && top.ability_text.is_none()
            && self.registry.get(&top.card_id).is_some_and(|definition| {
                definition.layout == Layout::Adventure && top.face_index == 1
            });
        // A resolving copy has no GameObject to move, but an explicit self-exile instruction
        // still determines how its StackResolved event reports the copy leaving the stack.
        let is_resolved_self_exiling_copy = exit == DeferredStackExit::Resolved
            && top.is_copy
            && self
                .build_resolution_effects(top)
                .0
                .iter()
                .any(|entry| matches!(entry.effect, SpellEffectKind::ExileResolvingSpell));
        let physical_owner = (!top.is_copy)
            .then(|| self.state.objects.get(&top.id).map(|object| object.owner))
            .flatten();
        let shuffle_player = physical_owner.unwrap_or(top.controller);
        let destination = if is_resolved_omen {
            rv1::StackResolveDestination::Library
        } else if top.cast_method.exiles_on_leave_stack()
            || is_resolved_adventure
            || is_resolved_self_exiling_copy
        {
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
                } else if top.cast_method.exiles_on_leave_stack() || is_resolved_adventure {
                    Zone::Exile
                } else {
                    Zone::Graveyard
                },
                None,
            )?;
            if is_resolved_adventure {
                self.grant_adventure_play_permission(top)?;
            }
        }
        if is_resolved_omen {
            shuffle_player_library_for_current_command(&mut self.state, shuffle_player);
            events.push(ev_log(format!(
                "P{} shuffles the resolving Omen into P{}'s library.",
                top.controller, shuffle_player
            )));
        }
        if released_state_trigger {
            self.check_millennium_calendar_state_triggers();
        }
        Ok(())
    }

    fn grant_adventure_play_permission(&mut self, top: &StackItem) -> Result<(), EngineError> {
        let source_label = self
            .registry
            .get(&top.card_id)
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
            let name = self
                .state
                .stack_presentations
                .get(&top.id)
                .and_then(|presentation| presentation.source_label.clone())
                .or_else(|| def.map(|d| d.name.clone()))
                .unwrap_or_else(|| "Ability".into());
            // Ability indices are relative to the face recorded on the stack item, which is `0`
            // for abilities (see `StackItem::face_index`) — the same face `activate_ability`
            // and the trigger scan read them from.
            let face = def.and_then(|d| d.face(top.face_index));
            let abilities = if top.is_triggered {
                top.triggered_ability
                    .as_ref()
                    .or_else(|| face.and_then(|f| f.triggered_abilities.get(ability_index)))
                    .map(|ability| ability.effect.clone())
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

        let build_entries =
            |effects: &[SpellEffectKind],
             targeting: Option<&TargetingDef>,
             chosen_targets: &[StackTarget],
             program: Option<&tricerules_card_model::primitives::CompiledProgram>| {
                let schema = target_schema_with_program(effects, targeting, program);
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
                            target_receipts: selected.iter().map(|target| **target).collect(),
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
                        let program = if is_ability {
                            top.ability_program_owner().and_then(|owner| match owner {
                                tricerules_card_model::ProgramOwnerId::Ability(definition) => {
                                    ability_mode_program(self, &definition, &mode.mode_id)
                                }
                                _ => None,
                            })
                        } else {
                            self.registry
                                .get(card_id)
                                .and_then(|definition| definition.face(top.face_index))
                                .and_then(|face| {
                                    face_mode_program(
                                        self,
                                        &top.card_id,
                                        &face.face_id,
                                        &mode.mode_id,
                                    )
                                })
                        };
                        resolution_effects.extend(build_entries(
                            &mode.effects,
                            mode.targeting.as_ref(),
                            &chosen.targets,
                            program,
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
            let program = if is_ability {
                top.ability_program_owner()
                    .and_then(|owner| self.registry.compiled_program_for_owner(&owner))
            } else {
                self.registry
                    .get(card_id)
                    .and_then(|definition| definition.face(top.face_index))
                    .and_then(|face| face_program(self, card_id, &face.face_id))
            };
            resolution_effects.extend(build_entries(&effects, targeting, &top.targets, program));
        }

        if !top.resolution_branch_choices.is_empty() {
            let mut expanded = Vec::new();
            for (effect_index, mut entry) in resolution_effects.into_iter().enumerate() {
                if let SpellEffectKind::TapOrUntap { target } = &entry.effect {
                    if let Some(choice) = top.resolution_branch_choices.get(&(effect_index as u32))
                    {
                        entry.effect = match choice {
                            Some(0) => SpellEffectKind::Tap {
                                subject: EffectSubject::Chosen(Box::new(target.clone())),
                            },
                            Some(1) => SpellEffectKind::Untap {
                                subject: EffectSubject::Chosen(Box::new(target.clone())),
                            },
                            _ => continue,
                        };
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
    ) -> Result<ResolutionProgress, EngineError> {
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
    ) -> Result<ResolutionProgress, EngineError> {
        if self.state.is_terminal() {
            return Ok(ResolutionProgress::GameEnded);
        }
        let controller = top.controller;
        for (index, entry) in resolution_effects.into_iter().enumerate().skip(start) {
            let ResolutionEffect {
                effect,
                targets: effect_targets,
                target_receipts,
                role_group_indices,
            } = entry;
            if !role_group_indices.is_empty() && effect_targets.is_empty() {
                previous_effect_result = EffectResult::default();
                continue;
            }
            let targets_by_role = role_group_indices
                .iter()
                .map(|group_index| {
                    target_receipts
                        .iter()
                        .filter(|target| target.group_index == *group_index)
                        .map(|target| target.object_id)
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
                    target_receipts: &target_receipts,
                    target_role_group_indices: &role_group_indices,
                    top,
                    controller,
                    affected_player: top.trigger_context.affected_player.unwrap_or(controller),
                    spell_label,
                    previous_effect_result: &previous_effect_result,
                    resume_previous_result: Some(&previous_effect_result),
                    effect_result: &mut effect_result,
                    effect_index: index as u32,
                    program_state: None,
                };
                dispatch::execute_effect(&mut cx, effect)?
            };
            self.refresh_hideaway_look_permissions();
            if outcome == EffectOutcome::GameEnded {
                self.state
                    .millennium_calendar_state_trigger_stack_keys
                    .remove(&top.id);
                return Ok(ResolutionProgress::GameEnded);
            }
            let mut completed_item = top.clone();
            if let EffectOutcome::Blighted(receipt) = &outcome {
                completed_item.blight_receipts.push(*receipt);
            }
            if let EffectOutcome::RetainedExile(id, objects) = &outcome {
                completed_item
                    .exiled_cohorts
                    .insert(id.clone(), objects.clone());
            }
            self.refresh_enduring_story_designations();
            if let EffectOutcome::ChaosWarpOwnerInstructions(owner) = outcome {
                completed_item
                    .chaos_warp_owner_instructions
                    .insert(index as u32, owner);
                let mut resume = ParkedStackResolution::new(completed_item.clone());
                resume.resume_effect_index = Some(index as u32);
                resume.previous_result = previous_effect_result.clone();
                if self.drain_immediate_observer_actions(Some(resume), events)? {
                    return Ok(ResolutionProgress::Parked);
                }
                let (effects, label) = self.build_resolution_effects(&completed_item);
                return self.run_effect_list_with_previous(
                    &completed_item,
                    &label,
                    effects,
                    index,
                    previous_effect_result,
                    events,
                );
            }
            if outcome == EffectOutcome::ChaosWarpCommanderChoice {
                self.reconcile_combat_characteristics(events);
                if let Some(stack) = self
                    .state
                    .pending_resolution
                    .as_mut()
                    .and_then(|pending| pending.continuation.stack_mut())
                {
                    // The owner has not finished this primitive yet; retain its exact input.
                    stack.resume_effect_index = Some(index as u32);
                    stack.previous_result = previous_effect_result;
                }
                return Ok(ResolutionProgress::Parked);
            }
            let mut observer_stack = ParkedStackResolution::new(completed_item.clone());
            observer_stack.resume_effect_index = Some(index as u32 + 1);
            observer_stack.previous_result = effect_result.clone();
            if self.drain_immediate_observer_actions(Some(observer_stack), events)? {
                return Ok(ResolutionProgress::Parked);
            }
            self.refresh_hideaway_look_permissions();
            self.check_millennium_calendar_state_triggers();
            match outcome {
                EffectOutcome::Blighted(_) | EffectOutcome::RetainedExile(..) => {
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
                    return Ok(ResolutionProgress::Parked);
                }
                EffectOutcome::Continue => {}
                EffectOutcome::ChaosWarpOwnerInstructions(_)
                | EffectOutcome::ChaosWarpCommanderChoice => {
                    unreachable!("Chaos Warp instruction stages handled before observer completion")
                }
                EffectOutcome::GameEnded => unreachable!("terminal resolution returned above"),
            }
            previous_effect_result = effect_result;
        }
        self.finish_deferred_stack_exit(top, DeferredStackExit::Resolved, events)?;
        events.push(ev_log(format!("{spell_label} resolves.")));
        // CR 608.2m: the spell lands in its owner's graveyard *after* its effects, so it sits
        // beneath anything those effects put there (e.g. a self-targeted Tome Scour's five cards).
        seat_resolved_spell_last_in_graveyard(&mut self.state, top.id);
        Ok(ResolutionProgress::Completed)
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
        self.reconcile_combat_characteristics(events);
        // CR 404.3 completes the simultaneous departure's owner ordering before a CR 610.3
        // return may park another choice. Never overwrite its frozen event continuation.
        if self
            .state
            .pending_resolution
            .as_ref()
            .is_some_and(|pending| {
                matches!(
                    pending.continuation,
                    ResolutionContinuation::MassSacrificeGraveyardOrder { .. }
                )
            })
        {
            return Ok(false);
        }
        let pending_player_set_sacrifice =
            self.state
                .pending_resolution
                .as_ref()
                .is_some_and(|pending| {
                    matches!(
                        pending.continuation,
                        ResolutionContinuation::PlayerSetSacrifice { .. }
                    )
                });
        if self.state.pending_observer_return_batch.is_some()
            && self.state.pending_resolution.is_some()
            && !pending_player_set_sacrifice
        {
            return Ok(true);
        }
        if self.state.pending_observer_return_batch.is_none()
            && self.state.pending_immediate_observer_actions.is_empty()
        {
            return Ok(false);
        }
        let prior = self.state.pending_observer_return_batch.take();
        let prior_suspended_resolution = prior
            .as_ref()
            .and_then(|batch| batch.suspended_resolution.clone());
        // A paired observer return can interrupt any already-parked resolution choice. Preserve
        // the complete continuation (including a linked-play cohort's current prompt) so the
        // observer's Aura or timestamp choice can finish before the original choice is restored.
        let active_suspended_resolution = self.state.pending_resolution.take();
        debug_assert!(
            prior_suspended_resolution.is_none() || active_suspended_resolution.is_none(),
            "an observer return batch cannot suspend two resolution prompts"
        );
        let suspended_resolution = prior_suspended_resolution.or(active_suspended_resolution);
        let prior_suspended_replacement_event = prior
            .as_ref()
            .and_then(|batch| batch.suspended_replacement_event.clone());
        let active_suspended_replacement_event = suspended_resolution
            .as_ref()
            .and_then(|_| self.state.pending_replacement_event.take());
        let suspended_replacement_event =
            prior_suspended_replacement_event.or(active_suspended_replacement_event);
        let mut actions = prior
            .as_ref()
            .map_or_else(VecDeque::new, |batch| batch.remaining.clone());
        actions.extend(std::mem::take(
            &mut self.state.pending_immediate_observer_actions,
        ));
        let mut entries = prior
            .as_ref()
            .map_or_else(Vec::new, |batch| batch.ready.clone());
        let resume_stack =
            resume_stack.or_else(|| prior.as_ref().and_then(|batch| batch.resume_stack.clone()));
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
                self.state.pending_observer_return_batch = Some(PendingObserverReturnBatch {
                    ready: entries,
                    remaining: actions,
                    resume_stack: resume_stack.clone(),
                    suspended_resolution: suspended_resolution.clone(),
                    suspended_replacement_event: suspended_replacement_event.clone(),
                });
                let prompt = format!("Choose what {label} will enchant as it returns.");
                events.push(rv1::RuledEvent {
                    ev: Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(
                        rv1::ResolutionChoiceRequired {
                            resolution_prompt_id: 0,
                            accepted_permanent_selections: Vec::new(),
                            variable_mana_contribution: false,
                            candidate_token_identities: Vec::new(),
                            candidate_player_ids: Vec::new(),
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
                            public_reveal: None,
                            candidate_source_zones: Vec::new(),
                            combat_defender_options: Vec::new(),
                            waterbend: false,
                            selection_slots: Vec::new(),
                            replacement_options: Vec::new(),
                            selection_alternatives: Vec::new(),
                        },
                    )),
                });
                events.push(ev_log(prompt.clone()));
                self.state.pending_resolution = Some(PendingResolution {
                    resolution_prompt_id: crate::state::ResolutionPromptId::UNASSIGNED,
                    resolution_prompt_semantics: None,

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
                entry_reveal_receipts: Vec::new(),
                mana_colors_spent_to_cast: Default::default(),
                prepared: false,
                object_id: exiled.object_id,
                deciding_player: owner,
                destination_controller: owner,
                battle_protector: None,
                face_index: 0,
                unlock_room_door: None,
                chosen_x: 0,
                cast_by: None,
                cast_cost_receipts: Vec::new(),
                player_life_snapshot: self.player_life_snapshot(),
                tapped: false,
                set_types: None,
                chosen_basic_land_type: None,
                chosen_opponents: Vec::new(),
                chosen_creature_types: Vec::new(),
                entry_counters: BTreeMap::new(),
                entry_counter_placements: Vec::new(),
                entry_counter_replacements_resolved: false,
                entry_modifiers: Vec::new(),
                entry_cause: None,
                attached_to: None,
                pending_copy_candidate: None,
                pending_aura_recipient: None,
                accepted_aura_recipient: None,
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
            )? {
                super::replacement::BattlefieldEntryProgress::Parked => {
                    if let (Some(resume), Some(pending)) = (
                        resume_stack.as_ref(),
                        self.state.pending_resolution.as_mut(),
                    ) {
                        if let Some(parked) = pending.continuation.stack_mut() {
                            parked.resume_effect_index = resume.resume_effect_index;
                            parked.previous_result = resume.previous_result.clone();
                            parked.program = resume.program.clone();
                        }
                    }
                    self.state.pending_observer_return_batch = Some(PendingObserverReturnBatch {
                        ready: entries,
                        remaining: actions,
                        resume_stack: resume_stack.clone(),
                        suspended_resolution: suspended_resolution.clone(),
                        suspended_replacement_event: suspended_replacement_event.clone(),
                    });
                    return Ok(true);
                }
                super::replacement::BattlefieldEntryProgress::Ready(entry) => {
                    let entry = *entry;
                    entries.push(ObserverReturnEntry {
                        event: entry,
                        owner,
                        label,
                        attached_to: None,
                    });
                }
                super::replacement::BattlefieldEntryProgress::Skipped(entry) => {
                    self.restore_skipped_battlefield_entry(&entry)?;
                }
            }
        }
        let Some(batch) = self.begin_entry_timestamp_order(
            SimultaneousEntryBatch::Observer(Box::new(PendingObserverReturnBatch {
                ready: entries,
                remaining: VecDeque::new(),
                resume_stack: resume_stack.clone(),
                suspended_resolution: suspended_resolution.clone(),
                suspended_replacement_event: suspended_replacement_event.clone(),
            })),
            resume_stack.clone(),
            events,
        )?
        else {
            return Ok(true);
        };
        let Some(batch) =
            self.begin_entry_counter_placements_for_batch(batch, resume_stack.clone(), events)?
        else {
            return Ok(true);
        };
        let SimultaneousEntryBatch::Observer(batch) = batch else {
            return Err(EngineError::Illegal("observer entry batch changed type"));
        };
        self.commit_observer_return_batch(batch.ready, events)?;
        if self.restore_suspended_observer_resolution(
            suspended_resolution,
            suspended_replacement_event,
            events,
        )? {
            return Ok(true);
        }
        Ok(false)
    }

    pub(super) fn restore_suspended_observer_resolution(
        &mut self,
        suspended_resolution: Option<PendingResolution>,
        suspended_replacement_event: Option<crate::engine::replacement::PendingReplacementEvent>,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<bool, EngineError> {
        let Some(mut suspended_resolution) = suspended_resolution else {
            return Ok(false);
        };
        // Returning from a nested observer choice restores the saved operation, but creates a
        // new externally answerable offer. Keep the exact private projection only as a source for
        // re-publication after its current continuation and generation bindings are revalidated.
        // The fresh zero ID forces the normal publisher to allocate a new correlation ID.
        suspended_resolution.resolution_prompt_id = crate::state::ResolutionPromptId::UNASSIGNED;
        self.state.pending_resolution = Some(suspended_resolution);
        self.state.pending_replacement_event = suspended_replacement_event;
        if self.refresh_suspended_ward_payment(events)? {
            return Ok(true);
        }
        if matches!(
            self.state
                .pending_resolution
                .as_ref()
                .map(|pending| &pending.continuation),
            Some(ResolutionContinuation::PlayerSetSacrifice { .. })
        ) {
            self.refresh_player_set_sacrifice_departure(events)?;
        }
        if self.reprocess_suspended_battlefield_entry(events)? {
            return Ok(true);
        }
        // A player may have left while the observer's own choice was parked. Rebuild a restored
        // linked-play offer against the surviving cohort now that its original continuation is
        // visible again.
        self.reconcile_pending_linked_exile_after_departure(events)?;
        if self.reconcile_departed_special_cast_resolution(events)? {
            return Ok(true);
        }
        if self.reconcile_pending_player_set_discard_departure(events, true)? {
            return Ok(true);
        }
        if self.reconcile_departed_target_hand_discard_program(events)? {
            return Ok(true);
        }
        self.refresh_optional_triggered_ability_departure(events)?;
        if self.refresh_suspended_program_draw_replacement(events)?
            || self.refresh_suspended_program_discard_replacement(events)?
            || self.refresh_suspended_program_choice(events)?
            || self.refresh_suspended_nonprogram_choice(events)?
        {
            return Ok(true);
        }
        if let Some(event) = self.special_cast_commander_choice_event() {
            events.push(event);
        } else if let Some(pending) = self.state.pending_resolution.as_ref() {
            if let ResolutionContinuation::SpecialCast { .. } = &pending.continuation {
                if let Some(event) = self.resolution_payment_choice_event() {
                    events.push(event);
                }
            }
        }
        self.republish_cached_resolution_choice(events)?;
        Ok(true)
    }

    /// Resume the existing draw transaction and rebuild only its currently live replacement
    /// prompt. Its receipts and node cursor may already contain committed work, so do not restart
    /// the authored Draw instruction.
    fn refresh_suspended_program_draw_replacement(
        &mut self,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<bool, EngineError> {
        let Some(pending) = self.state.pending_resolution.as_ref() else {
            return Ok(false);
        };
        let ResolutionContinuation::DrawReplacement { stack: Some(stack) } = &pending.continuation
        else {
            return Ok(false);
        };
        if stack.program.is_none()
            || !matches!(
                self.state.pending_replacement_event,
                Some(crate::engine::replacement::PendingReplacementEvent::Draw(_))
            )
        {
            return Ok(false);
        }
        self.reconcile_draw_departure(events)?;
        Ok(true)
    }

    /// Re-enter an authored chooser leaf that was suspended beneath an observer return.
    /// Choice leaves have not committed a transaction, so rebuilding them is safe and lets their
    /// normal domain handler refresh private candidates, generations, names, and the full prompt.
    /// Player-set discards are reconciled by their dedicated transaction path and are never
    /// restarted here, so an accepted live prefix cannot be lost.
    fn refresh_suspended_program_choice(
        &mut self,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<bool, EngineError> {
        let Some(pending) = self.state.pending_resolution.as_ref() else {
            return Ok(false);
        };
        let stack = match &pending.continuation {
            ResolutionContinuation::HandChoice { stack, hand_choice }
                if stack.program.is_some()
                    && (hand_choice.program_selection
                        || self.is_target_hand_discard_program(stack)) =>
            {
                stack.clone()
            }
            ResolutionContinuation::PermanentChoice { stack, .. } if stack.program.is_some() => {
                stack.clone()
            }
            ResolutionContinuation::BeholdChoice { stack, .. } if stack.program.is_some() => {
                stack.clone()
            }
            _ => return Ok(false),
        };
        let Some(mut program) = stack.program.clone() else {
            return Ok(false);
        };
        let target_hand_discard_target = match &pending.continuation {
            ResolutionContinuation::HandChoice { stack, hand_choice }
                if !hand_choice.program_selection && self.is_target_hand_discard_program(stack) =>
            {
                Some(hand_choice.affected_player)
            }
            _ => None,
        };
        if target_hand_discard_target.is_some_and(|target| {
            self.state
                .player_idx(target)
                .is_none_or(|index| self.state.players[index].has_lost)
        }) {
            self.state.pending_resolution = None;
            self.state.pending_replacement_event = None;
            let resumed = self.complete_parked_stack_with_previous(
                stack,
                EffectResult::default(),
                Vec::new(),
            )?;
            events.extend(resumed.events);
            return Ok(true);
        }
        let effect = self.current_program_perform(&program)?;
        let chooser_leaf = match &pending.continuation {
            ResolutionContinuation::HandChoice { hand_choice, .. }
                if hand_choice.program_selection =>
            {
                matches!(effect, SpellEffectKind::SelectHandCards { .. })
            }
            ResolutionContinuation::HandChoice { stack, hand_choice }
                if !hand_choice.program_selection && self.is_target_hand_discard_program(stack) =>
            {
                matches!(effect, SpellEffectKind::ChooseHandCards { .. })
            }
            ResolutionContinuation::PermanentChoice { .. } => matches!(
                effect,
                SpellEffectKind::ChoosePermanents {
                    chooser: tricerules_card_model::primitives::PlayerRecipient::Controller,
                    ..
                }
            ),
            ResolutionContinuation::BeholdChoice { .. } => {
                matches!(effect, SpellEffectKind::MayBehold { .. })
            }
            _ => false,
        };
        if !chooser_leaf {
            return Err(EngineError::ProgramInvariant(
                "restored program choice does not match its in-flight chooser leaf",
            ));
        }
        let Some(frame) = program.frames.last_mut() else {
            return Err(EngineError::ProgramInvariant(
                "restored program choice lost its active frame",
            ));
        };
        frame.next_instruction =
            frame
                .next_instruction
                .checked_sub(1)
                .ok_or(EngineError::ProgramInvariant(
                    "restored program chooser cursor is invalid",
                ))?;
        program.in_flight = None;
        let mut retry = stack;
        retry.program = Some(program);

        self.state.pending_resolution = None;
        self.state.pending_replacement_event = None;
        let resumed =
            self.complete_parked_stack_with_previous(retry, EffectResult::default(), Vec::new())?;
        events.extend(resumed.events);
        Ok(true)
    }

    /// Re-evaluate a restored non-program choice leaf whose candidate cohort depends on current
    /// battlefield state. Generation equality alone cannot make a cached offer safe after an
    /// object left or lost eligibility. Rewind exactly the current effect in the parked standard
    /// list and keep its already-completed prefix and previous-effect result.
    fn refresh_suspended_nonprogram_choice(
        &mut self,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<bool, EngineError> {
        #[derive(Clone, Copy)]
        enum ChooserLeaf {
            ChoosePermanents,
            Proliferate,
            Populate,
            Amass,
            Blight,
            TargetPlayerSacrifices,
        }

        let Some(pending) = self.state.pending_resolution.as_ref() else {
            return Ok(false);
        };
        let (stack, expected_leaf) = match &pending.continuation {
            ResolutionContinuation::PermanentChoice { stack, .. } if stack.program.is_none() => {
                (stack.clone(), ChooserLeaf::ChoosePermanents)
            }
            ResolutionContinuation::Proliferate { stack, .. } if stack.program.is_none() => {
                (stack.clone(), ChooserLeaf::Proliferate)
            }
            ResolutionContinuation::Populate { stack, .. } if stack.program.is_none() => {
                (stack.clone(), ChooserLeaf::Populate)
            }
            ResolutionContinuation::AmassChoice { stack, .. } if stack.program.is_none() => {
                (stack.clone(), ChooserLeaf::Amass)
            }
            ResolutionContinuation::Blight { stack, .. } if stack.program.is_none() => {
                (stack.clone(), ChooserLeaf::Blight)
            }
            ResolutionContinuation::Sacrifice { stack } if stack.program.is_none() => {
                (stack.clone(), ChooserLeaf::TargetPlayerSacrifices)
            }
            _ => return Ok(false),
        };
        if !self.resolution_prompt_should_be_active(pending) {
            return Ok(false);
        }
        if self
            .state
            .player_idx(stack.item.controller)
            .is_none_or(|index| self.state.players[index].has_lost)
        {
            return Err(EngineError::ProgramInvariant(
                "restored choice belongs to a departed resolution controller",
            ));
        }
        let leaf_index = stack
            .resume_effect_index
            .and_then(|next| next.checked_sub(1))
            .ok_or(EngineError::ProgramInvariant(
                "restored choice has no current effect index",
            ))?;
        let (mut effects, spell_label) = self.build_resolution_effects(&stack.item);
        let expected_leaf_matches = |effect: &SpellEffectKind| match expected_leaf {
            ChooserLeaf::ChoosePermanents => {
                matches!(effect, SpellEffectKind::ChoosePermanents { .. })
            }
            ChooserLeaf::Proliferate => matches!(effect, SpellEffectKind::Proliferate),
            ChooserLeaf::Populate => matches!(effect, SpellEffectKind::Populate),
            ChooserLeaf::Amass => matches!(effect, SpellEffectKind::Amass { .. }),
            ChooserLeaf::Blight => matches!(effect, SpellEffectKind::Blight { .. }),
            ChooserLeaf::TargetPlayerSacrifices => {
                matches!(effect, SpellEffectKind::TargetPlayerSacrifices { .. })
            }
        };
        let Some(current_effect) = effects.get(leaf_index as usize) else {
            return Err(EngineError::ProgramInvariant(
                "restored non-program choice does not point to its current chooser leaf",
            ));
        };
        let mut entered_effect = &current_effect.effect;
        while let SpellEffectKind::Conditional { effect, .. }
        | SpellEffectKind::ConditionalCastCost { effect, .. } = entered_effect
        {
            entered_effect = effect;
        }
        if !expected_leaf_matches(entered_effect) {
            return Err(EngineError::ProgramInvariant(
                "restored non-program choice does not point to its current chooser leaf",
            ));
        }
        let entered_effect = entered_effect.clone();
        effects[leaf_index as usize].effect = entered_effect;

        let previous_result = stack.previous_result.clone();
        let mut retry = stack;
        retry.resume_effect_index = Some(leaf_index);
        self.state.pending_resolution = None;
        self.state.pending_replacement_event = None;
        let resume_effect_index = retry.resume_effect_index;
        let resumed = self.complete_parked_resolution_with_effects_and_previous(
            retry.item,
            resume_effect_index,
            effects,
            spell_label,
            previous_result,
            Vec::new(),
        )?;
        events.extend(resumed.events);
        Ok(true)
    }

    /// Re-run a suspended as-enters transaction after observer choices. This rebuilds the live
    /// prompt from its saved event and currently applicable replacement effects, instead of
    /// restoring a pending choice whose candidates or entrant may have become stale.
    fn reprocess_suspended_battlefield_entry(
        &mut self,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<bool, EngineError> {
        let Some(pending) = self.state.pending_resolution.clone() else {
            return Ok(false);
        };
        if !matches!(
            pending.continuation,
            ResolutionContinuation::EntryCopySource { .. }
                | ResolutionContinuation::EntryAuraRecipient { .. }
                | ResolutionContinuation::EntryReplacement { .. }
                | ResolutionContinuation::EntryCost { .. }
                | ResolutionContinuation::EntryReveal { .. }
                | ResolutionContinuation::EntryBasicLandType { .. }
                | ResolutionContinuation::EntryChooseOpponent { .. }
                | ResolutionContinuation::EntryChooseCreatureType { .. }
                | ResolutionContinuation::SagaReadAhead { .. }
                | ResolutionContinuation::BattleProtector { .. }
        ) {
            return Ok(false);
        }
        let Some(replacement_event) = self.state.pending_replacement_event.take() else {
            return Ok(false);
        };
        let crate::engine::replacement::PendingReplacementEvent::BattlefieldEntry(entry) =
            replacement_event
        else {
            self.state.pending_replacement_event = Some(replacement_event);
            return Ok(false);
        };
        let mut entry = *entry;
        let Some(stack) = pending.continuation.stack().cloned() else {
            self.state.pending_replacement_event = Some(
                crate::engine::replacement::PendingReplacementEvent::BattlefieldEntry(Box::new(
                    entry,
                )),
            );
            return Ok(false);
        };
        let linked_current = self
            .state
            .pending_linked_exile_play
            .as_ref()
            .filter(|cohort| {
                cohort
                    .in_flight
                    .is_some_and(|current| current.object_id == entry.event.object_id)
            })
            .and_then(|cohort| {
                cohort
                    .in_flight
                    .map(|current| (cohort.stack.item.controller, current))
            });
        let linked_entry_is_current = linked_current.is_none_or(|(chooser, current)| {
            let object_current = self
                .state
                .objects
                .get(&current.object_id)
                .is_some_and(|object| object.zone == Zone::Exile);
            let generation_current = self
                .state
                .zone_change_generation
                .get(&current.object_id)
                .copied()
                == Some(current.zone_change_generation);
            let chooser_live = self
                .state
                .player_idx(chooser)
                .is_some_and(|index| !self.state.players[index].has_lost);
            object_current
                && generation_current
                && chooser_live
                && entry.event.destination_controller == chooser
        });
        if !self.state.objects.contains_key(&entry.event.object_id) || !linked_entry_is_current {
            self.state.pending_resolution = None;
            self.state.pending_replacement_event = None;
            let matching_linked_entry =
                self.state
                    .pending_linked_exile_play
                    .as_ref()
                    .is_some_and(|cohort| {
                        cohort
                            .in_flight
                            .is_some_and(|current| current.object_id == entry.event.object_id)
                    });
            if matching_linked_entry {
                let cohort = self
                    .state
                    .pending_linked_exile_play
                    .take()
                    .expect("matching linked entry checked above");
                let resumed = self.advance_pending_linked_exile_play(*cohort, Vec::new())?;
                events.extend(resumed.events);
                return Ok(true);
            }
            // The stale event can never commit. Avoid rolling its provisional copy onto a newer
            // incarnation that reused the object ID.
            entry.event.pending_copy_candidate = None;
            entry.event.pending_aura_recipient = None;
            entry.event.accepted_aura_recipient = None;
            let resumed = self.finish_entry_copy_without_recipient(
                stack,
                entry.event,
                entry.completion,
                Vec::new(),
            )?;
            events.extend(resumed.events);
            return Ok(true);
        }

        // An Aura recipient prompt stores a prospective copy receipt on the event. Return that
        // exact receipt to the entry pipeline so it can recompute legal recipients and republish
        // the prompt without granting a new zone incarnation.
        if let Some(recipient) = entry.event.pending_aura_recipient.take() {
            if entry.event.pending_copy_candidate.is_none() {
                entry.event.pending_copy_candidate = recipient.copy_candidate;
            }
        }
        let resumed_prompt = match &pending.continuation {
            ResolutionContinuation::EntryBasicLandType { .. } => {
                self.resumed_basic_land_type_choice_event(&pending, &entry)
            }
            ResolutionContinuation::EntryCost { .. } => {
                self.resumed_entry_cost_choice_event(&pending, &entry)
            }
            _ => None,
        };
        if let Some(prompt) = resumed_prompt {
            self.state.pending_resolution = Some(pending);
            self.state.pending_replacement_event = Some(
                crate::engine::replacement::PendingReplacementEvent::BattlefieldEntry(Box::new(
                    entry,
                )),
            );
            events.push(prompt);
            return Ok(true);
        }
        self.state.pending_resolution = None;
        self.state.pending_replacement_event = None;
        let completion = entry.completion;
        let progress = self.advance_or_park_battlefield_entry(
            stack.item.clone(),
            entry.event,
            completion.clone(),
            events,
        );
        match progress {
            super::replacement::BattlefieldEntryProgress::Parked => {
                self.transfer_entry_choice_resume(&stack);
            }
            super::replacement::BattlefieldEntryProgress::Skipped(event) => {
                let resumed = self.finish_entry_copy_without_recipient(
                    stack,
                    *event,
                    completion,
                    Vec::new(),
                )?;
                events.extend(resumed.events);
            }
            super::replacement::BattlefieldEntryProgress::Ready(event) => {
                let resumed = self.complete_pending_battlefield_entry(
                    pending,
                    *event,
                    completion,
                    Vec::new(),
                )?;
                events.extend(resumed.events);
            }
        }
        Ok(true)
    }

    pub(super) fn commit_observer_return_batch(
        &mut self,
        mut entries: Vec<ObserverReturnEntry>,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<(), EngineError> {
        self.prune_invalid_observer_entry_auras(&mut entries)?;
        if entries.iter().any(|entry| {
            !entry.event.entry_counter_placements.is_empty()
                && !entry.event.entry_counter_replacements_resolved
        }) {
            return Err(EngineError::Illegal(
                "observer-entry counters were not processed by the replacement planner",
            ));
        }
        let mut trigger_events = Vec::new();
        for entry in entries {
            let object_id = entry.event.object_id;
            let chosen_x = entry.event.chosen_x;
            let entry_cause = entry.event.entry_cause.clone();
            // Observer returns are independent one-shot effects. Entry replacement ordering is
            // added in the Aura/choice increment; the base path still uses the canonical commit
            // reset and static-registration machinery.
            let recipient = entry.event.attached_to.or(entry.attached_to);
            let door_event = self.commit_battlefield_entry_state(entry.event, recipient)?;
            trigger_events.push(GameEvent::EntersBattlefield {
                object_id,
                chosen_x,
                entry_cause,
            });
            trigger_events.extend(door_event);
            events.push(permanent_moved_event(
                &self.state,
                object_id,
                entry.owner,
                rv1::permanent_moved::Destination::Battlefield,
            ));
            if let Some(recipient) = recipient {
                events.push(rv1::RuledEvent {
                    ev: Some(rv1::ruled_event::Ev::AuraAttached(rv1::AuraAttached {
                        aura_object_id: object_id,
                        attachment_recipient: Some(attachment_recipient_proto(recipient)),
                    })),
                });
            }
            events.push(ev_log(format!(
                "{} returns to the battlefield under its owner's control.",
                entry.label
            )));
        }
        self.fire_triggers(&trigger_events, events);
        Ok(())
    }

    pub(super) fn refresh_observer_aura_departure(
        &mut self,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<(), EngineError> {
        let Some(pending) = self.state.pending_resolution.as_ref() else {
            return Ok(());
        };
        let ResolutionContinuation::AuraReturn { stack, exiled } = &pending.continuation else {
            return Ok(());
        };
        let decider_live = self
            .state
            .player_idx(pending.deciding_player)
            .is_some_and(|index| !self.state.players[index].has_lost);
        let recipients_live = pending.presentation.candidates.iter().all(|oid| {
            if pending.presentation.choice_kind == rv1::ChoiceKind::AuraPlayer {
                self.state
                    .player_idx(*oid as PlayerId)
                    .is_some_and(|index| !self.state.players[index].has_lost)
            } else {
                self.state
                    .objects
                    .get(oid)
                    .is_some_and(|object| object.zone == Zone::Battlefield)
            }
        });
        if decider_live && recipients_live {
            return Ok(());
        }
        let stack = stack.clone();
        let exiled = *exiled;
        self.state.pending_resolution = None;
        let batch = self
            .state
            .pending_observer_return_batch
            .get_or_insert_with(|| PendingObserverReturnBatch {
                ready: Vec::new(),
                remaining: VecDeque::new(),
                resume_stack: stack.clone(),
                suspended_resolution: None,
                suspended_replacement_event: None,
            });
        if decider_live {
            // The current action was popped before parking. Refresh it through the normal
            // chooser path without losing the ready prefix or later independent returns.
            batch
                .remaining
                .push_front(ImmediateObserverAction::ReturnExiledObject { exiled });
        }
        let mut resumed = Vec::new();
        if !self.drain_immediate_observer_actions(stack.clone(), &mut resumed)? {
            if let Some(stack) = stack {
                let previous_result = stack.previous_result.clone();
                resumed = self
                    .complete_parked_stack_with_previous(stack, previous_result, resumed)?
                    .events;
            }
        }
        events.extend(resumed);
        Ok(())
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
            mana_colors_spent_to_cast: Default::default(),
            id: object_id,
            controller,
            card_id,
            prototyped: false,
            targets: Vec::new(),
            ability_text: Some("paired one-shot return".into()),
            source_permanent_id: None,
            source_owner: None,
            source_zone_change: 0,
            trigger_use_key: None,
            source_face_change: 0,
            ability_index: None,
            activated_ability: None,
            activated_ability_occurrence: None,
            triggered_ability: None,
            is_triggered: false,
            is_copy: true,
            face_index: 0,
            cast_method: SpellCastMethod::Normal,
            returned_attacker_assignment: None,
            chosen_x: 0,
            chosen_modes: Vec::new(),
            cast_condition_results: Vec::new(),
            cast_occurrence: None,
            cast_by: None,
            cast_cost_receipts: Vec::new(),
            payment_result: CardResultCohort::default(),
            exiled_cohorts: Default::default(),
            chaos_warp_owner_instructions: Default::default(),
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
    /// Returns `None` when a CR 616 entry replacement parked the batch for a player choice, or the
    /// committed token references when the tokens entered. Publishing those refs lets a later
    /// instruction in the same resolution name "the token it created" without targeting (CR 608.2),
    /// exactly as `ManifestDread` already publishes its manifested permanent.
    fn create_tokens(
        &mut self,
        request: TokenCreationRequest<'_>,
        stack: ParkedStackResolution,
        enters_tapped: bool,
        delayed_sacrifice: Option<DelayedTokenSacrificeTiming>,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<Option<Vec<TriggerObjectRef>>, EngineError> {
        let (entries, logs) = self.prepare_token_entries(request, enters_tapped)?;
        let created_ids = entries
            .iter()
            .map(|entry| entry.event.object_id)
            .collect::<Vec<_>>();
        // CR 603.6: one token-making instruction puts all of its tokens onto the battlefield
        // simultaneously, so every entrant exists before their ETB triggers are collected.
        if self.begin_token_entry_batch_with_stack(
            stack,
            entries,
            logs,
            TokenEntryBatchOptions {
                delayed_sacrifice,
                ..Default::default()
            },
            events,
        )? {
            return Ok(None);
        }
        Ok(Some(self.token_entry_object_refs(&created_ids)))
    }

    /// The committed public references for a token batch, in mint order.
    pub(super) fn token_entry_object_refs(&self, ids: &[ObjectId]) -> Vec<TriggerObjectRef> {
        ids.iter()
            .filter_map(|&object_id| {
                self.state
                    .objects
                    .get(&object_id)
                    .map(|object| TriggerObjectRef {
                        object_id,
                        zone_change_generation: self
                            .state
                            .zone_change_generation
                            .get(&object_id)
                            .copied()
                            .unwrap_or(0),
                        controller_at_event: object.controller,
                    })
            })
            .collect()
    }

    pub(super) fn prepare_token_entries(
        &mut self,
        request: TokenCreationRequest<'_>,
        enters_tapped: bool,
    ) -> Result<(Vec<TokenBattlefieldEntry>, Vec<String>), EngineError> {
        let TokenCreationRequest {
            token_id,
            copy,
            count,
            recipients,
            spell_label,
        } = request;
        let registry = self.registry;
        let values = if let Some(copy) = copy {
            copy.values.clone()
        } else {
            let def = registry
                .get(token_id)
                .ok_or_else(|| EngineError::MissingCard(token_id.to_string()))?;
            CopiableValues {
                source_card_id: token_id.to_string(),
                source_face_index: 0,
                face: def.primary_face().clone(),
                prototyped: false,
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
            if self
                .state
                .player_idx(pid)
                .is_none_or(|index| self.state.players[index].has_lost)
            {
                continue;
            }
            let replaced_count = self.token_creation_replaced_count(pid, count);
            for _ in 0..replaced_count {
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
                        prototyped: values.prototyped,
                        token_origin: Some(values.clone()),
                        token_faces: copy.and_then(|snapshot| snapshot.faces.clone()),
                        copiable_values: None,
                        copy_revision: 0,
                        active_copy_occurrence: None,
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
                        face_up_index: copy.map_or(0, |snapshot| snapshot.face_up_index),
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
                        entry_reveal_receipts: Vec::new(),
                        mana_colors_spent_to_cast: Default::default(),
                        prepared: false,
                        object_id: oid,
                        deciding_player: pid,
                        destination_controller: pid,
                        battle_protector: None,
                        face_index: copy.map_or(0, |snapshot| snapshot.face_up_index),
                        unlock_room_door: None,
                        chosen_x: 0,
                        cast_by: None,
                        cast_cost_receipts: Vec::new(),
                        player_life_snapshot: player_life_snapshot.clone(),
                        tapped: enters_tapped,
                        set_types: None,
                        chosen_basic_land_type: None,
                        chosen_opponents: Vec::new(),
                        chosen_creature_types: Vec::new(),
                        entry_counters: BTreeMap::new(),
                        entry_counter_placements: Vec::new(),
                        entry_counter_replacements_resolved: false,
                        entry_modifiers: Vec::new(),
                        entry_cause: None,
                        attached_to: None,
                        pending_copy_candidate: None,
                        pending_aura_recipient: None,
                        accepted_aura_recipient: None,
                        applied_effects: Vec::new(),
                    },
                    created,
                });
            }
            let noun = if replaced_count == 1 {
                "token"
            } else {
                "tokens"
            };
            logs.push(format!(
                "P{pid} creates {replaced_count} {name} {noun} ({spell_label})."
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

/// Ordinary draws are zone changes, including a card discarded onto the library earlier
/// during this same resolution. Use the shared funnel so all references and resets agree.
pub(super) fn draw_card(
    state: &mut GameState,
    registry: &'static CardRegistry,
    player: PlayerId,
) -> Result<(), EngineError> {
    let idx = state
        .player_idx(player)
        .ok_or(EngineError::UnknownPlayer(player))?;
    let oid = *state.players[idx]
        .library
        .front()
        .ok_or(EngineError::Illegal("library empty"))?;
    move_object_to_zone(state, registry, oid, Zone::Hand, None)
}

/// Deal the initial pre-game hand before ordinary game-object references exist.
pub(super) fn deal_opening_card(
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
    let face_down_exile = state
        .objects
        .get(&oid)
        .is_some_and(|object| object.zone == Zone::Exile && object.face_down);
    let card_id = if face_down_exile {
        String::new()
    } else {
        state
            .objects
            .get(&oid)
            .map(|o| o.card_id.clone())
            .unwrap_or_default()
    };
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
            face_down: state.objects.get(&oid).is_some_and(|object| {
                object.face_down && matches!(object.zone, Zone::Battlefield | Zone::Exile)
            }),
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

fn scoped_player_target(
    engine: &GameEngine,
    targets: &[StackTarget],
    group_index: u32,
) -> Option<PlayerId> {
    targets
        .iter()
        .find_map(|target| {
            (target.group_index == group_index).then_some(target.object_id as PlayerId)
        })
        .filter(|&player| {
            engine
                .state
                .player_idx(player)
                .is_some_and(|idx| !engine.state.players[idx].has_lost)
        })
}

pub(super) fn snapshot_mass_creature_scope(
    engine: &GameEngine,
    filter: &CreatureScopeFilter,
    controller: PlayerId,
    source: ObjectId,
    targets: &[StackTarget],
) -> Vec<ObjectId> {
    let Some(CreatureScopeController::TargetedPlayer { group_index, .. }) = filter.controller
    else {
        return snapshot_creature_scope(engine, filter, controller, source);
    };
    let Some(player) = scoped_player_target(engine, targets, group_index) else {
        return Vec::new();
    };
    let mut ordinary = filter.clone();
    ordinary.controller = None;
    snapshot_creature_scope(engine, &ordinary, controller, source)
        .into_iter()
        .filter(|&oid| engine.controller_of(oid) == Some(player))
        .collect()
}

/// The semantic discard seam for one self-contained discard. Direct exile and ordinary zone moves
/// never call it. Multi-card instructions commit each card through [`GameEngine::commit_discard`]
/// and publish one batch through [`GameEngine::fire_discard_batches`] instead.
pub(crate) fn perform_discard(
    engine: &mut GameEngine,
    affected_player: PlayerId,
    object_id: ObjectId,
    cause: crate::state::DiscardCause,
    out: &mut Vec<rv1::RuledEvent>,
) -> Result<(String, rv1::RuledEvent), EngineError> {
    let (name, moved, receipt) = engine.commit_discard(affected_player, object_id, cause, false)?;
    engine.fire_discard_batches(vec![(affected_player, vec![receipt])], out);
    Ok((name, moved))
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
/// Record source qualities without inventing a zone change (also used by CR 800.4a removal).
pub(super) fn record_last_known_characteristics(
    state: &mut GameState,
    oid: ObjectId,
    generation: u64,
    characteristics: super::characteristics::Characteristics,
) {
    state
        .last_known_mana_value_by_generation
        .insert((oid, generation), characteristics.mana_value);
    state.last_known_pt_by_generation.insert(
        (oid, generation),
        (
            characteristics.signed_power,
            characteristics.signed_toughness,
        ),
    );
    state
        .last_known_controller_by_generation
        .insert((oid, generation), characteristics.controller);
    state
        .last_known_keywords_by_generation
        .insert((oid, generation), characteristics.keywords);
    state
        .last_known_colors_by_generation
        .insert((oid, generation), characteristics.colors);
    state
        .last_known_types_by_generation
        .insert((oid, generation), characteristics.types);
    state
        .last_known_names_by_generation
        .insert((oid, generation), characteristics.names);
}

pub(super) struct PreparedZoneMove {
    oid: ObjectId,
    owner: PlayerId,
    old_zone: Option<Zone>,
    prior_generation: u64,
    z: Zone,
    leaving_battlefield: bool,
    last_known_characteristics: Option<super::characteristics::Characteristics>,
    copy: Option<crate::state::TokenCopySnapshot>,
    last_known_attached_object: Option<crate::state::AttachedObjectLki>,
    front_face_values: Option<(Option<u32>, Option<u32>, bool, bool)>,
    counters: std::collections::BTreeMap<CounterKind, u32>,
    tapped: bool,
    old_controller: PlayerId,
    holder: PlayerId,
    holder_index: usize,
}

impl PreparedZoneMove {
    pub(super) fn object_id(&self) -> ObjectId {
        self.oid
    }
}

/// Prepare every departure in a simultaneous instruction against the same battlefield.
pub(super) fn prepare_zone_move(
    state: &GameState,
    registry: &'static CardRegistry,
    oid: ObjectId,
    mut z: Zone,
    controller: Option<PlayerId>,
) -> Result<Option<PreparedZoneMove>, EngineError> {
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
    let has_finality = state
        .objects
        .get(&oid)
        .is_some_and(|object| object.counter_count(CounterKind::Finality) > 0);
    if old_zone == Some(Zone::Battlefield)
        && z == Zone::Graveyard
        && (has_finality
            || state.death_replacement_effects.iter().any(|effect| {
                effect.object_id == oid && effect.zone_change_generation == prior_generation
            }))
    {
        z = Zone::Exile;
    }
    let last_known_characteristics = leaving_battlefield
        .then(|| super::characteristics::characteristics_from(state, registry, oid))
        .flatten();
    let copy = leaving_battlefield
        .then(|| copying::token_copy_snapshot_from(state, registry, oid))
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
                Some(crate::state::AttachedObjectLki {
                    object_id: target_oid,
                    zone_change_generation: state
                        .zone_change_generation
                        .get(&target_oid)
                        .copied()
                        .unwrap_or(0),
                    owner: state.objects.get(&target_oid)?.owner,
                })
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
    let object = &state.objects[&oid];
    let counters = object.counters.clone();
    let tapped = object.tapped;
    let old_controller = object.controller;
    let holder = if z == Zone::Battlefield {
        controller.unwrap_or(owner)
    } else {
        owner
    };
    let holder_index = state
        .player_idx(holder)
        .ok_or(EngineError::Illegal("no such player"))?;
    Ok(Some(PreparedZoneMove {
        oid,
        owner,
        old_zone,
        prior_generation,
        z,
        leaving_battlefield,
        last_known_characteristics,
        copy,
        last_known_attached_object,
        front_face_values,
        counters,
        tapped,
        old_controller,
        holder,
        holder_index,
    }))
}

fn move_object_to_zone_with_entry_receipt(
    state: &mut GameState,
    registry: &'static CardRegistry,
    oid: ObjectId,
    z: Zone,
    controller: Option<PlayerId>,
) -> Result<Option<crate::state::PermanentHistoryFact>, EngineError> {
    let Some(prepared) = prepare_zone_move(state, registry, oid, z, controller)? else {
        return Ok(None);
    };
    commit_zone_move(state, registry, prepared)
}

pub(super) fn commit_zone_move(
    state: &mut GameState,
    registry: &'static CardRegistry,
    prepared: PreparedZoneMove,
) -> Result<Option<crate::state::PermanentHistoryFact>, EngineError> {
    let PreparedZoneMove {
        oid,
        owner,
        old_zone,
        prior_generation,
        z,
        leaving_battlefield,
        last_known_characteristics,
        copy,
        last_known_attached_object,
        front_face_values,
        counters,
        tapped,
        old_controller,
        holder,
        holder_index,
    } = prepared;
    debug_assert_eq!(state.objects[&oid].owner, owner);
    debug_assert_eq!(state.objects[&oid].zone, old_zone.unwrap());
    debug_assert_eq!(
        state.zone_change_generation.get(&oid).copied().unwrap_or(0),
        prior_generation
    );
    if leaving_battlefield {
        state.active_temporary_copies.retain(|effect| {
            effect.object_id != oid || effect.zone_change_generation != prior_generation
        });
        // A resolving "for as long as you control this source" effect belongs to this exact
        // source incarnation. Drain it before the zone-change generation is incremented; the
        // ability's stack object is a different id and cannot serve as this binding.
        state.continuous_effects.retain(|effect| {
            !matches!(
                &effect.duration,
                EffectDuration::WhileSourceControlledBy {
                    source_object_id,
                    source_zone_change_generation,
                    ..
                } if *source_object_id == oid
                    && *source_zone_change_generation == prior_generation
            )
        });
        state.death_replacement_effects.retain(|effect| {
            effect.object_id != oid || effect.zone_change_generation != prior_generation
        });
        if let Some(snapshot) = copy {
            state
                .last_known_copy_by_generation
                .insert((oid, prior_generation), snapshot);
        }
    }
    if leaving_battlefield {
        state.battlefield_entry_timestamps.remove(&oid);
        state.room_states.remove(&oid);
        state.class_levels.remove(&oid);
        state.case_solved_generations.remove(&oid);
        super::preparation::unprepare_permanent(state, oid);
        state.battle_protectors.remove(&oid);
        state
            .chosen_opponents
            .retain(|record| record.key.source_object_id != oid);
        state
            .chosen_creature_types
            .retain(|record| record.key.source_object_id != oid);
        {
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
        state.activated_ability_slots.remove(&oid);
        state.spell_effects_carry_to_permanent.remove(&oid);
        state.spell_entry_facts.remove(&oid);
        state
            .discard_reference_successors
            .retain(|(object_id, _), _| *object_id != oid);
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
    state.continuous_effects.retain(|e| {
        let single_on_this =
            old_zone != Some(z) && matches!(&e.affected, AffectedScope::Single(id) if *id == oid);
        let static_from_this = leaving_battlefield
            && e.source_id == Some(oid)
            && e.duration == EffectDuration::WhileSourceOnBattlefield;
        let graveyard_static_from_this = old_zone == Some(Zone::Graveyard)
            && e.source_id == Some(oid)
            && e.duration == EffectDuration::WhileSourceInGraveyard;
        !single_on_this && !static_from_this && !graveyard_static_from_this
    });
    if leaving_battlefield {
        if let Some(attached_object) = last_known_attached_object {
            state
                .last_known_attached_object_by_generation
                .insert((oid, prior_generation), attached_object);
        }
        state
            .skip_next_untap
            .retain(|&(object_id, _)| object_id != oid);
        state.damage_prevention_effects.retain(|effect| {
            let recipient_is_this_object = match effect.scope {
                DamagePreventionScope::Recipient(recipient) => recipient == oid,
                DamagePreventionScope::CombatRecipient { object_id, .. }
                | DamagePreventionScope::CombatSource { object_id, .. } => object_id == oid,
                DamagePreventionScope::Combat
                | DamagePreventionScope::OtherCreaturesYouControl { .. } => false,
            };
            let static_from_this = effect.source_id == Some(oid)
                && effect.duration == EffectDuration::WhileSourceOnBattlefield;
            !recipient_is_this_object && !static_from_this
        });
        state.damage_doubling_effects.retain(|effect| {
            !(effect.source_id == Some(oid)
                && effect.duration == EffectDuration::WhileSourceOnBattlefield)
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
            let was_tapped = tapped;
            o.tapped = false;
            state
                .last_known_counters_by_generation
                .insert((oid, prior_generation), counters);
            o.counters.clear();
            o.counter_timestamps.clear();
            o.attached_to = None;
            o.regeneration_shields = 0;
            o.face_up_index = 0;
            o.face_down = false;
            o.copiable_values = None;
            o.copy_revision = 0;
            o.active_copy_occurrence = None;
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
            record_last_known_characteristics(state, oid, prior_generation, characteristics);
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
        p.command_zone.retain(|&x| x != oid);
    }
    // CR 400.3: the battlefield is entered under a *controller*; every other zone belongs to the
    // card's owner, so that is where a permanent goes when it leaves.
    let p = &mut state.players[holder_index];
    match z {
        Zone::Graveyard => p.graveyard.push(oid),
        Zone::Hand => p.hand.push(oid),
        Zone::Battlefield => p.battlefield.push(oid),
        Zone::Library => p.library.push_back(oid),
        Zone::Exile => p.exile.push(oid),
        Zone::Stack => {}
        Zone::Command => p.command_zone.push(oid),
    }
    if z == Zone::Battlefield {
        state.next_game_rule_timestamp = state.next_game_rule_timestamp.saturating_add(1);
        state
            .battlefield_entry_timestamps
            .insert(oid, state.next_game_rule_timestamp);
    }
    if let Some(o) = state.objects.get_mut(&oid) {
        o.zone = z;
        if z != Zone::Battlefield {
            o.prototyped = false;
        }
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
    if z == Zone::Graveyard {
        super::continuous::emit_graveyard_static_abilities(state, registry, oid);
    }
    Ok((old_zone != Some(Zone::Graveyard))
        .then(|| super::history::graveyard_entry_fact(state, registry, oid))
        .flatten())
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

/// One sacrifice instruction freezes all replacement destinations and departure LKI before
/// the first move. The caller owns its single zone-event boundary and per-object receipts.
pub(super) fn sacrifice_permanents(
    state: &mut GameState,
    registry: &'static CardRegistry,
    objects: &[ObjectId],
) -> Result<Vec<bool>, EngineError> {
    let mut distinct = HashSet::new();
    let mut prepared = Vec::with_capacity(objects.len());
    for &oid in objects {
        if !distinct.insert(oid)
            || !state
                .objects
                .get(&oid)
                .is_some_and(|object| object.zone == Zone::Battlefield)
        {
            return Err(EngineError::Illegal(
                "sacrifice group requires distinct battlefield permanents",
            ));
        }
        prepared.push(
            prepare_zone_move(state, registry, oid, Zone::Graveyard, None)?
                .ok_or(EngineError::Illegal("sacrifice permanent missing"))?,
        );
    }
    let mut deaths = Vec::with_capacity(prepared.len());
    for movement in prepared {
        let died = movement.z == Zone::Graveyard;
        if let Some(fact) = commit_zone_move(state, registry, movement)? {
            state
                .turn_history
                .current
                .permanent_cards_entered_graveyard
                .push(fact);
        }
        deaths.push(died);
    }
    Ok(deaths)
}

/// CR 608.2n: the spell enters its owner's graveyard after its effects have been applied. A
/// self-targeted Tome Scour must therefore end up beneath the five cards it milled, not on top
/// of them.
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

#[cfg(test)]
mod exile_permission_generation_tests {
    use super::*;

    #[test]
    fn exile_to_exile_creates_a_new_generation_and_invalidates_permission() {
        let mut engine = GameEngine::new_with_default_decks(
            tricerules_cards::registry::global(),
            123_007,
            &[0, 1],
            20,
        )
        .expect("engine");
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
                max_face_mana_value: None,
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
        let engine = GameEngine::new(
            tricerules_cards::registry::global(),
            110_001,
            &[0, 1],
            20,
            decks,
            true,
        )
        .expect("new game");
        let object_id = engine.state.players[0].library[0];
        let front_face_power = ZoneCardFilter {
            printed_power: Some(PowerComparison::AtMost(1)),
            ..Default::default()
        };
        let back_face_power = ZoneCardFilter {
            printed_power: Some(PowerComparison::AtLeast(3)),
            ..Default::default()
        };

        assert!(zone_card_matches_filter(
            &engine.state,
            engine.registry,
            object_id,
            Some(&front_face_power),
        ));
        assert!(!zone_card_matches_filter(
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
    engine.remove_combat_participants(&[oid], events);
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
                prototyped: false,
                id,
                owner: controller,
                base_controller: controller,
                controller,
                card_id: "grizzly_bears".to_string(),
                token_origin: None,
                token_faces: None,
                copiable_values: None,
                copy_revision: 0,
                active_copy_occurrence: None,
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
        let mut engine = GameEngine::new(
            tricerules_cards::registry::global(),
            4610,
            &[0, 1],
            20,
            None,
            true,
        )
        .unwrap();
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
        let mut engine = GameEngine::new(
            tricerules_cards::registry::global(),
            75_004,
            &[10, 20],
            20,
            None,
            true,
        )
        .expect("two-player engine");
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

    fn publish_current_special_cast_prompt_id(engine: &mut GameEngine) -> u64 {
        let mut events = vec![engine
            .resolution_payment_choice_event()
            .expect("special-cast offer is publishable")];
        engine
            .publish_resolution_prompt_ids(&mut events)
            .expect("special-cast prompt ID is allocated");
        match events[0].ev.as_ref() {
            Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(choice)) => {
                choice.resolution_prompt_id
            }
            _ => unreachable!("special-cast offer emits a resolution choice"),
        }
    }

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
                prototyped: false,
                id,
                owner: controller,
                base_controller: controller,
                controller,
                card_id: card_id.to_string(),
                token_origin: None,
                token_faces: None,
                copiable_values: None,
                copy_revision: 0,
                active_copy_occurrence: None,
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
            prototyped: false,
            mana_colors_spent_to_cast: Default::default(),
            id: source + 10_000,
            controller: 0,
            card_id: "capture_sphere".to_string(),
            targets: vec![],
            ability_text: Some("When this Aura enters, tap enchanted creature.".to_string()),
            source_permanent_id: Some(source),
            source_owner: Some(0),
            source_zone_change: generation,
            trigger_use_key: None,
            source_face_change: 0,
            ability_index: Some(0),
            activated_ability: None,
            activated_ability_occurrence: None,
            triggered_ability: None,
            is_triggered: true,
            is_copy: false,
            face_index: 0,
            cast_method: SpellCastMethod::Normal,
            returned_attacker_assignment: None,
            chosen_x: 0,
            chosen_modes: vec![],
            cast_condition_results: Vec::new(),
            cast_occurrence: None,
            cast_by: None,
            cast_cost_receipts: vec![],
            payment_result: CardResultCohort::default(),
            exiled_cohorts: Default::default(),
            chaos_warp_owner_instructions: Default::default(),
            resolution_branch_choices: Default::default(),
            blight_receipts: Vec::new(),
            trigger_context: TriggerContext::default(),
        }
    }

    #[test]
    fn chosen_zone_effect_subject_requires_the_captured_incarnation() {
        let mut engine = observer_restore_test_engine(494_049);
        let target = add_battlefield_object(&mut engine, 1, "hill_giant");
        let captured_generation = 7;
        engine
            .state
            .zone_change_generation
            .insert(target, captured_generation);
        let mut item = triggered_item(target, 0);
        item.targets = vec![crate::state::StackTarget {
            object_id: target,
            required_controller: None,
            group_index: 0,
            damage_amount: 0,
            kind: rv1::TargetRefKind::Permanent as i32,
            zone_change_generation: Some(captured_generation),
        }];
        let subject = tricerules_card_model::primitives::EffectSubject::Chosen(Box::new(
            tricerules_card_model::primitives::TargetFilter::default_creature(),
        ));

        assert_eq!(
            resolve_zone_effect_subject(&engine, &item, &[target], &item.targets, &[0], &subject,),
            Some(target),
            "the captured target incarnation remains eligible for its zone effect"
        );

        engine
            .state
            .zone_change_generation
            .insert(target, captured_generation + 1);
        assert!(engine.state.objects.contains_key(&target));
        assert_eq!(
            resolve_zone_effect_subject(&engine, &item, &[target], &item.targets, &[0], &subject,),
            None,
            "the same ObjectId in a later zone-change generation is a new object"
        );
    }

    fn observer_restore_test_engine(seed: u64) -> GameEngine {
        let mut engine = GameEngine::new(
            tricerules_cards::registry::global(),
            seed,
            &[0, 1, 2],
            20,
            None,
            true,
        )
        .expect("three-player observer-restore engine");
        engine.state.opening = None;
        engine.state.turn_step = TurnStep::Main1;
        engine.state.active_player_idx = 0;
        engine.state.priority_idx = 0;
        clear_observer_restore_starting_zones(&mut engine);
        engine
    }

    fn clear_observer_restore_starting_zones(engine: &mut GameEngine) {
        let mut starting_cards = Vec::new();
        for player in &mut engine.state.players {
            starting_cards.append(&mut player.hand);
            starting_cards.extend(player.library.drain(..));
        }
        for object_id in starting_cards {
            engine.state.objects.remove(&object_id);
            engine.state.zone_change_generation.remove(&object_id);
        }
    }

    fn add_observer_restore_zone_card(
        engine: &mut GameEngine,
        owner: PlayerId,
        card_id: &str,
        zone: Zone,
    ) -> ObjectId {
        let object_id = add_battlefield_object(engine, owner, card_id);
        let index = engine.state.player_idx(owner).expect("zone-card owner");
        engine.state.players[index]
            .battlefield
            .retain(|candidate| *candidate != object_id);
        match zone {
            Zone::Hand => engine.state.players[index].hand.push(object_id),
            Zone::Library => engine.state.players[index].library.push_front(object_id),
            _ => panic!("observer-restore fixture only moves cards to hand or library"),
        }
        engine.state.objects.get_mut(&object_id).unwrap().zone = zone;
        object_id
    }

    fn captured_printed_trigger_item(
        engine: &GameEngine,
        source: ObjectId,
        card_id: &str,
    ) -> StackItem {
        let definition = engine
            .registry
            .get(card_id)
            .expect("trigger card definition");
        let face = definition.primary_face();
        let ability = face
            .triggered_abilities
            .first()
            .expect("trigger card has a first ability")
            .clone();
        let generation = engine
            .state
            .zone_change_generation
            .get(&source)
            .copied()
            .unwrap_or(0);
        let ability_definition = tricerules_card_model::AbilityDefinitionId {
            card_id: card_id.into(),
            face_id: face.face_id.clone(),
            ability_path: vec![ability.ability_id.clone()],
        };
        let mut item = triggered_item(source, generation);
        item.card_id = card_id.into();
        item.source_owner = engine.state.objects.get(&source).map(|object| object.owner);
        item.triggered_ability = Some(ability);
        item.trigger_use_key = Some(crate::state::TriggerUseKey {
            object_id: source,
            zone_change_generation: generation,
            ability_origin: crate::state::TriggerAbilityOrigin::Printed(ability_definition),
        });
        item
    }

    fn resolution_choice_offer(events: &[rv1::RuledEvent]) -> rv1::ResolutionChoiceRequired {
        events
            .iter()
            .rev()
            .find_map(|event| match &event.ev {
                Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(choice)) => {
                    Some(choice.clone())
                }
                _ => None,
            })
            .expect("resolution-choice offer")
    }

    #[test]
    fn admitted_target_hand_discard_profile_is_not_card_name_bound() {
        let card = r#"(
            id: "alternate_target_discard_fixture",
            name: "Alternate Target Discard Fixture",
            face_id: "alternate_target_discard_fixture",
            mana_cost: "{0}",
            types: ["Instant"],
            spell_effect: [],
            spell_program: Some((body: (steps: [
                (id: "destroy", bind: ["destroyed"], input: None,
                    kind: Perform(Destroy(subject: Chosen((kind: AnyPermanent))))),
                (id: "discard", bind: ["discarded"], input: None,
                    kind: Perform(ChooseHandCards(action: Discard, count: 1,
                        target: (kind: AnyPlayer), chooser: AffectedPlayer,
                        card_filter: None, optional: false))),
            ], yields: []), target_bindings: [
                (instruction_id: "destroy", role: Subject, target_group_index: 0),
                (instruction_id: "discard", role: Target, target_group_index: 1),
            ])),
            targeting: Some((groups: [
                (min: 1, max: 1, prompt: "Choose target permanent", effect_indices: []),
                (min: 1, max: 1, prompt: "Choose target player", effect_indices: []),
            ])),
        )"#;
        let registry = Box::leak(Box::new(
            tricerules_card_model::CardRegistry::from_chunks_and_tokens(&[card], &[])
                .expect("alternate owner with count 1 and target group 1 compiles"),
        ));
        let card_id = "alternate_target_discard_fixture";
        let decks = Some(vec![
            vec![card_id.to_string(); 30],
            vec![card_id.to_string(); 30],
        ]);
        let mut engine = GameEngine::new(registry, 494_041, &[0, 1], 20, decks, true)
            .expect("alternate target-discard engine");
        let owner = tricerules_card_model::ProgramOwnerId::face(
            card_id,
            tricerules_card_model::CardFaceId::new(card_id).unwrap(),
        );
        let compiled = engine
            .registry
            .compiled_program_for_owner(&owner)
            .expect("alternate target-discard Program compiled");
        assert_eq!(compiled.target_bindings.len(), 2);
        let discard = compiled
            .body
            .steps
            .get(1)
            .expect("discard is second instruction");
        let [discard_output] = discard.outputs.as_slice() else {
            panic!("targeted hand discard binds one typed result");
        };
        assert!(matches!(
            compiled
                .slots
                .get(discard_output.0 as usize)
                .map(|slot| &slot.result_type),
            Some(
                tricerules_card_model::primitives::ResultType::TargetCardResultCohort {
                    action: tricerules_card_model::primitives::CardResultAction::Discard,
                    cardinality: tricerules_card_model::primitives::Cardinality {
                        min: 0,
                        max: Some(1),
                    },
                    target_group_index: 1,
                }
            )
        ));

        let spell = *engine.state.players[0]
            .hand
            .first()
            .expect("fixture spell dealt");
        move_object_to_zone(&mut engine.state, engine.registry, spell, Zone::Stack, None)
            .expect("put fixture spell on the stack");
        let mut item = engine.observer_return_item(spell, 0);
        item.card_id = card_id.to_string();
        item.ability_text = None;
        item.is_copy = false;
        item.cast_by = Some(0);
        item.targets = vec![
            crate::state::StackTarget {
                object_id: 99,
                group_index: 0,
                damage_amount: 0,
                kind: rv1::TargetRefKind::Permanent as i32,
                zone_change_generation: None,
                required_controller: None,
            },
            crate::state::StackTarget {
                object_id: 1,
                group_index: 1,
                damage_amount: 0,
                kind: rv1::TargetRefKind::Player as i32,
                zone_change_generation: None,
                required_controller: None,
            },
        ];
        let mut run = engine
            .begin_program_run(&item, vec![owner.clone()])
            .expect("alternate target-discard Program starts");
        run.frames.last_mut().unwrap().next_instruction = 2;
        run.in_flight = Some(discard.outputs.clone());
        let mut parked = ParkedStackResolution::new(item);
        parked.program = Some(run);

        let signature = engine
            .target_hand_discard_program_signature(&parked)
            .expect(
                "derive the count, result slot, and nonzero target group from compiled metadata",
            );
        assert_eq!(signature.card_count, 1);
        assert_eq!(signature.target_group_index, 1);
        assert_eq!(
            signature.cardinality,
            tricerules_card_model::primitives::Cardinality {
                min: 0,
                max: Some(1),
            }
        );
        assert_eq!(signature.result_slot, *discard_output);
        assert!(
            engine.is_target_hand_discard_program(&parked),
            "the admitted alternate owner must share the same target-hand-discard continuation profile"
        );

        let mut wrong_output = parked.clone();
        wrong_output.program.as_mut().unwrap().in_flight =
            Some(compiled.body.steps[0].outputs.clone());
        assert!(
            !engine.is_target_hand_discard_program(&wrong_output),
            "a current hand-discard leaf with another instruction's result must fail closed"
        );

        let mut wrong_cursor = parked.clone();
        wrong_cursor
            .program
            .as_mut()
            .unwrap()
            .frames
            .last_mut()
            .unwrap()
            .next_instruction = 1;
        assert!(
            !engine.is_target_hand_discard_program(&wrong_cursor),
            "the typed discard output cannot classify a different current operation"
        );

        let mut wrong_group = parked;
        wrong_group.item.targets[1].group_index = 0;
        assert!(
            !engine.is_target_hand_discard_program(&wrong_group),
            "the current target projection must match the compiled result group"
        );
    }

    #[test]
    fn synthetic_observer_restore_of_actual_gerrard_choice_rebuilds_private_hand_candidates() {
        // The owed observer return is staged synthetically; the parked chooser is the actual
        // Gerrard's Verdict hand-choice effect and its target remains the only private recipient.
        let mut engine = observer_restore_test_engine(494_040);
        let spell = add_observer_restore_zone_card(&mut engine, 0, "gerrards_verdict", Zone::Hand);
        let original_candidate =
            add_observer_restore_zone_card(&mut engine, 1, "forest", Zone::Hand);
        let departed_candidate =
            add_observer_restore_zone_card(&mut engine, 1, "mountain", Zone::Hand);

        move_object_to_zone(&mut engine.state, engine.registry, spell, Zone::Stack, None)
            .expect("put actual Gerrard's Verdict on the stack");
        let source_generation = engine.state.zone_change_generation[&spell];
        let mut item = engine.observer_return_item(spell, 0);
        item.card_id = "gerrards_verdict".into();
        item.controller = 0;
        item.is_copy = false;
        item.ability_text = None;
        item.source_owner = Some(0);
        item.source_zone_change = source_generation;
        item.cast_by = Some(0);
        item.cast_occurrence = Some(crate::state::StackObjectRef {
            object_id: spell,
            zone_change_generation: Some(source_generation),
        });
        item.targets = vec![crate::state::StackTarget {
            object_id: 1,
            group_index: 0,
            damage_amount: 0,
            kind: rv1::TargetRefKind::Player as i32,
            zone_change_generation: None,
            required_controller: None,
        }];

        let owners = engine
            .compiled_resolution_program_owners(&item)
            .expect("inspect actual Gerrard's Verdict owners")
            .expect("the actual spell face owns a compiled Program");
        let program = engine
            .begin_program_run(&item, owners)
            .expect("start the actual Gerrard's Verdict Program");
        let mut initial_events = Vec::new();
        assert_eq!(
            engine
                .run_program_with_previous(
                    &item,
                    program,
                    EffectResult::default(),
                    &mut initial_events,
                )
                .expect("run actual Gerrard's Verdict to its target's hand choice"),
            ResolutionProgress::Parked
        );
        engine
            .publish_resolution_prompt_ids(&mut initial_events)
            .expect("publish the actual targeted hand choice");
        let initial_offer = resolution_choice_offer(&initial_events);
        assert_eq!(initial_offer.deciding_player_id, 1);
        assert_eq!(
            initial_offer.candidate_object_ids,
            [original_candidate, departed_candidate]
        );
        assert_eq!((initial_offer.min, initial_offer.max), (2, 2));
        let original_prompt_id = initial_offer.resolution_prompt_id;

        let original_pending = engine
            .state
            .pending_resolution
            .take()
            .expect("the actual hand choice is suspended for the synthetic observer");
        let crate::state::ResolutionContinuation::HandChoice { stack, hand_choice } =
            &original_pending.continuation
        else {
            panic!("actual Gerrard remains parked on its targeted hand choice");
        };
        assert_eq!(stack.item.id, spell);
        assert_eq!(hand_choice.affected_player, 1);
        assert_eq!(hand_choice.candidate_generations.len(), 2);
        assert!(!hand_choice.program_selection);
        let program = stack
            .program
            .as_ref()
            .expect("actual Gerrard choice retains its compiled Program cursor");
        assert_eq!(
            program.owner,
            tricerules_card_model::ProgramOwnerId::face(
                "gerrards_verdict",
                tricerules_card_model::CardFaceId::new("gerrard_s_verdict").unwrap(),
            )
        );
        assert_eq!(program.frames.last().unwrap().next_instruction, 1);
        assert_eq!(program.in_flight.as_deref().unwrap().len(), 1);
        let output = program.in_flight.as_ref().unwrap()[0];
        assert!(matches!(
            engine
                .registry
                .compiled_program_for_owner(&program.owner)
                .unwrap()
                .slots
                .get(output.0 as usize)
                .map(|slot| &slot.result_type),
            Some(
                tricerules_card_model::primitives::ResultType::TargetCardResultCohort {
                    action: tricerules_card_model::primitives::CardResultAction::Discard,
                    cardinality: tricerules_card_model::primitives::Cardinality {
                        min: 0,
                        max: Some(2),
                    },
                    target_group_index: 0,
                }
            )
        ));
        assert!(engine.is_target_hand_discard_program(stack));
        assert_eq!(
            hand_choice
                .candidate_generations
                .iter()
                .map(|(object_id, _)| *object_id)
                .collect::<Vec<_>>(),
            [original_candidate, departed_candidate]
        );
        assert!(hand_choice
            .candidate_generations
            .iter()
            .all(|(object_id, generation)| {
                engine
                    .state
                    .zone_change_generation
                    .get(object_id)
                    .copied()
                    .unwrap_or(0)
                    == *generation
            }));

        for departed in [1, 0] {
            let mut saved_engine = engine.clone();
            let owed_aura =
                add_observer_restore_zone_card(&mut saved_engine, 2, "pacifism", Zone::Hand);
            move_object_to_zone(
                &mut saved_engine.state,
                saved_engine.registry,
                owed_aura,
                Zone::Exile,
                None,
            )
            .expect("stage the synthetic owed Aura's Exile incarnation");
            let aura_generation = saved_engine.state.zone_change_generation[&owed_aura];
            let aura_target = add_battlefield_object(&mut saved_engine, 2, "grizzly_bears");
            saved_engine.state.pending_observer_return_batch =
                Some(crate::state::PendingObserverReturnBatch {
                    ready: Vec::new(),
                    remaining: Default::default(),
                    resume_stack: None,
                    suspended_resolution: Some(original_pending.clone()),
                    suspended_replacement_event: None,
                });
            saved_engine.state.pending_resolution = Some(crate::state::PendingResolution {
                deciding_player: 2,
                presentation: crate::state::PendingResolutionPresentation {
                    source_object_id: owed_aura,
                    candidates: vec![aura_target],
                    min: 1,
                    max: 1,
                    ordered: false,
                    prompt: "Synthetic owed Aura-return choice".into(),
                    choice_kind: rv1::ChoiceKind::AuraPermanent,
                    unique_names: false,
                },
                continuation: crate::state::ResolutionContinuation::AuraReturn {
                    stack: None,
                    exiled: TriggerObjectRef {
                        object_id: owed_aura,
                        zone_change_generation: aura_generation,
                        controller_at_event: 2,
                    },
                },
                // The synthetic top Aura prompt was already published before this fixture's
                // leave command; only the saved Gerrard prompt will be freshly rebuilt below.
                resolution_prompt_id: crate::state::ResolutionPromptId::assigned(1).unwrap(),
                resolution_prompt_semantics: None,
            });
            saved_engine
                .apply_command(
                    departed,
                    &rv1::RuledCommand {
                        cmd: Some(rv1::ruled_command::Cmd::Concede(rv1::Concede {})),
                    },
                )
                .expect("a player can leave while the Gerrard choice waits below an observer");
            assert!(
                saved_engine.state.players[saved_engine
                    .state
                    .player_idx(departed)
                    .expect("departing player")]
                .has_lost
            );
            let saved_observer = saved_engine
                .state
                .pending_observer_return_batch
                .take()
                .expect("the synthetic owed observer remains pending");
            assert!(matches!(
                saved_engine
                    .state
                    .pending_resolution
                    .take()
                    .expect("the synthetic Aura return still has its own choice")
                    .continuation,
                crate::state::ResolutionContinuation::AuraReturn { .. }
            ));
            let mut restored_events = Vec::new();
            assert!(saved_engine
                .restore_suspended_observer_resolution(
                    saved_observer.suspended_resolution,
                    saved_observer.suspended_replacement_event,
                    &mut restored_events,
                )
                .expect("restore or retire the saved Gerrard choice"));

            if departed == 1 {
                assert!(
                    saved_engine.state.pending_resolution.is_none(),
                    "a saved choice for the departed target completes with an empty cohort"
                );
                assert!(saved_engine.state.stack.is_empty());
                assert_eq!(saved_engine.state.players[0].life, 20);
                assert!(restored_events.iter().all(|event| !matches!(
                    &event.ev,
                    Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(choice))
                        if choice.deciding_player_id == 2
                )));
            } else {
                saved_engine
                    .publish_resolution_prompt_ids(&mut restored_events)
                    .expect("restoration publishes a fresh live-target prompt");
                let restored_offer = resolution_choice_offer(&restored_events);
                assert_eq!(restored_offer.deciding_player_id, 1);
                assert_ne!(restored_offer.resolution_prompt_id, original_prompt_id);
                assert_eq!(
                    restored_offer.candidate_object_ids,
                    [original_candidate, departed_candidate]
                );
                saved_engine
                    .apply_command(
                        1,
                        &submit_choice_command(
                            restored_offer.resolution_prompt_id,
                            vec![original_candidate, departed_candidate],
                        ),
                    )
                    .expect("the live target finishes the saved discard");
                assert!(saved_engine.state.pending_resolution.is_none());
                assert!(saved_engine.state.stack.is_empty());
                assert_eq!(saved_engine.state.players[0].life, 20);
                assert_eq!(saved_engine.state.players[1].life, 20);
                assert_eq!(saved_engine.state.players[2].life, 20);
            }
        }

        move_object_to_zone(
            &mut engine.state,
            engine.registry,
            departed_candidate,
            Zone::Graveyard,
            None,
        )
        .expect("observer resolves while one original candidate leaves the hand");
        let replacement_candidate =
            add_observer_restore_zone_card(&mut engine, 1, "plains", Zone::Hand);

        engine.state.pending_observer_return_batch =
            Some(crate::state::PendingObserverReturnBatch {
                ready: Vec::new(),
                remaining: Default::default(),
                resume_stack: None,
                suspended_resolution: Some(original_pending),
                suspended_replacement_event: None,
            });
        let observer = engine
            .state
            .pending_observer_return_batch
            .take()
            .expect("synthetic owed Aura-return observer batch");
        let mut restored = Vec::new();
        assert!(engine
            .restore_suspended_observer_resolution(
                observer.suspended_resolution,
                observer.suspended_replacement_event,
                &mut restored,
            )
            .expect("restore the saved actual Gerrard choice"));
        engine
            .publish_resolution_prompt_ids(&mut restored)
            .expect("restoration publishes a fresh target-only prompt");

        let restored_offer = resolution_choice_offer(&restored);
        assert_ne!(restored_offer.resolution_prompt_id, original_prompt_id);
        assert_eq!(restored_offer.deciding_player_id, 1);
        assert_eq!(
            restored_offer.candidate_object_ids,
            [original_candidate, replacement_candidate],
            "the private offer is rebuilt from P1's current hand"
        );
        assert_eq!((restored_offer.min, restored_offer.max), (2, 2));
        let before_stale_answer = engine.diagnostic_snapshot().unwrap();
        let command_index_before_stale_answer = engine.state.command_index;
        let stale = engine
            .apply_command(
                1,
                &submit_choice_command(
                    original_prompt_id,
                    vec![original_candidate, replacement_candidate],
                ),
            )
            .expect_err("the old same-player prompt cannot answer the refreshed offer");
        assert!(stale.to_string().contains("stale"));
        assert_eq!(
            engine.state.command_index,
            command_index_before_stale_answer
        );
        assert_eq!(engine.diagnostic_snapshot().unwrap(), before_stale_answer);
        assert!(restored.iter().all(|event| match &event.ev {
            Some(rv1::ruled_event::Ev::Log(log)) => {
                !log.text.contains("Forest")
                    && !log.text.contains("Mountain")
                    && !log.text.contains("Plains")
            }
            _ => true,
        }));
    }

    fn submit_choice_command(
        prompt_id: u64,
        chosen_object_ids: Vec<ObjectId>,
    ) -> rv1::RuledCommand {
        rv1::RuledCommand {
            cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(
                rv1::SubmitResolutionChoice {
                    resolution_prompt_id: prompt_id,
                    chosen_object_ids,
                    ..Default::default()
                },
            )),
        }
    }

    #[test]
    fn synthetic_ward_discard_observer_restore_rebuilds_candidates_and_keeps_cursor() {
        // This deliberately stages the internal observer-return continuation. A live discard-only
        // Ward offer does not grant priority to activate a mana ability; the fixture isolates the
        // saved-choice restore boundary without claiming that interaction is reachable in a game.
        let mut engine = observer_restore_test_engine(494_036);
        let ward_source = add_battlefield_object(&mut engine, 1, "spectral_snatcher");
        let initial_candidate =
            add_observer_restore_zone_card(&mut engine, 0, "grizzly_bears", Zone::Hand);
        let target = add_battlefield_object(&mut engine, 0, "unsummon");
        let target_generation = 1;
        let target_owner_index = engine.state.player_idx(0).expect("target owner");
        engine.state.players[target_owner_index]
            .battlefield
            .retain(|candidate| *candidate != target);
        engine
            .state
            .objects
            .get_mut(&target)
            .expect("targeting spell object")
            .zone = Zone::Stack;
        engine
            .state
            .zone_change_generation
            .insert(target, target_generation);
        let mut target_item = triggered_item(target, target_generation);
        target_item.id = target;
        target_item.controller = 0;
        target_item.card_id = "unsummon".into();
        target_item.source_permanent_id = None;
        target_item.source_owner = Some(0);
        target_item.triggered_ability = None;
        target_item.is_triggered = false;
        engine.state.stack.push(target_item);

        let mut ward_item =
            captured_printed_trigger_item(&engine, ward_source, "spectral_snatcher");
        ward_item.controller = 1;
        ward_item.trigger_context.affected_player = Some(0);
        ward_item.trigger_context.targeting_stack_object = Some(crate::state::StackObjectRef {
            object_id: target,
            zone_change_generation: Some(target_generation),
        });
        let owners = engine
            .compiled_resolution_program_owners(&ward_item)
            .expect("resolve printed Spectral Snatcher Program owners")
            .expect("printed Spectral Snatcher Ward owns a Program");
        let program = engine
            .begin_program_run(&ward_item, owners)
            .expect("begin the printed Ward Program");
        let mut started = Vec::new();
        assert_eq!(
            engine
                .run_program_with_previous(
                    &ward_item,
                    program,
                    crate::state::EffectResult::default(),
                    &mut started,
                )
                .expect("run the printed discard Ward Program"),
            ResolutionProgress::Parked
        );
        engine
            .publish_resolution_prompt_ids(&mut started)
            .expect("publish the initial Ward discard prompt");
        let original_offer = resolution_choice_offer(&started);
        let original_prompt_id = original_offer.resolution_prompt_id;
        assert_ne!(original_prompt_id, 0);
        let original_pending = engine
            .state
            .pending_resolution
            .take()
            .expect("Ward discard choice is saved for observer return");
        let original_payer = original_pending.deciding_player;
        let ResolutionContinuation::WardPayment { stack, ward } = &original_pending.continuation
        else {
            panic!("saved observer fixture retains its Ward continuation");
        };
        let original_target = ward.target;
        let original_program_debug = format!("{:?}", stack.program);
        let original_previous_result_debug = format!("{:?}", stack.previous_result);
        let original_stack_id = stack.item.id;
        let original_source_card_id = stack.item.card_id.clone();
        let original_source_controller = stack.item.controller;
        let original_source_owner = stack.item.source_owner;
        let original_source_permanent_id = stack.item.source_permanent_id;
        let original_source_generation = stack.item.source_zone_change;
        let original_trigger_use_key = format!("{:?}", stack.item.trigger_use_key);
        let original_trigger_context = stack.item.trigger_context;
        let original_resume_effect_index = stack.resume_effect_index;
        assert_eq!(original_resume_effect_index, None);
        assert!(stack.program.is_some(), "Ward parks its Program cursor");
        let crate::state::PendingWardPaymentStage::Discard {
            candidate_generations: original_generations,
        } = &ward.stage
        else {
            panic!("printed Spectral Snatcher uses discard Ward");
        };
        assert_eq!(original_generations.len(), 1);
        assert_eq!(original_generations[0].0, initial_candidate);

        let original_generation = engine
            .state
            .zone_change_generation
            .get(&initial_candidate)
            .copied()
            .unwrap_or(0);
        move_object_to_zone(
            &mut engine.state,
            engine.registry,
            initial_candidate,
            Zone::Graveyard,
            None,
        )
        .expect("temporarily move the saved candidate out of hand");
        move_object_to_zone(
            &mut engine.state,
            engine.registry,
            initial_candidate,
            Zone::Hand,
            None,
        )
        .expect("return the physical card as a new hand incarnation");
        let refreshed_candidate =
            add_observer_restore_zone_card(&mut engine, 0, "island", Zone::Hand);
        let refreshed_generation = engine
            .state
            .zone_change_generation
            .get(&initial_candidate)
            .copied()
            .unwrap_or(0);
        assert!(refreshed_generation > original_generation);

        engine.state.pending_observer_return_batch =
            Some(crate::state::PendingObserverReturnBatch {
                ready: Vec::new(),
                remaining: Default::default(),
                resume_stack: None,
                suspended_resolution: Some(original_pending),
                suspended_replacement_event: None,
            });
        let observer = engine
            .state
            .pending_observer_return_batch
            .take()
            .expect("synthetic observer batch fixture");
        let mut restored = Vec::new();
        assert!(engine
            .restore_suspended_observer_resolution(
                observer.suspended_resolution,
                observer.suspended_replacement_event,
                &mut restored,
            )
            .expect("restore the saved Ward choice"));
        engine
            .publish_resolution_prompt_ids(&mut restored)
            .expect("restoration must publish a fresh, current Ward choice");
        let restored_offer = resolution_choice_offer(&restored);
        assert_ne!(restored_offer.resolution_prompt_id, original_prompt_id);
        assert_eq!(
            restored_offer.candidate_object_ids,
            [initial_candidate, refreshed_candidate]
        );
        let restored_pending = engine
            .state
            .pending_resolution
            .as_ref()
            .expect("restored Ward payment remains pending");
        assert_eq!(restored_pending.deciding_player, original_payer);
        assert_eq!(
            restored_pending.resolution_prompt_id.get(),
            restored_offer.resolution_prompt_id
        );
        assert_eq!(
            restored_pending.presentation.candidates,
            [initial_candidate, refreshed_candidate]
        );
        let ResolutionContinuation::WardPayment { stack, ward } = &restored_pending.continuation
        else {
            panic!("restored observer fixture retains its Ward continuation");
        };
        assert_eq!(ward.target, original_target);
        assert_eq!(stack.item.id, original_stack_id);
        assert_eq!(stack.item.card_id, original_source_card_id);
        assert_eq!(stack.item.controller, original_source_controller);
        assert_eq!(stack.item.source_owner, original_source_owner);
        assert_eq!(stack.item.source_permanent_id, original_source_permanent_id);
        assert_eq!(stack.item.source_zone_change, original_source_generation);
        assert_eq!(
            format!("{:?}", stack.item.trigger_use_key),
            original_trigger_use_key
        );
        assert_eq!(stack.item.trigger_context, original_trigger_context);
        assert_eq!(format!("{:?}", stack.program), original_program_debug);
        assert_eq!(
            format!("{:?}", stack.previous_result),
            original_previous_result_debug
        );
        assert_eq!(stack.resume_effect_index, original_resume_effect_index);
        let crate::state::PendingWardPaymentStage::Discard {
            candidate_generations,
        } = &ward.stage
        else {
            panic!("restored Ward continuation retains its discard payment");
        };
        assert_eq!(candidate_generations.len(), 2);
        assert_eq!(candidate_generations[0].0, initial_candidate);
        assert_eq!(candidate_generations[0].1, refreshed_generation);
        assert_eq!(candidate_generations[1].0, refreshed_candidate);
        assert_eq!(
            candidate_generations[1].1,
            engine
                .state
                .zone_change_generation
                .get(&refreshed_candidate)
                .copied()
                .unwrap_or(0)
        );
    }

    fn synthetic_saved_spectral_ward_observer_batch(
        seed: u64,
        ward_controller: PlayerId,
        source_owner: PlayerId,
        payer: PlayerId,
    ) -> (
        GameEngine,
        ObjectId,
        ObjectId,
        ObjectId,
        ObjectId,
        ObjectId,
        String,
    ) {
        // Internal observer staging intentionally tests a saved Ward branch across concession.
        // This does not claim a live discard-only Ward prompt grants priority for mana abilities.
        let mut engine = observer_restore_test_engine(seed);
        let ward_source = add_battlefield_object(&mut engine, source_owner, "spectral_snatcher");
        let original_candidate =
            add_observer_restore_zone_card(&mut engine, payer, "grizzly_bears", Zone::Hand);
        let target = add_battlefield_object(&mut engine, payer, "unsummon");
        let target_generation = 1;
        let target_owner_index = engine.state.player_idx(payer).expect("target owner");
        engine.state.players[target_owner_index]
            .battlefield
            .retain(|candidate| *candidate != target);
        engine
            .state
            .objects
            .get_mut(&target)
            .expect("targeting spell object")
            .zone = Zone::Stack;
        engine
            .state
            .zone_change_generation
            .insert(target, target_generation);
        let mut target_item = triggered_item(target, target_generation);
        target_item.id = target;
        target_item.controller = payer;
        target_item.card_id = "unsummon".into();
        target_item.source_permanent_id = None;
        target_item.source_owner = Some(payer);
        target_item.triggered_ability = None;
        target_item.is_triggered = false;
        engine.state.stack.push(target_item);

        let mut ward_item =
            captured_printed_trigger_item(&engine, ward_source, "spectral_snatcher");
        ward_item.controller = ward_controller;
        ward_item.trigger_context.affected_player = Some(payer);
        ward_item.trigger_context.targeting_stack_object = Some(crate::state::StackObjectRef {
            object_id: target,
            zone_change_generation: Some(target_generation),
        });
        let ward_stack_id = ward_item.id;
        let owners = engine
            .compiled_resolution_program_owners(&ward_item)
            .expect("resolve printed Spectral Snatcher Program owners")
            .expect("printed Spectral Snatcher Ward owns a Program");
        let program = engine
            .begin_program_run(&ward_item, owners)
            .expect("begin the printed Ward Program");
        let mut started = Vec::new();
        assert_eq!(
            engine
                .run_program_with_previous(
                    &ward_item,
                    program,
                    crate::state::EffectResult::default(),
                    &mut started,
                )
                .expect("run the printed discard Ward Program"),
            ResolutionProgress::Parked
        );
        engine
            .publish_resolution_prompt_ids(&mut started)
            .expect("publish the initial Ward discard prompt");
        let original_offer = resolution_choice_offer(&started);
        let original_prompt_id = original_offer.resolution_prompt_id;
        let original_pending = engine
            .state
            .pending_resolution
            .take()
            .expect("Ward discard choice is saved for observer return");
        assert_eq!(original_pending.deciding_player, payer);
        let ResolutionContinuation::WardPayment { stack, ward } = &original_pending.continuation
        else {
            panic!("saved observer fixture retains its Ward continuation");
        };
        assert_eq!(ward.target.object_id, target);
        assert!(
            stack.program.is_some(),
            "saved Ward keeps its Program cursor"
        );
        let original_program = format!("{:?}", stack.program);
        let original_generation = engine
            .state
            .zone_change_generation
            .get(&original_candidate)
            .copied()
            .unwrap_or(0);
        move_object_to_zone(
            &mut engine.state,
            engine.registry,
            original_candidate,
            Zone::Graveyard,
            None,
        )
        .expect("temporarily move the saved candidate out of hand");
        move_object_to_zone(
            &mut engine.state,
            engine.registry,
            original_candidate,
            Zone::Hand,
            None,
        )
        .expect("return the physical card as a new hand incarnation");
        let refreshed_candidate =
            add_observer_restore_zone_card(&mut engine, payer, "island", Zone::Hand);
        assert!(
            engine.state.zone_change_generation[&original_candidate] > original_generation,
            "the re-entered candidate has a fresh incarnation"
        );
        engine.state.pending_observer_return_batch =
            Some(crate::state::PendingObserverReturnBatch {
                ready: Vec::new(),
                remaining: Default::default(),
                resume_stack: None,
                suspended_resolution: Some(original_pending),
                suspended_replacement_event: None,
            });
        (
            engine,
            ward_source,
            target,
            original_candidate,
            refreshed_candidate,
            ward_stack_id,
            format!("{original_prompt_id}:{original_program}"),
        )
    }

    // Keep expected identity, cursor, prompt, and candidate values explicit in this focused test.
    #[allow(clippy::too_many_arguments)]
    fn assert_saved_spectral_ward_prompt_after_concession(
        engine: &GameEngine,
        events: &[rv1::RuledEvent],
        payer: PlayerId,
        ward_controller: PlayerId,
        source_owner: PlayerId,
        source: ObjectId,
        target: ObjectId,
        original_candidate: ObjectId,
        refreshed_candidate: ObjectId,
        ward_stack_id: ObjectId,
        original_prompt_and_program: &str,
    ) {
        let original_prompt_id = original_prompt_and_program
            .split_once(':')
            .expect("fixture prompt/program separator")
            .0
            .parse::<u64>()
            .expect("fixture prompt ID");
        let original_program = original_prompt_and_program
            .split_once(':')
            .expect("fixture prompt/program separator")
            .1;
        let offer = resolution_choice_offer(events);
        assert_ne!(offer.resolution_prompt_id, original_prompt_id);
        assert_eq!(offer.deciding_player_id, payer);
        assert_eq!(
            offer.candidate_object_ids,
            [original_candidate, refreshed_candidate]
        );
        assert!(engine.state.pending_observer_return_batch.is_none());
        let pending = engine
            .state
            .pending_resolution
            .as_ref()
            .expect("concession restores the saved Ward offer");
        assert_eq!(pending.deciding_player, payer);
        assert_eq!(
            pending.presentation.candidates,
            [original_candidate, refreshed_candidate]
        );
        assert_eq!(
            pending.resolution_prompt_id.get(),
            offer.resolution_prompt_id
        );
        let ResolutionContinuation::WardPayment { stack, ward } = &pending.continuation else {
            panic!("restored continuation remains on its Ward payment");
        };
        assert_eq!(ward.target.object_id, target);
        assert!(stack.program.is_some());
        // ProgramRunState's debug snapshot includes frames, instruction cursor, and in-flight op.
        assert_eq!(format!("{:?}", stack.program), original_program);
        assert_eq!(stack.resume_effect_index, None);
        assert_eq!(stack.item.controller, ward_controller);
        assert_eq!(stack.item.source_owner, Some(source_owner));
        assert_eq!(stack.item.source_permanent_id, Some(source));
        assert_eq!(stack.item.trigger_context.affected_player, Some(payer));
        assert_eq!(
            stack
                .item
                .trigger_context
                .targeting_stack_object
                .as_ref()
                .map(|object| object.object_id),
            Some(target)
        );
        let crate::state::PendingWardPaymentStage::Discard {
            candidate_generations,
        } = &ward.stage
        else {
            panic!("restored Ward offer remains a discard payment");
        };
        assert_eq!(
            candidate_generations
                .iter()
                .map(|(object_id, _)| *object_id)
                .collect::<Vec<_>>(),
            [original_candidate, refreshed_candidate]
        );
        assert_eq!(
            candidate_generations[0].1,
            engine
                .state
                .zone_change_generation
                .get(&original_candidate)
                .copied()
                .unwrap_or(0)
        );
        assert_eq!(
            candidate_generations[1].1,
            engine
                .state
                .zone_change_generation
                .get(&refreshed_candidate)
                .copied()
                .unwrap_or(0)
        );
        assert_eq!(stack.item.id, ward_stack_id);
    }

    #[test]
    fn synthetic_saved_spectral_ward_controller_concedes_and_keeps_program_choice() {
        let (mut engine, source, target, original, refreshed, ward_stack, saved) =
            synthetic_saved_spectral_ward_observer_batch(494_037, 1, 2, 0);
        let batch = engine
            .apply_command(
                1,
                &rv1::RuledCommand {
                    cmd: Some(rv1::ruled_command::Cmd::Concede(rv1::Concede {})),
                },
            )
            .expect("Ward controller concedes during the synthetic saved observer batch");
        assert!(engine.state.players[engine.state.player_idx(1).unwrap()].has_lost);
        assert!(engine.state.objects.contains_key(&source));
        assert!(engine.state.objects.contains_key(&target));
        assert_saved_spectral_ward_prompt_after_concession(
            &engine,
            &batch.events,
            0,
            1,
            2,
            source,
            target,
            original,
            refreshed,
            ward_stack,
            &saved,
        );
    }

    #[test]
    fn synthetic_saved_spectral_ward_source_owner_concedes_and_keeps_program_choice() {
        let (mut engine, source, target, original, refreshed, ward_stack, saved) =
            synthetic_saved_spectral_ward_observer_batch(494_038, 2, 1, 0);
        let batch = engine
            .apply_command(
                1,
                &rv1::RuledCommand {
                    cmd: Some(rv1::ruled_command::Cmd::Concede(rv1::Concede {})),
                },
            )
            .expect("physical Ward source owner concedes during saved observer batch");
        assert!(engine.state.players[engine.state.player_idx(1).unwrap()].has_lost);
        assert!(!engine.state.objects.contains_key(&source));
        assert!(engine.state.objects.contains_key(&target));
        assert_saved_spectral_ward_prompt_after_concession(
            &engine,
            &batch.events,
            0,
            2,
            1,
            source,
            target,
            original,
            refreshed,
            ward_stack,
            &saved,
        );
    }

    #[test]
    fn synthetic_saved_spectral_ward_payer_concedes_without_restoring_choice_or_parent() {
        let (mut engine, source, target, original, refreshed, ward_stack, _saved) =
            synthetic_saved_spectral_ward_observer_batch(494_039, 1, 2, 0);
        let batch = engine
            .apply_command(
                0,
                &rv1::RuledCommand {
                    cmd: Some(rv1::ruled_command::Cmd::Concede(rv1::Concede {})),
                },
            )
            .expect("Ward payer concedes after their objects leave");
        assert!(engine.state.players[engine.state.player_idx(0).unwrap()].has_lost);
        assert!(!engine.state.objects.contains_key(&target));
        assert!(!engine.state.objects.contains_key(&original));
        assert!(!engine.state.objects.contains_key(&refreshed));
        assert!(engine.state.objects.contains_key(&source));
        assert!(engine.state.pending_resolution.is_none());
        assert!(engine.state.pending_observer_return_batch.is_none());
        assert!(batch.events.iter().all(|event| !matches!(
            event.ev.as_ref(),
            Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(_))
        )));
        assert!(engine
            .state
            .stack
            .iter()
            .all(|item| item.id != target && item.id != ward_stack));
    }

    fn begin_linked_play_fixture(
        engine: &mut GameEngine,
        linked_cards: &[ObjectId],
        play_kind: tricerules_card_model::primitives::LinkedExiledCardPlayKind,
    ) -> (ObjectId, StackItem) {
        begin_linked_play_fixture_for(engine, 0, linked_cards, play_kind)
    }

    fn begin_linked_play_fixture_for(
        engine: &mut GameEngine,
        chooser: PlayerId,
        linked_cards: &[ObjectId],
        play_kind: tricerules_card_model::primitives::LinkedExiledCardPlayKind,
    ) -> (ObjectId, StackItem) {
        let source = add_battlefield_object(engine, chooser, "nautiloid_ship");
        let source_generation = engine
            .state
            .zone_change_generation
            .get(&source)
            .copied()
            .unwrap_or(0);
        let mut item = triggered_item(source, source_generation);
        item.controller = chooser;
        item.source_owner = Some(chooser);
        item.card_id = "nautiloid_ship".to_string();
        item.trigger_context.linked_exile_occurrence =
            Some(crate::state::LinkedExileOccurrence::NativeOrTokenBase);
        let mut ability = engine
            .registry
            .get("nautiloid_ship")
            .unwrap()
            .primary_face()
            .triggered_abilities[1]
            .clone();
        ability.effect = vec![
            SpellEffectKind::PlayLinkedExiledCard {
                producer_ability_id: tricerules_card_model::AbilityId::new(
                    "exile_target_graveyard",
                )
                .unwrap(),
                linked_exile_id: tricerules_card_model::AbilityLinkId::new("exiled_cards").unwrap(),
                filter: None,
                play_kind,
                return_source_to_owner_hand_if_cast: false,
            },
            SpellEffectKind::GainLife {
                amount: Amount::Fixed(2),

                who: tricerules_card_model::primitives::PlayerRecipient::Controller,
            },
        ];
        item.triggered_ability = Some(ability);

        let key = zones::linked_exile_key(
            engine,
            &item,
            tricerules_card_model::AbilityLinkId::new("exiled_cards").unwrap(),
            tricerules_card_model::AbilityId::new("exile_target_graveyard").unwrap(),
        )
        .unwrap();
        engine.state.linked_exile_records.insert(
            key,
            linked_cards
                .iter()
                .copied()
                .map(|object_id| crate::state::LinkedExiledObject {
                    object_id,
                    zone_change_generation: engine.state.zone_change_generation[&object_id],
                })
                .collect(),
        );

        let (effects, label) = engine.build_resolution_effects(&item);
        let mut events = Vec::new();
        engine
            .run_effect_list(&item, &label, effects, 0, &mut events)
            .unwrap();
        engine.publish_resolution_prompt_ids(&mut events).unwrap();
        (source, item)
    }

    fn exile_for_fixture(engine: &mut GameEngine, owner: PlayerId, card_id: &str) -> ObjectId {
        let object_id = add_battlefield_object(engine, owner, card_id);
        move_object_to_zone(
            &mut engine.state,
            engine.registry,
            object_id,
            Zone::Exile,
            None,
        )
        .unwrap();
        object_id
    }

    fn queue_observer_aura_return(engine: &mut GameEngine, object_id: ObjectId) {
        engine.state.pending_immediate_observer_actions.push(
            ImmediateObserverAction::ReturnExiledObject {
                exiled: TriggerObjectRef {
                    object_id,
                    zone_change_generation: engine.state.zone_change_generation[&object_id],
                    controller_at_event: engine.state.objects[&object_id].owner,
                },
            },
        );
    }

    fn observe_leave_and_return_aura(
        engine: &mut GameEngine,
        watched_source: ObjectId,
        exiled_aura: ObjectId,
    ) {
        engine
            .state
            .active_event_observers
            .push(ActiveEventObserver {
                watched: Some(TriggerObjectRef {
                    object_id: watched_source,
                    zone_change_generation: engine
                        .state
                        .zone_change_generation
                        .get(&watched_source)
                        .copied()
                        .unwrap_or(0),
                    controller_at_event: engine.state.objects[&watched_source].controller,
                }),
                matcher: EventObserverMatcher::WhenWatchedObjectLeavesBattlefield,
                payload: EventObserverPayload::ReturnExiledObject {
                    exiled: TriggerObjectRef {
                        object_id: exiled_aura,
                        zone_change_generation: engine.state.zone_change_generation[&exiled_aura],
                        controller_at_event: engine.state.objects[&exiled_aura].owner,
                    },
                },
            });
    }

    fn reserve_linked_spell_for_test(engine: &mut GameEngine, object_id: ObjectId) -> u64 {
        engine
            .apply_scenario_command(
                0,
                &RuledCommand {
                    cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(
                        rv1::SubmitResolutionChoice {
                            chosen_object_ids: vec![object_id],
                            ..Default::default()
                        },
                    )),
                },
            )
            .unwrap();
        let permission = engine.special_cast_permission(0).unwrap();
        let generation = engine.state.zone_change_generation[&object_id];
        let announcement = rv1::SpellCastAnnouncement {
            source: Some(rv1::CastSource {
                expected_zone_change_generation: Some(generation),
                location: Some(rv1::cast_source::Location::ExileObjectId(object_id)),
            }),
            cast_method: rv1::CastMethod::Permission as i32,
            casting_permission_id: Some(permission.group_id),
            ..Default::default()
        };
        engine
            .apply_scenario_command(
                0,
                &rv1::RuledCommand {
                    cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(
                        rv1::SubmitResolutionChoice {
                            decision: rv1::ResolutionChoiceDecision::CastSpell as i32,
                            spell_cast_announcement: Some(announcement),
                            ..Default::default()
                        },
                    )),
                },
            )
            .unwrap();
        engine
            .state
            .pending_spell_cast
            .as_ref()
            .unwrap()
            .transaction_id
    }

    #[test]
    fn winning_effect_stops_remaining_draw_and_life_tail_immediately() {
        let mut engine = GameEngine::new(
            tricerules_cards::registry::global(),
            507_001,
            &[0, 1],
            20,
            None,
            true,
        )
        .unwrap();
        let source = add_battlefield_object(&mut engine, 0, "grizzly_bears");
        let top = triggered_item(source, 0);
        let hand_before = engine.state.players[0].hand.clone();
        let library_before = engine.state.players[0].library.clone();
        let count = hand_before.len() as u32;
        let effects = vec![
            SpellEffectKind::WinGameIf {
                condition: tricerules_card_model::GameCondition::CardsInHand {
                    players: tricerules_card_model::primitives::ConditionPlayerSet::Relative(
                        tricerules_card_model::primitives::RelativePlayerSet::Controller,
                    ),
                    min: Some(count),
                    max: Some(count),
                },
            },
            SpellEffectKind::Draw {
                who: tricerules_card_model::primitives::PlayerRecipient::Controller,
                count: Amount::Fixed(2),
            },
            SpellEffectKind::GainLife {
                amount: Amount::Fixed(3),

                who: tricerules_card_model::primitives::PlayerRecipient::Controller,
            },
        ]
        .into_iter()
        .map(|effect| ResolutionEffect {
            effect,
            targets: vec![],
            target_receipts: vec![],
            role_group_indices: vec![],
        })
        .collect();
        let mut events = Vec::new();
        engine
            .run_effect_list(&top, "winning tail fixture", effects, 0, &mut events)
            .unwrap();
        assert_eq!(engine.state.winner(), Some(0));
        assert_eq!(engine.state.players[0].hand, hand_before);
        assert_eq!(engine.state.players[0].library, library_before);
        assert_eq!(engine.state.players[0].life, 20);
        assert!(!events.iter().any(|event| matches!(
            event.ev,
            Some(rv1::ruled_event::Ev::LifeChanged(_))
                | Some(rv1::ruled_event::Ev::StackResolved(_))
        )));
    }

    #[test]
    fn terminal_command_preserves_winner_before_pending_loss_and_sba() {
        let mut engine = GameEngine::new(
            tricerules_cards::registry::global(),
            507_002,
            &[0, 1],
            20,
            None,
            true,
        )
        .unwrap();
        let source = add_battlefield_object(&mut engine, 0, "grizzly_bears");
        let returning = add_battlefield_object(&mut engine, 0, "grizzly_bears");
        move_object_to_zone(
            &mut engine.state,
            engine.registry,
            returning,
            Zone::Exile,
            None,
        )
        .unwrap();
        engine.state.pending_immediate_observer_actions.push(
            ImmediateObserverAction::ReturnExiledObject {
                exiled: TriggerObjectRef {
                    object_id: returning,
                    zone_change_generation: engine.state.zone_change_generation[&returning],
                    controller_at_event: 0,
                },
            },
        );
        let item = quantity_item(
            source,
            vec![SpellEffectKind::WinGameIf {
                condition: tricerules_card_model::GameCondition::CardsInHand {
                    players: tricerules_card_model::primitives::ConditionPlayerSet::Relative(
                        tricerules_card_model::primitives::RelativePlayerSet::Controller,
                    ),
                    min: None,
                    max: None,
                },
            }],
        );
        engine.state.stack.push(item);
        engine.state.passes_since_stack_change = 1;
        engine.state.players[0].pending_library_loss = true;
        engine.state.players[0].life = 0;
        let player = engine.state.priority_player_id();
        let batch = engine
            .apply_scenario_command(
                player,
                &RuledCommand {
                    cmd: Some(rv1::ruled_command::Cmd::PassPriority(rv1::PassPriority {})),
                },
            )
            .unwrap();
        assert_eq!(engine.state.winner(), Some(0));
        assert!(!engine.state.players[0].has_lost);
        assert!(engine.state.players[0].pending_library_loss);
        assert!(batch.legal_by_player.is_empty());
        assert_eq!(engine.state.objects[&returning].zone, Zone::Exile);
        assert!(engine.state.pending_immediate_observer_actions.is_empty());
        assert!(!batch
            .events
            .iter()
            .any(|event| matches!(event.ev, Some(rv1::ruled_event::Ev::StackResolved(_)))));
        assert_eq!(
            batch
                .events
                .iter()
                .filter(|event| matches!(&event.ev,
                    Some(rv1::ruled_event::Ev::Log(log)) if log.text == "Game over. Winner: 0"
                ))
                .count(),
            1
        );
        let index = engine.state.command_index;
        assert!(engine
            .apply_scenario_command(
                player,
                &RuledCommand {
                    cmd: Some(rv1::ruled_command::Cmd::PassPriority(rv1::PassPriority {})),
                }
            )
            .is_err());
        assert_eq!(engine.state.command_index, index);
    }

    #[test]
    fn terminal_life_sweep_does_not_overwrite_winner_or_eliminate_players() {
        let mut engine = GameEngine::new(
            tricerules_cards::registry::global(),
            507_003,
            &[0, 1],
            20,
            None,
            true,
        )
        .unwrap();
        engine.state.outcome = Some(crate::state::GameOutcome::Winner(0));
        engine.state.players[0].life = 0;
        engine.sweep_life();
        assert_eq!(engine.state.winner(), Some(0));
        assert!(!engine.state.players[0].has_lost);
    }

    #[test]
    fn terminal_draw_preserves_committed_state_and_ignores_later_losses() {
        let mut engine = GameEngine::new(
            tricerules_cards::registry::global(),
            505_054,
            &[4, 9, 27],
            20,
            None,
            true,
        )
        .unwrap();
        engine.state.outcome = Some(crate::state::GameOutcome::Draw);
        engine.state.players[0].life = 0;
        engine.state.players[1].pending_library_loss = true;
        let before = engine.diagnostic_snapshot().unwrap();
        engine.commit_pending_library_losses();
        engine.sweep_life();
        let mut events = Vec::new();
        engine.apply_sbas(&mut events).unwrap();
        assert!(events.is_empty());
        assert_eq!(engine.diagnostic_snapshot().unwrap(), before);
    }

    #[test]
    fn terminal_sbas_preserve_lethal_permanent() {
        let mut engine = GameEngine::new(
            tricerules_cards::registry::global(),
            507_005,
            &[0, 1],
            20,
            None,
            true,
        )
        .unwrap();
        let creature = add_battlefield_object(&mut engine, 0, "grizzly_bears");
        engine.state.objects.get_mut(&creature).unwrap().damage = 2;
        engine.state.outcome = Some(crate::state::GameOutcome::Winner(0));
        let mut events = Vec::new();
        engine.apply_sbas(&mut events).unwrap();
        assert_eq!(engine.state.objects[&creature].zone, Zone::Battlefield);
        assert!(events.is_empty());
    }

    fn quantity_item(source: ObjectId, effects: Vec<SpellEffectKind>) -> StackItem {
        let mut item = triggered_item(source, 0);
        let mut ability = tricerules_cards::registry::global()
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
    fn restored_vow_chooser_does_not_reevaluate_its_entered_condition() {
        let mut engine = GameEngine::new(
            tricerules_cards::registry::global(),
            507_117,
            &[0, 1],
            20,
            None,
            true,
        )
        .expect("two-player engine");
        let creature = add_battlefield_object(&mut engine, 0, "dwarven_priest");
        let first_equipment = add_battlefield_object(&mut engine, 0, "bonesplitter");
        let second_equipment = add_battlefield_object(&mut engine, 0, "vulshok_morningstar");
        let mut item = triggered_item(80_117, 0);
        item.card_id = "vow_to_erebor".to_string();
        item.ability_text = None;
        item.source_permanent_id = None;
        item.is_triggered = false;
        item.triggered_ability = None;
        item.ability_index = None;
        item.targets = vec![StackTarget {
            object_id: creature,
            required_controller: None,
            group_index: 0,
            damage_amount: 0,
            kind: rv1::TargetRefKind::Permanent as i32,
            zone_change_generation: Some(0),
        }];

        let owners = engine
            .compiled_resolution_program_owners(&item)
            .expect("Vow Program owner lookup")
            .expect("Vow resolves through its authored Program");
        let program = engine
            .begin_program_run(&item, owners)
            .expect("Vow Program starts");
        engine.state.stack.push(item.clone());
        let mut initial_events = Vec::new();
        assert!(matches!(
            engine
                .run_program_with_previous(
                    &item,
                    program,
                    crate::state::EffectResult::default(),
                    &mut initial_events,
                )
                .expect("Vow reaches its Dwarf-gated Equipment chooser"),
            ResolutionProgress::Parked
        ));
        engine
            .publish_resolution_prompt_ids(&mut initial_events)
            .expect("initial Vow prompt receives an ID");
        let original_prompt_id = engine
            .state
            .pending_resolution
            .as_ref()
            .expect("the entered chooser is pending")
            .resolution_prompt_id
            .get();
        assert_ne!(original_prompt_id, 0);
        assert_eq!(
            engine
                .state
                .pending_resolution
                .as_ref()
                .unwrap()
                .presentation
                .candidates,
            [first_equipment, second_equipment]
        );
        assert!(!engine.state.objects[&creature].tapped);
        assert_eq!(engine.effective_power(creature), Some(4));

        // Direct state fixture: the target loses Dwarf while Vow's Program is parked at its
        // already-entered Equipment choice. Rebuilding that leaf must not rewind its Dwarf gate.
        engine.state.objects.get_mut(&creature).unwrap().card_id = "grizzly_bears".to_string();
        assert!(engine
            .characteristics(creature)
            .is_some_and(|characteristics| !characteristics.has_type("Dwarf")));

        let mut refreshed_events = Vec::new();
        assert!(engine
            .refresh_suspended_program_choice(&mut refreshed_events)
            .expect(
                "the already-entered Program chooser refreshes without reevaluating its condition"
            ));
        engine
            .publish_resolution_prompt_ids(&mut refreshed_events)
            .expect("refreshed Vow prompt receives a fresh ID");
        let refreshed = engine
            .state
            .pending_resolution
            .as_ref()
            .expect("the chooser remains active after its condition becomes false");
        let refreshed_prompt_id = refreshed.resolution_prompt_id.get();
        assert_ne!(refreshed_prompt_id, original_prompt_id);
        assert_eq!(
            refreshed.presentation.candidates,
            [first_equipment, second_equipment]
        );
        assert_eq!(engine.effective_power(creature), Some(4));

        assert!(engine
            .apply_command(
                0,
                &rv1::RuledCommand {
                    cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(
                        rv1::SubmitResolutionChoice {
                            resolution_prompt_id: original_prompt_id,
                            chosen_object_ids: vec![first_equipment],
                            ..Default::default()
                        },
                    )),
                },
            )
            .is_err());
        assert_eq!(
            engine
                .state
                .pending_resolution
                .as_ref()
                .unwrap()
                .resolution_prompt_id
                .get(),
            refreshed_prompt_id,
            "a stale answer leaves the current chooser intact"
        );

        engine
            .apply_command(
                0,
                &rv1::RuledCommand {
                    cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(
                        rv1::SubmitResolutionChoice {
                            resolution_prompt_id: refreshed_prompt_id,
                            chosen_object_ids: vec![first_equipment],
                            ..Default::default()
                        },
                    )),
                },
            )
            .expect("the refreshed offer resolves the selected Equipment and tail");
        assert!(engine.state.pending_resolution.is_none());
        assert_eq!(
            engine.state.objects[&first_equipment].attached_to,
            Some(crate::state::AttachmentRecipient::Object(creature))
        );
        assert_eq!(engine.state.objects[&second_equipment].attached_to, None);
    }

    #[test]
    fn hideaway_links_one_face_down_exile_and_does_not_log_random_bottom_order() {
        let mut engine = GameEngine::new(
            tricerules_cards::registry::global(),
            38109,
            &[0, 1],
            20,
            None,
            true,
        )
        .unwrap();
        let source = add_battlefield_object(&mut engine, 0, "nautiloid_ship");
        let source_generation = engine
            .state
            .zone_change_generation
            .get(&source)
            .copied()
            .unwrap_or(0);
        let mut item = triggered_item(source, source_generation);
        item.card_id = "nautiloid_ship".to_string();
        item.triggered_ability = Some(
            engine
                .registry
                .get("nautiloid_ship")
                .unwrap()
                .primary_face()
                .triggered_abilities[0]
                .clone(),
        );
        let ability = item.triggered_ability.as_mut().unwrap();
        ability.targeting = None;
        ability.effect = vec![
            SpellEffectKind::Hideaway {
                count: 4,
                linked_exile_id: tricerules_card_model::AbilityLinkId::new("exiled_cards").unwrap(),
            },
            SpellEffectKind::GainLife {
                amount: Amount::Fixed(2),

                who: tricerules_card_model::primitives::PlayerRecipient::Controller,
            },
        ];
        item.trigger_context.linked_exile_occurrence =
            Some(crate::state::LinkedExileOccurrence::NativeOrTokenBase);
        let looked_at = engine.state.players[0]
            .library
            .iter()
            .take(4)
            .copied()
            .collect::<Vec<_>>();
        let library_count_before = engine.state.players[0].library.len();
        let selected = looked_at[1];
        let remainder = looked_at
            .iter()
            .copied()
            .filter(|object_id| *object_id != selected)
            .collect::<std::collections::BTreeSet<_>>();

        let (effects, label) = engine.build_resolution_effects(&item);
        let mut events = Vec::new();
        engine
            .run_effect_list(&item, &label, effects, 0, &mut events)
            .unwrap();
        let pending = engine.state.pending_resolution.as_ref().unwrap();
        assert_eq!(pending.presentation.candidates, looked_at);
        assert_eq!(pending.presentation.min, 1);
        assert_eq!(pending.presentation.max, 1);
        assert_eq!(engine.state.players[0].life, 20);

        let batch = engine
            .submit_resolution_choice(
                0,
                &rv1::SubmitResolutionChoice {
                    chosen_object_ids: vec![selected],
                    ..Default::default()
                },
            )
            .unwrap();

        let exiled = &engine.state.objects[&selected];
        assert_eq!(engine.state.players[0].life, 22);
        assert_eq!(exiled.zone, Zone::Exile);
        assert!(exiled.face_down);
        assert_eq!(engine.state.zone_change_generation[&selected], 1);
        let linked = engine
            .state
            .linked_exile_records
            .values()
            .flatten()
            .find(|linked| linked.object_id == selected)
            .unwrap();
        assert_eq!(linked.zone_change_generation, 1);
        assert!(engine
            .state
            .hideaway_look_permissions
            .values()
            .any(|players| { players.contains(&0) }));
        let library = engine.state.players[0]
            .library
            .iter()
            .copied()
            .collect::<Vec<_>>();
        assert_eq!(library.len(), library_count_before - 1);
        assert_eq!(
            library[library.len() - 3..]
                .iter()
                .copied()
                .collect::<std::collections::BTreeSet<_>>(),
            remainder
        );
        assert!(batch.events.iter().any(|event| matches!(
            &event.ev,
            Some(rv1::ruled_event::Ev::PermanentMoved(moved))
                if moved.object_id == selected && moved.face_down && moved.card_id.is_empty()
        )));
        assert!(!batch.events.iter().any(|event| match &event.ev {
            Some(rv1::ruled_event::Ev::Log(log)) => {
                log.visible_to_player_id.is_none() && log.text.contains("randomly bottoms")
            }
            _ => false,
        }));
        let permission_key = engine
            .state
            .hideaway_look_permissions
            .keys()
            .find(|(_, object_id, generation)| *object_id == selected && *generation == 1)
            .cloned()
            .unwrap();
        assert_eq!(
            engine.state.hideaway_look_permissions[&permission_key],
            std::collections::BTreeSet::from([0])
        );
        let blank_copy = add_battlefield_object(&mut engine, 1, "grizzly_bears");
        let blank_values = engine.copiable_values_for(blank_copy).unwrap();
        let source_object = engine.state.objects.get_mut(&source).unwrap();
        source_object.copiable_values = Some(blank_values);
        source_object.copy_revision = source_object.copy_revision.saturating_add(1);
        assert!(engine
            .effective_triggered_abilities(source, "nautiloid_ship", 0)
            .is_empty());
        engine
            .state
            .objects
            .get_mut(&source)
            .unwrap()
            .base_controller = 1;
        engine.refresh_hideaway_look_permissions();
        assert_eq!(
            engine.state.hideaway_look_permissions[&permission_key],
            std::collections::BTreeSet::from([0, 1])
        );
        engine
            .state
            .objects
            .get_mut(&source)
            .unwrap()
            .base_controller = 0;
        engine.refresh_hideaway_look_permissions();
        assert_eq!(
            engine.state.hideaway_look_permissions[&permission_key],
            std::collections::BTreeSet::from([0, 1])
        );
    }

    #[test]
    fn declining_linked_cast_resumes_the_effect_tail_once() {
        use tricerules_card_model::primitives::LinkedExiledCardPlayKind;

        let mut engine = GameEngine::new(
            tricerules_cards::registry::global(),
            38110,
            &[0, 1],
            20,
            None,
            true,
        )
        .unwrap();
        let source = add_battlefield_object(&mut engine, 0, "nautiloid_ship");
        let source_generation = engine
            .state
            .zone_change_generation
            .get(&source)
            .copied()
            .unwrap_or(0);
        let mut item = triggered_item(source, source_generation);
        item.card_id = "nautiloid_ship".to_string();
        item.trigger_context.linked_exile_occurrence =
            Some(crate::state::LinkedExileOccurrence::NativeOrTokenBase);
        let mut ability = engine
            .registry
            .get("nautiloid_ship")
            .unwrap()
            .primary_face()
            .triggered_abilities[1]
            .clone();
        ability.effect = vec![
            SpellEffectKind::PlayLinkedExiledCard {
                producer_ability_id: tricerules_card_model::AbilityId::new(
                    "exile_target_graveyard",
                )
                .unwrap(),
                linked_exile_id: tricerules_card_model::AbilityLinkId::new("exiled_cards").unwrap(),
                filter: None,
                play_kind: LinkedExiledCardPlayKind::Cast,
                return_source_to_owner_hand_if_cast: true,
            },
            SpellEffectKind::GainLife {
                amount: Amount::Fixed(2),

                who: tricerules_card_model::primitives::PlayerRecipient::Controller,
            },
        ];
        item.triggered_ability = Some(ability);

        let linked_card = add_battlefield_object(&mut engine, 0, "opt");
        move_object_to_zone(
            &mut engine.state,
            engine.registry,
            linked_card,
            Zone::Exile,
            None,
        )
        .unwrap();
        engine
            .state
            .objects
            .get_mut(&linked_card)
            .unwrap()
            .face_down = true;
        let linked_generation = engine.state.zone_change_generation[&linked_card];
        let key = zones::linked_exile_key(
            &engine,
            &item,
            tricerules_card_model::AbilityLinkId::new("exiled_cards").unwrap(),
            tricerules_card_model::AbilityId::new("exile_target_graveyard").unwrap(),
        )
        .unwrap();
        engine.state.linked_exile_records.insert(
            key.clone(),
            vec![crate::state::LinkedExiledObject {
                object_id: linked_card,
                zone_change_generation: linked_generation,
            }],
        );
        engine.state.hideaway_look_permissions.insert(
            (key, linked_card, linked_generation),
            std::collections::BTreeSet::from([0]),
        );

        let (effects, label) = engine.build_resolution_effects(&item);
        engine
            .run_effect_list(&item, &label, effects, 0, &mut Vec::new())
            .unwrap();
        let choice = engine.state.pending_resolution.as_ref().unwrap();
        assert_eq!(choice.presentation.candidates, [linked_card]);
        engine
            .submit_resolution_choice(
                0,
                &rv1::SubmitResolutionChoice {
                    chosen_object_ids: vec![linked_card],
                    ..Default::default()
                },
            )
            .unwrap();
        let offer = engine.state.pending_resolution.as_ref().unwrap();
        assert_eq!(offer.presentation.choice_kind, rv1::ChoiceKind::SpecialCast);

        engine
            .submit_resolution_choice(
                0,
                &rv1::SubmitResolutionChoice {
                    decision: rv1::ResolutionChoiceDecision::Decline as i32,
                    ..Default::default()
                },
            )
            .unwrap();

        assert_eq!(engine.state.players[0].life, 22);
        assert!(engine.state.pending_resolution.is_none());

        let (effects, label) = engine.build_resolution_effects(&item);
        engine
            .run_effect_list(&item, &label, effects, 0, &mut Vec::new())
            .unwrap();
        engine
            .submit_resolution_choice(
                0,
                &rv1::SubmitResolutionChoice {
                    chosen_object_ids: vec![linked_card],
                    ..Default::default()
                },
            )
            .unwrap();
        let permission = engine
            .special_cast_permission(0)
            .expect("resolving linked-cast permission");
        let cast = rv1::CastSpell {
            cast_method: rv1::CastMethod::Permission as i32,
            casting_permission_id: Some(permission.group_id),
            source: Some(rv1::CastSource {
                expected_zone_change_generation: Some(linked_generation),
                location: Some(rv1::cast_source::Location::ExileObjectId(linked_card)),
            }),
            ..Default::default()
        };
        engine
            .submit_resolution_choice(
                0,
                &rv1::SubmitResolutionChoice {
                    decision: rv1::ResolutionChoiceDecision::CastSpell as i32,
                    cast_spell: Some(cast),
                    ..Default::default()
                },
            )
            .unwrap();

        assert_eq!(engine.state.players[0].life, 24);
        assert_eq!(engine.state.objects[&source].zone, Zone::Hand);
        assert_eq!(engine.state.objects[&linked_card].zone, Zone::Stack);
        assert!(engine.state.pending_resolution.is_none());
    }

    #[test]
    fn linked_cast_does_not_return_a_source_that_left_during_cast_payment() {
        use tricerules_card_model::primitives::LinkedExiledCardPlayKind;

        let mut engine = GameEngine::new(
            tricerules_cards::registry::global(),
            38112,
            &[0, 1],
            20,
            None,
            true,
        )
        .unwrap();
        let source = add_battlefield_object(&mut engine, 0, "nautiloid_ship");
        let source_generation = engine
            .state
            .zone_change_generation
            .get(&source)
            .copied()
            .unwrap_or(0);
        let mut item = triggered_item(source, source_generation);
        item.card_id = "nautiloid_ship".to_string();
        item.trigger_context.linked_exile_occurrence =
            Some(crate::state::LinkedExileOccurrence::NativeOrTokenBase);
        let mut ability = engine
            .registry
            .get("nautiloid_ship")
            .unwrap()
            .primary_face()
            .triggered_abilities[1]
            .clone();
        ability.effect = vec![SpellEffectKind::PlayLinkedExiledCard {
            producer_ability_id: tricerules_card_model::AbilityId::new("exile_target_graveyard")
                .unwrap(),
            linked_exile_id: tricerules_card_model::AbilityLinkId::new("exiled_cards").unwrap(),
            filter: None,
            play_kind: LinkedExiledCardPlayKind::Cast,
            return_source_to_owner_hand_if_cast: true,
        }];
        item.triggered_ability = Some(ability);

        let linked_card = add_battlefield_object(&mut engine, 0, "opt");
        move_object_to_zone(
            &mut engine.state,
            engine.registry,
            linked_card,
            Zone::Exile,
            None,
        )
        .unwrap();
        engine
            .state
            .objects
            .get_mut(&linked_card)
            .unwrap()
            .face_down = true;
        let linked_generation = engine.state.zone_change_generation[&linked_card];
        let key = zones::linked_exile_key(
            &engine,
            &item,
            tricerules_card_model::AbilityLinkId::new("exiled_cards").unwrap(),
            tricerules_card_model::AbilityId::new("exile_target_graveyard").unwrap(),
        )
        .unwrap();
        engine.state.linked_exile_records.insert(
            key.clone(),
            vec![crate::state::LinkedExiledObject {
                object_id: linked_card,
                zone_change_generation: linked_generation,
            }],
        );
        engine.state.hideaway_look_permissions.insert(
            (key, linked_card, linked_generation),
            std::collections::BTreeSet::from([0]),
        );

        let (effects, label) = engine.build_resolution_effects(&item);
        engine
            .run_effect_list(&item, &label, effects, 0, &mut Vec::new())
            .unwrap();
        engine
            .submit_resolution_choice(
                0,
                &rv1::SubmitResolutionChoice {
                    chosen_object_ids: vec![linked_card],
                    ..Default::default()
                },
            )
            .unwrap();
        let permission = engine
            .special_cast_permission(0)
            .expect("resolving linked-cast permission");
        engine
            .submit_resolution_choice(
                0,
                &rv1::SubmitResolutionChoice {
                    decision: rv1::ResolutionChoiceDecision::CastSpell as i32,
                    spell_cast_announcement: Some(rv1::SpellCastAnnouncement {
                        cast_method: rv1::CastMethod::Permission as i32,
                        casting_permission_id: Some(permission.group_id),
                        source: Some(rv1::CastSource {
                            expected_zone_change_generation: Some(linked_generation),
                            location: Some(rv1::cast_source::Location::ExileObjectId(linked_card)),
                        }),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
            )
            .unwrap();

        let transaction_id = engine
            .state
            .pending_spell_cast
            .as_ref()
            .unwrap()
            .transaction_id;
        move_object_to_zone(
            &mut engine.state,
            engine.registry,
            source,
            Zone::Graveyard,
            None,
        )
        .unwrap();
        let departed_generation = engine.state.zone_change_generation[&source];
        assert!(departed_generation > source_generation);

        engine
            .commit_spell_cast(
                0,
                &rv1::CommitSpellCast {
                    transaction_id,
                    ..Default::default()
                },
            )
            .unwrap();

        assert_eq!(engine.state.objects[&source].zone, Zone::Graveyard);
        assert_eq!(
            engine.state.zone_change_generation[&source],
            departed_generation
        );
        assert_eq!(engine.state.objects[&linked_card].zone, Zone::Stack);
        assert!(engine.state.pending_resolution.is_none());
    }

    #[test]
    fn linked_play_can_play_a_land_only_on_its_controller_turn_and_resumes_after_entry() {
        use tricerules_card_model::primitives::LinkedExiledCardPlayKind;

        let mut engine = GameEngine::new(
            tricerules_cards::registry::global(),
            38111,
            &[0, 1],
            20,
            None,
            true,
        )
        .unwrap();
        let source = add_battlefield_object(&mut engine, 0, "nautiloid_ship");
        let source_generation = engine
            .state
            .zone_change_generation
            .get(&source)
            .copied()
            .unwrap_or(0);
        let mut item = triggered_item(source, source_generation);
        item.card_id = "nautiloid_ship".to_string();
        item.trigger_context.linked_exile_occurrence =
            Some(crate::state::LinkedExileOccurrence::NativeOrTokenBase);
        let mut ability = engine
            .registry
            .get("nautiloid_ship")
            .unwrap()
            .primary_face()
            .triggered_abilities[1]
            .clone();
        ability.effect = vec![
            SpellEffectKind::PlayLinkedExiledCard {
                producer_ability_id: tricerules_card_model::AbilityId::new(
                    "exile_target_graveyard",
                )
                .unwrap(),
                linked_exile_id: tricerules_card_model::AbilityLinkId::new("exiled_cards").unwrap(),
                filter: None,
                play_kind: LinkedExiledCardPlayKind::Play,
                return_source_to_owner_hand_if_cast: false,
            },
            SpellEffectKind::GainLife {
                amount: Amount::Fixed(2),

                who: tricerules_card_model::primitives::PlayerRecipient::Controller,
            },
        ];
        item.triggered_ability = Some(ability);

        let land = add_battlefield_object(&mut engine, 0, "forest");
        move_object_to_zone(&mut engine.state, engine.registry, land, Zone::Exile, None).unwrap();
        let land_generation = engine.state.zone_change_generation[&land];
        let key = zones::linked_exile_key(
            &engine,
            &item,
            tricerules_card_model::AbilityLinkId::new("exiled_cards").unwrap(),
            tricerules_card_model::AbilityId::new("exile_target_graveyard").unwrap(),
        )
        .unwrap();
        engine.state.linked_exile_records.insert(
            key,
            vec![crate::state::LinkedExiledObject {
                object_id: land,
                zone_change_generation: land_generation,
            }],
        );

        let (effects, label) = engine.build_resolution_effects(&item);
        engine
            .run_effect_list(&item, &label, effects, 0, &mut Vec::new())
            .unwrap();
        engine
            .submit_resolution_choice(
                0,
                &rv1::SubmitResolutionChoice {
                    chosen_object_ids: vec![land],
                    ..Default::default()
                },
            )
            .unwrap();
        let prompt_id = publish_current_special_cast_prompt_id(&mut engine);
        assert_eq!(
            engine.special_cast_permission(0).unwrap().scope,
            ExilePlayPermissionScope::PlayCard
        );

        let play_land = rv1::RuledCommand {
            cmd: Some(rv1::ruled_command::Cmd::PlayLand(rv1::PlayLand {
                resolution_prompt_id: prompt_id,
                source: Some(rv1::LandSource {
                    location: Some(rv1::land_source::Location::ExileObjectId(land)),
                    expected_zone_change_generation: Some(land_generation),
                }),
                face_index: 0,
            })),
        };
        engine.state.active_player_idx = 1;
        assert!(engine.initial_response_batch().legal_by_player[&0]
            .zone_land_actions
            .is_empty());
        assert!(engine.apply_scenario_command(0, &play_land).is_err());
        assert!(engine.state.pending_resolution.is_some());

        engine.state.active_player_idx = 0;
        engine.state.lands_played_this_turn = 1;
        assert!(engine.initial_response_batch().legal_by_player[&0]
            .zone_land_actions
            .is_empty());
        assert!(engine.apply_scenario_command(0, &play_land).is_err());
        assert!(engine.state.pending_resolution.is_some());

        engine.state.lands_played_this_turn = 0;
        assert!(engine.initial_response_batch().legal_by_player[&0]
            .zone_land_actions
            .iter()
            .any(|action| action.object_id == land));
        engine.apply_scenario_command(0, &play_land).unwrap();
        assert_eq!(engine.state.objects[&land].zone, Zone::Battlefield);
        assert_eq!(engine.state.lands_played_this_turn, 1);
        assert_eq!(engine.state.players[0].life, 22);
        assert!(engine.state.pending_resolution.is_none());
    }

    #[test]
    fn pending_linked_land_play_rejects_other_land_sources_and_hides_their_actions() {
        use tricerules_card_model::primitives::LinkedExiledCardPlayKind;

        let mut engine = GameEngine::new(
            tricerules_cards::registry::global(),
            38_112,
            &[0, 1],
            20,
            None,
            true,
        )
        .unwrap();
        let source = add_battlefield_object(&mut engine, 0, "nautiloid_ship");
        let source_generation = engine
            .state
            .zone_change_generation
            .get(&source)
            .copied()
            .unwrap_or(0);
        let mut item = triggered_item(source, source_generation);
        item.card_id = "nautiloid_ship".to_string();
        item.trigger_context.linked_exile_occurrence =
            Some(crate::state::LinkedExileOccurrence::NativeOrTokenBase);
        let mut ability = engine
            .registry
            .get("nautiloid_ship")
            .unwrap()
            .primary_face()
            .triggered_abilities[1]
            .clone();
        ability.effect = vec![SpellEffectKind::PlayLinkedExiledCard {
            producer_ability_id: tricerules_card_model::AbilityId::new("exile_target_graveyard")
                .unwrap(),
            linked_exile_id: tricerules_card_model::AbilityLinkId::new("exiled_cards").unwrap(),
            filter: None,
            play_kind: LinkedExiledCardPlayKind::Play,
            return_source_to_owner_hand_if_cast: false,
        }];
        item.triggered_ability = Some(ability);

        let linked_land = add_battlefield_object(&mut engine, 0, "forest");
        move_object_to_zone(
            &mut engine.state,
            engine.registry,
            linked_land,
            Zone::Exile,
            None,
        )
        .unwrap();
        let linked_generation = engine.state.zone_change_generation[&linked_land];
        let key = zones::linked_exile_key(
            &engine,
            &item,
            tricerules_card_model::AbilityLinkId::new("exiled_cards").unwrap(),
            tricerules_card_model::AbilityId::new("exile_target_graveyard").unwrap(),
        )
        .unwrap();
        engine.state.linked_exile_records.insert(
            key,
            vec![crate::state::LinkedExiledObject {
                object_id: linked_land,
                zone_change_generation: linked_generation,
            }],
        );

        let hand_land = add_battlefield_object(&mut engine, 0, "forest");
        move_object_to_zone(
            &mut engine.state,
            engine.registry,
            hand_land,
            Zone::Hand,
            None,
        )
        .unwrap();
        let other_exile_land = add_battlefield_object(&mut engine, 0, "forest");
        move_object_to_zone(
            &mut engine.state,
            engine.registry,
            other_exile_land,
            Zone::Exile,
            None,
        )
        .unwrap();
        let other_exile_generation = engine.state.zone_change_generation[&other_exile_land];
        engine
            .grant_exile_play_permission(
                0,
                other_exile_land,
                "unrelated play permission",
                crate::state::ExilePlayPermissionGrant::printed(
                    ExilePlayPermissionScope::PlayCard,
                    false,
                ),
            )
            .unwrap();
        let graveyard_land = add_battlefield_object(&mut engine, 0, "forest");
        move_object_to_zone(
            &mut engine.state,
            engine.registry,
            graveyard_land,
            Zone::Graveyard,
            None,
        )
        .unwrap();
        engine.state.continuous_effects.push(ContinuousEffect {
            trigger_grant_origin: None,
            source_id: None,
            affected: AffectedScope::Player(0),
            kind: ContinuousEffectKind::PlayLandsFromOwnGraveyard,
            condition: None,
            duration: EffectDuration::Indefinite,
            timestamp: 1,
        });

        let (effects, label) = engine.build_resolution_effects(&item);
        engine
            .run_effect_list(&item, &label, effects, 0, &mut Vec::new())
            .unwrap();
        engine
            .submit_resolution_choice(
                0,
                &rv1::SubmitResolutionChoice {
                    chosen_object_ids: vec![linked_land],
                    ..Default::default()
                },
            )
            .unwrap();
        let prompt_id = publish_current_special_cast_prompt_id(&mut engine);

        let hand_play = rv1::RuledCommand {
            cmd: Some(rv1::ruled_command::Cmd::PlayLand(rv1::PlayLand {
                resolution_prompt_id: prompt_id,
                source: Some(rv1::LandSource {
                    location: Some(rv1::land_source::Location::HandIndex(0)),
                    expected_zone_change_generation: None,
                }),
                face_index: 0,
            })),
        };
        assert!(engine.apply_scenario_command(0, &hand_play).is_err());
        assert_eq!(engine.state.objects[&hand_land].zone, Zone::Hand);
        assert!(engine.state.pending_resolution.is_some());

        for (object_id, generation, location) in [
            (
                other_exile_land,
                other_exile_generation,
                rv1::land_source::Location::ExileObjectId(other_exile_land),
            ),
            (
                graveyard_land,
                engine.state.zone_change_generation[&graveyard_land],
                rv1::land_source::Location::GraveyardObjectId(graveyard_land),
            ),
        ] {
            let play = rv1::RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::PlayLand(rv1::PlayLand {
                    resolution_prompt_id: prompt_id,
                    source: Some(rv1::LandSource {
                        location: Some(location),
                        expected_zone_change_generation: Some(generation),
                    }),
                    face_index: 0,
                })),
            };
            assert!(
                engine.apply_scenario_command(0, &play).is_err(),
                "object {object_id}"
            );
            assert!(engine.state.pending_resolution.is_some());
        }

        let action_ids = engine.initial_response_batch().legal_by_player[&0]
            .zone_land_actions
            .iter()
            .map(|action| action.object_id)
            .collect::<Vec<_>>();
        assert_eq!(action_ids, [linked_land]);
    }

    #[test]
    fn linked_play_processes_every_card_in_the_linked_exile_cohort_before_resuming() {
        use tricerules_card_model::primitives::LinkedExiledCardPlayKind;

        let mut engine = GameEngine::new(
            tricerules_cards::registry::global(),
            38_113,
            &[0, 1],
            20,
            None,
            true,
        )
        .unwrap();
        let source = add_battlefield_object(&mut engine, 0, "nautiloid_ship");
        let source_generation = engine
            .state
            .zone_change_generation
            .get(&source)
            .copied()
            .unwrap_or(0);
        let mut item = triggered_item(source, source_generation);
        item.card_id = "nautiloid_ship".to_string();
        item.trigger_context.linked_exile_occurrence =
            Some(crate::state::LinkedExileOccurrence::NativeOrTokenBase);
        let mut ability = engine
            .registry
            .get("nautiloid_ship")
            .unwrap()
            .primary_face()
            .triggered_abilities[1]
            .clone();
        ability.effect = vec![
            SpellEffectKind::PlayLinkedExiledCard {
                producer_ability_id: tricerules_card_model::AbilityId::new(
                    "exile_target_graveyard",
                )
                .unwrap(),
                linked_exile_id: tricerules_card_model::AbilityLinkId::new("exiled_cards").unwrap(),
                filter: None,
                play_kind: LinkedExiledCardPlayKind::Play,
                return_source_to_owner_hand_if_cast: false,
            },
            SpellEffectKind::GainLife {
                amount: Amount::Fixed(2),

                who: tricerules_card_model::primitives::PlayerRecipient::Controller,
            },
        ];
        item.triggered_ability = Some(ability);

        let linked_lands = (0..2)
            .map(|_| {
                let land = add_battlefield_object(&mut engine, 0, "forest");
                move_object_to_zone(&mut engine.state, engine.registry, land, Zone::Exile, None)
                    .unwrap();
                land
            })
            .collect::<Vec<_>>();
        let generations = linked_lands
            .iter()
            .map(|object_id| engine.state.zone_change_generation[object_id])
            .collect::<Vec<_>>();
        let key = zones::linked_exile_key(
            &engine,
            &item,
            tricerules_card_model::AbilityLinkId::new("exiled_cards").unwrap(),
            tricerules_card_model::AbilityId::new("exile_target_graveyard").unwrap(),
        )
        .unwrap();
        engine.state.linked_exile_records.insert(
            key,
            linked_lands
                .iter()
                .copied()
                .zip(generations.iter().copied())
                .map(
                    |(object_id, zone_change_generation)| crate::state::LinkedExiledObject {
                        object_id,
                        zone_change_generation,
                    },
                )
                .collect(),
        );
        engine.state.continuous_effects.push(ContinuousEffect {
            trigger_grant_origin: None,
            source_id: None,
            affected: AffectedScope::Player(0),
            kind: ContinuousEffectKind::ExtraLandPlays(1),
            condition: None,
            duration: EffectDuration::Indefinite,
            timestamp: 1,
        });

        let (effects, label) = engine.build_resolution_effects(&item);
        engine
            .run_effect_list(&item, &label, effects, 0, &mut Vec::new())
            .unwrap();
        let initial_choice = engine.state.pending_resolution.as_ref().unwrap();
        assert_eq!(initial_choice.presentation.min, 0);
        assert_eq!(initial_choice.presentation.max, 1);
        assert_eq!(initial_choice.presentation.candidates, linked_lands);

        for (index, land) in linked_lands.iter().copied().enumerate() {
            engine
                .submit_resolution_choice(
                    0,
                    &rv1::SubmitResolutionChoice {
                        chosen_object_ids: vec![land],
                        ..Default::default()
                    },
                )
                .unwrap();
            let prompt_id = publish_current_special_cast_prompt_id(&mut engine);
            let play = rv1::RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::PlayLand(rv1::PlayLand {
                    resolution_prompt_id: prompt_id,
                    source: Some(rv1::LandSource {
                        location: Some(rv1::land_source::Location::ExileObjectId(land)),
                        expected_zone_change_generation: Some(generations[index]),
                    }),
                    face_index: 0,
                })),
            };
            engine.apply_scenario_command(0, &play).unwrap();
            if index == 0 {
                let next_choice = engine
                    .state
                    .pending_resolution
                    .as_ref()
                    .expect("the second linked card remains available before the effect tail");
                assert_eq!(
                    next_choice.presentation.choice_kind,
                    custom::ChoiceKind::LinkedExileCards
                );
                assert_eq!(next_choice.presentation.candidates, [linked_lands[1]]);
                assert_eq!(engine.state.players[0].life, 20);
            }
        }

        assert!(linked_lands
            .iter()
            .all(|land| engine.state.objects[land].zone == Zone::Battlefield));
        assert_eq!(engine.state.lands_played_this_turn, 2);
        assert_eq!(engine.state.players[0].life, 22);
        assert!(engine.state.pending_resolution.is_none());
    }

    #[test]
    fn observer_aura_return_restores_and_reconciles_linked_choice_after_concession() {
        use tricerules_card_model::primitives::LinkedExiledCardPlayKind;

        let mut engine = GameEngine::new(
            tricerules_cards::registry::global(),
            38_130,
            &[0, 1, 2],
            20,
            None,
            true,
        )
        .unwrap();
        let linked_land = exile_for_fixture(&mut engine, 0, "forest");
        let (_, _) =
            begin_linked_play_fixture(&mut engine, &[linked_land], LinkedExiledCardPlayKind::Play);
        assert!(matches!(
            engine
                .state
                .pending_resolution
                .as_ref()
                .map(|pending| &pending.continuation),
            Some(ResolutionContinuation::LinkedExileChoice { .. })
        ));

        let creature = add_battlefield_object(&mut engine, 0, "grizzly_bears");
        let departing_source = add_battlefield_object(&mut engine, 2, "grizzly_bears");
        let aura = exile_for_fixture(&mut engine, 1, "pacifism");
        observe_leave_and_return_aura(&mut engine, departing_source, aura);
        engine.concede_batch(2).unwrap();

        assert!(matches!(
            engine
                .state
                .pending_resolution
                .as_ref()
                .map(|pending| &pending.continuation),
            Some(ResolutionContinuation::AuraReturn { .. })
        ));
        assert!(engine.state.pending_observer_return_batch.is_some());
        engine
            .submit_resolution_choice(
                1,
                &rv1::SubmitResolutionChoice {
                    chosen_object_ids: vec![creature],
                    ..Default::default()
                },
            )
            .unwrap();

        let pending = engine
            .state
            .pending_resolution
            .as_ref()
            .expect("the original linked choice resumes after the Aura returns");
        assert_eq!(
            pending.presentation.choice_kind,
            rv1::ChoiceKind::LinkedExileCards
        );
        assert_eq!(pending.presentation.candidates, [linked_land]);
        assert!(engine.state.pending_linked_exile_play.is_some());

        engine
            .submit_resolution_choice(
                0,
                &rv1::SubmitResolutionChoice {
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(engine.state.players[0].life, 22);
        assert!(engine.state.pending_resolution.is_none());
        assert!(engine.state.pending_linked_exile_play.is_none());
    }

    #[test]
    fn observer_aura_return_restores_special_cast_choice_and_tail() {
        use tricerules_card_model::primitives::LinkedExiledCardPlayKind;

        let mut engine = GameEngine::new(
            tricerules_cards::registry::global(),
            38_131,
            &[0, 1, 2],
            20,
            None,
            true,
        )
        .unwrap();
        let linked_spell = exile_for_fixture(&mut engine, 0, "opt");
        begin_linked_play_fixture(&mut engine, &[linked_spell], LinkedExiledCardPlayKind::Cast);
        engine
            .submit_resolution_choice(
                0,
                &rv1::SubmitResolutionChoice {
                    chosen_object_ids: vec![linked_spell],
                    ..Default::default()
                },
            )
            .unwrap();
        assert!(matches!(
            engine
                .state
                .pending_resolution
                .as_ref()
                .map(|pending| &pending.continuation),
            Some(ResolutionContinuation::SpecialCast { .. })
        ));

        let creature = add_battlefield_object(&mut engine, 0, "grizzly_bears");
        let aura = exile_for_fixture(&mut engine, 1, "pacifism");
        queue_observer_aura_return(&mut engine, aura);
        let mut events = Vec::new();
        assert!(engine
            .drain_immediate_observer_actions(None, &mut events)
            .unwrap());
        assert!(matches!(
            engine
                .state
                .pending_resolution
                .as_ref()
                .map(|pending| &pending.continuation),
            Some(ResolutionContinuation::AuraReturn { .. })
        ));
        let restored_batch = engine
            .submit_resolution_choice(
                1,
                &rv1::SubmitResolutionChoice {
                    chosen_object_ids: vec![creature],
                    ..Default::default()
                },
            )
            .unwrap();

        assert!(
            restored_batch.events.iter().any(|event| {
                matches!(
                    event.ev.as_ref(),
                    Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(required))
                        if required.deciding_player_id == 0
                            && required.choice_kind == rv1::ChoiceKind::SpecialCast as i32
                            && required.candidate_object_ids == [linked_spell]
                )
            }),
            "the restored SpecialCast prompt must be republished to clients"
        );

        assert!(matches!(
            engine
                .state
                .pending_resolution
                .as_ref()
                .map(|pending| &pending.continuation),
            Some(ResolutionContinuation::SpecialCast { .. })
        ));
        engine
            .submit_resolution_choice(
                0,
                &rv1::SubmitResolutionChoice {
                    decision: rv1::ResolutionChoiceDecision::Decline as i32,
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(engine.state.players[0].life, 22);
        assert!(engine.state.pending_resolution.is_none());
        assert!(engine.state.pending_linked_exile_play.is_none());
    }

    #[test]
    fn authored_program_destroy_resumes_its_sentinel_after_immediate_aura_return() {
        let program_source = r#"(
            id: "authored_program_destroy_tail",
            name: "Authored Program Destroy Tail",
            face_id: "authored_program_destroy_tail",
            mana_cost: "{0}",
            types: ["Creature"],
            power: 1,
            toughness: 1,
            triggered_abilities: [(
                ability_id: "triggered_01",
                presentation: Fallback,
                trigger: WhenSelfEntersBattlefield,
                effect: [],
                program: Some((body: (steps: [
                    (id: "destroy_artifact", kind: Perform(Destroy(subject: Chosen(
                        (kind: AnyPermanent, permanent_types: [Artifact])
                    )))),
                    (id: "sentinel_life", kind: Perform(GainLife(amount: 1))),
                ], yields: []), target_bindings: [
                    (instruction_id: "destroy_artifact", role: Subject, target_group_index: 0),
                ])),
                modal: None,
                targeting: Some((groups: [(min: 1, max: 1,
                    prompt: "Choose target artifact", effect_indices: [])])),
                may: false,
                intervening_if: None,
                triggers_only_once: false,
                max_triggers_per_turn: None,
            )]
        )"#;
        let registry = Box::leak(Box::new(
            tricerules_card_model::CardRegistry::from_chunks_and_tokens(
                &[
                    program_source,
                    include_str!("../../../../tricerules-cards/data/banishing_light.ron"),
                    include_str!("../../../../tricerules-cards/data/pacifism.ron"),
                    include_str!("../../../../tricerules-cards/data/grizzly_bears.ron"),
                ],
                &[],
            )
            .expect("the exact targeted Destroy Program fixture compiles"),
        ));
        let decks = Some(vec![vec!["grizzly_bears".into(); 10]; 2]);
        let mut engine = GameEngine::new(registry, 38_132, &[0, 1], 20, decks, true)
            .expect("Destroy observer-boundary engine");
        engine.state.opening = None;
        engine.state.turn_step = TurnStep::Main1;
        engine.state.active_player_idx = 0;
        engine.state.priority_idx = 0;
        clear_observer_restore_starting_zones(&mut engine);

        let target = add_battlefield_object(&mut engine, 0, "banishing_light");
        let timestamp = engine.state.command_index;
        engine
            .state
            .continuous_effects
            .push(crate::state::ContinuousEffect {
                trigger_grant_origin: None,
                source_id: None,
                affected: crate::state::AffectedScope::Single(target),
                kind: tricerules_cards::ContinuousEffectKind::Layer4AddTypes(
                    tricerules_cards::TypeLineAddition {
                        card_types: vec![tricerules_cards::PermanentTypeFilter::Artifact],
                        ..Default::default()
                    },
                ),
                condition: None,
                duration: tricerules_cards::EffectDuration::Indefinite,
                timestamp,
            });
        assert!(engine
            .characteristics(target)
            .expect("Artifact target characteristics")
            .has_type("Artifact"));
        let creature = add_battlefield_object(&mut engine, 1, "grizzly_bears");
        let aura = exile_for_fixture(&mut engine, 1, "pacifism");
        // This synthetic immediate observer action isolates the instruction-boundary resume
        // contract; it does not assert that Destroy itself created the Aura-return event.
        queue_observer_aura_return(&mut engine, aura);

        let source = add_battlefield_object(&mut engine, 0, "authored_program_destroy_tail");
        let mut item =
            captured_printed_trigger_item(&engine, source, "authored_program_destroy_tail");
        item.targets.push(crate::state::StackTarget {
            object_id: target,
            group_index: 0,
            damage_amount: 0,
            kind: rv1::TargetRefKind::Permanent as i32,
            zone_change_generation: Some(
                engine
                    .state
                    .zone_change_generation
                    .get(&target)
                    .copied()
                    .unwrap_or(0),
            ),
            required_controller: None,
        });
        let owners = engine
            .compiled_resolution_program_owners(&item)
            .expect("resolve the test trigger's compiled Program owner")
            .expect("the test trigger owns a Program");
        let program = engine
            .begin_program_run(&item, owners)
            .expect("begin the targeted Destroy Program");
        let mut events = Vec::new();
        assert_eq!(
            engine
                .run_program_with_previous(
                    &item,
                    program,
                    crate::state::EffectResult::default(),
                    &mut events,
                )
                .expect("Destroy parks at the immediate Aura-return boundary"),
            ResolutionProgress::Parked
        );
        assert_eq!(engine.state.objects[&target].zone, Zone::Graveyard);
        assert_eq!(engine.state.players[0].life, 20);
        assert!(engine.state.pending_triggers.is_empty());
        let pending = engine
            .state
            .pending_resolution
            .as_ref()
            .expect("Aura return suspends between the Destroy and sentinel leaves");
        assert_eq!(pending.deciding_player, 1);
        assert_eq!(
            pending.presentation.choice_kind,
            rv1::ChoiceKind::AuraPermanent
        );
        assert!(pending.presentation.candidates.contains(&creature));
        let ResolutionContinuation::AuraReturn {
            stack: Some(stack), ..
        } = &pending.continuation
        else {
            panic!("the Aura-return choice retains the parked Program stack");
        };
        let saved_program = stack
            .program
            .as_ref()
            .expect("the in-flight Program cursor is retained");
        assert_eq!(saved_program.frames.len(), 1);
        assert_eq!(saved_program.frames[0].next_instruction, 1);
        assert!(saved_program.in_flight.is_none());

        let resumed = engine
            .submit_resolution_choice(
                1,
                &rv1::SubmitResolutionChoice {
                    chosen_object_ids: vec![creature],
                    ..Default::default()
                },
            )
            .expect("choose the legal Aura recipient and resume the next Program leaf");
        assert_eq!(engine.state.objects[&aura].zone, Zone::Battlefield);
        assert_eq!(
            engine.state.objects[&aura].attached_to,
            Some(AttachmentRecipient::Object(creature))
        );
        assert_eq!(engine.state.players[0].life, 21);
        assert!(engine.state.pending_resolution.is_none());
        assert!(engine.state.pending_observer_return_batch.is_none());
        assert!(engine.state.pending_triggers.is_empty());
        assert_eq!(
            resumed
                .events
                .iter()
                .filter(|event| matches!(
                    event.ev.as_ref(),
                    Some(rv1::ruled_event::Ev::LifeChanged(change)) if change.player_id == 0 && change.delta == 1
                ))
                .count(),
            1,
            "the post-Destroy sentinel runs exactly once after the Aura choice"
        );
    }

    #[test]
    fn linked_land_entry_replacement_resumes_the_remaining_cohort_and_tail_once() {
        use tricerules_card_model::primitives::LinkedExiledCardPlayKind;

        let mut engine = GameEngine::new(
            tricerules_cards::registry::global(),
            38_134,
            &[0, 1],
            20,
            None,
            true,
        )
        .unwrap();
        let replaced_land = exile_for_fixture(&mut engine, 0, "multiversal_passage");
        let remaining_land = exile_for_fixture(&mut engine, 0, "forest");
        let generations = [
            engine.state.zone_change_generation[&replaced_land],
            engine.state.zone_change_generation[&remaining_land],
        ];
        begin_linked_play_fixture(
            &mut engine,
            &[replaced_land, remaining_land],
            LinkedExiledCardPlayKind::Play,
        );
        engine
            .submit_resolution_choice(
                0,
                &rv1::SubmitResolutionChoice {
                    chosen_object_ids: vec![replaced_land],
                    ..Default::default()
                },
            )
            .unwrap();
        let prompt_id = publish_current_special_cast_prompt_id(&mut engine);
        engine
            .apply_scenario_command(
                0,
                &RuledCommand {
                    cmd: Some(rv1::ruled_command::Cmd::PlayLand(rv1::PlayLand {
                        resolution_prompt_id: prompt_id,
                        source: Some(rv1::LandSource {
                            location: Some(rv1::land_source::Location::ExileObjectId(
                                replaced_land,
                            )),
                            expected_zone_change_generation: Some(generations[0]),
                        }),
                        face_index: 0,
                    })),
                },
            )
            .unwrap();
        assert!(matches!(
            engine
                .state
                .pending_resolution
                .as_ref()
                .map(|pending| &pending.continuation),
            Some(ResolutionContinuation::EntryBasicLandType { .. })
        ));

        engine
            .submit_resolution_choice(
                0,
                &rv1::SubmitResolutionChoice {
                    decision: rv1::ResolutionChoiceDecision::SelectBranch as i32,
                    selected_branch_index: 4,
                    ..Default::default()
                },
            )
            .unwrap();
        assert!(matches!(
            engine
                .state
                .pending_resolution
                .as_ref()
                .map(|pending| &pending.continuation),
            Some(ResolutionContinuation::EntryCost { .. })
        ));
        engine
            .submit_resolution_choice(
                0,
                &rv1::SubmitResolutionChoice {
                    decision: rv1::ResolutionChoiceDecision::Decline as i32,
                    ..Default::default()
                },
            )
            .unwrap();

        let pending = engine
            .state
            .pending_resolution
            .as_ref()
            .expect("entry replacement resumes the next linked land offer");
        assert_eq!(
            pending.presentation.choice_kind,
            rv1::ChoiceKind::LinkedExileCards
        );
        assert_eq!(pending.presentation.candidates, [remaining_land]);
        assert_eq!(engine.state.objects[&replaced_land].zone, Zone::Battlefield);
        assert!(engine.state.objects[&replaced_land].tapped);
        assert!(engine
            .characteristics(replaced_land)
            .unwrap()
            .has_type("Forest"));
        assert_eq!(engine.state.lands_played_this_turn, 1);
        assert_eq!(engine.state.players[0].life, 20);

        engine
            .submit_resolution_choice(0, &rv1::SubmitResolutionChoice::default())
            .unwrap();
        assert_eq!(engine.state.players[0].life, 22);
        assert_eq!(engine.state.lands_played_this_turn, 1);
        assert!(engine.state.pending_resolution.is_none());
        assert!(engine.state.pending_linked_exile_play.is_none());
    }

    #[test]
    fn linked_land_entry_replacement_survives_observer_copy_choice_and_tail_once() {
        use tricerules_card_model::primitives::LinkedExiledCardPlayKind;

        let mut engine = GameEngine::new(
            tricerules_cards::registry::global(),
            38_135,
            &[0, 1, 2],
            20,
            None,
            true,
        )
        .unwrap();
        let passage = exile_for_fixture(&mut engine, 0, "multiversal_passage");
        let remaining_land = exile_for_fixture(&mut engine, 0, "forest");
        let passage_generation = engine.state.zone_change_generation[&passage];
        begin_linked_play_fixture(
            &mut engine,
            &[passage, remaining_land],
            LinkedExiledCardPlayKind::Play,
        );
        engine
            .submit_resolution_choice(
                0,
                &rv1::SubmitResolutionChoice {
                    chosen_object_ids: vec![passage],
                    ..Default::default()
                },
            )
            .unwrap();
        let prompt_id = publish_current_special_cast_prompt_id(&mut engine);
        engine
            .apply_scenario_command(
                0,
                &RuledCommand {
                    cmd: Some(rv1::ruled_command::Cmd::PlayLand(rv1::PlayLand {
                        resolution_prompt_id: prompt_id,
                        source: Some(rv1::LandSource {
                            location: Some(rv1::land_source::Location::ExileObjectId(passage)),
                            expected_zone_change_generation: Some(passage_generation),
                        }),
                        face_index: 0,
                    })),
                },
            )
            .unwrap();
        assert!(matches!(
            engine
                .state
                .pending_resolution
                .as_ref()
                .map(|pending| &pending.continuation),
            Some(ResolutionContinuation::EntryBasicLandType { .. })
        ));

        let artifact = add_battlefield_object(&mut engine, 0, "sol_ring");
        let departing_source = add_battlefield_object(&mut engine, 2, "grizzly_bears");
        let sculpting_steel = exile_for_fixture(&mut engine, 1, "sculpting_steel");
        let observer_forest = exile_for_fixture(&mut engine, 1, "forest");
        observe_leave_and_return_aura(&mut engine, departing_source, sculpting_steel);
        observe_leave_and_return_aura(&mut engine, departing_source, observer_forest);
        engine.concede_batch(2).unwrap();
        assert!(matches!(
            engine
                .state
                .pending_resolution
                .as_ref()
                .map(|pending| &pending.continuation),
            Some(ResolutionContinuation::EntryCopySource { .. })
        ));

        engine
            .submit_resolution_choice(
                1,
                &rv1::SubmitResolutionChoice {
                    chosen_object_ids: vec![artifact],
                    ..Default::default()
                },
            )
            .unwrap();
        let timestamp_order = engine
            .state
            .pending_resolution
            .as_ref()
            .expect("the observer entrants require an APNAP timestamp choice");
        assert_eq!(
            timestamp_order.presentation.choice_kind,
            rv1::ChoiceKind::SimultaneousEntryOrder
        );

        let restored_batch = engine
            .submit_resolution_choice(
                1,
                &rv1::SubmitResolutionChoice {
                    chosen_object_ids: vec![sculpting_steel, observer_forest],
                    ..Default::default()
                },
            )
            .unwrap();
        let land_type_prompt = restored_batch
            .events
            .iter()
            .find_map(|event| match &event.ev {
                Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(required))
                    if required.deciding_player_id == 0
                        && required.choice_kind == rv1::ChoiceKind::ResolutionBranch as i32 =>
                {
                    Some(required)
                }
                _ => None,
            });
        assert!(
            land_type_prompt.is_some(),
            "restoring the land entry choice must republish it to clients"
        );
        let land_type_prompt = land_type_prompt.unwrap();
        assert_eq!(land_type_prompt.source_object_id, passage);
        assert_eq!(land_type_prompt.resolution_branches.len(), 5);
        let Some(crate::engine::replacement::PendingReplacementEvent::BattlefieldEntry(entry)) =
            engine.state.pending_replacement_event.as_ref()
        else {
            panic!("the exact pending battlefield entry must be restored with its prompt");
        };
        assert_eq!(entry.event.object_id, passage);
        assert_eq!(engine.state.objects[&passage].zone, Zone::Exile);
        assert_eq!(
            engine.state.zone_change_generation[&passage],
            passage_generation
        );
        assert!(engine.state.pending_replacement_event.is_some());

        engine
            .submit_resolution_choice(
                0,
                &rv1::SubmitResolutionChoice {
                    decision: rv1::ResolutionChoiceDecision::SelectBranch as i32,
                    selected_branch_index: 4,
                    ..Default::default()
                },
            )
            .unwrap();
        assert!(matches!(
            engine
                .state
                .pending_resolution
                .as_ref()
                .map(|pending| &pending.continuation),
            Some(ResolutionContinuation::EntryCost { .. })
        ));
        engine
            .submit_resolution_choice(
                0,
                &rv1::SubmitResolutionChoice {
                    decision: rv1::ResolutionChoiceDecision::Decline as i32,
                    ..Default::default()
                },
            )
            .unwrap();

        assert_eq!(engine.state.objects[&passage].zone, Zone::Battlefield);
        assert_eq!(
            engine.state.zone_change_generation[&passage],
            passage_generation + 1
        );
        assert_eq!(
            engine.state.objects[&sculpting_steel].zone,
            Zone::Battlefield
        );
        assert_eq!(
            engine.state.objects[&observer_forest].zone,
            Zone::Battlefield
        );
        assert_eq!(engine.state.lands_played_this_turn, 1);
        let next_choice = engine
            .state
            .pending_resolution
            .as_ref()
            .expect("the remaining linked card is offered before the effect tail");
        assert_eq!(next_choice.presentation.candidates, [remaining_land]);
        assert_eq!(engine.state.players[0].life, 20);

        engine
            .submit_resolution_choice(0, &rv1::SubmitResolutionChoice::default())
            .unwrap();
        assert_eq!(engine.state.players[0].life, 22);
        assert_eq!(engine.state.lands_played_this_turn, 1);
        assert!(engine.state.pending_resolution.is_none());
        assert!(engine.state.pending_linked_exile_play.is_none());
    }

    #[test]
    fn linked_land_entry_cost_is_republished_after_observer_timestamp_choice() {
        use tricerules_card_model::primitives::LinkedExiledCardPlayKind;

        let mut engine = GameEngine::new(
            tricerules_cards::registry::global(),
            38_142,
            &[0, 1, 2, 3],
            20,
            None,
            true,
        )
        .unwrap();
        let passage = exile_for_fixture(&mut engine, 0, "multiversal_passage");
        let remaining_land = exile_for_fixture(&mut engine, 0, "forest");
        let passage_generation = engine.state.zone_change_generation[&passage];
        begin_linked_play_fixture(
            &mut engine,
            &[passage, remaining_land],
            LinkedExiledCardPlayKind::Play,
        );
        engine
            .submit_resolution_choice(
                0,
                &rv1::SubmitResolutionChoice {
                    chosen_object_ids: vec![passage],
                    ..Default::default()
                },
            )
            .unwrap();
        let prompt_id = publish_current_special_cast_prompt_id(&mut engine);
        engine
            .apply_scenario_command(
                0,
                &RuledCommand {
                    cmd: Some(rv1::ruled_command::Cmd::PlayLand(rv1::PlayLand {
                        resolution_prompt_id: prompt_id,
                        source: Some(rv1::LandSource {
                            location: Some(rv1::land_source::Location::ExileObjectId(passage)),
                            expected_zone_change_generation: Some(passage_generation),
                        }),
                        face_index: 0,
                    })),
                },
            )
            .unwrap();
        engine
            .submit_resolution_choice(
                0,
                &rv1::SubmitResolutionChoice {
                    decision: rv1::ResolutionChoiceDecision::SelectBranch as i32,
                    selected_branch_index: 4,
                    ..Default::default()
                },
            )
            .unwrap();
        assert!(matches!(
            engine
                .state
                .pending_resolution
                .as_ref()
                .map(|pending| &pending.continuation),
            Some(ResolutionContinuation::EntryCost { .. })
        ));

        let artifact = add_battlefield_object(&mut engine, 0, "sol_ring");
        let departing_source = add_battlefield_object(&mut engine, 1, "grizzly_bears");
        let observer_copy = exile_for_fixture(&mut engine, 0, "sculpting_steel");
        let observer_forest = exile_for_fixture(&mut engine, 0, "forest");
        observe_leave_and_return_aura(&mut engine, departing_source, observer_copy);
        observe_leave_and_return_aura(&mut engine, departing_source, observer_forest);
        engine.concede_batch(1).unwrap();

        engine
            .submit_resolution_choice(
                0,
                &rv1::SubmitResolutionChoice {
                    chosen_object_ids: vec![artifact],
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(
            engine
                .state
                .pending_resolution
                .as_ref()
                .unwrap()
                .presentation
                .choice_kind,
            rv1::ChoiceKind::SimultaneousEntryOrder
        );
        let restored = engine
            .submit_resolution_choice(
                0,
                &rv1::SubmitResolutionChoice {
                    chosen_object_ids: vec![observer_copy, observer_forest],
                    ..Default::default()
                },
            )
            .unwrap();
        assert!(restored.events.iter().any(|event| matches!(
            &event.ev,
            Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(required))
                if required.source_object_id == passage
                    && required.choice_kind == rv1::ChoiceKind::ResolutionBranch as i32
                    && required.resolution_branches.iter().any(|branch| branch.label == "Pay 2 life")
        )), "restored EntryCost must republish its current pay-or-decline options");
        assert!(matches!(
            engine
                .state
                .pending_resolution
                .as_ref()
                .map(|pending| &pending.continuation),
            Some(ResolutionContinuation::EntryCost { .. })
        ));
        engine
            .submit_resolution_choice(
                0,
                &rv1::SubmitResolutionChoice {
                    decision: rv1::ResolutionChoiceDecision::Decline as i32,
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(engine.state.objects[&passage].zone, Zone::Battlefield);
        let next = engine.state.pending_resolution.as_ref().unwrap();
        assert_eq!(next.presentation.candidates, [remaining_land]);
        assert_eq!(engine.state.players[0].life, 20);
        engine
            .submit_resolution_choice(0, &rv1::SubmitResolutionChoice::default())
            .unwrap();
        assert_eq!(engine.state.players[0].life, 22);
        assert!(engine.state.pending_resolution.is_none());
        assert!(engine.state.pending_linked_exile_play.is_none());
    }

    #[test]
    fn linked_land_entry_is_retired_if_its_owner_concedes_while_suspended() {
        use tricerules_card_model::primitives::LinkedExiledCardPlayKind;

        let mut engine = GameEngine::new(
            tricerules_cards::registry::global(),
            38_143,
            &[0, 1, 2, 3],
            20,
            None,
            true,
        )
        .unwrap();
        let passage = exile_for_fixture(&mut engine, 2, "multiversal_passage");
        let remaining_land = exile_for_fixture(&mut engine, 0, "forest");
        let passage_generation = engine.state.zone_change_generation[&passage];
        begin_linked_play_fixture(
            &mut engine,
            &[passage, remaining_land],
            LinkedExiledCardPlayKind::Play,
        );
        engine
            .submit_resolution_choice(
                0,
                &rv1::SubmitResolutionChoice {
                    chosen_object_ids: vec![passage],
                    ..Default::default()
                },
            )
            .unwrap();
        let prompt_id = publish_current_special_cast_prompt_id(&mut engine);
        engine
            .apply_scenario_command(
                0,
                &RuledCommand {
                    cmd: Some(rv1::ruled_command::Cmd::PlayLand(rv1::PlayLand {
                        resolution_prompt_id: prompt_id,
                        source: Some(rv1::LandSource {
                            location: Some(rv1::land_source::Location::ExileObjectId(passage)),
                            expected_zone_change_generation: Some(passage_generation),
                        }),
                        face_index: 0,
                    })),
                },
            )
            .unwrap();
        assert!(matches!(
            engine
                .state
                .pending_resolution
                .as_ref()
                .map(|pending| &pending.continuation),
            Some(ResolutionContinuation::EntryBasicLandType { .. })
        ));

        let artifact = add_battlefield_object(&mut engine, 0, "sol_ring");
        let departing_source = add_battlefield_object(&mut engine, 2, "grizzly_bears");
        let observer_copy = exile_for_fixture(&mut engine, 0, "sculpting_steel");
        let observer_forest = exile_for_fixture(&mut engine, 0, "forest");
        observe_leave_and_return_aura(&mut engine, departing_source, observer_copy);
        observe_leave_and_return_aura(&mut engine, departing_source, observer_forest);
        engine.concede_batch(2).unwrap();
        engine
            .submit_resolution_choice(
                0,
                &rv1::SubmitResolutionChoice {
                    chosen_object_ids: vec![artifact],
                    ..Default::default()
                },
            )
            .unwrap();
        let restored = engine
            .submit_resolution_choice(
                0,
                &rv1::SubmitResolutionChoice {
                    chosen_object_ids: vec![observer_copy, observer_forest],
                    ..Default::default()
                },
            )
            .unwrap();
        assert!(!engine.state.objects.contains_key(&passage));
        assert!(
            !restored.events.iter().any(|event| matches!(
                &event.ev,
                Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(required))
                    if required.source_object_id == passage
            )),
            "departure retires the absent entry instead of publishing an impossible mandatory choice"
        );
        let next = engine
            .state
            .pending_resolution
            .as_ref()
            .expect("the surviving linked card is offered after the lost entrant is retired");
        assert_eq!(
            next.presentation.choice_kind,
            rv1::ChoiceKind::LinkedExileCards
        );
        assert_eq!(next.presentation.candidates, [remaining_land]);
        engine
            .submit_resolution_choice(0, &rv1::SubmitResolutionChoice::default())
            .unwrap();
        assert_eq!(engine.state.players[0].life, 22);
        assert!(engine.state.pending_resolution.is_none());
        assert!(engine.state.pending_linked_exile_play.is_none());
    }

    #[test]
    fn linked_land_entry_owner_departure_during_suspended_observer_choice_advances_cohort() {
        use tricerules_card_model::primitives::LinkedExiledCardPlayKind;

        let mut engine = GameEngine::new(
            tricerules_cards::registry::global(),
            38_144,
            &[0, 1, 2, 3],
            20,
            None,
            true,
        )
        .unwrap();
        let passage = exile_for_fixture(&mut engine, 2, "multiversal_passage");
        let remaining_land = exile_for_fixture(&mut engine, 0, "forest");
        let passage_generation = engine.state.zone_change_generation[&passage];
        begin_linked_play_fixture(
            &mut engine,
            &[passage, remaining_land],
            LinkedExiledCardPlayKind::Play,
        );
        engine
            .submit_resolution_choice(
                0,
                &rv1::SubmitResolutionChoice {
                    chosen_object_ids: vec![passage],
                    ..Default::default()
                },
            )
            .unwrap();
        let prompt_id = publish_current_special_cast_prompt_id(&mut engine);
        engine
            .apply_scenario_command(
                0,
                &RuledCommand {
                    cmd: Some(rv1::ruled_command::Cmd::PlayLand(rv1::PlayLand {
                        resolution_prompt_id: prompt_id,
                        source: Some(rv1::LandSource {
                            location: Some(rv1::land_source::Location::ExileObjectId(passage)),
                            expected_zone_change_generation: Some(passage_generation),
                        }),
                        face_index: 0,
                    })),
                },
            )
            .unwrap();
        assert!(matches!(
            engine
                .state
                .pending_resolution
                .as_ref()
                .map(|pending| &pending.continuation),
            Some(ResolutionContinuation::EntryBasicLandType { .. })
        ));

        let artifact = add_battlefield_object(&mut engine, 0, "sol_ring");
        let observer_copy = exile_for_fixture(&mut engine, 0, "sculpting_steel");
        let observer_forest = exile_for_fixture(&mut engine, 0, "forest");
        queue_observer_aura_return(&mut engine, observer_copy);
        queue_observer_aura_return(&mut engine, observer_forest);
        let mut observer_events = Vec::new();
        assert!(engine
            .drain_immediate_observer_actions(None, &mut observer_events)
            .unwrap());
        assert!(matches!(
            engine
                .state
                .pending_resolution
                .as_ref()
                .map(|pending| &pending.continuation),
            Some(ResolutionContinuation::EntryCopySource { .. })
        ));
        assert!(matches!(
            engine
                .state
                .pending_observer_return_batch
                .as_ref()
                .and_then(|batch| batch.suspended_resolution.as_ref())
                .map(|pending| &pending.continuation),
            Some(ResolutionContinuation::EntryBasicLandType { .. })
        ));

        engine.concede_batch(2).unwrap();
        assert!(!engine.state.objects.contains_key(&passage));
        engine
            .submit_resolution_choice(
                0,
                &rv1::SubmitResolutionChoice {
                    chosen_object_ids: vec![artifact],
                    ..Default::default()
                },
            )
            .unwrap();
        let timestamp = engine.state.pending_resolution.as_ref().unwrap();
        assert_eq!(
            timestamp.presentation.choice_kind,
            rv1::ChoiceKind::SimultaneousEntryOrder
        );
        let restored = engine
            .submit_resolution_choice(
                0,
                &rv1::SubmitResolutionChoice {
                    chosen_object_ids: vec![observer_copy, observer_forest],
                    ..Default::default()
                },
            )
            .unwrap();
        assert!(!restored.events.iter().any(|event| matches!(
            &event.ev,
            Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(required))
                if required.source_object_id == passage
        )));
        let next = engine
            .state
            .pending_resolution
            .as_ref()
            .expect("the surviving linked card is offered after the lost entrant is retired");
        assert_eq!(
            next.presentation.choice_kind,
            rv1::ChoiceKind::LinkedExileCards
        );
        assert_eq!(next.presentation.candidates, [remaining_land]);
        engine
            .submit_resolution_choice(0, &rv1::SubmitResolutionChoice::default())
            .unwrap();
        assert_eq!(engine.state.players[0].life, 22);
        assert!(engine.state.pending_resolution.is_none());
        assert!(engine.state.pending_linked_exile_play.is_none());
    }

    #[test]
    fn departed_linked_land_is_retired_without_observer_return() {
        use tricerules_card_model::primitives::LinkedExiledCardPlayKind;

        let mut engine = GameEngine::new(
            tricerules_cards::registry::global(),
            38_145,
            &[0, 1, 2],
            20,
            None,
            true,
        )
        .unwrap();
        let passage = exile_for_fixture(&mut engine, 2, "multiversal_passage");
        let remaining_land = exile_for_fixture(&mut engine, 0, "forest");
        let passage_generation = engine.state.zone_change_generation[&passage];
        begin_linked_play_fixture(
            &mut engine,
            &[passage, remaining_land],
            LinkedExiledCardPlayKind::Play,
        );
        engine
            .submit_resolution_choice(
                0,
                &rv1::SubmitResolutionChoice {
                    chosen_object_ids: vec![passage],
                    ..Default::default()
                },
            )
            .unwrap();
        let prompt_id = publish_current_special_cast_prompt_id(&mut engine);
        engine
            .apply_scenario_command(
                0,
                &RuledCommand {
                    cmd: Some(rv1::ruled_command::Cmd::PlayLand(rv1::PlayLand {
                        resolution_prompt_id: prompt_id,
                        source: Some(rv1::LandSource {
                            location: Some(rv1::land_source::Location::ExileObjectId(passage)),
                            expected_zone_change_generation: Some(passage_generation),
                        }),
                        face_index: 0,
                    })),
                },
            )
            .unwrap();
        assert!(matches!(
            engine
                .state
                .pending_resolution
                .as_ref()
                .map(|pending| &pending.continuation),
            Some(ResolutionContinuation::EntryBasicLandType { .. })
        ));

        engine.concede_batch(2).unwrap();
        assert!(!engine.state.objects.contains_key(&passage));
        assert!(
            engine.state.pending_replacement_event.is_none(),
            "retiring the linked land must clear its stale entry transaction"
        );
        let pending = engine
            .state
            .pending_resolution
            .as_ref()
            .expect("the departed entrant is retired and the linked choice resumes");
        assert_eq!(
            pending.presentation.choice_kind,
            rv1::ChoiceKind::LinkedExileCards
        );
        assert_eq!(pending.presentation.candidates, [remaining_land]);
        engine
            .submit_resolution_choice(0, &rv1::SubmitResolutionChoice::default())
            .unwrap();
        assert_eq!(engine.state.players[0].life, 22);
        assert!(engine.state.pending_resolution.is_none());
        assert!(engine.state.pending_linked_exile_play.is_none());
    }

    #[test]
    fn retired_linked_land_entry_does_not_mask_later_departed_resolution() {
        use tricerules_card_model::primitives::LinkedExiledCardPlayKind;

        let mut engine = GameEngine::new(
            tricerules_cards::registry::global(),
            38_146,
            &[0, 1, 2, 3],
            20,
            None,
            true,
        )
        .unwrap();
        let passage = exile_for_fixture(&mut engine, 2, "multiversal_passage");
        let remaining_land = exile_for_fixture(&mut engine, 0, "forest");
        let passage_generation = engine.state.zone_change_generation[&passage];
        begin_linked_play_fixture(
            &mut engine,
            &[passage, remaining_land],
            LinkedExiledCardPlayKind::Play,
        );
        engine
            .submit_resolution_choice(
                0,
                &rv1::SubmitResolutionChoice {
                    chosen_object_ids: vec![passage],
                    ..Default::default()
                },
            )
            .unwrap();
        let prompt_id = publish_current_special_cast_prompt_id(&mut engine);
        engine
            .apply_scenario_command(
                0,
                &RuledCommand {
                    cmd: Some(rv1::ruled_command::Cmd::PlayLand(rv1::PlayLand {
                        resolution_prompt_id: prompt_id,
                        source: Some(rv1::LandSource {
                            location: Some(rv1::land_source::Location::ExileObjectId(passage)),
                            expected_zone_change_generation: Some(passage_generation),
                        }),
                        face_index: 0,
                    })),
                },
            )
            .unwrap();

        let retired_entry = engine
            .state
            .pending_replacement_event
            .clone()
            .expect("the selected Passage has an active entry transaction");
        engine.concede_batch(2).unwrap();
        engine
            .submit_resolution_choice(0, &rv1::SubmitResolutionChoice::default())
            .unwrap();
        assert!(engine.state.pending_linked_exile_play.is_none());
        assert!(engine.state.pending_replacement_event.is_none());

        // Reinstall the retired event to exercise the priority guard against stale serialized
        // state in addition to checking that normal retirement clears it.
        engine.state.pending_replacement_event = Some(retired_entry);
        let later_source = add_battlefield_object(&mut engine, 3, "nautiloid_ship");
        let later_source_generation = engine
            .state
            .zone_change_generation
            .get(&later_source)
            .copied()
            .unwrap_or(0);
        let mut later_item = triggered_item(later_source, later_source_generation);
        later_item.controller = 3;
        later_item.source_owner = Some(3);
        later_item.card_id = "nautiloid_ship".to_string();
        later_item.trigger_context.linked_exile_occurrence =
            Some(crate::state::LinkedExileOccurrence::NativeOrTokenBase);
        let mut hideaway = engine
            .registry
            .get("nautiloid_ship")
            .unwrap()
            .primary_face()
            .triggered_abilities[0]
            .clone();
        hideaway.targeting = None;
        hideaway.effect = vec![
            SpellEffectKind::Hideaway {
                count: 4,
                linked_exile_id: tricerules_card_model::AbilityLinkId::new("exiled_cards").unwrap(),
            },
            SpellEffectKind::GainLife {
                amount: Amount::Fixed(2),

                who: tricerules_card_model::primitives::PlayerRecipient::Controller,
            },
        ];
        later_item.triggered_ability = Some(hideaway);
        let (effects, label) = engine.build_resolution_effects(&later_item);
        engine
            .run_effect_list(&later_item, &label, effects, 0, &mut Vec::new())
            .unwrap();
        assert_eq!(
            engine
                .state
                .pending_resolution
                .as_ref()
                .unwrap()
                .deciding_player,
            3
        );
        assert_eq!(
            engine
                .state
                .pending_resolution
                .as_ref()
                .unwrap()
                .presentation
                .choice_kind,
            rv1::ChoiceKind::LibraryLook
        );

        engine.concede_batch(3).unwrap();

        assert!(engine.state.players[3].has_lost);
        assert!(engine.state.pending_resolution.is_none());
        assert!(engine.state.pending_linked_exile_play.is_none());
        assert!(engine.state.pending_replacement_event.is_none());
    }

    #[test]
    fn linked_cast_reservation_clears_when_exiled_card_owner_concedes() {
        use tricerules_card_model::primitives::LinkedExiledCardPlayKind;

        let mut engine = GameEngine::new(
            tricerules_cards::registry::global(),
            38_132,
            &[0, 1, 2],
            20,
            None,
            true,
        )
        .unwrap();
        let departed_owner_spell = exile_for_fixture(&mut engine, 1, "opt");
        let remaining_spell = exile_for_fixture(&mut engine, 0, "village_rites");
        begin_linked_play_fixture(
            &mut engine,
            &[departed_owner_spell, remaining_spell],
            LinkedExiledCardPlayKind::Cast,
        );
        let transaction_id = reserve_linked_spell_for_test(&mut engine, departed_owner_spell);
        assert!(engine.state.pending_spell_cast.is_some());
        assert!(engine.pending_spell_cast_internal.is_some());

        engine.concede_batch(1).unwrap();

        assert!(engine.state.players[1].has_lost);
        assert!(engine.state.pending_spell_cast.is_none());
        assert!(engine.pending_spell_cast_internal.is_none());
        assert!(!engine.state.objects.contains_key(&departed_owner_spell));
        let pending = engine
            .state
            .pending_resolution
            .as_ref()
            .expect("surviving chooser receives the remaining linked card");
        assert_eq!(
            pending.presentation.choice_kind,
            rv1::ChoiceKind::LinkedExileCards
        );
        assert_eq!(pending.presentation.candidates, [remaining_spell]);
        assert!(engine
            .apply_scenario_command(
                0,
                &rv1::RuledCommand {
                    cmd: Some(rv1::ruled_command::Cmd::CancelSpellCast(
                        rv1::CancelSpellCast { transaction_id },
                    )),
                },
            )
            .is_err());
        assert!(!engine.state.objects.contains_key(&departed_owner_spell));
    }

    #[test]
    fn linked_cast_reservation_returns_live_owner_card_when_caster_concedes() {
        use tricerules_card_model::primitives::LinkedExiledCardPlayKind;

        let mut engine = GameEngine::new(
            tricerules_cards::registry::global(),
            38_133,
            &[0, 1, 2],
            20,
            None,
            true,
        )
        .unwrap();
        let live_owner_spell = exile_for_fixture(&mut engine, 1, "opt");
        let original_generation = engine.state.zone_change_generation[&live_owner_spell];
        begin_linked_play_fixture(
            &mut engine,
            &[live_owner_spell],
            LinkedExiledCardPlayKind::Cast,
        );
        reserve_linked_spell_for_test(&mut engine, live_owner_spell);

        engine.concede_batch(0).unwrap();

        assert!(engine.state.players[0].has_lost);
        assert!(engine.state.pending_spell_cast.is_none());
        assert!(engine.pending_spell_cast_internal.is_none());
        assert_eq!(engine.state.objects[&live_owner_spell].zone, Zone::Exile);
        assert_eq!(
            engine.state.zone_change_generation[&live_owner_spell],
            original_generation
        );
        // The conceded caster is no longer a live recipient of the trailing life-gain effect.
        assert_eq!(engine.state.players[0].life, 20);
        assert!(engine.state.pending_resolution.is_none());
        assert!(engine.state.pending_linked_exile_play.is_none());
    }

    #[test]
    fn linked_cast_defers_buggy_return_until_later_cards_finish_the_cohort() {
        use tricerules_card_model::primitives::LinkedExiledCardPlayKind;

        let mut engine = GameEngine::new(
            tricerules_cards::registry::global(),
            38_114,
            &[0, 1],
            20,
            None,
            true,
        )
        .unwrap();
        let source = add_battlefield_object(&mut engine, 0, "smugglers_buggy");
        let source_generation = engine
            .state
            .zone_change_generation
            .get(&source)
            .copied()
            .unwrap_or(0);
        engine.state.continuous_effects.push(ContinuousEffect {
            trigger_grant_origin: None,
            source_id: None,
            affected: AffectedScope::Single(source),
            kind: ContinuousEffectKind::Layer4AddTypes(
                tricerules_card_model::primitives::TypeLineAddition {
                    card_types: vec![PermanentTypeFilter::Creature],
                    ..Default::default()
                },
            ),
            condition: None,
            duration: EffectDuration::Indefinite,
            timestamp: 1,
        });

        let mut item = triggered_item(source, source_generation);
        item.card_id = "smugglers_buggy".to_string();
        item.trigger_context.linked_exile_occurrence =
            Some(crate::state::LinkedExileOccurrence::NativeOrTokenBase);
        let mut ability = engine
            .registry
            .get("smugglers_buggy")
            .unwrap()
            .primary_face()
            .triggered_abilities[1]
            .clone();
        ability.effect = vec![
            SpellEffectKind::PlayLinkedExiledCard {
                producer_ability_id: tricerules_card_model::AbilityId::new("hideaway").unwrap(),
                linked_exile_id: tricerules_card_model::AbilityLinkId::new("exiled_cards").unwrap(),
                filter: None,
                play_kind: LinkedExiledCardPlayKind::Cast,
                return_source_to_owner_hand_if_cast: true,
            },
            SpellEffectKind::GainLife {
                amount: Amount::Fixed(2),

                who: tricerules_card_model::primitives::PlayerRecipient::Controller,
            },
        ];
        item.triggered_ability = Some(ability);

        let linked_cards = [
            add_battlefield_object(&mut engine, 0, "opt"),
            add_battlefield_object(&mut engine, 0, "village_rites"),
        ];
        for card in linked_cards {
            move_object_to_zone(&mut engine.state, engine.registry, card, Zone::Exile, None)
                .unwrap();
        }
        let generations = linked_cards.map(|card| engine.state.zone_change_generation[&card]);
        let key = zones::linked_exile_key(
            &engine,
            &item,
            tricerules_card_model::AbilityLinkId::new("exiled_cards").unwrap(),
            tricerules_card_model::AbilityId::new("hideaway").unwrap(),
        )
        .unwrap();
        engine.state.linked_exile_records.insert(
            key,
            linked_cards
                .into_iter()
                .zip(generations)
                .map(
                    |(object_id, zone_change_generation)| crate::state::LinkedExiledObject {
                        object_id,
                        zone_change_generation,
                    },
                )
                .collect(),
        );

        let (effects, label) = engine.build_resolution_effects(&item);
        let mut events = Vec::new();
        engine
            .run_effect_list(&item, &label, effects, 0, &mut events)
            .unwrap();
        engine.publish_resolution_prompt_ids(&mut events).unwrap();
        engine
            .apply_scenario_command(
                0,
                &RuledCommand {
                    cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(
                        rv1::SubmitResolutionChoice {
                            chosen_object_ids: vec![linked_cards[0]],
                            ..Default::default()
                        },
                    )),
                },
            )
            .unwrap();
        let first_permission = engine.special_cast_permission(0).unwrap();
        let announcement = rv1::SpellCastAnnouncement {
            source: Some(rv1::CastSource {
                expected_zone_change_generation: Some(generations[0]),
                location: Some(rv1::cast_source::Location::ExileObjectId(linked_cards[0])),
            }),
            cast_method: rv1::CastMethod::Permission as i32,
            casting_permission_id: Some(first_permission.group_id),
            ..Default::default()
        };
        engine
            .apply_scenario_command(
                0,
                &rv1::RuledCommand {
                    cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(
                        rv1::SubmitResolutionChoice {
                            decision: rv1::ResolutionChoiceDecision::CastSpell as i32,
                            spell_cast_announcement: Some(announcement),
                            ..Default::default()
                        },
                    )),
                },
            )
            .expect("announce the first linked spell before payment");
        let transaction_id = engine
            .state
            .pending_spell_cast
            .as_ref()
            .unwrap()
            .transaction_id;
        engine
            .apply_scenario_command(
                0,
                &rv1::RuledCommand {
                    cmd: Some(rv1::ruled_command::Cmd::CancelSpellCast(
                        rv1::CancelSpellCast { transaction_id },
                    )),
                },
            )
            .expect("cancel the first cast proposal without consuming the linked card");
        assert_eq!(engine.state.objects[&linked_cards[0]].zone, Zone::Exile);
        assert_eq!(
            engine.state.zone_change_generation[&linked_cards[0]],
            generations[0]
        );
        assert_eq!(
            engine
                .state
                .pending_resolution
                .as_ref()
                .unwrap()
                .presentation
                .candidates,
            [linked_cards[0]]
        );

        let first_permission = engine.special_cast_permission(0).unwrap();
        engine
            .submit_resolution_choice(
                0,
                &rv1::SubmitResolutionChoice {
                    decision: rv1::ResolutionChoiceDecision::CastSpell as i32,
                    cast_spell: Some(rv1::CastSpell {
                        source: Some(rv1::CastSource {
                            expected_zone_change_generation: Some(generations[0]),
                            location: Some(rv1::cast_source::Location::ExileObjectId(
                                linked_cards[0],
                            )),
                        }),
                        cast_method: rv1::CastMethod::Permission as i32,
                        casting_permission_id: Some(first_permission.group_id),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
            )
            .unwrap();

        let next = engine
            .state
            .pending_resolution
            .as_ref()
            .expect("Buggy remains available until every linked card has been handled");
        assert_eq!(
            next.presentation.choice_kind,
            rv1::ChoiceKind::LinkedExileCards
        );
        assert_eq!(next.presentation.candidates, [linked_cards[1]]);
        assert_eq!(engine.state.objects[&source].zone, Zone::Battlefield);
        assert_eq!(engine.state.players[0].life, 20);

        engine
            .submit_resolution_choice(
                0,
                &rv1::SubmitResolutionChoice {
                    chosen_object_ids: vec![linked_cards[1]],
                    ..Default::default()
                },
            )
            .unwrap();
        let second_permission = engine.special_cast_permission(0).unwrap();
        let cost = rv1::CostSelection {
            cost_index: 0,
            selection: Some(rv1::cost_selection::Selection::PermanentId(source)),
        };
        engine
            .submit_resolution_choice(
                0,
                &rv1::SubmitResolutionChoice {
                    decision: rv1::ResolutionChoiceDecision::CastSpell as i32,
                    cast_spell: Some(rv1::CastSpell {
                        source: Some(rv1::CastSource {
                            expected_zone_change_generation: Some(generations[1]),
                            location: Some(rv1::cast_source::Location::ExileObjectId(
                                linked_cards[1],
                            )),
                        }),
                        cost_selections: vec![cost],
                        cast_method: rv1::CastMethod::Permission as i32,
                        casting_permission_id: Some(second_permission.group_id),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
            )
            .expect("Buggy is a creature and can pay Village Rites' additional cost");

        assert_eq!(engine.state.objects[&source].zone, Zone::Graveyard);
        assert_eq!(engine.state.objects[&linked_cards[0]].zone, Zone::Stack);
        assert_eq!(engine.state.objects[&linked_cards[1]].zone, Zone::Stack);
        assert_eq!(engine.state.players[0].life, 22);
        assert!(engine.state.pending_resolution.is_none());
    }

    #[test]
    fn linked_play_prunes_a_card_owned_by_a_conceding_nonchooser() {
        use tricerules_card_model::primitives::LinkedExiledCardPlayKind;

        let mut engine = GameEngine::new(
            tricerules_cards::registry::global(),
            38_115,
            &[0, 1, 2],
            20,
            None,
            true,
        )
        .unwrap();
        let source = add_battlefield_object(&mut engine, 0, "nautiloid_ship");
        let source_generation = engine
            .state
            .zone_change_generation
            .get(&source)
            .copied()
            .unwrap_or(0);
        let mut item = triggered_item(source, source_generation);
        item.card_id = "nautiloid_ship".to_string();
        item.trigger_context.linked_exile_occurrence =
            Some(crate::state::LinkedExileOccurrence::NativeOrTokenBase);
        let mut ability = engine
            .registry
            .get("nautiloid_ship")
            .unwrap()
            .primary_face()
            .triggered_abilities[1]
            .clone();
        ability.effect = vec![
            SpellEffectKind::PlayLinkedExiledCard {
                producer_ability_id: tricerules_card_model::AbilityId::new(
                    "exile_target_graveyard",
                )
                .unwrap(),
                linked_exile_id: tricerules_card_model::AbilityLinkId::new("exiled_cards").unwrap(),
                filter: None,
                play_kind: LinkedExiledCardPlayKind::Play,
                return_source_to_owner_hand_if_cast: false,
            },
            SpellEffectKind::GainLife {
                amount: Amount::Fixed(2),

                who: tricerules_card_model::primitives::PlayerRecipient::Controller,
            },
        ];
        item.triggered_ability = Some(ability);

        let departed_owner_land = add_battlefield_object(&mut engine, 1, "forest");
        let remaining_land = add_battlefield_object(&mut engine, 0, "forest");
        for land in [departed_owner_land, remaining_land] {
            move_object_to_zone(&mut engine.state, engine.registry, land, Zone::Exile, None)
                .unwrap();
        }
        let key = zones::linked_exile_key(
            &engine,
            &item,
            tricerules_card_model::AbilityLinkId::new("exiled_cards").unwrap(),
            tricerules_card_model::AbilityId::new("exile_target_graveyard").unwrap(),
        )
        .unwrap();
        engine.state.linked_exile_records.insert(
            key,
            [departed_owner_land, remaining_land]
                .into_iter()
                .map(|object_id| crate::state::LinkedExiledObject {
                    object_id,
                    zone_change_generation: engine.state.zone_change_generation[&object_id],
                })
                .collect(),
        );

        let (effects, label) = engine.build_resolution_effects(&item);
        engine
            .run_effect_list(&item, &label, effects, 0, &mut Vec::new())
            .unwrap();
        assert_eq!(
            engine
                .state
                .pending_resolution
                .as_ref()
                .unwrap()
                .presentation
                .candidates,
            [departed_owner_land, remaining_land]
        );
        engine
            .apply_scenario_command(
                1,
                &rv1::RuledCommand {
                    cmd: Some(rv1::ruled_command::Cmd::Concede(rv1::Concede {})),
                },
            )
            .unwrap();

        assert!(engine.state.players[1].has_lost);
        assert!(!engine.state.objects.contains_key(&departed_owner_land));
        let pending = engine
            .state
            .pending_resolution
            .as_ref()
            .expect("the living trigger controller keeps choosing from the surviving cohort");
        assert_eq!(pending.presentation.candidates, [remaining_land]);
    }

    #[test]
    fn authoring_conditional_choices_freeze_and_resume_the_tail() {
        for inner in [
            SpellEffectKind::Discard {
                who: PlayerRecipient::Controller,
                quantity: tricerules_card_model::primitives::DiscardQuantity::Exact(1),
            },
            SpellEffectKind::LookChooseToHand {
                count: 1,
                filter: None,
                min: 1,
                max: 1,
                reveal: false,
                bottom_order: LibraryBottomOrder::Chosen,
            },
        ] {
            let mut engine = GameEngine::new(
                tricerules_cards::registry::global(),
                38101,
                &[0, 1],
                20,
                None,
                true,
            )
            .unwrap();
            let source = add_battlefield_object(&mut engine, 0, "grizzly_bears");
            let item = quantity_item(
                source,
                vec![
                    SpellEffectKind::Conditional {
                        condition: GameCondition::ActivePlayer {
                            players: RelativePlayerSet::Controller,
                        },
                        effect: Box::new(inner),
                    },
                    SpellEffectKind::GainLife {
                        amount: Amount::Fixed(2),

                        who: tricerules_card_model::primitives::PlayerRecipient::Controller,
                    },
                ],
            );
            let (effects, label) = engine.build_resolution_effects(&item);
            engine
                .run_effect_list(&item, &label, effects, 0, &mut Vec::new())
                .unwrap();
            let pending = engine.state.pending_resolution.as_ref().unwrap().clone();
            let chosen = pending.presentation.candidates[0];
            let answer = rv1::SubmitResolutionChoice {
                chosen_object_ids: vec![chosen],
                ..Default::default()
            };
            let before = format!("{:?}", engine.state.pending_resolution);
            let command_index = engine.state.command_index;
            assert!(engine.submit_resolution_choice(1, &answer).is_err());
            assert!(engine
                .submit_resolution_choice(
                    0,
                    &rv1::SubmitResolutionChoice {
                        chosen_object_ids: vec![chosen, chosen],
                        ..Default::default()
                    }
                )
                .is_err());
            let generation = engine
                .state
                .zone_change_generation
                .entry(chosen)
                .or_default();
            *generation += 1;
            assert!(engine.submit_resolution_choice(0, &answer).is_err());
            *engine
                .state
                .zone_change_generation
                .get_mut(&chosen)
                .unwrap() -= 1;
            assert_eq!(format!("{:?}", engine.state.pending_resolution), before);
            assert_eq!(engine.state.command_index, command_index);
            engine.state.active_player_idx = 1; // the already-selected condition is now false
            engine.submit_resolution_choice(0, &answer).unwrap();
            assert!(engine.state.pending_resolution.is_none());
            assert_eq!(engine.state.players[0].life, 22);
            assert!(engine.submit_resolution_choice(0, &answer).is_err());
            assert_eq!(engine.state.players[0].life, 22);
        }
    }

    #[test]
    fn authoring_false_conditional_does_not_inspect_private_library() {
        let mut engine = GameEngine::new(
            tricerules_cards::registry::global(),
            38102,
            &[0, 1],
            20,
            None,
            true,
        )
        .unwrap();
        let source = add_battlefield_object(&mut engine, 0, "grizzly_bears");
        let library = engine.state.players[0].library.clone();
        let item = quantity_item(
            source,
            vec![SpellEffectKind::Conditional {
                condition: GameCondition::ActivePlayer {
                    players: RelativePlayerSet::Opponents,
                },
                effect: Box::new(SpellEffectKind::LookChooseToHand {
                    count: 3,
                    filter: None,
                    min: 1,
                    max: 1,
                    reveal: false,
                    bottom_order: LibraryBottomOrder::Chosen,
                }),
            }],
        );
        let (effects, label) = engine.build_resolution_effects(&item);
        let mut events = Vec::new();
        engine
            .run_effect_list(&item, &label, effects, 0, &mut events)
            .unwrap();
        assert!(engine.state.pending_resolution.is_none());
        assert_eq!(engine.state.players[0].library, library);
        assert!(!events.iter().any(|event| match &event.ev {
            Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(_)) => true,
            Some(rv1::ruled_event::Ev::Log(log)) => log.text.contains("looks at"),
            _ => false,
        }));
    }

    #[test]
    fn authoring_draw_action_preserves_a_started_resolution_when_its_controller_leaves() {
        let mut engine = GameEngine::new(
            tricerules_cards::registry::global(),
            80004,
            &[0, 1, 2],
            20,
            Some(vec![vec!["forest".into(); 24]; 3]),
            true,
        )
        .unwrap();
        engine.registry = Box::leak(Box::new(CardRegistry::from_chunks_and_tokens(&[
            r#"(id: "forest", name: "Forest", face_id: "forest", types: ["Basic", "Land", "Forest"])"#,
            r#"(id: "grizzly_bears", name: "Grizzly Bears", face_id: "grizzly_bears", types: ["Creature"], power: 2, toughness: 2)"#,
            r#"(id: "replacement_fixture", name: "Replacement fixture", face_id: "replacement_fixture", types: ["Creature"], power: 2, toughness: 3, static_abilities: [(ability_id: "static_01", presentation: Fallback, definition: ReplaceControllerDrawWithLibraryChoice(kind: LookAtTopThree))])"#,
        ], &[]).unwrap()));
        engine.state.opening = None;
        engine.state.turn_step = TurnStep::Main1;
        let source = add_battlefield_object(&mut engine, 1, "grizzly_bears");
        add_battlefield_object(&mut engine, 0, "replacement_fixture");
        let mut item = quantity_item(
            source,
            vec![
                SpellEffectKind::Draw {
                    who: PlayerRecipient::AffectedPlayer,
                    count: Amount::Fixed(1),
                },
                SpellEffectKind::Draw {
                    who: PlayerRecipient::EachOpponent,
                    count: Amount::Fixed(1),
                },
            ],
        );
        item.controller = 1;
        item.source_owner = Some(1);
        item.trigger_context.affected_player = Some(0);
        let before: Vec<_> = engine.state.players.iter().map(|p| p.hand.len()).collect();
        let (effects, label) = engine.build_resolution_effects(&item);
        engine
            .run_effect_list(&item, &label, effects, 0, &mut Vec::new())
            .unwrap();
        engine.concede_batch(1).unwrap();
        assert!(
            engine.state.pending_resolution.is_some(),
            "the started replacement and resolution continue (CR 608.2m)"
        );
        for _ in 0..2 {
            let cards = engine
                .state
                .pending_resolution
                .as_ref()
                .unwrap()
                .presentation
                .candidates
                .clone();
            engine
                .submit_resolution_choice(
                    0,
                    &rv1::SubmitResolutionChoice {
                        chosen_object_ids: vec![cards[0]],
                        ..Default::default()
                    },
                )
                .unwrap();
            let rest = engine
                .state
                .pending_resolution
                .as_ref()
                .unwrap()
                .presentation
                .candidates
                .clone();
            engine
                .submit_resolution_choice(
                    0,
                    &rv1::SubmitResolutionChoice {
                        chosen_object_ids: rest,
                        ..Default::default()
                    },
                )
                .unwrap();
        }
        assert!(engine.state.pending_resolution.is_none());
        assert_eq!(engine.state.players[0].hand.len(), before[0] + 2);
        assert_eq!(engine.state.players[2].hand.len(), before[2] + 1);
    }

    #[test]
    fn observer_restore_fanatic_program_discard_keeps_accepted_prefix_and_runs_tail_once() {
        let mut engine = observer_restore_test_engine(38_211);
        let source = add_battlefield_object(&mut engine, 0, "fanatic_of_the_harrowing");
        let cards = [
            add_observer_restore_zone_card(&mut engine, 0, "forest", Zone::Hand),
            add_observer_restore_zone_card(&mut engine, 1, "island", Zone::Hand),
            add_observer_restore_zone_card(&mut engine, 2, "swamp", Zone::Hand),
        ];
        let draw_reserve = add_observer_restore_zone_card(&mut engine, 0, "island", Zone::Library);
        let draw_top = add_observer_restore_zone_card(&mut engine, 0, "forest", Zone::Library);
        let item = captured_printed_trigger_item(&engine, source, "fanatic_of_the_harrowing");
        let owners = engine
            .compiled_resolution_program_owners(&item)
            .expect("Fanatic Program owner lookup")
            .expect("actual Fanatic trigger has a compiled Program");
        let program = engine.begin_program_run(&item, owners).unwrap();
        let mut initial_events = Vec::new();
        assert_eq!(
            engine
                .run_program_with_previous(
                    &item,
                    program,
                    crate::state::EffectResult::default(),
                    &mut initial_events,
                )
                .unwrap(),
            ResolutionProgress::Parked
        );
        engine
            .publish_resolution_prompt_ids(&mut initial_events)
            .unwrap();
        let initial_offer = resolution_choice_offer(&initial_events);
        assert_eq!(initial_offer.deciding_player_id, 0);
        let initial_id = initial_offer.resolution_prompt_id;
        let accepted = engine
            .apply_command(0, &submit_choice_command(initial_id, vec![cards[0]]))
            .unwrap();
        let next_offer = resolution_choice_offer(&accepted.events);
        assert_eq!(next_offer.deciding_player_id, 1);
        assert_eq!(next_offer.candidate_object_ids, [cards[1]]);
        let old_id = next_offer.resolution_prompt_id;
        let (saved_frames, saved_in_flight, saved_slot_count) = {
            let pending = engine.state.pending_resolution.as_ref().unwrap();
            let ResolutionContinuation::PlayerSetDiscard { stack, discard } = &pending.continuation
            else {
                panic!("Fanatic remains parked on its shared discard transaction");
            };
            assert_eq!(discard.current, 1);
            assert_eq!(discard.selections, [vec![cards[0]]]);
            let program = stack
                .program
                .as_ref()
                .expect("Fanatic Program cursor is parked");
            (
                program
                    .frames
                    .iter()
                    .map(|frame| {
                        (
                            frame.path.clone(),
                            frame.next_instruction,
                            frame.yield_to.clone(),
                        )
                    })
                    .collect::<Vec<_>>(),
                program.in_flight.clone(),
                program.slots.len(),
            )
        };

        let suspended = engine.state.pending_resolution.take();
        let replacement = engine.state.pending_replacement_event.take();
        let mut restored_events = Vec::new();
        engine
            .restore_suspended_observer_resolution(suspended, replacement, &mut restored_events)
            .expect("restore accepted-prefix Fanatic discard");
        engine
            .publish_resolution_prompt_ids(&mut restored_events)
            .unwrap();
        let restored = resolution_choice_offer(&restored_events);
        assert_eq!(restored.deciding_player_id, 1);
        assert_eq!(restored.candidate_object_ids, [cards[1]]);
        assert_ne!(restored.resolution_prompt_id, old_id);
        let pending = engine.state.pending_resolution.as_ref().unwrap();
        let ResolutionContinuation::PlayerSetDiscard { stack, discard } = &pending.continuation
        else {
            panic!("restored Fanatic remains parked on its shared discard transaction");
        };
        assert_eq!(discard.current, 1);
        assert_eq!(discard.selections, [vec![cards[0]]]);
        let program = stack.program.as_ref().unwrap();
        assert_eq!(
            program
                .frames
                .iter()
                .map(|frame| {
                    (
                        frame.path.clone(),
                        frame.next_instruction,
                        frame.yield_to.clone(),
                    )
                })
                .collect::<Vec<_>>(),
            saved_frames
        );
        assert_eq!(program.in_flight, saved_in_flight);
        assert_eq!(program.slots.len(), saved_slot_count);
        assert!(engine
            .apply_command(1, &submit_choice_command(old_id, vec![cards[1]]))
            .is_err());
        let second = engine
            .apply_command(
                1,
                &submit_choice_command(restored.resolution_prompt_id, vec![cards[1]]),
            )
            .unwrap();
        let third = resolution_choice_offer(&second.events);
        assert_eq!(third.deciding_player_id, 2);
        engine
            .apply_command(
                2,
                &submit_choice_command(third.resolution_prompt_id, vec![cards[2]]),
            )
            .unwrap();

        for (player, card) in cards.into_iter().enumerate() {
            assert_eq!(engine.state.objects[&card].zone, Zone::Graveyard);
            assert!(engine.state.players[player].graveyard.contains(&card));
        }
        assert_eq!(engine.state.players[0].hand.len(), 1);
        assert_eq!(engine.state.players[0].hand.first(), Some(&draw_top));
        assert_eq!(engine.state.players[0].library.len(), 1);
        assert_eq!(engine.state.players[0].library.front(), Some(&draw_reserve));
        assert!(engine.state.pending_resolution.is_none());
    }

    fn observer_restore_player_set_discard_survives_controller_concession(
        card_id: &str,
        seed: u64,
        expects_program: bool,
    ) {
        let mut engine = observer_restore_test_engine(seed);
        engine.state.active_player_idx = 1;
        engine.state.priority_idx = 1;
        let source = add_battlefield_object(&mut engine, 0, card_id);
        let cards = [
            add_observer_restore_zone_card(&mut engine, 0, "forest", Zone::Hand),
            add_observer_restore_zone_card(&mut engine, 1, "island", Zone::Hand),
            add_observer_restore_zone_card(&mut engine, 2, "swamp", Zone::Hand),
        ];
        let controller_draws_before = engine.state.turn_history.current.player(0).cards_drawn;
        let generations_before = [
            engine
                .state
                .zone_change_generation
                .get(&cards[1])
                .copied()
                .unwrap_or_default(),
            engine
                .state
                .zone_change_generation
                .get(&cards[2])
                .copied()
                .unwrap_or_default(),
        ];
        let item = captured_printed_trigger_item(&engine, source, card_id);
        let expected_ability_owner = item.ability_program_owner();
        assert!(
            expected_ability_owner.is_some(),
            "the printed trigger has an owner"
        );
        assert_eq!(
            item.triggered_ability
                .as_ref()
                .is_some_and(|ability| ability.program.is_some()),
            expects_program
        );

        let mut initial_events = Vec::new();
        if let Some(owners) = engine
            .compiled_resolution_program_owners(&item)
            .expect("printed trigger Program lookup")
        {
            assert!(expects_program, "only Fanatic uses the Program path here");
            let program = engine
                .begin_program_run(&item, owners)
                .expect("begin the printed Program");
            assert_eq!(
                engine
                    .run_program_with_previous(
                        &item,
                        program,
                        crate::state::EffectResult::default(),
                        &mut initial_events,
                    )
                    .expect("run until the mandatory player-set choice"),
                ResolutionProgress::Parked
            );
        } else {
            assert!(!expects_program, "Burglar Rat uses its legacy effect list");
            let (effects, label) = engine.build_resolution_effects(&item);
            engine
                .run_effect_list(&item, &label, effects, 0, &mut initial_events)
                .expect("run until the mandatory player-set choice");
        }
        engine
            .publish_resolution_prompt_ids(&mut initial_events)
            .expect("publish the first private choice");
        let first = resolution_choice_offer(&initial_events);
        assert_eq!(first.deciding_player_id, 1);
        assert_eq!(first.candidate_object_ids, [cards[1]]);

        let accepted = engine
            .apply_command(
                1,
                &submit_choice_command(first.resolution_prompt_id, vec![cards[1]]),
            )
            .expect("P1 stages its discard");
        let next = resolution_choice_offer(&accepted.events);
        assert_eq!(next.deciding_player_id, 2);
        assert_eq!(next.candidate_object_ids, [cards[2]]);
        {
            let pending = engine.state.pending_resolution.as_ref().unwrap();
            assert_eq!(pending.deciding_player, 2);
            let ResolutionContinuation::PlayerSetDiscard { stack, discard } = &pending.continuation
            else {
                panic!("the printed trigger is parked on its player-set discard");
            };
            assert_eq!(stack.item.controller, 0);
            assert_eq!(stack.item.card_id, card_id);
            assert_eq!(stack.item.ability_program_owner(), expected_ability_owner);
            assert_eq!(stack.program.is_some(), expects_program);
            assert_eq!(discard.current, 1);
            assert_eq!(discard.selections, [vec![cards[1]]]);
        }

        let suspended_resolution = engine
            .state
            .pending_resolution
            .take()
            .expect("the saved choice is suspended in the observer-return batch");
        {
            let ResolutionContinuation::PlayerSetDiscard { stack, discard } =
                &suspended_resolution.continuation
            else {
                panic!("the saved continuation remains on its player-set discard");
            };
            assert_eq!(stack.item.controller, 0);
            assert_eq!(stack.item.ability_program_owner(), expected_ability_owner);
            assert_eq!(stack.program.is_some(), expects_program);
            assert_eq!(discard.current, 1);
            assert_eq!(discard.selections, [vec![cards[1]]]);
        }
        engine.state.pending_observer_return_batch =
            Some(crate::state::PendingObserverReturnBatch {
                ready: Vec::new(),
                remaining: Default::default(),
                resume_stack: None,
                suspended_resolution: Some(suspended_resolution),
                suspended_replacement_event: None,
            });

        let concession = engine
            .apply_command(
                0,
                &rv1::RuledCommand {
                    cmd: Some(rv1::ruled_command::Cmd::Concede(rv1::Concede {})),
                },
            )
            .expect("the saved-choice controller concedes through apply_command");

        assert!(engine.state.players[0].has_lost);
        assert!(!engine.state.objects.contains_key(&source));
        assert!(!engine.state.objects.contains_key(&cards[0]));
        let restored = resolution_choice_offer(&concession.events);
        assert_eq!(restored.deciding_player_id, 2);
        assert_eq!(restored.candidate_object_ids, [cards[2]]);
        assert!(engine.state.pending_observer_return_batch.is_none());
        {
            let pending = engine
                .state
                .pending_resolution
                .as_ref()
                .expect("concession restores the surviving discard offer");
            assert_eq!(pending.deciding_player, 2);
            let ResolutionContinuation::PlayerSetDiscard { stack, discard } = &pending.continuation
            else {
                panic!("the restored continuation remains on its player-set discard");
            };
            assert_eq!(stack.item.controller, 0, "retain the original controller");
            assert_eq!(stack.item.card_id, card_id);
            assert_eq!(stack.item.ability_program_owner(), expected_ability_owner);
            assert_eq!(stack.program.is_some(), expects_program);
            assert_eq!(discard.current, 1);
            assert_eq!(discard.selections, [vec![cards[1]]]);
            assert!(discard.choices.iter().all(|choice| choice.player != 0));
        }
        assert_eq!(engine.state.objects[&cards[1]].zone, Zone::Hand);
        assert_eq!(engine.state.objects[&cards[2]].zone, Zone::Hand);

        engine
            .apply_command(
                2,
                &submit_choice_command(restored.resolution_prompt_id, vec![cards[2]]),
            )
            .expect("P2 finishes the surviving discard cohort");

        for (offset, (player, card)) in [(1, cards[1]), (2, cards[2])].into_iter().enumerate() {
            assert_eq!(engine.state.objects[&card].zone, Zone::Graveyard);
            assert_eq!(
                engine.state.players[player]
                    .graveyard
                    .iter()
                    .filter(|&&object_id| object_id == card)
                    .count(),
                1,
                "each accepted discard commits exactly once"
            );
            assert_eq!(
                engine.state.zone_change_generation[&card],
                generations_before[offset] + 1,
                "each accepted card changes zones once"
            );
        }
        assert_eq!(
            engine.state.turn_history.current.player(0).cards_drawn,
            controller_draws_before,
            "the departed controller receives no draw"
        );
        assert!(engine.state.pending_resolution.is_none());
        assert!(engine.state.stack.is_empty());
        assert!(engine.state.pending_observer_return_batch.is_none());
    }

    #[test]
    fn observer_restore_fanatic_player_set_discard_survives_controller_concession() {
        observer_restore_player_set_discard_survives_controller_concession(
            "fanatic_of_the_harrowing",
            38_221,
            true,
        );
    }

    #[test]
    fn observer_restore_legacy_burglar_rat_player_set_discard_survives_controller_concession() {
        observer_restore_player_set_discard_survives_controller_concession(
            "burglar_rat",
            38_222,
            false,
        );
    }

    #[test]
    fn observer_restore_each_opponent_program_fixture_keeps_accepted_prefix() {
        // Runtime fixture: this admitted Program shape isolates EachOpponent from Fanatic's
        // actual EachPlayer card while exercising the same controller-owned Program invariant.
        let card = r#"(id: "observer_each_opponent_program_fixture", name: "Observer Each Opponent Program Fixture",
            face_id: "observer_each_opponent_program_fixture", types: ["Creature"], power: 1, toughness: 1,
            triggered_abilities: [(
                ability_id: "triggered_01", presentation: Fallback,
                trigger: WhenSelfEntersBattlefield, effect: [],
                program: Some((body: (steps: [
                    (id: "discard", bind: ["discarded"], input: None,
                        kind: Perform(Discard(who: EachOpponent, quantity: Exact(1)))),
                    (id: "tail", bind: [], input: None, kind: Perform(GainLife(amount: 2))),
                ], yields: []), target_bindings: [])),
            )])"#;
        let registry = Box::leak(Box::new(
            tricerules_card_model::CardRegistry::from_chunks_and_tokens(
                &[
                    r#"(id: "island", name: "Island", face_id: "island", types: ["Basic", "Land", "Island"])"#,
                    r#"(id: "swamp", name: "Swamp", face_id: "swamp", types: ["Basic", "Land", "Swamp"])"#,
                    card,
                ],
                &[],
            )
            .expect("EachOpponent Program runtime fixture compiles"),
        ));
        let mut engine = GameEngine::new(
            registry,
            38_212,
            &[0, 1, 2],
            20,
            Some(vec![vec!["island".into(); 20]; 3]),
            true,
        )
        .unwrap();
        engine.state.opening = None;
        engine.state.turn_step = TurnStep::Main1;
        engine.state.active_player_idx = 0;
        engine.state.priority_idx = 0;
        clear_observer_restore_starting_zones(&mut engine);
        let source =
            add_battlefield_object(&mut engine, 0, "observer_each_opponent_program_fixture");
        let cards = [
            add_observer_restore_zone_card(&mut engine, 1, "island", Zone::Hand),
            add_observer_restore_zone_card(&mut engine, 2, "swamp", Zone::Hand),
        ];
        let item = captured_printed_trigger_item(
            &engine,
            source,
            "observer_each_opponent_program_fixture",
        );
        let owners = engine
            .compiled_resolution_program_owners(&item)
            .unwrap()
            .unwrap();
        let program = engine.begin_program_run(&item, owners).unwrap();
        let mut initial_events = Vec::new();
        assert_eq!(
            engine
                .run_program_with_previous(
                    &item,
                    program,
                    crate::state::EffectResult::default(),
                    &mut initial_events
                )
                .unwrap(),
            ResolutionProgress::Parked
        );
        engine
            .publish_resolution_prompt_ids(&mut initial_events)
            .unwrap();
        let first = resolution_choice_offer(&initial_events);
        assert_eq!(first.deciding_player_id, 1);
        let accepted = engine
            .apply_command(
                1,
                &submit_choice_command(first.resolution_prompt_id, vec![cards[0]]),
            )
            .unwrap();
        let next = resolution_choice_offer(&accepted.events);
        assert_eq!(next.deciding_player_id, 2);
        assert_eq!(next.candidate_object_ids, [cards[1]]);
        let suspended = engine.state.pending_resolution.take();
        let replacement = engine.state.pending_replacement_event.take();
        let mut restored_events = Vec::new();
        engine
            .restore_suspended_observer_resolution(suspended, replacement, &mut restored_events)
            .expect("restore EachOpponent Program accepted prefix");
        engine
            .publish_resolution_prompt_ids(&mut restored_events)
            .unwrap();
        let restored = resolution_choice_offer(&restored_events);
        assert_eq!(restored.deciding_player_id, 2);
        assert_eq!(restored.candidate_object_ids, [cards[1]]);
        assert_ne!(restored.resolution_prompt_id, next.resolution_prompt_id);
        let pending = engine.state.pending_resolution.as_ref().unwrap();
        let ResolutionContinuation::PlayerSetDiscard { discard, .. } = &pending.continuation else {
            panic!("restored EachOpponent fixture remains on its shared discard transaction");
        };
        assert_eq!(discard.current, 1);
        assert_eq!(discard.selections, [vec![cards[0]]]);
        assert!(engine
            .apply_command(
                2,
                &submit_choice_command(next.resolution_prompt_id, vec![cards[1]])
            )
            .is_err());
        engine
            .apply_command(
                2,
                &submit_choice_command(restored.resolution_prompt_id, vec![cards[1]]),
            )
            .unwrap();
        assert_eq!(
            engine.state.players[0].life, 22,
            "the Program tail runs exactly once"
        );
        for (player, card) in [(1, cards[0]), (2, cards[1])] {
            assert_eq!(engine.state.objects[&card].zone, Zone::Graveyard);
            assert!(engine.state.players[player].graveyard.contains(&card));
        }
        assert!(engine.state.pending_resolution.is_none());
    }

    #[test]
    fn observer_restore_republishes_leng_order_without_losing_discard_destinations() {
        let mut engine = GameEngine::new(
            tricerules_cards::registry::global(),
            38_203,
            &[0, 1],
            20,
            Some(vec![vec!["forest".into(); 24]; 2]),
            true,
        )
        .unwrap();
        engine.state.opening = None;
        engine.state.turn_step = TurnStep::Main1;
        engine.state.active_player_idx = 0;
        engine.state.priority_idx = 0;
        let initial_hand_size = engine.state.players[0].hand.len();
        let spell = engine.state.players[0].hand[0];
        engine.state.objects.get_mut(&spell).unwrap().card_id = "faithless_looting".into();
        engine.state.players[0].mana_pool.red = 1;
        add_battlefield_object(&mut engine, 0, "library_of_leng");

        let cast = rv1::CastSpell {
            cast_method: rv1::CastMethod::Normal as i32,
            source: Some(rv1::CastSource {
                location: Some(rv1::cast_source::Location::HandIndex(0)),
                expected_zone_change_generation: Some(
                    engine
                        .state
                        .zone_change_generation
                        .get(&spell)
                        .copied()
                        .unwrap_or(0),
                ),
            }),
            ..Default::default()
        };
        engine.cast_spell(0, &cast).unwrap();
        engine.resolve_top_of_stack(&mut Vec::new()).unwrap();
        let discard_choice = engine.state.pending_resolution.as_ref().unwrap();
        assert_eq!(
            discard_choice.presentation.choice_kind,
            rv1::ChoiceKind::HandCards
        );
        assert_eq!(
            (
                discard_choice.presentation.min,
                discard_choice.presentation.max
            ),
            (2, 2)
        );
        let discarded = discard_choice.presentation.candidates[..2].to_vec();
        let generations: Vec<_> = discarded
            .iter()
            .map(|object_id| {
                engine
                    .state
                    .zone_change_generation
                    .get(object_id)
                    .copied()
                    .unwrap_or(0)
            })
            .collect();
        engine
            .submit_resolution_choice(
                0,
                &rv1::SubmitResolutionChoice {
                    chosen_object_ids: discarded.clone(),
                    ..Default::default()
                },
            )
            .unwrap();

        let mut original_order_offer = None;
        for _ in &discarded {
            let choice = engine.state.pending_resolution.as_ref().unwrap();
            assert_eq!(
                choice.presentation.choice_kind,
                rv1::ChoiceKind::PrivateReplacement
            );
            assert!(!choice.presentation.ordered);
            let result = engine
                .submit_resolution_choice(
                    0,
                    &rv1::SubmitResolutionChoice {
                        chosen_object_ids: vec![1],
                        ..Default::default()
                    },
                )
                .unwrap();
            if let Some(offer) = result.events.iter().find_map(|event| match &event.ev {
                Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(choice)) => {
                    Some(choice.clone())
                }
                _ => None,
            }) {
                original_order_offer = Some(offer);
            }
        }
        let original_order_offer = original_order_offer
            .expect("the original discard batch publishes a full ordering offer");
        let prior_batch = match engine.state.pending_replacement_event.as_ref().unwrap() {
            crate::engine::replacement::PendingReplacementEvent::Discard(batch) => batch,
            other => panic!("expected the saved Library of Leng batch, found {other:?}"),
        };
        assert_eq!(prior_batch.current, discarded.len());
        assert_eq!(
            prior_batch
                .cards
                .iter()
                .map(|card| card.object.object_id)
                .collect::<Vec<_>>(),
            discarded
        );
        assert!(prior_batch
            .cards
            .iter()
            .all(|card| card.destination == Some(Zone::Library)));
        assert!(prior_batch.library_orders.is_empty());
        let prior_order = engine.state.pending_resolution.as_ref().unwrap();
        assert!(prior_order.presentation.ordered);
        assert_eq!(prior_order.presentation.candidates, discarded);

        // The observer temporarily owns both pending values. Restoring must continue the same
        // discard batch and publish a fresh full ordering offer from its committed destinations.
        let suspended_resolution = engine.state.pending_resolution.take();
        let suspended_replacement_event = engine.state.pending_replacement_event.take();
        assert!(suspended_resolution.is_some());
        assert!(suspended_replacement_event.is_some());
        let mut restored_events = Vec::new();
        assert!(engine
            .restore_suspended_observer_resolution(
                suspended_resolution,
                suspended_replacement_event,
                &mut restored_events,
            )
            .unwrap());
        let restored_offer = restored_events
            .iter()
            .find_map(|event| match &event.ev {
                Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(choice)) => {
                    Some(choice.clone())
                }
                _ => None,
            })
            .expect("restoration republishes the full Library of Leng ordering offer");
        assert_eq!(
            restored_offer, original_order_offer,
            "restoration republishes the same serialized offer, including implicit-all candidate mask"
        );
        assert_eq!(
            restored_offer.choice_kind,
            rv1::ChoiceKind::PrivateReplacement as i32
        );
        assert_eq!(restored_offer.deciding_player_id, 0);
        assert_eq!(restored_offer.source_object_id, spell);
        assert_eq!(
            restored_offer.prompt_text,
            "Order discarded cards, top of library first."
        );
        assert_eq!(restored_offer.candidate_object_ids, discarded);
        assert_eq!(restored_offer.candidate_names, ["Forest", "Forest"]);
        assert_eq!((restored_offer.min, restored_offer.max), (2, 2));
        assert!(restored_offer.ordered);
        match engine.state.pending_replacement_event.as_ref().unwrap() {
            crate::engine::replacement::PendingReplacementEvent::Discard(batch) => {
                assert_eq!(batch.current, discarded.len());
                assert!(batch
                    .cards
                    .iter()
                    .all(|card| card.destination == Some(Zone::Library)));
                assert!(batch.library_orders.is_empty());
            }
            other => panic!("expected the restored Library of Leng batch, found {other:?}"),
        }

        let final_order = vec![discarded[1], discarded[0]];
        engine
            .submit_resolution_choice(
                0,
                &rv1::SubmitResolutionChoice {
                    chosen_object_ids: final_order.clone(),
                    ..Default::default()
                },
            )
            .unwrap();
        assert!(engine.state.pending_resolution.is_none());
        assert!(engine.state.pending_replacement_event.is_none());
        assert!(engine.state.stack.is_empty());
        assert_eq!(
            engine.state.players[0]
                .library
                .iter()
                .take(2)
                .copied()
                .collect::<Vec<_>>(),
            final_order
        );
        for (object_id, generation) in discarded.iter().zip(generations) {
            assert_eq!(engine.state.objects[object_id].zone, Zone::Library);
            assert_eq!(
                engine.state.zone_change_generation[object_id],
                generation + 1
            );
        }
        assert_eq!(engine.state.objects[&spell].zone, Zone::Graveyard);
        assert_eq!(
            engine.state.players[0]
                .graveyard
                .iter()
                .filter(|object_id| **object_id == spell)
                .count(),
            1
        );
        assert_eq!(engine.state.turn_history.current.player(0).cards_drawn, 2);
        assert_eq!(engine.state.players[0].hand.len(), initial_hand_size - 1);
    }

    #[test]
    fn authoring_scoped_result_keeps_you_and_opponents_relative_to_the_spell_controller() {
        use tricerules_card_model::primitives::{CardResultFilter, CardResultSource};
        let mut engine = GameEngine::new(
            tricerules_cards::registry::global(),
            38203,
            &[0, 1],
            20,
            Some(vec![vec!["forest".into(); 24]; 2]),
            true,
        )
        .unwrap();
        let source = add_battlefield_object(&mut engine, 0, "grizzly_bears");
        let item = quantity_item(source, Vec::new());
        let mut previous = EffectResult::default();
        for (player, count) in [(0, 1), (1, 2)] {
            let cards = engine.state.players[player].hand[..count].to_vec();
            for card in cards {
                let (entry, _) = zones::perform_discard_action(
                    &mut engine,
                    &mut Vec::new(),
                    player as i32,
                    card,
                    "fixture",
                    crate::state::DiscardCause::Effect,
                )
                .unwrap();
                previous.cards.push(entry);
            }
        }
        for (players, recipient, expected) in [
            (RelativePlayerSet::Controller, 1, 0),
            (RelativePlayerSet::Opponents, 1, 2),
            (RelativePlayerSet::All, 0, 1),
            (RelativePlayerSet::Opponents, 0, 0),
        ] {
            let count = Amount::Count(CountExpression::CardsMatchingResultForAffectedPlayer {
                filter: CardResultFilter {
                    source: CardResultSource::PreviousEffect,
                    action: CardResultAction::Discard,
                    players,
                    card_type: None,
                },
            });
            assert_eq!(
                engine.resolve_amount(
                    &count,
                    AmountContext::for_stack_item(&item, 0)
                        .with_affected_player(recipient)
                        .with_previous_effect_result(&previous)
                ),
                expected
            );
        }
    }

    #[test]
    fn authoring_accumulate_wisdom_selects_one_complete_library_operation() {
        let registry = Box::leak(Box::new(CardRegistry::from_chunks_and_tokens(&[
            r#"(id: "forest", name: "Forest", face_id: "forest", types: ["Basic", "Land", "Forest"])"#,
            r#"(id: "lesson_fixture", name: "Lesson fixture", face_id: "lesson_fixture", mana_cost: "{U}", types: ["Sorcery", "Lesson"])"#,
            r#"(id: "accumulate_fixture", name: "Accumulate fixture", face_id: "accumulate_fixture", mana_cost: "{U}", types: ["Sorcery", "Lesson"], spell_program: Some((body: (steps: [(id: "lesson_threshold", bind: [], input: None, kind: If(condition: GraveyardAggregate(owners: Controller, aggregate: CardCount, filter: Some((required_subtypes: ["Lesson"])), min: Some(3), max: None), yes: (steps: [(id: "take_three", bind: [], input: None, kind: Perform(LookChooseToHand(count: 3, min: 3, max: 3, reveal: false, bottom_order: Chosen)))], yields: []), no: (steps: [(id: "take_one", bind: [], input: None, kind: Perform(LookChooseToHand(count: 3, min: 1, max: 1, reveal: false, bottom_order: Chosen)))], yields: [])))], yields: []))))"#,
        ], &[]).unwrap()));
        for (lessons, owner) in [(2, 0), (3, 0), (3, 1)] {
            for size in 0..=3 {
                let mut engine = GameEngine::new(
                    tricerules_cards::registry::global(),
                    38104,
                    &[0, 1],
                    20,
                    Some(vec![vec!["forest".into(); 24]; 2]),
                    true,
                )
                .unwrap();
                engine.registry = registry;
                for _ in 0..lessons {
                    let oid = add_battlefield_object(&mut engine, owner, "lesson_fixture");
                    move_object_to_zone(
                        &mut engine.state,
                        engine.registry,
                        oid,
                        Zone::Graveyard,
                        None,
                    )
                    .unwrap();
                }
                let source = add_battlefield_object(&mut engine, 0, "accumulate_fixture");
                move_object_to_zone(
                    &mut engine.state,
                    engine.registry,
                    source,
                    Zone::Graveyard,
                    None,
                )
                .unwrap();
                let mut item = triggered_item(source, 0);
                item.id = source;
                item.card_id = "accumulate_fixture".into();
                item.ability_text = None;
                item.source_permanent_id = None;
                item.ability_index = None;
                item.is_triggered = false;
                let excess: Vec<_> = engine.state.players[0]
                    .library
                    .iter()
                    .skip(size)
                    .copied()
                    .collect();
                for oid in excess {
                    move_object_to_zone(&mut engine.state, engine.registry, oid, Zone::Exile, None)
                        .unwrap();
                }
                let cards: Vec<_> = engine.state.players[0].library.iter().copied().collect();
                let count = if lessons == 3 && owner == 0 {
                    size
                } else {
                    size.min(1)
                };
                let owners = engine
                    .compiled_resolution_program_owners(&item)
                    .expect("inspect Accumulate fixture Program owners")
                    .expect("the fixture uses an authored Program");
                let program = engine
                    .begin_program_run(&item, owners)
                    .expect("start the Accumulate fixture Program");
                engine
                    .run_program_with_previous(
                        &item,
                        program,
                        EffectResult::default(),
                        &mut Vec::new(),
                    )
                    .expect("run the ordered graveyard threshold into a single library operation");
                if size == 0 {
                    assert!(engine.state.pending_resolution.is_none());
                    assert!(engine.state.players[0].library.is_empty());
                    continue;
                }
                let pending = engine.state.pending_resolution.as_ref().unwrap();
                assert_eq!(pending.presentation.candidates, cards);
                assert_eq!(
                    (pending.presentation.min, pending.presentation.max),
                    (count as u32, count as u32)
                );
                assert_eq!(
                    pending.presentation.choice_kind,
                    rv1::ChoiceKind::LibraryLook
                );
                // Flipping the threshold after parking cannot select another library operation.
                let moved = add_battlefield_object(&mut engine, 0, "lesson_fixture");
                move_object_to_zone(
                    &mut engine.state,
                    engine.registry,
                    moved,
                    Zone::Graveyard,
                    None,
                )
                .unwrap();
                engine
                    .submit_resolution_choice(
                        0,
                        &rv1::SubmitResolutionChoice {
                            chosen_object_ids: cards[..count].to_vec(),
                            ..Default::default()
                        },
                    )
                    .unwrap();
                if size - count > 1 {
                    engine
                        .submit_resolution_choice(
                            0,
                            &rv1::SubmitResolutionChoice {
                                chosen_object_ids: cards[count..].iter().rev().copied().collect(),
                                ..Default::default()
                            },
                        )
                        .unwrap();
                }
                assert!(engine.state.pending_resolution.is_none());
                assert_eq!(engine.state.players[0].library.len(), size - count);
            }
        }
    }

    #[test]
    fn authoring_shoreline_threshold_is_evaluated_after_a_parked_draw_finishes() {
        let mut engine = GameEngine::new(
            tricerules_cards::registry::global(),
            38105,
            &[0, 1],
            20,
            Some(vec![vec!["forest".into(); 24]; 2]),
            true,
        )
        .unwrap();
        let source = add_battlefield_object(&mut engine, 0, "grizzly_bears");
        add_battlefield_object(&mut engine, 0, "thought_reflection");
        add_battlefield_object(&mut engine, 0, "alhammarrets_archive");
        for _ in 0..6 {
            let oid = add_battlefield_object(&mut engine, 0, "forest");
            move_object_to_zone(
                &mut engine.state,
                engine.registry,
                oid,
                Zone::Graveyard,
                None,
            )
            .unwrap();
        }
        let item = quantity_item(
            source,
            vec![
                SpellEffectKind::Draw {
                    count: Amount::Fixed(1),
                    who: PlayerRecipient::Controller,
                },
                SpellEffectKind::Conditional {
                    condition: GameCondition::GraveyardAggregate {
                        owners: RelativePlayerSet::Controller,
                        aggregate: GraveyardAggregate::CardCount,
                        filter: None,
                        min: None,
                        max: Some(6),
                    },
                    effect: Box::new(SpellEffectKind::Discard {
                        who: PlayerRecipient::Controller,
                        quantity: tricerules_card_model::primitives::DiscardQuantity::Exact(1),
                    }),
                },
                SpellEffectKind::GainLife {
                    amount: Amount::Fixed(2),

                    who: tricerules_card_model::primitives::PlayerRecipient::Controller,
                },
            ],
        );
        let (effects, label) = engine.build_resolution_effects(&item);
        engine
            .run_effect_list(&item, &label, effects, 0, &mut Vec::new())
            .unwrap();
        let selected = engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .presentation
            .candidates[0];
        let seventh = engine.state.players[0].hand[0];
        move_object_to_zone(
            &mut engine.state,
            engine.registry,
            seventh,
            Zone::Graveyard,
            None,
        )
        .unwrap();
        engine
            .submit_resolution_choice(
                0,
                &rv1::SubmitResolutionChoice {
                    chosen_object_ids: vec![selected],
                    ..Default::default()
                },
            )
            .unwrap();
        assert!(
            engine.state.pending_resolution.is_none(),
            "threshold is read after draw replacement completion"
        );
        assert_eq!(engine.state.players[0].graveyard.len(), 7);
        assert_eq!(engine.state.players[0].life, 24); // Archive doubles the single life-gain tail
    }

    #[test]
    fn archive_numeric_command_failure_restores_earlier_draw_and_publication_caches() {
        let mut engine = GameEngine::new(
            tricerules_cards::registry::global(),
            104_805,
            &[0, 1],
            20,
            None,
            true,
        )
        .unwrap();
        engine.state.opening = None;
        engine.state.turn_step = TurnStep::Main1;
        engine.state.active_player_idx = 0;
        engine.state.priority_idx = 0;
        let archive = add_battlefield_object(&mut engine, 0, "alhammarrets_archive");
        engine.state.stack.push(quantity_item(
            archive,
            vec![
                SpellEffectKind::Draw {
                    count: Amount::Fixed(1),
                    who: PlayerRecipient::Controller,
                },
                SpellEffectKind::GainLife {
                    amount: Amount::Fixed(u32::MAX),

                    who: tricerules_card_model::primitives::PlayerRecipient::Controller,
                },
            ],
        ));
        let pass = RuledCommand {
            cmd: Some(rv1::ruled_command::Cmd::PassPriority(rv1::PassPriority {})),
        };
        engine.apply_scenario_command(0, &pass).unwrap();
        engine.state.untapped_this_command.push(archive);
        let before = engine.diagnostic_snapshot().unwrap();
        let zones = engine.private_zone_cache.clone();
        let battlefield = engine.battlefield_view_cache.clone();
        let strike = engine.first_strike_step_pending_cache;
        assert!(matches!(
            engine.apply_scenario_command(1, &pass),
            Err(EngineError::LifeNumericRange(_))
        ));
        assert_eq!(engine.diagnostic_snapshot().unwrap(), before);
        assert!(engine.private_zone_cache == zones);
        assert!(engine.battlefield_view_cache == battlefield);
        assert_eq!(engine.first_strike_step_pending_cache, strike);
        assert!(engine.pending_spell_cast_internal.is_none());
        assert!(engine.pending_ability_activation_internal.is_none());
        // The rejected pass is reusable after removing the unsupported amount; no draw escaped.
        engine
            .state
            .stack
            .last_mut()
            .unwrap()
            .triggered_ability
            .as_mut()
            .unwrap()
            .effect[1] = SpellEffectKind::GainLife {
            amount: Amount::Fixed(1),

            who: tricerules_card_model::primitives::PlayerRecipient::Controller,
        };
        let hand = engine.state.players[0].hand.len();
        engine.apply_scenario_command(1, &pass).unwrap();
        assert_eq!(engine.state.players[0].hand.len(), hand + 2);
        assert_eq!(engine.state.players[0].life, 22);
    }

    #[test]
    fn archive_upstream_three_opponent_drain_sum_overflow_rolls_back_all_losses() {
        let mut engine = GameEngine::new(
            tricerules_cards::registry::global(),
            104_818,
            &[0, 1, 2, 3],
            20,
            None,
            true,
        )
        .unwrap();
        engine.state.opening = None;
        engine.state.turn_step = TurnStep::Main1;
        engine.state.active_player_idx = 0;
        engine.state.priority_idx = 0;
        let archive = add_battlefield_object(&mut engine, 0, "alhammarrets_archive");
        engine.state.stack.push(quantity_item(
            archive,
            vec![SpellEffectKind::EachOpponentLosesLifeYouGainEqual {
                amount: i32::MAX as u32,
            }],
        ));
        let pass = RuledCommand {
            cmd: Some(rv1::ruled_command::Cmd::PassPriority(rv1::PassPriority {})),
        };
        for player in [0, 1, 2] {
            engine.apply_scenario_command(player, &pass).unwrap();
        }
        let before = engine.diagnostic_snapshot().unwrap();
        assert!(matches!(
            engine.apply_scenario_command(3, &pass),
            Err(EngineError::LifeNumericRange("opponent drain sum"))
        ));
        assert_eq!(engine.diagnostic_snapshot().unwrap(), before);
    }

    #[test]
    fn blue_spell_shuffle_continues_tail_and_never_moves_an_off_stack_incarnation() {
        for on_stack in [true, false] {
            let mut engine = GameEngine::new(
                tricerules_cards::registry::global(),
                504_008,
                &[0, 1],
                20,
                None,
                true,
            )
            .unwrap();
            let source = add_battlefield_object(&mut engine, 0, "blue_suns_zenith");
            move_object_to_zone(
                &mut engine.state,
                engine.registry,
                source,
                if on_stack {
                    Zone::Stack
                } else {
                    Zone::Graveyard
                },
                None,
            )
            .unwrap();
            let generation = engine.state.zone_change_generation[&source];
            let mut top = triggered_item(source, 0);
            top.id = source;
            top.card_id = "blue_suns_zenith".into();
            top.controller = 1;
            top.is_triggered = false;
            top.ability_text = None;
            top.source_permanent_id = None;
            top.ability_index = None;
            let mut events = Vec::new();
            let effects = vec![
                SpellEffectKind::ShuffleResolvingSpellIntoOwnersLibrary,
                SpellEffectKind::GainLife {
                    amount: Amount::Fixed(2),

                    who: tricerules_card_model::primitives::PlayerRecipient::Controller,
                },
            ]
            .into_iter()
            .map(|effect| ResolutionEffect {
                effect,
                targets: vec![],
                target_receipts: vec![],
                role_group_indices: vec![],
            })
            .collect();
            engine
                .run_effect_list(&top, "Blue Sun fixture", effects, 0, &mut events)
                .unwrap();
            assert_eq!(
                engine.state.players[1].life, 22,
                "tail executes after the source leaves stack"
            );
            assert_eq!(
                engine.state.objects[&source].zone,
                if on_stack {
                    Zone::Library
                } else {
                    Zone::Graveyard
                }
            );
            assert_eq!(
                engine.state.zone_change_generation[&source],
                generation + u64::from(on_stack)
            );
            assert_eq!(
                events
                    .iter()
                    .filter(|event| matches!(&event.ev,
                Some(rv1::ruled_event::Ev::StackResolved(exit)) if exit.object_id == source))
                    .count(),
                usize::from(on_stack)
            );
            assert_eq!(
                events
                    .iter()
                    .filter(|event| matches!(&event.ev,
                Some(rv1::ruled_event::Ev::Log(log)) if log.text == "P0 shuffles their library."))
                    .count(),
                1
            );
        }
    }

    #[test]
    fn zenith_draw_recipient_uses_optional_caster_anchor_and_current_apnap_players() {
        for (caster, lost, expected) in [
            (None, None, vec![]),
            (Some(20), None, vec![30, 10]),
            (Some(20), Some(20), vec![30, 10]),
            (Some(20), Some(30), vec![10]),
        ] {
            let mut engine = GameEngine::new(
                tricerules_cards::registry::global(),
                503_004,
                &[10, 20, 30],
                20,
                None,
                true,
            )
            .unwrap();
            engine.state.active_player_idx = 2;
            if let Some(lost) = lost {
                let seat = engine.state.player_idx(lost).unwrap();
                engine.state.players[seat].has_lost = true;
            }
            let source = add_battlefield_object(&mut engine, 10, "zenith_chronicler");
            let mut item = quantity_item(
                source,
                vec![SpellEffectKind::Draw {
                    count: Amount::Fixed(1),
                    who: PlayerRecipient::EachOtherPlayerThanAffectedPlayer,
                }],
            );
            // Ordinary effect wrappers retain the same captured context through their children.
            item.triggered_ability.as_mut().unwrap().effect = vec![SpellEffectKind::Conditional {
                condition: GameCondition::ActivePlayer {
                    players: RelativePlayerSet::All,
                },
                effect: Box::new(item.triggered_ability.as_ref().unwrap().effect[0].clone()),
            }];
            item.controller = 10;
            item.trigger_context.affected_player = caster;
            let hands: Vec<_> = engine.state.players.iter().map(|p| p.hand.len()).collect();
            let (effects, label) = engine.build_resolution_effects(&item);
            let mut events = Vec::new();
            engine
                .run_effect_list(&item, &label, effects, 0, &mut events)
                .unwrap();
            let logs: Vec<_> = events
                .iter()
                .filter_map(|event| match &event.ev {
                    Some(rv1::ruled_event::Ev::Log(log)) if log.text.contains("draws 1 card") => {
                        Some(log.text.clone())
                    }
                    _ => None,
                })
                .collect();
            assert_eq!(logs.len(), expected.len());
            for (log, player) in logs.iter().zip(&expected) {
                assert!(log.starts_with(&format!("P{player} draws")));
            }
            for (seat, player) in engine.state.players.iter().enumerate() {
                assert_eq!(
                    player.hand.len(),
                    hands[seat] + usize::from(expected.contains(&player.id))
                );
            }
            assert!(engine.state.pending_resolution.is_none());
        }
    }

    #[test]
    fn proliferate_parks_for_a_mixed_any_number_choice() {
        let mut engine = GameEngine::new_with_default_decks(
            tricerules_cards::registry::global(),
            701_034,
            &[0, 1],
            20,
        )
        .unwrap();
        let source = add_battlefield_object(&mut engine, 0, "grizzly_bears");
        engine.state.objects.get_mut(&source).unwrap().add_counters(
            CounterKind::PlusOnePlusOne,
            1,
            0,
        );
        engine
            .state
            .objects
            .get_mut(&source)
            .unwrap()
            .add_counters(CounterKind::Lore, 2, 0);
        engine.state.players[1]
            .counters
            .insert(CounterKind::Poison, 2);
        let item = quantity_item(
            source,
            vec![
                SpellEffectKind::Proliferate,
                SpellEffectKind::GainLife {
                    amount: Amount::Fixed(2),

                    who: tricerules_card_model::primitives::PlayerRecipient::Controller,
                },
            ],
        );
        let (effects, label) = engine.build_resolution_effects(&item);
        let mut events = Vec::new();

        engine
            .run_effect_list(&item, &label, effects, 0, &mut events)
            .expect("Proliferate effect");

        let choice = events
            .iter()
            .find_map(|event| match &event.ev {
                Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(choice)) => Some(choice),
                _ => None,
            })
            .expect("Proliferate must request an any-number choice");
        assert_eq!(choice.choice_kind, rv1::ChoiceKind::Proliferate as i32);
        assert_eq!(choice.candidate_object_ids, [source]);
        assert_eq!(choice.candidate_player_ids, [1]);
        assert_eq!(choice.min, 0);
        assert_eq!(choice.max, 2);

        engine
            .submit_resolution_choice(
                0,
                &rv1::SubmitResolutionChoice {
                    chosen_object_ids: vec![source],
                    chosen_player_ids: vec![1],
                    ..Default::default()
                },
            )
            .expect("accept mixed Proliferate choice");
        assert_eq!(
            engine.state.objects[&source].counter_count(CounterKind::PlusOnePlusOne),
            2
        );
        assert_eq!(
            engine.state.objects[&source].counter_count(CounterKind::Lore),
            3
        );
        assert_eq!(
            engine.state.players[1].counters.get(&CounterKind::Poison),
            Some(&3)
        );
        assert_eq!(engine.state.players[0].life, 22, "effect-list tail resumes");
        assert!(engine.state.pending_resolution.is_none());
    }

    #[test]
    fn proliferate_rejects_an_unoffered_player_and_restores_the_prompt() {
        let mut engine = GameEngine::new_with_default_decks(
            tricerules_cards::registry::global(),
            701_035,
            &[0, 1],
            20,
        )
        .unwrap();
        let source = add_battlefield_object(&mut engine, 0, "grizzly_bears");
        engine.state.objects.get_mut(&source).unwrap().add_counters(
            CounterKind::PlusOnePlusOne,
            1,
            0,
        );
        engine.state.players[1]
            .counters
            .insert(CounterKind::Poison, 2);
        let item = quantity_item(source, vec![SpellEffectKind::Proliferate]);
        let (effects, label) = engine.build_resolution_effects(&item);
        engine
            .run_effect_list(&item, &label, effects, 0, &mut Vec::new())
            .expect("Proliferate effect parks");

        assert!(engine
            .submit_resolution_choice(
                0,
                &rv1::SubmitResolutionChoice {
                    chosen_object_ids: vec![source],
                    chosen_player_ids: vec![0],
                    ..Default::default()
                },
            )
            .is_err());
        assert!(engine.state.pending_resolution.is_some());
        assert_eq!(
            engine.state.objects[&source].counter_count(CounterKind::PlusOnePlusOne),
            1
        );
        assert_eq!(
            engine.state.players[1].counters.get(&CounterKind::Poison),
            Some(&2)
        );
    }

    #[test]
    fn proliferate_revalidates_candidate_generation_and_counter_eligibility() {
        let mut engine = GameEngine::new_with_default_decks(
            tricerules_cards::registry::global(),
            701_036,
            &[0, 1],
            20,
        )
        .unwrap();
        let source = add_battlefield_object(&mut engine, 0, "grizzly_bears");
        engine.state.objects.get_mut(&source).unwrap().add_counters(
            CounterKind::PlusOnePlusOne,
            1,
            0,
        );
        let item = quantity_item(source, vec![SpellEffectKind::Proliferate]);
        let (effects, label) = engine.build_resolution_effects(&item);
        engine
            .run_effect_list(&item, &label, effects, 0, &mut Vec::new())
            .expect("Proliferate effect parks");

        let generation = engine
            .state
            .zone_change_generation
            .get(&source)
            .copied()
            .unwrap_or(0);
        engine
            .state
            .zone_change_generation
            .insert(source, generation + 1);
        assert!(engine
            .submit_resolution_choice(
                0,
                &rv1::SubmitResolutionChoice {
                    chosen_object_ids: vec![source],
                    ..Default::default()
                },
            )
            .is_err());
        assert!(engine.state.pending_resolution.is_some());
        assert_eq!(
            engine.state.objects[&source].counter_count(CounterKind::PlusOnePlusOne),
            1
        );

        engine
            .state
            .zone_change_generation
            .insert(source, generation);
        engine
            .state
            .objects
            .get_mut(&source)
            .unwrap()
            .counters
            .remove(&CounterKind::PlusOnePlusOne);
        assert!(engine
            .submit_resolution_choice(
                0,
                &rv1::SubmitResolutionChoice {
                    chosen_object_ids: vec![source],
                    ..Default::default()
                },
            )
            .is_err());
        assert!(engine.state.pending_resolution.is_some());
        assert_eq!(
            engine.state.objects[&source].counter_count(CounterKind::PlusOnePlusOne),
            0
        );
    }

    #[test]
    fn proliferate_revalidates_player_counter_eligibility_after_choice_is_offered() {
        let mut engine = GameEngine::new_with_default_decks(
            tricerules_cards::registry::global(),
            701_037,
            &[0, 1],
            20,
        )
        .unwrap();
        let source = add_battlefield_object(&mut engine, 0, "grizzly_bears");
        engine.state.players[1]
            .counters
            .insert(CounterKind::Poison, 1);
        let item = quantity_item(source, vec![SpellEffectKind::Proliferate]);
        let (effects, label) = engine.build_resolution_effects(&item);
        engine
            .run_effect_list(&item, &label, effects, 0, &mut Vec::new())
            .expect("Proliferate effect parks");

        let pending = engine
            .state
            .pending_resolution
            .as_ref()
            .expect("Proliferate choice remains pending");
        assert_eq!(
            pending.presentation.choice_kind,
            rv1::ChoiceKind::Proliferate
        );
        assert!(matches!(
            &pending.continuation,
            ResolutionContinuation::Proliferate {
                candidate_player_ids,
                ..
            } if candidate_player_ids == &[1]
        ));

        engine.state.players[1]
            .counters
            .remove(&CounterKind::Poison);
        assert!(engine
            .submit_resolution_choice(
                0,
                &rv1::SubmitResolutionChoice {
                    chosen_player_ids: vec![1],
                    ..Default::default()
                },
            )
            .is_err());
        assert!(engine.state.pending_resolution.is_some());
        assert!(!engine.state.players[1]
            .counters
            .contains_key(&CounterKind::Poison));
    }

    #[test]
    fn issue_483_life_loss_uses_copied_battlefield_mana_value_after_destroy() {
        let mut engine = GameEngine::new_with_default_decks(
            tricerules_cards::registry::global(),
            483_001,
            &[0, 1],
            20,
        )
        .unwrap();
        let original = add_battlefield_object(&mut engine, 1, "grizzly_bears");
        let copied_from = add_battlefield_object(&mut engine, 1, "serra_angel");
        engine
            .state
            .objects
            .get_mut(&original)
            .unwrap()
            .copiable_values = engine.copiable_values_for(copied_from);
        assert_eq!(
            super::super::characteristics::characteristics_from(
                &engine.state,
                engine.registry,
                original,
            )
            .unwrap()
            .mana_value,
            5
        );
        let mut top = triggered_item(original, 0);
        top.targets.push(StackTarget {
            object_id: original,
            required_controller: None,
            group_index: 0,
            damage_amount: 0,
            kind: 0,
            zone_change_generation: Some(0),
        });
        move_object_to_zone(
            &mut engine.state,
            engine.registry,
            original,
            Zone::Graveyard,
            None,
        )
        .unwrap();
        let mut events = Vec::new();
        let previous = EffectResult::default();
        let mut result = EffectResult::default();
        let mut cx = EffectCx {
            engine: &mut engine,
            events: &mut events,
            targets: &[original],
            targets_by_role: &[],
            target_receipts: &[],
            target_role_group_indices: &[],
            top: &top,
            controller: 0,
            affected_player: 0,
            spell_label: "Feed the Swarm",
            previous_effect_result: &previous,
            effect_result: &mut result,
            effect_index: 1,
            resume_previous_result: None,
            program_state: None,
        };
        life::lose_life(
            &mut cx,
            SpellEffectKind::LoseLife {
                amount: LifeAmount::TargetManaValue,
                who: PlayerRecipient::Controller,
            },
        )
        .unwrap();
        assert_eq!(engine.state.players[0].life, 15);
    }

    #[test]
    fn issue_230_residual_hands_discard_as_much_as_possible_and_resume_once() {
        for size in 0..=2 {
            let mut engine = GameEngine::new(
                tricerules_cards::registry::global(),
                23003,
                &[10, 20],
                20,
                Some(vec![vec!["forest".into(); 12]; 2]),
                true,
            )
            .unwrap();
            // Test the post-draw instruction directly, including future draw replacement outcomes.
            engine.state.players.push(PlayerState::new(30, 20));
            for index in 0..2 {
                let hand = engine.state.players[index].hand.clone();
                for oid in hand {
                    move_object_to_zone(
                        &mut engine.state,
                        engine.registry,
                        oid,
                        Zone::Library,
                        None,
                    )
                    .unwrap();
                }
            }
            let source = add_battlefield_object(&mut engine, 10, "tatterkite");
            let mut selected = Vec::new();
            for _ in 0..size {
                let oid = engine.state.players[0].library[0];
                move_object_to_zone(&mut engine.state, engine.registry, oid, Zone::Hand, None)
                    .unwrap();
                selected.push(oid);
            }
            let mut item = quantity_item(
                source,
                vec![
                    SpellEffectKind::Discard {
                        who: PlayerRecipient::EachPlayer,
                        quantity: tricerules_card_model::primitives::DiscardQuantity::UnlessOne {
                            count: 2,
                            filter: tricerules_card_model::primitives::ZoneCardFilter {
                                card_type: Some(
                                    tricerules_card_model::primitives::CardTypeFilter::Creature,
                                ),
                                ..Default::default()
                            },
                        },
                    },
                    SpellEffectKind::GainLife {
                        amount: Amount::Fixed(2),

                        who: tricerules_card_model::primitives::PlayerRecipient::Controller,
                    },
                ],
            );
            item.controller = 10;
            let (effects, label) = engine.build_resolution_effects(&item);
            let mut events = Vec::new();
            engine
                .run_effect_list(&item, &label, effects, 0, &mut events)
                .unwrap();
            if size > 0 {
                let choice = events
                    .iter()
                    .find_map(|event| match &event.ev {
                        Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(choice)) => {
                            Some(choice)
                        }
                        _ => None,
                    })
                    .unwrap();
                assert_eq!(choice.selection_alternatives.len(), 1);
                assert_eq!(choice.selection_alternatives[0].count, size);
                assert_eq!(
                    choice.min, 1,
                    "public minimum never reveals creature eligibility"
                );
                if size == 2 {
                    let answer = rv1::SubmitResolutionChoice {
                        chosen_object_ids: vec![selected[0]],
                        ..Default::default()
                    };
                    assert!(engine.submit_resolution_choice(10, &answer).is_err());
                }
                engine
                    .submit_resolution_choice(
                        10,
                        &rv1::SubmitResolutionChoice {
                            chosen_object_ids: selected.clone(),
                            ..Default::default()
                        },
                    )
                    .unwrap();
            }
            assert!(engine.state.players.iter().all(|p| p.hand.is_empty()));
            assert!(selected
                .iter()
                .all(|oid| engine.state.objects[oid].zone == Zone::Graveyard));
            assert_eq!(engine.state.players[0].life, 22);
            assert!(engine.state.pending_resolution.is_none());
        }
    }

    #[test]
    fn issue_228_dual_type_card_and_third_seat_resume_the_tail_once() {
        for dual_type in [false, true] {
            // This synthetic spell copies Flow State's graveyard gate and private look,
            // then adds a life-gain tail so both completion paths prove cursor resumption.
            let registry = Box::leak(Box::new(
                CardRegistry::from_chunks_and_tokens(
                    &[
                        r#"(id: "forest", name: "Forest", face_id: "forest", types: ["Basic", "Land", "Forest"])"#,
                        r#"(id: "dual", name: "Dual", face_id: "dual", mana_cost: "{U}", types: ["Instant", "Sorcery"])"#,
                        r#"(
                            id: "flow_state_tail_fixture",
                            name: "Flow State tail fixture",
                            face_id: "flow_state_tail_fixture",
                            mana_cost: "{1}{U}",
                            types: ["Sorcery"],
                            spell_program: Some((body: (steps: [
                                (id: "graveyard_gate", bind: [], input: None,
                                 kind: If(
                                   condition: AllOf([
                                     GraveyardAggregate(
                                       owners: Controller, aggregate: CardCount,
                                       filter: Some((card_type: Some(Instant))), min: Some(1), max: None,
                                     ),
                                     GraveyardAggregate(
                                       owners: Controller, aggregate: CardCount,
                                       filter: Some((card_type: Some(Sorcery))), min: Some(1), max: None,
                                     ),
                                   ]),
                                   yes: (steps: [
                                     (id: "take_two", bind: [], input: None,
                                      kind: Perform(LookChooseToHand(
                                        count: 3, filter: None, min: 2, max: 2,
                                        reveal: false, bottom_order: Chosen,
                                      ))),
                                   ], yields: []),
                                   no: (steps: [
                                     (id: "take_one", bind: [], input: None,
                                      kind: Perform(LookChooseToHand(
                                        count: 3, filter: None, min: 1, max: 1,
                                        reveal: false, bottom_order: Chosen,
                                      ))),
                                   ], yields: []),
                                 )),
                                (id: "tail_gain", bind: [], input: None,
                                 kind: Perform(GainLife(amount: 2))),
                            ], yields: []))),
                        )"#,
                    ],
                    &[],
                )
                .expect("synthetic Flow State Program fixture compiles"),
            ));
            let mut engine = GameEngine::new(
                registry,
                22803,
                &[10, 20],
                20,
                Some(vec![vec!["forest".into(); 12]; 2]),
                true,
            )
            .unwrap();
            engine.state.players.push(PlayerState::new(30, 20));
            engine.state.active_player_idx = 1;
            // A single fixture card with both printed types must satisfy both graveyard
            // predicates. It is deliberately not a new supported real card.
            let source = add_battlefield_object(&mut engine, 30, "flow_state_tail_fixture");
            move_object_to_zone(
                &mut engine.state,
                engine.registry,
                source,
                Zone::Hand,
                Some(30),
            )
            .unwrap();
            move_object_to_zone(
                &mut engine.state,
                engine.registry,
                source,
                Zone::Stack,
                None,
            )
            .unwrap();
            let mut library = Vec::new();
            for _ in 0..3 {
                let oid = add_battlefield_object(&mut engine, 30, "forest");
                move_object_to_zone(&mut engine.state, engine.registry, oid, Zone::Library, None)
                    .unwrap();
                library.push(oid);
            }
            if dual_type {
                let dual = add_battlefield_object(&mut engine, 30, "dual");
                move_object_to_zone(
                    &mut engine.state,
                    engine.registry,
                    dual,
                    Zone::Graveyard,
                    None,
                )
                .unwrap();
            }
            let generation = engine
                .state
                .zone_change_generation
                .get(&source)
                .copied()
                .unwrap_or(0);
            let mut item = engine.observer_return_item(source, 0);
            item.id = source;
            item.controller = 30;
            item.source_owner = Some(30);
            item.card_id = "flow_state_tail_fixture".into();
            item.ability_text = None;
            item.is_triggered = false;
            item.is_copy = false;
            item.face_index = 0;
            item.source_permanent_id = None;
            item.cast_by = Some(30);
            item.cast_occurrence = Some(crate::state::StackObjectRef {
                object_id: source,
                zone_change_generation: Some(generation),
            });
            engine.state.stack.push(item);
            assert_eq!(
                engine
                    .resolve_top_of_stack(&mut Vec::new())
                    .expect("synthetic Flow State Program reaches its library choice"),
                ResolutionProgress::Parked
            );
            let pending = engine.state.pending_resolution.as_ref().unwrap();
            assert_eq!(pending.deciding_player, 30);
            let count = if dual_type { 2 } else { 1 };
            assert_eq!(
                (pending.presentation.min, pending.presentation.max),
                (count, count)
            );
            assert_eq!(engine.state.players[2].life, 20);
            let answer = rv1::SubmitResolutionChoice {
                chosen_object_ids: library[..count as usize].to_vec(),
                ..Default::default()
            };
            assert!(engine.submit_resolution_choice(10, &answer).is_err());
            engine.submit_resolution_choice(30, &answer).unwrap();
            if !dual_type {
                assert_eq!(
                    engine.state.players[2].life, 20,
                    "tail waits for bottom ordering"
                );
                engine
                    .submit_resolution_choice(
                        30,
                        &rv1::SubmitResolutionChoice {
                            chosen_object_ids: vec![library[2], library[1]],
                            ..Default::default()
                        },
                    )
                    .unwrap();
            }
            assert!(engine.state.pending_resolution.is_none());
            assert_eq!(engine.state.players[2].life, 22);
            assert_eq!(engine.state.players[0].life, 20);
            assert_eq!(engine.state.players[1].life, 20);
            assert!(engine.submit_resolution_choice(30, &answer).is_err());
            assert_eq!(
                engine.state.players[2].life, 22,
                "duplicate submission cannot replay the tail"
            );
            assert!(engine.state.stack.is_empty());
        }
    }

    #[test]
    fn issue_227_whole_hand_discard_preserves_player_sets_and_runs_the_tail() {
        for who in [PlayerRecipient::EachPlayer, PlayerRecipient::EachOpponent] {
            let mut engine = GameEngine::new(
                tricerules_cards::registry::global(),
                227_003,
                &[10, 20],
                20,
                None,
                true,
            )
            .unwrap();
            // Exercise a third nonconsecutive seat below the two-player session admission gate.
            engine.state.players.push(PlayerState::new(30, 20));
            let third_card = add_battlefield_object(&mut engine, 30, "island");
            move_object_to_zone(
                &mut engine.state,
                engine.registry,
                third_card,
                Zone::Hand,
                None,
            )
            .unwrap();
            engine.state.active_player_idx = 1;
            let source = add_battlefield_object(&mut engine, 10, "tatterkite");
            let hands: Vec<_> = engine
                .state
                .players
                .iter()
                .map(|p| (p.id, p.hand.clone()))
                .collect();
            let mut item = quantity_item(
                source,
                vec![
                    SpellEffectKind::Discard {
                        who,
                        quantity: tricerules_card_model::primitives::DiscardQuantity::All,
                    },
                    SpellEffectKind::GainLife {
                        amount: Amount::Fixed(2),

                        who: tricerules_card_model::primitives::PlayerRecipient::Controller,
                    },
                ],
            );
            item.controller = 10;
            let (effects, label) = engine.build_resolution_effects(&item);
            engine
                .run_effect_list(&item, &label, effects, 0, &mut Vec::new())
                .unwrap();
            for (pid, hand) in hands {
                let player = &engine.state.players[engine.state.player_idx(pid).unwrap()];
                if pid == 10 && who == PlayerRecipient::EachOpponent {
                    assert_eq!(player.hand, hand);
                } else {
                    assert!(player.hand.is_empty());
                    assert!(hand.iter().all(|oid| player.graveyard.contains(oid)));
                }
            }
            assert_eq!(engine.state.players[0].life, 22);
            assert!(engine.state.pending_resolution.is_none());
        }
    }

    #[test]
    fn issue_157_blossombind_prohibition_precedes_stun_and_expires_on_departure() {
        let mut engine = GameEngine::new_with_default_decks(
            tricerules_cards::registry::global(),
            15705,
            &[0, 1],
            20,
        )
        .unwrap();
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
            engine.place_counters(
                target,
                CounterKind::PlusOnePlusOne,
                1,
                super::continuous::CounterPlacementOrigin::Effect,
            ),
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
        assert_eq!(
            engine.place_counters(
                target,
                CounterKind::Stun,
                1,
                super::continuous::CounterPlacementOrigin::Effect,
            ),
            1
        );
        assert_eq!(
            attempt_untap(&mut engine, target),
            UntapOutcome::ReplacedByStun
        );
        assert_eq!(attempt_untap(&mut engine, target), UntapOutcome::Untapped);
    }

    #[test]
    fn issue_157_multiple_observers_recreate_the_same_departure_counter_bag() {
        use tricerules_card_model::primitives::CounterSnapshotSource;
        let mut engine = GameEngine::new_with_default_decks(
            tricerules_cards::registry::global(),
            15709,
            &[0, 1],
            20,
        )
        .unwrap();
        let departed = add_battlefield_object(&mut engine, 0, "dockworker_drone");
        let first = add_battlefield_object(&mut engine, 0, "hill_giant");
        let second = add_battlefield_object(&mut engine, 1, "hill_giant");
        let bag = BTreeMap::from([
            (CounterKind::PlusOnePlusOne, 2),
            (CounterKind::Stun, 3),
            (CounterKind::Keyword(Keyword::Flying), 1),
        ]);
        for (&kind, &count) in &bag {
            engine.place_counters(
                departed,
                kind,
                count,
                super::continuous::CounterPlacementOrigin::Effect,
            );
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
            let mut engine = GameEngine::new_with_default_decks(
                tricerules_cards::registry::global(),
                167201,
                &[0, 1],
                20,
            )
            .unwrap();
            // A permanent-front MDFC using an authored spell program on its instant face
            // exercises destination card types and the program park/resume boundary together.
            engine.registry = Box::leak(Box::new(
                CardRegistry::from_chunks_and_tokens(
                    &[
                        r#"(id: "brainstorm", name: "Brainstorm", layout: ModalDfc, faces: [
                (name: "Body", face_id: "body", types: ["Creature"], power: 2, toughness: 2),
                (name: "Thought", face_id: "thought", types: ["Instant"], spell_program: Some((body: (
                    steps: [
                        (id: "draw_three", kind: Perform(Draw(who: Controller, count: 3))),
                        (id: "choose_put_back", bind: ["chosen"], kind: Perform(SelectHandCards(
                            count: 2, ordered: true, prompt: "Choose cards for the library top."))),
                        (id: "put_back", input: Some("chosen"), kind: Perform(PutSelectedHandCardsOnLibraryTop))
                    ], yields: []))))])"#,
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
            engine.publish_resolution_prompt_ids(&mut events).unwrap();
            assert!(engine.state.pending_resolution.is_some());
            assert_eq!(
                engine.state.objects[&oid].zone,
                Zone::Stack,
                "the spell's physical exit waits for its private put-back choice"
            );
            assert!(
                engine
                    .state
                    .turn_history
                    .current
                    .permanent_cards_entered_graveyard
                    .is_empty(),
                "an unfinished resolution has no graveyard entry"
            );
            if moved_again {
                // Completion must not move an off-stack incarnation a second time.
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
            assert!(engine.apply_scenario_command(1, &command).is_err());
            assert_eq!(engine.state.turn_history, history);
            assert_eq!(engine.state.command_index, command_index);
            engine.apply_scenario_command(0, &command).unwrap();
            assert!(engine.state.pending_resolution.is_none());
            assert_eq!(
                engine
                    .state
                    .turn_history
                    .current
                    .permanent_cards_entered_graveyard
                    .len(),
                1
            );
            assert!(engine.apply_scenario_command(0, &command).is_err());
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
        let mut engine = GameEngine::new_with_default_decks(
            tricerules_cards::registry::global(),
            167202,
            &[0, 1],
            20,
        )
        .unwrap();
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
            tricerules_cards::registry::global(),
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
        perform_discard(
            &mut engine,
            0,
            discard,
            crate::state::DiscardCause::Effect,
            &mut Vec::new(),
        )
        .unwrap();
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
        assert!(perform_discard(
            &mut engine,
            0,
            discard,
            crate::state::DiscardCause::Effect,
            &mut Vec::new()
        )
        .is_err());
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
            counter_stack_object(&mut engine, oid, "test counter", &mut Vec::new()).unwrap();
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
        let mut engine = GameEngine::new_with_default_decks(
            tricerules_cards::registry::global(),
            167204,
            &[0, 1],
            20,
        )
        .unwrap();
        let first = add_battlefield_object(&mut engine, 0, "grizzly_bears");
        let second = add_battlefield_object(&mut engine, 0, "grizzly_bears");
        engine.state.continuous_effects.push(ContinuousEffect {
            source_id: Some(first),
            affected: AffectedScope::Single(second),
            kind: ContinuousEffectKind::Layer4AddTypes(tricerules_card_model::TypeLineAddition {
                land_types: Vec::new(),
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
            let mut engine = GameEngine::new_with_default_decks(
                tricerules_cards::registry::global(),
                165_201,
                &[0, 1],
                20,
            )
            .unwrap();
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
            engine.publish_resolution_prompt_ids(&mut events).unwrap();
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
                assert!(choice.public_reveal.is_none());
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
            let mut engine = GameEngine::new_with_default_decks(
                tricerules_cards::registry::global(),
                165_202,
                &[0, 1],
                20,
            )
            .unwrap();
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
                            required_supertypes: Vec::new(),
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
                            required_supertypes: Vec::new(),
                            token: None,
                            controllers: RelativePlayerSet::Controller,
                            card_type: Some(CardTypeFilter::Creature),
                            any_of: None,
                            color: None,
                            name: None,
                            required_subtypes: vec![],
                            exclude_source: false,
                        },
                        characteristic:
                            tricerules_card_model::BattlefieldQuantityCharacteristic::Power,
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

                        who: tricerules_card_model::primitives::PlayerRecipient::Controller,
                    },
                ],
            );
            item.targets = vec![StackTarget {
                object_id: target_id,
                required_controller: None,
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
            engine.publish_resolution_prompt_ids(&mut events).unwrap();
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
            answer.resolution_prompt_id = engine
                .state
                .pending_resolution
                .as_ref()
                .expect("the soft-counter offer is active")
                .resolution_prompt_id
                .get();
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
                engine.reconcile_activated_ability_slots();
                for (index, treasure) in treasures.iter().enumerate() {
                    engine
                        .apply_scenario_command(
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
            answer.resolution_prompt_id = engine
                .state
                .pending_resolution
                .as_ref()
                .expect("the soft-counter offer remains active")
                .resolution_prompt_id
                .get();
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
            let mut engine = GameEngine::new_with_default_decks(
                tricerules_cards::registry::global(),
                175_301,
                &[0, 1],
                20,
            )
            .unwrap();
            let ferocidon = add_battlefield_object(&mut engine, 1, "rampaging_ferocidon");
            let creature = add_battlefield_object(&mut engine, 0, "grizzly_bears");
            let triggers = engine.collect_event_triggers(&[
                GameEvent::EntersBattlefield {
                    object_id: ferocidon,
                    chosen_x: 0,
                    entry_cause: None,
                },
                GameEvent::EntersBattlefield {
                    object_id: creature,
                    chosen_x: 0,
                    entry_cause: None,
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
                    controller: tricerules_card_model::ControllerReference::Fixed(1),
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

                    who: tricerules_card_model::primitives::PlayerRecipient::Controller,
                },
                vec![],
                0,
            ),
            (
                SpellEffectKind::TargetPlayerGainsLife {
                    amount: 3.into(),
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
                let mut engine = GameEngine::new_with_default_decks(
                    tricerules_cards::registry::global(),
                    175_302,
                    &[0, 1],
                    20,
                )
                .unwrap();
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
                    target_receipts: vec![],
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
            let mut engine = GameEngine::new_with_default_decks(
                tricerules_cards::registry::global(),
                170007,
                &[0, 1],
                20,
            )
            .unwrap();
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
                targets,
                effect,
                target_receipts: vec![],
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
        let mut engine = GameEngine::new_with_default_decks(
            tricerules_cards::registry::global(),
            82_101,
            &[0, 1],
            20,
        )
        .expect("engine");
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
        let mut engine = GameEngine::new_with_default_decks(
            tricerules_cards::registry::global(),
            82_102,
            &[0, 1],
            20,
        )
        .expect("engine");
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
        let mut engine = GameEngine::new_with_default_decks(
            tricerules_cards::registry::global(),
            82_103,
            &[0, 1],
            20,
        )
        .expect("engine");
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
        let mut engine = GameEngine::new_with_default_decks(
            tricerules_cards::registry::global(),
            82_104,
            &[0, 1],
            20,
        )
        .expect("engine");
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
        let mut engine = GameEngine::new_with_default_decks(
            tricerules_cards::registry::global(),
            142_101,
            &[0, 1],
            20,
        )
        .expect("engine");
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
        let mut engine = GameEngine::new_with_default_decks(
            tricerules_cards::registry::global(),
            153_001,
            &[0, 1],
            20,
        )
        .expect("engine");
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
            assert_eq!(
                engine.place_counters(
                    source,
                    kind,
                    2,
                    super::continuous::CounterPlacementOrigin::Effect,
                ),
                0
            );
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

                    who: tricerules_card_model::primitives::PlayerRecipient::Controller,
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
        use tricerules_card_model::primitives::CounterPlacementAffected;
        let mut engine = GameEngine::new_with_default_decks(
            tricerules_cards::registry::global(),
            153_002,
            &[0, 1],
            20,
        )
        .unwrap();
        let aura = add_battlefield_object(&mut engine, 0, "pacifism");
        let first = add_battlefield_object(&mut engine, 0, "grizzly_bears");
        let second = add_battlefield_object(&mut engine, 1, "grizzly_bears");
        engine.place_counters(
            first,
            CounterKind::PlusOnePlusOne,
            1,
            super::continuous::CounterPlacementOrigin::Effect,
        );
        let mut values = engine.copiable_values_for(aura).unwrap();
        values.face.static_abilities = vec![tricerules_card_model::IdentifiedAbility::fallback(
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
        let mut engine = GameEngine::new_with_default_decks(
            tricerules_cards::registry::global(),
            153_003,
            &[0, 1],
            20,
        )
        .unwrap();
        let source = add_battlefield_object(&mut engine, 0, "tatterkite");
        let item = quantity_item(
            source,
            vec![
                SpellEffectKind::Blight { count: 2 },
                SpellEffectKind::Discard {
                    who: PlayerRecipient::EachOpponent,
                    quantity: tricerules_card_model::primitives::DiscardQuantity::Exact(1),
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
        let mut engine = GameEngine::new_with_default_decks(
            tricerules_cards::registry::global(),
            211_001,
            &[0, 1],
            20,
        )
        .unwrap();
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
                        tricerules_card_model::primitives::ResolutionReceiptCondition::CountersPlaced {
                            counter: CounterKind::PlusOnePlusOne,
                            object: tricerules_card_model::primitives::ConditionObjectRef::Source,
                        },
                    ),
                    ability: Box::new(tricerules_card_model::primitives::ReflexiveTriggeredAbilityDef {
                        ability_id: tricerules_card_model::AbilityId::new("reflexive_01").unwrap(),
                        presentation: tricerules_card_model::AbilityPresentation::Fallback,
                        effect: vec![SpellEffectKind::GainLife {
                            amount: Amount::Fixed(1),

                             who: tricerules_card_model::primitives::PlayerRecipient::Controller,}],
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

    fn optional_triggered_item_with_effect(
        engine: &mut GameEngine,
        source: ObjectId,
        effect: Vec<SpellEffectKind>,
    ) -> StackItem {
        let generation = engine
            .state
            .zone_change_generation
            .get(&source)
            .copied()
            .unwrap_or(0);
        let mut item = triggered_item(source, generation);
        item.card_id = "brambleguard_captain".into();
        let mut ability = engine
            .registry
            .get("brambleguard_captain")
            .unwrap()
            .primary_face()
            .triggered_abilities[0]
            .clone();
        ability.effect = effect;
        ability.may = true;
        ability.program = None;
        ability.modal = None;
        ability.targeting = None;
        item.triggered_ability = Some(ability);
        item
    }

    fn resolution_choice_command(
        prompt_id: u64,
        decision: rv1::ResolutionChoiceDecision,
    ) -> RuledCommand {
        RuledCommand {
            cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(
                rv1::SubmitResolutionChoice {
                    resolution_prompt_id: prompt_id,
                    decision: decision as i32,
                    ..Default::default()
                },
            )),
        }
    }

    #[test]
    fn untargeted_optional_trigger_auto_declines_when_effects_are_unavailable() {
        let mut engine = GameEngine::new_with_default_decks(
            tricerules_cards::registry::global(),
            603_504,
            &[0, 1],
            20,
        )
        .unwrap();
        engine.state.opening = None;
        engine.state.turn_step = TurnStep::Main1;
        let source = add_battlefield_object(&mut engine, 0, "grizzly_bears");
        move_object_to_zone(
            &mut engine.state,
            engine.registry,
            source,
            Zone::Graveyard,
            None,
        )
        .unwrap();
        let item = optional_triggered_item_with_effect(
            &mut engine,
            source,
            vec![
                SpellEffectKind::PutCounters {
                    counter: CounterKind::PlusOnePlusOne,
                    count: Amount::Fixed(1),
                    subject: EffectSubject::Source,
                },
                SpellEffectKind::GainLife {
                    amount: Amount::Fixed(3),

                    who: tricerules_card_model::primitives::PlayerRecipient::Controller,
                },
            ],
        );
        let stack_object_id = item.id;
        engine.state.stack.push(item);

        let mut events = Vec::new();
        assert_eq!(
            engine.resolve_top_of_stack(&mut events).unwrap(),
            ResolutionProgress::Completed,
        );
        assert!(engine.state.pending_resolution.is_none());
        assert!(engine.state.stack.is_empty());
        assert_eq!(engine.state.players[0].life, 20);
        assert_eq!(
            events
                .iter()
                .filter(|event| matches!(
                    &event.ev,
                    Some(rv1::ruled_event::Ev::StackResolved(resolved))
                        if resolved.object_id == stack_object_id
                ))
                .count(),
            1,
            "an unavailable optional trigger completes its stack occurrence once",
        );
        assert!(!events.iter().any(|event| matches!(
            &event.ev,
            Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(_))
        )));
    }

    #[test]
    fn untargeted_optional_trigger_apply_rechecks_and_decline_finishes_once() {
        let mut engine = GameEngine::new_with_default_decks(
            tricerules_cards::registry::global(),
            603_505,
            &[0, 1],
            20,
        )
        .unwrap();
        engine.state.opening = None;
        engine.state.turn_step = TurnStep::Main1;
        let source = add_battlefield_object(&mut engine, 0, "grizzly_bears");
        let item = optional_triggered_item_with_effect(
            &mut engine,
            source,
            vec![
                SpellEffectKind::PutCounters {
                    counter: CounterKind::PlusOnePlusOne,
                    count: Amount::Fixed(1),
                    subject: EffectSubject::Source,
                },
                SpellEffectKind::GainLife {
                    amount: Amount::Fixed(3),

                    who: tricerules_card_model::primitives::PlayerRecipient::Controller,
                },
            ],
        );
        let stack_object_id = item.id;
        engine.state.stack.push(item);
        let mut events = Vec::new();
        assert_eq!(
            engine.resolve_top_of_stack(&mut events).unwrap(),
            ResolutionProgress::Parked,
        );
        engine.publish_resolution_prompt_ids(&mut events).unwrap();
        let prompt_id = engine
            .state
            .pending_resolution
            .as_ref()
            .expect("optional trigger is parked")
            .resolution_prompt_id
            .get();
        let offer = events
            .iter()
            .find_map(|event| match &event.ev {
                Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(choice)) => Some(choice),
                _ => None,
            })
            .expect("the optional trigger offers its existing resolution choice");
        assert_eq!(offer.choice_kind, rv1::ChoiceKind::ResolutionBranch as i32);
        assert_eq!((offer.min, offer.max), (0, 1));
        assert_eq!(
            offer.resolution_branches[0].label,
            "Apply the optional effect"
        );

        let wrong_actor =
            resolution_choice_command(prompt_id, rv1::ResolutionChoiceDecision::SelectBranch);
        let before_wrong_actor = format!("{:?}", engine.state);
        assert!(engine.apply_scenario_command(1, &wrong_actor).is_err());
        assert_eq!(format!("{:?}", engine.state), before_wrong_actor);

        let stale_prompt = resolution_choice_command(
            prompt_id.saturating_add(1),
            rv1::ResolutionChoiceDecision::SelectBranch,
        );
        assert!(engine.apply_scenario_command(0, &stale_prompt).is_err());
        assert_eq!(format!("{:?}", engine.state), before_wrong_actor);

        let invalid_shape = RuledCommand {
            cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(
                rv1::SubmitResolutionChoice {
                    resolution_prompt_id: prompt_id,
                    decision: rv1::ResolutionChoiceDecision::SelectBranch as i32,
                    chosen_object_ids: vec![source],
                    ..Default::default()
                },
            )),
        };
        assert!(engine.apply_scenario_command(0, &invalid_shape).is_err());
        assert_eq!(format!("{:?}", engine.state), before_wrong_actor);

        move_object_to_zone(
            &mut engine.state,
            engine.registry,
            source,
            Zone::Graveyard,
            None,
        )
        .unwrap();
        let before_rejected_apply = format!("{:?}", engine.state);
        let apply =
            resolution_choice_command(prompt_id, rv1::ResolutionChoiceDecision::SelectBranch);
        assert!(engine.apply_scenario_command(0, &apply).is_err());
        assert_eq!(format!("{:?}", engine.state), before_rejected_apply);
        assert!(engine.state.pending_resolution.is_some());

        let declined = resolution_choice_command(prompt_id, rv1::ResolutionChoiceDecision::Decline);
        let batch = engine
            .apply_scenario_command(0, &declined)
            .expect("declining skips the optional effect after its source leaves");
        assert_eq!(engine.state.players[0].life, 20);
        assert!(engine.state.pending_resolution.is_none());
        assert!(engine.state.stack.is_empty());
        assert_eq!(
            batch
                .events
                .iter()
                .filter(|event| matches!(
                    &event.ev,
                    Some(rv1::ruled_event::Ev::StackResolved(resolved))
                        if resolved.object_id == stack_object_id
                ))
                .count(),
            1,
            "decline skips the complete optional effect list and resolves the stack item once",
        );
    }

    #[test]
    fn typed_program_trigger_does_not_receive_the_legacy_optional_gate() {
        let mut engine = GameEngine::new_with_default_decks(
            tricerules_cards::registry::global(),
            603_506,
            &[0, 1],
            20,
        )
        .unwrap();
        engine.state.opening = None;
        engine.state.turn_step = TurnStep::Main1;
        let source = add_battlefield_object(&mut engine, 0, "kiora,_the_rising_tide");
        let generation = engine
            .state
            .zone_change_generation
            .get(&source)
            .copied()
            .unwrap_or(0);
        let mut item = triggered_item(source, generation);
        item.card_id = "kiora,_the_rising_tide".into();
        let stack_object_id = item.id;
        let mut ability = engine
            .registry
            .get("kiora,_the_rising_tide")
            .unwrap()
            .primary_face()
            .triggered_abilities[0]
            .clone();
        ability.may = true;
        let face = engine
            .registry
            .get("kiora,_the_rising_tide")
            .unwrap()
            .primary_face();
        item.trigger_use_key = Some(TriggerUseKey {
            object_id: source,
            zone_change_generation: generation,
            ability_origin: TriggerAbilityOrigin::Printed(AbilityDefinitionId {
                card_id: "kiora,_the_rising_tide".into(),
                face_id: face.face_id.clone(),
                ability_path: vec![ability.ability_id.clone()],
            }),
        });
        item.triggered_ability = Some(ability);
        engine.state.stack.push(item);

        let mut events = Vec::new();
        assert_eq!(
            engine.resolve_top_of_stack(&mut events).unwrap(),
            ResolutionProgress::Parked,
            "the Program's own discard choice parks after its draw instruction",
        );
        let choice = events
            .iter()
            .find_map(|event| match &event.ev {
                Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(choice)) => Some(choice),
                _ => None,
            })
            .expect("the Program reaches its discard choice");
        assert_eq!(choice.choice_kind, rv1::ChoiceKind::HandCards as i32);
        assert!(events.iter().all(|event| !matches!(
            &event.ev,
            Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(choice))
                if choice.choice_kind == rv1::ChoiceKind::ResolutionBranch as i32
        )));
        let chosen = engine
            .state
            .pending_resolution
            .as_ref()
            .expect("the Program's discard choice remains pending")
            .presentation
            .candidates
            .iter()
            .take(2)
            .copied()
            .collect::<Vec<_>>();
        assert_eq!(chosen.len(), 2);
        let completed = engine
            .submit_resolution_choice(
                0,
                &rv1::SubmitResolutionChoice {
                    chosen_object_ids: chosen,
                    ..Default::default()
                },
            )
            .expect("finish the Program discard choice");
        assert!(engine.state.pending_resolution.is_none());
        assert!(engine.state.stack.is_empty());
        assert_eq!(
            completed
                .events
                .iter()
                .filter(|event| matches!(
                    &event.ev,
                    Some(rv1::ruled_event::Ev::StackResolved(resolved))
                        if resolved.object_id == stack_object_id
                ))
                .count(),
            1,
            "the typed Program continues and resolves its stack occurrence once",
        );
    }

    #[test]
    fn issue_86_attacking_recipient_rechecks_attackers_at_resolution() {
        let mut engine = GameEngine::new(
            tricerules_cards::registry::global(),
            86_107,
            &[10, 20],
            20,
            None,
            true,
        )
        .expect("two-player engine");
        engine.state.players.push(PlayerState::new(30, 20));
        let source = add_battlefield_object(&mut engine, 10, "capture_sphere");
        let attacker = add_battlefield_object(&mut engine, 30, "grizzly_bears");
        let attacker_generation = engine
            .state
            .zone_change_generation
            .get(&attacker)
            .copied()
            .unwrap_or(0);
        engine.state.combat = Some(CombatState {
            attacking: vec![attacker],
            attack_assignments: HashMap::from([(
                attacker,
                CombatAttackAssignment {
                    attacker: TriggerObjectRef {
                        object_id: attacker,
                        zone_change_generation: attacker_generation,
                        controller_at_event: 30,
                    },
                    defender: CombatDefenderTarget::Player(20),
                    defending_player: 20,
                },
            )]),
            blockers: HashMap::new(),
            damage_assignments: HashMap::new(),
            trample_player_damage: HashMap::new(),
            damage_assignment_needed: false,
            attackers_declared: true,
            blockers_declared_by: Vec::new(),
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
            target_receipts: &[],
            target_role_group_indices: &[],
            top: &item,
            controller: 10,
            affected_player: 10,
            spell_label: "Curse",
            previous_effect_result: &previous_effect_result,
            effect_result: &mut effect_result,
            effect_index: 0,
            resume_previous_result: None,
            program_state: None,
        };

        assert_eq!(player_recipients(&cx, PlayerRecipient::Controller), [10]);
        assert_eq!(
            player_recipients(&cx, PlayerRecipient::AttackingOpponentsOfDefendingPlayer),
            [30]
        );
        assert_eq!(
            player_recipients(&cx, PlayerRecipient::TriggeringAttackingPlayer),
            [30]
        );
        cx.engine
            .state
            .combat
            .as_mut()
            .expect("combat")
            .attack_assignments
            .get_mut(&attacker)
            .expect("assignment")
            .defender = CombatDefenderTarget::Player(10);
        assert!(
            player_recipients(&cx, PlayerRecipient::AttackingOpponentsOfDefendingPlayer).is_empty(),
            "an attacker aimed at another multiplayer defender does not count"
        );
        assert_eq!(
            player_recipients(&cx, PlayerRecipient::TriggeringAttackingPlayer),
            [30],
            "the event-time recipient is not recalculated from other current combat assignments"
        );
        cx.engine
            .state
            .combat
            .as_mut()
            .expect("combat")
            .attack_assignments
            .get_mut(&attacker)
            .expect("assignment")
            .defender = CombatDefenderTarget::Player(20);
        cx.engine
            .state
            .combat
            .as_mut()
            .expect("combat")
            .attacking
            .clear();
        move_object_to_zone(
            &mut cx.engine.state,
            cx.engine.registry,
            attacker,
            Zone::Graveyard,
            None,
        )
        .expect("attacker leaves the battlefield before trigger resolution");
        assert!(
            player_recipients(&cx, PlayerRecipient::AttackingOpponentsOfDefendingPlayer).is_empty(),
            "the Curse reward has no attacking-player recipient after all attackers leave"
        );
        assert_eq!(
            player_recipients(&cx, PlayerRecipient::TriggeringAttackingPlayer),
            [30],
            "the event-time recipient remains after all attackers leave"
        );
        let attacker_player_idx = cx.engine.state.player_idx(30).expect("attacking player");
        cx.engine.state.players[attacker_player_idx].has_lost = true;
        assert!(
            player_recipients(&cx, PlayerRecipient::AttackingOpponentsOfDefendingPlayer).is_empty(),
            "a player who has left the game cannot receive the effect"
        );
        assert!(
            player_recipients(&cx, PlayerRecipient::TriggeringAttackingPlayer).is_empty(),
            "a player who has left the game cannot receive the event-time effect"
        );
        cx.engine.state.players[attacker_player_idx].has_lost = false;
        let defender_player_idx = cx.engine.state.player_idx(20).expect("defending player");
        cx.engine.state.players[defender_player_idx].has_lost = true;
        assert_eq!(
            player_recipients(&cx, PlayerRecipient::TriggeringAttackingPlayer),
            [30],
            "the recipient is the live event-time attacker even if the defender leaves"
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
                    let mut engine = GameEngine::new_with_default_decks(
                        tricerules_cards::registry::global(),
                        15704,
                        &[0, 1],
                        20,
                    )
                    .unwrap();
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
                        .unwrap()
                        .expect("damage batch completes");
                    engine
                        .commit_completed_damage_batch(&completed, &mut events)
                        .unwrap();
                    let dealt = 3 - prevented;
                    let object = &engine.state.objects[&target];
                    assert_eq!(object.damage, 0);
                    assert_eq!(
                        object.counter_count(CounterKind::MinusOneMinusOne),
                        if prohibited { 0 } else { dealt }
                    );
                    assert_eq!(object.deathtouch_damage, dealt > 0);
                    assert_eq!(engine.state.players[0].life, 20 + dealt as i32);
                    assert_eq!(completed[0].result.prevented, u64::from(prevented));
                }
            }
        }
    }

    fn ability_item(source: ObjectId, generation: u64) -> StackItem {
        StackItem {
            prototyped: false,
            mana_colors_spent_to_cast: Default::default(),
            id: source + 1,
            controller: 0,
            card_id: "prodigal_sorcerer".to_string(),
            targets: vec![],
            ability_text: Some("ping".to_string()),
            source_permanent_id: Some(source),
            source_owner: Some(0),
            source_zone_change: generation,
            trigger_use_key: None,
            source_face_change: 0,
            ability_index: Some(0),
            activated_ability: None,
            activated_ability_occurrence: None,
            triggered_ability: None,
            is_triggered: false,
            is_copy: false,
            face_index: 0,
            cast_method: SpellCastMethod::Normal,
            returned_attacker_assignment: None,
            chosen_x: 0,
            chosen_modes: vec![],
            cast_condition_results: Vec::new(),
            cast_occurrence: None,
            cast_by: None,
            cast_cost_receipts: vec![],
            payment_result: CardResultCohort::default(),
            exiled_cohorts: Default::default(),
            chaos_warp_owner_instructions: Default::default(),
            resolution_branch_choices: Default::default(),
            blight_receipts: Vec::new(),
            trigger_context: TriggerContext::default(),
        }
    }

    fn deathtouch_spell_item(chosen_x: u32) -> StackItem {
        StackItem {
            prototyped: false,
            mana_colors_spent_to_cast: Default::default(),
            id: u32::MAX,
            controller: 0,
            card_id: "pharikas_chosen".to_string(),
            targets: vec![],
            ability_text: None,
            source_permanent_id: None,
            source_owner: None,
            source_zone_change: 0,
            trigger_use_key: None,
            source_face_change: 0,
            ability_index: None,
            activated_ability: None,
            activated_ability_occurrence: None,
            triggered_ability: None,
            is_triggered: false,
            is_copy: false,
            face_index: 0,
            cast_method: SpellCastMethod::Normal,
            returned_attacker_assignment: None,
            chosen_x,
            chosen_modes: vec![],
            cast_condition_results: Vec::new(),
            cast_occurrence: None,
            cast_by: None,
            cast_cost_receipts: vec![],
            payment_result: CardResultCohort::default(),
            exiled_cohorts: Default::default(),
            chaos_warp_owner_instructions: Default::default(),
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
                prototyped: false,
                id,
                owner: controller,
                base_controller: controller,
                controller,
                card_id: "hill_giant".to_string(),
                token_origin: None,
                token_faces: None,
                copiable_values: None,
                copy_revision: 0,
                active_copy_occurrence: None,
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
        let mut engine = GameEngine::new_with_default_decks(
            tricerules_cards::registry::global(),
            7022,
            &[0, 1],
            20,
        )
        .expect("new engine");
        let source = engine.state.next_object_id;
        engine.state.next_object_id += 2;
        engine.state.objects.insert(
            source,
            GameObject {
                prototyped: false,
                id: source,
                owner: 0,
                base_controller: 0,
                controller: 0,
                card_id: "prodigal_sorcerer".to_string(),
                token_origin: None,
                token_faces: None,
                copiable_values: None,
                copy_revision: 0,
                active_copy_occurrence: None,
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
        let mut engine = GameEngine::new_with_default_decks(
            tricerules_cards::registry::global(),
            7023,
            &[0, 1],
            20,
        )
        .expect("new engine");
        let first = add_three_toughness_creature(&mut engine, 0);
        let second = add_three_toughness_creature(&mut engine, 1);
        let targets = vec![first, second];
        let targets_by_role = vec![targets.clone()];
        let target_receipts = targets
            .iter()
            .map(|&object_id| StackTarget {
                object_id,
                group_index: 0,
                damage_amount: 0,
                kind: rv1::TargetRefKind::Permanent as i32,
                zone_change_generation: Some(
                    engine
                        .state
                        .zone_change_generation
                        .get(&object_id)
                        .copied()
                        .unwrap_or(0),
                ),
                required_controller: None,
            })
            .collect::<Vec<_>>();
        let target_role_group_indices = vec![0];
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
            targets_by_role: &targets_by_role,
            target_receipts: &target_receipts,
            target_role_group_indices: &target_role_group_indices,
            top: &top,
            controller: 0,
            affected_player: 0,
            spell_label: "deathtouch source",
            previous_effect_result: &previous_effect_result,
            effect_result: &mut effect_result,
            effect_index: 0,
            resume_previous_result: None,
            program_state: None,
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
        let mut engine = GameEngine::new_with_default_decks(
            tricerules_cards::registry::global(),
            7024,
            &[0, 1],
            20,
        )
        .expect("new engine");
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
            target_receipts: &[],
            target_role_group_indices: &[],
            top: &top,
            controller: 0,
            affected_player: 0,
            spell_label: "deathtouch source",
            previous_effect_result: &previous_effect_result,
            effect_result: &mut effect_result,
            effect_index: 0,
            resume_previous_result: None,
            program_state: None,
        };

        mass::damage_all(&mut cx, effect).unwrap();

        for target in [first, second] {
            let object = engine.state.objects.get(&target).unwrap();
            assert_eq!(object.damage, 2);
            assert!(object.deathtouch_damage);
        }
    }
}

#[cfg(test)]
mod hideaway_authoring_tests {
    use tricerules_card_model::CardRegistry;

    #[test]
    fn hideaway_and_its_linked_consumer_form_a_valid_card_pair() {
        let registry = CardRegistry::from_chunks_and_tokens(
            &[
                r#"(id: "hideaway_fixture", name: "Hideaway Fixture", face_id: "hideaway_fixture", types: ["Artifact", "Vehicle"], power: 5, toughness: 5, triggered_abilities: [(ability_id: "hideaway", presentation: OracleLines([1]), trigger: WhenSelfEntersBattlefield, effect: [Hideaway(count: 4, linked_exile_id: "hidden")]), (ability_id: "play_linked", presentation: OracleLines([2]), trigger: WheneverSelfDealsCombatDamageToPlayer, effect: [PlayLinkedExiledCard(producer_ability_id: "hideaway", linked_exile_id: "hidden", filter: Some((card_type: Some(Instant))), play_kind: Cast, return_source_to_owner_hand_if_cast: true)])])"#,
            ],
            &[],
        );

        assert!(
            registry.is_ok(),
            "Hideaway paired ability should validate: {registry:?}"
        );
    }
}
