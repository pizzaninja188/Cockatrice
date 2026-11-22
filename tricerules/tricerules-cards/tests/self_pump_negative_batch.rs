//! Registry conformance for the queue-v21 negative-toughness self-pump cohort.
//!
//! Every included card is a normal-layout creature with one activated ability that
//! gives itself a printed positive power and negative toughness modification until
//! end of turn, with no other ability. Each fingerprint is SHA-256 of the exact
//! pinned normalized Oracle text.

use tricerules_cards::primitives::{AbilityCost, EffectSubject, SpellEffectKind};
use tricerules_cards::{AbilityPresentation, ActivationTiming, Layout};

struct ExpectedCard {
    id: &'static str,
    name: &'static str,
    mana_cost: &'static str,
    types: &'static [&'static str],
    power: u32,
    toughness: u32,
    activation: &'static str,
    pump: (i32, i32),
    oracle_text_sha256: &'static str,
}

const CARDS: &[ExpectedCard] = &[
    ExpectedCard {
        id: "breathstealer",
        name: "Breathstealer",
        mana_cost: "{2}{B}",
        types: &["Creature", "Nightstalker"],
        power: 2,
        toughness: 2,
        activation: "{B}",
        pump: (1, -1),
        oracle_text_sha256: "3ea72f12d2e752940089e711470e84f60bea7956acd4b24ee1430e5f3acc68d9",
    },
    ExpectedCard {
        id: "flowstone_crusher",
        name: "Flowstone Crusher",
        mana_cost: "{3}{R}{R}",
        types: &["Creature", "Beast"],
        power: 4,
        toughness: 4,
        activation: "{R}",
        pump: (1, -1),
        oracle_text_sha256: "1a0cf6e9dcf870bfa712f8bfe87e0e1fe8309bad40d5877204983b44c626bd26",
    },
    ExpectedCard {
        id: "flowstone_giant",
        name: "Flowstone Giant",
        mana_cost: "{2}{R}{R}",
        types: &["Creature", "Giant"],
        power: 3,
        toughness: 3,
        activation: "{R}",
        pump: (2, -2),
        oracle_text_sha256: "d993a4824263cc42d3d17065516644ef9f60193f75d6d67dff601b752250b38e",
    },
    ExpectedCard {
        id: "flowstone_shambler",
        name: "Flowstone Shambler",
        mana_cost: "{2}{R}",
        types: &["Creature", "Beast"],
        power: 2,
        toughness: 2,
        activation: "{R}",
        pump: (1, -1),
        oracle_text_sha256: "1a0cf6e9dcf870bfa712f8bfe87e0e1fe8309bad40d5877204983b44c626bd26",
    },
    ExpectedCard {
        id: "frostburn_weird",
        name: "Frostburn Weird",
        mana_cost: "{U/R}{U/R}",
        types: &["Creature", "Weird"],
        power: 1,
        toughness: 4,
        activation: "{U/R}",
        pump: (1, -1),
        oracle_text_sha256: "07c2996b889f902116657fbef9dc48ce835e306283128a34624147b539d5dfb3",
    },
    ExpectedCard {
        id: "retrieval_agent",
        name: "Retrieval Agent",
        mana_cost: "{3}{U}",
        types: &["Creature", "Human", "Soldier"],
        power: 2,
        toughness: 5,
        activation: "{2}",
        pump: (1, -1),
        oracle_text_sha256: "ed10d288c770b328b34bd08027940481790ad60284b0404ec42169e3dd558e07",
    },
    ExpectedCard {
        id: "scion_of_glaciers",
        name: "Scion of Glaciers",
        mana_cost: "{2}{U}{U}",
        types: &["Creature", "Elemental"],
        power: 2,
        toughness: 5,
        activation: "{U}",
        pump: (1, -1),
        oracle_text_sha256: "93b39b5cc78f4d1c9bc70bfde1a78415829d3d8f30c45c43cf98088720c07160",
    },
    ExpectedCard {
        id: "viashino_slasher",
        name: "Viashino Slasher",
        mana_cost: "{1}{R}",
        types: &["Creature", "Lizard", "Warrior"],
        power: 1,
        toughness: 2,
        activation: "{R}",
        pump: (1, -1),
        oracle_text_sha256: "1a0cf6e9dcf870bfa712f8bfe87e0e1fe8309bad40d5877204983b44c626bd26",
    },
    ExpectedCard {
        id: "watercourser",
        name: "Watercourser",
        mana_cost: "{2}{U}",
        types: &["Creature", "Elemental"],
        power: 2,
        toughness: 3,
        activation: "{U}",
        pump: (1, -1),
        oracle_text_sha256: "93b39b5cc78f4d1c9bc70bfde1a78415829d3d8f30c45c43cf98088720c07160",
    },
    ExpectedCard {
        id: "yavimaya_ancients",
        name: "Yavimaya Ancients",
        mana_cost: "{3}{G}{G}",
        types: &["Creature", "Treefolk"],
        power: 2,
        toughness: 7,
        activation: "{G}",
        pump: (1, -2),
        oracle_text_sha256: "85591b9517b936a522aa07496fcad686bae336f212fb7904c3657d260a91b504",
    },
];

