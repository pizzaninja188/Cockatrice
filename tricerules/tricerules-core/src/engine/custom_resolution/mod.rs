//! Parked resolution and engine-authored choice coordination.
//!
//! This module owns command routing, shared validation, and the common park/resume boundary.
//! Domain modules own the mechanics that apply an accepted choice; they must restore the
//! outstanding [`PendingResolution`] before returning an error so rejected commands are atomic.

use super::events::{
    ev_log, ev_log_ability, ev_log_hidden_from, ev_log_private, ev_priority_changed,
    finish_with_events, format_spell_targets_log, object_display_name,
};
use super::legal_actions::fill_legal;
use super::resolution::{
    counter_stack_object, counter_stack_object_ref, move_object_to_zone,
    optional_triggered_ability_branch, permanent_moved_event,
    permanent_moved_event_with_library_position, resolution_branch_is_selectable,
    sacrifice_permanent, ResolutionEffect,
};
use super::targeting::{
    artifact_exchange_player, capture_stack_target, compute_spell_targets_with_program,
    face_mode_program, face_program, target_role_legality_error, target_schema_with_program,
    TargetResolutionContext, TargetSourceIdentity,
};
use super::*;
use crate::state::{PendingTargetedPlayerChoiceStage, SpecialCastCompletion};

mod attacking_tokens;
mod branches;
mod copy_choices;
mod counter_move;
mod explore;
mod hand_choice;
mod join_forces;
pub(in crate::engine) mod library_cohorts;
mod library_order;
mod library_search;
mod manifest_dread;
mod proliferate;
mod sacrifice_choices;
mod trigger_choices;
mod ward;

#[cfg(test)]
fn synthetic_mill_receipt_gain_life_tail() -> tricerules_card_model::primitives::SpellEffectKind {
    use tricerules_card_model::primitives::{
        Amount, CardResultAction, CardResultFilter, CardResultSource, CountExpression,
        RelativePlayerSet, SpellEffectKind,
    };

    SpellEffectKind::GainLife {
        amount: Amount::Count(CountExpression::CardsMatchingResult {
            filter: CardResultFilter {
                source: CardResultSource::PreviousEffect,
                action: CardResultAction::Mill,
                players: RelativePlayerSet::Controller,
                card_type: None,
            },
        }),

        who: tricerules_card_model::primitives::PlayerRecipient::Controller,
    }
}

pub(in crate::engine) fn can_preserve_player_set_discard_after_controller_departure(
    stack: &ParkedStackResolution,
    discard: &PendingPlayerSetDiscard,
) -> bool {
    stack.item.ability_program_owner().is_some()
        && !discard.choices.is_empty()
        && discard
            .choices
            .iter()
            .all(|choice| !choice.optional_exact && choice.alternative_filter.is_none())
}

fn equipment_receipt_source_label(
    engine: &GameEngine,
    stack: &crate::state::ParkedStackResolution,
) -> String {
    engine
        .registry
        .get(&stack.item.card_id)
        .and_then(|definition| definition.face(stack.item.face_index))
        .map(|face| face.name.to_string())
        .unwrap_or_else(|| stack.item.card_id.clone())
}

impl GameEngine {
    pub(in crate::engine) fn optional_triggered_ability_choice_event(
        &self,
    ) -> Option<rv1::RuledEvent> {
        let pending = self.state.pending_resolution.as_ref()?;
        let (stack, stage) = match &pending.continuation {
            ResolutionContinuation::OptionalTriggeredAbility { stack, stage } => (stack, stage),
            _ => return None,
        };
        let options = match stage {
            crate::state::PendingOptionalTriggeredAbilityStage::Selecting => {
                let label = "Apply the optional effect".to_string();
                vec![rv1::ResolutionBranchOption {
                    branch_index: 0,
                    label: label.clone(),
                    cost_kind: rv1::ResolutionBranchCostKind::Unspecified as i32,
                    cost_text: String::new(),
                    selectable: true,
                    search_zones: Vec::new(),
                    presentation: Some(rv1::PresentationRef {
                        fallback_text: label,
                        ..Default::default()
                    }),
                }]
            }
            crate::state::PendingOptionalTriggeredAbilityStage::ChoosingDelegate { candidates } => {
                super::resolution::resolution_choice_delegate_options(
                    &self.state,
                    stack.item.controller,
                    stack.item.controller,
                    candidates,
                )
            }
        };
        Some(rv1::RuledEvent {
            ev: Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(
                rv1::ResolutionChoiceRequired {
                    resolution_prompt_id: pending.resolution_prompt_id.get(),
                    deciding_player_id: pending.deciding_player,
                    source_object_id: pending.presentation.source_object_id,
                    prompt_text: pending.presentation.prompt.clone(),
                    choice_kind: rv1::ChoiceKind::ResolutionBranch as i32,
                    min: pending.presentation.min,
                    max: pending.presentation.max,
                    resolution_branches: options,
                    ..Default::default()
                },
            )),
        })
    }
}

fn extend_disjoint_card_names(
    previously_seen: &mut HashSet<String>,
    card_names: impl IntoIterator<Item = String>,
) -> bool {
    let card_names = card_names.into_iter().collect::<Vec<_>>();
    if card_names.is_empty() || card_names.iter().any(|name| previously_seen.contains(name)) {
        return false;
    }
    previously_seen.extend(card_names);
    true
}

struct SpecialCastCleanupPlan {
    stack: ParkedStackResolution,
    library_owner: PlayerId,
    completion: SpecialCastCompletion,
    cast_committed: bool,
    destinations: Vec<(TriggerObjectRef, Zone)>,
    next_destination_index: usize,
    command_zone_objects: Vec<ObjectId>,
}

impl GameEngine {
    pub(in crate::engine) fn resolution_prompt_should_be_active(
        &self,
        pending: &PendingResolution,
    ) -> bool {
        !self
            .state
            .pending_spell_cast
            .as_ref()
            .is_some_and(|cast| cast.resolution_time_offer)
            && self
                .state
                .player_idx(pending.deciding_player)
                .is_some_and(|index| !self.state.players[index].has_lost)
    }

    /// Build the authoritative final prompt status. An unassigned pending value can exist briefly
    /// inside dispatch before the command-boundary publisher allocates its ID; it is never exposed
    /// as active during that intermediate step.
    pub(in crate::engine) fn resolution_prompt_state_snapshot(&self) -> rv1::ResolutionPromptState {
        let active = self.state.pending_resolution.as_ref().filter(|pending| {
            self.resolution_prompt_should_be_active(pending)
                && pending.resolution_prompt_id.get() != 0
        });
        if let Some(pending) = active {
            rv1::ResolutionPromptState {
                active: true,
                deciding_player_id: pending.deciding_player,
                resolution_prompt_id: Some(pending.resolution_prompt_id.get()),
            }
        } else {
            rv1::ResolutionPromptState {
                active: false,
                deciding_player_id: 0,
                resolution_prompt_id: None,
            }
        }
    }

    pub(in crate::engine) fn refresh_resolution_prompt_state(&self, batch: &mut RuledEventBatch) {
        batch.resolution_prompt_state = Some(self.resolution_prompt_state_snapshot());
    }

    /// Reject every missing, stale, or wrong-actor answer before command settlement takes a
    /// checkpoint or mutates the parked continuation.
    pub(in crate::engine) fn validate_resolution_prompt_id(
        &self,
        player: PlayerId,
        provided_id: u64,
    ) -> Result<(), EngineError> {
        let pending = self
            .state
            .pending_resolution
            .as_ref()
            .ok_or(EngineError::Illegal("no resolution awaiting a choice"))?;
        if pending.deciding_player != player
            || provided_id == 0
            || pending.resolution_prompt_id.get() == 0
            || pending.resolution_prompt_id.get() != provided_id
        {
            return Err(EngineError::Illegal("stale or foreign resolution prompt"));
        }
        Ok(())
    }

    pub(in crate::engine) fn validate_play_land_prompt_id(
        &self,
        player: PlayerId,
        command: &rv1::PlayLand,
    ) -> Result<(), EngineError> {
        let resolves_play_card = self.state.blocking_choice() == Some(BlockingChoice::Resolution)
            && self
                .state
                .pending_resolution
                .as_ref()
                .is_some_and(|pending| pending.deciding_player == player)
            && self
                .special_cast_permission(player)
                .is_some_and(|permission| permission.scope == ExilePlayPermissionScope::PlayCard);
        if resolves_play_card {
            self.validate_resolution_prompt_id(player, command.resolution_prompt_id)
        } else if command.resolution_prompt_id != 0 {
            Err(EngineError::Illegal(
                "resolution prompt ID is not valid for this land play",
            ))
        } else {
            Ok(())
        }
    }

    /// Assign IDs to the exact offers emitted by one accepted command. New active pending values
    /// have the zero sentinel, so a byte-identical next prompt still allocates unconditionally.
    /// The existing ID is retained only for a same-offer republication with equal answer semantics.
    pub(in crate::engine) fn publish_resolution_prompt_ids(
        &mut self,
        events: &mut [rv1::RuledEvent],
    ) -> Result<(), EngineError> {
        let prompt_indices = events
            .iter()
            .enumerate()
            .filter_map(|(index, event)| {
                matches!(
                    event.ev,
                    Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(_))
                )
                .then_some(index)
            })
            .collect::<Vec<_>>();
        let active_index = prompt_indices.last().copied();
        let active_pending = self
            .state
            .pending_resolution
            .as_ref()
            .filter(|pending| self.resolution_prompt_should_be_active(pending));

