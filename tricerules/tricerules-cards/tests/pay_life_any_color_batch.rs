//! Registry conformance for the queue-v21 tap-and-pay-life mana cohort.
//!
//! Every included card is a normal-layout permanent with one mana ability costing a
//! tap and 1 life and producing one mana of any color, with no other ability. Each
//! fingerprint is SHA-256 of the exact pinned normalized Oracle text.

use tricerules_cards::primitives::{AbilityCost, SpellEffectKind};
use tricerules_cards::{AbilityPresentation, ActivationTiming, Layout};

struct ExpectedCard {
    id: &'static str,
    name: &'static str,
    mana_cost: &'static str,
    types: &'static [&'static str],
    body: Option<(u32, u32)>,
    oracle_text_sha256: &'static str,
}

const CARDS: &[ExpectedCard] = &[
    ExpectedCard {
        id: "mana_confluence",
        name: "Mana Confluence",
        mana_cost: "",
        types: &["Land"],
        body: None,
        oracle_text_sha256: "e381b99c0c1ec7115ac2f615ff3c483d2b2cea38791c7cb7b3adddc2180506b4",
    },
    ExpectedCard {
        id: "phyrexian_lens",
        name: "Phyrexian Lens",
        mana_cost: "{3}",
        types: &["Artifact"],
        body: None,
        oracle_text_sha256: "e381b99c0c1ec7115ac2f615ff3c483d2b2cea38791c7cb7b3adddc2180506b4",
    },
    ExpectedCard {
        id: "vesper_ghoul",
        name: "Vesper Ghoul",
        mana_cost: "{2}{B}",
        types: &["Creature", "Zombie", "Druid"],
        body: Some((1, 1)),
        oracle_text_sha256: "e381b99c0c1ec7115ac2f615ff3c483d2b2cea38791c7cb7b3adddc2180506b4",
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
    assert_eq!(
        ability.costs,
        [AbilityCost::Tap, AbilityCost::PayLife { amount: 1 }],
        "{id}"
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
fn pay_life_any_color_batch_canary_cards_match_their_typed_shape() {
    for expected in CARDS
        .iter()
        .filter(|card| matches!(card.id, "phyrexian_lens" | "vesper_ghoul"))
    {
        check_card(expected);
    }
}

#[test]
fn pay_life_any_color_batch_maps_definitions() {
    for expected in CARDS {
        check_card(expected);
    }
}

#[test]
fn pay_life_any_color_batch_fingerprints_match_the_pinned_oracle_text() {
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
