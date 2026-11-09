//! Read-only payment witnesses for action highlighting. The normal transaction validator
//! checks every proposed debit; a witness is never permission to commit a later command.
use super::super::*;
use super::transaction::PreparedPaymentCosts;

pub(in crate::engine) struct PaymentWitness {
    pub selection: rv1::PaymentSelection,
    pub restricted_mana: Vec<rv1::ManaSpendSelection>,
}

impl GameEngine {
    /// Optimistic lower bound for a normal printed spell cost, before enumerating targets.
    /// All generic cost is removed and every Phyrexian pip may use life. A false result is
    /// therefore impossible even with target-based generic reductions. Complex mana engines
    /// deliberately bypass this optimization and keep the complete witness search.
    pub(in crate::engine) fn minimum_printed_mana_possible(
        &self,
        player: PlayerId,
        mana: &ManaCost,
        inventory: &super::candidates::AbilityInventory,
    ) -> bool {
        let Some(idx) = self.state.player_idx(player) else {
            return false;
        };
        let pool = &self.state.players[idx].mana_pool;
        let mut available = [
            pool.white,
            pool.blue,
            pool.black,
            pool.red,
            pool.green,
            pool.colorless,
        ]
        .map(u64::from);
        for group in &self.state.players[idx].restricted_mana {
            for (slot, amount) in [
                group.amount.w,
                group.amount.u,
                group.amount.b,
                group.amount.r,
                group.amount.g,
                group.amount.c,
            ]
            .into_iter()
            .enumerate()
            {
                available[slot] += u64::from(amount);
            }
        }
        let mana_abilities: Vec<_> = inventory
            .iter()
            .flat_map(|(_, abilities)| abilities)
            .filter(|ability| ability.definition.is_mana_ability())
            .collect();
        let no_sources = mana_abilities.is_empty();
        if !no_sources {
            if !self.state.continuous_effects.is_empty()
                || !self.state.static_emblems.is_empty()
                || self
                    .state
                    .players
                    .iter()
                    .flat_map(|p| &p.battlefield)
                    .any(|&oid| {
                        self.effective_face(oid).is_none_or(|face| {
                            !face.static_abilities.is_empty()
                                || !face.triggered_abilities.is_empty()
                        })
                    })
            {
                return true;
            }
            for ability in mana_abilities {
                let ability = &ability.definition;
                if !ability.intrinsic_land_mana || ability.costs != [AbilityCost::Tap] {
                    return true;
                }
                let Some(options) = ability.mana_options() else {
                    return true;
                };
                for option in options {
                    for (slot, amount) in
                        [option.w, option.u, option.b, option.r, option.g, option.c]
                            .into_iter()
                            .enumerate()
                    {
                        if amount != 0 {
                            available[slot] = u64::MAX / 16;
                        }
                    }
                }
            }
        }
        let flex = mana
            .pips
            .iter()
            .enumerate()
            .filter_map(|(index, pip)| {
                matches!(
                    pip,
                    ManaSymbol::Phyrexian(_) | ManaSymbol::HybridPhyrexian(_, _)
                )
                .then_some(rv1::FlexPipPayment {
                    pip_index: index as u32,
                    pay_life: true,
                    mana_color: 0,
                })
            })
            .collect::<Vec<_>>();
        let Ok(demands) = super::demand::normalize(mana, 0, 0, u32::MAX, &flex) else {
            return true;
        };
        if no_sources || self.may_spend_mana_as_any_color(player) {
            // With no sources, ignoring colors also covers any live as-though permission.
            let total = available.iter().sum::<u64>();
            return demands
                .iter()
                .any(|demand| demand.amounts.iter().map(|&n| u64::from(n)).sum::<u64>() <= total);
        }
        demands
            .iter()
            .any(|demand| (0..6).all(|slot| u64::from(demand.amounts[slot]) <= available[slot]))
    }

