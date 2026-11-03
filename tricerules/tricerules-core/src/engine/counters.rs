//! Counter payments shared by Brambleback Brute and Walking Ballista.
use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(super) struct CounterInventoryKey {
    pub(super) object_id: ObjectId,
    pub(super) zone_change_generation: u64,
    pub(super) kind: CounterKind,
}

#[derive(Debug, Clone, Default)]
pub(super) struct CounterInventory {
    slots: BTreeMap<CounterInventoryKey, u32>,
}

impl CounterInventory {
    pub(super) fn capacity(&self, key: CounterInventoryKey) -> u32 {
        self.slots.get(&key).copied().unwrap_or(0)
    }

    pub(super) fn keys(&self) -> impl Iterator<Item = CounterInventoryKey> + '_ {
        self.slots.keys().copied()
    }
}

#[derive(Debug, Clone)]
pub(super) enum CounterDemand {
    Whole {
        count: u32,
        candidates: Vec<CounterInventoryKey>,
    },
    Split {
        count: u32,
        candidates: Vec<CounterInventoryKey>,
    },
}

impl CounterDemand {
    fn count(&self) -> u32 {
        match self {
            Self::Whole { count, .. } | Self::Split { count, .. } => *count,
        }
    }
}

impl GameEngine {
    /// Preserve ordinary cost affordability's existing inventory domain. Transaction validation
    /// uses `live_counter_inventory`, which intentionally excludes phased objects and zero slots.
    fn ordinary_counter_inventory(&self) -> CounterInventory {
        let mut slots = BTreeMap::new();
        for player in &self.state.players {
            for &object_id in &player.battlefield {
                let Some(object) = self.state.objects.get(&object_id) else {
                    continue;
                };
                let zone_change_generation = self
                    .state
                    .zone_change_generation
                    .get(&object_id)
                    .copied()
                    .unwrap_or(0);
                for (&kind, &count) in &object.counters {
                    slots.insert(
                        CounterInventoryKey {
                            object_id,
                            zone_change_generation,
                            kind,
                        },
                        count,
                    );
                }
            }
        }
        CounterInventory { slots }
    }

    pub(super) fn live_counter_inventory(&self) -> CounterInventory {
        let mut slots = BTreeMap::new();
        for (&object_id, object) in &self.state.objects {
            if object.zone != Zone::Battlefield || object.phase_status.is_some() {
                continue;
            }
            let zone_change_generation = self
                .state
                .zone_change_generation
                .get(&object_id)
                .copied()
                .unwrap_or(0);
            for (&kind, &count) in &object.counters {
                if count > 0 {
                    slots.insert(
                        CounterInventoryKey {
                            object_id,
                            zone_change_generation,
                            kind,
                        },
                        count,
                    );
                }
            }
        }
        CounterInventory { slots }
    }

    pub(super) fn filtered_counter_inventory_keys(
        &self,
        inventory: &CounterInventory,
        player: PlayerId,
        source: rv1::CostObjectRef,
        filter: &TargetFilter,
        fixed_kind: Option<CounterKind>,
    ) -> Vec<CounterInventoryKey> {
        inventory
            .keys()
            .filter(|key| fixed_kind.is_none_or(|kind| key.kind == kind))
            .filter(|key| {
                self.aggregate_counter_permanent_matches(player, source, key.object_id, filter)
            })
            .collect()
    }

    /// Whether the accepted fixed aggregate counter components can be funded by the current
    /// generation-bound inventory. The caller must first prove that the full ability shape is
    /// supported; ordinary activation and payment paths remain fail-closed.
    pub(super) fn accepted_aggregate_counter_costs_payable(
        &self,
        source: ObjectId,
        costs: &[AbilityCost],
    ) -> bool {
        let Some(player) = self.controller_of(source) else {
            return false;
        };
        let source = self.payment_object_ref(source);
        let inventory = self.live_counter_inventory();
        let mut demands = Vec::new();
        for cost in costs {
            match cost {
                AbilityCost::RemoveCountersFromPermanents {
                    count,
                    counter,
                    filter,
                } if *count > 0 => {
                    let candidates = self.filtered_counter_inventory_keys(
                        &inventory, player, source, filter, *counter,
                    );
                    demands.push(CounterDemand::Split {
                        count: *count,
                        candidates,
                    });
                }
                AbilityCost::Mana(_) | AbilityCost::PayLife { .. } => {}
                _ => return false,
            }
        }
        !demands.is_empty() && counter_demands_payable(&inventory, &demands)
    }

