//! Registry conformance for the queue-v23 trample self-pump cohort.
//!
//! Every included card has the printed Trample keyword and exactly one
//! mana-cost activated ability that pumps only its own source. Each fingerprint
//! is SHA-256 of the exact pinned normalized Oracle text.

use tricerules_cards::primitives::{AbilityCost, EffectSubject, SpellEffectKind};
use tricerules_cards::{AbilityPresentation, Keyword, Layout};

struct ExpectedCard {
    id: &'static str,
    name: &'static str,
    mana_cost: &'static str,
    types: &'static [&'static str],
    body: (u32, u32),
    activation: &'static str,
    pump: (i32, i32),
    oracle_text_sha256: &'static str,
}

const CARDS: &[ExpectedCard] = &[
    ExpectedCard {
        id: "almighty_brushwagg",
        name: "Almighty Brushwagg",
        mana_cost: "{G}",
        types: &["Creature", "Brushwagg"],
        body: (1, 1),
        activation: "{3}{G}",
        pump: (3, 3),
        oracle_text_sha256: "13828e3d291f552d580bced5f86c74c7a67d0862ae339f459a104526c203ab54",
    },
    ExpectedCard {
        id: "feral_ridgewolf",
        name: "Feral Ridgewolf",
        mana_cost: "{2}{R}",
        types: &["Creature", "Wolf"],
        body: (1, 2),
        activation: "{1}{R}",
        pump: (2, 0),
        oracle_text_sha256: "79d2d9472d2097faceee164f9a47a7b4738de3535d2dae072fa558d8c68840b9",
    },
    ExpectedCard {
        id: "sun-collared_raptor",
        name: "Sun-Collared Raptor",
        mana_cost: "{1}{R}",
        types: &["Creature", "Dinosaur"],
        body: (1, 2),
        activation: "{2}{R}",
        pump: (3, 0),
        oracle_text_sha256: "5b62979c3aede12311df34e3c0b6eb1fbef3ad3c44cf42beebc67d815d2199ec",
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
    assert_eq!(face.face_id.as_str(), id.replace('-', "_"), "{id} face id");
    assert_eq!(face.mana_cost.to_string(), expected.mana_cost, "{id}");
    assert_eq!(face.types, expected.types, "{id} types");
    assert!(face.supertypes.is_empty(), "{id}");
    assert_eq!(face.keywords, [Keyword::Trample], "{id}");
    assert_eq!(
        (face.power, face.toughness),
        (Some(expected.body.0), Some(expected.body.1)),
        "{id} body"
    );
    assert!(face.triggered_abilities.is_empty(), "{id}");
    assert!(face.static_abilities.is_empty(), "{id}");
    assert!(face.spell_effect.is_empty(), "{id}");
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
    assert_eq!(ability.costs.len(), 1, "{id}");
    assert!(
        matches!(&ability.costs[0], AbilityCost::Mana(cost) if cost.to_string() == expected.activation),
        "{id}: printed activation"
    );
    assert!(ability.targeting.is_none(), "{id}");
    assert!(
        matches!(
            ability.effect.as_slice(),
            [SpellEffectKind::PumpTarget {
                power,
                toughness,
                subject: EffectSubject::Source,
                ..
            }] if (*power, *toughness) == expected.pump
        ),
        "{id}: source-only printed pump"
    );
}

#[test]
fn trample_self_pump_batch_canary_cards_match_their_typed_shape() {
    for expected in CARDS
        .iter()
        .filter(|card| matches!(card.id, "almighty_brushwagg" | "sun-collared_raptor"))
    {
        check_card(expected);
    }
}

#[test]
fn trample_self_pump_batch_maps_definitions() {
    for expected in CARDS {
        check_card(expected);
    }
}

#[test]
fn trample_self_pump_batch_fingerprints_match_the_pinned_oracle_text() {
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
        assert_eq!(fields[2], id.replace('-', "_"));
        assert_eq!(
            fields[4], expected.oracle_text_sha256,
            "fingerprint drift for {id}"
        );
    }
}
