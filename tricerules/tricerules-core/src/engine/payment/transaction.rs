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
    Waterbend,
    RemoveCounters {
        object: rv1::CostObjectRef,
        kind: CounterKind,
        count: u32,
    },
    Blight {
        object: rv1::CostObjectRef,
        count: u32,
    },
    Loyalty {
        object_id: ObjectId,
        generation: u64,
        delta: i32,
    },
    Tap {
        object_id: ObjectId,
        generation: u64,
    },
    /// One authored multi-permanent tap component, distinct from a separate source {T} cost.
    TapGroup(Vec<(ObjectId, u64)>),
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
    /// All cards selected for one exile instruction leave simultaneously.
    ExileGroup(Vec<(ObjectId, u64, PlayerId)>),
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
    pub(in crate::engine) blight_receipts: Vec<crate::state::BlightReceipt>,
    pub(in crate::engine) move_events: Vec<rv1::RuledEvent>,
    pub(in crate::engine) trigger_events: Vec<GameEvent>,
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

pub(in crate::engine) struct PreparedPaymentCosts {
    pub waterbend_limit: Option<u32>,
    pub transaction: CostTransactionPlan,
    pub mana: ManaCost,
    pub x_value: u32,
    pub extra_generic: u32,
    pub generic_reduction: u32,
    pub flex_payments: Vec<rv1::FlexPipPayment>,
    pub restricted_mana: Vec<rv1::ManaSpendSelection>,
    pub eligible_restricted_mana: Vec<u32>,
}

impl PreparedPaymentCosts {
    pub fn can_convoke(&self, oid: ObjectId) -> bool {
        !self.transaction.debits.iter().any(|d| match d {
            CostDebit::Tap { object_id, .. } => *object_id == oid,
            CostDebit::TapGroup(objects) => objects.iter().any(|(id, _)| *id == oid),
            _ => false,
        })
    }

