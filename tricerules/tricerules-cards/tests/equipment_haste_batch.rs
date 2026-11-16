//! Registry conformance for the queue-v23 Equipment haste cohort.
//!
//! Every included card is a normal-layout Equipment with exactly one attached
//! modifier granting a fixed pump and haste and one `{1}` equip ability. Each
//! fingerprint is SHA-256 of the exact pinned normalized Oracle text.

use tricerules_cards::primitives::{AbilityCost, SpellEffectKind, StaticAbilityDef};
use tricerules_cards::{AbilityPresentation, Keyword, Layout};

struct ExpectedCard {
    id: &'static str,
    name: &'static str,
    mana_cost: &'static str,
    delta_power: i32,
    delta_toughness: i32,
    oracle_text_sha256: &'static str,
}

const CARDS: &[ExpectedCard] = &[
    ExpectedCard {
        id: "boots_of_speed",
        name: "Boots of Speed",
        mana_cost: "{R}",
        delta_power: 1,
        delta_toughness: 0,
        oracle_text_sha256: "51ccf68c0e444eb6c5a412f9468bcd30ea5d4fea140e6c65d0cdfe06870c453f",
    },
    ExpectedCard {
        id: "crystal_slipper",
        name: "Crystal Slipper",
        mana_cost: "{1}{R}",
        delta_power: 1,
        delta_toughness: 0,
        oracle_text_sha256: "51ccf68c0e444eb6c5a412f9468bcd30ea5d4fea140e6c65d0cdfe06870c453f",
    },
    ExpectedCard {
        id: "strider_harness",
        name: "Strider Harness",
        mana_cost: "{3}",
        delta_power: 1,
        delta_toughness: 1,
        oracle_text_sha256: "9306d56169735e2e19379d069ab80294ab0e8a26ce40566f54a88616b1b78c48",
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
    assert_eq!(face.types, ["Artifact", "Equipment"], "{id} types");
    assert!(face.supertypes.is_empty(), "{id}");
    assert!(face.keywords.is_empty(), "{id}");
    assert!(face.spell_effect.is_empty(), "{id}");
    assert!(face.triggered_abilities.is_empty(), "{id}");
    assert!(face.targeting.is_none(), "{id}");
    assert_eq!((face.power, face.toughness), (None, None), "{id}");

    let [static_ability] = face.static_abilities.as_slice() else {
        panic!("{id}: exactly one static ability");
    };
    assert_eq!(static_ability.ability_id.as_str(), "static_01", "{id}");
    assert_eq!(
        static_ability.presentation,
        AbilityPresentation::OracleLines(vec![1]),
        "{id}"
    );
    let StaticAbilityDef::AttachedModifier {
        delta_power,
        delta_toughness,
        keywords,
        ..
    } = &static_ability.definition
    else {
        panic!("{id}: attached modifier");
    };
    assert_eq!(*delta_power, expected.delta_power, "{id}");
    assert_eq!(*delta_toughness, expected.delta_toughness, "{id}");
    assert_eq!(keywords, &[Keyword::Haste], "{id}: haste");

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
        matches!(&ability.costs[0], AbilityCost::Mana(cost) if cost.to_string() == "{1}"),
        "{id}: equip {{1}}"
    );
    assert!(
        matches!(ability.effect.as_slice(), [SpellEffectKind::Equip { .. }]),
        "{id}: equip effect"
    );
}

#[test]
fn equipment_haste_batch_canary_cards_match_their_typed_shape() {
    for expected in CARDS
        .iter()
        .filter(|card| matches!(card.id, "boots_of_speed" | "strider_harness"))
    {
        check_card(expected);
    }
}

#[test]
fn equipment_haste_batch_maps_definitions() {
    for expected in CARDS {
        check_card(expected);
    }
}

#[test]
fn equipment_haste_batch_fingerprints_match_the_pinned_oracle_text() {
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
