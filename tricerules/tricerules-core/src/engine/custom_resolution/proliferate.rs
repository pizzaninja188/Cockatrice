use super::*;
use crate::engine::continuous::CounterPlacementOrigin;

impl GameEngine {
    pub(super) fn finish_proliferate_choice(
        &mut self,
        pending: PendingResolution,
        answer: &rv1::SubmitResolutionChoice,
        decision: rv1::ResolutionChoiceDecision,
    ) -> Result<RuledEventBatch, EngineError> {
        let ResolutionContinuation::Proliferate {
            stack,
            candidate_generations,
            candidate_player_ids,
        } = &pending.continuation
        else {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal("not a Proliferate choice"));
        };

        if decision != rv1::ResolutionChoiceDecision::Unspecified
            || answer.selected_branch_index != 0
            || answer.cast_spell.is_some()
            || answer.spell_cast_announcement.is_some()
            || answer.chosen_combat_defender.is_some()
            || answer.payment.is_some()
            || !answer.restricted_mana.is_empty()
        {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal("invalid Proliferate choice payload"));
        }

        let chosen_objects = answer.chosen_object_ids.as_slice();
        let chosen_players = answer.chosen_player_ids.as_slice();
        let total = chosen_objects.len().saturating_add(chosen_players.len()) as u32;
        if total < pending.presentation.min || total > pending.presentation.max {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal(
                "wrong number of Proliferate recipients",
            ));
        }

        let mut seen_objects = HashSet::new();
        for &object_id in chosen_objects {
            let current_generation = self
                .state
                .zone_change_generation
                .get(&object_id)
                .copied()
                .unwrap_or(0);
            let current_candidate = candidate_generations
                .iter()
                .any(|&(candidate, generation)| {
                    candidate == object_id && generation == current_generation
                });
            let still_has_counters = self.state.objects.get(&object_id).is_some_and(|object| {
                object.zone == Zone::Battlefield && object.counters.values().any(|count| *count > 0)
            });
            if !pending.presentation.candidates.contains(&object_id)
                || !current_candidate
                || !still_has_counters
                || !seen_objects.insert(object_id)
            {
                self.state.pending_resolution = Some(pending);
                return Err(EngineError::Illegal(
                    "invalid or stale Proliferate permanent",
                ));
            }
        }

        let mut seen_players = HashSet::new();
        for &player_id in chosen_players {
            let eligible_now = self.state.player_idx(player_id).is_some_and(|index| {
                let player = &self.state.players[index];
                !player.has_lost && player.counters.values().any(|count| *count > 0)
            });
            if !candidate_player_ids.contains(&player_id)
                || !eligible_now
                || !seen_players.insert(player_id)
            {
                self.state.pending_resolution = Some(pending);
                return Err(EngineError::Illegal("invalid or stale Proliferate player"));
            }
        }

        let mut placements = Vec::new();
        for &object_id in chosen_objects {
            let counters = self
                .state
                .objects
                .get(&object_id)
                .into_iter()
                .flat_map(|object| object.counters.iter())
                .filter_map(|(&kind, &count)| (count > 0).then_some((kind, 1)))
                .collect::<Vec<_>>();
            if let Some(event) = self.proposed_object_counter_placement(
                pending.deciding_player,
                object_id,
                counters,
                CounterPlacementOrigin::Effect,
            ) {
                placements.push(event);
            }
        }

        for &player_id in chosen_players {
            let Some(index) = self.state.player_idx(player_id) else {
                unreachable!("validated player remains present during the command")
            };
            let counters = self.state.players[index]
                .counters
                .iter()
                .filter_map(|(&kind, &count)| (count > 0).then_some((kind, 1)))
                .collect::<Vec<_>>();
            if let Some(event) = self.proposed_player_counter_placement(
                pending.deciding_player,
                player_id,
                counters,
                CounterPlacementOrigin::Effect,
            ) {
                placements.push(event);
            }
        }

        let stack = stack.clone();
        let mut logs = Vec::new();
        let receipts = self.begin_counter_placement_batch(
            placements,
            stack.clone(),
            crate::state::CounterPlacementCompletion::Proliferate {
                player_id: pending.deciding_player,
            },
            &mut logs,
        )?;
        let Some(receipts) = receipts else {
            return Ok(finish_with_events(self, logs));
        };
        let mut previous_result = stack.previous_result.clone();
        previous_result.counter_placements.extend(receipts);
        self.complete_parked_resolution_with_previous(
            stack.item,
            stack.resume_effect_index,
            previous_result,
            logs,
        )
    }
}
