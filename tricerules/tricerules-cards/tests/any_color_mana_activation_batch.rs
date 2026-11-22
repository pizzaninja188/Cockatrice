//! Registry conformance for the queue-v21 any-color mana activation cohort.
//!
//! Every included card is a normal-layout permanent with one mana ability costing the
//! printed generic mana and a tap and producing one mana of any color, with no other
//! ability. Each fingerprint is SHA-256 of the exact pinned normalized Oracle text.

use tricerules_cards::primitives::{AbilityCost, SpellEffectKind};
use tricerules_cards::{AbilityPresentation, ActivationTiming, Layout};

struct ExpectedCard {
    id: &'static str,
    name: &'static str,
    mana_cost: &'static str,
    types: &'static [&'static str],
    body: Option<(u32, u32)>,
    activation: &'static str,
    oracle_text_sha256: &'static str,
}

const CARDS: &[ExpectedCard] = &[
    ExpectedCard {
        id: "celestial_prism",
        name: "Celestial Prism",
        mana_cost: "{3}",
        types: &["Artifact"],
        body: None,
        activation: "{2}",
        oracle_text_sha256: "1ca811c6ec17468c3bf43decc0f4d2806fd34d617f5419d4dee062312c10d7eb",
    },
    ExpectedCard {
        id: "mana_cylix",
        name: "Mana Cylix",
        mana_cost: "{1}",
        types: &["Artifact"],
        body: None,
        activation: "{1}",
        oracle_text_sha256: "775370524b31efa9d987f120742403a8f58afe68c1c378b4c59908167bd8c206",
    },
    ExpectedCard {
        id: "viridian_acolyte",
        name: "Viridian Acolyte",
        mana_cost: "{G}",
        types: &["Creature", "Elf", "Shaman"],
        body: Some((1, 1)),
        activation: "{1}",
        oracle_text_sha256: "775370524b31efa9d987f120742403a8f58afe68c1c378b4c59908167bd8c206",
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
        match expected.body {
            Some((power, toughness)) => (Some(power), Some(toughness)),
            None => (None, None),
        },
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
            [AbilityCost::Mana(cost), AbilityCost::Tap]
                if cost.to_string() == expected.activation
        ),
        "{id}: {:?}",
        ability.costs
    );
    assert!(ability.is_mana_ability(), "{id}: mana ability");
    let [SpellEffectKind::ProduceMana { options, .. }] = ability.effect.as_slice() else {
        panic!("{id}: one mana effect");
    };
    let masks: Vec<[u32; 5]> = options
        .iter()
        .map(|amount| [amount.w, amount.u, amount.b, amount.r, amount.g])
        .collect();
    assert_eq!(
        masks,
        [
            [1, 0, 0, 0, 0],
            [0, 1, 0, 0, 0],
            [0, 0, 1, 0, 0],
            [0, 0, 0, 1, 0],
            [0, 0, 0, 0, 1],
        ],
        "{id}: the five distinct printed color options in order"
    );
    assert!(ability.targeting.is_none(), "{id}: no target");
}

#[test]
fn any_color_mana_activation_batch_canary_cards_match_their_typed_shape() {
    for expected in CARDS
        .iter()
        .filter(|card| matches!(card.id, "viridian_acolyte" | "celestial_prism"))
    {
        check_card(expected);
    }
}

#[test]
fn any_color_mana_activation_batch_maps_definitions() {
    for expected in CARDS {
        check_card(expected);
    }
}

#[test]
fn any_color_mana_activation_batch_fingerprints_match_the_pinned_oracle_text() {
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
