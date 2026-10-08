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
            if let Some(event) = self.move_one_counter_between(
                source_target.object_id,
                destination_target.object_id,
                kind,
            ) {
                let source_name =
                    object_display_name(&self.state, self.registry, source_target.object_id);
                let destination_name =
                    object_display_name(&self.state, self.registry, destination_target.object_id);
                let source_label = self
                    .registry
                    .get(&stack.item.card_id)
                    .map(|definition| definition.name.clone())
                    .unwrap_or_else(|| stack.item.card_id.clone());
                events.push(ev_log(format!(
                    "{source_label} moves one {} counter from {source_name} to {destination_name}.",
                    kind.label(),
                )));
                self.fire_triggers(&[event], &mut events);
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