    pub(super) fn aggregate_counter_removal_group_choice(
        &self,
        player: PlayerId,
        source: ObjectId,
        cost_index: usize,
        required_count: u32,
        fixed_kind: Option<CounterKind>,
        filter: &TargetFilter,
    ) -> rv1::LegalCostChoice {
        let inventory = self.live_counter_inventory();
        let source_ref = self.payment_object_ref(source);
        let mut grouped =
            std::collections::BTreeMap::<(ObjectId, u64), Vec<rv1::CounterRemovalOption>>::new();
        for key in
            self.filtered_counter_inventory_keys(&inventory, player, source_ref, filter, fixed_kind)
        {
            let available_count = inventory.capacity(key);
            if available_count == 0 {
                continue;
            }
            grouped
                .entry((key.object_id, key.zone_change_generation))
                .or_default()
                .push(rv1::CounterRemovalOption {
                    option_id: counter_option_id(key.kind),
                    label: key.kind.label(),
                    available_count,
                });
        }
        let candidates = grouped
            .into_iter()
            .map(|((object_id, zone_change_generation), options)| {
                rv1::CounterRemovalGroupCandidate {
                    object: Some(rv1::CostObjectRef {
                        object_id,
                        zone_change_generation,
                    }),
                    options,
                }
            })
            .collect();
        rv1::LegalCostChoice {
            cost_index: cost_index as u32,
            zone: rv1::CostChoiceZone::Battlefield as i32,
            kind: rv1::CostChoiceKind::RemoveCounters as i32,
            counter_removal_group: Some(rv1::CounterRemovalGroupChoices {
                required_count,
                candidates,
            }),
            ..Default::default()
        }
    }
}

pub(super) fn counter_demands_payable(
    inventory: &CounterInventory,
    demands: &[CounterDemand],
) -> bool {
    let total = demands.iter().try_fold(0u64, |total, demand| {
        total.checked_add(u64::from(demand.count()))
    });
    if total.is_none() {
        return false;
    }
    let total_split = demands
        .iter()
        .filter_map(|demand| match demand {
            CounterDemand::Split { count, .. } => Some(u64::from(*count)),
            CounterDemand::Whole { .. } => None,
        })
        .try_fold(0u64, u64::checked_add);
    let Some(total_split) = total_split else {
        return false;
    };

    let mut whole = demands
        .iter()
        .filter_map(|demand| match demand {
            CounterDemand::Whole { count, candidates } => Some((*count, candidates.clone())),
            CounterDemand::Split { .. } => None,
        })
        .collect::<Vec<_>>();
    whole.sort_by(|left, right| {
        left.1
            .len()
            .cmp(&right.1.len())
            .then_with(|| right.0.cmp(&left.0))
            .then_with(|| left.1.cmp(&right.1))
    });
    let mut split = demands
        .iter()
        .filter_map(|demand| match demand {
            CounterDemand::Split { count, candidates } => Some((*count, candidates.clone())),
            CounterDemand::Whole { .. } => None,
        })
        .collect::<Vec<_>>();
    split.sort_by(|left, right| {
        left.1
            .len()
            .cmp(&right.1.len())
            .then_with(|| right.0.cmp(&left.0))
            .then_with(|| left.1.cmp(&right.1))
    });

    let mut remaining = inventory.slots.clone();
    assign_whole_demands(0, &whole, &split, &mut remaining, total_split)
}

fn assign_whole_demands(
    index: usize,
    whole: &[(u32, Vec<CounterInventoryKey>)],
    split: &[(u32, Vec<CounterInventoryKey>)],
    remaining: &mut BTreeMap<CounterInventoryKey, u32>,
    total_split: u64,
) -> bool {
    let Some((count, candidates)) = whole.get(index) else {
        return split_demands_fit(split, remaining, total_split);
    };
    if *count == 0 {
        return assign_whole_demands(index + 1, whole, split, remaining, total_split);
    }
    for candidate in candidates {
        let available = remaining.get(candidate).copied().unwrap_or(0);
        if available < *count {
            continue;
        }
        remaining.insert(*candidate, available - count);
        if assign_whole_demands(index + 1, whole, split, remaining, total_split) {
            return true;
        }
        remaining.insert(*candidate, available);
    }
    false
}

