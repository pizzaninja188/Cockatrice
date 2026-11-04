//! Scope and recorded-history validation for fixed announced counter distributions.
use super::*;

impl GameEngine {
    pub(super) fn validate_distributed_counter_completion(
        &self,
        events: &[ProposedCounterPlacement],
        completion: &crate::state::CounterPlacementCompletion,
        parked: Option<&ParkedStackResolution>,
    ) -> Result<(), EngineError> {
        let crate::state::CounterPlacementCompletion::DistributedCounters {
            stack_id,
            leaf_index,
            controller,
            group_index,
            counter,
            total,
        } = completion
        else {
            return Ok(());
        };
        let invalid = || {
            EngineError::Illegal("distributed counters do not match the captured frame or history")
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
            || ability.source_zone
                != tricerules_card_model::primitives::AbilitySourceZone::Battlefield
            || !matches!(ability.costs.as_slice(), [AbilityCost::Loyalty(_)])
            || frame.resume_effect_index() != leaf_index.checked_add(1)
            || *total == 0
        {
            return Err(invalid());
        }
        let Some(SpellEffectKind::DistributeCounters {
            counter: authored_counter,
            count,
            subject,
        }) = ability.effect.get(*leaf_index as usize)
        else {
            return Err(invalid());
        };
        if authored_counter != counter
            || count != total
            || !matches!(subject, tricerules_card_model::primitives::EffectSubject::Chosen(filter) if filter.kind == tricerules_card_model::primitives::TargetKind::Creature)
            || ability
                .effect
                .iter()
                .filter(|effect| matches!(effect, SpellEffectKind::DistributeCounters { .. }))
                .count()
                != 1
        {
            return Err(invalid());
        }
        let schema = tricerules_card_model::primitives::TargetSchema::compile(
            &ability.effect,
            ability.targeting.as_ref(),
        )
        .map_err(|_| invalid())?;
        let actual_group = schema
            .groups
            .iter()
            .position(|group| {
                group.bindings.iter().any(|binding| {
                    binding.effect_index == *leaf_index as usize && binding.role_index == 0
                })
            })
            .ok_or_else(invalid)?;
        if actual_group != *group_index as usize {
            return Err(invalid());
        }
        let mut receipts = BTreeMap::new();
        let mut remaining_total = 0u32;
        for target in &frame.item.targets {
            if target.group_index != *group_index {
                if target.counter_amount != 0 {
                    return Err(invalid());
                }
                continue;
            }
            let generation = target.zone_change_generation.ok_or_else(invalid)?;
            if target.kind != rv1::TargetRefKind::Permanent as i32
                || target.damage_amount != 0
                || target.counter_amount == 0
                || target.counter_amount > *total
                || receipts
                    .insert((target.object_id, generation), target.counter_amount)
                    .is_some()
            {
                return Err(invalid());
            }
            remaining_total = remaining_total
                .checked_add(target.counter_amount)
                .ok_or_else(invalid)?;
        }
        // CR608.2b removed illegal receipts before this frame was captured. Never redistribute
        // their missing shares or require the remaining cohort to sum to the original total.
        if remaining_total > *total {
            return Err(invalid());
        }
        let mut recipients = BTreeSet::new();
        for event in events {
            let CounterPlacementRecipient::Object {
                object_id,
                zone_change_generation,
            } = &event.recipient
            else {
                return Err(invalid());
            };
            let key = (*object_id, *zone_change_generation);
            let amount = *receipts.get(&key).ok_or_else(invalid)?;
            let baseline = BTreeMap::from([(*counter, amount)]);
            if !recipients.insert(key)
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
            if !Self::counter_placement_has_authenticated_replacement_history(event, true) {
                return Err(invalid());
            }
        }
        Ok(())
    }

    pub(in crate::engine) fn begun_distributed_counter_placement(
        &self,
        pending: &PendingResolution,
        replacement: Option<&PendingReplacementEvent>,
    ) -> bool {
        let ResolutionContinuation::CounterReplacement {
            stack: Some(stack),
            completion:
                completion @ crate::state::CounterPlacementCompletion::DistributedCounters { .. },
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
                .validate_distributed_counter_completion(&batch.events, completion, Some(stack))
                .is_ok()
    }
}
