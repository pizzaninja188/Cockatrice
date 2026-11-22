#![allow(unused_imports)]
//! Registry conformance for the queue-v33 upkeep self-damage cohort.
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
        id: "fledgling_djinn",
        name: "Fledgling Djinn",
        mana_cost: "{1}{B}",
        face_id: "fledgling_djinn",
        fingerprint: "7b813915dc7379ba0c0a33c2ab326de153f0c67fbf1940722f55522e2b71f69e",
        types: &["Creature", "Djinn"],
        body: Some((2, 2)),
        keywords: &[Keyword::Flying],
    },
    Card {
        id: "nettletooth_djinn",
        name: "Nettletooth Djinn",
        mana_cost: "{3}{G}",
        face_id: "nettletooth_djinn",
        fingerprint: "4bd471289b4c6e5230a3b849959c11ee49ae528f2a85f597771e458860aa1100",
        types: &["Creature", "Djinn"],
        body: Some((4, 4)),
        keywords: &[],
    },
    Card {
        id: "serendib_efreet",
        name: "Serendib Efreet",
        mana_cost: "{2}{U}",
        face_id: "serendib_efreet",
        fingerprint: "7b813915dc7379ba0c0a33c2ab326de153f0c67fbf1940722f55522e2b71f69e",
        types: &["Creature", "Efreet"],
        body: Some((3, 4)),
        keywords: &[Keyword::Flying],
    },
];

#[test]
fn q33_upkeep_damage_batch_maps_definitions() {
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
            "fledgling_djinn" => {
                assert_eq!(f.triggered_abilities.len(), 1);
                let ability = &f.triggered_abilities[0];
                assert_eq!(
                    ability.presentation,
                    AbilityPresentation::OracleLines(vec![2])
                );
                assert!(matches!(
                    ability.trigger,
                    TriggerCondition::AtBeginningOfUpkeep { .. }
                ));
                assert!(matches!(
                    ability.effect.as_slice(),
                    [SpellEffectKind::DamagePlayer {
                        amount: Amount::Fixed(1),
                        ..
                    }]
                ));
            }
            "nettletooth_djinn" => {
                assert_eq!(f.triggered_abilities.len(), 1);
                let ability = &f.triggered_abilities[0];
                assert_eq!(
                    ability.presentation,
                    AbilityPresentation::OracleLines(vec![1])
                );
                assert!(matches!(
                    ability.trigger,
                    TriggerCondition::AtBeginningOfUpkeep { .. }
                ));
                assert!(matches!(
                    ability.effect.as_slice(),
                    [SpellEffectKind::DamagePlayer {
                        amount: Amount::Fixed(1),
                        ..
                    }]
                ));
            }
            "serendib_efreet" => {
                assert_eq!(f.triggered_abilities.len(), 1);
                let ability = &f.triggered_abilities[0];
                assert_eq!(
                    ability.presentation,
                    AbilityPresentation::OracleLines(vec![2])
                );
                assert!(matches!(
                    ability.trigger,
                    TriggerCondition::AtBeginningOfUpkeep { .. }
                ));
                assert!(matches!(
                    ability.effect.as_slice(),
                    [SpellEffectKind::DamagePlayer {
                        amount: Amount::Fixed(1),
                        ..
                    }]
                ));
            }
            other => panic!("unexpected {other}"),
        }
    }
}

#[test]
fn q33_upkeep_damage_batch_fingerprints_match_the_pinned_oracle_text() {
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