fn split_demands_fit(
    demands: &[(u32, Vec<CounterInventoryKey>)],
    remaining: &BTreeMap<CounterInventoryKey, u32>,
    total: u64,
) -> bool {
    if total == 0 {
        return true;
    }
    let slots = demands
        .iter()
        .flat_map(|(_, candidates)| candidates.iter().copied())
        .filter(|slot| remaining.get(slot).copied().unwrap_or(0) > 0)
        .collect::<BTreeSet<_>>();
    let slot_indices = slots
        .iter()
        .enumerate()
        .map(|(index, slot)| (*slot, index))
        .collect::<BTreeMap<_, _>>();

    let source = 0usize;
    let demand_start = 1usize;
    let slot_start = demand_start + demands.len();
    let sink = slot_start + slots.len();
    let mut graph = std::iter::repeat_with(Vec::<CounterFlowEdge>::new)
        .take(sink + 1)
        .collect::<Vec<_>>();
    for (index, (count, candidates)) in demands.iter().enumerate() {
        if *count == 0 {
            continue;
        }
        let demand_node = demand_start + index;
        add_counter_flow_edge(&mut graph, source, demand_node, u64::from(*count));
        for candidate in candidates {
            let Some(&slot_index) = slot_indices.get(candidate) else {
                continue;
            };
            add_counter_flow_edge(
                &mut graph,
                demand_node,
                slot_start + slot_index,
                u64::from(*count),
            );
        }
    }
    for (slot_index, slot) in slots.iter().enumerate() {
        add_counter_flow_edge(
            &mut graph,
            slot_start + slot_index,
            sink,
            u64::from(remaining.get(slot).copied().unwrap_or(0)),
        );
    }
    counter_max_flow(&mut graph, source, sink, total) == Some(total)
}

#[derive(Clone)]
struct CounterFlowEdge {
    to: usize,
    reverse: usize,
    residual: u64,
}

fn add_counter_flow_edge(
    graph: &mut [Vec<CounterFlowEdge>],
    from: usize,
    to: usize,
    capacity: u64,
) {
    let forward_reverse = graph[to].len();
    let backward_reverse = graph[from].len();
    graph[from].push(CounterFlowEdge {
        to,
        reverse: forward_reverse,
        residual: capacity,
    });
    graph[to].push(CounterFlowEdge {
        to: from,
        reverse: backward_reverse,
        residual: 0,
    });
}

fn counter_max_flow(
    graph: &mut [Vec<CounterFlowEdge>],
    source: usize,
    sink: usize,
    target: u64,
) -> Option<u64> {
    let mut flow = 0u64;
    while flow < target {
        let mut parent = vec![None::<(usize, usize)>; graph.len()];
        let mut queue = std::collections::VecDeque::from([source]);
        parent[source] = Some((source, usize::MAX));
        while let Some(node) = queue.pop_front() {
            for (edge_index, edge) in graph[node].iter().enumerate() {
                if edge.residual > 0 && parent[edge.to].is_none() {
                    parent[edge.to] = Some((node, edge_index));
                    if edge.to == sink {
                        break;
                    }
                    queue.push_back(edge.to);
                }
            }
            if parent[sink].is_some() {
                break;
            }
        }
        if parent[sink].is_none() {
            break;
        }
        let mut amount = target - flow;
        let mut node = sink;
        while node != source {
            let (previous, edge_index) = parent[node]?;
            amount = amount.min(graph[previous][edge_index].residual);
            node = previous;
        }
        if amount == 0 {
            return None;
        }
        node = sink;
        while node != source {
            let (previous, edge_index) = parent[node]?;
            let reverse = graph[previous][edge_index].reverse;
            graph[previous][edge_index].residual -= amount;
            graph[node][reverse].residual = graph[node][reverse].residual.checked_add(amount)?;
            node = previous;
        }
        flow = flow.checked_add(amount)?;
    }
    Some(flow)
}

/// Stable within the rules vocabulary, independent of the changing counter bag's ordering.
pub(super) fn counter_option_id(kind: CounterKind) -> u32 {
    match kind {
        CounterKind::PlusOnePlusOne => 1,
        CounterKind::MinusOneMinusOne => 2,
        CounterKind::Stun => 3,
        CounterKind::Loyalty => 4,
        CounterKind::Defense => 5,
        CounterKind::Lore => 6,
        CounterKind::Charge => 7,
        CounterKind::Quest => 8,
        CounterKind::Finality => 9,
        CounterKind::Foreshadow => 10,
        CounterKind::Storage => 11,
        CounterKind::Poison => 12,
        CounterKind::Night => 13,
        CounterKind::Doom => 14,
        CounterKind::Time => 15,
        CounterKind::Luck => 16,
        CounterKind::Keyword(keyword) => 256 + keyword as u32,
    }
}

