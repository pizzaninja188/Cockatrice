//! Shared planning and commit machinery for engine-authoritative cost payments.
//!
//! Callers remain responsible for action-specific timing, targeting, stack placement, logging,
//! trigger staging, priority, and legal-action publication. This module owns only the reusable
//! resource transaction.

pub(super) mod convoke;
pub(super) mod demand;
pub(super) mod mana;
pub(super) mod transaction;

#[cfg(test)]
pub(in crate::engine) use mana::plan_mana_payment_with_reduction;
pub(in crate::engine) use mana::{commit_mana_payment, plan_mana_payment};
pub(in crate::engine) use transaction::{card_result_entry, PaidCardCost, SacrificeSnapshot};

use super::*;

fn mana_filter_matches_face(filter: &ManaSpendFilter, face: &CardFace) -> bool {
    filter
        .card_type
        .is_none_or(|card_type| face.matches_card_type(card_type))
        && filter
            .subtype
            .as_ref()
            .is_none_or(|subtype| face.has_subtype(subtype))
}

fn mana_filter_matches_characteristics(
    filter: &ManaSpendFilter,
    characteristics: &Characteristics,
) -> bool {
    filter.card_type.is_none_or(|card_type| match card_type {
        CardTypeFilter::BasicLand => {
            characteristics.has_type("Land")
                && characteristics
                    .supertypes
                    .iter()
                    .any(|value| value == "Basic")
        }
        CardTypeFilter::Land => characteristics.has_type("Land"),
        CardTypeFilter::Enchantment => characteristics.has_type("Enchantment"),
        CardTypeFilter::Instant => characteristics.has_type("Instant"),
        CardTypeFilter::Sorcery => characteristics.has_type("Sorcery"),
        CardTypeFilter::InstantOrSorcery => {
            characteristics.has_type("Instant") || characteristics.has_type("Sorcery")
        }
        CardTypeFilter::Creature => characteristics.is_creature(),
        CardTypeFilter::Artifact => characteristics.is_artifact(),
        CardTypeFilter::Planeswalker => characteristics.has_type("Planeswalker"),
        CardTypeFilter::Battle => characteristics.has_type("Battle"),
        CardTypeFilter::Nonland => !characteristics.has_type("Land"),
        CardTypeFilter::Noncreature => !characteristics.is_creature(),
    }) && filter
        .subtype
        .as_ref()
        .is_none_or(|subtype| characteristics.has_type(subtype))
}

/// Pays `cost` after first proving the whole mana component is affordable.
#[cfg(test)]
pub(super) fn pay_mana(
    state: &mut GameState,
    player_idx: usize,
    cost: &ManaCost,
    x_value: u32,
    extra_generic: u32,
    flex_payments: &[rv1::FlexPipPayment],
) -> Result<u32, EngineError> {
    let plan = plan_mana_payment(
        state,
        player_idx,
        cost,
        x_value,
        extra_generic,
        flex_payments,
    )?;
    let life_cost = plan.life_cost;
    commit_mana_payment(state, player_idx, plan);
    Ok(life_cost)
}

impl GameEngine {
    pub(in crate::engine) fn activated_generic_reduction(
        &self,
        controller: PlayerId,
        source_id: ObjectId,
        ability: &ActivatedAbilityDef,
    ) -> u32 {
        let context = ConditionContext {
            controller,
            source_object_id: source_id,
            source_zone_change: self
                .state
                .zone_change_generation
                .get(&source_id)
                .copied()
                .unwrap_or(0),
            resolving_spell_id: None,
            stack_item: None,
        };
        ability
            .cost_modifiers
            .iter()
            .fold(0u32, |total, modifier| match modifier {
                ActivatedCostModifier::ConditionalGenericReduction { amount, condition }
                    if self.condition_holds(condition, context) =>
                {
                    total.saturating_add(*amount)
                }
                ActivatedCostModifier::ConditionalGenericReduction { .. } => total,
            })
    }

    pub(in crate::engine) fn effective_ability_mana_cost(
        &self,
        controller: PlayerId,
        source_id: ObjectId,
        ability: &ActivatedAbilityDef,
    ) -> String {
        let reduction = self.activated_generic_reduction(controller, source_id, ability);
        let Some(cost) = ability.costs.iter().find_map(|cost| match cost {
            AbilityCost::Mana(cost) => Some(cost),
            _ => None,
        }) else {
            return String::new();
        };
        let mut remaining = reduction;
        let mut reduced = cost.clone();
        for pip in &mut reduced.pips {
            if let ManaSymbol::Generic(amount) = pip {
                let applied = (*amount).min(remaining);
                *amount -= applied;
                remaining -= applied;
            }
        }
        reduced
            .pips
            .retain(|pip| !matches!(pip, ManaSymbol::Generic(0)));
        reduced.to_string()
    }

