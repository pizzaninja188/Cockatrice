#![allow(unused_imports)]
//! Registry conformance for the queue-v35 pump and shrink spells cohort.
use tricerules_cards::primitives::{
    AbilityCost, Amount, CombatRestrictionScope, EffectSubject, EntersTappedAffected,
    SpellEffectKind, StaticAbilityDef, TargetKind,
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
        id: "consume_strength",
        name: "Consume Strength",
        mana_cost: "{1}{B}{G}",
        face_id: "consume_strength",
        fingerprint: "e70a2ca40e0d2a5028e9c50e8390a6ff21b57cbe4ad47378eb08d0f009f2f045",
        types: &["Instant"],
        body: None,
        keywords: &[],
    },
    Card {
        id: "leeching_bite",
        name: "Leeching Bite",
        mana_cost: "{1}{G}",
        face_id: "leeching_bite",
        fingerprint: "383aea74d1eed1c80b73f2a8fae0b77cd7a9ee5b4ec79d58f4cd9f31fe9cee81",
        types: &["Instant"],
        body: None,
        keywords: &[],
    },
    Card {
        id: "rites_of_reaping",
        name: "Rites of Reaping",
        mana_cost: "{4}{B}{G}",
        face_id: "rites_of_reaping",
        fingerprint: "78aff52ac96a4aabe0b4dd17fbebf87176c04c7df801517349633ade608c27f6",
        types: &["Sorcery"],
        body: None,
        keywords: &[],
    },
    Card {
        id: "schismotivate",
        name: "Schismotivate",
        mana_cost: "{1}{U}{R}",
        face_id: "schismotivate",
        fingerprint: "d433dd23080676349d32ec3e0fdafb5001abe8e486c0652e128d987b1ce2005d",
        types: &["Instant"],
        body: None,
        keywords: &[],
    },
    Card {
        id: "steal_strength",
        name: "Steal Strength",
        mana_cost: "{1}{B}",
        face_id: "steal_strength",
        fingerprint: "383aea74d1eed1c80b73f2a8fae0b77cd7a9ee5b4ec79d58f4cd9f31fe9cee81",
        types: &["Instant"],
        body: None,
        keywords: &[],
    },
];

#[test]
fn q35_double_pump_shrink_batch_maps_definitions() {
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
            "consume_strength" => {
                assert_eq!(f.spell_effect.len(), 2);
                assert!(matches!(
                    f.spell_effect.as_slice(),
                    [
                        SpellEffectKind::PumpTarget {
                            power: 2,
                            toughness: 2,
                            subject: EffectSubject::Chosen(_),
                            ..
                        },
                        SpellEffectKind::PumpTarget {
                            power: -2,
                            toughness: -2,
                            subject: EffectSubject::Chosen(_),
                            ..
                        }
                    ]
                ));
                let groups = &f.targeting.as_ref().expect("two target groups").groups;
                assert_eq!(groups.len(), 2);
                assert_eq!((groups[0].min, groups[0].max), (1, 1));
                assert_eq!(groups[0].effect_indices, [0]);
                assert_eq!((groups[1].min, groups[1].max), (1, 1));
                assert_eq!(groups[1].effect_indices, [1]);
                assert_eq!(groups[1].distinct_from, [0]);
            }
            "leeching_bite" => {
                assert_eq!(f.spell_effect.len(), 2);
                assert!(matches!(
                    f.spell_effect.as_slice(),
                    [
                        SpellEffectKind::PumpTarget {
                            power: 1,
                            toughness: 1,
                            subject: EffectSubject::Chosen(_),
                            ..
                        },
                        SpellEffectKind::PumpTarget {
                            power: -1,
                            toughness: -1,
                            subject: EffectSubject::Chosen(_),
                            ..
                        }
                    ]
                ));
                let groups = &f.targeting.as_ref().expect("two target groups").groups;
                assert_eq!(groups.len(), 2);
                assert_eq!((groups[0].min, groups[0].max), (1, 1));
                assert_eq!(groups[0].effect_indices, [0]);
                assert_eq!((groups[1].min, groups[1].max), (1, 1));
                assert_eq!(groups[1].effect_indices, [1]);
                assert_eq!(groups[1].distinct_from, [0]);
            }
            "rites_of_reaping" => {
                assert_eq!(f.spell_effect.len(), 2);
                assert!(matches!(
                    f.spell_effect.as_slice(),
                    [
                        SpellEffectKind::PumpTarget {
                            power: 3,
                            toughness: 3,
                            subject: EffectSubject::Chosen(_),
                            ..
                        },
                        SpellEffectKind::PumpTarget {
                            power: -3,
                            toughness: -3,
                            subject: EffectSubject::Chosen(_),
                            ..
                        }
                    ]
                ));
                let groups = &f.targeting.as_ref().expect("two target groups").groups;
                assert_eq!(groups.len(), 2);
                assert_eq!((groups[0].min, groups[0].max), (1, 1));
                assert_eq!(groups[0].effect_indices, [0]);
                assert_eq!((groups[1].min, groups[1].max), (1, 1));
                assert_eq!(groups[1].effect_indices, [1]);
                assert_eq!(groups[1].distinct_from, [0]);
            }
            "schismotivate" => {
                assert_eq!(f.spell_effect.len(), 2);
                assert!(matches!(
                    f.spell_effect.as_slice(),
                    [
                        SpellEffectKind::PumpTarget {
                            power: 4,
                            toughness: 0,
                            subject: EffectSubject::Chosen(_),
                            ..
                        },
                        SpellEffectKind::PumpTarget {
                            power: -4,
                            toughness: -0,
                            subject: EffectSubject::Chosen(_),
                            ..
                        }
                    ]
                ));
                let groups = &f.targeting.as_ref().expect("two target groups").groups;
                assert_eq!(groups.len(), 2);
                assert_eq!((groups[0].min, groups[0].max), (1, 1));
                assert_eq!(groups[0].effect_indices, [0]);
                assert_eq!((groups[1].min, groups[1].max), (1, 1));
                assert_eq!(groups[1].effect_indices, [1]);
                assert_eq!(groups[1].distinct_from, [0]);
            }
            "steal_strength" => {
                assert_eq!(f.spell_effect.len(), 2);
                assert!(matches!(
                    f.spell_effect.as_slice(),
                    [
                        SpellEffectKind::PumpTarget {
                            power: 1,
                            toughness: 1,
                            subject: EffectSubject::Chosen(_),
                            ..
                        },
                        SpellEffectKind::PumpTarget {
                            power: -1,
                            toughness: -1,
                            subject: EffectSubject::Chosen(_),
                            ..
                        }
                    ]
                ));
                let groups = &f.targeting.as_ref().expect("two target groups").groups;
                assert_eq!(groups.len(), 2);
                assert_eq!((groups[0].min, groups[0].max), (1, 1));
                assert_eq!(groups[0].effect_indices, [0]);
                assert_eq!((groups[1].min, groups[1].max), (1, 1));
                assert_eq!(groups[1].effect_indices, [1]);
                assert_eq!(groups[1].distinct_from, [0]);
            }
            other => panic!("unexpected {other}"),
        }
    }
}

#[test]
fn q35_double_pump_shrink_batch_fingerprints_match_the_pinned_oracle_text() {
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
