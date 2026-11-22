//! Registry conformance for the queue-v26 unconditional other-creature anthem cohort.
//!
//! Every card is a normal-layout creature whose only ability is a layer-7c
//! controller-scoped anthem that excludes the source. Each fingerprint is
//! SHA-256 of the exact pinned normalized Oracle text.

use tricerules_cards::primitives::{CreatureScopeController, StaticAbilityDef};
use tricerules_cards::{AbilityPresentation, Layout};

struct Card {
    id: &'static str,
    name: &'static str,
    mana_cost: &'static str,
    types: &'static [&'static str],
    power: u32,
    toughness: u32,
    delta_power: i32,
    delta_toughness: i32,
    fingerprint: &'static str,
}

const CARDS: &[Card] = &[
    Card {
        id: "veteran_armorer",
        name: "Veteran Armorer",
        mana_cost: "{1}{W}",
        types: &["Creature", "Human", "Soldier"],
        power: 2,
        toughness: 2,
        delta_power: 0,
        delta_toughness: 1,
        fingerprint: "5b231542b4eb00cf248c8c2b66f5477518da8231b5d43a408bdf505dc531ab46",
    },
    Card {
        id: "benalish_marshal",
        name: "Benalish Marshal",
        mana_cost: "{W}{W}{W}",
        types: &["Creature", "Human", "Knight"],
        power: 3,
        toughness: 3,
        delta_power: 1,
        delta_toughness: 1,
        fingerprint: "51a409183459ccfb4d2671d3bcf939a1266a125ddbf46a7ea601b8f0bccb5821",
    },
    Card {
        id: "blessed_orator",
        name: "Blessed Orator",
        mana_cost: "{3}{W}",
        types: &["Creature", "Human", "Cleric"],
        power: 1,
        toughness: 4,
        delta_power: 0,
        delta_toughness: 1,
        fingerprint: "5b231542b4eb00cf248c8c2b66f5477518da8231b5d43a408bdf505dc531ab46",
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
fn other_anthems_batch_maps_definitions() {
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
        assert_eq!(
            f.mana_cost.to_string(),
            card.mana_cost,
            "{} mana cost",
            card.id
        );
        assert_eq!(f.types, card.types, "{} types", card.id);
        assert!(f.supertypes.is_empty(), "{}", card.id);
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
        let StaticAbilityDef::AnthemPt {
            filter,
            condition: None,
            delta_power,
            delta_toughness,
        } = &entry.definition
        else {
            panic!("{}: unconditional anthem", card.id);
        };
        assert_eq!(
            filter.controller,
            Some(CreatureScopeController::YouControl),
            "{}",
            card.id
        );
        assert!(filter.exclude_self, "{} excludes the source", card.id);
        assert!(filter.subtype.is_none(), "{}", card.id);
        assert!(filter.color.is_none(), "{}", card.id);
        assert!(filter.required_keyword.is_none(), "{}", card.id);
        assert_eq!(
            (*delta_power, *delta_toughness),
            (card.delta_power, card.delta_toughness),
            "{}",
            card.id
        );
    }
}

#[test]
fn other_anthems_batch_fingerprints_match_the_pinned_oracle_text() {
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
        assert_eq!(fields[2], card.id);
        assert_eq!(
            fields[4], card.fingerprint,
            "fingerprint drift for {}",
            card.id
        );
    }
}
