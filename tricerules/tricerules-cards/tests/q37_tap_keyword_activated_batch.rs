//! Registry conformance for the queue-v37 09-tap-keyword-activated cohort.
//! Printed identity, costs, typed shape and pinned fingerprints.

use tricerules_cards::primitives::{AbilityCost, EffectSubject, SpellEffectKind, TargetKind};
use tricerules_cards::{AbilityPresentation, AbilitySourceZone, Keyword, Layout};

struct Card {
    id: &'static str,
    name: &'static str,
    mana_cost: &'static str,
    types: &'static [&'static str],
    power: Option<u32>,
    toughness: Option<u32>,
    cost: &'static str,
    keywords: &'static [Keyword],
    fingerprint: &'static str,
}

const CARDS: &[Card] = &[
    Card {
        id: "flying_carpet",
        name: "Flying Carpet",
        mana_cost: "{4}",
        types: &["Artifact"],
        power: None,
        toughness: None,
        cost: "{2}",
        keywords: &[Keyword::Flying],
        fingerprint: "667ae43c48f13dd61148c228689cd4913c271d5c5101e151ff51575928794fc2",
    },
    Card {
        id: "caller_of_gales",
        name: "Caller of Gales",
        mana_cost: "{U}",
        types: &["Creature", "Merfolk", "Wizard"],
        power: Some(1),
        toughness: Some(1),
        cost: "{1}{U}",
        keywords: &[Keyword::Flying],
        fingerprint: "9a03c890cc57ed3428ce8f7f136de37360cb9f821cc51be6e0c4459d03a26ebb",
    },
    Card {
        id: "war_chariot",
        name: "War Chariot",
        mana_cost: "{3}",
        types: &["Artifact"],
        power: None,
        toughness: None,
        cost: "{3}",
        keywords: &[Keyword::Trample],
        fingerprint: "7e62c93fa49aa6623af66e87cd153f019bf930f2ae12aaaafab84225b3e2c061",
    },
];

#[test]
fn q37_tap_keyword_activated_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for card in CARDS {
        let definition = registry
            .get(card.id)
            .unwrap_or_else(|| panic!("missing {}", card.id));
        assert_eq!(definition.layout, Layout::Normal, "{}", card.id);
        assert_eq!(registry.id_for_name(card.name), Some(card.id));
        let f = definition.primary_face();
        assert_eq!(f.name, card.name);
        assert_eq!(f.mana_cost.to_string(), card.mana_cost);
        assert_eq!(
            f.types.iter().map(String::as_str).collect::<Vec<_>>(),
            card.types
        );
        assert_eq!((f.power, f.toughness), (card.power, card.toughness));
        assert!(f.keywords.is_empty(), "{}: no printed keyword", card.id);
        assert!(f.static_abilities.is_empty(), "{}", card.id);
        assert!(f.triggered_abilities.is_empty(), "{}", card.id);
        let [ability] = f.activated_abilities.as_slice() else {
            panic!("{}: exactly one activated ability", card.id);
        };
        assert_eq!(ability.ability_id.as_str(), "activated_01");
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![1])
        );
        assert_eq!(ability.source_zone, AbilitySourceZone::Battlefield);
        assert!(
            matches!(
                ability.costs.as_slice(),
                [AbilityCost::Mana(mana), AbilityCost::Tap] if mana.to_string() == card.cost
            ),
            "{}: printed mana and tap costs",
            card.id
        );
        assert!(
            matches!(
                ability.effect.as_slice(),
                [SpellEffectKind::GrantKeywords { subject, keywords }]
                    if matches!(subject, EffectSubject::Chosen(filter)
                        if filter.kind == TargetKind::Creature)
                        && keywords == card.keywords
            ),
            "{}: targeted keyword grant",
            card.id
        );
        let targeting = ability.targeting.as_ref().expect("targeting");
        assert_eq!(targeting.groups.len(), 1);
        assert_eq!((targeting.groups[0].min, targeting.groups[0].max), (1, 1));
        assert_eq!(targeting.groups[0].effect_indices, [0]);
    }
}

#[test]
fn q37_tap_keyword_activated_batch_fingerprints_match_the_pinned_oracle_text() {
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
        assert_eq!(fields[2], card.id.replace('-', "_"));
        assert_eq!(
            fields[4], card.fingerprint,
            "fingerprint drift for {}",
            card.id
        );
    }
}
