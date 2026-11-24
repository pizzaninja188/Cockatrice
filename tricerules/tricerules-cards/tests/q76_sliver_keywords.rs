//! Registry conformance for the queue-v76 01-sliver-keyword-anthems cohort.
//!
//! Every card is a normal-layout Sliver whose only rules text grants one keyword to every
//! Sliver creature on the battlefield, including the source. Fingerprints are checked after
//! the presentation metadata refresh.

use tricerules_cards::primitives::{CreatureScopeFilter, Keyword, StaticAbilityDef};
use tricerules_cards::{AbilityPresentation, Layout};

struct Card {
    id: &'static str,
    name: &'static str,
    mana_cost: &'static str,
    power: u32,
    toughness: u32,
    keyword: Keyword,
    fingerprint: &'static str,
}

const CARDS: &[Card] = &[
    Card {
        id: "winged_sliver",
        name: "Winged Sliver",
        mana_cost: "{1}{U}",
        power: 1,
        toughness: 1,
        keyword: Keyword::Flying,
        fingerprint: "a4774403926a690e3e86a7a21051ead0d301644a2ef831dd4b2c8fd5f3141ede",
    },
    Card {
        id: "heart_sliver",
        name: "Heart Sliver",
        mana_cost: "{1}{R}",
        power: 1,
        toughness: 1,
        keyword: Keyword::Haste,
        fingerprint: "3a74172075a57e55444c3bde0b9cdffcced6dc94cdb6556d9ab1d2fbfe4f15df",
    },
    Card {
        id: "reflex_sliver",
        name: "Reflex Sliver",
        mana_cost: "{3}{G}",
        power: 2,
        toughness: 2,
        keyword: Keyword::Haste,
        fingerprint: "3a74172075a57e55444c3bde0b9cdffcced6dc94cdb6556d9ab1d2fbfe4f15df",
    },
    Card {
        id: "battering_sliver",
        name: "Battering Sliver",
        mana_cost: "{5}{R}",
        power: 4,
        toughness: 4,
        keyword: Keyword::Trample,
        fingerprint: "2565f2db003c55b4dcc0b90d51e150d2011226f2b8dd19fcc85158bf61505a51",
    },
    Card {
        id: "horned_sliver",
        name: "Horned Sliver",
        mana_cost: "{2}{G}",
        power: 2,
        toughness: 2,
        keyword: Keyword::Trample,
        fingerprint: "2565f2db003c55b4dcc0b90d51e150d2011226f2b8dd19fcc85158bf61505a51",
    },
    Card {
        id: "spitting_sliver",
        name: "Spitting Sliver",
        mana_cost: "{4}{B}",
        power: 3,
        toughness: 3,
        keyword: Keyword::FirstStrike,
        fingerprint: "2efd1fcbb43ad8aad16c4c4e5f05fe777957ef44fdfa63d21cc95249f5c31876",
    },
    Card {
        id: "talon_sliver",
        name: "Talon Sliver",
        mana_cost: "{1}{W}",
        power: 1,
        toughness: 1,
        keyword: Keyword::FirstStrike,
        fingerprint: "2efd1fcbb43ad8aad16c4c4e5f05fe777957ef44fdfa63d21cc95249f5c31876",
    },
    Card {
        id: "synchronous_sliver",
        name: "Synchronous Sliver",
        mana_cost: "{4}{U}",
        power: 3,
        toughness: 3,
        keyword: Keyword::Vigilance,
        fingerprint: "1311c550a78b96ff3c375be60646c80f57ec9069ef75a200352ea20076c01fae",
    },
    Card {
        id: "fury_sliver",
        name: "Fury Sliver",
        mana_cost: "{5}{R}",
        power: 3,
        toughness: 3,
        keyword: Keyword::DoubleStrike,
        fingerprint: "dec6c73b206fc3e9ed82dc34e52aee3648b575d3dd01c6507c2a9a0a96e4c835",
    },
];

fn face<'a>(
    registry: &'a tricerules_cards::CardRegistry,
    id: &str,
) -> &'a tricerules_cards::CardFace {
    registry
        .get(id)
        .unwrap_or_else(|| panic!("missing {id}"))
        .primary_face()
}

#[test]
fn q76_sliver_keywords_map_definitions() {
    let registry = tricerules_cards::registry::global();
    for card in CARDS {
        let definition = registry
            .get(card.id)
            .unwrap_or_else(|| panic!("missing {}", card.id));
        assert_eq!(definition.layout, Layout::Normal, "{}", card.id);
        assert_eq!(
            registry.id_for_name(card.name),
            Some(card.id),
            "{}",
            card.id
        );
        let f = face(registry, card.id);
        assert_eq!(f.face_id.as_str(), card.id, "{} face id", card.id);
        assert_eq!(f.name, card.name, "{} name", card.id);
        assert_eq!(f.mana_cost.to_string(), card.mana_cost, "{} mana", card.id);
        assert_eq!(f.types, ["Creature", "Sliver"], "{} types", card.id);
        assert_eq!(
            (f.power, f.toughness),
            (Some(card.power), Some(card.toughness)),
            "{} body",
            card.id
        );
        assert!(f.keywords.is_empty(), "{}", card.id);
        assert!(f.triggered_abilities.is_empty(), "{}", card.id);
        assert!(f.activated_abilities.is_empty(), "{}", card.id);
        assert!(f.spell_effect.is_empty(), "{}", card.id);
        assert!(f.targeting.is_none(), "{}", card.id);

        let [entry] = f.static_abilities.as_slice() else {
            panic!("{}: exactly one static ability", card.id);
        };
        assert_eq!(entry.ability_id.as_str(), "static_01", "{}", card.id);
        assert_eq!(
            entry.presentation,
            AbilityPresentation::OracleLines(vec![1]),
            "{}",
            card.id
        );
        let StaticAbilityDef::AnthemKeyword {
            filter,
            condition: None,
            keyword,
        } = &entry.definition
        else {
            panic!("{}: unconditional AnthemKeyword", card.id);
        };
        assert_eq!(
            filter,
            &CreatureScopeFilter {
                subtype: Some("Sliver".to_string()),
                ..Default::default()
            },
            "{}: every Sliver, no controller restriction",
            card.id
        );
        assert_eq!(*keyword, card.keyword, "{}", card.id);
    }
}

#[test]
fn q76_sliver_keywords_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for card in CARDS {
        let row = fingerprints
            .lines()
            .find(|line| line.starts_with(&format!("{}\t", card.id)))
            .unwrap_or_else(|| panic!("missing fingerprint row for {}", card.id));
        let fields: Vec<&str> = row.split('\t').collect();
        assert_eq!(fields.len(), 5, "fingerprint row shape: {row}");
        assert_eq!(fields[0], card.id);
        assert_eq!(fields[1], card.name);
        assert_eq!(fields[2], card.id, "face id drift for {}", card.id);
        assert_eq!(
            fields[4], card.fingerprint,
            "fingerprint drift for {}",
            card.id
        );
    }
}
