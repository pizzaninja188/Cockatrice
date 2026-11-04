//! Captured scope and accepted replacement history for fixed player-targeted poison.
use super::*;

impl GameEngine {
    pub(super) fn validate_targeted_poison_completion(
        &self,
        events: &[ProposedCounterPlacement],
        completion: &crate::state::CounterPlacementCompletion,
        parked: Option<&ParkedStackResolution>,
    ) -> Result<(), EngineError> {
        let crate::state::CounterPlacementCompletion::TargetedPoison {
            stack_id,
            leaf_index,
            controller,
            target_player,
            count,
        } = completion
        else {
            return Ok(());
        };
        let invalid = || {
            EngineError::Illegal(
                "fixed poison completion does not match its captured instruction or history",
            )
        };
        let frame = parked.ok_or_else(invalid)?;
        if frame.item.id != *stack_id
            || frame.item.controller != *controller
            || frame.program().is_some()
            || !frame.item.chosen_modes.is_empty()
            || frame.resume_effect_index() != leaf_index.checked_add(1)
            || *count == 0
        {
            return Err(invalid());
        }
        let (effects, targeting) = if let Some(ability) = &frame.item.triggered_ability {
            if !frame.item.is_triggered
                || frame.item.activated_ability.is_some()
                || ability.program.is_some()
                || ability.modal.is_some()
            {
                return Err(invalid());
            }
            (ability.effect.as_slice(), ability.targeting.as_ref())
        } else if let Some(ability) = &frame.item.activated_ability {
            if frame.item.is_triggered || ability.program.is_some() || ability.modal.is_some() {
                return Err(invalid());
            }
            (ability.effect.as_slice(), ability.targeting.as_ref())
        } else {
            if frame.item.is_triggered {
                return Err(invalid());
            }
            let face = self
                .registry
                .get(&frame.item.card_id)
                .and_then(|definition| definition.face(frame.item.face_index))
                .ok_or_else(invalid)?;
            if face.spell_program.is_some() || face.modal_spell.is_some() {
                return Err(invalid());
            }
            (face.spell_effect.as_slice(), face.targeting.as_ref())
        };
        let Some(SpellEffectKind::PutPoisonCountersOnTargetPlayer {
            count: authored_count,
            target: filter,
        }) = effects.get(*leaf_index as usize)
        else {
            return Err(invalid());
        };
        if authored_count != count || !filter.is_player() {
            return Err(invalid());
        }
        let schema = tricerules_card_model::primitives::TargetSchema::compile(effects, targeting)
            .map_err(|_| invalid())?;
        let group = schema
            .groups
            .iter()
            .position(|group| {
                group.bindings.iter().any(|binding| {
                    binding.effect_index == *leaf_index as usize && binding.role_index == 0
                })
            })
            .ok_or_else(invalid)?;
        let selections = frame
            .item
            .targets
            .iter()
            .filter(|target| target.group_index as usize == group)
            .collect::<Vec<_>>();
        let [selection] = selections.as_slice() else {
            return Err(invalid());
        };
        if selection.kind != rv1::TargetRefKind::Player as i32
            || selection.zone_change_generation.is_some()
            || selection.object_id != *target_player as u32
            || selection.counter_amount != 0
            || selection.damage_amount != 0
        {
            return Err(invalid());
        }
        let player = self
            .state
            .players
            .get(self.state.player_idx(*target_player).ok_or_else(invalid)?)
            .ok_or_else(invalid)?;
        if events.len() > 1 || (!player.has_lost && events.len() != 1) {
            return Err(invalid());
        }
        let baseline = BTreeMap::from([(CounterKind::Poison, *count)]);
        for event in events {
            if !matches!(event.recipient,CounterPlacementRecipient::Player{player_id} if player_id==*target_player)
                || event.actor != *controller
                || event.origin != super::super::continuous::CounterPlacementOrigin::Effect
                || event.original_counters != baseline
                || event.entry_source.is_some()
                || event.entry_context.is_some()
                || event.counter_move_context.is_some()
                || event.cost_debit_index.is_some()
            {
                return Err(invalid());
            }
            if event.applied_replacements.iter().any(|replacement| {
                replacement.kind == CounterReplacementKind::DoublingSeason
                    || (replacement.kind == CounterReplacementKind::Laezel
                        && target_player != controller)
            }) || !Self::counter_placement_has_authenticated_replacement_history(event, true)
            {
                return Err(invalid());
            }
        }
        Ok(())
    }

    pub(in crate::engine) fn begun_targeted_poison_placement(
        &self,
        pending: &PendingResolution,
        replacement: Option<&PendingReplacementEvent>,
    ) -> bool {
        let ResolutionContinuation::CounterReplacement {
            stack: Some(stack),
            completion: completion @ crate::state::CounterPlacementCompletion::TargetedPoison { .. },
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
                .validate_targeted_poison_completion(&batch.events, completion, Some(stack))
                .is_ok()
    }
}
