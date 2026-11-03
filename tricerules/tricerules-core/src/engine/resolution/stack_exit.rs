//! Exact terminal replacement selection and commit for completed stack resolutions.
//!
//! The continuation in this module is terminal: it holds the completed stack receipt and the
//! exit transaction, then commits one final destination without resuming an authored effect.

use super::events::{ev_log, ev_priority_changed, finish_with_events};
use super::*;
use crate::state::{
    ParkedStackResolution, PendingResolution, PendingResolutionPresentation, PendingStackExit,
    ReplacementSourcePresentation, ScheduledRebound, StackExitReason,
    StackExitReplacementApplication, StackExitReplacementKind, StackExitTail, StackObjectRef,
    TriggerObjectRef,
};

#[derive(Debug, Clone, PartialEq, Eq)]
struct StackExitReplacementDraft {
    source: TriggerObjectRef,
    kind: StackExitReplacementKind,
    presentation: ReplacementSourcePresentation,
}

impl GameEngine {
    /// Prepare a final stack exit, parking only when multiple currently-applicable replacements
    /// compete. The caller supplies its exact completed continuation so an exit choice never
    /// rebuilds a cursor from only the visible StackItem.
    pub(super) fn finish_deferred_stack_exit_with_context(
        &mut self,
        stack: ParkedStackResolution,
        exit: DeferredStackExit,
        tail: StackExitTail,
        spell_label: String,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<ResolutionProgress, EngineError> {
        let exit_reason = match exit {
            DeferredStackExit::Resolved => StackExitReason::Resolved,
            DeferredStackExit::DidNotResolve => StackExitReason::DidNotResolve,
        };
        let drafts = self.stack_exit_replacement_drafts(&stack.item, exit_reason);
        if let Some(pending) = self.state.pending_resolution.as_ref() {
            // An unrelated parentless copy may exit beside this choice, but this path must not
            // replace it with a second terminal prompt.
            let allow_unrelated_copy_exit = match &pending.continuation {
                crate::state::ResolutionContinuation::CopyTargets { progress, .. } => {
                    progress.parent.is_none()
                        && progress.stack.item.id != stack.item.id
                        && progress
                            .remaining
                            .iter()
                            .all(|remaining| remaining.item.id != stack.item.id)
                        && drafts.len() <= 1
                }
                _ => false,
            };
            if !allow_unrelated_copy_exit {
                return Err(EngineError::ProgramInvariant(
                    "terminal stack exit began while another resolution choice was pending",
                ));
            }
        }
        let stack_ref = self.stack_exit_reference(&stack.item);
        let physical_owner = (!stack.item.is_copy)
            .then(|| {
                self.state
                    .objects
                    .get(&stack.item.id)
                    .map(|object| object.owner)
            })
            .flatten();

        if drafts.len() > 1 {
            let offered_applications = self.allocate_stack_exit_replacement_applications(drafts)?;
            let transaction = PendingStackExit {
                stack: stack_ref,
                controller: stack.item.controller,
                physical_owner,
                exit: exit_reason,
                spell_label,
                tail,
                offered_applications,
                selected_option: None,
            };
            self.park_stack_exit_replacement_choice(stack, transaction, events)?;
            return Ok(ResolutionProgress::Parked);
        }

        let selected_option = drafts.first().map(|draft| draft.kind);
        let transaction = PendingStackExit {
            stack: stack_ref,
            controller: stack.item.controller,
            physical_owner,
            exit: exit_reason,
            spell_label,
            tail,
            offered_applications: Vec::new(),
            selected_option,
        };
        self.commit_stack_exit_and_tail(stack, transaction, events)
    }

    fn stack_exit_replacement_drafts(
        &self,
        item: &StackItem,
        exit: StackExitReason,
    ) -> Vec<StackExitReplacementDraft> {
        if exit != StackExitReason::Resolved || item.ability_text.is_some() {
            return Vec::new();
        }

        let physical_stack_is_current = self.stack_backing_object_is_current(item);
        let virtual_copy_is_resolving = item.is_copy && !self.state.objects.contains_key(&item.id);
        if !physical_stack_is_current && !virtual_copy_is_resolving {
            // An effect may already have moved the physical card. Keep the established final
            // destination/event behavior, but no new replacement applies to that departure.
            return Vec::new();
        }

        let source = self.stack_exit_source_reference(item);
        let presentation = self.stack_exit_source_presentation(item, source);
        let mut drafts = Vec::new();

        if self
            .registry
            .get(&item.card_id)
            .is_some_and(|definition| definition.layout == Layout::Omen && item.face_index == 1)
        {
            drafts.push(StackExitReplacementDraft {
                source,
                kind: StackExitReplacementKind::Omen,
                presentation: presentation.clone(),
            });
        }
        if self.registry.get(&item.card_id).is_some_and(|definition| {
            definition.layout == Layout::Adventure && item.face_index == 1
        }) {
            drafts.push(StackExitReplacementDraft {
                source,
                kind: StackExitReplacementKind::Adventure,
                presentation: presentation.clone(),
            });
        }
        if self.rebound_replacement_is_current(item) {
            drafts.push(StackExitReplacementDraft {
                source,
                kind: StackExitReplacementKind::Rebound,
                presentation,
            });
        }
        drafts
    }

    fn rebound_replacement_is_current(&self, item: &StackItem) -> bool {
        if item.is_copy || item.ability_text.is_some() || item.cast_method.exiles_on_leave_stack() {
            return false;
        }
        let Some(occurrence) = item
            .cast_occurrence
            .filter(|occurrence| occurrence.object_id == item.id)
        else {
            return false;
        };
        let Some(generation) = occurrence.zone_change_generation else {
            return false;
        };
        let key = (occurrence.object_id, Some(generation));
        let Some(holder) = self.state.hand_cast_holders.get(&key).copied() else {
            return false;
        };
        let Some(face) = self
            .registry
            .get(&item.card_id)
            .and_then(|definition| definition.face(item.face_index))
        else {
            return false;
        };
        let is_instant_or_sorcery = face
            .types
            .iter()
            .any(|kind| kind == "Instant" || kind == "Sorcery");
        let has_printed_rebound = face.spell_keywords.contains(&SpellKeyword::Rebound);
        let has_exact_external_rebound_grant = self.state.rebound_spell_grants.contains(&key);
        // Resolution has popped this item from `state.stack`; the captured terminal item and its
        // exact physical Stack incarnation are the authority through completion and parking.
        is_instant_or_sorcery
            && item.cast_by == Some(item.controller)
            && holder == item.controller
            && (has_printed_rebound || has_exact_external_rebound_grant)
            && self.stack_backing_object_is_current(item)
    }

    fn stack_exit_reference(&self, item: &StackItem) -> StackObjectRef {
        let zone_change_generation = match item.cast_occurrence {
            Some(occurrence) if occurrence.object_id == item.id => {
                occurrence.zone_change_generation
            }
            Some(_) => None,
            None => self
                .state
                .objects
                .get(&item.id)
                .filter(|object| object.zone == Zone::Stack)
                .and_then(|_| self.state.zone_change_generation.get(&item.id).copied()),
        };
        StackObjectRef {
            object_id: item.id,
            zone_change_generation,
        }
    }

    fn stack_exit_source_reference(&self, item: &StackItem) -> TriggerObjectRef {
        let stack_ref = self.stack_exit_reference(item);
        let generation = stack_ref.zone_change_generation.unwrap_or_else(|| {
            self.state
                .zone_change_generation
                .get(&item.id)
                .copied()
                .unwrap_or(0)
        });
        TriggerObjectRef {
            object_id: item.id,
            zone_change_generation: generation,
            controller_at_event: item.controller,
        }
    }

    fn stack_exit_source_presentation(
        &self,
        item: &StackItem,
        source: TriggerObjectRef,
    ) -> ReplacementSourcePresentation {
        let card_name = self
            .registry
            .get(&item.card_id)
            .map(|definition| definition.name.clone())
            .unwrap_or_else(|| item.card_id.clone());
        ReplacementSourcePresentation {
            card_name,
            object_id: source.object_id,
            zone_change_generation: source.zone_change_generation,
        }
    }

    fn stack_backing_object_is_current(&self, item: &StackItem) -> bool {
        let Some(object) = self.state.objects.get(&item.id) else {
            return false;
        };
        if object.zone != Zone::Stack {
            return false;
        }
        self.stack_exit_reference(item)
            .zone_change_generation
            .is_some_and(|generation| {
                self.state
                    .zone_change_generation
                    .get(&item.id)
                    .copied()
                    .unwrap_or(0)
                    == generation
            })
    }

    fn stack_item_is_current(&self, item: &StackItem) -> bool {
        let reference = self.stack_exit_reference(item);
        self.state.stack.iter().any(|current| {
            current.id == item.id
                && current.controller == item.controller
                && current.cast_by == item.cast_by
                && current.card_id == item.card_id
                && current.face_index == item.face_index
                && current.is_copy == item.is_copy
                && current.ability_text == item.ability_text
                && self.stack_exit_reference(current) == reference
        })
    }

    fn allocate_stack_exit_replacement_applications(
        &mut self,
        drafts: Vec<StackExitReplacementDraft>,
    ) -> Result<Vec<StackExitReplacementApplication>, EngineError> {
        let count = u32::try_from(drafts.len())
            .map_err(|_| EngineError::Illegal("too many stack-exit replacements"))?;
        let first_id = self.state.next_replacement_application_id;
        if first_id == 0 {
            return Err(EngineError::ProgramInvariant(
                "stack-exit replacement application id is zero",
            ));
        }
        let next_id = first_id.checked_add(count).ok_or(EngineError::Illegal(
            "stack-exit replacement application id exhausted",
        ))?;
        let mut applications = Vec::with_capacity(drafts.len());
        for (offset, draft) in drafts.into_iter().enumerate() {
            let offset = u32::try_from(offset)
                .map_err(|_| EngineError::Illegal("too many stack-exit replacements"))?;
            applications.push(StackExitReplacementApplication {
                application_id: first_id.checked_add(offset).ok_or(EngineError::Illegal(
                    "stack-exit replacement application id exhausted",
                ))?,
                source: draft.source,
                kind: draft.kind,
                presentation: draft.presentation,
            });
        }
        self.state.next_replacement_application_id = next_id;
        Ok(applications)
    }

    fn park_stack_exit_replacement_choice(
        &mut self,
        stack: ParkedStackResolution,
        transaction: PendingStackExit,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<(), EngineError> {
        if transaction.offered_applications.len() < 2
            || transaction.exit != StackExitReason::Resolved
        {
            return Err(EngineError::ProgramInvariant(
                "stack-exit replacement prompt lacks multiple successful applications",
            ));
        }
        let prompt = format!(
            "{}: choose how it leaves the stack.",
            transaction.spell_label
        );
        let candidates = transaction
            .offered_applications
            .iter()
            .map(|application| application.application_id)
            .collect::<Vec<_>>();
        self.state.pending_resolution = Some(PendingResolution {
            resolution_prompt_id: crate::state::ResolutionPromptId::UNASSIGNED,
            resolution_prompt_semantics: None,
            deciding_player: transaction.controller,
            presentation: PendingResolutionPresentation {
                source_object_id: stack.item.id,
                candidates,
                min: 1,
                max: 1,
                ordered: false,
                prompt: prompt.clone(),
                choice_kind: rv1::ChoiceKind::ReplacementEffect,
                unique_names: false,
            },
            continuation: crate::state::ResolutionContinuation::StackExitReplacement {
                stack,
                transaction,
            },
        });
        events.push(self.stack_exit_replacement_choice_event().ok_or(
            EngineError::ProgramInvariant("stack-exit replacement offer could not be rendered"),
        )?);
        events.push(ev_log(prompt));
        Ok(())
    }

    pub(super) fn stack_exit_replacement_choice_event(&self) -> Option<rv1::RuledEvent> {
        let pending = self.state.pending_resolution.as_ref()?;
        let crate::state::ResolutionContinuation::StackExitReplacement { stack, transaction } =
            &pending.continuation
        else {
            return None;
        };
        let replacement_options = transaction
            .offered_applications
            .iter()
            .map(|application| rv1::ReplacementEffectOption {
                application_id: application.application_id,
                source_card_name: application.presentation.card_name.clone(),
                effect_summary: stack_exit_replacement_summary(application.kind).to_owned(),
                source_object_id: application.source.object_id,
                source_zone_change_generation: application.source.zone_change_generation,
            })
            .collect::<Vec<_>>();
        let candidate_names = transaction
            .offered_applications
            .iter()
            .map(|application| {
                format!(
                    "{} — {}",
                    application.presentation.card_name,
                    stack_exit_replacement_summary(application.kind)
                )
            })
            .collect::<Vec<_>>();
        Some(rv1::RuledEvent {
            ev: Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(
                rv1::ResolutionChoiceRequired {
                    resolution_prompt_id: pending.resolution_prompt_id.get(),
                    deciding_player_id: pending.deciding_player,
                    source_object_id: stack.item.id,
                    prompt_text: pending.presentation.prompt.clone(),
                    choice_kind: rv1::ChoiceKind::ReplacementEffect as i32,
                    candidate_object_ids: pending.presentation.candidates.clone(),
                    candidate_names,
                    candidate_selectable: vec![true; transaction.offered_applications.len()],
                    replacement_options,
                    min: 1,
                    max: 1,
                    ..Default::default()
                },
            )),
        })
    }

    fn commit_stack_exit_and_tail(
        &mut self,
        stack: ParkedStackResolution,
        transaction: PendingStackExit,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<ResolutionProgress, EngineError> {
        let progress = self.commit_stack_exit(stack.clone(), &transaction, events)?;
        if progress != ResolutionProgress::Completed {
            return Ok(progress);
        }
        if transaction.tail == StackExitTail::ResolveLogAndGraveyardSeat
            && transaction.exit == StackExitReason::Resolved
        {
            events.push(ev_log(format!("{} resolves.", transaction.spell_label)));
            seat_resolved_spell_last_in_graveyard(&mut self.state, stack.item.id);
        }
        Ok(ResolutionProgress::Completed)
    }

    fn commit_stack_exit(
        &mut self,
        stack: ParkedStackResolution,
        transaction: &PendingStackExit,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<ResolutionProgress, EngineError> {
        let top = &stack.item;
        if transaction.stack != self.stack_exit_reference(top)
            || transaction.controller != top.controller
            || transaction.physical_owner
                != (!top.is_copy)
                    .then(|| self.state.objects.get(&top.id).map(|object| object.owner))
                    .flatten()
        {
            return Err(EngineError::Illegal(
                "stack-exit transaction no longer identifies the exact spell",
            ));
        }
        if transaction.selected_option == Some(StackExitReplacementKind::Rebound)
            && !self.rebound_replacement_is_current(top)
        {
            return Err(EngineError::Illegal(
                "Rebound no longer applies to this exact stack spell",
            ));
        }

        let has_stack_item = self.stack_item_is_current(top);
        let has_stack_object = self.stack_backing_object_is_current(top);
        let is_omen_spell = self
            .registry
            .get(&top.card_id)
            .is_some_and(|definition| definition.layout == Layout::Omen && top.face_index == 1);
        let is_adventure_spell = top.ability_text.is_none()
            && self.registry.get(&top.card_id).is_some_and(|definition| {
                definition.layout == Layout::Adventure && top.face_index == 1
            });
        let is_resolved_omen = transaction.exit == StackExitReason::Resolved && is_omen_spell;
        let is_resolved_adventure =
            transaction.exit == StackExitReason::Resolved && is_adventure_spell;
        let is_resolved_self_exiling_copy = transaction.exit == StackExitReason::Resolved
            && top.is_copy
            && self
                .build_resolution_effects(top)
                .0
                .iter()
                .any(|entry| matches!(entry.effect, SpellEffectKind::ExileResolvingSpell));
        let selected_option = transaction.selected_option.or({
            if is_resolved_omen {
                Some(StackExitReplacementKind::Omen)
            } else if is_resolved_adventure {
                Some(StackExitReplacementKind::Adventure)
            } else {
                None
            }
        });
        if selected_option == Some(StackExitReplacementKind::Rebound) && !has_stack_object {
            return Err(EngineError::Illegal(
                "Rebound stack object already left its exact stack incarnation",
            ));
        }

        let has_deferred_exit = has_stack_item
            || has_stack_object
            || top.ability_text.is_some()
            || (top.is_copy && !self.state.objects.contains_key(&top.id));
        self.clear_rebound_grant_for_stack_item(top);
        if !has_deferred_exit {
            return Ok(if self.state.is_terminal() {
                ResolutionProgress::GameEnded
            } else {
                ResolutionProgress::Completed
            });
        }

        let rebound_record_data = if selected_option == Some(StackExitReplacementKind::Rebound) {
            let occurrence = top
                .cast_occurrence
                .filter(|occurrence| occurrence.object_id == top.id)
                .ok_or(EngineError::ProgramInvariant(
                    "selected Rebound exit lost its committed cast occurrence",
                ))?;
            let source_generation =
                occurrence
                    .zone_change_generation
                    .ok_or(EngineError::ProgramInvariant(
                        "selected Rebound exit has no physical generation",
                    ))?;
            let definition =
                self.registry
                    .get(&top.card_id)
                    .ok_or(EngineError::ProgramInvariant(
                        "selected Rebound spell definition disappeared",
                    ))?;
            Some((
                TriggerObjectRef {
                    object_id: occurrence.object_id,
                    zone_change_generation: source_generation,
                    controller_at_event: transaction.controller,
                },
                top.card_id.clone(),
                definition.name.clone(),
                top.face_index,
                self.state
                    .stack_presentations
                    .get(&top.id)
                    .and_then(|presentation| presentation.primary.clone()),
            ))
        } else {
            None
        };

        if has_stack_item {
            self.state.stack.retain(|candidate| candidate.id != top.id);
        }
        if let Some(key) = top.trigger_context.memory_jar_cohort {
            self.state.memory_jar_cohorts.remove(&key);
        }
        self.state.captured_activated_abilities.remove(&top.id);
        let released_state_trigger = self
            .state
            .millennium_calendar_state_trigger_stack_keys
            .remove(&top.id)
            .is_some();

        let physical_owner = transaction.physical_owner;
        let destination = if selected_option == Some(StackExitReplacementKind::Omen) {
            rv1::StackResolveDestination::Library
        } else if top.cast_method.exiles_on_leave_stack()
            || selected_option == Some(StackExitReplacementKind::Adventure)
            || selected_option == Some(StackExitReplacementKind::Rebound)
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
            let zone = if selected_option == Some(StackExitReplacementKind::Omen) {
                Zone::Library
            } else if top.cast_method.exiles_on_leave_stack()
                || selected_option == Some(StackExitReplacementKind::Adventure)
                || selected_option == Some(StackExitReplacementKind::Rebound)
            {
                Zone::Exile
            } else {
                Zone::Graveyard
            };
            move_object_to_zone(&mut self.state, self.registry, top.id, zone, None)?;
            if selected_option == Some(StackExitReplacementKind::Adventure) {
                self.grant_adventure_play_permission(top)?;
            }
            if let Some((source, card_id, card_name, source_face_index, presentation)) =
                rebound_record_data
            {
                let zone_change_generation = self
                    .state
                    .zone_change_generation
                    .get(&top.id)
                    .copied()
                    .ok_or(EngineError::ProgramInvariant(
                        "Rebound exile did not record a post-move generation",
                    ))?;
                if self
                    .state
                    .objects
                    .get(&top.id)
                    .is_none_or(|object| object.zone != Zone::Exile)
                {
                    return Err(EngineError::ProgramInvariant(
                        "Rebound replacement did not leave its card in exile",
                    ));
                }
                self.state.scheduled_rebounds.push(ScheduledRebound {
                    exiled: TriggerObjectRef {
                        object_id: top.id,
                        zone_change_generation,
                        controller_at_event: transaction.controller,
                    },
                    source,
                    controller: transaction.controller,
                    card_id,
                    card_name,
                    source_face_index,
                    presentation,
                });
            }
        }

        if selected_option == Some(StackExitReplacementKind::Omen) {
            let shuffle_player = physical_owner.unwrap_or(transaction.controller);
            shuffle_player_library_for_current_command(&mut self.state, shuffle_player);
            events.push(ev_log(format!(
                "P{} shuffles the resolving Omen into P{}'s library.",
                transaction.controller, shuffle_player
            )));
        }
        if released_state_trigger {
            self.check_millennium_calendar_state_triggers();
        }
        Ok(if self.state.is_terminal() {
            ResolutionProgress::GameEnded
        } else {
            ResolutionProgress::Completed
        })
    }

    pub(super) fn grant_adventure_play_permission(
        &mut self,
        top: &StackItem,
    ) -> Result<(), EngineError> {
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
                crate::state::ExilePlayPermissionScope::CastFace(0),
                false,
            ),
        )?;
        Ok(())
    }