impl GameEngine {
    pub(super) fn counter_removal_choice(
        &self,
        source: ObjectId,
        cost_index: usize,
        counter: Option<CounterKind>,
        count: u32,
    ) -> rv1::LegalCostChoice {
        let options = self
            .state
            .objects
            .get(&source)
            .filter(|o| o.zone == Zone::Battlefield)
            .map(|object| {
                object
                    .counters
                    .iter()
                    .filter(|(kind, available)| {
                        counter.is_none_or(|required| required == **kind) && **available >= count
                    })
                    .map(|(&kind, &available)| rv1::CounterRemovalOption {
                        option_id: counter_option_id(kind),
                        label: kind.label(),
                        available_count: available,
                    })
                    .collect()
            })
            .unwrap_or_default();
        rv1::LegalCostChoice {
            cost_index: cost_index as u32,
            zone: rv1::CostChoiceZone::Battlefield as i32,
            kind: rv1::CostChoiceKind::RemoveCounters as i32,
            min: 1,
            max: 1,
            counter_removal: Some(rv1::CounterRemovalChoices {
                source: Some(rv1::CostObjectRef {
                    object_id: source,
                    zone_change_generation: self
                        .state
                        .zone_change_generation
                        .get(&source)
                        .copied()
                        .unwrap_or(0),
                }),
                count,
                options,
            }),
            ..Default::default()
        }
    }

    pub(super) fn selected_counter_removal_choice(
        &self,
        player: PlayerId,
        source: ObjectId,
        cost_index: usize,
        counter: CounterKind,
        count: u32,
        filter: &TargetFilter,
    ) -> rv1::LegalCostChoice {
        let candidates = self
            .state
            .players
            .iter()
            .flat_map(|state| state.battlefield.iter().copied())
            .filter(|&oid| self.counter_payment_permanent_matches(player, source, oid, filter))
            .filter_map(|oid| {
                let available = self.state.objects[&oid].counter_count(counter);
                (available >= count).then_some((oid, available))
            })
            .collect::<Vec<_>>();
        rv1::LegalCostChoice {
            cost_index: cost_index as u32,
            zone: rv1::CostChoiceZone::Battlefield as i32,
            kind: rv1::CostChoiceKind::RemoveCounters as i32,
            min: 1,
            max: 1,
            candidate_ids: candidates.iter().map(|(oid, _)| *oid).collect(),
            candidate_objects: candidates
                .iter()
                .map(|(oid, available)| rv1::CostObjectCandidate {
                    object: Some(rv1::CostObjectRef {
                        object_id: *oid,
                        zone_change_generation: self
                            .state
                            .zone_change_generation
                            .get(oid)
                            .copied()
                            .unwrap_or(0),
                    }),
                    contribution: i64::from(*available),
                })
                .collect(),
            counter_removal: Some(rv1::CounterRemovalChoices {
                source: None,
                count,
                options: vec![rv1::CounterRemovalOption {
                    option_id: counter_option_id(counter),
                    label: counter.label(),
                    available_count: count,
                }],
            }),
            ..Default::default()
        }
    }

    /// Test every counter cost as one atomic assignment. This matters when the source itself is
    /// also an eligible selected permanent or one ability contains repeated counter costs.
    pub(super) fn counter_costs_payable(&self, source: ObjectId, costs: &[AbilityCost]) -> bool {
        let Some(object) = self.state.objects.get(&source) else {
            return false;
        };
        let player = object.controller;
        let inventory = self.ordinary_counter_inventory();
        let counter_inventory_key = |object_id, kind| CounterInventoryKey {
            object_id,
            zone_change_generation: self
                .state
                .zone_change_generation
                .get(&object_id)
                .copied()
                .unwrap_or(0),
            kind,
        };
        let mut demands = Vec::<CounterDemand>::new();
        for cost in costs {
            match cost {
                AbilityCost::RemoveCountersFromPermanents { .. } => return false,
                AbilityCost::RemoveCounters {
                    counter: Some(kind),
                    count,
                    payment_source: CounterRemovalPaymentSource::Source,
                } => demands.push(CounterDemand::Whole {
                    count: *count,
                    candidates: vec![counter_inventory_key(source, *kind)],
                }),
                AbilityCost::RemoveCounters {
                    counter: None,
                    count,
                    payment_source: CounterRemovalPaymentSource::Source,
                } => {
                    let candidates = object
                        .counters
                        .keys()
                        .copied()
                        .map(|kind| counter_inventory_key(source, kind))
                        .collect();
                    demands.push(CounterDemand::Whole {
                        count: *count,
                        candidates,
                    });
                }
                AbilityCost::RemoveCounters {
                    counter: Some(kind),
                    count,
                    payment_source: CounterRemovalPaymentSource::SelectedPermanent(filter),
                } => {
                    let candidates = self
                        .state
                        .players
                        .iter()
                        .flat_map(|state| state.battlefield.iter().copied())
                        .filter(|&oid| {
                            self.counter_payment_permanent_matches(player, source, oid, filter)
                        })
                        .map(|oid| counter_inventory_key(oid, *kind))
                        .collect();
                    demands.push(CounterDemand::Whole {
                        count: *count,
                        candidates,
                    });
                }
                AbilityCost::RemoveCounters {
                    counter: None,
                    payment_source: CounterRemovalPaymentSource::SelectedPermanent(_),
                    ..
                } => return false,
                AbilityCost::Loyalty(printed) => {
                    let actor = self.controller_of(source).unwrap_or(player);
                    let Some(delta) = self.effective_loyalty_cost(actor, source, *printed) else {
                        return false;
                    };
                    if delta < 0 {
                        demands.push(CounterDemand::Whole {
                            count: delta.unsigned_abs(),
                            candidates: vec![counter_inventory_key(source, CounterKind::Loyalty)],
                        });
                    }
                }
                AbilityCost::LoyaltyX => {
                    let actor = self.controller_of(source).unwrap_or(player);
                    if self.loyalty_x_range(actor, source).is_none() {
                        return false;
                    }
                }
                _ => continue,
            }
        }
        if demands.iter().any(|demand| {
            matches!(
                demand,
                CounterDemand::Whole {
                    count: 0,
                    candidates
                } if candidates.is_empty()
            )
        }) {
            return false;
        }
        counter_demands_payable(&inventory, &demands)
    }
}

