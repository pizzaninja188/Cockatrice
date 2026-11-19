//! Registry conformance for the queue-v36 team-haste cohort.
use tricerules_cards::primitives::{CreatureScopeController, StaticAbilityDef};
use tricerules_cards::{Keyword, Layout};

struct Card {
    id: &'static str,
    name: &'static str,
    mana_cost: &'static str,
    fingerprint: &'static str,
    types: &'static [&'static str],
    body: Option<(u32, u32)>,
    keywords: &'static [Keyword],
}

const CARDS: &[Card] = &[
    Card {
        id: "cyclops_of_eternal_fury",
        name: "Cyclops of Eternal Fury",
        mana_cost: "{4}{R}{R}",
        fingerprint: "797a896b5ec3eaa874cb6dd73003146fa35c4296e27be2babf0d3f0b33b85ee6",
        types: &["Enchantment", "Creature", "Cyclops"],
        body: Some((5, 3)),
        keywords: &[],
    },
    Card {
        id: "fervor",
        name: "Fervor",
        mana_cost: "{2}{R}",
        fingerprint: "32333d65ec47b47cdca99a8a94b2e51cc9fa65627c87b82297e32a934c553ed4",
        types: &["Enchantment"],
        body: None,
        keywords: &[],
    },
    Card {
        id: "madrush_cyclops",
        name: "Madrush Cyclops",
        mana_cost: "{1}{B}{R}{G}",
        fingerprint: "797a896b5ec3eaa874cb6dd73003146fa35c4296e27be2babf0d3f0b33b85ee6",
        types: &["Creature", "Cyclops", "Warrior"],
        body: Some((3, 4)),
        keywords: &[],
    },
    Card {
        id: "tuktuk_rubblefort",
        name: "Tuktuk Rubblefort",
        mana_cost: "{2}{R}",
        fingerprint: "b6b2174c86049ea452ebd4f4d5aab8ffd3998b875095833c24c1a095c174c7a4",
        types: &["Creature", "Wall"],
        body: Some((0, 3)),
        keywords: &[Keyword::Defender, Keyword::Reach],
    },
];

#[test]
fn q36_team_haste_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for card in CARDS {
        let definition = registry
            .get(card.id)
            .unwrap_or_else(|| panic!("missing {}", card.id));
        assert_eq!(definition.layout, Layout::Normal, "{}", card.id);
        assert_eq!(registry.id_for_name(card.name), Some(card.id));
        let f = definition.primary_face();
        assert_eq!(f.face_id.as_str(), card.id);
        assert_eq!(f.name, card.name);
        assert_eq!(f.mana_cost.to_string(), card.mana_cost);
        assert_eq!(
            f.types.iter().map(String::as_str).collect::<Vec<_>>(),
            card.types.to_vec()
        );
        assert_eq!(
            (f.power, f.toughness),
            card.body
                .map(|(p, t)| (Some(p), Some(t)))
                .unwrap_or((None, None)),
            "{}",
            card.id
        );
        assert_eq!(f.keywords, card.keywords, "{}", card.id);
        assert!(f.activated_abilities.is_empty(), "{}", card.id);
        assert!(f.triggered_abilities.is_empty(), "{}", card.id);
        assert_eq!(f.static_abilities.len(), 1, "{}", card.id);
        assert!(
            matches!(
                &f.static_abilities[0].definition,
                StaticAbilityDef::AnthemKeyword {
                    filter,
                    condition: None,
                    keyword: Keyword::Haste,
                } if filter.controller == Some(CreatureScopeController::YouControl)
                    && filter.subtype.is_none()
                    && !filter.exclude_self
                    && !filter.attacking
            ),
            "{}: creatures you control have haste",
            card.id
        );
        if card.id == "tuktuk_rubblefort" {
            assert!(
                matches!(
                    &f.static_abilities[0].presentation,
                    tricerules_cards::AbilityPresentation::OracleLines(lines) if lines == &[2]
                ),
                "Tuktuk's static ability maps to its second Oracle line"
            );
            assert!(matches!(
                &f.static_abilities[0].definition,
                StaticAbilityDef::AnthemKeyword { .. }
            ));
        }
        assert!(!f
            .static_abilities
            .iter()
            .any(|ability| matches!(&ability.definition, StaticAbilityDef::AnthemPt { .. })));
    }
}

#[test]
fn q36_team_haste_batch_fingerprints_match_the_pinned_oracle_text() {
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
        assert_eq!(fields[2], card.id, "face id drift for {}", card.id);
        assert_eq!(
            fields[4], card.fingerprint,
            "fingerprint drift for {}",
            card.id
        );
    }
}