    pub(in crate::engine) fn payment_with_available_sources(
        &self,
        cost: &PreparedPaymentCosts,
        source: rv1::CostObjectRef,
        convoke: bool,
        eligible: &dyn Fn(&GameEngine) -> Vec<u32>,
        remaining: &mut usize,
        initial_sources: Option<&super::candidates::AbilityInventory>,
    ) -> bool {
        self.search_available_mana(
            cost,
            source,
            convoke,
            eligible,
            remaining,
            0,
            initial_sources,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn search_available_mana(
        &self,
        cost: &PreparedPaymentCosts,
        source: rv1::CostObjectRef,
        convoke: bool,
        eligible: &dyn Fn(&GameEngine) -> Vec<u32>,
        remaining: &mut usize,
        depth: usize,
        initial_sources: Option<&super::candidates::AbilityInventory>,
    ) -> bool {
        if *remaining == 0 || depth == 64 {
            return false;
        }
        *remaining -= 1;
        let mut cost = cost.clone();
        cost.eligible_restricted_mana = eligible(self);
        if self
            .payment_from_current_pool(&cost, source, convoke)
            .is_some()
        {
            return true;
        }
        let player = cost.transaction.player();
        let Some(idx) = self.state.player_idx(player) else {
            return false;
        };

        let recomputed;
        let sources = if let Some(sources) = initial_sources {
            sources
        } else {
            recomputed = self.state.players[idx]
                .battlefield
                .iter()
                .map(|&oid| (oid, self.effective_activated_abilities(oid)))
                .collect::<Vec<_>>();
            &recomputed
        };
        for (oid, abilities) in sources {
            let oid = *oid;
            for effective in abilities {
                let ability = &effective.definition;

                if !ability.is_mana_ability() {
                    continue;
                }
                let mut outputs = Vec::new();
                if ability.storage_counter_split_mana().is_some() {
                    let count = self.state.objects[&oid].counter_count(CounterKind::Storage);
                    // Every split is a real engine command. The shared budget bounds large
                    // storage counters; exhaustion cannot manufacture an affordable result.
                    'splits: for x in (1..=count.min(*remaining as u32)).rev() {
                        for split in 0..=x {
                            outputs.push((0, x, split));
                            if outputs.len() == super::candidates::PROPOSAL_LIMIT {
                                break 'splits;
                            }
                        }
                    }
                } else if let Some(options) = self.active_mana_options(oid, ability) {
                    outputs.extend(
                        options
                            .iter()
                            .enumerate()
                            .filter(|(_, amount)| {
                                [amount.w, amount.u, amount.b, amount.r, amount.g, amount.c]
                                    .iter()
                                    .any(|&n| n > 0)
                            })
                            .map(|(n, _)| (n as u32, 0, 0)),
                    );
                }
                let choices = super::super::legal_actions::legal_ability_cost_choices(
                    self,
                    player,
                    oid,
                    effective.slot as usize,
                    ability,
                );
                for selections in super::candidates::cost_selections(&choices) {
                    for &(option, x, split) in &outputs {
                        if *remaining == 0 {
                            return false;
                        }
                        let Ok(reduction) = self.activated_mana_reduction(player, oid, ability)
                        else {
                            continue;
                        };
                        let Ok(prepared) = self.prepare_ability_costs(
                            player,
                            idx,
                            oid,
                            &ability.costs,
                            &[],
                            &selections,
                            &[],
                            x,
                            0,
                            reduction,
                        ) else {
                            continue;
                        };
                        for flex in super::candidates::phyrexian_choices(&prepared.mana) {
                            let mut prepared = prepared.clone();
                            prepared.flex_payments = flex.clone();
                            let Some(witness) = self.payment_from_current_pool(
                                &prepared,
                                self.payment_object_ref(oid),
                                false,
                            ) else {
                                continue;
                            };
                            let mut next = self.clone();
                            let command = rv1::ActivateAbility {
                                source_object_id: oid,
                                expected_zone_change_generation: self
                                    .payment_object_ref(oid)
                                    .zone_change_generation,
                                ability_index: effective.slot,
                                mana_option_index: option,
                                x_value: x,
                                mana_split_first_color_count: split,
                                payment: Some(witness.selection),
                                restricted_mana: witness.restricted_mana,
                                cost_selections: selections.clone(),
                                flex_payments: flex,
                                ..Default::default()
                            };
                            if next.activate_ability(player, &command).is_err() {
                                continue;
                            }
                            next.reconcile_activated_ability_slots();
                            if next.search_available_mana(
                                &cost,
                                source,
                                convoke,
                                eligible,
                                remaining,
                                depth + 1,
                                None,
                            ) {
                                return true;
                            }
                        }
                    }
                }
            }
        }
        false
    }

    pub(in crate::engine) fn payment_from_current_pool(
        &self,
        cost: &PreparedPaymentCosts,
        source: rv1::CostObjectRef,
        convoke: bool,
    ) -> Option<PaymentWitness> {
        let player = cost.transaction.player();
        let idx = self.state.player_idx(player)?;
        let pool = &self.state.players[idx].mana_pool;
        let mut available = [
            pool.white,
            pool.blue,
            pool.black,
            pool.red,
            pool.green,
            pool.colorless,
        ];
        let mut restricted = BTreeMap::<u32, [u32; 6]>::new();
        for group in &self.state.players[idx].restricted_mana {
            if !cost
                .eligible_restricted_mana
                .contains(&group.restriction_group_id)
            {
                continue;
            }
            let amount = &group.amount;
            let entry = restricted.entry(group.restriction_group_id).or_default();
            for (slot, n) in [amount.w, amount.u, amount.b, amount.r, amount.g, amount.c]
                .into_iter()
                .enumerate()
            {
                available[slot] = available[slot].checked_add(n)?;
                entry[slot] = entry[slot].checked_add(n)?;
            }
        }
        let demands = super::demand::normalize(
            &cost.mana,
            cost.x_value,
            cost.extra_generic,
            cost.generic_reduction,
            &cost.flex_payments,
        )
        .ok()?;
        // A color permission cannot increase the quantity of available mana. Reject an
        // impossible pool before querying global characteristics or validating transactions.
        // Object contributions are the exception and retain their normal search below.
        let total_available = available
            .iter()
            .map(|&amount| u64::from(amount))
            .sum::<u64>();
        if !convoke
            && cost.waterbend_limit.is_none()
            && demands.iter().all(|demand| {
                demand
                    .amounts
                    .iter()
                    .map(|&amount| u64::from(amount))
                    .sum::<u64>()
                    > total_available
            })
        {
            return None;
        }
        let as_any_color = self.may_spend_mana_as_any_color(player);
        let mut candidates = Vec::new();
        if convoke || cost.waterbend_limit.is_some() {
            for &oid in &self.state.players[idx].battlefield {
                let reference = self.payment_object_ref(oid);
                let mut options = Vec::new();
                if cost.waterbend_limit.is_some() {
                    if self.waterbend_candidate(player, cost, &reference) {
                        options.push(6);
                    }
                } else {
                    for slot in [0, 1, 2, 3, 4, 6] {
                        let contribution = rv1::ObjectPaymentContribution {
                            object: Some(reference),
                            kind: if slot == 6 { 6 } else { slot as i32 + 1 },
                        };
                        if self.convoke_candidate(player, cost, &contribution) {
                            options.push(slot);
                        }
                    }
                }
                if !options.is_empty() {
                    candidates.push((reference, options));
                }
            }
        }
        let context = PoolPaymentSearch {
            engine: self,
            cost,
            source,
            convoke,
            available,
            restricted,
            as_any_color,
            candidates,
        };
        for demand in demands {
            let mut failed = HashSet::new();
            let mut selection = rv1::PaymentSelection {
                expected_state_revision: self.state.command_index,
                source: Some(source),
                ..Default::default()
            };
            if let Some(witness) = context.search(0, &demand, &mut selection, &mut failed) {
                return Some(witness);
            }
        }
        None
    }
}

struct PoolPaymentSearch<'a> {
    engine: &'a GameEngine,
    cost: &'a PreparedPaymentCosts,
    source: rv1::CostObjectRef,
    convoke: bool,
    available: [u32; 6],
    restricted: BTreeMap<u32, [u32; 6]>,
    as_any_color: bool,
    candidates: Vec<(rv1::CostObjectRef, Vec<usize>)>,
}