#[cfg(test)]
mod replicating_ring_tests {
    use super::*;

    #[test]
    fn night_counter_option_preserves_existing_ids_and_keyword_namespace() {
        assert_eq!(counter_option_id(CounterKind::Night), 13);
        assert_eq!(counter_option_id(CounterKind::Doom), 14);
        assert_eq!(counter_option_id(CounterKind::Poison), 12);
        assert_eq!(counter_option_id(CounterKind::Charge), 7);
        assert!(counter_option_id(CounterKind::Keyword(Keyword::Flying)) >= 256);
    }
}

#[cfg(test)]
mod aggregate_counter_tests {
    use super::*;

    fn inventory(entries: &[(CounterInventoryKey, u32)]) -> CounterInventory {
        CounterInventory {
            slots: entries.iter().copied().collect(),
        }
    }

    fn slot(object_id: ObjectId, kind: CounterKind) -> CounterInventoryKey {
        CounterInventoryKey {
            object_id,
            zone_change_generation: 0,
            kind,
        }
    }

    fn whole(count: u32, candidates: &[CounterInventoryKey]) -> CounterDemand {
        CounterDemand::Whole {
            count,
            candidates: candidates.to_vec(),
        }
    }

    fn split(count: u32, candidates: &[CounterInventoryKey]) -> CounterDemand {
        CounterDemand::Split {
            count,
            candidates: candidates.to_vec(),
        }
    }

    #[test]
    fn aggregate_counter_private_split_allocator_preserves_singletons() {
        let charge_a = slot(10, CounterKind::Charge);
        let night_b = slot(11, CounterKind::Night);
        assert!(counter_demands_payable(
            &inventory(&[(charge_a, 2), (night_b, 1)]),
            &[split(3, &[charge_a, night_b])],
        ));

        let night_a = slot(10, CounterKind::Night);
        assert!(counter_demands_payable(
            &inventory(&[(charge_a, 2), (night_a, 1)]),
            &[split(3, &[charge_a, night_a])],
        ));
        let charge_c = slot(12, CounterKind::Charge);
        let night_d = slot(13, CounterKind::Night);
        assert!(counter_demands_payable(
            &inventory(&[(charge_a, 1), (charge_c, 1), (night_d, 1)]),
            &[split(3, &[charge_a, charge_c, night_d])],
        ));
        assert!(counter_demands_payable(
            &inventory(&[(charge_a, 3)]),
            &[split(3, &[charge_a])],
        ));

        let charge_b = slot(11, CounterKind::Charge);
        let one_each = inventory(&[(charge_a, 1), (charge_b, 1)]);
        assert!(!counter_demands_payable(
            &one_each,
            &[whole(2, &[charge_a, charge_b])]
        ));
        assert!(counter_demands_payable(
            &one_each,
            &[split(2, &[charge_a, charge_b])]
        ));

        let charge_two = inventory(&[(charge_a, 2), (charge_b, 1)]);
        assert!(counter_demands_payable(
            &charge_two,
            &[whole(1, &[charge_a, charge_b]), split(2, &[charge_a])],
        ));
        assert!(counter_demands_payable(
            &inventory(&[(charge_a, 2), (charge_b, 2)]),
            &[whole(2, &[charge_a, charge_b]), split(2, &[charge_a])],
        ));

        let charge_c = slot(12, CounterKind::Charge);
        assert!(!counter_demands_payable(
            &inventory(&[(charge_a, 2), (charge_b, 2), (charge_c, 2)]),
            &[
                split(3, &[charge_a, charge_b]),
                split(2, &[charge_a, charge_b]),
                split(1, &[charge_c]),
            ],
        ));
        assert!(counter_demands_payable(
            &inventory(&[(charge_a, 2), (charge_b, 2)]),
            &[split(2, &[charge_a, charge_b]), split(2, &[charge_a])],
        ));

        let loyalty = slot(20, CounterKind::Loyalty);
        let charge = slot(21, CounterKind::Charge);
        let night = slot(22, CounterKind::Night);
        assert!(counter_demands_payable(
            &inventory(&[(loyalty, 2), (charge, 1), (night, 1)]),
            &[whole(1, &[loyalty]), split(2, &[charge, night])],
        ));
        assert!(!counter_demands_payable(
            &inventory(&[(loyalty, 1)]),
            &[whole(1, &[loyalty]), split(1, &[loyalty])],
        ));
        assert!(!counter_demands_payable(
            &inventory(&[(loyalty, 1)]),
            &[whole(2, &[loyalty])],
        ));
    }

