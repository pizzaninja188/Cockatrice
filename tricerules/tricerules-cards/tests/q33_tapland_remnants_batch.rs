#![allow(unused_imports)]
//! Registry conformance for the queue-v33 enters-tapped dual lands cohort.
use tricerules_cards::primitives::{
    AbilityCost, Amount, CardTypeFilter, CastTriggerPlayer, Color, CounterKind,
    CreatureScopeController, EffectSubject, RelativePlayerSet, SpellEffectKind, StaticAbilityDef,
    TriggerCondition,
};
use tricerules_cards::{AbilityPresentation, AbilitySourceZone, Keyword, Layout};

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
        id: "crumbling_necropolis",
        name: "Crumbling Necropolis",
        mana_cost: "",
        face_id: "crumbling_necropolis",
        fingerprint: "2f3da2b211427347ce1e89e9a486598f6007a6c6897d297bc99c9a661e2145dd",
        types: &["Land"],
        body: None,
        keywords: &[],
    },
    Card {
        id: "highland_lake",
        name: "Highland Lake",
        mana_cost: "",
        face_id: "highland_lake",
        fingerprint: "6a7100ee9643d1f284ff8f7ed45e0c477082e74c0619fd256877cadea06cd6df",
        types: &["Land"],
        body: None,
        keywords: &[],
    },
    Card {
        id: "jungle_shrine",
        name: "Jungle Shrine",
        mana_cost: "",
        face_id: "jungle_shrine",
        fingerprint: "76aea621d16245be3085287430df4e4f99552a3d470715ef898af14a52646e0d",
        types: &["Land"],
        body: None,
        keywords: &[],
    },
    Card {
        id: "stone_quarry",
        name: "Stone Quarry",
        mana_cost: "",
        face_id: "stone_quarry",
        fingerprint: "d4aec84051741d694f319bd940388d83dbb54675c13f59c1faf0a6af7199b714",
        types: &["Land"],
        body: None,
        keywords: &[],
    },
];

#[test]
fn q33_tapland_remnants_batch_maps_definitions() {
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
            "crumbling_necropolis" => {
                assert_eq!(f.activated_abilities.len(), 1);
                assert!(
                    matches!(f.activated_abilities[0].effect.as_slice(), [SpellEffectKind::ProduceMana { options, .. }] if options.len() == 3)
                );
                assert_eq!(
                    f.activated_abilities[0].presentation,
                    AbilityPresentation::OracleLines(vec![2])
                );
                assert_eq!(f.static_abilities.len(), 1);
                assert!(matches!(
                    f.static_abilities[0].definition,
                    StaticAbilityDef::EntersTapped { .. }
                ));
                assert_eq!(
                    f.static_abilities[0].presentation,
                    AbilityPresentation::OracleLines(vec![1])
                );
            }
            "highland_lake" => {
                assert_eq!(f.activated_abilities.len(), 1);
                assert!(
                    matches!(f.activated_abilities[0].effect.as_slice(), [SpellEffectKind::ProduceMana { options, .. }] if options.len() == 2)
                );
                assert_eq!(
                    f.activated_abilities[0].presentation,
                    AbilityPresentation::OracleLines(vec![2])
                );
                assert_eq!(f.static_abilities.len(), 1);
                assert!(matches!(
                    f.static_abilities[0].definition,
                    StaticAbilityDef::EntersTapped { .. }
                ));
                assert_eq!(
                    f.static_abilities[0].presentation,
                    AbilityPresentation::OracleLines(vec![1])
                );
            }
            "jungle_shrine" => {
                assert_eq!(f.activated_abilities.len(), 1);
                assert!(
                    matches!(f.activated_abilities[0].effect.as_slice(), [SpellEffectKind::ProduceMana { options, .. }] if options.len() == 3)
                );
                assert_eq!(
                    f.activated_abilities[0].presentation,
                    AbilityPresentation::OracleLines(vec![2])
                );
                assert_eq!(f.static_abilities.len(), 1);
                assert!(matches!(
                    f.static_abilities[0].definition,
                    StaticAbilityDef::EntersTapped { .. }
                ));
                assert_eq!(
                    f.static_abilities[0].presentation,
                    AbilityPresentation::OracleLines(vec![1])
                );
            }
            "stone_quarry" => {
                assert_eq!(f.activated_abilities.len(), 1);
                assert!(
                    matches!(f.activated_abilities[0].effect.as_slice(), [SpellEffectKind::ProduceMana { options, .. }] if options.len() == 2)
                );
                assert_eq!(
                    f.activated_abilities[0].presentation,
                    AbilityPresentation::OracleLines(vec![2])
                );
                assert_eq!(f.static_abilities.len(), 1);
                assert!(matches!(
                    f.static_abilities[0].definition,
                    StaticAbilityDef::EntersTapped { .. }
                ));
                assert_eq!(
                    f.static_abilities[0].presentation,
                    AbilityPresentation::OracleLines(vec![1])
                );
            }
            other => panic!("unexpected {other}"),
        }
    }
}

#[test]
fn q33_tapland_remnants_batch_fingerprints_match_the_pinned_oracle_text() {
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
