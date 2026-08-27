//! Atomic non-mana and mana debit plans shared by spell casting and ability activation.

use super::super::events::object_display_name;
use super::super::resolution::{permanent_moved_event, sacrifice_permanent};
use super::super::*;
use super::mana::{
    commit_mana_payment, mana_payment_still_valid, plan_mana_payment_with_restricted_reduction,
    ManaPaymentPlan,
};
#[derive(Debug, Clone)]
pub(in crate::engine) struct SacrificeSnapshot {
    pub(in crate::engine) source: TriggerSourceSnapshot,
    pub(in crate::engine) was_creature: bool,
    pub(in crate::engine) died: bool,
}

enum CostDebit {
    Loyalty {
        object_id: ObjectId,
        generation: u64,
        delta: i32,
    },
    Tap {
        object_id: ObjectId,
        generation: u64,
    },
    Mana(ManaPaymentPlan),
    Discard {
        object_id: ObjectId,
        generation: u64,
        owner: PlayerId,
    },
    Exile {
        object_id: ObjectId,
        generation: u64,
        owner: PlayerId,
    },
    Sacrifice {
        snapshot: SacrificeSnapshot,
        owner: PlayerId,
    },
    ObserveHand {
        object_id: ObjectId,
        generation: u64,
        owner: PlayerId,
    },
    ObservePermanent {
        object_id: ObjectId,
        generation: u64,
        controller: PlayerId,
    },
}

pub(in crate::engine) struct CostPaymentReceipt {
    pub(in crate::engine) move_events: Vec<rv1::RuledEvent>,
    pub(in crate::engine) tap_events: Vec<GameEvent>,
    pub(in crate::engine) sacrificed: Vec<SacrificeSnapshot>,
    pub(in crate::engine) paid_card_costs: Vec<PaidCardCost>,
    pub(in crate::engine) life_paid: u32,
    pub(in crate::engine) mana_spent: u64,
    pub(in crate::engine) expend_triggers: Vec<crate::engine::triggers::CollectedTrigger>,
    pub(in crate::engine) restricted_mana_spent: Vec<(u32, ManaAmount)>,
    pub(in crate::engine) cast_cost_receipts: Vec<CastCostReceipt>,
}

pub(in crate::engine) enum PaidCardCost {
    Discard {
        object_id: ObjectId,
        card_name: String,
        result: CardResultEntry,
    },
    Exile {
        object_id: ObjectId,
        card_name: String,
        result: CardResultEntry,
    },
    Sacrifice {
        object_id: ObjectId,
        card_name: String,
        result: CardResultEntry,
    },
}

impl PaidCardCost {
    fn object_id(&self) -> ObjectId {
        match self {
            Self::Discard { object_id, .. }
            | Self::Exile { object_id, .. }
            | Self::Sacrifice { object_id, .. } => *object_id,
        }
    }

    pub(in crate::engine) fn log_phrase(&self) -> String {
        match self {
            Self::Discard { card_name, .. } => format!("discarding {card_name}"),
            Self::Exile { card_name, .. } => format!("exiling {card_name}"),
            Self::Sacrifice { card_name, .. } => format!("sacrificing {card_name}"),
        }
    }

    pub(in crate::engine) fn result(&self) -> &CardResultEntry {
        match self {
            Self::Discard { result, .. }
            | Self::Exile { result, .. }
            | Self::Sacrifice { result, .. } => result,
        }
    }
}

pub(in crate::engine) fn card_result_entry(
    state: &GameState,
    registry: &'static CardRegistry,
    action: CardResultAction,
    affected_player: PlayerId,
    object_id: ObjectId,
) -> CardResultEntry {
    let matched_card_types = state
        .objects
        .get(&object_id)
        .and_then(|object| registry.get(&object.card_id))
        .map(|definition| {
            CardTypeFilter::ALL
                .into_iter()
                .filter(|filter| definition.matches_card_type_outside_stack(*filter))
                .collect()
        })
        .unwrap_or_default();
    CardResultEntry {
        action,
        affected_player,
        object_id,
        zone_change_generation: state
            .zone_change_generation
            .get(&object_id)
            .copied()
            .unwrap_or(0),
        matched_card_types,
    }
}

/// The same atomic debits pay casting or activated costs, but only casting expends mana.
#[derive(Clone, Copy, PartialEq, Eq)]
enum CostPurpose {
    Spell,
    Ability,
}

pub(in crate::engine) struct CostTransactionPlan {
    purpose: CostPurpose,
    player: PlayerId,
    player_idx: usize,
    debits: Vec<CostDebit>,
    cast_cost_receipts: Vec<CastCostReceipt>,
}

pub(in crate::engine) struct PreparedSpellCosts {
    pub transaction: CostTransactionPlan,
    pub mana: ManaCost,
    pub x_value: u32,
    pub extra_generic: u32,
    pub generic_reduction: u32,
    pub flex_payments: Vec<rv1::FlexPipPayment>,
    pub restricted_mana: Vec<rv1::ManaSpendSelection>,
    pub eligible_restricted_mana: Vec<u32>,
}

impl PreparedSpellCosts {
    pub fn can_convoke(&self, oid: ObjectId) -> bool {
        !self
            .transaction
            .debits
            .iter()
            .any(|d| matches!(d, CostDebit::Tap { object_id, .. } if *object_id == oid))
    }

    pub fn finish_explicit(
        mut self,
        state: &GameState,
        selection: &rv1::SpellPaymentSelection,
        life: u32,
    ) -> Result<CostTransactionPlan, EngineError> {
        let mana = super::mana::plan_exact_mana_payment(
            state,
            self.transaction.player_idx,
            super::convoke::mana_counts(selection.mana.as_ref()),
            &self.restricted_mana,
            &self.eligible_restricted_mana,
            life,
        )?;
        let taps = selection
            .convoke
            .iter()
            .filter_map(|c| c.object.as_ref())
            .map(|c| CostDebit::Tap {
                object_id: c.object_id,
                generation: c.zone_change_generation,
            })
            .collect::<Vec<_>>();
        // CR 601.2h permits tapping for Convoke before sacrificing the same creature as another
        // cost. A second tap is forbidden by can_convoke; sacrifice is deliberately not excluded.
        self.transaction.debits.splice(0..0, taps);
        self.transaction.debits.insert(0, CostDebit::Mana(mana));
        Ok(self.transaction)
    }

