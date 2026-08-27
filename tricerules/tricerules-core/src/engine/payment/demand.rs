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
    pub fn pay(&self, creatures: &[usize], mana: [u32; 6]) -> Option<Self> {
        let mut result = self.clone();
        for &slot in creatures {
            let amount = result.amounts.get_mut(slot)?;
            *amount = amount.checked_sub(1)?;
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

pub(in crate::engine) fn normalize(
    cost: &ManaCost,
    x: u32,
    increase: u32,
    reduction: u32,
    life_pips: &[rv1::FlexPipPayment],
) -> Result<Vec<Demand>, EngineError> {
    let mut life = HashSet::new();
    for payment in life_pips {
        if !matches!(
            cost.pips.get(payment.pip_index as usize),
            Some(ManaSymbol::Phyrexian(_))
        ) || !life.insert(payment.pip_index as usize)
        {
            return Err(EngineError::Illegal("invalid flexible pip selection"));
        }
    }
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
            ManaSymbol::Phyrexian(_)
                if life_pips
                    .iter()
                    .any(|p| p.pip_index as usize == index && p.pay_life) =>
            {
                vec![(7, 2)]
            }
            ManaSymbol::Phyrexian(c) => vec![(super::mana::color_index(*c), 1)],
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
                pay_life: true
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