    pub(super) fn pay_permanent_action_mana(
        &mut self,
        player_idx: usize,
        cost: &ManaCost,
        flex_payments: &[rv1::FlexPipPayment],
        restricted_mana: &[rv1::ManaSpendSelection],
        purpose: SpecialActionManaPurpose,
    ) -> Result<(), EngineError> {
        let eligible = self.eligible_restricted_mana_for_special_action(player_idx, purpose);
        let plan = mana::plan_mana_payment_with_restricted_reduction(
            &self.state,
            player_idx,
            cost,
            0,
            0,
            0,
            flex_payments,
            restricted_mana,
            &eligible,
        )?;
        commit_mana_payment(&mut self.state, player_idx, plan);
        Ok(())
    }

    pub(in crate::engine) fn targeting_cost_increase(
        &self,
        actor: PlayerId,
        action: TargetingCostAction,
        targets: &[rv1::TargetRef],
    ) -> u32 {
        let mut sources = self
            .state
            .players
            .iter()
            .flat_map(|player| player.battlefield.iter().copied())
            .collect::<Vec<_>>();
        sources.sort_unstable();

        sources.into_iter().fold(0u32, |total, source_id| {
            let Some(source_controller) = self.controller_of(source_id) else {
                return total;
            };
            let actor_matches = |set: RelativePlayerSet| match set {
                RelativePlayerSet::Controller => actor == source_controller,
                RelativePlayerSet::Opponents => self.state.are_opponents(actor, source_controller),
                RelativePlayerSet::All => true,
            };
            let Some(face) = self.effective_face(source_id) else {
                return total;
            };
            face.static_abilities
                .iter()
                .filter_map(|ability| {
                    let StaticAbilityDef::TargetingCostIncrease {
                        protected,
                        actors,
                        actions,
                        amount,
                    } = ability
                    else {
                        return None;
                    };
                    let action_matches =
                        matches!(actions, TargetingCostAction::SpellsAndActivatedAbilities)
                            || actions == &action;
                    if !action_matches || !actor_matches(*actors) {
                        return None;
                    }
                    targets
                        .iter()
                        .any(|target| {
                            self.target_is_protected(
                                source_id,
                                source_controller,
                                protected,
                                target,
                            )
                        })
                        .then_some(*amount)
                })
                .fold(total, u32::saturating_add)
        })
    }

    fn target_is_protected(
        &self,
        source_id: ObjectId,
        source_controller: PlayerId,
        protected: &TargetingCostProtected,
        target: &rv1::TargetRef,
    ) -> bool {
        let kind =
            rv1::TargetRefKind::try_from(target.kind).unwrap_or(rv1::TargetRefKind::Unspecified);
        let is_object = match kind {
            rv1::TargetRefKind::Player => false,
            rv1::TargetRefKind::Permanent
            | rv1::TargetRefKind::Stack
            | rv1::TargetRefKind::Graveyard => true,
            rv1::TargetRefKind::Unspecified => self.state.objects.contains_key(&target.object_id),
        };
        match protected {
            TargetingCostProtected::Source => is_object && target.object_id == source_id,
            TargetingCostProtected::Creatures(filter) => {
                if !is_object {
                    return false;
                }
                let Some(object) = self.state.objects.get(&target.object_id) else {
                    return false;
                };
                if object.zone != Zone::Battlefield {
                    return false;
                }
                let Some(characteristics) = self.characteristics(target.object_id) else {
                    return false;
                };
                super::characteristics::creature_matches_scope(
                    &self.state,
                    self.registry,
                    filter,
                    source_controller,
                    filter.exclude_self.then_some(source_id),
                    target.object_id,
                    &characteristics,
                )
            }
            TargetingCostProtected::Players(set) => {
                if is_object {
                    return false;
                }
                let target_player = target.object_id as PlayerId;
                self.state.player_idx(target_player).is_some()
                    && match set {
                        RelativePlayerSet::Controller => target_player == source_controller,
                        RelativePlayerSet::Opponents => {
                            self.state.are_opponents(target_player, source_controller)
                        }
                        RelativePlayerSet::All => true,
                    }
            }
        }
    }

