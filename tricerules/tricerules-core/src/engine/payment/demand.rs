//! CR 601.2f and 702.51: normalized spell demands, shared by previews and Convoke commits.
//! Unexpected Assistance and Merrow Skyswimmer require the same colored/hybrid payment rules.
use super::super::*;

#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub(in crate::engine) struct Demand {
    /// W, U, B, R, G, C, generic. Generic is never a mana-pool slot.
    pub amounts: [u32; 7],
    pub life: u32,
}

impl Demand {
    #[cfg(test)]
    pub fn pay(&self, creatures: &[usize], mana: [u32; 6]) -> Option<Self> {
        self.pay_with_any_color(creatures, mana, false)
    }

    pub fn pay_with_any_color(
        &self,
        creatures: &[usize],
        mana: [u32; 6],
        as_any_color: bool,
    ) -> Option<Self> {
        let mut result = self.clone();
        for &slot in creatures {
            let amount = result.amounts.get_mut(slot)?;
            *amount = amount.checked_sub(1)?;
        }
        if as_any_color {
            // Actual colorless mana is the only resource that can satisfy {C}. Reserve it first;
            // all remaining actual mana can satisfy colored or generic pips. Paying colored
            // demands before generic leaves the most flexible residual for later Convoke taps.
            let true_colorless = mana[5].min(result.amounts[5]);
            result.amounts[5] -= true_colorless;
            let mut remaining =
                mana.into_iter().map(u64::from).sum::<u64>() - u64::from(true_colorless);
            for demand in &mut result.amounts[..5] {
                let used = remaining.min(u64::from(*demand)) as u32;
                *demand -= used;
                remaining -= u64::from(used);
            }
            result.amounts[6] = result.amounts[6].checked_sub(u32::try_from(remaining).ok()?)?;
            return Some(result);
        }
        for (slot, amount) in mana.into_iter().enumerate() {
            let colored = amount.min(result.amounts[slot]);
            result.amounts[slot] -= colored;
            result.amounts[6] = result.amounts[6].checked_sub(amount - colored)?;
        }
        Some(result)
    }

    pub fn complete(&self) -> bool {
        self.amounts.iter().all(|n| *n == 0)
    }

