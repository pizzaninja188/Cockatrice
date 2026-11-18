#![allow(unused_imports)]
//! Registry conformance for the queue-v34 power-threshold evasion cohort.
use tricerules_cards::primitives::{
    AbilityCost, Amount, CombatRole, EffectSubject, GameCondition, LifeAmount, PlayerRecipient,
    PowerComparison, RelativePlayerSet, SpellEffectKind, StaticAbilityDef, TargetKind,
    TriggerCondition,
};
use tricerules_cards::{AbilityPresentation, Keyword, Layout};

struct Card {
    id: &'static str,
    name: &'static str,
    mana_cost: &'static str,
    face_id: &'static str,
    fingerprint: &'static str,
    types: &'static [&'static str],
    body: Option<(u32, u32)>,
    keywords: &'static [Keyword],
}

const CARDS: &[Card] = &[
    Card {
        id: "amrou_kithkin",
        name: "Amrou Kithkin",
        mana_cost: "{W}{W}",
        face_id: "amrou_kithkin",
        fingerprint: "1adc02f96b131ba1df4c7e8acf483a60d748fe3429fc160bcfbcc06c0a096f8b",
        types: &["Creature", "Kithkin"],
        body: Some((1, 1)),
        keywords: &[],
    },
    Card {
        id: "duskmantle_operative",
        name: "Duskmantle Operative",
        mana_cost: "{1}{B}",
        face_id: "duskmantle_operative",
        fingerprint: "03a98a05f631a8ed704067eb4252ee3b78f831eaec6d9339eecf868bebc08b44",
        types: &["Creature", "Human", "Rogue"],
        body: Some((2, 2)),
        keywords: &[],
    },
    Card {
        id: "fleet-footed_monk",
        name: "Fleet-Footed Monk",
        mana_cost: "{1}{W}",
        face_id: "fleet_footed_monk",
        fingerprint: "a1bf4a674bf1466a0c14380c20b5c3ad2a0bf992f99a625cf1ba28a7d1964f47",
        types: &["Creature", "Human", "Monk"],
        body: Some((1, 1)),
        keywords: &[],
    },
    Card {
        id: "goldmeadow_dodger",
        name: "Goldmeadow Dodger",
        mana_cost: "{W}",
        face_id: "goldmeadow_dodger",
        fingerprint: "03a98a05f631a8ed704067eb4252ee3b78f831eaec6d9339eecf868bebc08b44",
        types: &["Creature", "Kithkin", "Rogue"],
        body: Some((1, 1)),
        keywords: &[],
    },
];

#[test]
fn q34_power_evasion_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for card in CARDS {
        let definition = registry
            .get(card.id)
            .unwrap_or_else(|| panic!("missing {}", card.id));
        assert_eq!(definition.layout, Layout::Normal, "{}", card.id);
        assert_eq!(registry.id_for_name(card.name), Some(card.id));
        let f = definition.primary_face();
        assert_eq!(f.face_id.as_str(), card.face_id);
        assert_eq!(f.name, card.name);
        assert_eq!(f.mana_cost.to_string(), card.mana_cost);
        assert_eq!(
            f.types.iter().map(String::as_str).collect::<Vec<_>>(),
            card.types.to_vec()
        );
        match card.body {
            Some((power, toughness)) => {
                assert_eq!((f.power, f.toughness), (Some(power), Some(toughness)))
            }
            None => assert_eq!((f.power, f.toughness), (None, None)),
        }
        assert_eq!(f.keywords, card.keywords);
        match card.id {
            "amrou_kithkin" => {
                assert_eq!(f.static_abilities.len(), 1);
                assert_eq!(
                    f.static_abilities[0].presentation,
                    AbilityPresentation::OracleLines(vec![1])
                );
                assert!(
                    matches!(&f.static_abilities[0].definition, StaticAbilityDef::SelfCombatRestriction { restriction, .. } if matches!(restriction.cant_be_blocked_by.as_slice(), [filter] if filter.power == Some(PowerComparison::AtLeast(3))))
                );
            }
            "duskmantle_operative" => {
                assert_eq!(f.static_abilities.len(), 1);
                assert_eq!(
                    f.static_abilities[0].presentation,
                    AbilityPresentation::OracleLines(vec![1])
                );
                assert!(
                    matches!(&f.static_abilities[0].definition, StaticAbilityDef::SelfCombatRestriction { restriction, .. } if matches!(restriction.cant_be_blocked_by.as_slice(), [filter] if filter.power == Some(PowerComparison::AtLeast(4))))
                );
            }
            "fleet-footed_monk" => {
                assert_eq!(f.static_abilities.len(), 1);
                assert_eq!(
                    f.static_abilities[0].presentation,
                    AbilityPresentation::OracleLines(vec![1])
                );
                assert!(
                    matches!(&f.static_abilities[0].definition, StaticAbilityDef::SelfCombatRestriction { restriction, .. } if matches!(restriction.cant_be_blocked_by.as_slice(), [filter] if filter.power == Some(PowerComparison::AtLeast(2))))
                );
            }
            "goldmeadow_dodger" => {
                assert_eq!(f.static_abilities.len(), 1);
                assert_eq!(
                    f.static_abilities[0].presentation,
                    AbilityPresentation::OracleLines(vec![1])
                );
                assert!(
                    matches!(&f.static_abilities[0].definition, StaticAbilityDef::SelfCombatRestriction { restriction, .. } if matches!(restriction.cant_be_blocked_by.as_slice(), [filter] if filter.power == Some(PowerComparison::AtLeast(4))))
                );
            }
            other => panic!("unexpected {other}"),
        }
    }
}

#[test]
fn q34_power_evasion_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for card in CARDS {
        let row = fingerprints
            .lines()
            .find(|line| line.starts_with(&format!("{}\t", card.id)))
            .unwrap_or_else(|| panic!("missing fingerprint row for {}", card.id));
        let fields: Vec<&str> = row.split('\t').collect();
        assert_eq!(fields.len(), 5, "{}", card.id);
        assert_eq!(fields[0], card.id);
        assert_eq!(fields[1], card.name);
        assert_eq!(fields[2], card.face_id, "face id drift for {}", card.id);
        assert_eq!(
            fields[4], card.fingerprint,
            "fingerprint drift for {}",
            card.id
        );
    }
}