    pub(super) fn targeting_cost_applications(
        &self,
        actor: PlayerId,
        action: TargetingCostAction,
        groups: &[rv1::LegalTargetGroup],
    ) -> Vec<rv1::TargetingCostApplication> {
        let mut candidates = Vec::new();
        for group in groups {
            candidates.extend(group.valid_permanent_ids.iter().map(|&object_id| {
                rv1::TargetCandidateRef {
                    kind: rv1::TargetRefKind::Permanent as i32,
                    object_id,
                }
            }));
            candidates.extend(group.valid_stack_ids.iter().map(|&object_id| {
                rv1::TargetCandidateRef {
                    kind: rv1::TargetRefKind::Stack as i32,
                    object_id,
                }
            }));
            candidates.extend(group.valid_graveyard_ids.iter().map(|&object_id| {
                rv1::TargetCandidateRef {
                    kind: rv1::TargetRefKind::Graveyard as i32,
                    object_id,
                }
            }));
            for player in &self.state.players {
                if (group.can_target_self && player.id == actor)
                    || (group.can_target_opponent && self.state.are_opponents(player.id, actor))
                {
                    candidates.push(rv1::TargetCandidateRef {
                        kind: rv1::TargetRefKind::Player as i32,
                        object_id: player.id as ObjectId,
                    });
                }
            }
        }
        candidates.sort_by_key(|candidate| (candidate.kind, candidate.object_id));
        candidates.dedup_by_key(|candidate| (candidate.kind, candidate.object_id));

        let mut sources = self
            .state
            .players
            .iter()
            .flat_map(|player| player.battlefield.iter().copied())
            .collect::<Vec<_>>();
        sources.sort_unstable();
        let mut applications = Vec::new();
        for source_id in sources {
            let Some(source_controller) = self.controller_of(source_id) else {
                continue;
            };
            let Some(face) = self.effective_face(source_id) else {
                continue;
            };
            for (ability_index, ability) in face.static_abilities.iter().enumerate() {
                let StaticAbilityDef::TargetingCostIncrease {
                    protected,
                    actors,
                    actions,
                    amount,
                } = ability
                else {
                    continue;
                };
                let actor_matches = match actors {
                    RelativePlayerSet::Controller => actor == source_controller,
                    RelativePlayerSet::Opponents => {
                        self.state.are_opponents(actor, source_controller)
                    }
                    RelativePlayerSet::All => true,
                };
                let action_matches =
                    matches!(actions, TargetingCostAction::SpellsAndActivatedAbilities)
                        || actions == &action;
                if !actor_matches || !action_matches {
                    continue;
                }
                let affected_targets = candidates
                    .iter()
                    .filter(|candidate| {
                        self.target_is_protected(
                            source_id,
                            source_controller,
                            protected,
                            &rv1::TargetRef {
                                object_id: candidate.object_id,
                                kind: candidate.kind,
                                ..Default::default()
                            },
                        )
                    })
                    .cloned()
                    .collect::<Vec<_>>();
                if !affected_targets.is_empty() {
                    applications.push(rv1::TargetingCostApplication {
                        application_id: (u64::from(source_id) << 32) | ability_index as u64,
                        generic_mana: *amount,
                        affected_targets,
                    });
                }
            }
        }
        applications
    }

    pub(super) fn targeted_cost_reduction_applications(
        &self,
        actor: PlayerId,
        source: super::targeting::TargetSourceIdentity,
        modifiers: &[SpellCostModifier],
        groups: &[rv1::LegalTargetGroup],
    ) -> Vec<rv1::TargetedCostReductionApplication> {
        let mut candidates = groups
            .iter()
            .flat_map(|group| group.valid_permanent_ids.iter().copied())
            .collect::<Vec<_>>();
        candidates.sort_unstable();
        candidates.dedup();

        modifiers
            .iter()
            .enumerate()
            .filter_map(|(modifier_index, modifier)| {
                let SpellCostModifier::TargetMatchGenericReduction { amount, filter } = modifier
                else {
                    return None;
                };
                let qualifying_targets = candidates
                    .iter()
                    .copied()
                    .filter(|candidate| {
                        super::targeting::target_filter_legal_at_resolution(
                            self,
                            filter,
                            *candidate,
                            actor,
                            source,
                            TriggerContext::default(),
                        )
                    })
                    .map(|object_id| rv1::TargetCandidateRef {
                        kind: rv1::TargetRefKind::Permanent as i32,
                        object_id,
                    })
                    .collect::<Vec<_>>();
                (!qualifying_targets.is_empty()).then_some(rv1::TargetedCostReductionApplication {
                    application_id: (u64::from(source.object_id()) << 32) | modifier_index as u64,
                    generic_mana: *amount,
                    qualifying_targets,
                })
            })
            .collect()
    }

