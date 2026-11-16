//! Registry conformance for the queue-v23 mana-activated target-pump cohort.
//!
//! Every included card has exactly one mana-cost activated ability that pumps a
//! chosen target creature. Each fingerprint is SHA-256 of the exact pinned
//! normalized Oracle text.

use tricerules_cards::primitives::{AbilityCost, EffectSubject, SpellEffectKind, TargetFilter};
use tricerules_cards::{AbilityPresentation, Layout};

struct ExpectedCard {
    id: &'static str,
    name: &'static str,
    mana_cost: &'static str,
    types: &'static [&'static str],
    body: Option<(u32, u32)>,
    activation: &'static str,
    pump: (i32, i32),
    oracle_text_sha256: &'static str,
}

const CARDS: &[ExpectedCard] = &[
    ExpectedCard {
        id: "ghitu_war_cry",
        name: "Ghitu War Cry",
        mana_cost: "{2}{R}",
        types: &["Enchantment"],
        body: None,
        activation: "{R}",
        pump: (1, 0),
        oracle_text_sha256: "794f10d34fd958cf8d542a9844c54ca30e71e130442528c72476f563a8e6d0c9",
    },
    ExpectedCard {
        id: "captive_flame",
        name: "Captive Flame",
        mana_cost: "{2}{R}",
        types: &["Enchantment"],
        body: None,
        activation: "{R}",
        pump: (1, 0),
        oracle_text_sha256: "794f10d34fd958cf8d542a9844c54ca30e71e130442528c72476f563a8e6d0c9",
    },
    ExpectedCard {
        id: "sacred_armory",
        name: "Sacred Armory",
        mana_cost: "{2}",
        types: &["Artifact"],
        body: None,
        activation: "{2}",
        pump: (1, 0),
        oracle_text_sha256: "2a9347cc807432cdc120ca33e96cc8e073472347cd856377d620d91fa28f5f01",
    },
    ExpectedCard {
        id: "ursapine",
        name: "Ursapine",
        mana_cost: "{3}{G}{G}",
        types: &["Creature", "Beast"],
        body: Some((3, 3)),
        activation: "{G}",
        pump: (1, 1),
        oracle_text_sha256: "c7dcab2d0956cc9f6ed7d37ba6916ff6088b7159469b01946ae329aa5e1ec2c5",
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
    assert!(face.supertypes.is_empty(), "{id}");
    assert!(face.keywords.is_empty(), "{id}");
    assert!(face.spell_effect.is_empty(), "{id}");
    assert!(face.triggered_abilities.is_empty(), "{id}");
    assert!(face.static_abilities.is_empty(), "{id}");
    assert!(face.targeting.is_none(), "{id}");
    assert_eq!(
        (face.power, face.toughness),
        expected
            .body
            .map_or((None, None), |(p, t)| (Some(p), Some(t))),
        "{id} body"
    );

    let [ability] = face.activated_abilities.as_slice() else {
        panic!("{id}: exactly one activated ability");
    };
    assert_eq!(ability.ability_id.as_str(), "activated_01", "{id}");
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![1]),
        "{id}"
    );
    assert_eq!(ability.costs.len(), 1, "{id}");
    assert!(
        matches!(&ability.costs[0], AbilityCost::Mana(cost) if cost.to_string() == expected.activation),
        "{id}: printed activation cost"
    );
    assert!(ability.targeting.is_none(), "{id}");
    assert!(
        matches!(
            ability.effect.as_slice(),
            [SpellEffectKind::PumpTarget {
                power,
                toughness,
                subject: EffectSubject::Chosen(filter),
                ..
            }] if (*power, *toughness) == expected.pump
                && **filter == TargetFilter::default_creature()
        ),
        "{id}: chosen-creature pump"
    );
}

#[test]
fn mana_target_pump_batch_canary_cards_match_their_typed_shape() {
    for expected in CARDS
        .iter()
        .filter(|card| matches!(card.id, "sacred_armory" | "ursapine"))
    {
        check_card(expected);
    }
}

#[test]
fn mana_target_pump_batch_maps_definitions() {
    for expected in CARDS {
        check_card(expected);
    }
}

#[test]
fn mana_target_pump_batch_fingerprints_match_the_pinned_oracle_text() {
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
