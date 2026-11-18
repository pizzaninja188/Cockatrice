#![allow(unused_imports)]
//! Registry conformance for the queue-v34 becomes-blocked self pump cohort.
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
        id: "cave_tiger",
        name: "Cave Tiger",
        mana_cost: "{2}{G}",
        face_id: "cave_tiger",
        fingerprint: "e2d7c62f7af7dd72a5627de47da32d3c0f2d05e42add9ad18c2260294fcf0c87",
        types: &["Creature", "Cat"],
        body: Some((2, 2)),
        keywords: &[],
    },
    Card {
        id: "rabid_wolverines",
        name: "Rabid Wolverines",
        mana_cost: "{3}{G}{G}",
        face_id: "rabid_wolverines",
        fingerprint: "e2d7c62f7af7dd72a5627de47da32d3c0f2d05e42add9ad18c2260294fcf0c87",
        types: &["Creature", "Wolverine"],
        body: Some((4, 4)),
        keywords: &[],
    },
    Card {
        id: "viashino_weaponsmith",
        name: "Viashino Weaponsmith",
        mana_cost: "{3}{R}",
        face_id: "viashino_weaponsmith",
        fingerprint: "703069744361e3f03e612974225cfcda1ef609e52cba066a04f87d5f54a2b48a",
        types: &["Creature", "Lizard"],
        body: Some((2, 2)),
        keywords: &[],
    },
];

#[test]
fn q34_blocked_pump_batch_maps_definitions() {
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
            "cave_tiger" => {
                assert_eq!(f.triggered_abilities.len(), 1);
                let ability = &f.triggered_abilities[0];
                assert_eq!(
                    ability.presentation,
                    AbilityPresentation::OracleLines(vec![1])
                );
                assert!(matches!(
                    ability.trigger,
                    TriggerCondition::WheneverSelfBecomesBlockedByCreature { .. }
                ));
                assert!(matches!(
                    ability.effect.as_slice(),
                    [SpellEffectKind::PumpTarget {
                        power: 1,
                        toughness: 1,
                        subject: EffectSubject::Source,
                        ..
                    }]
                ));
            }
            "rabid_wolverines" => {
                assert_eq!(f.triggered_abilities.len(), 1);
                let ability = &f.triggered_abilities[0];
                assert_eq!(
                    ability.presentation,
                    AbilityPresentation::OracleLines(vec![1])
                );
                assert!(matches!(
                    ability.trigger,
                    TriggerCondition::WheneverSelfBecomesBlockedByCreature { .. }
                ));
                assert!(matches!(
                    ability.effect.as_slice(),
                    [SpellEffectKind::PumpTarget {
                        power: 1,
                        toughness: 1,
                        subject: EffectSubject::Source,
                        ..
                    }]
                ));
            }
            "viashino_weaponsmith" => {
                assert_eq!(f.triggered_abilities.len(), 1);
                let ability = &f.triggered_abilities[0];
                assert_eq!(
                    ability.presentation,
                    AbilityPresentation::OracleLines(vec![1])
                );
                assert!(matches!(
                    ability.trigger,
                    TriggerCondition::WheneverSelfBecomesBlockedByCreature { .. }
                ));
                assert!(matches!(
                    ability.effect.as_slice(),
                    [SpellEffectKind::PumpTarget {
                        power: 2,
                        toughness: 2,
                        subject: EffectSubject::Source,
                        ..
                    }]
                ));
            }
            other => panic!("unexpected {other}"),
        }
    }
}

#[test]
fn q34_blocked_pump_batch_fingerprints_match_the_pinned_oracle_text() {
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