    /// Room unlocking and turning a manifested permanent face up are distinct from spells and
    /// activated abilities. Use this same engine-owned quote for publication and final payment.
    pub(super) fn eligible_restricted_mana_for_special_action(
        &self,
        player_idx: usize,
        purpose: SpecialActionManaPurpose,
    ) -> Vec<u32> {
        let mut ids: Vec<u32> = self.state.players[player_idx]
            .restricted_mana
            .iter()
            .filter_map(|entry| {
                let restriction = self
                    .state
                    .mana_restrictions
                    .get(entry.restriction_group_id.checked_sub(1)? as usize)?;
                restriction
                    .special_actions
                    .contains(&purpose)
                    .then_some(entry.restriction_group_id)
            })
            .collect();
        ids.sort_unstable();
        ids.dedup();
        ids
    }

    pub(super) fn eligible_restricted_mana_for_spell(
        &self,
        player_idx: usize,
        face: &CardFace,
    ) -> Vec<u32> {
        let mut ids: Vec<u32> = self.state.players[player_idx]
            .restricted_mana
            .iter()
            .filter_map(|entry| {
                let restriction = self
                    .state
                    .mana_restrictions
                    .get(entry.restriction_group_id.checked_sub(1)? as usize)?;
                restriction
                    .cast_spell
                    .iter()
                    .any(|filter| mana_filter_matches_face(filter, face))
                    .then_some(entry.restriction_group_id)
            })
            .collect();
        ids.sort_unstable();
        ids.dedup();
        ids
    }

    pub(super) fn eligible_restricted_mana_for_ability(
        &self,
        player_idx: usize,
        source_oid: ObjectId,
    ) -> Vec<u32> {
        let Some(characteristics) = self.characteristics(source_oid) else {
            return Vec::new();
        };
        let mut ids: Vec<u32> = self.state.players[player_idx]
            .restricted_mana
            .iter()
            .filter_map(|entry| {
                let restriction = self
                    .state
                    .mana_restrictions
                    .get(entry.restriction_group_id.checked_sub(1)? as usize)?;
                restriction
                    .activate_ability
                    .iter()
                    .any(|filter| mana_filter_matches_characteristics(filter, &characteristics))
                    .then_some(entry.restriction_group_id)
            })
            .collect();
        ids.sort_unstable();
        ids.dedup();
        ids
    }

    pub(super) fn spell_generic_reduction(
        &self,
        player: PlayerId,
        source_oid: ObjectId,
        face: &CardFace,
        modifiers: &[SpellCostModifier],
    ) -> u32 {
        let context = ConditionContext {
            controller: player,
            source_object_id: source_oid,
            source_zone_change: self
                .state
                .zone_change_generation
                .get(&source_oid)
                .copied()
                .unwrap_or(0),
            resolving_spell_id: None,
            stack_item: None,
        };
        let intrinsic = modifiers
            .iter()
            .fold(0u32, |total, modifier| match modifier {
                SpellCostModifier::ConditionalGenericReduction { amount, condition }
                    if self.condition_holds(condition, context) =>
                {
                    total.saturating_add(*amount)
                }
                SpellCostModifier::ConditionalGenericReduction { .. } => total,
                SpellCostModifier::BattlefieldCountGenericReduction {
                    amount_per_match,
                    filter,
                    aggregate,
                } => total.saturating_add(
                    self.battlefield_aggregate_value(filter, *aggregate, context)
                        .saturating_mul(*amount_per_match),
                ),
                SpellCostModifier::TargetMatchGenericReduction { .. } => total,
            });

        let mut sources = self
            .state
            .players
            .iter()
            .flat_map(|candidate| candidate.battlefield.iter().copied())
            .collect::<Vec<_>>();
        sources.sort_unstable();
        sources.into_iter().fold(intrinsic, |total, source_id| {
            let Some(source_controller) = self.controller_of(source_id) else {
                return total;
            };
            let Some(source_face) = self.effective_face(source_id) else {
                return total;
            };
            source_face
                .static_abilities
                .iter()
                .fold(total, |total, ability| {
                    let StaticAbilityDef::SpellGenericReduction {
                        casters,
                        spell_type,
                        amount,
                        condition,
                    } = ability
                    else {
                        return total;
                    };
                    let caster_matches = match casters {
                        RelativePlayerSet::Controller => player == source_controller,
                        RelativePlayerSet::Opponents => {
                            self.state.are_opponents(player, source_controller)
                        }
                        RelativePlayerSet::All => true,
                    };
                    if !caster_matches
                        || spell_type.is_some_and(|card_type| !face.matches_card_type(card_type))
                    {
                        return total;
                    }
                    let ability_context = ConditionContext {
                        controller: source_controller,
                        source_object_id: source_id,
                        source_zone_change: self
                            .state
                            .zone_change_generation
                            .get(&source_id)
                            .copied()
                            .unwrap_or(0),
                        resolving_spell_id: None,
                        stack_item: None,
                    };
                    if condition
                        .as_ref()
                        .is_some_and(|condition| !self.condition_holds(condition, ability_context))
                    {
                        return total;
                    }
                    total.saturating_add(*amount)
                })
        })
    }

