//! Registry conformance for the queue-v22 printed `+X/-Y` flying self-pump
//! cohort.
//!
//! Every included card is a normal-layout creature with flying and exactly one
//! mana-cost self-pump activation. Each fingerprint is SHA-256 of the exact
//! pinned normalized Oracle text.

use tricerules_cards::primitives::{AbilityCost, EffectSubject, SpellEffectKind};
use tricerules_cards::{AbilityPresentation, Keyword, Layout};

struct ExpectedCard {
    id: &'static str,
    name: &'static str,
    mana_cost: &'static str,
    types: &'static [&'static str],
    power: u32,
    toughness: u32,
    cost: &'static str,
    delta_power: i32,
    delta_toughness: i32,
    oracle_text_sha256: &'static str,
}

const CARDS: &[ExpectedCard] = &[
    ExpectedCard {
        id: "darklit_gargoyle",
        name: "Darklit Gargoyle",
        mana_cost: "{1}{W}",
        types: &["Artifact", "Creature", "Gargoyle"],
        power: 1,
        toughness: 2,
        cost: "{B}",
        delta_power: 2,
        delta_toughness: -1,
        oracle_text_sha256: "83137a573ba68df1f28b96d0173b5926f7677d31173c7d04c467aedb27e62335",
    },
    ExpectedCard {
        id: "flowstone_wyvern",
        name: "Flowstone Wyvern",
        mana_cost: "{3}{R}{R}",
        types: &["Creature", "Drake"],
        power: 3,
        toughness: 3,
        cost: "{R}",
        delta_power: 2,
        delta_toughness: -2,
        oracle_text_sha256: "84ed9d6a896e0ce750b425b9df82cfd77fc272ca354cd18cf6c0240811033353",
    },
    ExpectedCard {
        id: "metropolis_sprite",
        name: "Metropolis Sprite",
        mana_cost: "{1}{U}",
        types: &["Creature", "Faerie", "Rogue"],
        power: 1,
        toughness: 2,
        cost: "{U}",
        delta_power: 1,
        delta_toughness: -1,
        oracle_text_sha256: "9ee563e5c0d93089a61b34934c1feac5bca5b9e4b99bd6fd15249fb0c762c8cd",
    },
    ExpectedCard {
        id: "talonrend",
        name: "Talonrend",
        mana_cost: "{4}{U}",
        types: &["Creature", "Elemental"],
        power: 0,
        toughness: 5,
        cost: "{U/R}",
        delta_power: 1,
        delta_toughness: -1,
        oracle_text_sha256: "39408014075682e314bea7af971cfb27e577bc50de1145b93b4d55c95d7b362f",
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
        "{id} body"
    );
    assert_eq!(face.keywords, [Keyword::Flying], "{id} keywords");
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
        AbilityPresentation::OracleLines(vec![2]),
        "{id}"
    );
    assert!(
        matches!(
            ability.costs.as_slice(),
            [AbilityCost::Mana(cost)] if cost.to_string() == expected.cost
        ),
        "{id}: {:?}",
        ability.costs
    );
    assert_eq!(
        ability.effect,
        [SpellEffectKind::PumpTarget {
            power: expected.delta_power,
            toughness: expected.delta_toughness,
            subject: EffectSubject::Source,
            scale: None,
        }],
        "{id}: exact printed self-pump"
    );
    assert!(ability.targeting.is_none(), "{id}: no target");
}

#[test]
fn flying_neg_pump_batch_canary_cards_match_their_typed_shape() {
    for expected in CARDS
        .iter()
        .filter(|card| matches!(card.id, "metropolis_sprite" | "talonrend"))
    {
        check_card(expected);
    }
}

#[test]
fn flying_neg_pump_batch_maps_definitions() {
    for expected in CARDS {
        check_card(expected);
    }
}

#[test]
fn flying_neg_pump_batch_fingerprints_match_the_pinned_oracle_text() {
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