        if let Some(index) = active_index {
            let active_choice = match events[index].ev.as_ref() {
                Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(choice)) => choice,
                _ => unreachable!(),
            };
            if let Some(pending) = active_pending {
                if active_choice.deciding_player_id != pending.deciding_player
                    || active_choice.source_object_id != pending.presentation.source_object_id
                    || active_choice.choice_kind != pending.presentation.choice_kind as i32
                {
                    return Err(EngineError::ProgramInvariant(
                        "published resolution offer does not match its pending continuation",
                    ));
                }
                let semantics = self.resolution_prompt_semantics(pending, active_choice)?;
                let old_id = pending.resolution_prompt_id;
                let old_semantics_match = pending
                    .resolution_prompt_semantics
                    .as_ref()
                    .is_some_and(|snapshot| snapshot.answer_semantics == semantics);
                let assigned_id = if old_id.get() == 0 || !old_semantics_match {
                    self.allocate_resolution_prompt_id(pending.deciding_player)?
                } else {
                    old_id
                };
                let pending = self
                    .state
                    .pending_resolution
                    .as_mut()
                    .expect("active pending resolution was checked above");
                pending.resolution_prompt_id = assigned_id;
                let mut published_choice = active_choice.clone();
                published_choice.resolution_prompt_id = assigned_id.get();
                pending.resolution_prompt_semantics =
                    Some(crate::state::ResolutionPromptSemanticSnapshot {
                        answer_semantics: semantics,
                        published_choice,
                    });
                if let Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(choice)) =
                    events[index].ev.as_mut()
                {
                    choice.resolution_prompt_id = assigned_id.get();
                }
            } else {
                // This was a transient offer during a batch that leaves no actionable prompt (for
                // example, an accepted concession invalidating a retained entry cohort). Preserve
                // its reveal/event effects while making the stale choice visibly unanswerable.
                if let Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(choice)) =
                    events[index].ev.as_mut()
                {
                    choice.resolution_prompt_id = 0;
                }
            }
            for transient_index in prompt_indices
                .into_iter()
                .filter(|transient_index| *transient_index != index)
            {
                if let Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(choice)) =
                    events[transient_index].ev.as_mut()
                {
                    choice.resolution_prompt_id = 0;
                }
            }
        } else if let Some(pending) = self
            .state
            .pending_resolution
            .as_ref()
            .filter(|pending| self.resolution_prompt_should_be_active(pending))
        {
            if pending.resolution_prompt_id.get() == 0 {
                return Err(EngineError::ProgramInvariant(
                    "new active resolution prompt was not published",
                ));
            }
        }
        Ok(())
    }

    /// Re-publish a restored active offer when its domain continuation has no specialized
    /// renderer. The cached projection is only a source for the event; it is accepted only when
    /// the current continuation and generation-bound answer semantics still match exactly. The
    /// normal batch publisher assigns the fresh ID after this method appends the event.
    pub(in crate::engine) fn republish_cached_resolution_choice(
        &self,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<bool, EngineError> {
        let Some(pending) = self
            .state
            .pending_resolution
            .as_ref()
            .filter(|pending| self.resolution_prompt_should_be_active(pending))
        else {
            return Ok(false);
        };
        if pending.resolution_prompt_id.get() != 0 {
            return Ok(false);
        }
        let Some(snapshot) = pending.resolution_prompt_semantics.as_ref() else {
            return Ok(false);
        };
        let cached_is_mana_payment =
            snapshot.published_choice.choice_kind == rv1::ChoiceKind::ManaPayment as i32;
        let last_choice = events
            .iter()
            .rev()
            .find_map(|event| match event.ev.as_ref() {
                Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(choice)) => {
                    Some(choice.clone())
                }
                _ => None,
            });
        let last_choice_matches_pending = last_choice.as_ref().is_some_and(|choice| {
            choice.deciding_player_id == pending.deciding_player
                && choice.source_object_id == pending.presentation.source_object_id
                && choice.choice_kind == pending.presentation.choice_kind as i32
        });
        if last_choice_matches_pending && !cached_is_mana_payment {
            return Ok(false);
        }

        // Object generations are retained after a card leaves the game, so the cached semantic
        // snapshot alone cannot prove that every candidate is still live. Player candidates use
        // their separate candidate_player_ids field; every object candidate must still exist
        // before this fallback is allowed to republish the old projection.
        if snapshot
            .published_choice
            .candidate_object_ids
            .iter()
            .any(|object_id| !self.state.objects.contains_key(object_id))
        {
            return Ok(false);
        }

        let current_choice = if cached_is_mana_payment {
            let Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(choice)) = self
                .resolution_payment_choice_event()
                .and_then(|event| event.ev)
            else {
                return Ok(false);
            };
            if choice.choice_kind != rv1::ChoiceKind::ManaPayment as i32 {
                return Ok(false);
            }
            choice
        } else {
            snapshot.published_choice.clone()
        };
        let current_semantics = self.resolution_prompt_semantics(pending, &current_choice)?;
        if current_semantics != snapshot.answer_semantics {
            return Ok(false);
        }
        if last_choice_matches_pending {
            let mut previous = last_choice.expect("matching prior choice exists");
            previous.resolution_prompt_id = 0;
            let mut current = current_choice.clone();
            current.resolution_prompt_id = 0;
            if previous == current {
                return Ok(false);
            }
        }
        events.push(rv1::RuledEvent {
            ev: Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(
                current_choice,
            )),
        });
        Ok(true)
    }

    fn allocate_resolution_prompt_id(
        &mut self,
        deciding_player: PlayerId,
    ) -> Result<crate::state::ResolutionPromptId, EngineError> {
        let last = self
            .state
            .next_resolution_prompt_id
            .get(&deciding_player)
            .copied()
            .unwrap_or(0);
        let value = last.checked_add(1).filter(|value| *value != 0).ok_or(
            EngineError::ProgramInvariant("resolution prompt ID space exhausted"),
        )?;
        let id = crate::state::ResolutionPromptId::assigned(value).ok_or(
            EngineError::ProgramInvariant("resolution prompt ID allocation returned zero"),
        )?;
        self.state
            .next_resolution_prompt_id
            .insert(deciding_player, value);
        Ok(id)
    }

    fn resolution_prompt_semantics(
        &self,
        pending: &PendingResolution,
        choice: &rv1::ResolutionChoiceRequired,
    ) -> Result<Vec<u8>, EngineError> {
        let mut normalized = choice.clone();
        normalized.resolution_prompt_id = 0;
        if normalized.choice_kind == rv1::ChoiceKind::ManaPayment as i32 {
            // Whether a fixed payment is currently affordable depends on the live mana pool.
            // It refreshes the offer's enabled state but does not change the answer being made.
            normalized.payment_currently_legal = false;
        }
        normalized.prompt_text.clear();
        normalized.candidate_server_card_ids.clear();
        normalized.public_reveal = None;
        if !normalized.unique_names {
            normalized.candidate_card_ids.clear();
            normalized.candidate_names.clear();
        }
        for branch in &mut normalized.resolution_branches {
            branch.label.clear();
            branch.cost_text.clear();
            branch.presentation = None;
        }
        for slot in &mut normalized.selection_slots {
            slot.label.clear();
            slot.presentation = None;
        }
        for option in &mut normalized.replacement_options {
            option.source_card_name.clear();
            option.effect_summary.clear();
        }
        let source_generation = self
            .state
            .zone_change_generation
            .get(&pending.presentation.source_object_id)
            .copied()
            .unwrap_or(0);
        let candidate_incarnations = choice
            .candidate_object_ids
            .iter()
            .map(|object_id| {
                (
                    *object_id,
                    self.state
                        .zone_change_generation
                        .get(object_id)
                        .copied()
                        .unwrap_or(0),
                )
            })
            .collect::<Vec<_>>();
        let continuation = serde_json::to_vec(&pending.continuation).map_err(|_| {
            EngineError::ProgramInvariant("resolution continuation cannot be fingerprinted")
        })?;
        serde_json::to_vec(&(
            normalized,
            (pending.presentation.source_object_id, source_generation),
            candidate_incarnations,
            continuation,
        ))
        .map_err(|_| EngineError::ProgramInvariant("resolution offer cannot be fingerprinted"))
    }

    /// Finish the typed follow-up attached to a special-cast offer. The exact exile cohort returns
    /// in a stable event sequence; only the final owner-private zone view exposes any new library
    /// order.
    pub(in crate::engine) fn finish_special_cast_completion(
        &mut self,
        completion: &SpecialCastCompletion,
        cast_committed: bool,
        stack: ParkedStackResolution,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<bool, EngineError> {
        let (library_owner, found, cohort, uncast_found_destination) = match completion {
            SpecialCastCompletion::Discover {
                library_owner,
                found,
                cohort,
            } => (*library_owner, *found, cohort, Zone::Hand),
            SpecialCastCompletion::Cascade {
                library_owner,
                found,
                cohort,
            } => (*library_owner, *found, cohort, Zone::Library),
            SpecialCastCompletion::ResumeStack => return Ok(false),
        };

        let mut destinations = Vec::new();
        if !cast_committed {
            if let Some(reference) = found.filter(|reference| {
                self.special_cast_reference_is_current(library_owner, reference)
            }) {
                destinations.push((reference, uncast_found_destination));
            }
        }

        for reference in cohort {
            if found.is_some_and(|found| found.object_id == reference.object_id)
                || !self.special_cast_reference_is_current(library_owner, reference)
            {
                continue;
            }
            destinations.push((*reference, Zone::Library));
        }

        self.continue_special_cast_cleanup(
            SpecialCastCleanupPlan {
                stack,
                library_owner,
                completion: completion.clone(),
                cast_committed,
                destinations,
                next_destination_index: 0,
                command_zone_objects: Vec::new(),
            },
            events,
        )
    }

    fn special_cast_reference_is_current(
        &self,
        library_owner: PlayerId,
        reference: &TriggerObjectRef,
    ) -> bool {
        self.state
            .objects
            .get(&reference.object_id)
            .is_some_and(|object| {
                object.zone == Zone::Exile
                    && object.owner == library_owner
                    && self
                        .state
                        .zone_change_generation
                        .get(&reference.object_id)
                        .copied()
                        .unwrap_or(0)
                        == reference.zone_change_generation
            })
    }

    /// A resolution-time special cast temporarily reserves its offered exile incarnation on the
    /// stack while the caster pays. Keep that offer parked only when both halves of the exact cast
    /// transaction still bind it to the same owner, object, and zone generation.
    fn special_cast_cast_reservation_is_current(
        &self,
        deciding_player: PlayerId,
        library_owner: PlayerId,
        reference: &TriggerObjectRef,
    ) -> bool {
        let Some(cast) = self.state.pending_spell_cast.as_ref() else {
            return false;
        };
        let Some(object) = self.state.objects.get(&reference.object_id) else {
            return false;
        };
        let current_generation = self
            .state
            .zone_change_generation
            .get(&reference.object_id)
            .copied()
            .unwrap_or(0);

        cast.resolution_time_offer
            && cast.caster == deciding_player
            && cast.caster == library_owner
            && cast.reserved_object_id == reference.object_id
            && cast.original_zone == Zone::Exile
            && cast.original_holder == library_owner
            && cast.original_zone_change_generation == reference.zone_change_generation
            && object.owner == library_owner
            && object.zone == Zone::Stack
            && current_generation == cast.original_zone_change_generation.saturating_add(1)
            && self
                .pending_spell_cast_internal
                .as_ref()
                .is_some_and(|internal| internal.prepared.oid == reference.object_id)
    }

    /// Retire a special-cast choice whose library owner or exact exiled incarnation left while its
    /// resolution was suspended. The remaining cleanup and parent tail still belong to the
    /// already-started resolving ability (CR 608.2m), even when the player who chose it leaves.
    pub(in crate::engine) fn reconcile_departed_special_cast_resolution(
        &mut self,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<bool, EngineError> {
        let Some(pending) = self.state.pending_resolution.take() else {
            return Ok(false);
        };
        match pending.continuation.clone() {
            ResolutionContinuation::SpecialCastCommanderCleanup {
                stack,
                library_owner,
                completion,
                cast_committed,
                destinations,
                commander_choice_index,
                command_zone_objects,
            } => {
                let owner_live = self
                    .state
                    .player_idx(library_owner)
                    .is_some_and(|index| !self.state.players[index].has_lost);
                let offered_reference_is_current = destinations
                    .get(commander_choice_index)
                    .is_some_and(|(reference, _)| {
                        self.special_cast_reference_is_current(library_owner, reference)
                    });
                if owner_live && offered_reference_is_current {
                    self.state.pending_resolution = Some(pending);
                    return Ok(false);
                }

                if self.continue_special_cast_cleanup(
                    SpecialCastCleanupPlan {
                        stack: stack.clone(),
                        library_owner,
                        completion,
                        cast_committed,
                        destinations,
                        next_destination_index: commander_choice_index,
                        command_zone_objects,
                    },
                    events,
                )? {
                    return Ok(true);
                }
                let completed = self.complete_parked_resolution_with_previous(
                    stack.item,
                    stack.resume_effect_index,
                    stack.previous_result,
                    Vec::new(),
                )?;
                events.extend(completed.events);
                Ok(true)
            }
            ResolutionContinuation::SpecialCast {
                stack,
                exiled,
                completion,
                ..
            } => {
                let library_owner = match &completion {
                    SpecialCastCompletion::Discover { library_owner, .. }
                    | SpecialCastCompletion::Cascade { library_owner, .. } => *library_owner,
                    SpecialCastCompletion::ResumeStack => {
                        self.state.pending_resolution = Some(pending);
                        return Ok(false);
                    }
                };
                let owner_live = self
                    .state
                    .player_idx(library_owner)
                    .is_some_and(|index| !self.state.players[index].has_lost);
                let reserved_cast_is_current = self.special_cast_cast_reservation_is_current(
                    pending.deciding_player,
                    library_owner,
                    &exiled,
                );
                if owner_live
                    && (self.special_cast_reference_is_current(library_owner, &exiled)
                        || reserved_cast_is_current)
                {
                    self.state.pending_resolution = Some(pending);
                    return Ok(false);
                }
                if !self.finish_special_cast_completion(
                    &completion,
                    false,
                    stack.clone(),
                    events,
                )? {
                    let completed = self.complete_parked_resolution_with_previous(
                        stack.item,
                        stack.resume_effect_index,
                        stack.previous_result,
                        Vec::new(),
                    )?;
                    events.extend(completed.events);
                }
                Ok(true)
            }
            _ => {
                self.state.pending_resolution = Some(pending);
                Ok(false)
            }
        }
    }

    fn continue_special_cast_cleanup(
        &mut self,
        plan: SpecialCastCleanupPlan,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<bool, EngineError> {
        let SpecialCastCleanupPlan {
            stack,
            library_owner,
            completion,
            cast_committed,
            destinations,
            next_destination_index,
            command_zone_objects,
        } = plan;
        let commander_choice_index = self
            .state
            .player_idx(library_owner)
            .filter(|owner_index| !self.state.players[*owner_index].has_lost)
            .and_then(|owner_index| {
                let player = &self.state.players[owner_index];
                destinations
                    .iter()
                    .enumerate()
                    .skip(next_destination_index)
                    .find_map(|(index, (reference, _))| {
                        (self.special_cast_reference_is_current(library_owner, reference)
                            && player
                                .declared_commander_object_ids
                                .contains(&reference.object_id))
                        .then_some(index)
                    })
            });
        let Some(commander_choice_index) = commander_choice_index else {
            self.apply_special_cast_cleanup(
                library_owner,
                &completion,
                cast_committed,
                &destinations,
                &command_zone_objects,
                events,
            )?;
            return Ok(false);
        };
        let (commander, destination) = destinations[commander_choice_index];
        let name = object_display_name(&self.state, self.registry, commander.object_id);
        let destination_label = match destination {
            Zone::Hand => "their hand",
            Zone::Library => "the bottom of their library",
            _ => {
                return Err(EngineError::Illegal(
                    "invalid special-cast commander destination",
                ));
            }
        };
        let prompt = format!(
            "P{library_owner}: Put {name} into the command zone instead of {destination_label}?"
        );
        self.state.pending_resolution = Some(PendingResolution {
            resolution_prompt_id: crate::state::ResolutionPromptId::UNASSIGNED,
            resolution_prompt_semantics: None,

            deciding_player: library_owner,
            presentation: PendingResolutionPresentation {
                source_object_id: stack.item.id,
                candidates: Vec::new(),
                min: 1,
                max: 1,
                ordered: false,
                unique_names: false,
                prompt: prompt.clone(),
                choice_kind: rv1::ChoiceKind::ResolutionBranch,
            },
            continuation: ResolutionContinuation::SpecialCastCommanderCleanup {
                stack,
                library_owner,
                completion,
                cast_committed,
                destinations,
                commander_choice_index,
                command_zone_objects,
            },
        });
        events.push(
            self.special_cast_commander_choice_event()
                .expect("special-cast commander prompt was installed"),
        );
        events.push(ev_log(prompt));
        Ok(true)
    }

    pub(in crate::engine) fn special_cast_commander_choice_event(&self) -> Option<rv1::RuledEvent> {
        let pending = self.state.pending_resolution.as_ref()?;
        let ResolutionContinuation::SpecialCastCommanderCleanup {
            destinations,
            commander_choice_index,
            ..
        } = &pending.continuation
        else {
            return None;
        };
        let (_, destination) = destinations.get(*commander_choice_index)?;
        let ordinary_destination_label = match destination {
            Zone::Hand => "Hand",
            Zone::Library => "Library bottom",
            _ => return None,
        };
        let resolution_branches = ["Command zone", ordinary_destination_label]
            .into_iter()
            .enumerate()
            .map(|(index, label)| rv1::ResolutionBranchOption {
                branch_index: index as u32,
                label: label.into(),
                selectable: true,
                ..Default::default()
            })
            .collect();
        Some(rv1::RuledEvent {
            ev: Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(
                rv1::ResolutionChoiceRequired {
                    resolution_prompt_id: pending.resolution_prompt_id.get(),
                    deciding_player_id: pending.deciding_player,
                    source_object_id: pending.presentation.source_object_id,
                    prompt_text: pending.presentation.prompt.clone(),
                    choice_kind: rv1::ChoiceKind::ResolutionBranch as i32,
                    min: 1,
                    max: 1,
                    resolution_branches,
                    ..Default::default()
                },
            )),
        })
    }

    fn apply_special_cast_cleanup(
        &mut self,
        library_owner: PlayerId,
        completion: &SpecialCastCompletion,
        cast_committed: bool,
        destinations: &[(TriggerObjectRef, Zone)],
        command_zone_objects: &[ObjectId],
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<(), EngineError> {
        let (found, effect_label) = match completion {
            SpecialCastCompletion::Discover { found, .. } => (*found, "Discover"),
            SpecialCastCompletion::Cascade { found, .. } => (*found, "Cascade"),
            SpecialCastCompletion::ResumeStack => return Ok(()),
        };
        let live_destinations = destinations
            .iter()
            .copied()
            .filter(|(reference, _)| {
                self.special_cast_reference_is_current(library_owner, reference)
            })
            .collect::<Vec<_>>();
        if live_destinations.is_empty() {
            return Ok(());
        }
        let zone_snapshot = self.snapshot_zone_event();
        let mut bottom = Vec::new();
        for (reference, intended_destination) in live_destinations {
            let object_id = reference.object_id;
            let name = object_display_name(&self.state, self.registry, object_id);
            let destination = if command_zone_objects.contains(&object_id) {
                Zone::Command
            } else {
                intended_destination
            };
            let moved_destination = match destination {
                Zone::Hand => rv1::permanent_moved::Destination::Hand,
                Zone::Library => rv1::permanent_moved::Destination::Library,
                Zone::Command => rv1::permanent_moved::Destination::Command,
                _ => {
                    return Err(EngineError::Illegal(
                        "invalid special-cast cleanup destination",
                    ));
                }
            };
            move_object_to_zone(&mut self.state, self.registry, object_id, destination, None)?;
            events.push(permanent_moved_event(
                &self.state,
                object_id,
                library_owner,
                moved_destination,
            ));
            if destination == Zone::Library {
                bottom.push(object_id);
            } else if destination == Zone::Command {
                events.push(ev_log(format!(
                    "P{library_owner} moves {name} to the command zone instead of its {effect_label} destination."
                )));
            } else if !cast_committed && found.is_some_and(|found| found.object_id == object_id) {
                let description = match destination {
                    Zone::Hand => format!("puts {name} into their hand after declining Discover"),
                    Zone::Library => format!(
                        "puts {name} on the bottom of their library after declining Cascade"
                    ),
                    _ => unreachable!("destination was validated above"),
                };
                events.push(ev_log(format!("P{library_owner} {description}.")));
            }
        }

        if !bottom.is_empty() {
            let Some(index) = self.state.player_idx(library_owner) else {
                return Err(EngineError::UnknownPlayer(library_owner));
            };
            shuffle_object_ids_for_current_command(&self.state, library_owner, &mut bottom);
            let bottom_set: std::collections::HashSet<_> = bottom.iter().copied().collect();
            self.state.players[index]
                .library
                .retain(|object_id| !bottom_set.contains(object_id));
            self.state.players[index]
                .library
                .extend(bottom.iter().copied());
            events.push(ev_log(format!(
                "P{library_owner} puts {} cards on the bottom of their library in a random order after {effect_label}.",
                bottom.len(),
            )));
        }

        self.fire_zone_triggers(zone_snapshot, Vec::new(), events);
        Ok(())
    }

    /// Apply a deciding player's answer to the outstanding [`PendingResolution`] (CR 608).
    pub(super) fn submit_resolution_choice(
        &mut self,
        player: PlayerId,
        answer: &rv1::SubmitResolutionChoice,
    ) -> Result<RuledEventBatch, EngineError> {
        if self
            .state
            .pending_resolution
            .as_ref()
            .is_some_and(|pending| {
                matches!(
                    pending.continuation,
                    ResolutionContinuation::EquipmentReceiptAttach { .. }
                )
            })
        {
            return self.finish_equipment_receipt_attach_choice(player, answer);
        }
        let target_hand_discard_choice =
            self.state
                .pending_resolution
                .as_ref()
                .is_some_and(|pending| match &pending.continuation {
                    ResolutionContinuation::HandChoice { stack, hand_choice } => {
                        !hand_choice.program_selection && self.is_target_hand_discard_program(stack)
                    }
                    _ => false,
                });
        if target_hand_discard_choice
            && (answer.decision != rv1::ResolutionChoiceDecision::Unspecified as i32
                || answer.selected_branch_index != 0
                || !answer.chosen_player_ids.is_empty()
                || answer.cast_spell.is_some()
                || answer.spell_cast_announcement.is_some()
                || answer.chosen_combat_defender.is_some()
                || answer.payment.is_some()
                || !answer.restricted_mana.is_empty())
        {
            return Err(EngineError::Illegal(
                "targeted hand-discard Program choice has unrelated resolution data",
            ));
        }
        let manifest_dread_graveyard_order =
            self.state
                .pending_resolution
                .as_ref()
                .is_some_and(|pending| {
                    matches!(
                        pending.continuation,
                        ResolutionContinuation::ManifestDreadGraveyardOrder { .. }
                    )
                });
        if manifest_dread_graveyard_order
            && (answer.decision != rv1::ResolutionChoiceDecision::Unspecified as i32
                || answer.selected_branch_index != 0
                || !answer.chosen_player_ids.is_empty()
                || answer.cast_spell.is_some()
                || answer.spell_cast_announcement.is_some()
                || answer.chosen_combat_defender.is_some()
                || answer.payment.is_some()
                || !answer.restricted_mana.is_empty())
        {
            return Err(EngineError::Illegal(
                "Manifest Dread graveyard order contains unrelated resolution data",
            ));
        }
        if let Some(super::replacement::PendingReplacementEvent::BattlefieldEntry(entry)) =
            &self.state.pending_replacement_event
        {
            if let BattlefieldEntryCompletion::ZoneEntryBatch(batch) = &entry.completion {
                if !self.zone_entry_batch_current(batch) {
                    return Err(EngineError::Illegal("graveyard entry cohort became stale"));
                }
            }
        }
        let pending = self
            .state
            .pending_resolution
            .take()
            .ok_or(EngineError::Illegal("no resolution awaiting a choice"))?;
        if pending.deciding_player != player {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal("not your resolution choice"));
        }
        if matches!(
            &pending.continuation,
            ResolutionContinuation::PublicZoneSearchFixture { .. }
        ) {
            return self.finish_public_zone_search_fixture_choice(pending, answer, player);
        }
        let decision = match rv1::ResolutionChoiceDecision::try_from(answer.decision) {
            Ok(decision) => decision,
            Err(_) => {
                self.state.pending_resolution = Some(pending);
                return Err(EngineError::Illegal("unknown resolution choice decision"));
            }
        };
        if self.copy_target_choice_pending(&pending) {
            return self.finish_copy_target_occurrence_choice(pending, answer);
        }
        if matches!(
            pending.continuation,
            ResolutionContinuation::OptionalTriggeredAbility { .. }
        ) {
            return self.finish_optional_triggered_ability_choice(pending, answer, decision);
        }
        if matches!(
            &pending.continuation,
            ResolutionContinuation::ProgramCohortPartition {
                stage: crate::state::PendingProgramCohortPartitionStage::ChoosingDelegate { .. },
                ..
            } | ResolutionContinuation::ProgramPartitionMove {
                plan: crate::state::ProgramPartitionMovePlan {
                    stage: crate::state::PendingProgramPartitionMoveStage::ChoosingCommander { .. },
                    ..
                },
                ..
            } | ResolutionContinuation::ProgramHandLibraryReturn {
                plan: crate::state::ProgramHandLibraryReturnPlan {
                    stage: crate::state::PendingProgramHandLibraryReturnStage::ChoosingCommander { .. },
                    ..
                },
                ..
            }
        ) {
            return self.finish_program_branch_choice(pending, answer, decision);
        }
        if matches!(
            &pending.continuation,
            ResolutionContinuation::StateBasedActions { .. }
        ) {
            return self.finish_state_based_action_choice(pending, answer, decision);
        }
        if matches!(
            &pending.continuation,
            ResolutionContinuation::TargetedPlayerPermanentChoice {
                stage: PendingTargetedPlayerChoiceStage::ChoosingDelegate { .. },
                ..
            }
        ) {
            return self.select_targeted_player_choice_delegate(pending, answer, decision);
        }
        if matches!(
            pending.continuation,
            ResolutionContinuation::DrawReplacement { .. }
        ) {
            return self.finish_draw_replacement_choice(pending, answer, decision);
        }
        if matches!(
            pending.continuation,
            ResolutionContinuation::JoinForces { .. }
        ) {
            return self.finish_join_forces_payment(pending, answer, decision, player);
        }
        if matches!(
            pending.continuation,
            ResolutionContinuation::CounterMove { .. }
        ) {
            return self.finish_counter_move_choice(pending, answer, decision);
        }
        if matches!(
            pending.continuation,
            ResolutionContinuation::EntryCost { .. }
        ) {
            return self.finish_entry_cost_choice(pending, answer, decision);
        }
        if matches!(
            pending.continuation,
            ResolutionContinuation::EntryReveal { .. }
        ) {
            return self.finish_entry_reveal_choice(pending, answer, decision);
        }
        if matches!(
            pending.continuation,
            ResolutionContinuation::EntryBasicLandType { .. }
        ) {
            return self.finish_basic_land_type_choice(pending, answer, decision);
        }
        if matches!(
            pending.continuation,
            ResolutionContinuation::EntryChooseOpponent { .. }
        ) {
            return self.finish_entry_opponent_choice(pending, answer, decision);
        }
        if matches!(
            pending.continuation,
            ResolutionContinuation::EntryChooseCreatureType { .. }
        ) {
            return self.finish_entry_creature_type_choice(pending, answer, decision);
        }
        if matches!(
            pending.continuation,
            ResolutionContinuation::SagaReadAhead { .. }
        ) {
            return self.finish_saga_read_ahead_choice(pending, answer, decision);
        }
        if matches!(
            pending.continuation,
            ResolutionContinuation::SearchZoneScope { .. }
        ) {
            return self.finish_search_zone_scope(pending, answer, decision);
        }
        if matches!(
            pending.continuation,
            ResolutionContinuation::OptionalSearch { .. }
        ) {
            return self.finish_optional_search_choice(pending, answer, decision);
        }
        if matches!(
            pending.continuation,
            ResolutionContinuation::AuthoredBranch {
                branch: PendingResolutionBranch {
                    stage: PendingResolutionBranchStage::Selecting
                        | PendingResolutionBranchStage::ChoosingDelegate { .. },
                    ..
                },
                ..
            }
        ) {
            return self.select_resolution_branch(pending, answer, decision);
        }
        if matches!(
            pending.continuation,
            ResolutionContinuation::ProgramChoose {
                choose: crate::state::PendingProgramChoose {
                    stage: crate::state::PendingProgramChooseStage::Selecting
                        | crate::state::PendingProgramChooseStage::ChoosingDelegate { .. }
                        | crate::state::PendingProgramChooseStage::ChoosingCommanderDestination { .. },
                    ..
                },
                ..
            }
        ) {
            return self.select_program_choose(pending, answer, decision);
        }
        if matches!(
            &pending.continuation,
            ResolutionContinuation::ProgramChoose {
                choose: crate::state::PendingProgramChoose {
                    stage: crate::state::PendingProgramChooseStage::PayingPermanentCost { .. },
                    ..
                },
                ..
            }
        ) {
            return self.finish_program_choose_permanent_cost(pending, answer, decision);
        }
        if matches!(
            &pending.continuation,
            ResolutionContinuation::ProgramChoose {
                choose: crate::state::PendingProgramChoose {
                    stage: crate::state::PendingProgramChooseStage::PayingGraveyardCardCost { .. },
                    ..
                },
                ..
            }
        ) {
            return self.finish_program_choose_graveyard_card_cost(pending, answer, decision);
        }
        if matches!(
            &pending.continuation,
            ResolutionContinuation::ProgramChoose {
                choose: crate::state::PendingProgramChoose {
                    stage: crate::state::PendingProgramChooseStage::PayingDiscardCardCost { .. },
                    ..
                },
                ..
            }
        ) {
            return self.finish_program_choose_discard_card_cost(pending, answer, decision);
        }
        if let ResolutionContinuation::BattlefieldCohortMove { pending: plan } =
            &pending.continuation
        {
            if matches!(
                plan.stage.as_ref(),
                Some(
                    crate::state::PendingBattlefieldCohortMoveStage::ChoosingCommanderDestination { .. }
                )
            ) {
                return self.finish_battlefield_cohort_move_branch(pending, answer, decision);
            }
        }
        if matches!(
            pending.continuation,
            ResolutionContinuation::OwnerLibraryPlacement { .. }
        ) {
            return self.finish_owner_library_placement(pending, answer, decision);
        }
        if matches!(
            pending.continuation,
            ResolutionContinuation::ChaosWarpCommander { .. }
        ) {
            return self.finish_chaos_warp_commander(pending, answer, decision);
        }
        if matches!(
            pending.continuation,
            ResolutionContinuation::ChooseCardCommander { .. }
        ) {
            return self.finish_choose_card_commander(pending, answer, decision);
        }
        if matches!(
            &pending.continuation,
            ResolutionContinuation::SearchLibrary {
                commander_choice: Some(_),
                ..
            }
        ) {
            return self.finish_search_library_commander_choice(pending, answer, decision);
        }
        if matches!(
            pending.continuation,
            ResolutionContinuation::SpecialCastCommanderCleanup { .. }
        ) {
            return self.finish_special_cast_commander_cleanup(pending, answer, decision);
        }
        if let Some(payment) = pending.continuation.mana_payment().cloned() {
            return self.finish_resolution_mana_payment(pending, payment, answer, decision, player);
        }
        if matches!(
            pending.continuation,
            ResolutionContinuation::SpecialCast { .. }
        ) {
            return self.finish_special_cast_choice(pending, answer, decision);
        }
        if matches!(
            pending.continuation,
            ResolutionContinuation::AttackingTokenDefenders { .. }
        ) {
            return self.finish_attacking_token_defender_choice(pending, answer, decision);
        }
        if matches!(
            pending.continuation,
            ResolutionContinuation::Proliferate { .. }
                | ResolutionContinuation::ProgramProliferate { .. }
        ) {
            return self.finish_proliferate_choice(pending, answer, decision);
        }
        if matches!(
            pending.continuation,
            ResolutionContinuation::WardPayment {
                ward: PendingWardPayment {
                    stage: PendingWardPaymentStage::Discard { .. },
                    ..
                },
                ..
            }
        ) {
            if decision == rv1::ResolutionChoiceDecision::Decline {
                if !answer.chosen_object_ids.is_empty() {
                    self.state.pending_resolution = Some(pending);
                    return Err(EngineError::Illegal(
                        "declining Ward cannot include a discard",
                    ));
                }
                return self.finish_ward_discard(pending, &[]);
            }
            if decision != rv1::ResolutionChoiceDecision::Unspecified {
                self.state.pending_resolution = Some(pending);
                return Err(EngineError::Illegal(
                    "Ward discard requires a card choice or decline",
                ));
            }
        }
        if matches!(
            &pending.continuation,
            ResolutionContinuation::LinkedExileChoice {
                stack,
                key,
                action,
                stage: crate::state::PendingLinkedExileChoiceStage::ChoosingDelegate { .. },
                ..
            } if self.is_mandatory_program_linked_owner_graveyard_choice(stack, key, *action)
        ) {
            return self.select_linked_exile_delegate(pending, answer, decision);
        }
        let program_linked_owner_graveyard_choice = match &pending.continuation {
            ResolutionContinuation::LinkedExileChoice {
                stack,
                key,
                action,
                stage: crate::state::PendingLinkedExileChoiceStage::Selecting { .. },
                ..
            } => self.is_mandatory_program_linked_owner_graveyard_choice(stack, key, *action),
            _ => false,
        };
        if program_linked_owner_graveyard_choice
            && (decision != rv1::ResolutionChoiceDecision::Unspecified
                || answer.selected_branch_index != 0
                || !answer.chosen_player_ids.is_empty()
                || answer.cast_spell.is_some()
                || answer.chosen_combat_defender.is_some()
                || answer.payment.is_some()
                || !answer.restricted_mana.is_empty()
                || answer.spell_cast_announcement.is_some())
        {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal(
                "linked Exile choice contains unrelated answer fields",
            ));
        }
        if decision != rv1::ResolutionChoiceDecision::Unspecified {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal(
                "object resolution choice must leave decision unspecified",
            ));
        }
        if matches!(
            pending.continuation,
            ResolutionContinuation::PlayerSetDiscard { .. }
                | ResolutionContinuation::PlayerSetSacrifice { .. }
        ) && (!answer.chosen_player_ids.is_empty()
            || answer.selected_branch_index != 0
            || answer.cast_spell.is_some()
            || answer.chosen_combat_defender.is_some()
            || answer.payment.is_some()
            || !answer.restricted_mana.is_empty()
            || answer.spell_cast_announcement.is_some())
        {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal(
                "discard choice contains unrelated answer fields",
            ));
        }
        let chosen = answer.chosen_object_ids.as_slice();
        let n = chosen.len() as u32;
        if n < pending.presentation.min || n > pending.presentation.max {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal("wrong number of cards chosen"));
        }
        let mut seen = HashSet::new();
        for &oid in chosen {
            if !pending.presentation.candidates.contains(&oid) || !seen.insert(oid) {
                self.state.pending_resolution = Some(pending);
                return Err(EngineError::Illegal("invalid resolution choice"));
            }
        }
        if pending.presentation.unique_names {
            let mut name_seen: HashSet<String> = HashSet::new();
            for &oid in chosen {
                let card_id = self
                    .state
                    .objects
                    .get(&oid)
                    .map(|o| o.card_id.clone())
                    .unwrap_or_default();
                let names = self
                    .registry
                    .get(&card_id)
                    .map(|definition| {
                        definition
                            .names_outside_stack()
                            .into_iter()
                            .map(str::to_owned)
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_else(|| vec![card_id]);
                if !extend_disjoint_card_names(&mut name_seen, names) {
                    self.state.pending_resolution = Some(pending);
                    return Err(EngineError::Illegal(
                        "chosen cards must have different names",
                    ));
                }
            }
        }

        match &pending.continuation {
            ResolutionContinuation::OptionalTriggeredAbility { .. } => {
                unreachable!("optional triggered ability choice handled above")
            }
            ResolutionContinuation::SimultaneousEntryOrder { .. } => {
                self.finish_entry_timestamp_order_choice(pending, chosen)
            }
            ResolutionContinuation::DiscardReplacement { .. } => {
                self.finish_discard_replacement(pending, chosen)
            }
            ResolutionContinuation::AuraReturn { .. } => {
                self.finish_aura_return(pending, chosen[0])
            }
            ResolutionContinuation::CopyTargets { .. } => {
                unreachable!("copy-target branch handled before object-choice validation")
            }
            ResolutionContinuation::SearchLibrary { .. } => {
                self.finish_library_search(pending, chosen)
            }
            ResolutionContinuation::SearchZoneScope { .. } => {
                unreachable!("search-zone branch handled before object-choice validation")
            }
            ResolutionContinuation::OptionalSearch { .. } => {
                unreachable!("optional-search branch handled before object-choice validation")
            }
            ResolutionContinuation::OwnerLibraryPlacement { .. } => {
                unreachable!("owner placement branch handled before object-choice validation")
            }
            ResolutionContinuation::ChaosWarpCommander { .. } => {
                unreachable!("commander replacement handled before object-choice validation")
            }
            ResolutionContinuation::ChooseCardCommander { .. } => {
                unreachable!("commander replacement handled before object-choice validation")
            }
            ResolutionContinuation::SpecialCastCommanderCleanup { .. } => {
                unreachable!(
                    "special-cast commander replacement handled before object-choice validation"
                )
            }
            ResolutionContinuation::LibraryPartition { .. } => {
                self.finish_library_partition(pending, chosen)
            }
            ResolutionContinuation::ProgramCohortPartition { .. } => {
                self.finish_program_cohort_partition(pending, chosen)
            }
            ResolutionContinuation::ProgramPartitionMove { .. } => {
                self.finish_program_graveyard_order(pending, chosen)
            }
            ResolutionContinuation::ProgramHandLibraryReturn { .. } => {
                unreachable!("commander replacement handled before object-choice validation")
            }
            ResolutionContinuation::BattlefieldCohortMove { .. } => {
                self.finish_battlefield_cohort_move_order(pending, chosen)
            }
            ResolutionContinuation::LibraryLook { .. } => {
                self.finish_look_choose_bottom(pending, chosen)
            }
            ResolutionContinuation::Explore { .. } => self.finish_explore_choice(pending, chosen),
            ResolutionContinuation::ManifestDread { .. } => {
                self.finish_manifest_dread(pending, chosen[0])
            }
            ResolutionContinuation::ManifestDreadGraveyardOrder { .. } => {
                self.finish_manifest_dread_graveyard_order(pending, chosen)
            }
            ResolutionContinuation::HandChoice { .. } => self.finish_hand_choice(pending, chosen),
            ResolutionContinuation::PlayerSetDiscard { .. } => {
                self.finish_player_set_discard_choice(pending, chosen)
            }
            ResolutionContinuation::PlayerSetSacrifice { .. } => {
                self.finish_player_set_sacrifice_choice(pending, chosen)
            }
            ResolutionContinuation::ChooseCard { .. } => self.finish_choose_card(pending, chosen),
            ResolutionContinuation::DifferentNamedGraveyardReturn { .. } => {
                self.finish_different_named_graveyard_return(pending, chosen)
            }
            ResolutionContinuation::LinkedExileChoice { .. } => {
                self.finish_linked_exile_choice(pending, chosen)
            }
            ResolutionContinuation::Sacrifice { .. } => {
                self.finish_sacrifice_chosen(pending, chosen)
            }
            ResolutionContinuation::MassSacrificeGraveyardOrder { .. } => {
                self.finish_mass_sacrifice_graveyard_order(pending, chosen)
            }
            ResolutionContinuation::AuthoredBranch { .. } => {
                self.finish_resolution_branch_object(pending, chosen)
            }
            ResolutionContinuation::ProgramChoose {
                choose:
                    crate::state::PendingProgramChoose {
                        stage:
                            crate::state::PendingProgramChooseStage::PayingGraveyardCardCost { .. },
                        ..
                    },
                ..
            } => self.finish_program_choose_graveyard_card_cost(pending, answer, decision),
            ResolutionContinuation::ProgramChoose {
                choose:
                    crate::state::PendingProgramChoose {
                        stage: crate::state::PendingProgramChooseStage::PayingHandCard { .. },
                        ..
                    },
                ..
            } => self.finish_program_choose_hand_bottom_object(pending, chosen),
            ResolutionContinuation::ProgramChoose {
                choose:
                    crate::state::PendingProgramChoose {
                        stage: crate::state::PendingProgramChooseStage::PayingBlight { .. },
                        ..
                    },
                ..
            } => self.finish_program_choose_blight_object(pending, chosen),
            ResolutionContinuation::PermanentChoice { .. } => {
                self.finish_permanent_choice(pending, chosen)
            }
            ResolutionContinuation::TargetedPlayerPermanentChoice { .. } => {
                self.finish_targeted_player_permanent_choice(pending, chosen)
            }
            ResolutionContinuation::BeholdChoice { .. } => {
                self.finish_behold_choice(pending, chosen)
            }
            ResolutionContinuation::Proliferate { .. } => {
                unreachable!("Proliferate branch handled before object-choice validation")
            }
            ResolutionContinuation::ProgramProliferate { .. } => {
                unreachable!("Program Proliferate branch handled before object-choice validation")
            }
            ResolutionContinuation::AmassChoice { .. } => {
                self.finish_amass_choice(pending, chosen[0])
            }
            ResolutionContinuation::WardPayment { .. } => self.finish_ward_discard(pending, chosen),
            ResolutionContinuation::EntryCopySource { .. } => {
                self.finish_entry_copy_source_choice(pending, chosen)
            }
            ResolutionContinuation::EntryAuraRecipient { .. } => {
                self.finish_entry_aura_recipient_choice(pending, chosen[0])
            }
            ResolutionContinuation::Populate { .. } => {
                self.finish_populate_choice(pending, chosen[0])
            }
            ResolutionContinuation::Blight { .. } => self.finish_blight_choice(pending, chosen[0]),
            ResolutionContinuation::EntryReplacement { .. } => {
                self.finish_battlefield_entry_replacement_choice(pending, chosen[0])
            }
            ResolutionContinuation::CounterReplacement { .. } => {
                self.finish_counter_replacement_choice(pending, chosen[0])
            }
            ResolutionContinuation::EntryCounterReplacement { .. } => {
                self.finish_entry_counter_replacement_choice(pending, chosen[0])
            }
            ResolutionContinuation::EntryCost { .. }
            | ResolutionContinuation::EntryReveal { .. } => {
                unreachable!("entry-cost branch handled before object-choice validation")
            }
            ResolutionContinuation::EntryBasicLandType { .. } => {
                unreachable!("basic-land-type branch handled before object-choice validation")
            }
            ResolutionContinuation::EntryChooseOpponent { .. } => {
                unreachable!("opponent branch handled before object-choice validation")
            }
            ResolutionContinuation::EntryChooseCreatureType { .. } => {
                unreachable!("creature-type branch handled before object-choice validation")
            }
            ResolutionContinuation::SagaReadAhead { .. } => {
                unreachable!("read-ahead branch handled before object-choice validation")
            }
            ResolutionContinuation::DamageReplacement { .. }
            | ResolutionContinuation::CombatDamageReplacement { .. } => {
                self.finish_damage_replacement_choice(pending, chosen[0])
            }
            ResolutionContinuation::ManaAbilityDamageReplacement { .. } => {
                self.finish_damage_replacement_choice(pending, chosen[0])
            }
            ResolutionContinuation::StateBasedActions { .. } => {
                unreachable!("state-based action choice handled before object validation")
            }
            ResolutionContinuation::BattleProtector { .. } => {
                self.finish_battle_protector_choice(pending, chosen[0] as PlayerId)
            }
            ResolutionContinuation::SpecialCast { .. } => {
                unreachable!("Siege-cast branch handled before object-choice validation")
            }
            ResolutionContinuation::AttackingTokenDefenders { .. } => {
                unreachable!("attacking-token branch handled before object-choice validation")
            }
            ResolutionContinuation::DrawReplacement { .. } => {
                unreachable!("draw replacement handled above")
            }
            ResolutionContinuation::ManaPayment { .. }
            | ResolutionContinuation::JoinForces { .. }
            | ResolutionContinuation::ProgramChoose { .. }
            | ResolutionContinuation::EquipmentReceiptAttach { .. }
            | ResolutionContinuation::CounterMove { .. }
            | ResolutionContinuation::PublicZoneSearchFixture { .. } => {
                unreachable!("handled above")
            }
        }
    }

    /// Apply one answer to the exact created-token receipt without letting generic
    /// refresh/choice handling mutate the parked state before validating it.
    fn finish_equipment_receipt_attach_choice(
        &mut self,
        player: PlayerId,
        answer: &rv1::SubmitResolutionChoice,
    ) -> Result<RuledEventBatch, EngineError> {
        let pending = self
            .state
            .pending_resolution
            .as_ref()
            .expect("receipt chooser checked above");
        if pending.deciding_player != player {
            return Err(EngineError::Illegal("not your resolution choice"));
        }
        if answer.decision != rv1::ResolutionChoiceDecision::Unspecified as i32
            || answer.selected_branch_index != 0
            || answer.chosen_object_ids.len() != 1
            || !answer.chosen_player_ids.is_empty()
            || answer.cast_spell.is_some()
            || answer.spell_cast_announcement.is_some()
            || answer.chosen_combat_defender.is_some()
            || answer.payment.is_some()
            || !answer.restricted_mana.is_empty()
        {
            return Err(EngineError::Illegal(
                "Equipment receipt choice contains unrelated or malformed data",
            ));
        }
        let chosen = answer.chosen_object_ids[0];
        let crate::state::ResolutionContinuation::EquipmentReceiptAttach { attachment } =
            &pending.continuation
        else {
            unreachable!("receipt chooser continuation checked above")
        };
        let expected_generation = attachment
            .candidate_generations
            .iter()
            .find_map(|(object_id, generation)| (*object_id == chosen).then_some(*generation))
            .ok_or(EngineError::Illegal("invalid Equipment receipt choice"))?;
        let current_candidates = super::resolution::current_equipment_receipt_candidates(
            self,
            attachment.equipment,
            &attachment.created_tokens,
        );
        if !current_candidates.contains(&(chosen, expected_generation)) {
            return Err(EngineError::Illegal("stale Equipment receipt choice"));
        }
        let source_label = equipment_receipt_source_label(self, &attachment.stack);

        let pending = self
            .state
            .pending_resolution
            .take()
            .expect("receipt choice validated against parked state");
        let crate::state::ResolutionContinuation::EquipmentReceiptAttach { attachment } =
            pending.continuation
        else {
            unreachable!("receipt chooser continuation checked above")
        };
        let mut events = vec![ev_log(format!(
            "P{} chooses {}.",
            player,
            object_display_name(&self.state, self.registry, chosen)
        ))];
        if !super::resolution::attach_equipment_ids(
            self,
            attachment.equipment.object_id,
            chosen,
            &source_label,
            &mut events,
        ) {
            self.state.pending_resolution = Some(PendingResolution {
                resolution_prompt_id: pending.resolution_prompt_id,
                resolution_prompt_semantics: pending.resolution_prompt_semantics,
                deciding_player: pending.deciding_player,
                presentation: pending.presentation,
                continuation: ResolutionContinuation::EquipmentReceiptAttach { attachment },
            });
            return Err(EngineError::Illegal("stale Equipment receipt choice"));
        }
        self.complete_parked_stack_with_previous(attachment.stack, EffectResult::default(), events)
    }

    fn select_optional_triggered_ability_delegate(
        &mut self,
        mut pending: PendingResolution,
        answer: &rv1::SubmitResolutionChoice,
        decision: rv1::ResolutionChoiceDecision,
    ) -> Result<RuledEventBatch, EngineError> {
        if decision != rv1::ResolutionChoiceDecision::SelectBranch
            || !answer.chosen_object_ids.is_empty()
            || !answer.chosen_player_ids.is_empty()
            || answer.cast_spell.is_some()
            || answer.chosen_combat_defender.is_some()
            || answer.payment.is_some()
            || !answer.restricted_mana.is_empty()
            || answer.spell_cast_announcement.is_some()
        {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal(
                "selecting an optional-trigger delegate requires one delegate index",
            ));
        }
        let (controller, candidates) = match &pending.continuation {
            ResolutionContinuation::OptionalTriggeredAbility {
                stack,
                stage:
                    crate::state::PendingOptionalTriggeredAbilityStage::ChoosingDelegate { candidates },
            } => (stack.item.controller, candidates.clone()),
            _ => {
                self.state.pending_resolution = Some(pending);
                return Err(EngineError::Illegal(
                    "optional-trigger delegate continuation missing",
                ));
            }
        };
        let Some(delegate) = super::resolution::resolution_choice_delegate_at_index(
            &self.state,
            controller,
            controller,
            &candidates,
            answer.selected_branch_index,
        ) else {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal(
                "that replacement chooser is no longer available",
            ));
        };
        let selector = pending.deciding_player;
        pending.deciding_player = delegate;
        pending.presentation.choice_kind = rv1::ChoiceKind::ResolutionBranch;
        pending.presentation.min = 0;
        pending.presentation.max = 1;
        pending.presentation.prompt = "Choose a resolution option, or decline.".into();
        pending.resolution_prompt_semantics = None;
        let ResolutionContinuation::OptionalTriggeredAbility { stage, .. } =
            &mut pending.continuation
        else {
            unreachable!("validated optional-trigger delegate continuation")
        };
        *stage = crate::state::PendingOptionalTriggeredAbilityStage::Selecting;
        self.state.pending_resolution = Some(pending);
        let mut events = vec![ev_log(format!(
            "P{selector} selects P{delegate} to make P{controller}'s optional-trigger choice."
        ))];
        events.push(
            self.optional_triggered_ability_choice_event()
                .expect("delegated optional trigger remains parked"),
        );
        Ok(finish_with_events(self, events))
    }

    fn finish_optional_triggered_ability_choice(
        &mut self,
        pending: PendingResolution,
        answer: &rv1::SubmitResolutionChoice,
        decision: rv1::ResolutionChoiceDecision,
    ) -> Result<RuledEventBatch, EngineError> {
        let choosing_delegate = matches!(
            &pending.continuation,
            ResolutionContinuation::OptionalTriggeredAbility {
                stage: crate::state::PendingOptionalTriggeredAbilityStage::ChoosingDelegate { .. },
                ..
            }
        );
        let invalid_shape = !answer.chosen_object_ids.is_empty()
            || !answer.chosen_player_ids.is_empty()
            || (!choosing_delegate && answer.selected_branch_index != 0)
            || answer.cast_spell.is_some()
            || answer.chosen_combat_defender.is_some()
            || answer.payment.is_some()
            || !answer.restricted_mana.is_empty()
            || answer.spell_cast_announcement.is_some();
        if invalid_shape {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal(
                "optional triggered ability choice contains unrelated fields",
            ));
        }
        if choosing_delegate {
            return self.select_optional_triggered_ability_delegate(pending, answer, decision);
        }
        let (stack, stage) = match &pending.continuation {
            ResolutionContinuation::OptionalTriggeredAbility { stack, stage } => {
                (stack.clone(), stage.clone())
            }
            _ => {
                self.state.pending_resolution = Some(pending);
                return Err(EngineError::Illegal(
                    "optional triggered ability continuation missing",
                ));
            }
        };
        if !matches!(
            stage,
            crate::state::PendingOptionalTriggeredAbilityStage::Selecting
        ) {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal(
                "optional triggered ability choice is in the wrong stage",
            ));
        }
        let original_controller_live = self
            .state
            .player_idx(stack.item.controller)
            .is_some_and(|index| !self.state.players[index].has_lost);
        let is_delegated_answer = !original_controller_live
            && super::resolution::resolution_choice_delegate_candidates(
                &self.state,
                stack.item.controller,
                stack.item.controller,
            )
            .contains(&pending.deciding_player);
        let mut events = Vec::new();
        let ability_text = stack.item.ability_text.as_deref().unwrap_or_default();
        let ability_presentation = self
            .state
            .stack_presentations
            .get(&stack.item.id)
            .and_then(|presentation| presentation.primary.clone());
        let resume_index = match decision {
            rv1::ResolutionChoiceDecision::SelectBranch => {
                let ability = stack.item.triggered_ability.as_ref().or_else(|| {
                    self.registry
                        .get(&stack.item.card_id)
                        .and_then(|definition| definition.face(stack.item.face_index))
                        .and_then(|face| {
                            face.triggered_abilities
                                .get(stack.item.ability_index.unwrap_or(0))
                        })
                });
                if !is_delegated_answer {
                    if let Some(branch) = ability.and_then(optional_triggered_ability_branch) {
                        if !resolution_branch_is_selectable(
                            self,
                            &stack.item,
                            &stack.previous_result,
                            pending.deciding_player,
                            stack.item.controller,
                            &branch,
                            true,
                        ) {
                            self.state.pending_resolution = Some(pending);
                            return Err(EngineError::Illegal(
                                "the optional effect is no longer applicable",
                            ));
                        }
                    } else if ability.is_some_and(|ability| {
                        super::resolution::direct_draw_effect_is_selectable(
                            self,
                            &stack.item,
                            &stack.previous_result,
                            &ability.effect,
                        ) == Some(false)
                    }) {
                        self.state.pending_resolution = Some(pending);
                        return Err(EngineError::Illegal(
                            "the optional draw is no longer legal under the current draw limit",
                        ));
                    }
                }
                Some(0)
            }
            rv1::ResolutionChoiceDecision::Decline => {
                events.push(ev_log_ability(
                    format!("P{} declines optional trigger: ", pending.deciding_player),
                    ability_text,
                    ability_presentation,
                    String::new(),
                ));
                Some(self.build_resolution_effects(&stack.item).0.len() as u32)
            }
            _ => {
                self.state.pending_resolution = Some(pending);
                return Err(EngineError::Illegal(
                    "optional triggered ability requires apply or decline",
                ));
            }
        };
        self.complete_parked_resolution_with_previous(
            stack.item,
            resume_index,
            stack.previous_result,
            events,
        )
    }

    pub(in crate::engine) fn refresh_optional_triggered_ability_departure(
        &mut self,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<(), EngineError> {
        let Some(mut pending) = self.state.pending_resolution.take() else {
            return Ok(());
        };
        let (controller, stage) = match &pending.continuation {
            ResolutionContinuation::OptionalTriggeredAbility { stack, stage } => {
                (stack.item.controller, stage.clone())
            }
            _ => {
                self.state.pending_resolution = Some(pending);
                return Ok(());
            }
        };
        let controller_live = self
            .state
            .player_idx(controller)
            .is_some_and(|index| !self.state.players[index].has_lost);
        let current_chooser_live = self
            .state
            .player_idx(pending.deciding_player)
            .is_some_and(|index| !self.state.players[index].has_lost);
        if controller_live {
            // A source-owner-only departure does not transfer the original controller's choice.
            self.state.pending_resolution = Some(pending);
            return Ok(());
        }

        let eligible =
            resolution::resolution_choice_delegate_candidates(&self.state, controller, controller);
        if matches!(
            stage,
            crate::state::PendingOptionalTriggeredAbilityStage::Selecting
        ) && current_chooser_live
            && eligible.contains(&pending.deciding_player)
        {
            // The current live decider is the already selected delegate for Apply/Decline.
            self.state.pending_resolution = Some(pending);
            return Ok(());
        }

        let Some(selector) = resolution::resolution_choice_selector(&self.state, controller) else {
            self.state.pending_resolution = Some(pending);
            return Ok(());
        };
        if eligible.is_empty() {
            self.state.pending_resolution = Some(pending);
            return Ok(());
        }
        let previous_slots = match &stage {
            crate::state::PendingOptionalTriggeredAbilityStage::ChoosingDelegate { candidates } => {
                candidates.as_slice()
            }
            crate::state::PendingOptionalTriggeredAbilityStage::Selecting => &[],
        };
        let candidate_slots =
            resolution::reconcile_resolution_choice_delegate_slots(previous_slots, &eligible);
        let same_prompt = matches!(
            &stage,
            crate::state::PendingOptionalTriggeredAbilityStage::ChoosingDelegate { candidates }
                if current_chooser_live
                    && pending.deciding_player == selector
                    && *candidates == candidate_slots
        );
        if same_prompt {
            self.state.pending_resolution = Some(pending);
            return Ok(());
        }

        pending.deciding_player = selector;
        pending.presentation.choice_kind = rv1::ChoiceKind::ResolutionBranch;
        pending.presentation.min = 1;
        pending.presentation.max = 1;
        pending.presentation.prompt = format!(
            "P{selector}: choose a player to make P{controller}'s optional-trigger choice."
        );
        pending.resolution_prompt_semantics = None;
        let ResolutionContinuation::OptionalTriggeredAbility { stage, .. } =
            &mut pending.continuation
        else {
            unreachable!("validated optional-trigger continuation")
        };
        *stage = crate::state::PendingOptionalTriggeredAbilityStage::ChoosingDelegate {
            candidates: candidate_slots,
        };
        self.state.pending_resolution = Some(pending);
        events.push(ev_log(format!(
            "P{selector} must choose a replacement for P{controller}'s optional-trigger choice."
        )));
        events.push(
            self.optional_triggered_ability_choice_event()
                .expect("optional-trigger delegate prompt remains parked"),
        );
        Ok(())
    }

    fn finish_aura_return(
        &mut self,
        pending: PendingResolution,
        chosen: ObjectId,
    ) -> Result<RuledEventBatch, EngineError> {
        let ResolutionContinuation::AuraReturn { stack, exiled } = &pending.continuation else {
            unreachable!("Aura return continuation routed by caller")
        };
        let stack = stack.clone();
        let exiled = *exiled;
        let generation = self
            .state
            .zone_change_generation
            .get(&exiled.object_id)
            .copied()
            .unwrap_or(0);
        let Some(object) = self.state.objects.get(&exiled.object_id) else {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal("returning Aura no longer exists"));
        };
        if object.zone != Zone::Exile || generation != exiled.zone_change_generation {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal("returning Aura choice became stale"));
        }
        let owner = object.owner;
        let Some(filter) = self.effective_face(exiled.object_id).and_then(|face| {
            face.spell_effect.iter().find_map(|effect| match effect {
                SpellEffectKind::AuraAttach { target } => Some(target.clone()),
                _ => None,
            })
        }) else {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal(
                "returning object is no longer an Aura",
            ));
        };
        let recipient = match pending.presentation.choice_kind {
            rv1::ChoiceKind::AuraPermanent => AttachmentRecipient::Object(chosen),
            rv1::ChoiceKind::AuraPlayer => AttachmentRecipient::Player(chosen as PlayerId),
            _ => unreachable!("Aura return uses a typed choice kind"),
        };
        if !super::targeting::attachment_filter_legal(
            self,
            &filter,
            recipient,
            exiled.object_id,
            owner,
        ) {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal("Aura recipient is no longer legal"));
        }

        let label = object_display_name(&self.state, self.registry, exiled.object_id);
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
            attached_to: Some(recipient),
            pending_copy_candidate: None,
            pending_aura_recipient: None,
            accepted_aura_recipient: Some(crate::state::AcceptedAuraEntryRecipient {
                filter,
                entering_zone_generation: generation,
                entering_copy_revision: object.copy_revision,
                recipient_generation: match recipient {
                    AttachmentRecipient::Object(oid) => Some(
                        self.state
                            .zone_change_generation
                            .get(&oid)
                            .copied()
                            .unwrap_or(0),
                    ),
                    AttachmentRecipient::Player(_) => None,
                },
                copy_candidate: None,
            }),
            applied_effects: Vec::new(),
        };
        let resume_original_stack = stack.is_some();
        let item = stack
            .as_ref()
            .map(|parked| parked.item.clone())
            .unwrap_or_else(|| self.observer_return_item(exiled.object_id, owner));
        let mut events = Vec::new();
        let entry = match self.begin_battlefield_entry(
            item,
            entry,
            BattlefieldEntryCompletion::ObserverReturn {
                owner,
                object_label: label.clone(),
                attached_to: Some(recipient),
                resume_original_stack,
            },
            &mut events,
        )? {
            super::replacement::BattlefieldEntryProgress::Parked => {
                if let (Some(resume), Some(next_pending)) =
                    (stack.as_ref(), self.state.pending_resolution.as_mut())
                {
                    if let Some(parked) = next_pending.continuation.stack_mut() {
                        parked.resume_effect_index = resume.resume_effect_index;
                        parked.previous_result = resume.previous_result.clone();
                        parked.program = resume.program.clone();
                    }
                }
                return Ok(finish_with_events(self, events));
            }
            super::replacement::BattlefieldEntryProgress::Ready(entry) => *entry,
            super::replacement::BattlefieldEntryProgress::Skipped(entry) => {
                let item = stack
                    .as_ref()
                    .map(|stack| stack.item.clone())
                    .unwrap_or_else(|| self.observer_return_item(exiled.object_id, owner));
                return self.finish_entry_copy_without_recipient(
                    stack.unwrap_or_else(|| ParkedStackResolution::new(item)),
                    *entry,
                    BattlefieldEntryCompletion::ObserverReturn {
                        owner,
                        object_label: label,
                        attached_to: Some(recipient),
                        resume_original_stack,
                    },
                    events,
                );
            }
        };
        self.state
            .pending_observer_return_batch
            .get_or_insert_with(|| PendingObserverReturnBatch {
                ready: Vec::new(),
                remaining: VecDeque::new(),
                resume_stack: stack.clone(),
                suspended_resolution: None,
                suspended_replacement_event: None,
            })
            .ready
            .push(ObserverReturnEntry {
                event: entry,
                owner,
                label,
                attached_to: Some(recipient),
            });
        if self.drain_immediate_observer_actions(stack.clone(), &mut events)? {
            return Ok(finish_with_events(self, events));
        }
        if let Some(stack) = stack {
            let previous_result = stack.previous_result.clone();
            return self.complete_parked_stack_with_previous(stack, previous_result, events);
        }
        self.apply_sbas(&mut events)?;
        if let Some(index) = self.state.player_idx(self.state.active_player_id()) {
            self.state.priority_idx = index;
        }
        events.push(ev_priority_changed(self));
        Ok(finish_with_events(self, events))
    }

    fn finish_battle_protector_choice(
        &mut self,
        pending: PendingResolution,
        protector: PlayerId,
    ) -> Result<RuledEventBatch, EngineError> {
        let ResolutionContinuation::BattleProtector { stack } = &pending.continuation else {
            unreachable!("Battle protector continuation routed by caller")
        };
        let _stack = stack;
        let Some(pending_event) = self.state.pending_replacement_event.take() else {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal("Battle protector choice became stale"));
        };
        let mut entry = match pending_event {
            super::replacement::PendingReplacementEvent::BattlefieldEntry(entry) => *entry,
            other => {
                self.state.pending_replacement_event = Some(other);
                self.state.pending_resolution = Some(pending);
                return Err(EngineError::Illegal("Battle protector choice became stale"));
            }
        };
        let battle_id = entry.event.object_id;
        if !self.entry_battle_protector_is_live(&entry.event, protector)
            || !self
                .characteristics(battle_id)
                .is_some_and(|value| value.has_type("Battle"))
        {
            self.state.pending_replacement_event = Some(
                super::replacement::PendingReplacementEvent::BattlefieldEntry(Box::new(entry)),
            );
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal("Battle protector choice became stale"));
        }
        entry.event.battle_protector = Some(protector);
        let events = vec![ev_log(format!(
            "P{} chooses P{protector} to protect the Battle.",
            pending.deciding_player
        ))];
        self.complete_pending_battlefield_entry(pending, entry.event, entry.completion, events)
    }

    fn finish_special_cast_choice(
        &mut self,
        pending: PendingResolution,
        answer: &rv1::SubmitResolutionChoice,
        decision: rv1::ResolutionChoiceDecision,
    ) -> Result<RuledEventBatch, EngineError> {
        if !answer.chosen_object_ids.is_empty()
            || answer.selected_branch_index != 0
            || answer.chosen_combat_defender.is_some()
            || answer.payment.is_some()
            || !answer.restricted_mana.is_empty()
        {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal(
                "special cast has unrelated resolution choice data",
            ));
        }
        let ResolutionContinuation::SpecialCast {
            stack,
            exiled,
            method,
            completion,
            ..
        } = &pending.continuation
        else {
            unreachable!()
        };
        let stack = stack.clone();
        let exiled = *exiled;
        let method = *method;
        let completion = completion.clone();
        let mut events = Vec::new();
        match decision {
            rv1::ResolutionChoiceDecision::Decline => {
                if answer.cast_spell.is_some()
                    || answer.spell_cast_announcement.is_some()
                    || !answer.chosen_object_ids.is_empty()
                {
                    self.state.pending_resolution = Some(pending);
                    return Err(EngineError::Illegal(
                        "declining a cast cannot include an announcement",
                    ));
                }
                if method == SpellCastMethod::Madness
                    && self
                        .state
                        .objects
                        .get(&exiled.object_id)
                        .is_some_and(|o| o.zone == Zone::Exile)
                    && self
                        .state
                        .zone_change_generation
                        .get(&exiled.object_id)
                        .copied()
                        == Some(exiled.zone_change_generation)
                {
                    let owner = self.state.objects[&exiled.object_id].owner;
                    resolution::move_object_to_zone(
                        &mut self.state,
                        self.registry,
                        exiled.object_id,
                        Zone::Graveyard,
                        None,
                    )?;
                    self.state.discard_reference_successors.insert(
                        (exiled.object_id, exiled.zone_change_generation),
                        self.state.zone_change_generation[&exiled.object_id],
                    );
                    events.push(resolution::permanent_moved_event(
                        &self.state,
                        exiled.object_id,
                        owner,
                        rv1::permanent_moved::Destination::Graveyard,
                    ));
                }
                events.push(ev_log(format!(
                    "P{} declines to cast the offered card.",
                    pending.deciding_player
                )));
            }
            rv1::ResolutionChoiceDecision::CastSpell => {
                if answer.cast_spell.is_some() && answer.spell_cast_announcement.is_some() {
                    self.state.pending_resolution = Some(pending);
                    return Err(EngineError::Illegal(
                        "accepting a cast requires exactly one spell announcement",
                    ));
                }
                if let Some(announcement) = answer.spell_cast_announcement.as_ref() {
                    let player = pending.deciding_player;
                    self.state.pending_resolution = Some(pending);
                    let batch = self.begin_spell_cast(
                        player,
                        &rv1::BeginSpellCast {
                            announcement: Some(announcement.clone()),
                        },
                    )?;
                    self.retire_resolution_prompt_during_child_cast();
                    return Ok(batch);
                }
                let Some(cast) = answer.cast_spell.as_ref() else {
                    self.state.pending_resolution = Some(pending);
                    return Err(EngineError::Illegal(
                        "accepting a cast requires a spell announcement",
                    ));
                };
                let player = pending.deciding_player;
                self.state.pending_resolution = Some(pending);
                match self.cast_spell(player, cast) {
                    Ok(batch) => return Ok(batch),
                    Err(error) => return Err(error),
                }
            }
            _ => {
                self.state.pending_resolution = Some(pending);
                return Err(EngineError::Illegal("cast choice requires cast or decline"));
            }
        }
        if self.finish_special_cast_completion(&completion, false, stack.clone(), &mut events)? {
            return Ok(finish_with_events(self, events));
        }
        self.complete_parked_resolution_with_previous(
            stack.item,
            stack.resume_effect_index,
            stack.previous_result,
            events,
        )
    }

    /// The parent special-cast offer is suspended while its accepted child cast is being paid.
    /// Clearing only its publication identity makes a duplicate parent answer stale; cancellation
    /// republishes the same continuation as a fresh offer.
    fn retire_resolution_prompt_during_child_cast(&mut self) {
        if self
            .state
            .pending_spell_cast
            .as_ref()
            .is_some_and(|pending| pending.resolution_time_offer)
        {
            if let Some(pending) = self.state.pending_resolution.as_mut() {
                pending.resolution_prompt_id = crate::state::ResolutionPromptId::UNASSIGNED;
                pending.resolution_prompt_semantics = None;
            }
        }
    }

    fn finish_special_cast_commander_cleanup(
        &mut self,
        pending: PendingResolution,
        answer: &rv1::SubmitResolutionChoice,
        decision: rv1::ResolutionChoiceDecision,
    ) -> Result<RuledEventBatch, EngineError> {
        let ResolutionContinuation::SpecialCastCommanderCleanup {
            stack,
            library_owner,
            completion,
            cast_committed,
            destinations,
            commander_choice_index,
            mut command_zone_objects,
        } = pending.continuation.clone()
        else {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal(
                "special-cast commander continuation missing",
            ));
        };
        let Some((commander, _)) = destinations.get(commander_choice_index).copied() else {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal(
                "special-cast commander choice no longer matches its cohort",
            ));
        };
        let correct_shape = decision == rv1::ResolutionChoiceDecision::SelectBranch
            && answer.selected_branch_index <= 1
            && answer.chosen_object_ids.is_empty()
            && answer.chosen_player_ids.is_empty()
            && answer.payment.is_none()
            && answer.restricted_mana.is_empty()
            && answer.cast_spell.is_none()
            && answer.spell_cast_announcement.is_none()
            && answer.chosen_combat_defender.is_none();
        let owner_live = self.state.player_idx(library_owner).is_some_and(|index| {
            !self.state.players[index].has_lost
                && self.state.players[index]
                    .declared_commander_object_ids
                    .contains(&commander.object_id)
        });
        if !correct_shape
            || pending.deciding_player != library_owner
            || !owner_live
            || !self.special_cast_reference_is_current(library_owner, &commander)
        {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal(
                "invalid or stale special-cast commander replacement choice",
            ));
        }

        if answer.selected_branch_index == 0 {
            command_zone_objects.push(commander.object_id);
        }
        let mut events = Vec::new();
        if self.continue_special_cast_cleanup(
            SpecialCastCleanupPlan {
                stack: stack.clone(),
                library_owner,
                completion: completion.clone(),
                cast_committed,
                destinations: destinations.clone(),
                next_destination_index: commander_choice_index + 1,
                command_zone_objects,
            },
            &mut events,
        )? {
            return Ok(finish_with_events(self, events));
        }
        self.complete_parked_resolution_with_previous(
            stack.item,
            stack.resume_effect_index,
            stack.previous_result,
            events,
        )
    }

    /// Display names for `oids`, in order (registry lookup, never Oracle).
    fn object_names(&self, oids: &[ObjectId]) -> Vec<String> {
        oids.iter()
            .map(|&oid| object_display_name(&self.state, self.registry, oid))
            .collect()
    }

    /// Close out a parked *primitive* resolution once its choice has been applied.
    ///
    /// CR 608.2: a spell resolves its whole effect list. When the parked effect was not the last
    /// one, `resume_effect_index` says where to pick the list back up — `build_resolution_effects`
    /// re-derives it from the stack item, so nothing had to be stored across the park. Running the
    /// tail is also what emits the closing "resolves." log and seats the spell in the graveyard
    /// (CR 608.2m), which is why the `finish_*` callers do not log that themselves.
    ///
    /// Priority returns to the active player only if the tail did not park again (a second
    /// suspending effect in the same list, e.g. a hypothetical `[Scry, ChooseHandCards]`).
    pub(super) fn complete_parked_resolution(
        &mut self,
        item: StackItem,
        resume_effect_index: Option<u32>,
        ev: Vec<rv1::RuledEvent>,
    ) -> Result<RuledEventBatch, EngineError> {
        self.complete_parked_resolution_with_previous(
            item,
            resume_effect_index,
            EffectResult::default(),
            ev,
        )
    }

    fn finish_permanent_choice(
        &mut self,
        pending: PendingResolution,
        chosen: &[ObjectId],
    ) -> Result<RuledEventBatch, EngineError> {
        let (stack, candidate_generations) = match &pending.continuation {
            ResolutionContinuation::PermanentChoice {
                stack,
                candidate_generations,
            } => (stack.clone(), candidate_generations.clone()),
            _ => unreachable!("permanent-choice continuation"),
        };
        let mut produced_objects = Vec::with_capacity(chosen.len());
        for oid in chosen {
            let Some(expected_generation) = candidate_generations
                .iter()
                .find_map(|(candidate, generation)| (candidate == oid).then_some(*generation))
            else {
                self.state.pending_resolution = Some(pending);
                return Err(EngineError::Illegal("invalid permanent choice"));
            };
            let current_generation = self
                .state
                .zone_change_generation
                .get(oid)
                .copied()
                .unwrap_or(0);
            if current_generation != expected_generation
                || !self
                    .state
                    .objects
                    .get(oid)
                    .is_some_and(|object| object.zone == Zone::Battlefield)
            {
                self.state.pending_resolution = Some(pending);
                return Err(EngineError::Illegal("stale permanent choice"));
            }
            produced_objects.push(TriggerObjectRef {
                object_id: *oid,
                zone_change_generation: expected_generation,
                controller_at_event: self
                    .characteristics(*oid)
                    .map(|characteristics| characteristics.controller)
                    .unwrap_or(pending.deciding_player),
            });
        }
        let names = chosen
            .iter()
            .map(|oid| object_display_name(&self.state, self.registry, *oid))
            .collect::<Vec<_>>();
        let events = vec![ev_log(format!(
            "P{} chooses {}.",
            pending.deciding_player,
            names.join(", ")
        ))];
        self.complete_parked_stack_with_previous(
            stack,
            EffectResult {
                produced_objects,
                ..Default::default()
            },
            events,
        )
    }

    fn select_targeted_player_choice_delegate(
        &mut self,
        mut pending: PendingResolution,
        answer: &rv1::SubmitResolutionChoice,
        decision: rv1::ResolutionChoiceDecision,
    ) -> Result<RuledEventBatch, EngineError> {
        if decision != rv1::ResolutionChoiceDecision::SelectBranch
            || !answer.chosen_object_ids.is_empty()
            || !answer.chosen_player_ids.is_empty()
            || answer.cast_spell.is_some()
            || answer.chosen_combat_defender.is_some()
            || answer.payment.is_some()
            || !answer.restricted_mana.is_empty()
            || answer.spell_cast_announcement.is_some()
        {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal(
                "replacement chooser must select one available player",
            ));
        }
        let (controller, target_player, candidate_slots) = match &pending.continuation {
            ResolutionContinuation::TargetedPlayerPermanentChoice {
                stack,
                target_player,
                stage: PendingTargetedPlayerChoiceStage::ChoosingDelegate { candidates },
                ..
            } => (stack.item.controller, *target_player, candidates.clone()),
            _ => {
                self.state.pending_resolution = Some(pending);
                return Err(EngineError::Illegal(
                    "targeted-player delegate continuation missing",
                ));
            }
        };
        let delegate = candidate_slots
            .get(answer.selected_branch_index as usize)
            .copied()
            .flatten();
        let eligible = resolution::resolution_choice_delegate_candidates(
            &self.state,
            controller,
            target_player,
        );
        let Some(delegate) = delegate.filter(|delegate| eligible.contains(delegate)) else {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal(
                "that replacement chooser is no longer available",
            ));
        };
        let Some(index) = self.state.player_idx(delegate) else {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal(
                "that replacement chooser is no longer available",
            ));
        };
        if self.state.players[index].has_lost {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal(
                "that replacement chooser is no longer available",
            ));
        }
        let old_candidates = match &pending.continuation {
            ResolutionContinuation::TargetedPlayerPermanentChoice {
                candidate_generations,
                ..
            } => candidate_generations.clone(),
            _ => unreachable!("validated targeted-player continuation"),
        };
        let candidates = old_candidates
            .iter()
            .copied()
            .filter(|(object_id, generation)| {
                self.state
                    .zone_change_generation
                    .get(object_id)
                    .copied()
                    .unwrap_or(0)
                    == *generation
                    && self
                        .state
                        .objects
                        .get(object_id)
                        .is_some_and(|object| object.zone == Zone::Battlefield)
            })
            .collect::<Vec<_>>();
        if candidates.is_empty() {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal(
                "no legal permanent remains for the replacement chooser",
            ));
        }
        let candidate_ids = candidates.iter().map(|(object_id, _)| *object_id).collect();
        let ResolutionContinuation::TargetedPlayerPermanentChoice {
            stage,
            candidate_generations,
            ..
        } = &mut pending.continuation
        else {
            unreachable!("validated targeted-player continuation")
        };
        *stage = PendingTargetedPlayerChoiceStage::ChoosingPermanent;
        *candidate_generations = candidates;
        pending.deciding_player = delegate;
        pending.presentation.candidates = candidate_ids;
        pending.presentation.min = 1;
        pending.presentation.max = 1;
        pending.presentation.choice_kind = rv1::ChoiceKind::PermanentObjects;
        pending.presentation.prompt = "Choose a permanent they control.".into();
        self.state.pending_resolution = Some(pending);
        let assigned = ev_log(format!(
            "P{controller} chooses P{delegate} to make P{target_player}'s permanent choice."
        ));
        let event = resolution::targeted_player_permanent_choice_event(self)
            .expect("delegated targeted-player choice remains parked");
        Ok(finish_with_events(self, vec![assigned, event]))
    }

    fn finish_targeted_player_permanent_choice(
        &mut self,
        pending: PendingResolution,
        chosen: &[ObjectId],
    ) -> Result<RuledEventBatch, EngineError> {
        let (stack, target_player, candidate_generations, stage) = match &pending.continuation {
            ResolutionContinuation::TargetedPlayerPermanentChoice {
                stack,
                target_player,
                candidate_generations,
                stage,
                ..
            } => (
                stack.clone(),
                *target_player,
                candidate_generations.clone(),
                stage.clone(),
            ),
            _ => unreachable!("targeted-player permanent-choice continuation"),
        };
        if !matches!(stage, PendingTargetedPlayerChoiceStage::ChoosingPermanent) {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal(
                "replacement chooser must select a player before choosing a permanent",
            ));
        }
        let mut produced_objects = Vec::with_capacity(chosen.len());
        for oid in chosen {
            let Some(expected_generation) = candidate_generations
                .iter()
                .find_map(|(candidate, generation)| (candidate == oid).then_some(*generation))
            else {
                self.state.pending_resolution = Some(pending);
                return Err(EngineError::Illegal("invalid permanent choice"));
            };
            let current_generation = self
                .state
                .zone_change_generation
                .get(oid)
                .copied()
                .unwrap_or(0);
            if current_generation != expected_generation
                || !self
                    .state
                    .objects
                    .get(oid)
                    .is_some_and(|object| object.zone == Zone::Battlefield)
            {
                self.state.pending_resolution = Some(pending);
                return Err(EngineError::Illegal("stale permanent choice"));
            }
            produced_objects.push(TriggerObjectRef {
                object_id: *oid,
                zone_change_generation: expected_generation,
                controller_at_event: self
                    .characteristics(*oid)
                    .map(|characteristics| characteristics.controller)
                    .unwrap_or(target_player),
            });
        }
        let names = chosen
            .iter()
            .map(|oid| object_display_name(&self.state, self.registry, *oid))
            .collect::<Vec<_>>();
        let events = if names.is_empty() {
            Vec::new()
        } else {
            vec![ev_log(format!(
                "P{} chooses {}.",
                pending.deciding_player,
                names.join(", ")
            ))]
        };
        self.complete_parked_stack_with_previous(
            stack,
            EffectResult {
                produced_objects,
                ..Default::default()
            },
            events,
        )
    }

    fn complete_targeted_player_choice_without_selection(
        &mut self,
        pending: PendingResolution,
        message: &str,
    ) -> Result<RuledEventBatch, EngineError> {
        let stack = match &pending.continuation {
            ResolutionContinuation::TargetedPlayerPermanentChoice { stack, .. } => stack.clone(),
            _ => unreachable!("targeted-player permanent-choice continuation"),
        };
        self.complete_parked_stack_with_previous(
            stack,
            EffectResult::default(),
            vec![ev_log(message.to_string())],
        )
    }

    /// Refresh Teferi's parked permanent choice after a player leaves. A living target uses
    /// current characteristics and control; a departed target uses its predeparture LKI cohort.
    pub(in crate::engine) fn refresh_targeted_player_permanent_choice_departure(
        &mut self,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<(), EngineError> {
        let Some(mut pending) = self.state.pending_resolution.take() else {
            return Ok(());
        };
        let (
            stack,
            target_player,
            cohort,
            filter,
            constraints,
            old_candidate_generations,
            old_stage,
        ) = match &pending.continuation {
            ResolutionContinuation::TargetedPlayerPermanentChoice {
                stack,
                target_player,
                cohort,
                filter,
                constraints,
                candidate_generations,
                stage,
            } => (
                stack.clone(),
                *target_player,
                cohort.clone(),
                filter.clone(),
                constraints.clone(),
                candidate_generations.clone(),
                stage.clone(),
            ),
            _ => {
                self.state.pending_resolution = Some(pending);
                return Ok(());
            }
        };
        let controller = stack.item.controller;
        if self
            .state
            .player_idx(controller)
            .is_none_or(|index| self.state.players[index].has_lost)
        {
            self.state.pending_resolution = Some(pending);
            return Ok(());
        }
        let target_live = self
            .state
            .player_idx(target_player)
            .is_some_and(|index| !self.state.players[index].has_lost);
        let cohort = if target_live {
            cohort
        } else {
            self.state
                .departed_targeted_player_control_cohorts
                .get(&target_player)
                .filter(|saved| saved.player_id == target_player)
                .cloned()
                .ok_or(EngineError::ProgramInvariant(
                    "departed targeted player is missing its control cohort",
                ))?
        };
        let current_decider_live = self
            .state
            .player_idx(pending.deciding_player)
            .is_some_and(|index| !self.state.players[index].has_lost);
        let was_delegate_prompt = matches!(
            old_stage,
            PendingTargetedPlayerChoiceStage::ChoosingDelegate { .. }
        );
        let candidates = if target_live {
            resolution::current_targeted_player_permanent_candidates(
                self,
                &stack.item,
                target_player,
                &filter,
                &constraints,
            )
            .into_iter()
            .map(|object_id| {
                (
                    object_id,
                    self.state
                        .zone_change_generation
                        .get(&object_id)
                        .copied()
                        .unwrap_or(0),
                )
            })
            .collect::<Vec<_>>()
        } else {
            resolution::targeted_player_control_cohort_candidates(
                self,
                &stack.item,
                target_player,
                &cohort,
                &filter,
                &constraints,
            )
        };
        if candidates.len() < pending.presentation.min as usize {
            let completed = self.complete_targeted_player_choice_without_selection(
                pending,
                "No legal permanent remained for the targeted player's choice.",
            )?;
            events.extend(completed.events);
            return Ok(());
        }
        if !was_delegate_prompt && target_live && current_decider_live {
            if candidates != old_candidate_generations {
                let candidate_ids = candidates
                    .iter()
                    .map(|(object_id, _)| *object_id)
                    .collect::<Vec<_>>();
                let ResolutionContinuation::TargetedPlayerPermanentChoice {
                    candidate_generations,
                    ..
                } = &mut pending.continuation
                else {
                    unreachable!("validated targeted-player continuation")
                };
                *candidate_generations = candidates;
                pending.presentation.candidates = candidate_ids;
                self.state.pending_resolution = Some(pending);
                events.push(ev_log(format!(
                    "P{target_player}'s permanent choice options have changed."
                )));
                events.push(
                    resolution::targeted_player_permanent_choice_event(self)
                        .expect("refreshed targeted-player choice remains parked"),
                );
            } else {
                self.state.pending_resolution = Some(pending);
            }
            return Ok(());
        }

        let eligible = resolution::resolution_choice_delegate_candidates(
            &self.state,
            controller,
            target_player,
        );
        let mut candidate_slots = match old_stage {
            PendingTargetedPlayerChoiceStage::ChoosingDelegate { candidates } => candidates,
            PendingTargetedPlayerChoiceStage::ChoosingPermanent => Vec::new(),
        };
        for slot in &mut candidate_slots {
            if slot.is_some_and(|candidate| !eligible.contains(&candidate)) {
                *slot = None;
            }
        }
        for candidate in eligible.iter().copied() {
            if !candidate_slots.contains(&Some(candidate)) {
                candidate_slots.push(Some(candidate));
            }
        }
        if eligible.is_empty() {
            let completed = self.complete_targeted_player_choice_without_selection(
                pending,
                "No other player can make the targeted player's permanent choice.",
            )?;
            events.extend(completed.events);
            return Ok(());
        }

        let retain_delegate_prompt = was_delegate_prompt;
        let delegate = if eligible.len() == 1 && !retain_delegate_prompt {
            eligible.first().copied()
        } else {
            None
        };
        let candidate_ids = candidates
            .iter()
            .map(|(object_id, _)| *object_id)
            .collect::<Vec<_>>();
        let ResolutionContinuation::TargetedPlayerPermanentChoice {
            candidate_generations,
            cohort: saved_cohort,
            stage,
            ..
        } = &mut pending.continuation
        else {
            unreachable!("validated targeted-player continuation")
        };
        *candidate_generations = candidates;
        *saved_cohort = cohort;
        if let Some(delegate) = delegate {
            pending.deciding_player = delegate;
            pending.presentation.candidates = candidate_ids;
            pending.presentation.min = 1;
            pending.presentation.max = 1;
            pending.presentation.choice_kind = rv1::ChoiceKind::PermanentObjects;
            pending.presentation.prompt = "Choose a permanent they control.".into();
            *stage = PendingTargetedPlayerChoiceStage::ChoosingPermanent;
            self.state.pending_resolution = Some(pending);
            events.push(ev_log(format!(
                "P{controller} assigns P{delegate} to make P{target_player}'s permanent choice."
            )));
        } else {
            pending.deciding_player = controller;
            pending.presentation.candidates.clear();
            pending.presentation.min = 1;
            pending.presentation.max = 1;
            pending.presentation.choice_kind = rv1::ChoiceKind::ResolutionBranch;
            pending.presentation.prompt = format!(
                "P{controller}: choose a player to make P{target_player}'s permanent choice."
            );
            *stage = PendingTargetedPlayerChoiceStage::ChoosingDelegate {
                candidates: candidate_slots,
            };
            self.state.pending_resolution = Some(pending);
            events.push(ev_log(format!(
                "P{controller}'s replacement chooser options have changed."
            )));
        }
        events.push(
            resolution::targeted_player_permanent_choice_event(self)
                .expect("reassigned targeted-player choice remains parked"),
        );
        Ok(())
    }

    /// Reconcile the captured token cohort after an authoritative departure or
    /// controller change. The first chooser's seat cursor is retained for CR 800.4h delegation.
    pub(in crate::engine) fn refresh_equipment_receipt_attach_departure(
        &mut self,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<(), EngineError> {
        let Some(pending) = self.state.pending_resolution.take() else {
            return Ok(());
        };
        let crate::state::ResolutionContinuation::EquipmentReceiptAttach { attachment } =
            pending.continuation
        else {
            self.state.pending_resolution = Some(pending);
            return Ok(());
        };
        let mut attachment = *attachment;
        let source_label = equipment_receipt_source_label(self, &attachment.stack);
        let previous_equipment_controller = attachment.equipment.controller_at_event;
        let Some(equipment) = super::resolution::current_equipment_receipt_source(
            self,
            attachment.equipment.object_id,
            attachment.equipment.zone_change_generation,
        ) else {
            let completed = self.complete_parked_stack_with_previous(
                attachment.stack,
                EffectResult::default(),
                Vec::new(),
            )?;
            events.extend(completed.events);
            return Ok(());
        };
        attachment.equipment = equipment;
        let candidates = super::resolution::current_equipment_receipt_candidates(
            self,
            equipment,
            &attachment.created_tokens,
        );
        if candidates.is_empty() {
            let completed = self.complete_parked_stack_with_previous(
                attachment.stack,
                EffectResult::default(),
                Vec::new(),
            )?;
            events.extend(completed.events);
            return Ok(());
        }
        if let [(candidate, _generation)] = candidates.as_slice() {
            let mut attach_events = Vec::new();
            let _ = super::resolution::attach_equipment_ids(
                self,
                equipment.object_id,
                *candidate,
                &source_label,
                &mut attach_events,
            );
            let completed = self.complete_parked_stack_with_previous(
                attachment.stack,
                EffectResult::default(),
                attach_events,
            )?;
            events.extend(completed.events);
            return Ok(());
        }

        let old_candidates = attachment.candidate_generations.clone();
        attachment.candidate_generations = candidates.clone();
        let mut next_decider = pending.deciding_player;
        let mut next_cursor = attachment.turn_order_cursor;
        let decider_live = self
            .state
            .player_idx(next_decider)
            .is_some_and(|index| !self.state.players[index].has_lost);
        if !decider_live {
            // The captured seat advances only when its offered chooser actually leaves. A
            // controller change while that chooser is live remains an ordinary reoffer.
            attachment.delegated = true;
            if let Some((index, player_id)) =
                self.next_receipt_chooser_after_cursor(attachment.turn_order_cursor)
            {
                next_decider = player_id;
                next_cursor = index;
            }
        } else if equipment.controller_at_event != previous_equipment_controller {
            // A genuine live controller change takes precedence only while the current chooser is
            // still present. It may reoffer a delegated choice and reset the latch; if the chooser
            // departed in this same event, the branch above instead delegates from the old cursor.
            next_decider = equipment.controller_at_event;
            next_cursor =
                self.state
                    .player_idx(next_decider)
                    .ok_or(EngineError::ProgramInvariant(
                        "current Equipment controller is missing from turn order",
                    ))?;
            attachment.delegated = false;
        }
        let candidates_changed = old_candidates != candidates;
        let chooser_changed = next_decider != pending.deciding_player;
        if !candidates_changed && !chooser_changed {
            attachment.turn_order_cursor = next_cursor;
            self.state.pending_resolution = Some(PendingResolution {
                resolution_prompt_id: pending.resolution_prompt_id,
                resolution_prompt_semantics: pending.resolution_prompt_semantics,
                deciding_player: pending.deciding_player,
                presentation: pending.presentation,
                continuation: ResolutionContinuation::EquipmentReceiptAttach {
                    attachment: Box::new(attachment),
                },
            });
            return Ok(());
        }

        attachment.turn_order_cursor = next_cursor;
        if chooser_changed && pending.resolution_prompt_id.get() != 0 {
            // Prompt allocators are normally per chooser. Carry the displaced offer's high-water
            // mark forward so a delegate cannot accidentally accept the prior chooser's ID.
            let prior_id = pending.resolution_prompt_id.get();
            let next_id = self
                .state
                .next_resolution_prompt_id
                .entry(next_decider)
                .or_default();
            *next_id = (*next_id).max(prior_id);
        }
        let mut refreshed = PendingResolution {
            resolution_prompt_id: crate::state::ResolutionPromptId::UNASSIGNED,
            resolution_prompt_semantics: None,
            deciding_player: next_decider,
            presentation: pending.presentation,
            continuation: ResolutionContinuation::EquipmentReceiptAttach {
                attachment: Box::new(attachment),
            },
        };
        refreshed.presentation.candidates =
            candidates.iter().map(|(object_id, _)| *object_id).collect();
        refreshed.presentation.prompt =
            format!("Choose one of the creatures created by {source_label}.");
        self.state.pending_resolution = Some(refreshed);
        events.push(ev_log(if chooser_changed {
            format!("P{next_decider} now makes {source_label}'s Equipment receipt choice.")
        } else {
            format!("{source_label}'s available creature choices have changed.")
        }));
        events.push(
            super::resolution::equipment_receipt_choice_event(self).ok_or(
                EngineError::ProgramInvariant("Equipment receipt choice could not be republished"),
            )?,
        );
        Ok(())
    }

    fn next_receipt_chooser_after_cursor(&self, cursor: usize) -> Option<(usize, PlayerId)> {
        let players = &self.state.players;
        if players.is_empty() {
            return None;
        }
        (1..=players.len()).find_map(|offset| {
            let index = (cursor + offset) % players.len();
            (!players[index].has_lost).then_some((index, players[index].id))
        })
    }

    /// Reevaluate only an in-flight authored permanent-choice leaf after a player departs.
    /// The active frame path and already-bound values remain intact, so no earlier branch or
    /// operation is repeated against the changed battlefield.
    pub(in crate::engine) fn refresh_program_permanent_choice_departure(
        &mut self,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<(), EngineError> {
        let Some(pending) = self.state.pending_resolution.take() else {
            return Ok(());
        };
        let stack = match &pending.continuation {
            ResolutionContinuation::PermanentChoice { stack, .. } if stack.program.is_some() => {
                stack.clone()
            }
            _ => {
                self.state.pending_resolution = Some(pending);
                return Ok(());
            }
        };
        let Some(mut program) = stack.program.clone() else {
            self.state.pending_resolution = Some(pending);
            return Ok(());
        };
        let controller_live = self
            .state
            .player_idx(stack.item.controller)
            .is_some_and(|index| !self.state.players[index].has_lost);
        let current_effect = if controller_live {
            match self.current_program_perform(&program) {
                Ok(effect) => Some(effect),
                Err(error) => {
                    self.state.pending_resolution = Some(pending);
                    return Err(error);
                }
            }
        } else {
            None
        };
        if !matches!(
            current_effect,
            Some(SpellEffectKind::ChoosePermanents {
                chooser: PlayerRecipient::Controller,
                ..
            })
        ) {
            self.state.pending_resolution = Some(pending);
            return Ok(());
        }

        let Some(next_instruction) = program
            .frames
            .last()
            .and_then(|frame| frame.next_instruction.checked_sub(1))
        else {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::ProgramInvariant(
                "resolution program choice cursor is invalid",
            ));
        };
        program
            .frames
            .last_mut()
            .expect("active program frame checked")
            .next_instruction = next_instruction;
        program.in_flight = None;
        let mut retry = stack;
        retry.program = Some(program);
        self.state.pending_resolution = None;
        let completed =
            self.complete_parked_stack_with_previous(retry, EffectResult::default(), Vec::new())?;
        events.extend(completed.events);
        Ok(())
    }

    fn finish_behold_choice(
        &mut self,
        pending: PendingResolution,
        chosen: &[ObjectId],
    ) -> Result<RuledEventBatch, EngineError> {
        let (stack, candidate_generations, hand_candidates, hand_filter, permanent_filter) =
            match &pending.continuation {
                ResolutionContinuation::BeholdChoice {
                    stack,
                    candidate_generations,
                    hand_candidates,
                    hand_filter,
                    permanent_filter,
                } => (
                    stack.clone(),
                    candidate_generations.clone(),
                    hand_candidates.clone(),
                    hand_filter.clone(),
                    permanent_filter.clone(),
                ),
                _ => unreachable!("behold-choice continuation"),
            };
        if stack.program.is_none() {
            return Err(EngineError::ProgramInvariant(
                "non-program Behold continuation reached its retired resume path",
            ));
        }
        let item = stack.item.clone();
        let mut events = Vec::new();

        if chosen.is_empty() {
            events.push(ev_log(format!(
                "P{} declines to behold.",
                pending.deciding_player
            )));
        } else {
            let oid = chosen[0];
            let Some(expected_generation) = candidate_generations
                .iter()
                .find_map(|(candidate, generation)| (*candidate == oid).then_some(*generation))
            else {
                self.state.pending_resolution = Some(pending);
                return Err(EngineError::Illegal("invalid Behold choice"));
            };
            let current_generation = self
                .state
                .zone_change_generation
                .get(&oid)
                .copied()
                .unwrap_or(0);
            let is_hand_candidate = hand_candidates.contains(&oid);
            let is_current = current_generation == expected_generation
                && if is_hand_candidate {
                    self.state.objects.get(&oid).is_some_and(|object| {
                        object.owner == pending.deciding_player
                            && object.zone == Zone::Hand
                            && card_predicates::zone_card_matches_filter(
                                &self.state,
                                self.registry,
                                oid,
                                Some(&hand_filter),
                            )
                    })
                } else {
                    let source = TargetSourceIdentity::for_stack_item(self, &item);
                    targeting::permanent_choice_filter_legal(
                        self,
                        &permanent_filter,
                        oid,
                        pending.deciding_player,
                        source,
                        item.trigger_context,
                    )
                };
            if !is_current {
                self.state.pending_resolution = Some(pending);
                return Err(EngineError::Illegal(
                    "Behold choice became stale or no longer matches",
                ));
            }
            let name = object_display_name(&self.state, self.registry, oid);
            if is_hand_candidate {
                events.extend(super::reveals::reveal_cards(
                    &self.state,
                    self.registry,
                    &[oid],
                    item.id,
                    &object_display_name(&self.state, self.registry, item.id),
                ));
                events.push(ev_log(format!(
                    "P{} reveals {name}.",
                    pending.deciding_player
                )));
            } else {
                events.push(ev_log(format!(
                    "P{} beholds {name}.",
                    pending.deciding_player
                )));
            }
            self.execute_program_behold_untap(&stack, &mut events)?;
        }

        let previous_result = stack.previous_result.clone();
        self.complete_parked_stack_with_previous(stack, previous_result, events)
    }

    pub(super) fn complete_parked_resolution_with_previous(
        &mut self,
        item: StackItem,
        resume_effect_index: Option<u32>,
        previous_result: EffectResult,
        ev: Vec<rv1::RuledEvent>,
    ) -> Result<RuledEventBatch, EngineError> {
        if self.is_pending_linked_exile_play_root(&item) {
            return self.resume_pending_linked_exile_play_root(
                item,
                resume_effect_index,
                previous_result,
                ev,
            );
        }
        let (effects, spell_label) = if resume_effect_index.is_some() {
            self.build_resolution_effects(&item)
        } else {
            (Vec::new(), String::new())
        };
        self.finish_parked_resolution_with_effects_and_previous(
            item,
            resume_effect_index,
            effects,
            spell_label,
            previous_result,
            ev,
        )
    }

    /// Resume a standard effect list from an already captured list. Observer restoration uses
    /// this when it must replace the exact chooser that already began resolving, while preserving
    /// that entry's target bindings and keeping the normal tail and cleanup path.
    pub(super) fn complete_parked_resolution_with_effects_and_previous(
        &mut self,
        item: StackItem,
        resume_effect_index: Option<u32>,
        effects: Vec<ResolutionEffect>,
        spell_label: String,
        previous_result: EffectResult,
        ev: Vec<rv1::RuledEvent>,
    ) -> Result<RuledEventBatch, EngineError> {
        if self.is_pending_linked_exile_play_root(&item) {
            return self.resume_pending_linked_exile_play_root(
                item,
                resume_effect_index,
                previous_result,
                ev,
            );
        }
        self.finish_parked_resolution_with_effects_and_previous(
            item,
            resume_effect_index,
            effects,
            spell_label,
            previous_result,
            ev,
        )
    }

    fn finish_parked_resolution_with_effects_and_previous(
        &mut self,
        item: StackItem,
        resume_effect_index: Option<u32>,
        effects: Vec<ResolutionEffect>,
        spell_label: String,
        previous_result: EffectResult,
        mut ev: Vec<rv1::RuledEvent>,
    ) -> Result<RuledEventBatch, EngineError> {
        if let Some(start) = resume_effect_index {
            let progress = self.run_effect_list_with_previous(
                &item,
                &spell_label,
                effects,
                start as usize,
                previous_result,
                &mut ev,
            )?;
            if progress == super::resolution::ResolutionProgress::GameEnded {
                return Ok(finish_with_events(self, ev));
            }
        }
        if self.state.pending_resolution.is_none() {
            // The original pass-priority call deliberately skipped SBAs while this primitive was
            // parked. Run them only after the resumed effect tail, before granting priority.
            self.apply_sbas(&mut ev)?;
        }
        if self.state.pending_resolution.is_none() {
            if let Some(i) = self.state.player_idx(self.state.active_player_id()) {
                self.state.priority_idx = i;
            }
            ev.push(ev_priority_changed(self));
        }
        Ok(finish_with_events(self, ev))
    }

    fn is_pending_linked_exile_play_root(&self, item: &StackItem) -> bool {
        self.state
            .pending_linked_exile_play
            .as_ref()
            .is_some_and(|cohort| {
                cohort.stack.item.id == item.id
                    && cohort.stack.item.source_zone_change == item.source_zone_change
            })
    }

    fn resume_pending_linked_exile_play_root(
        &mut self,
        item: StackItem,
        resume_effect_index: Option<u32>,
        previous_result: EffectResult,
        ev: Vec<rv1::RuledEvent>,
    ) -> Result<RuledEventBatch, EngineError> {
        let mut cohort = *self
            .state
            .pending_linked_exile_play
            .take()
            .expect("linked play cohort checked above");
        cohort.stack.item = item;
        cohort.stack.resume_effect_index = resume_effect_index;
        cohort.stack.previous_result = previous_result;
        self.advance_pending_linked_exile_play_through_stack(cohort, ev)
    }

    /// Resume a standard effect list or a finite authored program using the exact parked cursor.
    /// Program continuations keep their immutable definition in the registry and carry only the
    /// entered frame path, slot values, and operation awaiting its domain completion.
    pub(super) fn complete_parked_stack_with_previous(
        &mut self,
        stack: ParkedStackResolution,
        previous_result: EffectResult,
        mut ev: Vec<rv1::RuledEvent>,
    ) -> Result<RuledEventBatch, EngineError> {
        let Some(program) = stack.program.clone() else {
            return self.complete_parked_resolution_with_previous(
                stack.item,
                stack.resume_effect_index,
                previous_result,
                ev,
            );
        };
        let token_copy_batch_in_flight = program.in_flight.as_ref().is_some_and(|outputs| {
            self.registry
                .compiled_program_for_owner(&program.owner)
                .is_some_and(|compiled| {
                    outputs.iter().any(|slot_id| {
                        compiled.slot(*slot_id).is_some_and(|slot| {
                            matches!(
                                slot.result_type,
                                tricerules_card_model::primitives::ResultType::TokenCopyBatch
                            )
                        })
                    })
                })
        });
        let mut operation_result = program.in_flight.as_ref().map(|_| previous_result);
        if token_copy_batch_in_flight {
            if let Some(result) = operation_result.as_mut() {
                // The copy executor's provenance marker was parked by EffectCx before the
                // replacement/entry choice. Pair it with the committed generation-bound refs;
                // output capture still rejects a missing marker.
                result.produced_object_provenance = program.in_flight_produced_object_provenance;
            }
        }
        let progress = self.resume_program_with_completion(
            &stack.item,
            program,
            stack.previous_result,
            operation_result,
            &mut ev,
        )?;
        if progress == super::resolution::ResolutionProgress::GameEnded {
            return Ok(finish_with_events(self, ev));
        }
        if self.state.pending_resolution.is_none() {
            self.apply_sbas(&mut ev)?;
        }
        if self.state.pending_resolution.is_none() {
            if let Some(i) = self.state.player_idx(self.state.active_player_id()) {
                self.state.priority_idx = i;
            }
            ev.push(ev_priority_changed(self));
        }
        Ok(finish_with_events(self, ev))
    }
}

#[cfg(test)]
mod card_name_set_tests {
    use super::extend_disjoint_card_names;
    use std::collections::HashSet;

    #[test]
    fn different_multiface_labels_reject_overlapping_name_sets() {
        let split_display = "Fire // Ice";
        let room_display = "Ice // Water";
        assert_ne!(split_display, room_display);

        let mut names = HashSet::new();
        assert!(extend_disjoint_card_names(
            &mut names,
            ["Fire".to_owned(), "Ice".to_owned()]
        ));
        assert!(!extend_disjoint_card_names(
            &mut names,
            ["Ice".to_owned(), "Water".to_owned()]
        ));
        assert_eq!(names, HashSet::from(["Fire".to_owned(), "Ice".to_owned()]));

        let mut disjoint = HashSet::new();
        assert!(extend_disjoint_card_names(
            &mut disjoint,
            ["Fire".to_owned(), "Ice".to_owned()]
        ));
        assert!(extend_disjoint_card_names(
            &mut disjoint,
            ["Water".to_owned(), "Earth".to_owned()]
        ));
    }
}

#[cfg(test)]
mod resolution_payment_actor_tests {
    use super::*;

    #[test]
    fn issue_169_resolution_tap_payment_uses_decider_not_stack_controller() {
        let mut engine = GameEngine::new(
            tricerules_cards::registry::global(),
            169_021,
            &[0, 1],
            20,
            None,
            true,
        )
        .unwrap();

        let sentry = engine.state.players[0].hand[0];
        engine.state.objects.get_mut(&sentry).unwrap().card_id = "icewrought_sentry".into();
        move_object_to_zone(
            &mut engine.state,
            engine.registry,
            sentry,
            Zone::Battlefield,
            Some(0),
        )
        .unwrap();

        let payer_creature = engine.state.players[1].hand[0];
        engine
            .state
            .objects
            .get_mut(&payer_creature)
            .unwrap()
            .card_id = "grizzly_bears".into();
        move_object_to_zone(
            &mut engine.state,
            engine.registry,
            payer_creature,
            Zone::Battlefield,
            Some(1),
        )
        .unwrap();

        let mut item = engine.observer_return_item(sentry, 0);
        item.id = engine.state.next_object_id;
        item.card_id = "grizzly_bears".into();
        item.ability_text = None;
        item.source_permanent_id = Some(sentry);
        item.source_owner = Some(0);
        item.source_zone_change = engine.state.zone_change_generation[&sentry];
        let stack_item_id = item.id;
        let mut stack = ParkedStackResolution::new(item.clone());
        stack.resume_effect_index = Some(1);
        engine.state.stack.push(item);

        let cost = ResolutionCost::TapPermanents {
            count: 1,
            filter: TargetFilter {
                kind: TargetKind::Creature,
                controller: TargetController::You,
                ..Default::default()
            },
            exclude_source: false,
        };
        let candidate_generation = engine.state.zone_change_generation[&payer_creature];
        let pending = PendingResolution {
            resolution_prompt_id: crate::state::ResolutionPromptId::UNASSIGNED,
            resolution_prompt_semantics: None,
            deciding_player: 1,
            presentation: PendingResolutionPresentation {
                source_object_id: stack_item_id,
                candidates: vec![payer_creature],
                min: 1,
                max: 1,
                ordered: false,
                prompt: "P1 chooses a creature to tap.".into(),
                choice_kind: rv1::ChoiceKind::ResolutionBranch,
                unique_names: false,
            },
            continuation: ResolutionContinuation::AuthoredBranch {
                stack,
                branch: PendingResolutionBranch {
                    optional: false,
                    chooser: PlayerRecipient::TargetedPlayer {
                        group_index: 0,
                        kind: TargetKind::OpponentPlayer,
                    },
                    original_chooser: 1,
                    branches: vec![tricerules_card_model::primitives::ResolutionBranchDef {
                        branch_id: tricerules_card_model::ChoiceId::new("tap_a_creature").unwrap(),
                        presentation: tricerules_card_model::AbilityPresentation::Fallback,
                        runtime_fallback: None,
                        cost,
                        requirement:
                            tricerules_card_model::primitives::ResolutionBranchRequirement::Always,
                        effects: Vec::new(),
                    }],
                    stage: PendingResolutionBranchStage::PayingObjects {
                        selected_branch: 0,
                        candidate_generations: vec![(payer_creature, candidate_generation)],
                    },
                },
            },
        };

        assert_eq!(engine.state.stack.last().unwrap().controller, 0);
        let completed = engine
            .finish_resolution_branch_object(pending, &[payer_creature])
            .expect("P1 pays the parked resolution branch controlled by P0");

        assert!(engine.state.objects[&payer_creature].tapped);
        assert!(completed.events.iter().any(|event| matches!(
            &event.ev,
            Some(rv1::ruled_event::Ev::Log(log)) if log.text == "P1 taps Grizzly Bears."
        )));
        assert_eq!(engine.characteristics(sentry).unwrap().power, Some(2));
        assert!(engine.state.pending_triggers.is_empty());
        assert!(engine.state.staged_trigger_groups.is_empty());
        assert!(engine.state.stack.is_empty());
    }
}

#[cfg(test)]
mod equipment_receipt_reoffer_tests {
    use super::*;
    use crate::state::{
        AffectedScope, ContinuousEffect, ParkedStackResolution, PendingEquipmentReceiptAttach,
        PendingResolution, PendingResolutionPresentation, ResolutionContinuation,
        ResolutionPromptId, TriggerObjectRef,
    };
    use tricerules_cards::primitives::{ContinuousEffectKind, ControllerReference, EffectDuration};

    fn pending_receipt_fixture(seed: u64) -> (GameEngine, ObjectId, Vec<ObjectId>) {
        let mut engine = GameEngine::new(
            tricerules_cards::registry::global(),
            seed,
            &[10, 20, 30, 40],
            20,
            None,
            true,
        )
        .expect("four-player engine");
        let p1_hand = engine.state.players[0].hand.clone();
        let equipment = p1_hand[0];
        engine.state.objects.get_mut(&equipment).unwrap().card_id = "kyoshi_battle_fan".into();
        move_object_to_zone(
            &mut engine.state,
            engine.registry,
            equipment,
            Zone::Battlefield,
            Some(20),
        )
        .expect("move Equipment under P2's control");
        let mut candidates = Vec::new();
        for candidate in p1_hand.into_iter().skip(1).take(2) {
            engine.state.objects.get_mut(&candidate).unwrap().card_id = "grizzly_bears".into();
            move_object_to_zone(
                &mut engine.state,
                engine.registry,
                candidate,
                Zone::Battlefield,
                Some(10),
            )
            .expect("move a legal creature receipt member");
            candidates.push(candidate);
        }
        let equipment_generation = engine.state.zone_change_generation[&equipment];
        let equipment_ref = TriggerObjectRef {
            object_id: equipment,
            zone_change_generation: equipment_generation,
            controller_at_event: 20,
        };
        let created_tokens = candidates
            .iter()
            .map(|object_id| TriggerObjectRef {
                object_id: *object_id,
                zone_change_generation: engine.state.zone_change_generation[object_id],
                controller_at_event: 10,
            })
            .collect::<Vec<_>>();
        let mut item = engine.observer_return_item(equipment, 10);
        item.id = engine.state.next_object_id;
        item.card_id = "kyoshi_battle_fan".into();
        item.is_triggered = true;
        item.source_permanent_id = Some(equipment);
        item.source_owner = Some(10);
        item.source_zone_change = equipment_generation;
        let stack = ParkedStackResolution::new(item);
        let candidate_generations = candidates
            .iter()
            .map(|object_id| (*object_id, engine.state.zone_change_generation[object_id]))
            .collect::<Vec<_>>();
        let pending = PendingResolution {
            resolution_prompt_id: ResolutionPromptId::UNASSIGNED,
            resolution_prompt_semantics: None,
            deciding_player: 20,
            presentation: PendingResolutionPresentation {
                source_object_id: stack.item.id,
                candidates: candidates.clone(),
                min: 1,
                max: 1,
                ordered: false,
                prompt: "Choose a created Ally.".into(),
                choice_kind: rv1::ChoiceKind::PermanentObjects,
                unique_names: false,
            },
            continuation: ResolutionContinuation::EquipmentReceiptAttach {
                attachment: Box::new(PendingEquipmentReceiptAttach {
                    stack,
                    equipment: equipment_ref,
                    created_tokens,
                    candidate_generations,
                    original_offered_chooser: 20,
                    delegated: false,
                    turn_order_cursor: 1,
                }),
            },
        };
        engine.state.pending_resolution = Some(pending);
        (engine, equipment, candidates)
    }

    fn set_equipment_controller(
        engine: &mut GameEngine,
        equipment: ObjectId,
        controller: PlayerId,
    ) {
        engine.state.continuous_effects.retain(
            |effect| !matches!(effect.affected, AffectedScope::Single(id) if id == equipment),
        );
        let base_controller = engine.state.objects[&equipment].base_controller;
        if controller != base_controller {
            engine.state.continuous_effects.push(ContinuousEffect {
                trigger_grant_origin: None,
                source_id: None,
                affected: AffectedScope::Single(equipment),
                kind: ContinuousEffectKind::Layer2Control {
                    controller: ControllerReference::Fixed(controller),
                },
                condition: None,
                duration: EffectDuration::UntilEndOfTurn,
                timestamp: engine.state.command_index,
            });
        }
    }

    fn pending_receipt(engine: &GameEngine) -> &PendingEquipmentReceiptAttach {
        match &engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .continuation
        {
            ResolutionContinuation::EquipmentReceiptAttach { attachment } => attachment,
            _ => panic!("Equipment receipt continuation remains parked"),
        }
    }

    #[test]
    fn live_equipment_controller_changes_reoffer_until_a_chooser_departure_delegates() {
        let (mut engine, equipment, _) = pending_receipt_fixture(507_220);

        set_equipment_controller(&mut engine, equipment, 30);
        engine
            .refresh_equipment_receipt_attach_departure(&mut Vec::new())
            .expect("first live control change reoffers the choice");
        assert_eq!(
            engine
                .state
                .pending_resolution
                .as_ref()
                .unwrap()
                .deciding_player,
            30
        );
        assert!(!pending_receipt(&engine).delegated);
        assert_eq!(pending_receipt(&engine).turn_order_cursor, 2);

        set_equipment_controller(&mut engine, equipment, 40);
        engine
            .refresh_equipment_receipt_attach_departure(&mut Vec::new())
            .expect("a second live control change reoffers the choice again");
        assert_eq!(
            engine
                .state
                .pending_resolution
                .as_ref()
                .unwrap()
                .deciding_player,
            40
        );
        assert!(!pending_receipt(&engine).delegated);
        assert_eq!(pending_receipt(&engine).turn_order_cursor, 3);

        set_equipment_controller(&mut engine, equipment, 20);
        engine.state.players[3].has_lost = true;
        engine
            .refresh_equipment_receipt_attach_departure(&mut Vec::new())
            .expect("the departed chooser delegates after Equipment returns to P2");
        assert_eq!(
            engine
                .state
                .pending_resolution
                .as_ref()
                .unwrap()
                .deciding_player,
            10
        );
        assert!(pending_receipt(&engine).delegated);
        assert_eq!(pending_receipt(&engine).turn_order_cursor, 0);
    }

    #[test]
    fn departed_live_controller_delegates_to_next_seat_even_after_source_reverts() {
        let (mut engine, equipment, _) = pending_receipt_fixture(507_221);

        set_equipment_controller(&mut engine, equipment, 30);
        engine
            .refresh_equipment_receipt_attach_departure(&mut Vec::new())
            .expect("live control change reoffers to P3");
        assert_eq!(
            engine
                .state
                .pending_resolution
                .as_ref()
                .unwrap()
                .deciding_player,
            30
        );
        assert!(!pending_receipt(&engine).delegated);

        engine.state.players[2].has_lost = true;
        set_equipment_controller(&mut engine, equipment, 20);
        engine
            .refresh_equipment_receipt_attach_departure(&mut Vec::new())
            .expect("P3 departure delegates from P3's captured seat despite source reversion");
        assert_eq!(
            engine
                .state
                .pending_resolution
                .as_ref()
                .unwrap()
                .deciding_player,
            40
        );
        assert!(pending_receipt(&engine).delegated);
        assert_eq!(pending_receipt(&engine).turn_order_cursor, 3);
    }

    #[test]
    fn delegated_offer_stays_with_delegate_until_a_new_live_controller_takes_control() {
        let (mut engine, equipment, _) = pending_receipt_fixture(507_222);
        let p1 = engine.state.players[0].id;
        let p2 = engine.state.players[1].id;
        let p3 = engine.state.players[2].id;
        let p4 = engine.state.players[3].id;

        // This seam models an Equipment owned by P1 and currently controlled by P3. The initial
        // offer therefore belongs to P3; P3's departure returns control to P1 and delegates to P4.
        engine.state.players[1]
            .battlefield
            .retain(|object_id| *object_id != equipment);
        engine.state.players[0].battlefield.push(equipment);
        let object = engine.state.objects.get_mut(&equipment).unwrap();
        object.base_controller = p1;
        object.controller = p1;
        set_equipment_controller(&mut engine, equipment, p3);
        let pending = engine.state.pending_resolution.as_mut().unwrap();
        pending.deciding_player = p3;
        if let ResolutionContinuation::EquipmentReceiptAttach { attachment } =
            &mut pending.continuation
        {
            attachment.equipment.controller_at_event = p3;
            attachment.original_offered_chooser = p3;
            attachment.turn_order_cursor = 2;
        }

        engine.state.players[2].has_lost = true;
        set_equipment_controller(&mut engine, equipment, p1);
        engine
            .refresh_equipment_receipt_attach_departure(&mut Vec::new())
            .expect("P3 departure delegates to P4 as the Equipment returns to P1");
        assert_eq!(
            engine
                .state
                .pending_resolution
                .as_ref()
                .unwrap()
                .deciding_player,
            p4
        );
        assert!(pending_receipt(&engine).delegated);
        assert_eq!(pending_receipt(&engine).turn_order_cursor, 3);

        engine
            .refresh_equipment_receipt_attach_departure(&mut Vec::new())
            .expect("unchanged P1 control does not recapture P4's live offer");
        assert_eq!(
            engine
                .state
                .pending_resolution
                .as_ref()
                .unwrap()
                .deciding_player,
            p4
        );
        assert!(pending_receipt(&engine).delegated);

        set_equipment_controller(&mut engine, equipment, p2);
        engine
            .refresh_equipment_receipt_attach_departure(&mut Vec::new())
            .expect("a new live P2 controller takes the choice back from P4");
        assert_eq!(
            engine
                .state
                .pending_resolution
                .as_ref()
                .unwrap()
                .deciding_player,
            p2
        );
        assert!(!pending_receipt(&engine).delegated);
        assert_eq!(pending_receipt(&engine).turn_order_cursor, 1);
    }

    #[test]
    fn receipt_answer_rejects_wrong_actor_malformed_and_nonmember_without_mutation() {
        let (mut engine, equipment, candidates) = pending_receipt_fixture(507_223);
        let original_prompt = engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .resolution_prompt_id;
        let original_candidates = pending_receipt(&engine).candidate_generations.clone();
        let original_cursor = pending_receipt(&engine).turn_order_cursor;
        let assert_unchanged = |engine: &GameEngine| {
            assert!(engine.state.objects[&equipment].attached_to.is_none());
            assert_eq!(
                engine
                    .state
                    .pending_resolution
                    .as_ref()
                    .unwrap()
                    .deciding_player,
                20
            );
            assert_eq!(
                engine
                    .state
                    .pending_resolution
                    .as_ref()
                    .unwrap()
                    .resolution_prompt_id,
                original_prompt
            );
            assert_eq!(
                pending_receipt(engine).candidate_generations,
                original_candidates
            );
            assert_eq!(pending_receipt(engine).turn_order_cursor, original_cursor);
        };

        let before_wrong_actor = engine.diagnostic_snapshot().unwrap();
        let wrong_actor = engine.finish_equipment_receipt_attach_choice(
            10,
            &rv1::SubmitResolutionChoice {
                chosen_object_ids: vec![candidates[0]],
                ..Default::default()
            },
        );
        assert!(wrong_actor.is_err());
        assert_eq!(engine.diagnostic_snapshot().unwrap(), before_wrong_actor);
        assert_unchanged(&engine);

        let before_malformed = engine.diagnostic_snapshot().unwrap();
        let malformed = engine.finish_equipment_receipt_attach_choice(
            20,
            &rv1::SubmitResolutionChoice {
                chosen_object_ids: vec![candidates[0]],
                chosen_player_ids: vec![10],
                ..Default::default()
            },
        );
        assert!(malformed.is_err());
        assert_eq!(engine.diagnostic_snapshot().unwrap(), before_malformed);
        assert_unchanged(&engine);

        let before_nonmember = engine.diagnostic_snapshot().unwrap();
        let nonmember = engine.finish_equipment_receipt_attach_choice(
            20,
            &rv1::SubmitResolutionChoice {
                chosen_object_ids: vec![equipment],
                ..Default::default()
            },
        );
        assert!(nonmember.is_err());
        assert_eq!(engine.diagnostic_snapshot().unwrap(), before_nonmember);
        assert_unchanged(&engine);
    }

    #[test]
    fn one_remaining_exact_receipt_member_auto_attaches_after_reconciliation() {
        let (mut engine, equipment, candidates) = pending_receipt_fixture(507_224);
        move_object_to_zone(
            &mut engine.state,
            engine.registry,
            candidates[0],
            Zone::Graveyard,
            None,
        )
        .expect("one receipt creature leaves the battlefield");

        engine
            .refresh_equipment_receipt_attach_departure(&mut Vec::new())
            .expect("one exact live receipt candidate attaches automatically");
        assert!(engine.state.pending_resolution.is_none());
        assert_eq!(
            engine.state.objects[&equipment].attached_to,
            Some(AttachmentRecipient::Object(candidates[1]))
        );
        assert_eq!(engine.state.objects[&candidates[0]].zone, Zone::Graveyard);
        assert_eq!(engine.state.objects[&candidates[1]].zone, Zone::Battlefield);
    }

    #[test]
    fn stale_or_new_source_incarnation_completes_without_attachment() {
        for (seed, return_to_battlefield) in [(507_225, false), (507_226, true)] {
            let (mut engine, equipment, candidates) = pending_receipt_fixture(seed);
            move_object_to_zone(
                &mut engine.state,
                engine.registry,
                equipment,
                Zone::Graveyard,
                None,
            )
            .expect("source leaves its original battlefield incarnation");
            if return_to_battlefield {
                move_object_to_zone(
                    &mut engine.state,
                    engine.registry,
                    equipment,
                    Zone::Battlefield,
                    Some(20),
                )
                .expect("the same object ID returns as a new Equipment incarnation");
            }

            engine
                .refresh_equipment_receipt_attach_departure(&mut Vec::new())
                .expect("stale source receipt is completed without attachment");
            assert!(engine.state.pending_resolution.is_none());
            assert!(engine.state.objects[&equipment].attached_to.is_none());
            assert!(candidates.iter().all(|object_id| {
                engine.state.objects[object_id].zone == Zone::Battlefield
                    && engine.state.objects[object_id].attached_to.is_none()
            }));
        }
    }
}