    pub(super) fn spell_target_generic_reduction(
        &self,
        player: PlayerId,
        source: super::targeting::TargetSourceIdentity,
        targets: &[rv1::TargetRef],
        modifiers: &[SpellCostModifier],
    ) -> u32 {
        modifiers.iter().fold(0u32, |total, modifier| {
            let SpellCostModifier::TargetMatchGenericReduction { amount, filter } = modifier else {
                return total;
            };
            if targets.iter().any(|target| {
                super::targeting::target_filter_legal_at_resolution(
                    self,
                    filter,
                    target.object_id,
                    player,
                    source,
                    TriggerContext::default(),
                )
            }) {
                total.saturating_add(*amount)
            } else {
                total
            }
        })
    }

    pub(super) fn can_pay_generic_mana(&self, player: PlayerId, amount: u32) -> bool {
        self.state.player_idx(player).is_some_and(|player_idx| {
            plan_mana_payment(
                &self.state,
                player_idx,
                &ManaCost::default(),
                0,
                amount,
                &[],
            )
            .is_ok()
        })
    }

    pub(super) fn pay_generic_mana(
        &mut self,
        player: PlayerId,
        amount: u32,
    ) -> Result<(), EngineError> {
        let player_idx = self
            .state
            .player_idx(player)
            .ok_or(EngineError::UnknownPlayer(player))?;
        let plan = plan_mana_payment(
            &self.state,
            player_idx,
            &ManaCost::default(),
            0,
            amount,
            &[],
        )?;
        commit_mana_payment(&mut self.state, player_idx, plan);
        Ok(())
    }

    pub(super) fn can_pay_resolution_mana(&self, player: PlayerId, cost: &ManaCost) -> bool {
        self.state.player_idx(player).is_some_and(|player_idx| {
            plan_mana_payment(&self.state, player_idx, cost, 0, 0, &[]).is_ok()
        })
    }

    pub(super) fn pay_resolution_mana(
        &mut self,
        player: PlayerId,
        cost: &ManaCost,
    ) -> Result<(), EngineError> {
        let player_idx = self
            .state
            .player_idx(player)
            .ok_or(EngineError::UnknownPlayer(player))?;
        let plan = plan_mana_payment(&self.state, player_idx, cost, 0, 0, &[])?;
        commit_mana_payment(&mut self.state, player_idx, plan);
        Ok(())
    }
}

#[cfg(test)]
mod restricted_mana_filter_tests {
    use super::*;

    #[test]
    fn planeswalker_subtype_filter_requires_both_characteristics() {
        let filter = ManaSpendFilter {
            card_type: Some(CardTypeFilter::Planeswalker),
            subtype: Some("Chandra".into()),
        };
        let chandra = CardFace {
            types: vec!["Planeswalker".into(), "Chandra".into()],
            ..Default::default()
        };
        let jaya = CardFace {
            types: vec!["Planeswalker".into(), "Jaya".into()],
            ..Default::default()
        };
        let elemental_named_chandra = CardFace {
            types: vec!["Creature".into(), "Elemental".into(), "Chandra".into()],
            ..Default::default()
        };

        assert!(mana_filter_matches_face(&filter, &chandra));
        assert!(!mana_filter_matches_face(&filter, &jaya));
        assert!(!mana_filter_matches_face(&filter, &elemental_named_chandra));
    }
}
