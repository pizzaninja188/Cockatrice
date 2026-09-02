//! Counter payments shared by Brambleback Brute and Walking Ballista.
use super::*;

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
        let mut remaining = self
            .state
            .players
            .iter()
            .flat_map(|state| state.battlefield.iter().copied())
            .filter_map(|oid| self.state.objects.get(&oid).map(|object| (oid, object)))
            .flat_map(|(oid, object)| {
                object
                    .counters
                    .iter()
                    .map(move |(&kind, &count)| ((oid, kind), count))
            })
            .collect::<BTreeMap<_, _>>();
        let mut demands = Vec::<(u32, Vec<(ObjectId, CounterKind)>)>::new();
        for cost in costs {
            match cost {
                AbilityCost::RemoveCounters {
                    counter: Some(kind),
                    count,
                    payment_source: CounterRemovalPaymentSource::Source,
                } => demands.push((*count, vec![(source, *kind)])),
                AbilityCost::RemoveCounters {
                    counter: None,
                    count,
                    payment_source: CounterRemovalPaymentSource::Source,
                } => {
                    let candidates = object
                        .counters
                        .keys()
                        .copied()
                        .map(|kind| (source, kind))
                        .collect();
                    demands.push((*count, candidates));
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
                        .map(|oid| (oid, *kind))
                        .collect();
                    demands.push((*count, candidates));
                }
                AbilityCost::RemoveCounters {
                    counter: None,
                    payment_source: CounterRemovalPaymentSource::SelectedPermanent(_),
                    ..
                } => return false,
                AbilityCost::Loyalty(delta) if *delta < 0 => {
                    demands.push((delta.unsigned_abs(), vec![(source, CounterKind::Loyalty)]));
                }
                _ => continue,
            }
        }
        demands.sort_by_key(|(_, candidates)| candidates.len());
        counter_assignment_exists(&demands, 0, &mut remaining)
    }
}

fn counter_assignment_exists(
    demands: &[(u32, Vec<(ObjectId, CounterKind)>)],
    index: usize,
    remaining: &mut BTreeMap<(ObjectId, CounterKind), u32>,
) -> bool {
    let Some((count, candidates)) = demands.get(index) else {
        return true;
    };
    for candidate in candidates {
        let available = remaining.get(candidate).copied().unwrap_or(0);
        if available < *count {
            continue;
        }
        remaining.insert(*candidate, available - count);
        if counter_assignment_exists(demands, index + 1, remaining) {
            return true;
        }
        remaining.insert(*candidate, available);
    }
    false
}
