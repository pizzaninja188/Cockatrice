//! Registry conformance for the queue-v22 aura cohort granting `+X/+X` and haste.
//!
//! Every included card is a normal-layout Aura with the attach spell effect and
//! one attached static modifier. Each fingerprint is SHA-256 of the exact pinned
//! normalized Oracle text.

use tricerules_cards::primitives::{SpellEffectKind, StaticAbilityDef};
use tricerules_cards::{Keyword, Layout};

struct ExpectedCard {
    id: &'static str,
    name: &'static str,
    mana_cost: &'static str,
    bonus: i32,
    oracle_text_sha256: &'static str,
}

const CARDS: &[ExpectedCard] = &[
    ExpectedCard {
        id: "goblin_war_paint",
        name: "Goblin War Paint",
        mana_cost: "{1}{R}",
        bonus: 2,
        oracle_text_sha256: "95209b7a4edc983fadd4ec2742b9885865a7bcb2696bd20036468aaeef2052d4",
    },
    ExpectedCard {
        id: "primal_visitation",
        name: "Primal Visitation",
        mana_cost: "{3}{R}{G}",
        bonus: 3,
        oracle_text_sha256: "a63d6a7ea1431ea5c41e61447bafd9ef127f99c8fff85bfcb5ff5360a0fb2c0c",
    },
    ExpectedCard {
        id: "swashbuckling",
        name: "Swashbuckling",
        mana_cost: "{1}{R}",
        bonus: 2,
        oracle_text_sha256: "95209b7a4edc983fadd4ec2742b9885865a7bcb2696bd20036468aaeef2052d4",
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
    assert_eq!(face.types, ["Enchantment", "Aura"], "{id} types");
    assert!(face.supertypes.is_empty(), "{id}");
    assert!(face.keywords.is_empty(), "{id}");
    assert!(
        matches!(
            face.spell_effect.as_slice(),
            [SpellEffectKind::AuraAttach { .. }]
        ),
        "{id}: attach spell effect"
    );
    assert!(face.activated_abilities.is_empty(), "{id}");
    assert!(face.triggered_abilities.is_empty(), "{id}");
    assert!(face.targeting.is_none(), "{id}");

    let [static_ability] = face.static_abilities.as_slice() else {
        panic!("{id}: exactly one static ability");
    };
    assert_eq!(static_ability.ability_id.as_str(), "static_01", "{id}");
    let StaticAbilityDef::AttachedModifier {
        delta_power,
        delta_toughness,
        keywords,
        ..
    } = &static_ability.definition
    else {
        panic!("{id}: attached modifier");
    };
    assert_eq!(*delta_power, expected.bonus, "{id}");
    assert_eq!(*delta_toughness, expected.bonus, "{id}");
    assert_eq!(keywords, &[Keyword::Haste], "{id}: haste");
}

#[test]
fn aura_pump_haste_batch_canary_cards_match_their_typed_shape() {
    for expected in CARDS
        .iter()
        .filter(|card| matches!(card.id, "goblin_war_paint" | "primal_visitation"))
    {
        check_card(expected);
    }
}

#[test]
fn aura_pump_haste_batch_maps_definitions() {
    for expected in CARDS {
        check_card(expected);
    }
}

#[test]
fn aura_pump_haste_batch_fingerprints_match_the_pinned_oracle_text() {
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