    pub fn finish(mut self, state: &GameState) -> Result<CostTransactionPlan, EngineError> {
        let mana = plan_mana_payment_with_restricted_reduction(
            state,
            self.transaction.player_idx,
            &self.mana,
            self.x_value,
            self.extra_generic,
            self.generic_reduction,
            &self.flex_payments,
            &self.restricted_mana,
            &self.eligible_restricted_mana,
        )?;
        self.transaction.debits.insert(0, CostDebit::Mana(mana));
        Ok(self.transaction)
    }
}

impl GameEngine {
    #[cfg(test)]
    #[allow(clippy::too_many_arguments)]
    pub(in crate::engine) fn plan_spell_costs(
        &self,
        player: PlayerId,
        player_idx: usize,
        source_oid: ObjectId,
        mana_cost: &ManaCost,
        x_value: u32,
        extra_generic: u32,
        generic_reduction: u32,
        flex_payments: &[rv1::FlexPipPayment],
        costs: &[AdditionalCost],
        selections: &[rv1::CostSelection],
        cast_cost_groups: &[CastCostGroupDef],
        cast_cost_group_selections: &[rv1::CastCostGroupSelection],
        restricted_mana: &[rv1::ManaSpendSelection],
        eligible_restricted_mana: &[u32],
        cast_method: SpellCastMethod,
    ) -> Result<CostTransactionPlan, EngineError> {
        self.prepare_spell_costs(
            player,
            player_idx,
            source_oid,
            mana_cost,
            x_value,
            extra_generic,
            generic_reduction,
            flex_payments,
            costs,
            selections,
            cast_cost_groups,
            cast_cost_group_selections,
            restricted_mana,
            eligible_restricted_mana,
            cast_method,
        )?
        .finish(&self.state)
    }

