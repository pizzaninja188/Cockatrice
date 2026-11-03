//! Onyx / Ransack's private hand-and-graveyard partition, committed as one zone batch.
use super::super::events::finish_with_events;
use super::*;
use crate::state::{HandGraveyardLibraryRef, PendingHandGraveyardPartition};

pub(super) fn begin(cx: &mut EffectCx<'_>, count: u32) -> Result<EffectOutcome, EngineError> {
    let Some(index) = cx.engine.state.player_idx(cx.controller) else {
        return Ok(EffectOutcome::Continue);
    };
    let looked: Vec<_> = cx.engine.state.players[index]
        .library
        .iter()
        .take(count as usize)
        .copied()
        .collect();
    if looked.is_empty() {
        return Ok(EffectOutcome::Continue);
    }
    let cohort = looked
        .iter()
        .enumerate()
        .map(|(position, &object_id)| HandGraveyardLibraryRef {
            object_id,
            generation: cx
                .engine
                .state
                .zone_change_generation
                .get(&object_id)
                .copied()
                .unwrap_or(0),
            library_position: position as u32,
        })
        .collect();
    let presentation = zones::publish_library_look(
        cx,
        &looked,
        vec![true; looked.len()],
        1,
        1,
        "Choose one card to put into your hand. You will order the rest into your graveyard."
            .into(),
    );
    let mut stack = cx.parked_stack();
    stack.set_flat_tail(
        cx.effect_index.checked_add(1),
        cx.resume_previous_result.cloned().unwrap_or_default(),
    );
    cx.engine.state.pending_resolution = Some(PendingResolution::new_choice(
        cx.controller,
        presentation,
        ResolutionContinuation::LibraryLook {
            stack,
            candidates: super::super::library_choices::capture(cx.engine, &looked),
            stage: PendingLibraryLookStage::HandGraveyard(PendingHandGraveyardPartition {
                owner: cx.controller,
                count,
                cohort,
                selected_hand: None,
            }),
        },
    ));
    Ok(EffectOutcome::Suspended)
}

impl GameEngine {
    /// Recheck the full original top cohort, including the already chosen hand card. Candidate
    /// lists alone shrink at the second stage and cannot authenticate this transaction.
    pub(in crate::engine) fn validate_hand_graveyard_partition(
        &self,
        pending: &PendingResolution,
    ) -> Result<(), EngineError> {
        let ResolutionContinuation::LibraryLook {
            stack,
            candidates,
            stage: PendingLibraryLookStage::HandGraveyard(partition),
        } = &pending.continuation
        else {
            return Err(EngineError::Illegal("hand/graveyard continuation missing"));
        };
        let index = self
            .state
            .player_idx(partition.owner)
            .filter(|&index| !self.state.players[index].has_lost)
            .ok_or(EngineError::Illegal(
                "hand/graveyard library owner departed",
            ))?;
        let expected: Vec<_> = partition
            .cohort
            .iter()
            .filter(|reference| Some(reference.object_id) != partition.selected_hand)
            .map(|reference| (reference.object_id, reference.generation))
            .collect();
        let expected_count = if partition.selected_hand.is_some() {
            expected.len() as u32
        } else {
            1
        };
        let (effects, _) = self.build_resolution_effects(&stack.item);
        let exact_instruction = stack.resume_effect_index().and_then(|next| next.checked_sub(1))
            .and_then(|index| effects.get(index as usize))
            .is_some_and(|instruction| matches!(instruction.effect,
                SpellEffectKind::LookChooseOneToHandRestToGraveyard { count } if count == partition.count));
        let valid = partition.count > 0
            && !partition.cohort.is_empty()
            && partition.cohort.len() <= partition.count as usize
            && stack.program().is_none()
            && exact_instruction
            && (partition.selected_hand.is_none() || expected.len() >= 2)
            && stack.item.controller == partition.owner
            && pending.deciding_player == partition.owner
            && pending.presentation.source_object_id == stack.item.id
            && pending.presentation.choice_kind == custom::ChoiceKind::LibraryLook
            && pending.presentation.min == expected_count
            && pending.presentation.max == expected_count
            && pending.presentation.ordered == partition.selected_hand.is_some()
            && !pending.presentation.unique_names
            && partition
                .selected_hand
                .is_none_or(|selected| partition.cohort.iter().any(|r| r.object_id == selected))
            && candidates == &expected
            && pending
                .presentation
                .candidates
                .iter()
                .copied()
                .eq(expected.iter().map(|r| r.0))
            && self.state.players[index]
                .library
                .iter()
                .take(partition.count as usize)
                .copied()
                .eq(partition.cohort.iter().map(|reference| reference.object_id))
            && partition
                .cohort
                .iter()
                .enumerate()
                .all(|(position, reference)| {
                    reference.library_position as usize == position
                        && self
                            .state
                            .objects
                            .get(&reference.object_id)
                            .is_some_and(|object| {
                                object.owner == partition.owner && object.zone == Zone::Library
                            })
                        && self
                            .state
                            .zone_change_generation
                            .get(&reference.object_id)
                            .copied()
                            .unwrap_or(0)
                            == reference.generation
                });
        if !valid {
            return Err(EngineError::Illegal(
                "stale hand/graveyard library partition",
            ));
        }
        Ok(())
    }

