//! Bounded proposal enumeration for read-only affordability witnesses. Candidates are always
//! checked by the normal announcement and payment validators before a positive result.
use super::super::*;

/// Already resolved abilities for one immutable legal-action snapshot. Never reuse after a
/// simulated mana activation, which can change costs, grants, control or characteristics.
pub(in crate::engine) type AbilityInventory = [(ObjectId, Vec<EffectiveActivatedAbility>)];

pub(in crate::engine) const PROPOSAL_LIMIT: usize = 256;

pub(in crate::engine) fn combinations<T: Clone>(
    items: &[T],
    min: usize,
    max: usize,
) -> Vec<Vec<T>> {
    fn visit<T: Clone>(
        items: &[T],
        start: usize,
        min: usize,
        max: usize,
        chosen: &mut Vec<T>,
        out: &mut Vec<Vec<T>>,
    ) {
        if out.len() >= PROPOSAL_LIMIT {
            return;
        }
        if chosen.len() >= min {
            out.push(chosen.clone());
        }
        if chosen.len() == max {
            return;
        }
        for index in start..items.len() {
            if chosen.len() + items.len() - index < min {
                break;
            }
            chosen.push(items[index].clone());
            visit(items, index + 1, min, max, chosen, out);
            chosen.pop();
            if out.len() >= PROPOSAL_LIMIT {
                break;
            }
        }
    }
    let mut out = Vec::new();
    if min <= max {
        visit(
            items,
            0,
            min,
            max.min(items.len()),
            &mut Vec::new(),
            &mut out,
        );
    }
    out
}

pub(in crate::engine) fn product<T: Clone>(groups: Vec<Vec<T>>) -> Vec<Vec<T>> {
    let mut result = vec![vec![]];
    for group in groups {
        let mut next = Vec::new();
        'outer: for prefix in result {
            for value in &group {
                let mut row = prefix.clone();
                row.push(value.clone());
                next.push(row);
                if next.len() == PROPOSAL_LIMIT {
                    break 'outer;
                }
            }
        }
        result = next;
    }
    result
}

pub(in crate::engine) fn cost_selections(
    costs: &rv1::LegalCostChoices,
) -> Vec<Vec<rv1::CostSelection>> {
    use rv1::cost_selection::Selection;
    let groups = costs
        .choices
        .iter()
        .map(|choice| {
            let mut values = Vec::new();
            if let Some(counter) = &choice.counter_removal {
                let sources = counter.source.map_or_else(
                    || {
                        choice
                            .candidate_objects
                            .iter()
                            .filter_map(|c| c.object)
                            .collect()
                    },
                    |source| vec![source],
                );
                for source in sources {
                    for option in &counter.options {
                        if option.available_count >= counter.count {
                            values.push(Selection::CounterRemoval(rv1::CounterRemovalSelection {
                                source: Some(source),
                                option_id: option.option_id,
                            }));
                        }
                    }
                }
            } else if let Some(group) = &choice.counter_removal_group {
                let rows: Vec<_> = group
                    .candidates
                    .iter()
                    .flat_map(|candidate| {
                        candidate.options.iter().map(|option| {
                            (candidate.object, option.option_id, option.available_count)
                        })
                    })
                    .collect();
                fn distribute(
                    rows: &[(Option<rv1::CostObjectRef>, u32, u32)],
                    index: usize,
                    remaining: u32,
                    selected: &mut Vec<rv1::CounterRemovalContribution>,
                    out: &mut Vec<Selection>,
                ) {
                    if out.len() >= PROPOSAL_LIMIT {
                        return;
                    }
                    if remaining == 0 {
                        out.push(Selection::CounterRemovalGroup(
                            rv1::CounterRemovalGroupSelection {
                                contributions: selected.clone(),
                            },
                        ));
                        return;
                    }
                    let Some(&(object, option_id, available)) = rows.get(index) else {
                        return;
                    };
                    for count in (1..=available.min(remaining)).rev() {
                        selected.push(rv1::CounterRemovalContribution {
                            object,
                            option_id,
                            count,
                        });
                        distribute(rows, index + 1, remaining - count, selected, out);
                        selected.pop();
                        if out.len() >= PROPOSAL_LIMIT {
                            return;
                        }
                    }
                    distribute(rows, index + 1, remaining, selected, out);
                }
                distribute(&rows, 0, group.required_count, &mut Vec::new(), &mut values);
            } else if choice.zone == rv1::CostChoiceZone::Hand as i32 {
                values.extend(
                    choice
                        .candidate_ids
                        .iter()
                        .copied()
                        .map(Selection::HandIndex),
                );
            } else if choice.kind == rv1::CostChoiceKind::Blight as i32
                || (choice.kind == rv1::CostChoiceKind::Sacrifice as i32 && choice.max == 1)
            {
                values.extend(
                    choice
                        .candidate_ids
                        .iter()
                        .copied()
                        .map(Selection::PermanentId),
                );
            } else {
                for selected in combinations(
                    &choice.candidate_objects,
                    choice.min as usize,
                    choice.max as usize,
                ) {
                    if choice.aggregate_minimum.as_ref().is_some_and(|constraint| {
                        selected
                            .iter()
                            .map(|c| i128::from(c.contribution))
                            .sum::<i128>()
                            < i128::from(constraint.minimum)
                    }) {
                        continue;
                    }
                    let objects = rv1::CostObjectRefs {
                        objects: selected.iter().filter_map(|c| c.object).collect(),
                    };
                    values.push(if choice.zone == rv1::CostChoiceZone::Graveyard as i32 {
                        Selection::GraveyardObjects(objects)
                    } else {
                        Selection::BattlefieldObjects(objects)
                    });
                }
            }
            values
                .into_iter()
                .map(|selection| rv1::CostSelection {
                    cost_index: choice.cost_index,
                    selection: Some(selection),
                })
                .collect()
        })
        .collect();
    product(groups)
}

pub(in crate::engine) fn phyrexian_choices(cost: &ManaCost) -> Vec<Vec<rv1::FlexPipPayment>> {
    let groups = cost
        .pips
        .iter()
        .enumerate()
        .filter_map(|(index, pip)| {
            let colors = match pip {
                ManaSymbol::Phyrexian(_) => vec![0],
                ManaSymbol::HybridPhyrexian(a, b) => vec![
                    super::mana::color_index(*a) as i32 + 1,
                    super::mana::color_index(*b) as i32 + 1,
                ],
                _ => return None,
            };
            let mut variants: Vec<_> = colors
                .into_iter()
                .map(|mana_color| rv1::FlexPipPayment {
                    pip_index: index as u32,
                    pay_life: false,
                    mana_color,
                })
                .collect();
            variants.push(rv1::FlexPipPayment {
                pip_index: index as u32,
                pay_life: true,
                mana_color: 0,
            });
            Some(variants)
        })
        .collect();
    product(groups)
}
