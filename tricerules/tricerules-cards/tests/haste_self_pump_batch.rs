//! Registry conformance for the queue-v23 haste self-pump cohort.
//!
//! Every included card has the printed Haste keyword and exactly one
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
        id: "crypt_ripper",
        name: "Crypt Ripper",
        mana_cost: "{2}{B}{B}",
        types: &["Creature", "Shade"],
        body: (2, 2),
        activation: "{B}",
        pump: (1, 1),
        oracle_text_sha256: "d09f5ff5a7999dbb294aa52ad1d4ee649c90ae75be3c336899ed0177b174a87e",
    },
    ExpectedCard {
        id: "furnace_spirit",
        name: "Furnace Spirit",
        mana_cost: "{2}{R}",
        types: &["Creature", "Spirit"],
        body: (1, 1),
        activation: "{R}",
        pump: (1, 0),
        oracle_text_sha256: "075da45edab4053c811f5c3cce073de7e7ca1dc35a1967251fc6cec3b3e1b6de",
    },
    ExpectedCard {
        id: "pyre_charger",
        name: "Pyre Charger",
        mana_cost: "{R}{R}",
        types: &["Creature", "Elemental", "Warrior"],
        body: (1, 1),
        activation: "{R}",
        pump: (1, 0),
        oracle_text_sha256: "075da45edab4053c811f5c3cce073de7e7ca1dc35a1967251fc6cec3b3e1b6de",
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
    assert_eq!(face.keywords, [Keyword::Haste], "{id}");
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
fn haste_self_pump_batch_canary_cards_match_their_typed_shape() {
    for expected in CARDS
        .iter()
        .filter(|card| matches!(card.id, "crypt_ripper" | "pyre_charger"))
    {
        check_card(expected);
    }
}

#[test]
fn haste_self_pump_batch_maps_definitions() {
    for expected in CARDS {
        check_card(expected);
    }
}

#[test]
fn haste_self_pump_batch_fingerprints_match_the_pinned_oracle_text() {
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
