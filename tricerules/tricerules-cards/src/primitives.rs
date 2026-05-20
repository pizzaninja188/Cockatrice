//! High-level spell effects referenced by `CardDefinition.spell_effect`.

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpellEffectKind {
    DealDamage { amount: u32 },
    Draw { count: u32 },
    DestroyTarget,
    PumpTarget { power: i32, toughness: i32 },
    CounterTargetSpell,
    GainLife { amount: u32 },
    TargetPlayerGainsLife { amount: u32 },
    TargetOpponentLosesLife { amount: u32 },
    EachOpponentLosesLifeYouGainEqual { amount: u32 },
    ExileTarget,
    ExileTargetGainLifeEqualToPower,
    ReturnTargetCreatureToHand,
    ReturnTargetPermanentToHand,
    MillTargetPlayer { count: u32, opponent_only: bool },
    None,
}

pub fn spell_effect_from_key(key: &str) -> SpellEffectKind {
    match key {
        "bolt" => SpellEffectKind::DealDamage { amount: 3 },
        "growth" => SpellEffectKind::PumpTarget {
            power: 3,
            toughness: 3,
        },
        "divination" => SpellEffectKind::Draw { count: 2 },
        "doom_blade" => SpellEffectKind::DestroyTarget,
        "counterspell" => SpellEffectKind::CounterTargetSpell,
        "healing_salve" => SpellEffectKind::TargetPlayerGainsLife { amount: 3 },
        "angels_mercy" => SpellEffectKind::GainLife { amount: 7 },
        "bump_in_the_night" => SpellEffectKind::TargetOpponentLosesLife { amount: 3 },
        "blood_tithe" => SpellEffectKind::EachOpponentLosesLifeYouGainEqual { amount: 3 },
        "swords_to_plowshares" => SpellEffectKind::ExileTargetGainLifeEqualToPower,
        "eyeblights_ending" => SpellEffectKind::DestroyTarget,
        "unsummon" => SpellEffectKind::ReturnTargetCreatureToHand,
        "boomerang" => SpellEffectKind::ReturnTargetPermanentToHand,
        "tome_scour" => SpellEffectKind::MillTargetPlayer {
            count: 5,
            opponent_only: false,
        },
        "mind_sculpt" => SpellEffectKind::MillTargetPlayer {
            count: 7,
            opponent_only: true,
        },
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
        fn unknown_keys_are_none(s in "[a-z_]{1,30}") {
            const KNOWN: &[&str] = &[
                "bolt",
                "growth",
                "divination",
                "doom_blade",
                "counterspell",
                "healing_salve",
                "angels_mercy",
                "bump_in_the_night",
                "blood_tithe",
                "swords_to_plowshares",
                "eyeblights_ending",
                "unsummon",
                "boomerang",
                "tome_scour",
                "mind_sculpt",
            ];
            prop_assume!(!KNOWN.contains(&s.as_str()));
            prop_assert!(matches!(spell_effect_from_key(&s), SpellEffectKind::None));
        }
    }

    #[test]
    fn damage_amount_matches_bolt() {
        assert!(matches!(
            spell_effect_from_key("bolt"),
            SpellEffectKind::DealDamage { amount: 3 }
        ));
    }
}
