//! Registry conformance for the reviewed controller-scoped Sliver anthem cohort.
//!
//! Every included card is a normal-layout Sliver whose only ability is a
//! controller-scoped layer-7c anthem for Sliver creatures, including itself.
//! Each fingerprint is SHA-256 of the exact pinned Oracle line.

use tricerules_cards::primitives::{CreatureScopeController, StaticAbilityDef};
use tricerules_cards::{AbilityPresentation, Layout};

struct Card {
    id: &'static str,
    name: &'static str,
    mana_cost: &'static str,
    power: u32,
    toughness: u32,
    pump_power: i32,
    pump_toughness: i32,
    fingerprint: &'static str,
}

const CARDS: &[Card] = &[
    Card {
        id: "battle_sliver",
        name: "Battle Sliver",
        mana_cost: "{4}{R}",
        power: 3,
        toughness: 3,
        pump_power: 2,
        pump_toughness: 0,
        fingerprint: "af83753c8d89bbd22793120e9d36f89b5938822fe9066165d8f6aa78641250ae",
    },
    Card {
        id: "cleaving_sliver",
        name: "Cleaving Sliver",
        mana_cost: "{3}{R}",
        power: 2,
        toughness: 2,
        pump_power: 2,
        pump_toughness: 0,
        fingerprint: "af83753c8d89bbd22793120e9d36f89b5938822fe9066165d8f6aa78641250ae",
    },
    Card {
        id: "megantic_sliver",
        name: "Megantic Sliver",
        mana_cost: "{5}{G}",
        power: 3,
        toughness: 3,
        pump_power: 3,
        pump_toughness: 3,
        fingerprint: "e39dda469ba623ba18f1f468bdaceb6a942d52e3e3568e6e09f41e82a5fa21c6",
    },
    Card {
        id: "predatory_sliver",
        name: "Predatory Sliver",
        mana_cost: "{1}{G}",
        power: 1,
        toughness: 1,
        pump_power: 1,
        pump_toughness: 1,
        fingerprint: "33fb4185ecfedabe2f0c3f166febb4436cd29262113173d14cc7e63318b0a3fb",
    },
    Card {
        id: "steelform_sliver",
        name: "Steelform Sliver",
        mana_cost: "{2}{W}",
        power: 2,
        toughness: 2,
        pump_power: 0,
        pump_toughness: 1,
        fingerprint: "6389dfd23e43115ef82a762cc3b4bb5af256e96e087ce2205795835289390b61",
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
fn sliver_anthem_controlled_batch_maps_definitions() {
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
        assert_eq!(f.types, ["Creature", "Sliver"], "{} types", card.id);
        assert!(f.supertypes.is_empty(), "{}", card.id);
        assert_eq!(f.power, Some(card.power), "{} power", card.id);
        assert_eq!(f.toughness, Some(card.toughness), "{} toughness", card.id);
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
            "{} presentation",
            card.id
        );
        let StaticAbilityDef::AnthemPt {
            filter,
            condition: None,
            delta_power,
            delta_toughness,
        } = &entry.definition
        else {
            panic!("{}: controller-scoped Sliver anthem", card.id);
        };
        assert_eq!(
            filter.controller,
            Some(CreatureScopeController::YouControl),
            "{}",
            card.id
        );
        assert_eq!(filter.subtype.as_deref(), Some("Sliver"), "{}", card.id);
        assert!(!filter.exclude_self, "{} includes itself", card.id);
        assert!(filter.required_keyword.is_none(), "{}", card.id);
        assert_eq!(
            (*delta_power, *delta_toughness),
            (card.pump_power, card.pump_toughness),
            "{}",
            card.id
        );
    }
}

#[test]
fn sliver_anthem_controlled_batch_fingerprints_match_the_pinned_oracle_text() {
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