    fn battlefield_card(engine: &mut GameEngine, player_index: usize, card_id: &str) -> ObjectId {
        let object_id = engine.state.players[player_index]
            .hand
            .first()
            .copied()
            .or_else(|| engine.state.players[player_index].library.front().copied())
            .expect("fixture has another physical card");
        engine.state.objects.get_mut(&object_id).unwrap().card_id = card_id.into();
        super::super::resolution::move_object_to_zone(
            &mut engine.state,
            engine.registry,
            object_id,
            Zone::Battlefield,
            None,
        )
        .expect("fixture card moves to the battlefield");
        object_id
    }

    fn aggregate_type_or_filter() -> TargetFilter {
        let branch = |permanent_type| TargetFilter {
            kind: TargetKind::AnyPermanent,
            controller: TargetController::You,
            owner: TargetOwner::You,
            excluded_objects: vec![tricerules_card_model::TargetObjectExclusion::Source],
            permanent_types: vec![permanent_type],
            ..TargetFilter::default()
        };
        TargetFilter {
            any_of: Some(vec![
                branch(PermanentTypeFilter::Artifact),
                branch(PermanentTypeFilter::Creature),
                branch(PermanentTypeFilter::Planeswalker),
            ]),
            ..TargetFilter::default()
        }
    }