    pub fn label(&self) -> String {
        let mut text = String::new();
        if self.amounts[6] > 0 {
            text.push_str(&format!("{{{}}}", self.amounts[6]));
        }
        for (symbol, amount) in ["W", "U", "B", "R", "G", "C"].into_iter().zip(self.amounts) {
            // Display huge costs compactly; never allocate proportional to a client-supplied X.
            if amount > 20 {
                text.push_str(&format!("{amount} x {{{symbol}}}"));
            } else {
                for _ in 0..amount {
                    text.push_str(&format!("{{{symbol}}}"));
                }
            }
        }
        if text.is_empty() {
            text.push_str("{0}");
        }
        if self.life > 0 {
            text.push_str(&format!(" + {} life", self.life));
        }
        text
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::engine) enum PhyrexianPayment {
    Mana(ColorPip),
    Life,
}

/// Resolve only declared Phyrexian branches against the active indexed cost. Ordinary
/// mono-Phyrexian omission retains its legacy mana branch; hybrid Phyrexian is explicit.
pub(in crate::engine) fn validated_phyrexian_choices(
    cost: &ManaCost,
    choices: &[rv1::FlexPipPayment],
) -> Result<Vec<Option<PhyrexianPayment>>, EngineError> {
    let mut selected = vec![None; cost.pips.len()];
    for choice in choices {
        let index = choice.pip_index as usize;
        let Some(pip) = cost.pips.get(index) else {
            return Err(EngineError::Illegal("invalid flexible pip selection"));
        };
        if selected[index].is_some() {
            return Err(EngineError::Illegal("duplicate flexible pip selection"));
        }
        selected[index] = Some(match pip {
            ManaSymbol::Phyrexian(color) if choice.mana_color == 0 => {
                if choice.pay_life {
                    PhyrexianPayment::Life
                } else {
                    PhyrexianPayment::Mana(*color)
                }
            }
            ManaSymbol::HybridPhyrexian(_, _) if choice.pay_life && choice.mana_color == 0 => {
                PhyrexianPayment::Life
            }
            ManaSymbol::HybridPhyrexian(a, b) if !choice.pay_life => {
                let color = match rv1::ManaColor::try_from(choice.mana_color).ok() {
                    Some(rv1::ManaColor::White) => ColorPip::W,
                    Some(rv1::ManaColor::Blue) => ColorPip::U,
                    Some(rv1::ManaColor::Black) => ColorPip::B,
                    Some(rv1::ManaColor::Red) => ColorPip::R,
                    Some(rv1::ManaColor::Green) => ColorPip::G,
                    _ => return Err(EngineError::Illegal("invalid hybrid Phyrexian color")),
                };
                if color != *a && color != *b {
                    return Err(EngineError::Illegal(
                        "hybrid Phyrexian color is not a component",
                    ));
                }
                PhyrexianPayment::Mana(color)
            }
            _ => return Err(EngineError::Illegal("invalid flexible pip selection")),
        });
    }
    for (index, pip) in cost.pips.iter().enumerate() {
        match pip {
            ManaSymbol::HybridPhyrexian(_, _) if selected[index].is_none() => {
                return Err(EngineError::Illegal(
                    "hybrid Phyrexian pip needs an announced branch",
                ));
            }
            ManaSymbol::Phyrexian(color) if selected[index].is_none() => {
                selected[index] = Some(PhyrexianPayment::Mana(*color));
            }
            _ => {}
        }
    }
    Ok(selected)
}

pub(in crate::engine) fn normalize(
    cost: &ManaCost,
    x: u32,
    increase: u32,
    reduction: u32,
    life_pips: &[rv1::FlexPipPayment],
) -> Result<Vec<Demand>, EngineError> {
    let phyrexian = validated_phyrexian_choices(cost, life_pips)?;
    let mut demands = vec![Demand {
        amounts: [0, 0, 0, 0, 0, 0, increase],
        life: 0,
    }];
    for (index, pip) in cost.pips.iter().enumerate() {
        let options: Vec<(usize, u32)> = match pip {
            ManaSymbol::W => vec![(0, 1)],
            ManaSymbol::U => vec![(1, 1)],
            ManaSymbol::B => vec![(2, 1)],
            ManaSymbol::R => vec![(3, 1)],
            ManaSymbol::G => vec![(4, 1)],
            ManaSymbol::C => vec![(5, 1)],
            ManaSymbol::Generic(n) => vec![(6, *n)],
            ManaSymbol::X => vec![(6, x)],
            ManaSymbol::Hybrid(a, b) => vec![
                (super::mana::color_index(*a), 1),
                (super::mana::color_index(*b), 1),
            ],
            ManaSymbol::MonoHybrid(n, c) => vec![(super::mana::color_index(*c), 1), (6, *n)],
            ManaSymbol::Phyrexian(_) | ManaSymbol::HybridPhyrexian(_, _) => {
                match phyrexian[index].expect("validated Phyrexian choice") {
                    PhyrexianPayment::Life => vec![(7, 2)],
                    PhyrexianPayment::Mana(color) => vec![(super::mana::color_index(color), 1)],
                }
            }
        };
        let mut next = std::collections::BTreeSet::new();
        for demand in demands {
            for &(slot, amount) in &options {
                let mut d = demand.clone();
                let target = if slot == 7 {
                    &mut d.life
                } else {
                    &mut d.amounts[slot]
                };
                *target = target
                    .checked_add(amount)
                    .ok_or(EngineError::Illegal("spell cost overflow"))?;
                next.insert(d);
            }
        }
        demands = next.into_iter().collect();
    }
    for demand in &mut demands {
        demand.amounts[6] = demand.amounts[6].saturating_sub(reduction);
    }
    demands.sort();
    demands.dedup();
    Ok(demands)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn convoke_normalizes_hybrid_before_reductions_and_keeps_colorless_distinct() {
        let variants = normalize(&ManaCost::parse("{X}{2/W}{C}").unwrap(), 2, 1, 4, &[])
            .expect("engine-calculated demands");
        assert!(variants.iter().any(|d| d.amounts == [1, 0, 0, 0, 0, 1, 0]));
        assert!(variants.iter().any(|d| d.amounts == [0, 0, 0, 0, 0, 1, 1]));
        assert_eq!(variants.len(), 2);
    }

    #[test]
    fn convoke_phyrexian_life_is_not_a_creature_payment() {
        let cost = ManaCost::parse("{1}{U/P}").unwrap();
        let demand = normalize(
            &cost,
            0,
            0,
            0,
            &[rv1::FlexPipPayment {
                pip_index: 1,
                pay_life: true,
                mana_color: 0,
            }],
        )
        .expect("life selection");
        assert_eq!(demand[0].life, 2);
        assert_eq!(demand[0].amounts, [0, 0, 0, 0, 0, 0, 1]);
        assert!(normalize(
            &cost,
            0,
            0,
            0,
            &[rv1::FlexPipPayment {
                pip_index: 0,
                pay_life: true,
                mana_color: 0,
            }]
        )
        .is_err());
    }

    #[test]
    fn convoke_demand_matches_hybrid_x_and_modifier_payment_without_mana_creation() {
        let variants = normalize(&ManaCost::parse("{X}{W/U}{C}").unwrap(), 3, 2, 4, &[]).unwrap();
        assert!(variants.iter().any(|d| d
            .pay(&[0, 6], [0, 0, 0, 0, 0, 1])
            .is_some_and(|r| r.complete())));
        assert!(variants.iter().any(|d| d
            .pay(&[1], [0, 0, 0, 0, 0, 2])
            .is_some_and(|r| r.complete())));
        assert!(
            variants.iter().all(|d| d.pay(&[0, 6, 6], [0; 6]).is_none()),
            "generic creatures never pay explicit C"
        );
        assert!(
            variants.iter().all(|d| d.pay(&[4], [0; 6]).is_none()),
            "green cannot pay W/U"
        );
        assert!(normalize(&ManaCost::parse("{X}{1}").unwrap(), u32::MAX, 0, 0, &[]).is_err());
    }
}