    #[allow(clippy::too_many_arguments)]
    pub(in crate::engine) fn prepare_spell_costs(
        &self,
        player: PlayerId,
        player_idx: usize,
        source_oid: ObjectId,
        mana_cost: &ManaCost,
        x_value: u32,
        extra_generic: u32,
        generic_reduction: u32,
        flex_payments: &[rv1::FlexPipPayment],
        costs: &[AdditionalCost],
        selections: &[rv1::CostSelection],
        cast_cost_groups: &[CastCostGroupDef],
        cast_cost_group_selections: &[rv1::CastCostGroupSelection],
        restricted_mana: &[rv1::ManaSpendSelection],
        eligible_restricted_mana: &[u32],
        cast_method: SpellCastMethod,
    ) -> Result<PreparedSpellCosts, EngineError> {
        use rv1::cost_selection::Selection;

        let mut by_index = HashMap::new();
        for selection in selections {
            let cost_index = selection.cost_index as usize;
            if cost_index >= costs.len() || by_index.insert(cost_index, selection).is_some() {
                return Err(EngineError::Illegal("invalid or duplicate cost selection"));
            }
        }

        let mut debits = Vec::with_capacity(costs.len() + cast_cost_groups.len() + 1);
        let mut combined_mana = mana_cost.clone();
        let mut cast_cost_receipts = Vec::new();
        let mut group_selections = HashMap::new();
        let harmonize_group_index = cast_cost_groups.len();
        for selection in cast_cost_group_selections {
            let group_index = selection.group_index as usize;
            let is_harmonize_group =
                cast_method == SpellCastMethod::Harmonize && group_index == harmonize_group_index;
            if (group_index >= cast_cost_groups.len() && !is_harmonize_group)
                || group_selections.insert(group_index, selection).is_some()
            {
                return Err(EngineError::Illegal(
                    "invalid or duplicate cast cost group selection",
                ));
            }
        }
        for (group_index, group) in cast_cost_groups.iter().enumerate() {
            let Some(selection) = group_selections.get(&group_index) else {
                if group.min == 0 {
                    continue;
                }
                return Err(EngineError::Illegal(
                    "missing required cast cost group selection",
                ));
            };
            let option = group
                .options
                .get(selection.option_index as usize)
                .ok_or(EngineError::Illegal("invalid cast cost option"))?;
            let object = match option {
                CastCostOptionDef::Mana { cost, .. } => {
                    if selection.selected_object.is_some() {
                        return Err(EngineError::Illegal(
                            "mana cast cost option cannot select an object",
                        ));
                    }
                    combined_mana.pips.extend(cost.pips.iter().cloned());
                    None
                }
                CastCostOptionDef::Behold {
                    hand_filter,
                    permanent_filter,
                    ..
                } => {
                    use rv1::cast_cost_group_selection::SelectedObject;
                    match selection.selected_object {
                        Some(SelectedObject::HandIndex(hand_index)) => {
                            let object_id = self.state.players[player_idx]
                                .hand
                                .get(hand_index as usize)
                                .copied()
                                .ok_or(EngineError::Illegal("invalid behold hand slot"))?;
                            if object_id == source_oid
                                || !super::super::resolution::library_card_matches_filter(
                                    &self.state,
                                    self.registry,
                                    object_id,
                                    Some(hand_filter),
                                )
                            {
                                return Err(EngineError::Illegal("illegal behold hand selection"));
                            }
                            let object = &self.state.objects[&object_id];
                            let generation = self
                                .state
                                .zone_change_generation
                                .get(&object_id)
                                .copied()
                                .unwrap_or(0);
                            debits.push(CostDebit::ObserveHand {
                                object_id,
                                generation,
                                owner: object.owner,
                            });
                            Some(CastCostObjectReceipt::RevealedHand {
                                object_id,
                                zone_change_generation: generation,
                                card_id: object.card_id.clone(),
                                card_name: object_display_name(
                                    &self.state,
                                    self.registry,
                                    object_id,
                                ),
                            })
                        }
                        Some(SelectedObject::PermanentId(object_id)) => {
                            if !self.ability_cost_permanent_matches(
                                player,
                                None,
                                object_id,
                                permanent_filter,
                            ) {
                                return Err(EngineError::Illegal(
                                    "illegal behold permanent selection",
                                ));
                            }
                            let object = &self.state.objects[&object_id];
                            let generation = self
                                .state
                                .zone_change_generation
                                .get(&object_id)
                                .copied()
                                .unwrap_or(0);
                            if selection.expected_zone_change_generation != generation {
                                return Err(EngineError::Illegal(
                                    "stale behold permanent selection",
                                ));
                            }
                            debits.push(CostDebit::ObservePermanent {
                                object_id,
                                generation,
                                controller: player,
                            });
                            Some(CastCostObjectReceipt::ChosenPermanent {
                                object_id,
                                zone_change_generation: generation,
                                card_id: object.card_id.clone(),
                                card_name: object_display_name(
                                    &self.state,
                                    self.registry,
                                    object_id,
                                ),
                            })
                        }
                        None => {
                            return Err(EngineError::Illegal("behold requires a selected object"))
                        }
                    }
                }
            };
            let label = match option {
                CastCostOptionDef::Mana { label, .. } | CastCostOptionDef::Behold { label, .. } => {
                    label.clone()
                }
            };
            cast_cost_receipts.push(CastCostReceipt {
                group_index: group_index as u32,
                option_index: selection.option_index,
                label,
                object,
            });
        }
        let mut harmonize_reduction = 0;
        if cast_method == SpellCastMethod::Harmonize {
            if let Some(selection) = group_selections.get(&harmonize_group_index) {
                use rv1::cast_cost_group_selection::SelectedObject;
                if selection.option_index != 0 {
                    return Err(EngineError::Illegal("invalid harmonize cost option"));
                }
                let Some(SelectedObject::PermanentId(object_id)) = selection.selected_object else {
                    return Err(EngineError::Illegal(
                        "harmonize requires a selected permanent",
                    ));
                };
                let object = self
                    .state
                    .objects
                    .get(&object_id)
                    .ok_or(EngineError::Illegal("unknown harmonize permanent"))?;
                let generation = self
                    .state
                    .zone_change_generation
                    .get(&object_id)
                    .copied()
                    .unwrap_or(0);
                let characteristics = self
                    .characteristics(object_id)
                    .ok_or(EngineError::Illegal("invalid harmonize permanent"))?;
                if object.zone != Zone::Battlefield
                    || object.tapped
                    || characteristics.controller != player
                    || !characteristics.is_creature()
                {
                    return Err(EngineError::Illegal(
                        "harmonize requires an untapped creature you control",
                    ));
                }
                if selection.expected_zone_change_generation != generation {
                    return Err(EngineError::Illegal("stale harmonize permanent selection"));
                }
                harmonize_reduction = characteristics.power.unwrap_or(0);
                debits.push(CostDebit::Tap {
                    object_id,
                    generation,
                });
                cast_cost_receipts.push(CastCostReceipt {
                    group_index: harmonize_group_index as u32,
                    option_index: 0,
                    label: format!(
                        "Harmonize — tap {} (reduce {{{harmonize_reduction}}})",
                        object_display_name(&self.state, self.registry, object_id)
                    ),
                    object: Some(CastCostObjectReceipt::ChosenPermanent {
                        object_id,
                        zone_change_generation: generation,
                        card_id: object.card_id.clone(),
                        card_name: object_display_name(&self.state, self.registry, object_id),
                    }),
                });
            }
        }
        if group_selections.len() != cast_cost_group_selections.len() {
            return Err(EngineError::Illegal("unexpected cast cost group selection"));
        }
        let mut consumed = HashSet::new();
        for (cost_index, cost) in costs.iter().enumerate() {
            let Some(selection) = by_index.get(&cost_index) else {
                return Err(EngineError::Illegal("missing additional cost selection"));
            };
            match cost {
                AdditionalCost::DiscardCard => {
                    let Some(Selection::HandIndex(hand_index)) = selection.selection else {
                        return Err(EngineError::Illegal("discard cost requires a hand card"));
                    };
                    let oid = self.state.players[player_idx]
                        .hand
                        .get(hand_index as usize)
                        .copied()
                        .ok_or(EngineError::Illegal("invalid discard hand slot"))?;
                    if oid == source_oid {
                        return Err(EngineError::Illegal(
                            "a spell cannot discard itself as a cost",
                        ));
                    }
                    if !consumed.insert(oid) {
                        return Err(EngineError::Illegal("one object cannot pay two costs"));
                    }
                    let object = &self.state.objects[&oid];
                    debits.push(CostDebit::Discard {
                        object_id: oid,
                        generation: self
                            .state
                            .zone_change_generation
                            .get(&oid)
                            .copied()
                            .unwrap_or(0),
                        owner: object.owner,
                    });
                }
                AdditionalCost::SacrificePermanent { filter } => {
                    let Some(Selection::PermanentId(oid)) = selection.selection else {
                        return Err(EngineError::Illegal(
                            "sacrifice cost requires a battlefield permanent",
                        ));
                    };
                    if !self.ability_cost_permanent_matches(player, None, oid, filter) {
                        return Err(EngineError::Illegal("illegal sacrifice cost selection"));
                    }
                    if !consumed.insert(oid) {
                        return Err(EngineError::Illegal("one object cannot pay two costs"));
                    }
                    let snapshot = self
                        .sacrifice_snapshot(oid)
                        .ok_or(EngineError::Illegal("sacrifice permanent missing"))?;
                    let owner = self.state.objects[&oid].owner;
                    debits.push(CostDebit::Sacrifice { snapshot, owner });
                }
                AdditionalCost::TapPermanents {
                    count,
                    filter,
                    exclude_source,
                } => {
                    let Some(Selection::BattlefieldObjects(selected)) =
                        selection.selection.as_ref()
                    else {
                        return Err(EngineError::Illegal(
                            "tap cost requires battlefield object references",
                        ));
                    };
                    if selected.objects.len() != *count as usize {
                        return Err(EngineError::Illegal("incorrect tap cost selection count"));
                    }
                    for selected in &selected.objects {
                        let oid = selected.object_id;
                        if (*exclude_source && oid == source_oid)
                            || !self.ability_cost_permanent_matches(player, None, oid, filter)
                            || self
                                .state
                                .objects
                                .get(&oid)
                                .is_none_or(|object| object.tapped)
                        {
                            return Err(EngineError::Illegal("illegal tap cost selection"));
                        }
                        let generation = self
                            .state
                            .zone_change_generation
                            .get(&oid)
                            .copied()
                            .unwrap_or(0);
                        if generation != selected.zone_change_generation {
                            return Err(EngineError::Illegal("stale tap cost selection"));
                        }
                        if !consumed.insert(oid) {
                            return Err(EngineError::Illegal("one object cannot pay two costs"));
                        }
                        debits.push(CostDebit::Tap {
                            object_id: oid,
                            generation,
                        });
                    }
                }
            }
        }
        if selections.len() != costs.len() {
            return Err(EngineError::Illegal("unexpected cost selection"));
        }

        Ok(PreparedSpellCosts {
            transaction: CostTransactionPlan {
                purpose: CostPurpose::Spell,
                player,
                player_idx,
                debits,
                cast_cost_receipts,
            },
            mana: combined_mana,
            x_value,
            extra_generic,
            generic_reduction: generic_reduction.saturating_add(harmonize_reduction),
            flex_payments: flex_payments.to_vec(),
            restricted_mana: restricted_mana.to_vec(),
            eligible_restricted_mana: eligible_restricted_mana.to_vec(),
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub(in crate::engine) fn plan_ability_costs(
        &self,
        player: PlayerId,
        idx: usize,
        permanent_id: ObjectId,
        costs: &[AbilityCost],
        flex_payments: &[rv1::FlexPipPayment],
        selections: &[rv1::CostSelection],
        restricted_mana: &[rv1::ManaSpendSelection],
        extra_generic: u32,
        generic_reduction: u32,
    ) -> Result<CostTransactionPlan, EngineError> {
        use rv1::cost_selection::Selection;

        let source_card_id = self
            .state
            .objects
            .get(&permanent_id)
            .map(|object| object.card_id.as_str())
            .ok_or(EngineError::Illegal("permanent missing"))?;
        let mut by_index = HashMap::new();
        for selection in selections {
            let cost_index = selection.cost_index as usize;
            if cost_index >= costs.len() || by_index.insert(cost_index, selection).is_some() {
                return Err(EngineError::Illegal("invalid or duplicate cost selection"));
            }
        }

        let mut debits = Vec::with_capacity(costs.len());
        let mut consumed = HashSet::new();
        let mut expected_selections = 0usize;
        let mut saw_mana = false;
        let mut saw_tap = false;
        let eligible_restricted_mana = self.eligible_restricted_mana_for_ability(idx, permanent_id);
        for (cost_index, cost) in costs.iter().enumerate() {
            match cost {
                AbilityCost::Loyalty(delta) => {
                    let object = self
                        .state
                        .objects
                        .get(&permanent_id)
                        .ok_or(EngineError::Illegal("planeswalker missing"))?;
                    if object.zone != Zone::Battlefield
                        || object.controller != player
                        || !self
                            .characteristics(permanent_id)
                            .is_some_and(|value| value.has_type("Planeswalker"))
                    {
                        return Err(EngineError::Illegal(
                            "loyalty cost requires a planeswalker you control",
                        ));
                    }
                    if *delta < 0
                        && object.counter_count(CounterKind::Loyalty) < delta.unsigned_abs()
                    {
                        return Err(EngineError::Illegal("not enough loyalty counters"));
                    }
                    debits.push(CostDebit::Loyalty {
                        object_id: permanent_id,
                        generation: self
                            .state
                            .zone_change_generation
                            .get(&permanent_id)
                            .copied()
                            .unwrap_or(0),
                        delta: *delta,
                    });
                }
                AbilityCost::Tap => {
                    if saw_tap {
                        return Err(EngineError::Illegal("duplicate tap cost"));
                    }
                    self.check_tappable(permanent_id, source_card_id)?;
                    saw_tap = true;
                    debits.push(CostDebit::Tap {
                        object_id: permanent_id,
                        generation: self
                            .state
                            .zone_change_generation
                            .get(&permanent_id)
                            .copied()
                            .unwrap_or(0),
                    });
                }
                AbilityCost::TapPermanents {
                    count,
                    filter,
                    exclude_source,
                } => {
                    expected_selections += 1;
                    let Some(selection) = by_index.get(&cost_index) else {
                        return Err(EngineError::Illegal("missing tap cost selection"));
                    };
                    let Some(Selection::BattlefieldObjects(selected)) =
                        selection.selection.as_ref()
                    else {
                        return Err(EngineError::Illegal(
                            "tap cost requires battlefield object references",
                        ));
                    };
                    if selected.objects.len() != *count as usize {
                        return Err(EngineError::Illegal("incorrect tap cost selection count"));
                    }
                    for selected in &selected.objects {
                        let oid = selected.object_id;
                        if (*exclude_source && oid == permanent_id)
                            || !self.ability_cost_permanent_matches(
                                player,
                                Some(permanent_id),
                                oid,
                                filter,
                            )
                            || self
                                .state
                                .objects
                                .get(&oid)
                                .is_none_or(|object| object.tapped)
                        {
                            return Err(EngineError::Illegal("illegal tap cost selection"));
                        }
                        let generation = self
                            .state
                            .zone_change_generation
                            .get(&oid)
                            .copied()
                            .unwrap_or(0);
                        if generation != selected.zone_change_generation {
                            return Err(EngineError::Illegal("stale tap cost selection"));
                        }
                        if !consumed.insert(oid) {
                            return Err(EngineError::Illegal("one object cannot pay two costs"));
                        }
                        debits.push(CostDebit::Tap {
                            object_id: oid,
                            generation,
                        });
                    }
                }
                AbilityCost::Mana(cost) => {
                    if saw_mana {
                        return Err(EngineError::Illegal("multiple mana cost components"));
                    }
                    saw_mana = true;
                    debits.push(CostDebit::Mana(
                        plan_mana_payment_with_restricted_reduction(
                            &self.state,
                            idx,
                            cost,
                            0,
                            extra_generic,
                            generic_reduction,
                            flex_payments,
                            restricted_mana,
                            &eligible_restricted_mana,
                        )?,
                    ));
                }
                AbilityCost::Discard => {
                    expected_selections += 1;
                    let Some(selection) = by_index.get(&cost_index) else {
                        return Err(EngineError::Illegal("missing discard cost selection"));
                    };
                    let Some(Selection::HandIndex(hand_index)) = selection.selection else {
                        return Err(EngineError::Illegal("discard cost requires a hand card"));
                    };
                    let oid = self.state.players[idx]
                        .hand
                        .get(hand_index as usize)
                        .copied()
                        .ok_or(EngineError::Illegal("invalid discard hand slot"))?;
                    if !consumed.insert(oid) {
                        return Err(EngineError::Illegal("one object cannot pay two costs"));
                    }
                    let object = &self.state.objects[&oid];
                    debits.push(CostDebit::Discard {
                        object_id: oid,
                        generation: self
                            .state
                            .zone_change_generation
                            .get(&oid)
                            .copied()
                            .unwrap_or(0),
                        owner: object.owner,
                    });
                }
                AbilityCost::DiscardSelf => {
                    if !consumed.insert(permanent_id) {
                        return Err(EngineError::Illegal("one object cannot pay two costs"));
                    }
                    let object = &self.state.objects[&permanent_id];
                    debits.push(CostDebit::Discard {
                        object_id: permanent_id,
                        generation: self
                            .state
                            .zone_change_generation
                            .get(&permanent_id)
                            .copied()
                            .unwrap_or(0),
                        owner: object.owner,
                    });
                }
                AbilityCost::ExileSelf => {
                    if !consumed.insert(permanent_id) {
                        return Err(EngineError::Illegal("one object cannot pay two costs"));
                    }
                    let object = &self.state.objects[&permanent_id];
                    debits.push(CostDebit::Exile {
                        object_id: permanent_id,
                        generation: self
                            .state
                            .zone_change_generation
                            .get(&permanent_id)
                            .copied()
                            .unwrap_or(0),
                        owner: object.owner,
                    });
                }
                AbilityCost::SacrificeSelf => {
                    if !consumed.insert(permanent_id) {
                        return Err(EngineError::Illegal("one object cannot pay two costs"));
                    }
                    let snapshot = self
                        .sacrifice_snapshot(permanent_id)
                        .ok_or(EngineError::Illegal("sacrifice permanent missing"))?;
                    let owner = self.state.objects[&permanent_id].owner;
                    debits.push(CostDebit::Sacrifice { snapshot, owner });
                }
                AbilityCost::SacrificePermanent { filter } => {
                    expected_selections += 1;
                    let Some(selection) = by_index.get(&cost_index) else {
                        return Err(EngineError::Illegal("missing sacrifice cost selection"));
                    };
                    let Some(Selection::PermanentId(oid)) = selection.selection else {
                        return Err(EngineError::Illegal(
                            "sacrifice cost requires a battlefield permanent",
                        ));
                    };
                    if !self.ability_cost_permanent_matches(player, Some(permanent_id), oid, filter)
                    {
                        return Err(EngineError::Illegal("illegal sacrifice cost selection"));
                    }
                    if !consumed.insert(oid) {
                        return Err(EngineError::Illegal("one object cannot pay two costs"));
                    }
                    let snapshot = self
                        .sacrifice_snapshot(oid)
                        .ok_or(EngineError::Illegal("sacrifice permanent missing"))?;
                    let owner = self.state.objects[&oid].owner;
                    debits.push(CostDebit::Sacrifice { snapshot, owner });
                }
                AbilityCost::ExileGraveyardCards {
                    count,
                    filter,
                    exclude_source,
                } => {
                    expected_selections += 1;
                    let Some(selection) = by_index.get(&cost_index) else {
                        return Err(EngineError::Illegal(
                            "missing graveyard-card cost selection",
                        ));
                    };
                    let Some(Selection::GraveyardObjectIds(selected)) =
                        selection.selection.as_ref()
                    else {
                        return Err(EngineError::Illegal(
                            "graveyard-card cost requires graveyard object ids",
                        ));
                    };
                    if selected.object_ids.len() != *count as usize {
                        return Err(EngineError::Illegal(
                            "incorrect graveyard-card cost selection count",
                        ));
                    }
                    for &oid in &selected.object_ids {
                        if (*exclude_source && oid == permanent_id)
                            || !self.state.players[idx].graveyard.contains(&oid)
                            || !super::super::resolution::library_card_matches_filter(
                                &self.state,
                                self.registry,
                                oid,
                                Some(filter),
                            )
                        {
                            return Err(EngineError::Illegal(
                                "illegal graveyard-card cost selection",
                            ));
                        }
                        if !consumed.insert(oid) {
                            return Err(EngineError::Illegal("one object cannot pay two costs"));
                        }
                        let object = &self.state.objects[&oid];
                        debits.push(CostDebit::Exile {
                            object_id: oid,
                            generation: self
                                .state
                                .zone_change_generation
                                .get(&oid)
                                .copied()
                                .unwrap_or(0),
                            owner: object.owner,
                        });
                    }
                }
            }
        }
        if !saw_mana && extra_generic > 0 {
            debits.push(CostDebit::Mana(
                plan_mana_payment_with_restricted_reduction(
                    &self.state,
                    idx,
                    &ManaCost::default(),
                    0,
                    extra_generic,
                    generic_reduction,
                    flex_payments,
                    restricted_mana,
                    &eligible_restricted_mana,
                )?,
            ));
        }
        if selections.len() != expected_selections {
            return Err(EngineError::Illegal("unexpected cost selection"));
        }
        if !restricted_mana.is_empty() && !saw_mana && extra_generic == 0 {
            return Err(EngineError::Illegal(
                "restricted mana supplied for an ability with no mana cost",
            ));
        }

        Ok(CostTransactionPlan {
            purpose: CostPurpose::Ability,
            player,
            player_idx: idx,
            debits,
            cast_cost_receipts: vec![],
        })
    }

    pub(in crate::engine) fn commit_cost_transaction(
        &mut self,
        plan: CostTransactionPlan,
    ) -> Result<CostPaymentReceipt, EngineError> {
        self.revalidate_cost_transaction(&plan)?;

        let mut payment = CostPaymentReceipt {
            move_events: vec![],
            tap_events: vec![],
            sacrificed: vec![],
            paid_card_costs: vec![],
            life_paid: 0,
            mana_spent: 0,
            expend_triggers: vec![],
            restricted_mana_spent: vec![],
            cast_cost_receipts: plan.cast_cost_receipts,
        };
        for debit in plan.debits {
            match debit {
                CostDebit::Loyalty {
                    object_id, delta, ..
                } => {
                    let timestamp = self.state.command_index;
                    let object = self
                        .state
                        .objects
                        .get_mut(&object_id)
                        .expect("prevalidated loyalty source must commit");
                    if delta >= 0 {
                        object.add_counters(CounterKind::Loyalty, delta as u32, timestamp);
                    } else {
                        let current = object.counter_count(CounterKind::Loyalty);
                        object.set_counter(
                            CounterKind::Loyalty,
                            current.saturating_sub(delta.unsigned_abs()),
                        );
                    }
                }
                CostDebit::Tap { object_id, .. } => {
                    if let Some(event) = crate::engine::become_tapped(&mut self.state, object_id) {
                        payment.tap_events.push(event);
                    }
                }
                CostDebit::Mana(mana) => {
                    let spent = mana.mana_spent();
                    payment.mana_spent += spent;
                    payment.life_paid += mana.life_cost;
                    payment
                        .restricted_mana_spent
                        .extend(mana.restricted_spent.iter().copied());
                    commit_mana_payment(&mut self.state, plan.player_idx, mana);
                    if plan.purpose == CostPurpose::Spell {
                        let event = self.record_spell_mana_spent(plan.player, spent);
                        // Mana is the first spell debit, so capture before later sacrifices.
                        // Stage only after casting completes, with every other waiting trigger.
                        payment
                            .expend_triggers
                            .extend(self.collect_event_triggers(&[event]));
                    }
                }
                CostDebit::Discard {
                    object_id: oid,
                    owner,
                    ..
                } => {
                    let (card_name, moved) = crate::engine::resolution::perform_discard(
                        &mut self.state,
                        self.registry,
                        owner,
                        oid,
                    )
                    .expect("prevalidated discard cost must commit");
                    payment.move_events.push(moved);
                    let paid_cost = PaidCardCost::Discard {
                        object_id: oid,
                        card_name,
                        result: card_result_entry(
                            &self.state,
                            self.registry,
                            CardResultAction::Discard,
                            owner,
                            oid,
                        ),
                    };
                    debug_assert_eq!(paid_cost.object_id(), oid);
                    payment.paid_card_costs.push(paid_cost);
                }
                CostDebit::Exile {
                    object_id: oid,
                    owner,
                    ..
                } => {
                    let card_name = object_display_name(&self.state, self.registry, oid);
                    crate::engine::resolution::move_object_to_zone(
                        &mut self.state,
                        self.registry,
                        oid,
                        Zone::Exile,
                        None,
                    )
                    .expect("prevalidated exile cost must commit");
                    payment.move_events.push(permanent_moved_event(
                        &self.state,
                        oid,
                        owner,
                        rv1::permanent_moved::Destination::Exile,
                    ));
                    let paid_cost = PaidCardCost::Exile {
                        object_id: oid,
                        card_name,
                        result: card_result_entry(
                            &self.state,
                            self.registry,
                            CardResultAction::Exile,
                            owner,
                            oid,
                        ),
                    };
                    debug_assert_eq!(paid_cost.object_id(), oid);
                    payment.paid_card_costs.push(paid_cost);
                }
                CostDebit::Sacrifice { snapshot, owner } => {
                    let mut snapshot = snapshot;
                    let oid = snapshot.source.object_id;
                    let card_name = object_display_name(&self.state, self.registry, oid);
                    snapshot.died = sacrifice_permanent(&mut self.state, self.registry, oid)
                        .expect("prevalidated sacrifice cost must commit");
                    payment.sacrificed.push(snapshot);
                    payment.move_events.push(permanent_moved_event(
                        &self.state,
                        oid,
                        owner,
                        rv1::permanent_moved::Destination::Graveyard,
                    ));
                    let paid_cost = PaidCardCost::Sacrifice {
                        object_id: oid,
                        card_name,
                        result: card_result_entry(
                            &self.state,
                            self.registry,
                            CardResultAction::Sacrifice,
                            owner,
                            oid,
                        ),
                    };
                    debug_assert_eq!(paid_cost.object_id(), oid);
                    payment.paid_card_costs.push(paid_cost);
                }
                CostDebit::ObserveHand { .. } | CostDebit::ObservePermanent { .. } => {}
            }
        }
        Ok(payment)
    }

    fn revalidate_cost_transaction(&self, plan: &CostTransactionPlan) -> Result<(), EngineError> {
        if self.state.player_idx(plan.player) != Some(plan.player_idx) {
            return Err(EngineError::Illegal("cost transaction player changed"));
        }
        for debit in &plan.debits {
            let valid =
                match debit {
                    CostDebit::Loyalty {
                        object_id,
                        generation,
                        delta,
                    } => {
                        self.state.objects.get(object_id).is_some_and(|object| {
                            object.zone == Zone::Battlefield
                                && object.controller == plan.player
                                && (*delta >= 0
                                    || object.counter_count(CounterKind::Loyalty)
                                        >= delta.unsigned_abs())
                        }) && self
                            .characteristics(*object_id)
                            .is_some_and(|value| value.has_type("Planeswalker"))
                            && self
                                .state
                                .zone_change_generation
                                .get(object_id)
                                .copied()
                                .unwrap_or(0)
                                == *generation
                    }
                    CostDebit::Mana(mana) => {
                        mana_payment_still_valid(&self.state, plan.player_idx, mana)
                    }
                    CostDebit::Tap {
                        object_id,
                        generation,
                    } => {
                        self.state.objects.get(object_id).is_some_and(|object| {
                            object.zone == Zone::Battlefield && !object.tapped
                        }) && self
                            .state
                            .zone_change_generation
                            .get(object_id)
                            .copied()
                            .unwrap_or(0)
                            == *generation
                    }
                    CostDebit::Discard {
                        object_id,
                        generation,
                        owner,
                    } => {
                        self.state.objects.get(object_id).is_some_and(|object| {
                            object.zone == Zone::Hand
                                && object.owner == *owner
                                && self.state.player_idx(*owner).is_some()
                                && self.state.players[plan.player_idx].hand.contains(object_id)
                        }) && self
                            .state
                            .zone_change_generation
                            .get(object_id)
                            .copied()
                            .unwrap_or(0)
                            == *generation
                    }
                    CostDebit::Exile {
                        object_id,
                        generation,
                        owner,
                    } => {
                        self.state.objects.get(object_id).is_some_and(|object| {
                            object.zone == Zone::Graveyard
                                && object.owner == *owner
                                && self.state.player_idx(*owner).is_some()
                                && self.state.players[plan.player_idx]
                                    .graveyard
                                    .contains(object_id)
                        }) && self
                            .state
                            .zone_change_generation
                            .get(object_id)
                            .copied()
                            .unwrap_or(0)
                            == *generation
                    }
                    CostDebit::Sacrifice { snapshot, owner } => {
                        let object_id = snapshot.source.object_id;
                        self.state.objects.get(&object_id).is_some_and(|object| {
                            object.zone == Zone::Battlefield
                                && object.owner == *owner
                                && self.state.player_idx(*owner).is_some()
                        }) && self
                            .state
                            .zone_change_generation
                            .get(&object_id)
                            .copied()
                            .unwrap_or(0)
                            == snapshot.source.zone_change_generation
                    }
                    CostDebit::ObserveHand {
                        object_id,
                        generation,
                        owner,
                    } => {
                        self.state.objects.get(object_id).is_some_and(|object| {
                            object.zone == Zone::Hand
                                && object.owner == *owner
                                && *owner == plan.player
                                && self.state.players[plan.player_idx].hand.contains(object_id)
                        }) && self
                            .state
                            .zone_change_generation
                            .get(object_id)
                            .copied()
                            .unwrap_or(0)
                            == *generation
                    }
                    CostDebit::ObservePermanent {
                        object_id,
                        generation,
                        controller,
                    } => {
                        self.state.objects.get(object_id).is_some_and(|object| {
                            object.zone == Zone::Battlefield && object.controller == *controller
                        }) && self
                            .state
                            .zone_change_generation
                            .get(object_id)
                            .copied()
                            .unwrap_or(0)
                            == *generation
                    }
                };
            if !valid {
                return Err(EngineError::Illegal("cost transaction became stale"));
            }
        }
        Ok(())
    }

    pub(in crate::engine) fn ability_cost_permanent_matches(
        &self,
        player: PlayerId,
        source: Option<ObjectId>,
        oid: ObjectId,
        filter: &TargetFilter,
    ) -> bool {
        if let Some(branches) = &filter.any_of {
            return branches
                .iter()
                .any(|branch| self.ability_cost_permanent_matches(player, source, oid, branch));
        }
        if filter.exclude_source && source == Some(oid) {
            return false;
        }
        let Some(object) = self.state.objects.get(&oid) else {
            return false;
        };
        if object.zone != Zone::Battlefield || object.controller != player {
            return false;
        }
        let Some(characteristics) = self.characteristics(oid) else {
            return false;
        };
        let kind_matches = match filter.kind {
            TargetKind::Creature => characteristics.is_creature(),
            TargetKind::AnyPermanent => true,
            _ => false,
        };
        kind_matches && crate::engine::targeting::filter_characteristics_match(self, filter, oid)
    }

    /// Snapshot a permanent about to be sacrificed as an activation cost. Taken *before* the cost
    /// is paid, because CR 603.6 reads the dying object's last-known information and the object is
    /// already in the graveyard (controller reset, characteristics gone) by the time it fires.
    fn sacrifice_snapshot(&self, permanent_id: ObjectId) -> Option<SacrificeSnapshot> {
        let source = self.trigger_source_snapshot(permanent_id)?;
        Some(SacrificeSnapshot {
            source,
            was_creature: self
                .characteristics(permanent_id)
                .is_some_and(|value| value.is_creature()),
            died: false,
        })
    }
}

#[cfg(test)]
mod convoke_transaction_tests {
    use super::*;

    #[test]
    fn issue_172_receipts_count_actual_mana_not_cost_or_life() {
        // Costs, X, taxes and reductions exercise the shared planner, not a printed-MV proxy.
        for (cost, x, extra, reduction, phyrexian, expected) in [
            ("{2}{G}", 0, 0, 2, false, 1),
            ("{X}{G}", 3, 2, 1, false, 5),
            ("{2}{G}", 0, 0, 0, false, 3),
            ("{0}", 0, 0, 0, false, 0),
            ("{G/P}", 0, 0, 0, true, 0),
            ("{G/P}", 0, 0, 0, false, 1),
        ] {
            for purpose in [CostPurpose::Spell, CostPurpose::Ability] {
                let mut engine = GameEngine::new(172010, &[0, 1], 20, None, true).unwrap();
                engine.state.players[0].mana_pool.green = 1;
                engine.state.players[0].mana_pool.colorless = 10;
                engine.state.players[0].retained_combat_mana.colorless = 10;
                let flex = if phyrexian {
                    vec![rv1::FlexPipPayment {
                        pip_index: 0,
                        pay_life: true,
                    }]
                } else {
                    vec![]
                };
                let mana = super::super::mana::plan_mana_payment_with_reduction(
                    &engine.state,
                    0,
                    &ManaCost::parse(cost).unwrap(),
                    x,
                    extra,
                    reduction,
                    &flex,
                )
                .unwrap();
                let payment = engine
                    .commit_cost_transaction(CostTransactionPlan {
                        purpose,
                        player: 0,
                        player_idx: 0,
                        debits: vec![CostDebit::Mana(mana)],
                        cast_cost_receipts: vec![],
                    })
                    .unwrap();
                assert_eq!(payment.mana_spent, expected, "{cost}");
                assert_eq!(payment.life_paid, if phyrexian { 2 } else { 0 });
                assert_eq!(
                    engine
                        .state
                        .turn_history
                        .current
                        .player(0)
                        .mana_spent_casting_spells,
                    if purpose == CostPurpose::Spell {
                        expected
                    } else {
                        0
                    }
                );
            }
        }
    }

    #[test]
    fn issue_172_restricted_and_retained_mana_are_counted_once_and_stale_debits_are_atomic() {
        let mut engine = GameEngine::new(172011, &[0, 1], 20, None, true).unwrap();
        engine.state.players[0].mana_pool.colorless = 2;
        engine.state.players[0].retained_combat_mana.colorless = 2;
        engine.state.players[0]
            .restricted_mana
            .push(crate::state::RestrictedManaContribution {
                restriction_group_id: 7,
                amount: ManaAmount {
                    g: 1,
                    ..Default::default()
                },
            });
        let selections = [rv1::ManaSpendSelection {
            restriction_group_id: 7,
            g: 1,
            ..Default::default()
        }];
        let mana = plan_mana_payment_with_restricted_reduction(
            &engine.state,
            0,
            &ManaCost::parse("{2}{G}").unwrap(),
            0,
            0,
            0,
            &[],
            &selections,
            &[7],
        )
        .unwrap();
        assert_eq!(mana.mana_spent(), 3);
        let stale = CostTransactionPlan {
            purpose: CostPurpose::Spell,
            player: 0,
            player_idx: 0,
            debits: vec![
                CostDebit::Mana(mana.clone()),
                CostDebit::Tap {
                    object_id: u32::MAX,
                    generation: 0,
                },
            ],
            cast_cost_receipts: vec![],
        };
        let before = format!("{:?}", engine.state);
        assert!(engine.commit_cost_transaction(stale).is_err());
        assert_eq!(format!("{:?}", engine.state), before);
        let payment = engine
            .commit_cost_transaction(CostTransactionPlan {
                purpose: CostPurpose::Spell,
                player: 0,
                player_idx: 0,
                debits: vec![CostDebit::Mana(mana)],
                cast_cost_receipts: vec![],
            })
            .unwrap();
        assert_eq!(payment.mana_spent, 3);
        assert_eq!(
            engine
                .state
                .turn_history
                .current
                .player(0)
                .mana_spent_casting_spells,
            3
        );
        assert!(engine.state.players[0].restricted_mana.is_empty());
        assert_eq!(engine.state.players[0].retained_combat_mana.colorless, 0);
    }

    #[test]
    fn convoke_can_tap_then_sacrifice_but_not_pay_a_second_tap_cost() {
        let mut engine = GameEngine::new(
            145,
            &[0, 1],
            20,
            Some(vec![
                vec!["grizzly_bears".into(); 12],
                vec!["island".into(); 12],
            ]),
            true,
        )
        .unwrap();
        let source = engine.state.players[0].hand[0];
        let bear = engine.state.players[0].hand[1];
        crate::engine::resolution::move_object_to_zone(
            &mut engine.state,
            engine.registry,
            bear,
            Zone::Battlefield,
            Some(0),
        )
        .unwrap();
        let reference = rv1::CostObjectRef {
            object_id: bear,
            zone_change_generation: engine.state.zone_change_generation[&bear],
        };
        let costs = [AdditionalCost::SacrificePermanent {
            filter: TargetFilter {
                kind: TargetKind::Creature,
                ..Default::default()
            },
        }];
        let selections = [rv1::CostSelection {
            cost_index: 0,
            selection: Some(rv1::cost_selection::Selection::PermanentId(bear)),
        }];
        let prepared = engine
            .prepare_spell_costs(
                0,
                0,
                source,
                &ManaCost::parse("{1}").unwrap(),
                0,
                0,
                0,
                &[],
                &costs,
                &selections,
                &[],
                &[],
                &[],
                &[],
                SpellCastMethod::Normal,
            )
            .unwrap();
        assert!(prepared.can_convoke(bear));
        let payment = rv1::SpellPaymentSelection {
            expected_state_revision: engine.state.command_index,
            source: Some(rv1::CostObjectRef {
                object_id: source,
                zone_change_generation: engine
                    .state
                    .zone_change_generation
                    .get(&source)
                    .copied()
                    .unwrap_or(0),
            }),
            convoke: vec![rv1::ConvokeContribution {
                object: Some(reference),
                kind: rv1::ConvokePaymentKind::Generic as i32,
            }],
            mana: None,
        };
        let life = engine
            .validate_explicit_spell_payment(0, source, true, &prepared, &payment)
            .unwrap();
        let plan = prepared
            .finish_explicit(&engine.state, &payment, life)
            .unwrap();
        // The debit order carries the rule, and revalidation happens before either mutation.
        assert!(matches!(plan.debits[1], CostDebit::Tap { object_id, .. } if object_id == bear));
        assert!(matches!(plan.debits[2], CostDebit::Sacrifice { .. }));
        let probe = PreparedSpellCosts {
            transaction: CostTransactionPlan {
                purpose: CostPurpose::Spell,
                player: 0,
                player_idx: 0,
                debits: vec![CostDebit::Tap {
                    object_id: bear,
                    generation: reference.zone_change_generation,
                }],
                cast_cost_receipts: vec![],
            },
            mana: ManaCost::default(),
            x_value: 0,
            extra_generic: 0,
            generic_reduction: 0,
            flex_payments: vec![],
            restricted_mana: vec![],
            eligible_restricted_mana: vec![],
        };
        assert!(!probe.can_convoke(bear));
        let committed = engine.commit_cost_transaction(plan).unwrap();
        assert_eq!(committed.tap_events.len(), 1);
        assert_eq!(committed.sacrificed.len(), 1);
        assert_eq!(engine.state.objects[&bear].zone, Zone::Graveyard);
    }
}