    pub(in crate::engine) fn finish_hand_graveyard_partition(
        &mut self,
        mut pending: PendingResolution,
        chosen: &[ObjectId],
    ) -> Result<RuledEventBatch, EngineError> {
        if let Err(error) = self.validate_hand_graveyard_partition(&pending) {
            self.state.pending_resolution = Some(pending);
            return Err(error);
        }
        if chosen.len() != pending.presentation.min as usize
            || chosen.iter().enumerate().any(|(index, id)| {
                chosen[..index].contains(id) || !pending.presentation.candidates.contains(id)
            })
        {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal(
                "invalid hand/graveyard partition choice",
            ));
        }
        let ResolutionContinuation::LibraryLook {
            stack,
            candidates,
            stage: PendingLibraryLookStage::HandGraveyard(partition),
        } = &mut pending.continuation
        else {
            unreachable!("validated partition");
        };
        let ordered_remainder;
        if partition.selected_hand.is_none() {
            partition.selected_hand = Some(chosen[0]);
            candidates.retain(|reference| reference.0 != chosen[0]);
            ordered_remainder = candidates
                .iter()
                .map(|reference| reference.0)
                .collect::<Vec<_>>();
            if ordered_remainder.len() >= 2 {
                let count = ordered_remainder.len() as u32;
                pending.presentation.candidates = ordered_remainder;
                pending.presentation.min = count;
                pending.presentation.max = count;
                pending.presentation.ordered = true;
                pending.presentation.prompt = "Order the remaining cards into your graveyard, oldest first. The last selected card is on top.".into();
                let event = self.hand_graveyard_choice_event(&pending);
                self.state.pending_resolution = Some(pending);
                return Ok(finish_with_events(self, vec![event]));
            }
        } else {
            ordered_remainder = chosen.to_vec();
        }
        let hand = partition.selected_hand.expect("validated hand selection");
        let owner = partition.owner;
        let stack = stack.clone();
        let cohort = partition.cohort.clone();
        let moves = std::iter::once((hand, Zone::Hand, None))
            .chain(
                ordered_remainder
                    .iter()
                    .map(|&id| (id, Zone::Graveyard, None)),
            )
            .collect::<Vec<_>>();
        let mut events = Vec::new();
        let receipts = match self.commit_observed_zone_move_batch(&moves, &mut events) {
            Ok(receipts) => receipts,
            Err(error) => {
                self.state.pending_resolution = Some(pending);
                return Err(error);
            }
        };
        for receipt in receipts {
            let reference = cohort
                .iter()
                .find(|r| r.object_id == receipt.object_id)
                .expect("partition member");
            events.push(permanent_moved_event_with_library_position(
                &self.state,
                receipt.object_id,
                owner,
                if receipt.object_id == hand {
                    rv1::permanent_moved::Destination::Hand
                } else {
                    rv1::permanent_moved::Destination::Graveyard
                },
                reference.library_position,
            ));
        }
        let previous = stack.previous_result().clone();
        self.complete_parked_stack_with_previous(stack, previous, events)
    }

    fn hand_graveyard_choice_event(&self, pending: &PendingResolution) -> rv1::RuledEvent {
        let presentation = &pending.presentation;
        let (candidate_card_ids, candidate_names) =
            candidate_identities(self, &presentation.candidates);
        rv1::RuledEvent {
            ev: Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(
                rv1::ResolutionChoiceRequired {
                    deciding_player_id: pending.deciding_player,
                    source_object_id: presentation.source_object_id,
                    prompt_text: presentation.prompt.clone(),
                    choice_kind: rv1::ChoiceKind::LibraryLook as i32,
                    candidate_object_ids: presentation.candidates.clone(),
                    candidate_card_ids,
                    candidate_names,
                    candidate_selectable: vec![true; presentation.candidates.len()],
                    min: presentation.min,
                    max: presentation.max,
                    ordered: presentation.ordered,
                    ..Default::default()
                },
            )),
        }
    }

    /// A vanished owner's hidden cards cannot be delegated. A live owner must retain the exact
    /// snapshot; source departure alone does not end an instruction that already began.
    pub(in crate::engine) fn refresh_hand_graveyard_partition(
        &mut self,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<bool, EngineError> {
        let Some(pending) = self.state.pending_resolution.as_ref() else {
            return Ok(false);
        };
        let ResolutionContinuation::LibraryLook {
            stack,
            stage: PendingLibraryLookStage::HandGraveyard(partition),
            ..
        } = &pending.continuation
        else {
            return Ok(false);
        };
        if self
            .state
            .player_idx(partition.owner)
            .is_some_and(|index| !self.state.players[index].has_lost)
        {
            self.validate_hand_graveyard_partition(pending)
                .map_err(|_| {
                    EngineError::ProgramInvariant("saved hand/graveyard library partition is stale")
                })?;
            if pending.resolution_prompt_id.get() == 0 {
                events.push(self.hand_graveyard_choice_event(pending));
            }
            return Ok(true);
        }
        let stack = stack.clone();
        let previous = stack.previous_result().clone();
        self.state.pending_resolution = None;
        let completed = self.complete_parked_stack_with_previous(stack, previous, Vec::new())?;
        events.extend(completed.events);
        Ok(true)
    }
}