    #[test]
    fn aggregate_counter_group_inventory_is_generation_bound_nontargeting_and_type_or() {
        let decks = (0..2).map(|_| vec!["forest".into(); 40]).collect();
        let mut engine = GameEngine::new(
            tricerules_cards::registry::global(),
            602_423,
            &[0, 1],
            20,
            Some(decks),
            true,
        )
        .expect("two-player inventory fixture");
        let source = battlefield_card(&mut engine, 0, "brambleback_brute");
        let artifact_creature = battlefield_card(&mut engine, 0, "ornithopter");
        let artifact_land = battlefield_card(&mut engine, 0, "darksteel_citadel");
        let creature = battlefield_card(&mut engine, 0, "grizzly_bears");
        let planeswalker = battlefield_card(&mut engine, 0, "jace_beleren");
        let pure_land = battlefield_card(&mut engine, 0, "forest");
        let enchantment = battlefield_card(&mut engine, 0, "pacifism");
        let protected_creature = battlefield_card(&mut engine, 0, "slippery_bogle");
        let phased_artifact = battlefield_card(&mut engine, 0, "ornithopter");
        let opponent_artifact = battlefield_card(&mut engine, 1, "ornithopter");
        let stolen_artifact = battlefield_card(&mut engine, 1, "ornithopter");
        engine
            .state
            .objects
            .get_mut(&stolen_artifact)
            .unwrap()
            .controller = 0;
        engine.state.players[1]
            .battlefield
            .retain(|object_id| *object_id != stolen_artifact);
        engine.state.players[0].battlefield.push(stolen_artifact);

        for (object_id, kind, count) in [
            (source, CounterKind::Charge, 1),
            (artifact_creature, CounterKind::Charge, 1),
            (artifact_creature, CounterKind::Night, 1),
            (artifact_land, CounterKind::Charge, 1),
            (creature, CounterKind::Night, 1),
            (planeswalker, CounterKind::Loyalty, 1),
            (pure_land, CounterKind::Charge, 1),
            (enchantment, CounterKind::Night, 1),
            (protected_creature, CounterKind::Doom, 1),
            (phased_artifact, CounterKind::Charge, 1),
            (opponent_artifact, CounterKind::Charge, 1),
            (stolen_artifact, CounterKind::Charge, 1),
        ] {
            engine
                .state
                .objects
                .get_mut(&object_id)
                .unwrap()
                .add_counters(kind, count, 0);
        }
        engine
            .state
            .objects
            .get_mut(&phased_artifact)
            .unwrap()
            .phase_status = Some(crate::state::PhasedOutStatus {
            phase_out_under: 0,
            phase_in_due_after_departed_turn: false,
            phase_in_with: None,
            direct_attachment_snapshot: None,
        });

        let source_ref = engine.payment_object_ref(source);
        let filter = aggregate_type_or_filter();
        let inventory = engine.live_counter_inventory();
        let candidates =
            engine.filtered_counter_inventory_keys(&inventory, 0, source_ref, &filter, None);
        let candidate_ids = candidates
            .iter()
            .map(|candidate| candidate.object_id)
            .collect::<BTreeSet<_>>();
        assert!(candidate_ids.contains(&artifact_creature));
        assert!(candidate_ids.contains(&artifact_land));
        assert!(candidate_ids.contains(&creature));
        assert!(candidate_ids.contains(&planeswalker));
        assert!(candidate_ids.contains(&protected_creature));
        assert!(
            !candidate_ids.contains(&source),
            "the captured source is excluded"
        );
        assert!(!candidate_ids.contains(&pure_land));
        assert!(!candidate_ids.contains(&enchantment));
        assert!(!candidate_ids.contains(&phased_artifact));
        assert!(!candidate_ids.contains(&opponent_artifact));
        assert!(!candidate_ids.contains(&stolen_artifact));
        let artifact_creature_slots = candidates
            .iter()
            .filter(|candidate| candidate.object_id == artifact_creature)
            .collect::<Vec<_>>();
        assert_eq!(artifact_creature_slots.len(), 2);
        assert!(artifact_creature_slots
            .iter()
            .any(|candidate| candidate.kind == CounterKind::Charge));
        assert!(artifact_creature_slots
            .iter()
            .any(|candidate| candidate.kind == CounterKind::Night));
        assert!(
            candidates.iter().any(|candidate| {
                candidate.object_id == artifact_land
                    && candidate.kind == CounterKind::Charge
                    && inventory.capacity(*candidate) == 1
            }),
            "one-unit options remain eligible for a larger total group"
        );
        assert_eq!(
            engine
                .filtered_counter_inventory_keys(
                    &inventory,
                    0,
                    source_ref,
                    &filter,
                    Some(CounterKind::Night),
                )
                .into_iter()
                .filter(|candidate| candidate.object_id == artifact_creature)
                .count(),
            1,
            "fixed-kind inventory preserves the existing Charge/Night option mapping"
        );

        engine.state.objects.get_mut(&source).unwrap().card_id = "brambleback_brute".into();
        super::super::resolution::move_object_to_zone(
            &mut engine.state,
            engine.registry,
            source,
            Zone::Graveyard,
            None,
        )
        .expect("captured source leaves");
        super::super::resolution::move_object_to_zone(
            &mut engine.state,
            engine.registry,
            source,
            Zone::Battlefield,
            None,
        )
        .expect("same physical source returns as a new incarnation");
        engine
            .state
            .objects
            .get_mut(&source)
            .unwrap()
            .add_counters(CounterKind::Charge, 1, 0);
        let returned_inventory = engine.live_counter_inventory();
        let returned_candidates = engine.filtered_counter_inventory_keys(
            &returned_inventory,
            0,
            source_ref,
            &filter,
            Some(CounterKind::Charge),
        );
        assert!(
            returned_candidates.iter().any(|candidate| {
                candidate.object_id == source
                    && candidate.zone_change_generation != source_ref.zone_change_generation
            }),
            "source exclusion remains anchored to the original generation"
        );

        let mut controller_only = filter.clone();
        controller_only.owner = TargetOwner::Any;
        controller_only.any_of = None;
        controller_only.kind = TargetKind::AnyPermanent;
        controller_only.controller = TargetController::You;
        controller_only.permanent_types = vec![PermanentTypeFilter::Artifact];
        controller_only.excluded_objects.clear();
        assert_eq!(engine.state.objects[&stolen_artifact].controller, 0);
        assert_eq!(engine.controller_of(stolen_artifact), Some(1));
        assert!(!engine
            .filtered_counter_inventory_keys(
                &returned_inventory,
                0,
                source_ref,
                &controller_only,
                Some(CounterKind::Charge),
            )
            .iter()
            .any(|candidate| candidate.object_id == stolen_artifact));
        engine
            .state
            .objects
            .get_mut(&stolen_artifact)
            .unwrap()
            .base_controller = 0;
        assert_eq!(engine.controller_of(stolen_artifact), Some(0));
        assert!(engine
            .filtered_counter_inventory_keys(
                &returned_inventory,
                0,
                source_ref,
                &controller_only,
                Some(CounterKind::Charge),
            )
            .iter()
            .any(|candidate| candidate.object_id == stolen_artifact));
    }

