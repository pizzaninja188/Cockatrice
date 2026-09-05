//! CR 701.67: Foggy Swamp Vinebender and Waterbending Lesson share mixed payments.
//! Oracle/rulings and official CR text checked 2026-08-28. This is not mana production.
use super::super::*;
use super::transaction::{CostTransactionPlan, PreparedPaymentCosts};

pub(super) fn generic_component(cost: &ManaCost) -> Result<u32, EngineError> {
    cost.pips.iter().try_fold(0u32, |sum, pip| match pip {
        ManaSymbol::Generic(n) => sum
            .checked_add(*n)
            .ok_or(EngineError::Illegal("Waterbend cost overflow")),
        ManaSymbol::X => Err(EngineError::Illegal("unbound X in Waterbend cost")),
        _ => Ok(sum),
    })
}

impl GameEngine {
    pub(in crate::engine) fn waterbend_candidate(
        &self,
        player: PlayerId,
        costs: &PreparedPaymentCosts,
        reference: &rv1::CostObjectRef,
    ) -> bool {
        let oid = reference.object_id;
        self.payment_object_ref(oid) == *reference
            && costs.can_convoke(oid)
            && self
                .state
                .objects
                .get(&oid)
                .is_some_and(|o| o.zone == Zone::Battlefield && !o.tapped)
            && self
                .characteristics(oid)
                .is_some_and(|c| c.controller == player && (c.is_creature() || c.is_artifact()))
    }

    pub(in crate::engine) fn prepare_activation_payment(
        &self,
        player: PlayerId,
        command: &rv1::ActivateAbility,
    ) -> Result<PreparedPaymentCosts, EngineError> {
        use super::super::targeting::{validate_ability_targets, TargetSourceIdentity};
        let source = command.source_object_id;
        if self.state.priority_player_id() != player
            || self.state.blocking_choice().is_some()
            || self.state.turn_step == TurnStep::Cleanup
            || super::super::combat::priority_locked_for_combat_declaration(&self.state)
        {
            return Err(EngineError::Illegal("activation payment unavailable now"));
        }
        let idx = self
            .state
            .player_idx(player)
            .ok_or(EngineError::UnknownPlayer(player))?;
        let source_zone = match rv1::AbilitySourceZone::try_from(command.source_zone)
            .map_err(|_| EngineError::Illegal("unknown ability source zone"))?
        {
            rv1::AbilitySourceZone::Battlefield => AbilitySourceZone::Battlefield,
            rv1::AbilitySourceZone::Hand => AbilitySourceZone::Hand,
            rv1::AbilitySourceZone::Graveyard => AbilitySourceZone::Graveyard,
        };
        let source_current =
            self.state
                .objects
                .get(&source)
                .is_some_and(|object| match source_zone {
                    AbilitySourceZone::Battlefield => {
                        object.zone == Zone::Battlefield
                            && object.controller == player
                            && self.state.players[idx].battlefield.contains(&source)
                    }
                    AbilitySourceZone::Hand => {
                        object.zone == Zone::Hand
                            && object.owner == player
                            && self.state.players[idx].hand.contains(&source)
                    }
                    AbilitySourceZone::Graveyard => {
                        object.zone == Zone::Graveyard
                            && object.owner == player
                            && self.state.players[idx].graveyard.contains(&source)
                    }
                });
        if !source_current
            || self.payment_object_ref(source).zone_change_generation
                != command.expected_zone_change_generation
        {
            return Err(EngineError::Illegal("stale activation source"));
        }
        let ability = match source_zone {
            AbilitySourceZone::Battlefield => self
                .effective_activated_abilities(source)
                .into_iter()
                .find(|(index, _, _, _)| *index == command.ability_index as usize)
                .map(|(_, ability, _, _)| ability),
            AbilitySourceZone::Hand | AbilitySourceZone::Graveyard => self
                .authored_zone_activated_abilities(source, source_zone)
                .into_iter()
                .find(|(index, _, _)| *index == command.ability_index as usize)
                .map(|(_, ability, _)| ability),
        }
        .ok_or(EngineError::Illegal("missing activated ability"))?;
        if !self.ability_activatable(source, command.ability_index as usize, &ability) {
            return Err(EngineError::Illegal("activation restrictions not met"));
        }
        validate_ability_targets(
            self,
            player,
            TargetSourceIdentity::captured(source, command.expected_zone_change_generation),
            &ability.effect,
            ability.targeting.as_ref(),
            &command.targets,
        )?;
        let prepared = self.prepare_ability_costs(
            player,
            idx,
            source,
            &ability.costs,
            &command.flex_payments,
            &command.cost_selections,
            &command.restricted_mana,
            self.targeting_cost_increase(
                player,
                TargetingCostAction::ActivatedAbilities,
                &command.targets,
            ),
            self.activated_mana_reduction(player, source, &ability)?,
        )?;
        Ok(prepared)
    }

    pub(in crate::engine) fn finish_ability_payment(
        &self,
        player: PlayerId,
        source: ObjectId,
        prepared: PreparedPaymentCosts,
        selection: Option<&rv1::PaymentSelection>,
    ) -> Result<CostTransactionPlan, EngineError> {
        if let Some(selection) = selection {
            let life =
                self.validate_explicit_payment(player, source, false, &prepared, selection)?;
            prepared.finish_explicit(&self.state, selection, life)
        } else {
            prepared.finish(&self.state)
        }
    }
}
