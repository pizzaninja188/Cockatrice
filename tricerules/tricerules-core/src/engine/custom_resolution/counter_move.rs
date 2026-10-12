use super::*;
use crate::engine::targeting::counter_move_target_pair_is_current;

impl GameEngine {
    pub(super) fn finish_counter_move_choice(
        &mut self,
        pending: PendingResolution,
        answer: &rv1::SubmitResolutionChoice,
        decision: rv1::ResolutionChoiceDecision,
    ) -> Result<RuledEventBatch, EngineError> {
        let (stack, counter_move) = match &pending.continuation {
            ResolutionContinuation::CounterMove {
                stack,
                counter_move,
            } => (stack.clone(), counter_move.clone()),
            _ => unreachable!("counter-move continuation"),
        };
        if decision != rv1::ResolutionChoiceDecision::SelectBranch
            || !answer.chosen_object_ids.is_empty()
            || !answer.chosen_player_ids.is_empty()
            || answer.cast_spell.is_some()
            || answer.spell_cast_announcement.is_some()
            || answer.chosen_combat_defender.is_some()
            || answer.payment.is_some()
            || !answer.restricted_mana.is_empty()
        {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal(
                "counter-move choice requires one offered branch index",
            ));
        }
        let Some(kind) = counter_move
            .offered_counter_kinds
            .get(answer.selected_branch_index as usize)
            .copied()
        else {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal("unoffered counter-move choice"));
        };

        let source_target = counter_move.source_target;
        let destination_target = counter_move.destination_target;
        let mut events = Vec::new();
        let can_move = counter_move_target_pair_is_current(
            self,
            &stack.item,
            &source_target,
            &destination_target,
        ) && self
            .state
            .objects
            .get(&source_target.object_id)
            .is_some_and(|source| source.counter_count(kind) > 0)
            && self.counter_move_can_complete(
                source_target.object_id,
                destination_target.object_id,
                kind,
            );
        if can_move {
            let actor = stack.item.controller;
            let Some(mut proposal) = self.proposed_object_counter_placement(
                actor,
                destination_target.object_id,
                [(kind, 1)],
                super::continuous::CounterPlacementOrigin::Effect,
            ) else {
                return self.complete_parked_resolution_with_previous(
                    stack.item,
                    stack.resume_effect_index,
                    stack.previous_result,
                    events,
                );
            };
            let Some(source_zone_change_generation) = source_target.zone_change_generation else {
                return self.complete_parked_resolution_with_previous(
                    stack.item,
                    stack.resume_effect_index,
                    stack.previous_result,
                    events,
                );
            };
            proposal.counter_move_context =
                Some(super::super::replacement::CounterMoveReplacementContext {
                    source_object_id: source_target.object_id,
                    source_zone_change_generation,
                    kind,
                });
            let ability_label = self
                .registry
                .get(&stack.item.card_id)
                .map(|definition| definition.name.clone())
                .unwrap_or_else(|| stack.item.card_id.clone());
            let completion = crate::state::CounterPlacementCompletion::CounterMove {
                source_target,
                destination_target,
                kind,
                actor,
                ability_label,
            };
            match self.begin_counter_placement_batch(
                vec![proposal],
                stack.clone(),
                completion,
                &mut events,
            ) {
                Ok(Some(receipts)) => {
                    let mut previous_result = stack.previous_result.clone();
                    previous_result.counter_placements.extend(receipts);
                    return self.complete_parked_resolution_with_previous(
                        stack.item,
                        stack.resume_effect_index,
                        previous_result,
                        events,
                    );
                }
                Ok(None) => return Ok(finish_with_events(self, events)),
                Err(error) => return Err(error),
            }
        }

        self.complete_parked_resolution_with_previous(
            stack.item,
            stack.resume_effect_index,
            stack.previous_result,
            events,
        )
    }
}