fn check_card(expected: &ExpectedCard) {
    let id = expected.id;
    let registry = tricerules_cards::registry::global();
    let definition = registry.get(id).unwrap_or_else(|| panic!("missing {id}"));
    assert_eq!(definition.layout, Layout::Normal, "{id}");
    assert_eq!(definition.name, expected.name, "{id}");
    assert_eq!(registry.id_for_name(expected.name), Some(id), "{id}");
    let face = definition.primary_face();
    assert_eq!(face.face_id.as_str(), id, "{id} face id");
    assert_eq!(face.mana_cost.to_string(), expected.mana_cost, "{id}");
    assert_eq!(face.types, expected.types, "{id} types");
    assert_eq!(
        (face.power, face.toughness),
        (Some(expected.power), Some(expected.toughness)),
        "{id}"
    );
    assert!(face.supertypes.is_empty(), "{id}");
    assert!(face.keywords.is_empty(), "{id}");
    assert!(face.spell_effect.is_empty(), "{id}");
    assert!(face.triggered_abilities.is_empty(), "{id}");
    assert!(face.static_abilities.is_empty(), "{id}");
    assert!(face.targeting.is_none(), "{id}");

    let [ability] = face.activated_abilities.as_slice() else {
        panic!("{id}: exactly one activated ability");
    };
    assert_eq!(ability.ability_id.as_str(), "activated_01", "{id}");
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![1]),
        "{id}"
    );
    assert_eq!(ability.timing, ActivationTiming::Normal, "{id}");
    assert!(
        matches!(
            ability.costs.as_slice(),
            [AbilityCost::Mana(cost)] if cost.to_string() == expected.activation
        ),
        "{id}: {:?}",
        ability.costs
    );
    assert_eq!(
        ability.effect,
        [SpellEffectKind::PumpTarget {
            power: expected.pump.0,
            toughness: expected.pump.1,
            subject: EffectSubject::Source,
            scale: None,
        }],
        "{id}: exact printed self-pump"
    );
    assert!(ability.targeting.is_none(), "{id}: no target");
}

#[test]
fn self_pump_negative_batch_canary_cards_match_their_typed_shape() {
    for expected in CARDS
        .iter()
        .filter(|card| matches!(card.id, "breathstealer" | "frostburn_weird"))
    {
        check_card(expected);
    }
}

#[test]
fn self_pump_negative_batch_maps_definitions() {
    for expected in CARDS {
        check_card(expected);
    }
}

#[test]
fn self_pump_negative_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for expected in CARDS {
        let id = expected.id;
        let row = fingerprints
            .lines()
            .find(|line| line.starts_with(&format!("{id}\t")))
            .unwrap_or_else(|| panic!("missing fingerprint row for {id}"));
        let fields: Vec<&str> = row.split('\t').collect();
        assert_eq!(fields.len(), 5, "fingerprint row shape: {row}");
        assert_eq!(fields[0], id);
        assert_eq!(fields[1], expected.name);
        assert_eq!(fields[2], id);
        assert_eq!(
            fields[4], expected.oracle_text_sha256,
            "fingerprint drift for {id}"
        );
    }
}
