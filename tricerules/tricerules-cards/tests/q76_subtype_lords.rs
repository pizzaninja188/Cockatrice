//! Registry conformance for the queue-v76 02-subtype-lords cohort.
//!
//! Every card is a normal-layout creature whose only rules text is a layer-7c anthem for
//! other controlled creatures of one subtype. Fingerprints are checked after the presentation
//! metadata refresh.

use tricerules_cards::primitives::{CreatureScopeController, StaticAbilityDef};
use tricerules_cards::{AbilityPresentation, Layout};

struct Card {
    id: &'static str,
    name: &'static str,
    mana_cost: &'static str,
    types: &'static [&'static str],
    power: u32,
    toughness: u32,
    subtype: &'static str,
    delta_power: i32,
    delta_toughness: i32,
    fingerprint: &'static str,
}

const CARDS: &[Card] = &[
    Card {
        id: "merfolk_mistbinder",
        name: "Merfolk Mistbinder",
        mana_cost: "{G}{U}",
        types: &["Creature", "Merfolk", "Shaman"],
        power: 2,
        toughness: 2,
        subtype: "Merfolk",
        delta_power: 1,
        delta_toughness: 1,
        fingerprint: "ac26fc0080723f2a3146b7321cc8706e2cb14f7b2b9fcc3266e35c8516143d3c",
    },
    Card {
        id: "legion_lieutenant",
        name: "Legion Lieutenant",
        mana_cost: "{W}{B}",
        types: &["Creature", "Vampire", "Knight"],
        power: 2,
        toughness: 2,
        subtype: "Vampire",
        delta_power: 1,
        delta_toughness: 1,
        fingerprint: "e3dec81a97e905c797ba0c7fe96a7ed524d2ecdd344947689300ecde3bdcfebb",
    },
    Card {
        id: "thirsting_bloodlord",
        name: "Thirsting Bloodlord",
        mana_cost: "{2}{B}{B}",
        types: &["Creature", "Vampire"],
        power: 3,
        toughness: 3,
        subtype: "Vampire",
        delta_power: 1,
        delta_toughness: 1,
        fingerprint: "e3dec81a97e905c797ba0c7fe96a7ed524d2ecdd344947689300ecde3bdcfebb",
    },
    Card {
        id: "kargan_warleader",
        name: "Kargan Warleader",
        mana_cost: "{1}{R}{W}",
        types: &["Creature", "Human", "Warrior"],
        power: 3,
        toughness: 3,
        subtype: "Warrior",
        delta_power: 1,
        delta_toughness: 1,
        fingerprint: "e8240095b1f9e614859d6efc91e9f6765fcb32b975a7843b259c658ac91bc02e",
    },
    Card {
        id: "yotian_tactician",
        name: "Yotian Tactician",
        mana_cost: "{2}{W}{U}",
        types: &["Creature", "Human", "Soldier"],
        power: 3,
        toughness: 4,
        subtype: "Soldier",
        delta_power: 1,
        delta_toughness: 1,
        fingerprint: "35ce9fa3c357a30a8165ef228e2a47fe7e3159691f26929b9d283b19e27da1a3",
    },
    Card {
        id: "regal_imperiosaur",
        name: "Regal Imperiosaur",
        mana_cost: "{1}{G}{G}",
        types: &["Creature", "Dinosaur"],
        power: 5,
        toughness: 4,
        subtype: "Dinosaur",
        delta_power: 1,
        delta_toughness: 1,
        fingerprint: "4607cba4d41c120f0b259921220f97b0e3d600cb16aaf56c62710bf7ca21dc0d",
    },
    Card {
        id: "squirrel_sovereign",
        name: "Squirrel Sovereign",
        mana_cost: "{1}{G}",
        types: &["Creature", "Squirrel", "Noble"],
        power: 2,
        toughness: 2,
        subtype: "Squirrel",
        delta_power: 1,
        delta_toughness: 1,
        fingerprint: "c435840d2e79e4f6ed4b87684eedc71136eff2c60e828006508100125bae1ffe",
    },
    Card {
        id: "inspiring_veteran",
        name: "Inspiring Veteran",
        mana_cost: "{R}{W}",
        types: &["Creature", "Human", "Knight"],
        power: 2,
        toughness: 2,
        subtype: "Knight",
        delta_power: 1,
        delta_toughness: 1,
        fingerprint: "84fae48cebb87f8876c0970f7b3f0d9e99f42422949765a813227431f77d56f3",
    },
    Card {
        id: "bladestitched_skaab",
        name: "Bladestitched Skaab",
        mana_cost: "{U}{B}",
        types: &["Creature", "Zombie", "Soldier"],
        power: 2,
        toughness: 3,
        subtype: "Zombie",
        delta_power: 1,
        delta_toughness: 0,
        fingerprint: "6a59bdad25e58e3fefeb9908bcdf239201f8b3a199104e8c850d11c711213443",
    },
    Card {
        id: "king_of_the_pride",
        name: "King of the Pride",
        mana_cost: "{2}{W}",
        types: &["Creature", "Cat"],
        power: 2,
        toughness: 1,
        subtype: "Cat",
        delta_power: 2,
        delta_toughness: 1,
        fingerprint: "559710be1fe2116eade690e0f01439c0a51c371542190b2c8250b2b4aad023e5",
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
fn q76_subtype_lords_map_definitions() {
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
        assert_eq!(f.types, card.types, "{} types", card.id);
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
            panic!("{}: unconditional AnthemPt", card.id);
        };
        assert_eq!(
            filter.controller,
            Some(CreatureScopeController::YouControl),
            "{}",
            card.id
        );
        assert_eq!(filter.subtype.as_deref(), Some(card.subtype), "{}", card.id);
        assert!(filter.exclude_self, "{} excludes the source", card.id);
        assert!(filter.color.is_none(), "{}", card.id);
        assert!(filter.required_keyword.is_none(), "{}", card.id);
        assert!(!filter.attacking, "{}", card.id);
        assert_eq!(
            (*delta_power, *delta_toughness),
            (card.delta_power, card.delta_toughness),
            "{}",
            card.id
        );
    }
}

#[test]
fn q76_subtype_lords_fingerprints_match_the_pinned_oracle_text() {
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