    pub(in crate::engine) fn finish_stack_exit_replacement_choice(
        &mut self,
        pending: PendingResolution,
        answer: &rv1::SubmitResolutionChoice,
        decision: rv1::ResolutionChoiceDecision,
    ) -> Result<RuledEventBatch, EngineError> {
        let (stack, mut transaction) = match &pending.continuation {
            crate::state::ResolutionContinuation::StackExitReplacement { stack, transaction } => {
                (stack.clone(), transaction.clone())
            }
            _ => {
                self.state.pending_resolution = Some(pending);
                return Err(EngineError::ProgramInvariant(
                    "stack-exit replacement handler received another continuation",
                ));
            }
        };
        let expected_ids = transaction
            .offered_applications
            .iter()
            .map(|application| application.application_id)
            .collect::<Vec<_>>();
        let answer_is_exact = decision == rv1::ResolutionChoiceDecision::Unspecified
            && pending.resolution_prompt_id.get() != 0
            && answer.resolution_prompt_id == pending.resolution_prompt_id.get()
            && pending.presentation.choice_kind == rv1::ChoiceKind::ReplacementEffect
            && pending.presentation.source_object_id == stack.item.id
            && pending.presentation.candidates == expected_ids
            && pending.presentation.min == 1
            && pending.presentation.max == 1
            && !pending.presentation.ordered
            && !pending.presentation.unique_names
            && answer.chosen_object_ids.len() == 1
            && answer.chosen_player_ids.is_empty()
            && answer.selected_branch_index == 0
            && answer.cast_spell.is_none()
            && answer.spell_cast_announcement.is_none()
            && answer.chosen_combat_defender.is_none()
            && answer.payment.is_none()
            && answer.restricted_mana.is_empty();
        if !answer_is_exact {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal(
                "choose exactly one published stack-exit replacement",
            ));
        }
        let selected = transaction
            .offered_applications
            .iter()
            .find(|application| application.application_id == answer.chosen_object_ids[0])
            .cloned();
        let Some(selected) = selected else {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal(
                "choose one published stack-exit replacement",
            ));
        };
        if transaction.selected_option.is_some()
            || transaction.exit != StackExitReason::Resolved
            || transaction.stack != self.stack_exit_reference(&stack.item)
            || transaction.controller != stack.item.controller
            || transaction.physical_owner
                != (!stack.item.is_copy)
                    .then(|| {
                        self.state
                            .objects
                            .get(&stack.item.id)
                            .map(|object| object.owner)
                    })
                    .flatten()
            || self.state.pending_replacement_event.is_some()
        {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal(
                "stack-exit replacement offer is stale",
            ));
        }
        let current_drafts = self.stack_exit_replacement_drafts(&stack.item, transaction.exit);
        let exact_applications_are_current = current_drafts.len()
            == transaction.offered_applications.len()
            && current_drafts
                .iter()
                .zip(&transaction.offered_applications)
                .all(|(draft, application)| {
                    draft.source == application.source
                        && draft.kind == application.kind
                        && draft.presentation == application.presentation
                });
        if !exact_applications_are_current || !self.stack_backing_object_is_current(&stack.item) {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal(
                "stack-exit replacement applicability or stack incarnation changed",
            ));
        }
        transaction.selected_option = Some(selected.kind);
        let mut events = Vec::new();
        let progress = self.commit_stack_exit_and_tail(stack, transaction, &mut events)?;
        if progress == ResolutionProgress::Parked {
            return Ok(finish_with_events(self, events));
        }
        if progress == ResolutionProgress::GameEnded || self.state.is_terminal() {
            return Ok(finish_with_events(self, events));
        }
        if self.state.pending_resolution.is_none() {
            self.apply_sbas(&mut events)?;
        }
        if self.state.pending_resolution.is_none() && !self.state.is_terminal() {
            if let Some(index) = self.state.player_idx(self.state.active_player_id()) {
                self.state.priority_idx = index;
            }
            events.push(ev_priority_changed(self));
        }
        Ok(finish_with_events(self, events))
    }
}