    pub fn finish_explicit(
        mut self,
        state: &GameState,
        selection: &rv1::PaymentSelection,
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
            .chain(selection.waterbend.iter())
            .map(|c| (c.object_id, c.zone_change_generation))
            .collect::<Vec<_>>();
        // CR 601.2h permits tapping for Convoke before sacrificing the same creature as another
        // cost. A second tap is forbidden by can_convoke; sacrifice is deliberately not excluded.
        if !taps.is_empty() {
            self.transaction.debits.insert(0, CostDebit::TapGroup(taps));
        }
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
    pub(in crate::engine) fn prepare_resolution_payment_costs(
        &self,
        player: PlayerId,
        payment: &PendingManaPayment,
        restricted_mana: &[rv1::ManaSpendSelection],
    ) -> Result<PreparedPaymentCosts, EngineError> {
        let idx = self
            .state
            .player_idx(player)
            .ok_or(EngineError::UnknownPlayer(player))?;
        let cost = if payment.mana_cost.pips.is_empty() {
            ManaCost {
                pips: vec![ManaSymbol::Generic(payment.generic_mana_cost)],
            }
        } else {
            payment.mana_cost.clone()
        };
        Ok(PreparedPaymentCosts {
            waterbend_limit: payment
                .waterbend
                .then(|| super::waterbend::generic_component(&cost))
                .transpose()?,
            transaction: CostTransactionPlan {
                purpose: CostPurpose::Ability,
                player,
                player_idx: idx,
                debits: if payment.waterbend {
                    vec![CostDebit::Waterbend]
                } else {
                    vec![]
                },
                cast_cost_receipts: vec![],
            },
            mana: cost,
            x_value: 0,
            extra_generic: 0,
            generic_reduction: 0,
            flex_payments: vec![],
            restricted_mana: restricted_mana.to_vec(),
            eligible_restricted_mana: vec![],
        })
    }

    fn plan_blight_selection(
        &self,
        player: PlayerId,
        count: u32,
        selection: &rv1::CostSelection,
    ) -> Result<CostDebit, EngineError> {
        let Some(rv1::cost_selection::Selection::BattlefieldObjects(objects)) =
            &selection.selection
        else {
            return Err(EngineError::Illegal(
                "Blight requires a generation-bound creature",
            ));
        };
        let [object] = objects.objects.as_slice() else {
            return Err(EngineError::Illegal("Blight requires exactly one creature"));
        };
        self.validate_blight(player, count, object)?;
        Ok(CostDebit::Blight {
            object: *object,
            count,
        })
    }

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
    ) -> Result<PreparedPaymentCosts, EngineError> {
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
                CastCostOptionDef::Blight { count, .. } => {
                    let Some(rv1::cast_cost_group_selection::SelectedObject::PermanentId(
                        object_id,
                    )) = selection.selected_object
                    else {
                        return Err(EngineError::Illegal(
                            "Blight requires one battlefield creature",
                        ));
                    };
                    let reference = rv1::CostObjectRef {
                        object_id,
                        zone_change_generation: selection.expected_zone_change_generation,
                    };
                    self.validate_blight(player, *count, &reference)?;
                    debits.push(CostDebit::Blight {
                        object: reference,
                        count: *count,
                    });
                    Some(CastCostObjectReceipt::ChosenPermanent {
                        object_id,
                        zone_change_generation: selection.expected_zone_change_generation,
                        card_id: self.state.objects[&object_id].card_id.clone(),
                        card_name: object_display_name(&self.state, self.registry, object_id),
                    })
                }
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
                CastCostOptionDef::Mana { label, .. }
                | CastCostOptionDef::Behold { label, .. }
                | CastCostOptionDef::Blight { label, .. } => label.clone(),
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
                AdditionalCost::Blight { count } => {
                    debits.push(self.plan_blight_selection(player, *count, selection)?);
                }
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
                    let mut taps = Vec::new();
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
                        taps.push((oid, generation));
                    }
                    debits.push(CostDebit::TapGroup(taps));
                }
            }
        }
        if selections.len() != costs.len() {
            return Err(EngineError::Illegal("unexpected cost selection"));
        }

        Ok(PreparedPaymentCosts {
            waterbend_limit: None,
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

    #[cfg(test)]
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
        self.prepare_ability_costs(
            player,
            idx,
            permanent_id,
            costs,
            flex_payments,
            selections,
            restricted_mana,
            extra_generic,
            generic_reduction,
        )?
        .finish(&self.state)
    }

    #[allow(clippy::too_many_arguments)]
    pub(in crate::engine) fn prepare_ability_costs(
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
    ) -> Result<PreparedPaymentCosts, EngineError> {
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
        let mut mana_cost = ManaCost::default();
        let mut waterbend_limit = None;
        let mut saw_tap = false;
        let eligible_restricted_mana = self.eligible_restricted_mana_for_ability(idx, permanent_id);
        for (cost_index, cost) in costs.iter().enumerate() {
            match cost {
                AbilityCost::RemoveCounters { counter, count } => {
                    expected_selections += 1;
                    let Some(Selection::CounterRemoval(selected)) =
                        by_index.get(&cost_index).and_then(|s| s.selection.as_ref())
                    else {
                        return Err(EngineError::Illegal("missing counter removal selection"));
                    };
                    let reference = selected
                        .source
                        .as_ref()
                        .ok_or(EngineError::Illegal("missing counter source"))?;
                    if reference.object_id != permanent_id {
                        return Err(EngineError::Illegal("counter cost must use its source"));
                    }
                    let kind = self.state.objects[&permanent_id]
                        .counters
                        .keys()
                        .copied()
                        .find(|kind| {
                            crate::engine::counters::counter_option_id(*kind) == selected.option_id
                        })
                        .filter(|kind| counter.is_none_or(|expected| expected == *kind))
                        .ok_or(EngineError::Illegal("invalid counter kind"))?;
                    debits.push(CostDebit::RemoveCounters {
                        object: *reference,
                        kind,
                        count: *count,
                    });
                }
                AbilityCost::Blight { count } => {
                    expected_selections += 1;
                    let selection = by_index
                        .get(&cost_index)
                        .ok_or(EngineError::Illegal("missing Blight selection"))?;
                    debits.push(self.plan_blight_selection(player, *count, selection)?);
                }
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
                    if (*delta > 0 && !self.can_receive_counters(permanent_id))
                        || (*delta < 0
                            && object.counter_count(CounterKind::Loyalty) < delta.unsigned_abs())
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
                    let mut taps = Vec::new();
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
                        taps.push((oid, generation));
                    }
                    debits.push(CostDebit::TapGroup(taps));
                }
                AbilityCost::Mana(cost) | AbilityCost::Waterbend(cost) => {
                    if matches!(costs[cost_index], AbilityCost::Waterbend(_)) {
                        if waterbend_limit.is_some() {
                            return Err(EngineError::Illegal("multiple Waterbend components"));
                        }
                        waterbend_limit = Some(super::waterbend::generic_component(cost)?);
                        debits.push(CostDebit::Waterbend);
                    } else {
                        if saw_mana {
                            return Err(EngineError::Illegal("multiple mana cost components"));
                        }
                        saw_mana = true;
                    }
                    mana_cost.pips.extend(cost.pips.iter().cloned());
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
                    let mut exiles = Vec::new();
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
                        exiles.push((
                            oid,
                            self.state
                                .zone_change_generation
                                .get(&oid)
                                .copied()
                                .unwrap_or(0),
                            object.owner,
                        ));
                    }
                    debits.push(CostDebit::ExileGroup(exiles));
                }
            }
        }
        if selections.len() != expected_selections {
            return Err(EngineError::Illegal("unexpected cost selection"));
        }
        if !restricted_mana.is_empty()
            && !saw_mana
            && waterbend_limit.is_none()
            && extra_generic == 0
        {
            return Err(EngineError::Illegal(
                "restricted mana supplied for an ability with no mana cost",
            ));
        }

        Ok(PreparedPaymentCosts {
            transaction: CostTransactionPlan {
                purpose: CostPurpose::Ability,
                player,
                player_idx: idx,
                debits,
                cast_cost_receipts: vec![],
            },
            waterbend_limit,
            mana: mana_cost,
            x_value: 0,
            extra_generic,
            generic_reduction,
            flex_payments: flex_payments.to_vec(),
            restricted_mana: restricted_mana.to_vec(),
            eligible_restricted_mana,
        })
    }

    pub(in crate::engine) fn commit_cost_transaction(
        &mut self,
        mut plan: CostTransactionPlan,
    ) -> Result<CostPaymentReceipt, EngineError> {
        self.revalidate_cost_transaction(&plan)?;

        // Counter placement is nonconsuming. Complete it before any selected zone departure,
        // including when the same creature also pays a sacrifice cost (CR 601.2h).
        if plan.debits.iter().any(|debit| {
            matches!(
                debit,
                CostDebit::Blight { .. } | CostDebit::RemoveCounters { .. }
            )
        }) {
            plan.debits.sort_by_key(|debit| {
                matches!(
                    debit,
                    CostDebit::Sacrifice { .. }
                        | CostDebit::Exile { .. }
                        | CostDebit::ExileGroup(_)
                        | CostDebit::Discard { .. }
                )
            });
        }

        let mut payment = CostPaymentReceipt {
            blight_receipts: vec![],
            move_events: vec![],
            trigger_events: vec![],
            sacrificed: vec![],
            paid_card_costs: vec![],
            life_paid: 0,
            mana_spent: 0,
            expend_triggers: vec![],
            restricted_mana_spent: vec![],
            cast_cost_receipts: plan.cast_cost_receipts,
        };
        for debit in plan.debits {
            let zones = matches!(
                &debit,
                CostDebit::Exile { .. } | CostDebit::ExileGroup(_) | CostDebit::Sacrifice { .. }
            )
            .then(|| self.snapshot_zone_event());
            match debit {
                CostDebit::Waterbend => payment.trigger_events.push(GameEvent::Waterbent {
                    player: plan.player,
                }),
                CostDebit::RemoveCounters {
                    object,
                    kind,
                    count,
                } => {
                    self.remove_counters(object.object_id, kind, count);
                }
                CostDebit::Blight { object, count } => {
                    let receipt = self.complete_blight(plan.player, count, Some(object.object_id));
                    payment.trigger_events.push(GameEvent::Blighted(receipt));
                    payment.blight_receipts.push(receipt);
                }
                CostDebit::Loyalty {
                    object_id, delta, ..
                } => {
                    if delta > 0 {
                        self.place_counters(object_id, CounterKind::Loyalty, delta as u32);
                        continue;
                    }
                    let object = self
                        .state
                        .objects
                        .get_mut(&object_id)
                        .expect("prevalidated loyalty source must commit");
                    if delta < 0 {
                        let current = object.counter_count(CounterKind::Loyalty);
                        object.set_counter(
                            CounterKind::Loyalty,
                            current.saturating_sub(delta.unsigned_abs()),
                        );
                    }
                }
                CostDebit::Tap { object_id, .. } => {
                    payment
                        .trigger_events
                        .extend(self.tap_permanents(plan.player, &[object_id]));
                }
                CostDebit::TapGroup(objects) => {
                    let ids = objects.into_iter().map(|(oid, _)| oid).collect::<Vec<_>>();
                    payment
                        .trigger_events
                        .extend(self.tap_permanents(plan.player, &ids));
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
                    object_id, owner, ..
                } => self.commit_exile_cost(&mut payment, object_id, owner),
                CostDebit::ExileGroup(objects) => {
                    for (oid, _, owner) in objects {
                        self.commit_exile_cost(&mut payment, oid, owner);
                    }
                }
                CostDebit::Sacrifice { snapshot, owner } => {
                    let oid = snapshot.source.object_id;
                    let mut snapshot = self
                        .sacrifice_snapshot(oid)
                        .expect("prevalidated sacrifice source");
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
            if let Some(zones) = zones {
                payment.trigger_events.push(self.finish_zone_event(zones));
            }
        }
        Ok(payment)
    }

    fn commit_exile_cost(
        &mut self,
        payment: &mut CostPaymentReceipt,
        oid: ObjectId,
        owner: PlayerId,
    ) {
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
        payment.paid_card_costs.push(PaidCardCost::Exile {
            object_id: oid,
            card_name,
            result: card_result_entry(
                &self.state,
                self.registry,
                CardResultAction::Exile,
                owner,
                oid,
            ),
        });
    }

    fn exile_cost_object_current(
        &self,
        player_idx: usize,
        oid: ObjectId,
        generation: u64,
        owner: PlayerId,
    ) -> bool {
        self.state.objects.get(&oid).is_some_and(|object| {
            object.zone == Zone::Graveyard
                && object.owner == owner
                && self.state.player_idx(owner).is_some()
                && self.state.players[player_idx].graveyard.contains(&oid)
        }) && self
            .state
            .zone_change_generation
            .get(&oid)
            .copied()
            .unwrap_or(0)
            == generation
    }

    fn revalidate_cost_transaction(&self, plan: &CostTransactionPlan) -> Result<(), EngineError> {
        if self.state.player_idx(plan.player) != Some(plan.player_idx) {
            return Err(EngineError::Illegal("cost transaction player changed"));
        }
        let mut counter_debits: BTreeMap<(ObjectId, CounterKind), u64> = BTreeMap::new();
        for debit in &plan.debits {
            let valid = match debit {
                CostDebit::Waterbend => true,
                CostDebit::RemoveCounters {
                    object,
                    kind,
                    count,
                } => {
                    *counter_debits.entry((object.object_id, *kind)).or_default() +=
                        u64::from(*count);
                    self.state
                        .objects
                        .get(&object.object_id)
                        .is_some_and(|o| o.zone == Zone::Battlefield && o.controller == plan.player)
                        && self
                            .state
                            .zone_change_generation
                            .get(&object.object_id)
                            .copied()
                            .unwrap_or(0)
                            == object.zone_change_generation
                }
                CostDebit::Blight { object, count } => {
                    self.validate_blight(plan.player, *count, object).is_ok()
                }
                CostDebit::Loyalty {
                    object_id,
                    generation,
                    delta,
                } => {
                    if *delta < 0 {
                        *counter_debits
                            .entry((*object_id, CounterKind::Loyalty))
                            .or_default() += u64::from(delta.unsigned_abs());
                    }
                    (*delta <= 0 || self.can_receive_counters(*object_id))
                        && self.state.objects.get(object_id).is_some_and(|object| {
                            object.zone == Zone::Battlefield
                                && object.controller == plan.player
                                && (*delta >= 0
                                    || object.counter_count(CounterKind::Loyalty)
                                        >= delta.unsigned_abs())
                        })
                        && self
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
                    self.state
                        .objects
                        .get(object_id)
                        .is_some_and(|object| object.zone == Zone::Battlefield && !object.tapped)
                        && self
                            .state
                            .zone_change_generation
                            .get(object_id)
                            .copied()
                            .unwrap_or(0)
                            == *generation
                }
                CostDebit::TapGroup(objects) => objects.iter().all(|(oid, generation)| {
                    self.state.objects.get(oid).is_some_and(|object| {
                        object.zone == Zone::Battlefield
                            && !object.tapped
                            && object.controller == plan.player
                    }) && self
                        .state
                        .zone_change_generation
                        .get(oid)
                        .copied()
                        .unwrap_or(0)
                        == *generation
                }),
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
                    self.exile_cost_object_current(plan.player_idx, *object_id, *generation, *owner)
                }
                CostDebit::ExileGroup(objects) => objects.iter().all(|(oid, generation, owner)| {
                    self.exile_cost_object_current(plan.player_idx, *oid, *generation, *owner)
                }),
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
        for ((oid, kind), count) in counter_debits {
            if self
                .state
                .objects
                .get(&oid)
                .is_none_or(|o| u64::from(o.counter_count(kind)) < count)
            {
                return Err(EngineError::Illegal("not enough counters to pay all costs"));
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
        if source.is_some_and(|source| {
            super::super::targeting::object_is_excluded(
                &self.state,
                &filter.excluded_objects,
                oid,
                super::super::targeting::TargetSourceIdentity::current(self, source),
                TriggerContext::default(),
            )
        }) {
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
    fn waterbend_cap_modifiers_double_taps_and_completion_use_one_transaction() {
        for (cost, increase, reduction, taps, mana) in [
            (2, 2, 0, 2, 2),
            (2, 0, 1, 1, 0),
            (2, 0, 0, 0, 2),
            (0, 0, 0, 0, 0),
        ] {
            let mut engine = GameEngine::new(146020, &[0, 1], 20, None, true).unwrap();
            let objects = engine.state.players[0].hand[..3].to_vec();
            for &oid in &objects {
                engine.state.objects.get_mut(&oid).unwrap().card_id = "ornithopter".into();
                move_object_to_zone(
                    &mut engine.state,
                    engine.registry,
                    oid,
                    Zone::Battlefield,
                    Some(0),
                )
                .unwrap();
                engine.state.objects.get_mut(&oid).unwrap().summoning_sick = true;
            }
            let source = objects[0];
            engine.state.players[0].mana_pool.colorless = 4;
            let costs = [AbilityCost::Waterbend(
                ManaCost::parse(&format!("{{{cost}}}")).unwrap(),
            )];
            let prepare = |engine: &GameEngine| {
                engine
                    .prepare_ability_costs(0, 0, source, &costs, &[], &[], &[], increase, reduction)
                    .unwrap()
            };
            let prepared = prepare(&engine);
            let selection = rv1::PaymentSelection {
                expected_state_revision: engine.state.command_index,
                source: Some(engine.payment_object_ref(source)),
                waterbend: objects[..taps]
                    .iter()
                    .map(|oid| engine.payment_object_ref(*oid))
                    .collect(),
                mana: Some(rv1::PaymentMana {
                    c: mana,
                    ..Default::default()
                }),
                ..Default::default()
            };
            if cost == 2 && increase == 2 {
                let mut excessive = selection.clone();
                excessive
                    .waterbend
                    .push(engine.payment_object_ref(objects[2]));
                excessive.mana.as_mut().unwrap().c = 1;
                assert!(
                    engine
                        .validate_explicit_payment(0, source, false, &prepared, &excessive)
                        .is_err(),
                    "cost increases do not enlarge the Waterbend component"
                );
                let tapping = [AbilityCost::Tap, costs[0].clone()];
                engine
                    .state
                    .objects
                    .get_mut(&source)
                    .unwrap()
                    .summoning_sick = false;
                let tap_costs = engine
                    .prepare_ability_costs(
                        0,
                        0,
                        source,
                        &tapping,
                        &[],
                        &[],
                        &[],
                        increase,
                        reduction,
                    )
                    .unwrap();
                assert!(
                    !engine.waterbend_candidate(0, &tap_costs, &engine.payment_object_ref(source)),
                    "a source cannot pay two tap costs"
                );
            }
            let life = engine
                .validate_explicit_payment(0, source, false, &prepared, &selection)
                .unwrap();
            let plan = prepared
                .finish_explicit(&engine.state, &selection, life)
                .unwrap();
            let receipt = engine.commit_cost_transaction(plan).unwrap();
            assert_eq!(
                receipt
                    .trigger_events
                    .iter()
                    .filter(|event| matches!(event, GameEvent::Waterbent { player: 0 }))
                    .count(),
                1,
                "including all-mana and zero-cost payments"
            );
            assert_eq!(
                receipt
                    .trigger_events
                    .iter()
                    .filter(|event| matches!(event, GameEvent::BecameTapped { .. }))
                    .count(),
                taps
            );
            assert_eq!(engine.state.players[0].mana_pool.colorless, 4 - mana);
        }
    }

    #[test]
    fn issue_157_counter_debits_aggregate_before_any_payment_and_precede_sacrifice() {
        let mut engine = GameEngine::new(15707, &[0, 1], 20, None, true).unwrap();
        let oid = engine.state.players[0].hand[0];
        engine.state.objects.get_mut(&oid).unwrap().card_id = "dockworker_drone".into();
        move_object_to_zone(
            &mut engine.state,
            engine.registry,
            oid,
            Zone::Battlefield,
            None,
        )
        .unwrap();
        engine
            .state
            .objects
            .get_mut(&oid)
            .unwrap()
            .add_counters(CounterKind::PlusOnePlusOne, 1, 0);
        let generation = engine.state.zone_change_generation[&oid];
        let debit = || CostDebit::RemoveCounters {
            object: rv1::CostObjectRef {
                object_id: oid,
                zone_change_generation: generation,
            },
            kind: CounterKind::PlusOnePlusOne,
            count: 1,
        };
        let plan = |debits| CostTransactionPlan {
            purpose: CostPurpose::Ability,
            player: 0,
            player_idx: 0,
            cast_cost_receipts: vec![],
            debits,
        };
        assert!(engine
            .commit_cost_transaction(plan(vec![debit(), debit()]))
            .is_err());
        assert_eq!(
            engine.state.objects[&oid].counter_count(CounterKind::PlusOnePlusOne),
            1
        );
        assert!(!engine.counter_costs_payable(
            oid,
            &[
                AbilityCost::RemoveCounters {
                    counter: Some(CounterKind::PlusOnePlusOne),
                    count: 1
                },
                AbilityCost::RemoveCounters {
                    counter: None,
                    count: 1
                },
            ]
        ));
        let snapshot = engine.sacrifice_snapshot(oid).unwrap();
        let receipt = engine
            .commit_cost_transaction(plan(vec![
                CostDebit::Sacrifice { snapshot, owner: 0 },
                debit(),
            ]))
            .unwrap();
        assert!(receipt.sacrificed[0].source.counters.is_empty());
        assert!(engine.state.last_known_counters_by_generation[&(oid, generation)].is_empty());
        assert_eq!(engine.state.objects[&oid].zone, Zone::Graveyard);
    }

    #[test]
    fn issue_153_blight_then_sacrifice_preserves_post_counter_lki_and_one_event() {
        let mut engine = GameEngine::new(153020, &[0, 1], 20, None, true).unwrap();
        let oid = engine.state.players[0].hand[0];
        engine.state.objects.get_mut(&oid).unwrap().card_id = "grizzly_bears".into();
        move_object_to_zone(
            &mut engine.state,
            engine.registry,
            oid,
            Zone::Battlefield,
            None,
        )
        .unwrap();
        let generation = engine.state.zone_change_generation[&oid];
        let snapshot = engine.sacrifice_snapshot(oid).unwrap();
        // Authored order is deliberately reversed: all counters must be placed before departure.
        let plan = CostTransactionPlan {
            purpose: CostPurpose::Ability,
            player: 0,
            player_idx: 0,
            cast_cost_receipts: vec![],
            debits: vec![
                CostDebit::Sacrifice { snapshot, owner: 0 },
                CostDebit::Blight {
                    object: rv1::CostObjectRef {
                        object_id: oid,
                        zone_change_generation: generation,
                    },
                    count: 2,
                },
                CostDebit::Tap {
                    object_id: oid,
                    generation,
                },
            ],
        };
        let receipt = engine.commit_cost_transaction(plan).unwrap();
        assert_eq!(receipt.blight_receipts.len(), 1);
        assert_eq!(
            receipt
                .trigger_events
                .iter()
                .filter(|event| matches!(event, GameEvent::Blighted(_)))
                .count(),
            1
        );
        assert_eq!(
            receipt.sacrificed[0].source.power_toughness,
            (Some(0), Some(0))
        );
        assert_eq!(
            receipt.blight_receipts[0]
                .creature
                .unwrap()
                .zone_change_generation,
            generation
        );
        assert_eq!(engine.state.objects[&oid].zone, Zone::Graveyard);
        assert!(engine.state.zone_change_generation[&oid] > generation);
    }

    #[test]
    fn issue_168_one_exile_cost_keeps_its_simultaneous_group() {
        let mut engine = GameEngine::new(168020, &[0, 1], 20, None, true).unwrap();
        let ids = engine.state.players[0].hand[..3].to_vec();
        for (i, oid) in ids.iter().enumerate() {
            engine.state.objects.get_mut(oid).unwrap().card_id = "grizzly_bears".into();
            move_object_to_zone(
                &mut engine.state,
                engine.registry,
                *oid,
                if i == 0 {
                    Zone::Battlefield
                } else {
                    Zone::Graveyard
                },
                None,
            )
            .unwrap();
        }
        let costs = [AbilityCost::ExileGraveyardCards {
            count: 2,
            filter: Default::default(),
            exclude_source: true,
        }];
        let selections = [rv1::CostSelection {
            cost_index: 0,
            selection: Some(rv1::cost_selection::Selection::GraveyardObjectIds(
                rv1::GraveyardObjectIds {
                    object_ids: ids[1..].to_vec(),
                },
            )),
        }];
        let plan = engine
            .plan_ability_costs(0, 0, ids[0], &costs, &[], &selections, &[], 0, 0)
            .unwrap();
        let receipt = engine.commit_cost_transaction(plan).unwrap();
        let batches: Vec<_> = receipt
            .trigger_events
            .iter()
            .filter_map(|event| match event {
                GameEvent::ZoneChanges(batch) if !batch.moves.is_empty() => Some(batch),
                _ => None,
            })
            .collect();
        assert_eq!(
            batches.len(),
            1,
            "one instruction must not become per-card events"
        );
        assert_eq!(batches[0].moves.len(), 2);
    }

    #[test]
    fn issue_169_tap_components_keep_action_boundaries_and_stale_groups_are_atomic() {
        let mut engine = GameEngine::new(169030, &[7, 19], 20, None, true).unwrap();
        let objects = engine.state.players[1].hand[..3].to_vec();
        for oid in &objects {
            engine.state.objects.get_mut(oid).unwrap().card_id = "grizzly_bears".into();
            super::super::super::resolution::move_object_to_zone(
                &mut engine.state,
                engine.registry,
                *oid,
                Zone::Battlefield,
                None,
            )
            .unwrap();
        }
        let references = objects
            .iter()
            .map(|oid| (*oid, engine.state.zone_change_generation[oid]))
            .collect::<Vec<_>>();
        let plan = |stale| CostTransactionPlan {
            purpose: CostPurpose::Ability,
            player: 19,
            player_idx: 1,
            cast_cost_receipts: vec![],
            debits: vec![
                CostDebit::Tap {
                    object_id: objects[0],
                    generation: references[0].1,
                },
                CostDebit::TapGroup(vec![
                    references[1],
                    (objects[2], references[2].1 + u64::from(stale)),
                ]),
            ],
        };
        let before = format!("{:?}", engine.state);
        assert!(engine.commit_cost_transaction(plan(true)).is_err());
        assert_eq!(format!("{:?}", engine.state), before);
        let receipt = engine.commit_cost_transaction(plan(false)).unwrap();
        let actions = receipt
            .trigger_events
            .iter()
            .map(|event| {
                let GameEvent::BecameTapped { action, .. } = event else {
                    panic!("tap receipt")
                };
                assert_eq!(action.actor, 19);
                action.id
            })
            .collect::<Vec<_>>();
        assert_eq!(actions.len(), 3);
        assert_ne!(
            actions[0], actions[1],
            "source tap is a separate cost component"
        );
        assert_eq!(
            actions[1], actions[2],
            "one selected tap cost is simultaneous"
        );
        assert_eq!(engine.state.next_tap_action_id, 2);
    }

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
    fn issue_170_life_payment_history_is_committed_atomically() {
        for purpose in [CostPurpose::Spell, CostPurpose::Ability] {
            let mut engine = GameEngine::new(170002, &[0, 1], 20, None, true).unwrap();
            let mana = super::super::mana::plan_mana_payment_with_reduction(
                &engine.state,
                0,
                &ManaCost::parse("{G/P}").unwrap(),
                0,
                0,
                0,
                &[rv1::FlexPipPayment {
                    pip_index: 0,
                    pay_life: true,
                }],
            )
            .unwrap();
            let plan = CostTransactionPlan {
                purpose,
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
            assert!(engine.commit_cost_transaction(plan).is_err());
            assert_eq!(format!("{:?}", engine.state), before);
            let receipt = engine
                .commit_cost_transaction(CostTransactionPlan {
                    purpose,
                    player: 0,
                    player_idx: 0,
                    debits: vec![CostDebit::Mana(mana)],
                    cast_cost_receipts: vec![],
                })
                .unwrap();
            assert_eq!(receipt.life_paid, 2);
            assert_eq!(engine.state.players[0].life, 18);
            assert_eq!(engine.state.turn_history.current.player(0).life_lost, 2);
            assert_eq!(engine.state.turn_history.current.player(0).life_gained, 0);
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
        let payment = rv1::PaymentSelection {
            waterbend: vec![],
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
            convoke: vec![rv1::ObjectPaymentContribution {
                object: Some(reference),
                kind: rv1::ObjectPaymentKind::Generic as i32,
            }],
            mana: None,
        };
        let life = engine
            .validate_explicit_payment(0, source, true, &prepared, &payment)
            .unwrap();
        let plan = prepared
            .finish_explicit(&engine.state, &payment, life)
            .unwrap();
        // The debit order carries the rule, and revalidation happens before either mutation.
        assert!(
            matches!(&plan.debits[1], CostDebit::TapGroup(objects) if objects.len() == 1 && objects[0].0 == bear)
        );
        assert!(matches!(plan.debits[2], CostDebit::Sacrifice { .. }));
        let probe = PreparedPaymentCosts {
            waterbend_limit: None,
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
        assert_eq!(
            committed
                .trigger_events
                .iter()
                .filter(|e| matches!(e, GameEvent::BecameTapped { .. }))
                .count(),
            1
        );
        assert_eq!(committed.sacrificed.len(), 1);
        assert_eq!(engine.state.objects[&bear].zone, Zone::Graveyard);
    }
}
