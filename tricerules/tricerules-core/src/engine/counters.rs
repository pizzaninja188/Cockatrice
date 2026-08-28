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

    /// Reserve fixed kinds first, then test the combined arbitrary-counter demand.
    pub(super) fn counter_costs_payable(&self, source: ObjectId, costs: &[AbilityCost]) -> bool {
        let Some(object) = self.state.objects.get(&source) else {
            return false;
        };
        let mut bag = object.counters.clone();
        let mut any = 0u64;
        for cost in costs {
            let (kind, amount) = match cost {
                AbilityCost::RemoveCounters {
                    counter: Some(kind),
                    count,
                } => (*kind, *count),
                AbilityCost::RemoveCounters {
                    counter: None,
                    count,
                } => {
                    any += u64::from(*count);
                    continue;
                }
                AbilityCost::Loyalty(delta) if *delta < 0 => {
                    (CounterKind::Loyalty, delta.unsigned_abs())
                }
                _ => continue,
            };
            let available = bag.entry(kind).or_default();
            let Some(left) = available.checked_sub(amount) else {
                return false;
            };
            *available = left;
        }
        bag.values().map(|n| u64::from(*n)).sum::<u64>() >= any
    }
}
