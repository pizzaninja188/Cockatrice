//! High-level spell effects referenced by `CardDefinition.spell_effect`.

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpellEffectKind {
    DealDamage { amount: u32 },
    Draw { count: u32 },
    DestroyTarget,
    PumpTarget { power: i32, toughness: i32 },
    CounterTargetSpell,
    None,
}

pub fn spell_effect_from_key(key: &str) -> SpellEffectKind {
    match key {
        "bolt" => SpellEffectKind::DealDamage { amount: 3 },
        "growth" => SpellEffectKind::PumpTarget { power: 3, toughness: 3 },
        "divination" => SpellEffectKind::Draw { count: 2 },
        "doom_blade" => SpellEffectKind::DestroyTarget,
        "counterspell" => SpellEffectKind::CounterTargetSpell,
        _ => SpellEffectKind::None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn maps_known_keys() {
        assert!(matches!(
            spell_effect_from_key("bolt"),
            SpellEffectKind::DealDamage { amount: 3 }
        ));
    }

    proptest! {
        #[test]
        fn unknown_keys_are_none(s in "[a-z]{1,20}") {
            prop_assume!(s != "bolt" && s != "growth" && s != "divination" && s != "doom_blade" && s != "counterspell");
            prop_assert!(matches!(spell_effect_from_key(&s), SpellEffectKind::None));
        }
    }
}
