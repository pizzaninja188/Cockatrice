#![allow(unused_imports)]
//! Registry conformance for the queue-v35 up-to-two target pumps cohort.
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
        id: "dauntless_onslaught",
        name: "Dauntless Onslaught",
        mana_cost: "{2}{W}",
        face_id: "dauntless_onslaught",
        fingerprint: "a4f549c3fa06cf57f3cd6933c6758b45d79b599b325b6098247c6af7cce2deb0",
        types: &["Instant"],
        body: None,
        keywords: &[],
    },
    Card {
        id: "mischief_and_mayhem",
        name: "Mischief and Mayhem",
        mana_cost: "{4}{G}",
        face_id: "mischief_and_mayhem",
        fingerprint: "082acfb23815ff09edb1b1567cc40f36850ce5d4d6e538eefb50c0b4c105da13",
        types: &["Sorcery"],
        body: None,
        keywords: &[],
    },
    Card {
        id: "nahiris_stoneblades",
        name: "Nahiri's Stoneblades",
        mana_cost: "{1}{R}",
        face_id: "nahiris_stoneblades",
        fingerprint: "2831453b375bbf3910c999eec9418ba4d6693fe6e362b8d73b85b9acd6803ac4",
        types: &["Instant"],
        body: None,
        keywords: &[],
    },
];

#[test]
fn q35_two_target_pump_batch_maps_definitions() {
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
            "dauntless_onslaught" => {
                assert_eq!(f.spell_effect.len(), 1);
                assert!(matches!(
                    f.spell_effect.as_slice(),
                    [SpellEffectKind::PumpTarget {
                        power: 2,
                        toughness: 2,
                        subject: EffectSubject::Chosen(_),
                        ..
                    }]
                ));
                let groups = &f.targeting.as_ref().expect("one target group").groups;
                assert_eq!(groups.len(), 1);
                assert_eq!((groups[0].min, groups[0].max), (0, 2));
                assert_eq!(groups[0].effect_indices, [0]);
            }
            "mischief_and_mayhem" => {
                assert_eq!(f.spell_effect.len(), 1);
                assert!(matches!(
                    f.spell_effect.as_slice(),
                    [SpellEffectKind::PumpTarget {
                        power: 4,
                        toughness: 4,
                        subject: EffectSubject::Chosen(_),
                        ..
                    }]
                ));
                let groups = &f.targeting.as_ref().expect("one target group").groups;
                assert_eq!(groups.len(), 1);
                assert_eq!((groups[0].min, groups[0].max), (0, 2));
                assert_eq!(groups[0].effect_indices, [0]);
            }
            "nahiris_stoneblades" => {
                assert_eq!(f.spell_effect.len(), 1);
                assert!(matches!(
                    f.spell_effect.as_slice(),
                    [SpellEffectKind::PumpTarget {
                        power: 2,
                        toughness: 0,
                        subject: EffectSubject::Chosen(_),
                        ..
                    }]
                ));
                let groups = &f.targeting.as_ref().expect("one target group").groups;
                assert_eq!(groups.len(), 1);
                assert_eq!((groups[0].min, groups[0].max), (0, 2));
                assert_eq!(groups[0].effect_indices, [0]);
            }
            other => panic!("unexpected {other}"),
        }
    }
}

#[test]
fn q35_two_target_pump_batch_fingerprints_match_the_pinned_oracle_text() {
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