    #[test]
    fn ordinary_counter_costs_preserve_legacy_zero_demand_candidate_rules() {
        let decks = (0..2).map(|_| vec!["forest".into(); 40]).collect();
        let mut engine = GameEngine::new(
            tricerules_cards::registry::global(),
            602_424,
            &[0, 1],
            20,
            Some(decks),
            true,
        )
        .expect("ordinary counter cost fixture");
        let source = battlefield_card(&mut engine, 0, "brambleback_brute");
        engine
            .state
            .objects
            .get_mut(&source)
            .unwrap()
            .counters
            .clear();

        let source_any_kind_zero = [AbilityCost::RemoveCounters {
            counter: None,
            count: 0,
            payment_source: CounterRemovalPaymentSource::Source,
        }];
        assert!(
            !engine.counter_costs_payable(source, &source_any_kind_zero),
            "a zero Whole demand with no candidates remains rejected"
        );
        engine
            .state
            .objects
            .get_mut(&source)
            .unwrap()
            .counters
            .insert(CounterKind::Charge, 0);
        assert!(
            engine.counter_costs_payable(source, &source_any_kind_zero),
            "a stored zero-valued counter kind still supplies a legacy candidate"
        );

        let selected_empty = [AbilityCost::RemoveCounters {
            counter: Some(CounterKind::Charge),
            count: 0,
            payment_source: CounterRemovalPaymentSource::SelectedPermanent(Box::new(
                TargetFilter {
                    kind: TargetKind::Creature,
                    controller: TargetController::You,
                    owner: TargetOwner::You,
                    excluded_objects: vec![tricerules_card_model::TargetObjectExclusion::Source],
                    ..TargetFilter::default()
                },
            )),
        }];
        assert!(
            !engine.counter_costs_payable(source, &selected_empty),
            "a selected-permanent zero Whole demand with no candidates remains rejected"
        );
    }

    #[test]
    fn ordinary_counter_costs_keep_historical_phased_inventory_and_any_kind_rejection() {
        let decks = (0..2).map(|_| vec!["forest".into(); 40]).collect();
        let mut engine = GameEngine::new(
            tricerules_cards::registry::global(),
            602_425,
            &[0, 1],
            20,
            Some(decks),
            true,
        )
        .expect("ordinary counter inventory fixture");
        let source = battlefield_card(&mut engine, 0, "brambleback_brute");
        engine
            .state
            .objects
            .get_mut(&source)
            .unwrap()
            .add_counters(CounterKind::Charge, 1, 0);
        engine.state.objects.get_mut(&source).unwrap().phase_status =
            Some(crate::state::PhasedOutStatus {
                phase_out_under: 0,
                phase_in_due_after_departed_turn: false,
                phase_in_with: None,
                direct_attachment_snapshot: None,
            });

        let key = CounterInventoryKey {
            object_id: source,
            zone_change_generation: engine.state.zone_change_generation[&source],
            kind: CounterKind::Charge,
        };
        assert_eq!(
            engine.live_counter_inventory().capacity(key),
            0,
            "transaction validation keeps phased objects out of live inventory"
        );
        let source_cost = [AbilityCost::RemoveCounters {
            counter: Some(CounterKind::Charge),
            count: 1,
            payment_source: CounterRemovalPaymentSource::Source,
        }];
        assert!(
            engine.counter_costs_payable(source, &source_cost),
            "ordinary source affordability retains its historical battlefield-vector inventory"
        );

        let bearer = battlefield_card(&mut engine, 0, "grizzly_bears");
        engine.state.objects.get_mut(&bearer).unwrap().add_counters(
            CounterKind::PlusOnePlusOne,
            1,
            0,
        );
        let filter = TargetFilter {
            kind: TargetKind::Creature,
            controller: TargetController::You,
            owner: TargetOwner::You,
            excluded_objects: vec![tricerules_card_model::TargetObjectExclusion::Source],
            ..TargetFilter::default()
        };
        assert!(engine.counter_payment_permanent_matches(0, source, bearer, &filter));

        let selected_any_kind = [AbilityCost::RemoveCounters {
            counter: None,
            count: 1,
            payment_source: CounterRemovalPaymentSource::SelectedPermanent(Box::new(filter)),
        }];
        assert!(
            !engine.counter_costs_payable(source, &selected_any_kind),
            "selected-permanent any-kind costs remain outside ordinary admission"
        );
    }
}