impl PoolPaymentSearch<'_> {
    fn witness(
        &self,
        demand: &super::demand::Demand,
        objects: &rv1::PaymentSelection,
    ) -> Option<PaymentWitness> {
        let mut available = self.available;
        let mut spent = [0; 6];
        if self.as_any_color {
            spent[5] = demand.amounts[5];
            available[5] = available[5].checked_sub(spent[5])?;
            let mut remaining = demand.amounts[..5]
                .iter()
                .map(|&n| u64::from(n))
                .sum::<u64>()
                + u64::from(demand.amounts[6]);
            for slot in [5, 0, 1, 2, 3, 4] {
                let take = u64::from(available[slot]).min(remaining) as u32;
                spent[slot] += take;
                remaining -= u64::from(take);
            }
            if remaining != 0 {
                return None;
            }
        } else {
            for slot in 0..6 {
                spent[slot] = demand.amounts[slot];
                available[slot] = available[slot].checked_sub(spent[slot])?;
            }
            let mut generic = demand.amounts[6];
            for slot in [5, 0, 1, 2, 3, 4] {
                let take = available[slot].min(generic);
                spent[slot] += take;
                generic -= take;
            }
            if generic != 0 {
                return None;
            }
        }
        let mut prepared = self.cost.clone();
        prepared.restricted_mana.clear();
        for (&group, available) in &self.restricted {
            let mut used = [0; 6];
            for slot in 0..6 {
                used[slot] = spent[slot].min(available[slot]);
                spent[slot] -= used[slot];
            }
            if used.iter().any(|&n| n != 0) {
                prepared.restricted_mana.push(rv1::ManaSpendSelection {
                    restriction_group_id: group,
                    w: used[0],
                    u: used[1],
                    b: used[2],
                    r: used[3],
                    g: used[4],
                    c: used[5],
                });
            }
        }
        let mut selection = objects.clone();
        selection.mana = Some(rv1::PaymentMana {
            w: spent[0],
            u: spent[1],
            b: spent[2],
            r: spent[3],
            g: spent[4],
            c: spent[5],
        });
        let life = self
            .engine
            .validate_explicit_payment_at_receipt(
                prepared.transaction.player(),
                self.source,
                self.convoke,
                &prepared,
                &selection,
            )
            .ok()?;
        let plan = prepared
            .clone()
            .finish_explicit_with_any_color(&self.engine.state, &selection, life, self.as_any_color)
            .ok()?;
        self.engine.prepare_cost_transaction_commit(plan).ok()?;
        Some(PaymentWitness {
            selection,
            restricted_mana: prepared.restricted_mana,
        })
    }

    fn search(
        &self,
        start: usize,
        demand: &super::demand::Demand,
        selection: &mut rv1::PaymentSelection,
        failed: &mut HashSet<(usize, [u32; 7])>,
    ) -> Option<PaymentWitness> {
        if let Some(witness) = self.witness(demand, selection) {
            return Some(witness);
        }
        if !failed.insert((start, demand.amounts)) {
            return None;
        }
        if self
            .cost
            .waterbend_limit
            .is_some_and(|limit| selection.waterbend.len() >= limit as usize)
        {
            return None;
        }
        for index in start..self.candidates.len() {
            let (reference, options) = &self.candidates[index];
            for &slot in options {
                if demand.amounts[slot] == 0 {
                    continue;
                }
                let mut remaining = demand.clone();
                remaining.amounts[slot] -= 1;
                if self.cost.waterbend_limit.is_some() {
                    selection.waterbend.push(*reference);
                } else {
                    selection.convoke.push(rv1::ObjectPaymentContribution {
                        object: Some(*reference),
                        kind: if slot == 6 { 6 } else { slot as i32 + 1 },
                    });
                }
                if let Some(witness) = self.search(index + 1, &remaining, selection, failed) {
                    return Some(witness);
                }
                if self.cost.waterbend_limit.is_some() {
                    selection.waterbend.pop();
                } else {
                    selection.convoke.pop();
                }
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn engine() -> GameEngine {
        GameEngine::new(
            tricerules_cards::registry::global(),
            13_110_601,
            &[0, 1],
            20,
            None,
            true,
        )
        .unwrap()
    }

    fn cost(e: &GameEngine, mana: &str) -> PreparedPaymentCosts {
        e.prepare_resolution_payment_costs(
            0,
            &PendingManaPayment::from_cost(0, ManaCost::parse(mana).unwrap(), 0),
            &[],
            &[],
        )
        .unwrap()
    }

    fn put(e: &mut GameEngine, name: &str) -> ObjectId {
        e.apply_dev_command(
            0,
            &rv1::DevCommand {
                target_player_id: 0,
                dev: Some(rv1::dev_command::Dev::PutCardInZone(
                    rv1::DevPutCardInZone {
                        card_name: name.into(),
                        zone: rv1::DevZone::Battlefield as i32,
                        ready: true,
                    },
                )),
            },
        )
        .unwrap();
        e.reconcile_activated_ability_slots();
        e.state.next_object_id - 1
    }

    #[test]
    fn printed_mana_bound_preserves_phyrexian_and_as_though_payment_paths() {
        let mut e = engine();
        let forest = put(&mut e, "Forest");
        let inventory = vec![(forest, e.effective_activated_abilities(forest))];
        let before = serde_json::to_vec(&e.state).unwrap();
        assert!(!e.minimum_printed_mana_possible(0, &ManaCost::parse("{W}").unwrap(), &inventory));
        // A lower bound deliberately permits generic reductions and Phyrexian life payment;
        // its positive result still requires the ordinary complete payment witness.
        assert!(e.minimum_printed_mana_possible(
            0,
            &ManaCost::parse("{100}{G}").unwrap(),
            &inventory
        ));
        assert!(e.minimum_printed_mana_possible(0, &ManaCost::parse("{W/P}").unwrap(), &inventory));
        assert_eq!(serde_json::to_vec(&e.state).unwrap(), before);
        put(&mut e, "Mycosynth Lattice");
        assert!(e.minimum_printed_mana_possible(0, &ManaCost::parse("{W}").unwrap(), &inventory));
    }

    #[test]
    fn pool_witness_is_read_only_and_preserves_colored_and_true_colorless_requirements() {
        let mut e = engine();
        e.state.players[0].mana_pool.white = 1;
        e.state.players[0].mana_pool.blue = 1;
        e.state.players[0].mana_pool.colorless = 2;
        let before = serde_json::to_vec(&e.state).unwrap();
        assert!(e
            .payment_from_current_pool(&cost(&e, "{2}{W}{U}"), Default::default(), false)
            .is_some());
        assert!(e
            .payment_from_current_pool(&cost(&e, "{W}{W}"), Default::default(), false)
            .is_none());
        assert!(e
            .payment_from_current_pool(&cost(&e, "{3}{C}"), Default::default(), false)
            .is_some());
        assert!(e
            .payment_from_current_pool(&cost(&e, "{3}{C}{C}"), Default::default(), false)
            .is_none());
        assert_eq!(serde_json::to_vec(&e.state).unwrap(), before);
    }

    #[test]
    fn pool_witness_can_use_only_eligible_restricted_mana_without_double_counting_it() {
        let mut e = engine();
        e.state.players[0]
            .restricted_mana
            .push(crate::state::RestrictedManaContribution {
                restriction_group_id: 7,
                amount: ManaAmount::default(),
            });
        let mut prepared = cost(&e, "{G}{1}");
        prepared.eligible_restricted_mana = vec![7];
        e.state.players[0].restricted_mana[0].amount = ManaAmount {
            g: 1,
            c: 1,
            ..Default::default()
        };
        let witness = e
            .payment_from_current_pool(&prepared, Default::default(), false)
            .unwrap();
        assert_eq!(witness.restricted_mana.len(), 1);
        assert_eq!(witness.restricted_mana[0].restriction_group_id, 7);
        assert_eq!(witness.selection.mana, Some(rv1::PaymentMana::default()));
        prepared.eligible_restricted_mana.clear();
        assert!(e
            .payment_from_current_pool(&prepared, Default::default(), false)
            .is_none());
        prepared.eligible_restricted_mana = vec![7, 7];
        prepared.mana = ManaCost::parse("{G}{G}{2}").unwrap();
        assert!(e
            .payment_from_current_pool(&prepared, Default::default(), false)
            .is_none());
    }

    #[test]
    fn pool_witness_validates_phyrexian_life_together_with_other_life_costs() {
        let mut e = engine();
        let source = put(&mut e, "Grizzly Bears");
        e.state.players[0].life = 3;
        let prepared = e
            .prepare_ability_costs(
                0,
                0,
                source,
                &[
                    AbilityCost::Mana(ManaCost::parse("{U/P}").unwrap()),
                    AbilityCost::PayLife { amount: 2 },
                ],
                &[rv1::FlexPipPayment {
                    pip_index: 0,
                    pay_life: true,
                    mana_color: 0,
                }],
                &[],
                &[],
                0,
                0,
                Default::default(),
            )
            .unwrap();
        assert!(e
            .payment_from_current_pool(&prepared, Default::default(), false)
            .is_none());
        e.state.players[0].life = 4;
        assert!(e
            .payment_from_current_pool(&prepared, Default::default(), false)
            .is_some());
    }

    #[test]
    fn pool_witness_does_not_double_tap_convoke_and_other_costs() {
        let mut e = engine();
        let bear = put(&mut e, "Grizzly Bears");
        let source = e.payment_object_ref(bear);
        let prepared = cost(&e, "{G}");
        let witness = e
            .payment_from_current_pool(&prepared, source, true)
            .unwrap();
        assert_eq!(witness.selection.convoke.len(), 1);
        assert_eq!(witness.selection.convoke[0].object, Some(source));
        assert!(!e.state.objects[&bear].tapped);
        let tapped_cost = e
            .prepare_ability_costs(
                0,
                0,
                bear,
                &[
                    AbilityCost::Tap,
                    AbilityCost::Mana(ManaCost::parse("{G}").unwrap()),
                ],
                &[],
                &[],
                &[],
                0,
                0,
                Default::default(),
            )
            .unwrap();
        assert!(e
            .payment_from_current_pool(&tapped_cost, source, true)
            .is_none());
        let sacrifice_cost = e
            .prepare_ability_costs(
                0,
                0,
                bear,
                &[
                    AbilityCost::SacrificeSelf,
                    AbilityCost::Mana(ManaCost::parse("{G}").unwrap()),
                ],
                &[],
                &[],
                &[],
                0,
                0,
                Default::default(),
            )
            .unwrap();
        assert!(e
            .payment_from_current_pool(&sacrifice_cost, source, true)
            .is_some());
    }

    #[test]
    fn pool_witness_uses_waterbend_artifacts_without_treating_them_as_colored_mana() {
        let mut e = engine();
        let stone = put(&mut e, "Mind Stone");
        let mut payment = PendingManaPayment::from_cost(0, ManaCost::parse("{1}").unwrap(), 0);
        payment.waterbend = true;
        let prepared = e
            .prepare_resolution_payment_costs(0, &payment, &[], &[])
            .unwrap();
        let witness = e
            .payment_from_current_pool(&prepared, Default::default(), false)
            .unwrap();
        assert_eq!(
            witness.selection.waterbend,
            vec![e.payment_object_ref(stone)]
        );
        payment.mana_cost = ManaCost::parse("{G}").unwrap();
        let colored = e
            .prepare_resolution_payment_costs(0, &payment, &[], &[])
            .unwrap();
        assert!(e
            .payment_from_current_pool(&colored, Default::default(), false)
            .is_none());
    }

    #[test]
    fn available_sources_include_untapped_lands_and_do_not_mutate_the_game() {
        let mut e = engine();
        e.state.turn_step = TurnStep::Main1;
        e.state.priority_idx = 0;
        let forest = put(&mut e, "Forest");
        let before = serde_json::to_vec(&e.state).unwrap();
        let prepared = cost(&e, "{G}");
        assert!(e.payment_with_available_sources(
            &prepared,
            Default::default(),
            false,
            &|_| vec![],
            &mut 512,
            None,
        ));
        assert_eq!(serde_json::to_vec(&e.state).unwrap(), before);
        e.state.objects.get_mut(&forest).unwrap().tapped = true;
        assert!(!e.payment_with_available_sources(
            &prepared,
            Default::default(),
            false,
            &|_| vec![],
            &mut 512,
            None,
        ));
    }

    #[test]
    fn available_sources_do_not_spend_the_action_source_twice() {
        let mut e = engine();
        e.state.turn_step = TurnStep::Main1;
        e.state.priority_idx = 0;
        let stone = put(&mut e, "Mind Stone");
        let prepared = e
            .prepare_ability_costs(
                0,
                0,
                stone,
                &[
                    AbilityCost::Mana(ManaCost::parse("{1}").unwrap()),
                    AbilityCost::Tap,
                    AbilityCost::SacrificeSelf,
                ],
                &[],
                &[],
                &[],
                0,
                0,
                Default::default(),
            )
            .unwrap();
        let source = e.payment_object_ref(stone);
        assert!(!e.payment_with_available_sources(
            &prepared,
            source,
            false,
            &|_| vec![],
            &mut 512,
            None
        ));
        put(&mut e, "Island");
        assert!(e.payment_with_available_sources(
            &prepared,
            source,
            false,
            &|_| vec![],
            &mut 512,
            None
        ));
        assert!(!e.state.objects[&stone].tapped);
        assert_eq!(e.state.objects[&stone].zone, Zone::Battlefield);
    }
}
