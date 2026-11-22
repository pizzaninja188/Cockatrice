#![allow(unused_imports)]
//! Registry conformance for the queue-v34 dies-shrink creatures cohort.
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
        id: "disease_carriers",
        name: "Disease Carriers",
        mana_cost: "{2}{B}{B}",
        face_id: "disease_carriers",
        fingerprint: "b9f3e1c652225f02a15345441b6723933bdf03e58547ce6b0e64a17bd0b565ff",
        types: &["Creature", "Rat"],
        body: Some((2, 2)),
        keywords: &[],
    },
    Card {
        id: "festering_goblin",
        name: "Festering Goblin",
        mana_cost: "{B}",
        face_id: "festering_goblin",
        fingerprint: "de770256e14b46110cdc64e73f76acffc3136c3d8f8684ee299b67fee24ee2a6",
        types: &["Creature", "Zombie", "Goblin"],
        body: Some((1, 1)),
        keywords: &[],
    },
    Card {
        id: "hornet_harasser",
        name: "Hornet Harasser",
        mana_cost: "{2}{B}{B}",
        face_id: "hornet_harasser",
        fingerprint: "b9f3e1c652225f02a15345441b6723933bdf03e58547ce6b0e64a17bd0b565ff",
        types: &["Creature", "Goblin", "Shaman"],
        body: Some((2, 2)),
        keywords: &[],
    },
];

#[test]
fn q34_death_shrink_batch_maps_definitions() {
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
            "disease_carriers" => {
                assert_eq!(f.triggered_abilities.len(), 1);
                let ability = &f.triggered_abilities[0];
                assert_eq!(
                    ability.presentation,
                    AbilityPresentation::OracleLines(vec![1])
                );
                assert!(matches!(ability.trigger, TriggerCondition::WhenSelfDies));
                assert!(matches!(
                    ability.effect.as_slice(),
                    [SpellEffectKind::PumpTarget {
                        power: -2,
                        toughness: -2,
                        subject: EffectSubject::Chosen(_),
                        ..
                    }]
                ));
            }
            "festering_goblin" => {
                assert_eq!(f.triggered_abilities.len(), 1);
                let ability = &f.triggered_abilities[0];
                assert_eq!(
                    ability.presentation,
                    AbilityPresentation::OracleLines(vec![1])
                );
                assert!(matches!(ability.trigger, TriggerCondition::WhenSelfDies));
                assert!(matches!(
                    ability.effect.as_slice(),
                    [SpellEffectKind::PumpTarget {
                        power: -1,
                        toughness: -1,
                        subject: EffectSubject::Chosen(_),
                        ..
                    }]
                ));
            }
            "hornet_harasser" => {
                assert_eq!(f.triggered_abilities.len(), 1);
                let ability = &f.triggered_abilities[0];
                assert_eq!(
                    ability.presentation,
                    AbilityPresentation::OracleLines(vec![1])
                );
                assert!(matches!(ability.trigger, TriggerCondition::WhenSelfDies));
                assert!(matches!(
                    ability.effect.as_slice(),
                    [SpellEffectKind::PumpTarget {
                        power: -2,
                        toughness: -2,
                        subject: EffectSubject::Chosen(_),
                        ..
                    }]
                ));
            }
            other => panic!("unexpected {other}"),
        }
    }
}

#[test]
fn q34_death_shrink_batch_fingerprints_match_the_pinned_oracle_text() {
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
