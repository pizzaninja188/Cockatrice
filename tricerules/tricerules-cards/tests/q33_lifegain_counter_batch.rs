#![allow(unused_imports)]
//! Registry conformance for the queue-v33 life-gain +1/+1 counters cohort.
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
        id: "blood_researcher",
        name: "Blood Researcher",
        mana_cost: "{1}{B}{G}",
        face_id: "blood_researcher",
        fingerprint: "7e045a10d20629da80dccb9543a6ab29719b2c6264751b7d37f52272ddef8b50",
        types: &["Creature", "Vampire", "Druid"],
        body: Some((2, 2)),
        keywords: &[Keyword::Menace],
    },
    Card {
        id: "bloodbond_vampire",
        name: "Bloodbond Vampire",
        mana_cost: "{2}{B}{B}",
        face_id: "bloodbond_vampire",
        fingerprint: "9427fa8386fa895447709324a606bb01e0ad3b7409e396a0fe279340060e2afb",
        types: &["Creature", "Vampire", "Shaman", "Ally"],
        body: Some((3, 3)),
        keywords: &[],
    },
    Card {
        id: "celestial_unicorn",
        name: "Celestial Unicorn",
        mana_cost: "{2}{W}",
        face_id: "celestial_unicorn",
        fingerprint: "9427fa8386fa895447709324a606bb01e0ad3b7409e396a0fe279340060e2afb",
        types: &["Creature", "Unicorn"],
        body: Some((3, 2)),
        keywords: &[],
    },
    Card {
        id: "scion_of_the_swarm",
        name: "Scion of the Swarm",
        mana_cost: "{3}{B}{B}",
        face_id: "scion_of_the_swarm",
        fingerprint: "deb89b0ca3e60cb89f1898a6454d18fec2c7d029bb38cfe9f13d11c0f5bd2825",
        types: &["Creature", "Vampire", "Cleric"],
        body: Some((3, 3)),
        keywords: &[Keyword::Flying],
    },
];

#[test]
fn q33_lifegain_counter_batch_maps_definitions() {
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
            "blood_researcher" => {
                assert_eq!(f.triggered_abilities.len(), 1);
                let ability = &f.triggered_abilities[0];
                assert_eq!(
                    ability.presentation,
                    AbilityPresentation::OracleLines(vec![2])
                );
                assert!(matches!(
                    ability.trigger,
                    TriggerCondition::WheneverPlayerGainsLife { .. }
                ));
                assert!(matches!(
                    ability.effect.as_slice(),
                    [SpellEffectKind::PutCounters {
                        counter: CounterKind::PlusOnePlusOne,
                        count: Amount::Fixed(1),
                        subject: EffectSubject::Source
                    }]
                ));
            }
            "bloodbond_vampire" => {
                assert_eq!(f.triggered_abilities.len(), 1);
                let ability = &f.triggered_abilities[0];
                assert_eq!(
                    ability.presentation,
                    AbilityPresentation::OracleLines(vec![1])
                );
                assert!(matches!(
                    ability.trigger,
                    TriggerCondition::WheneverPlayerGainsLife { .. }
                ));
                assert!(matches!(
                    ability.effect.as_slice(),
                    [SpellEffectKind::PutCounters {
                        counter: CounterKind::PlusOnePlusOne,
                        count: Amount::Fixed(1),
                        subject: EffectSubject::Source
                    }]
                ));
            }
            "celestial_unicorn" => {
                assert_eq!(f.triggered_abilities.len(), 1);
                let ability = &f.triggered_abilities[0];
                assert_eq!(
                    ability.presentation,
                    AbilityPresentation::OracleLines(vec![1])
                );
                assert!(matches!(
                    ability.trigger,
                    TriggerCondition::WheneverPlayerGainsLife { .. }
                ));
                assert!(matches!(
                    ability.effect.as_slice(),
                    [SpellEffectKind::PutCounters {
                        counter: CounterKind::PlusOnePlusOne,
                        count: Amount::Fixed(1),
                        subject: EffectSubject::Source
                    }]
                ));
            }
            "scion_of_the_swarm" => {
                assert_eq!(f.triggered_abilities.len(), 1);
                let ability = &f.triggered_abilities[0];
                assert_eq!(
                    ability.presentation,
                    AbilityPresentation::OracleLines(vec![2])
                );
                assert!(matches!(
                    ability.trigger,
                    TriggerCondition::WheneverPlayerGainsLife { .. }
                ));
                assert!(matches!(
                    ability.effect.as_slice(),
                    [SpellEffectKind::PutCounters {
                        counter: CounterKind::PlusOnePlusOne,
                        count: Amount::Fixed(1),
                        subject: EffectSubject::Source
                    }]
                ));
            }
            other => panic!("unexpected {other}"),
        }
    }
}

#[test]
fn q33_lifegain_counter_batch_fingerprints_match_the_pinned_oracle_text() {
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