fn stack_exit_replacement_summary(kind: StackExitReplacementKind) -> &'static str {
    match kind {
        StackExitReplacementKind::Rebound => "Exile it with Rebound.",
        StackExitReplacementKind::Adventure => "Exile it with its Adventure permission.",
        StackExitReplacementKind::Omen => "Shuffle it into its owner's library.",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::ScenarioCommandExt;
    use tricerules_proto::ruled::v1 as rv1;

    fn actual_opt_hand_casts(count: usize) -> (GameEngine, Vec<StackItem>) {
        let mut engine = GameEngine::new(
            tricerules_cards::registry::global(),
            6_110_201,
            &[0, 1],
            20,
            Some(vec![
                vec!["opt".to_owned(); 20],
                vec!["island".to_owned(); 20],
            ]),
            true,
        )
        .expect("prepared Rebound test engine");
        engine.state.opening = None;
        engine.state.turn_step = TurnStep::Main1;
        engine.state.priority_idx = 0;
        engine.state.players[0].mana_pool.blue = count as u32;

        for _ in 0..count {
            engine.state.priority_idx = 0;
            engine
                .apply_scenario_command(
                    0,
                    &rv1::RuledCommand {
                        cmd: Some(rv1::ruled_command::Cmd::CastSpell(rv1::CastSpell {
                            cast_method: rv1::CastMethod::Normal as i32,
                            source: Some(rv1::CastSource {
                                location: Some(rv1::cast_source::Location::HandIndex(0)),
                                ..Default::default()
                            }),
                            ..Default::default()
                        })),
                    },
                )
                .expect("commit an actual Opt from hand");
        }

        let items = engine.state.stack.clone();
        assert_eq!(items.len(), count);
        for item in &items {
            assert_eq!(item.card_id, "opt");
            assert!(!item.is_copy);
            assert_eq!(item.cast_method, crate::state::SpellCastMethod::Normal);
            assert_eq!(item.cast_by, Some(item.controller));
            let occurrence = item
                .cast_occurrence
                .expect("actual Opt retains its committed cast occurrence");
            let generation = occurrence
                .zone_change_generation
                .expect("physical cast occurrences carry a stack generation");
            let key = (occurrence.object_id, Some(generation));
            assert_eq!(occurrence.object_id, item.id);
            assert_eq!(engine.state.objects[&item.id].zone, Zone::Stack);
            assert_eq!(engine.state.zone_change_generation[&item.id], generation);
            assert_eq!(
                engine.state.hand_cast_holders.get(&key).copied(),
                Some(item.controller),
                "the real hand cast records its exact holder receipt"
            );
        }
        (engine, items)
    }

    fn actual_skimming_strike_face_one_hand_casts() -> (GameEngine, Vec<StackItem>) {
        let card_id = "dirgur_island_dragon_skimming_strike";
        let mut engine = GameEngine::new(
            tricerules_cards::registry::global(),
            6_110_202,
            &[0, 1],
            20,
            Some(vec![
                vec![card_id.to_owned(); 20],
                vec!["island".to_owned(); 20],
            ]),
            true,
        )
        .expect("prepared Skimming Strike test engine");
        engine.state.opening = None;
        engine.state.turn_step = TurnStep::Main1;
        engine.state.priority_idx = 0;
        // Prime the normal startup view; the separately prepared terminal offer is published below.
        let _initial_batch = engine.initial_response_batch();
        engine.state.players[0].mana_pool.blue = 2;
        engine.state.players[0].mana_pool.colorless = 2;

        for _ in 0..2 {
            engine.state.priority_idx = 0;
            engine
                .apply_scenario_command(
                    0,
                    &rv1::RuledCommand {
                        cmd: Some(rv1::ruled_command::Cmd::CastSpell(rv1::CastSpell {
                            cast_method: rv1::CastMethod::Normal as i32,
                            face_index: 1,
                            source: Some(rv1::CastSource {
                                location: Some(rv1::cast_source::Location::HandIndex(0)),
                                ..Default::default()
                            }),
                            ..Default::default()
                        })),
                    },
                )
                .expect("commit an actual zero-target Skimming Strike face cast");
        }

        let items = engine.state.stack.clone();
        assert_eq!(items.len(), 2);
        for item in &items {
            assert_eq!(item.card_id, card_id);
            assert_eq!(item.face_index, 1);
            assert_eq!(item.cast_method, crate::state::SpellCastMethod::Normal);
            assert!(item.targets.is_empty());
            assert!(!item.is_copy);
            assert_eq!(item.cast_by, Some(item.controller));
            let occurrence = item
                .cast_occurrence
                .expect("actual Skimming Strike cast retains its occurrence");
            let generation = occurrence
                .zone_change_generation
                .expect("physical cast occurrences carry a stack generation");
            let key = (occurrence.object_id, Some(generation));
            assert_eq!(occurrence.object_id, item.id);
            assert_eq!(engine.state.objects[&item.id].zone, Zone::Stack);
            assert_eq!(engine.state.zone_change_generation[&item.id], generation);
            assert_eq!(
                engine.state.hand_cast_holders.get(&key).copied(),
                Some(item.controller),
                "the real hand cast records its exact holder receipt"
            );
        }
        (engine, items)
    }

    #[test]
    fn prepared_rebound_eligibility_and_exact_cleanup() {
        let (mut engine, items) = actual_opt_hand_casts(2);
        let spell = items[0].clone();
        let other_live_spell = items[1].clone();
        let occurrence = spell
            .cast_occurrence
            .expect("actual Opt has its exact cast occurrence");
        let current_generation = occurrence
            .zone_change_generation
            .expect("physical cast occurrence has its stack generation");
        let current_key = (occurrence.object_id, Some(current_generation));
        let other_occurrence = other_live_spell
            .cast_occurrence
            .expect("the other actual Opt retains its cast occurrence");
        let other_key = (
            other_occurrence.object_id,
            other_occurrence.zone_change_generation,
        );
        assert_ne!(current_key, other_key);

        // Prepared receipt state exercises the exact predicate; it does not claim legal-session
        // reachability for a second hand cast or the completed Rebound lifecycle.
        engine.state.rebound_spell_grants.insert(current_key);
        assert!(engine.rebound_replacement_is_current(&spell));

        let saved_holder = engine
            .state
            .hand_cast_holders
            .remove(&current_key)
            .expect("the actual cast automatically recorded its holder");
        assert!(!engine.rebound_replacement_is_current(&spell));
        engine
            .state
            .hand_cast_holders
            .insert(current_key, saved_holder);
        assert!(engine.rebound_replacement_is_current(&spell));

        engine.state.hand_cast_holders.insert(current_key, 1);
        assert!(!engine.rebound_replacement_is_current(&spell));
        engine
            .state
            .hand_cast_holders
            .insert(current_key, spell.controller);
        assert!(engine.rebound_replacement_is_current(&spell));

        let mut wrong_caster = spell.clone();
        wrong_caster.cast_by = Some(1);
        assert!(!engine.rebound_replacement_is_current(&wrong_caster));
        assert!(engine.rebound_replacement_is_current(&spell));

        let mut wrong_current_controller = spell.clone();
        wrong_current_controller.controller = 1;
        assert!(!engine.rebound_replacement_is_current(&wrong_current_controller));
        assert!(engine.rebound_replacement_is_current(&spell));

        let stale_occurrence = crate::state::StackObjectRef {
            object_id: occurrence.object_id,
            zone_change_generation: Some(
                current_generation
                    .checked_sub(1)
                    .expect("hand-to-stack movement gives Opt a prior generation"),
            ),
        };
        let stale_key = (
            stale_occurrence.object_id,
            stale_occurrence.zone_change_generation,
        );
        let mut stale_item = spell.clone();
        stale_item.cast_occurrence = Some(stale_occurrence);
        engine
            .state
            .hand_cast_holders
            .insert(stale_key, spell.controller);
        engine.state.rebound_spell_grants.insert(stale_key);
        assert!(engine.state.objects[&spell.id].zone == Zone::Stack);
        assert_eq!(
            engine.state.zone_change_generation[&spell.id], current_generation,
            "the stale receipt does not change the physical stack generation"
        );
        assert!(!engine.rebound_replacement_is_current(&stale_item));

        engine.state.rebound_spell_grants.insert(other_key);
        engine.clear_rebound_grant_for_stack_item(&stale_item);
        assert!(!engine.state.rebound_spell_grants.contains(&stale_key));
        assert!(!engine.state.hand_cast_holders.contains_key(&stale_key));
        assert!(engine.state.rebound_spell_grants.contains(&current_key));
        assert_eq!(
            engine.state.hand_cast_holders.get(&current_key).copied(),
            Some(spell.controller)
        );
        assert!(engine.state.rebound_spell_grants.contains(&other_key));
        assert!(engine.state.hand_cast_holders.contains_key(&other_key));
        assert!(engine
            .state
            .stack
            .iter()
            .any(|item| item.id == other_live_spell.id));
        assert!(engine.rebound_replacement_is_current(&spell));
    }

    #[test]
    fn prepared_terminal_park_preserves_receipts_until_commit() {
        let (mut engine, items) = actual_skimming_strike_face_one_hand_casts();
        let older_spell = items[0].clone();
        let top_spell = items[1].clone();
        let older_occurrence = older_spell
            .cast_occurrence
            .expect("older actual Skimming Strike has its cast occurrence");
        let top_occurrence = top_spell
            .cast_occurrence
            .expect("top actual Skimming Strike has its cast occurrence");
        let older_key = (
            older_occurrence.object_id,
            older_occurrence.zone_change_generation,
        );
        let top_key = (
            top_occurrence.object_id,
            top_occurrence.zone_change_generation,
        );
        assert_ne!(older_key, top_key);
        engine.state.rebound_spell_grants.insert(older_key);
        engine.state.rebound_spell_grants.insert(top_key);

        // Resolver ownership is prepared by popping only the model StackItem; the physical card
        // remains on the Stack until terminal commit. No authored Tap or Draw effect is executed.
        let popped = engine
            .state
            .stack
            .pop()
            .expect("the newest actual cast is the resolving top item");
        assert_eq!(popped.id, top_spell.id);
        assert_eq!(engine.state.objects[&top_spell.id].zone, Zone::Stack);
        assert_eq!(engine.state.stack.len(), 1);
        assert_eq!(engine.state.stack[0].id, older_spell.id);

        let mut events = Vec::new();
        let progress = engine
            .finish_deferred_stack_exit_with_context(
                ParkedStackResolution::dedicated(top_spell.clone()),
                DeferredStackExit::Resolved,
                StackExitTail::ExitOnly,
                "Skimming Strike".to_owned(),
                &mut events,
            )
            .expect("a prepared completed Omen/Rebound exit parks for a choice");
        assert_eq!(progress, ResolutionProgress::Parked);
        engine
            .publish_resolution_prompt_ids(&mut events)
            .expect("publish the exact prepared terminal replacement offer");

        let (prompt_id, offered_ids, rebound_application_id) = {
            let pending = engine
                .state
                .pending_resolution
                .as_ref()
                .expect("the terminal replacement choice is pending");
            let crate::state::ResolutionContinuation::StackExitReplacement { stack, transaction } =
                &pending.continuation
            else {
                panic!("the pending choice owns the exact terminal stack exit");
            };
            assert_eq!(stack.item.id, top_spell.id);
            assert_eq!(transaction.offered_applications.len(), 2);
            let rebound_application_id = transaction
                .offered_applications
                .iter()
                .find(|application| application.kind == StackExitReplacementKind::Rebound)
                .expect("the published options include the current Rebound application")
                .application_id;
            let offered_ids = transaction
                .offered_applications
                .iter()
                .map(|application| application.application_id)
                .collect::<Vec<_>>();
            assert_eq!(pending.presentation.candidates, offered_ids);
            (
                pending.resolution_prompt_id.get(),
                offered_ids,
                rebound_application_id,
            )
        };
        assert_ne!(prompt_id, 0, "the current offer has a published nonce");
        assert!(offered_ids.contains(&rebound_application_id));
        assert!(events.iter().any(|event| matches!(
            &event.ev,
            Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(choice))
                if choice.resolution_prompt_id == prompt_id
                    && choice.candidate_object_ids == offered_ids
        )));
        assert!(engine
            .state
            .stack
            .iter()
            .any(|item| item.id == older_spell.id));
        assert_eq!(engine.state.objects[&top_spell.id].zone, Zone::Stack);
        assert!(engine.state.scheduled_rebounds.is_empty());
        assert!(engine.state.hand_cast_holders.contains_key(&older_key));
        assert!(engine.state.hand_cast_holders.contains_key(&top_key));
        assert!(engine.state.rebound_spell_grants.contains(&older_key));
        assert!(engine.state.rebound_spell_grants.contains(&top_key));

        // This is prepared cleanup with a live terminal stack exit, not a claimed legal cleanup
        // sequence or a completed game-session reachability proof.
        let holders_before_cleanup = engine.state.hand_cast_holders.clone();
        let grants_before_cleanup = engine.state.rebound_spell_grants.clone();
        engine
            .cleanup_until_end_of_turn_effects()
            .expect("prepared cleanup completes while the stack-exit offer remains live");
        assert_eq!(engine.state.hand_cast_holders, holders_before_cleanup);
        assert_eq!(engine.state.rebound_spell_grants, grants_before_cleanup);
        let pending_after_cleanup = engine
            .state
            .pending_resolution
            .as_ref()
            .expect("prepared cleanup preserves the pending terminal transaction");
        assert_eq!(pending_after_cleanup.resolution_prompt_id.get(), prompt_id);
        assert_eq!(pending_after_cleanup.presentation.candidates, offered_ids);
        assert_eq!(engine.state.objects[&top_spell.id].zone, Zone::Stack);
        assert_eq!(engine.state.stack[0].id, older_spell.id);
        assert!(engine.state.scheduled_rebounds.is_empty());

        engine
            .apply_command(
                top_spell.controller,
                &rv1::RuledCommand {
                    cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(
                        rv1::SubmitResolutionChoice {
                            resolution_prompt_id: prompt_id,
                            chosen_object_ids: vec![rebound_application_id],
                            ..Default::default()
                        },
                    )),
                },
            )
            .expect("answer the currently published Rebound application");

        assert!(engine.state.pending_resolution.is_none());
        assert_eq!(engine.state.objects[&top_spell.id].zone, Zone::Exile);
        assert_eq!(engine.state.stack.len(), 1);
        assert_eq!(engine.state.stack[0].id, older_spell.id);
        assert_eq!(engine.state.objects[&older_spell.id].zone, Zone::Stack);
        assert!(!engine.state.hand_cast_holders.contains_key(&top_key));
        assert!(!engine.state.rebound_spell_grants.contains(&top_key));
        assert!(engine.state.hand_cast_holders.contains_key(&older_key));
        assert!(engine.state.rebound_spell_grants.contains(&older_key));

        assert_eq!(engine.state.scheduled_rebounds.len(), 1);
        let scheduled = &engine.state.scheduled_rebounds[0];
        assert_eq!(scheduled.exiled.object_id, top_spell.id);
        assert_ne!(
            scheduled.exiled.zone_change_generation,
            top_occurrence
                .zone_change_generation
                .expect("physical cast generation")
        );
        assert_eq!(scheduled.exiled.controller_at_event, top_spell.controller);
        assert_eq!(scheduled.source.object_id, top_spell.id);
        assert_eq!(
            scheduled.source.zone_change_generation,
            top_occurrence
                .zone_change_generation
                .expect("physical cast generation")
        );
        assert_eq!(scheduled.controller, top_spell.controller);
        assert_eq!(scheduled.card_id, top_spell.card_id);
        assert_eq!(scheduled.source_face_index, 1);
    }
}
