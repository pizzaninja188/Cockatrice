//! Authentication of Vraska's saved player-counter instruction across replacement choices.
use super::*;

impl GameEngine {
    pub(super) fn validate_poison_difference_completion(
        &self,
        events: &[ProposedCounterPlacement],
        completion: &crate::state::CounterPlacementCompletion,
        parked: Option<&ParkedStackResolution>,
    ) -> Result<(), EngineError> {
        let crate::state::CounterPlacementCompletion::PoisonTargetUpToNine {
            stack_id,
            leaf_index,
            controller,
            target_player,
            poison_before,
        } = completion
        else {
            return Ok(());
        };
        let invalid = || {
            EngineError::Illegal(
                "poison difference completion does not match its captured instruction",
            )
        };
        let frame = parked.ok_or_else(invalid)?;
        let ability = frame.item.activated_ability.as_ref().ok_or_else(invalid)?;
        if frame.item.id != *stack_id
            || frame.item.controller != *controller
            || frame.item.is_triggered
            || frame.program().is_some()
            || !frame.item.chosen_modes.is_empty()
            || ability.program.is_some()
            || ability.modal.is_some()
            || frame.resume_effect_index() != leaf_index.checked_add(1)
            || *poison_before >= 9
        {
            return Err(invalid());
        }
        let Some(SpellEffectKind::PoisonTargetUpToNine { target: filter }) =
            ability.effect.get(*leaf_index as usize)
        else {
            return Err(invalid());
        };
        if !filter.is_player() {
            return Err(invalid());
        }
        let schema = tricerules_card_model::primitives::TargetSchema::compile(
            &ability.effect,
            ability.targeting.as_ref(),
        )
        .map_err(|_| invalid())?;
        let (group_index, _) = schema
            .groups
            .iter()
            .enumerate()
            .find(|(_, group)| {
                group.bindings.iter().any(|binding| {
                    binding.effect_index == *leaf_index as usize && binding.role_index == 0
                })
            })
            .ok_or_else(invalid)?;
        let mut selections = frame
            .item
            .targets
            .iter()
            .filter(|target| target.group_index as usize == group_index);
        let selection = selections.next().ok_or_else(invalid)?;
        if selections.next().is_some()
            || selection.kind != rv1::TargetRefKind::Player as i32
            || selection.zone_change_generation.is_some()
            || selection.object_id != *target_player as u32
        {
            return Err(invalid());
        }
        let player_index = self.state.player_idx(*target_player).ok_or_else(invalid)?;
        let player = &self.state.players[player_index];
        if (!player.has_lost
            && (events.len() != 1
                || player
                    .counters
                    .get(&CounterKind::Poison)
                    .copied()
                    .unwrap_or(0)
                    != *poison_before))
            || events.len() > 1
        {
            return Err(invalid());
        }
        let baseline = BTreeMap::from([(CounterKind::Poison, 9 - *poison_before)]);
        for event in events {
            if !matches!(event.recipient, CounterPlacementRecipient::Player { player_id } if player_id == *target_player)
                || event.actor != *controller
                || event.origin != super::super::continuous::CounterPlacementOrigin::Effect
                || event.original_counters != baseline
                || event.counters.keys().copied().collect::<Vec<_>>() != [CounterKind::Poison]
                || event.entry_source.is_some()
                || event.entry_context.is_some()
                || event.counter_move_context.is_some()
                || event.cost_debit_index.is_some()
            {
                return Err(invalid());
            }
            let mut replayed = event.clone();
            replayed.counters = baseline.clone();
            replayed.effect_created_counters = true;
            replayed.applied_replacements.clear();
            let mut seen = BTreeSet::new();
            for replacement in &event.applied_replacements {
                if !replayed.counters.values().any(|count| *count > 0)
                    || replacement.kind == CounterReplacementKind::DoublingSeason
                    || (replacement.kind == CounterReplacementKind::Laezel
                        && target_player != controller)
                    || replacement.ability_id.trim().is_empty()
                    || !seen.insert((
                        replacement.source_object_id,
                        replacement.source_zone_change_generation,
                        replacement.ability_id.as_str(),
                    ))
                {
                    return Err(invalid());
                }
                Self::apply_counter_replacement(&mut replayed, replacement);
            }
            if replayed.counters != event.counters
                || replayed.effect_created_counters != event.effect_created_counters
                || replayed.applied_replacements != event.applied_replacements
            {
                return Err(invalid());
            }
        }
        Ok(())
    }

    pub(in crate::engine) fn begun_poison_difference(
        &self,
        pending: &PendingResolution,
        replacement: Option<&PendingReplacementEvent>,
    ) -> bool {
        let ResolutionContinuation::CounterReplacement {
            stack: Some(stack),
            completion:
                completion @ crate::state::CounterPlacementCompletion::PoisonTargetUpToNine { .. },
        } = &pending.continuation
        else {
            return false;
        };
        let Some(PendingReplacementEvent::Counters(batch)) = replacement else {
            return false;
        };
        pending.presentation.source_object_id == stack.item.id
            && batch.event_index < batch.events.len()
            && self
                .validate_poison_difference_completion(&batch.events, completion, Some(stack))
                .is_ok()
    }
}
