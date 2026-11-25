//! Registry conformance for the queue-v22 "sacrifice a creature, draw two or
//! three" cohort.
//!
//! Every included card is a normal-layout spell with one mandatory additional
//! creature-sacrifice cost and one fixed draw. Each fingerprint is SHA-256 of the
//! exact pinned normalized Oracle text.

use tricerules_cards::primitives::{AdditionalCost, ObjectFilter, TargetController, TargetKind};
use tricerules_cards::{Layout, SpellEffectKind};

struct ExpectedCard {
    id: &'static str,
    name: &'static str,
    mana_cost: &'static str,
    types: &'static [&'static str],
    draw: u32,
    oracle_text_sha256: &'static str,
}

const CARDS: &[ExpectedCard] = &[
    ExpectedCard {
        id: "blood_divination",
        name: "Blood Divination",
        mana_cost: "{3}{B}",
        types: &["Sorcery"],
        draw: 3,
        oracle_text_sha256: "392082c8e0389f8ad2003e2410d6eea15a7613a51d8e76214bf97cd7a18c372a",
    },
    ExpectedCard {
        id: "vivisection",
        name: "Vivisection",
        mana_cost: "{3}{U}",
        types: &["Sorcery"],
        draw: 3,
        oracle_text_sha256: "392082c8e0389f8ad2003e2410d6eea15a7613a51d8e76214bf97cd7a18c372a",
    },
    ExpectedCard {
        id: "altars_reap",
        name: "Altar's Reap",
        mana_cost: "{1}{B}",
        types: &["Instant"],
        draw: 2,
        oracle_text_sha256: "c35fcea4edc49b0a5e42bbaec5073eb29a951b12fda9f6f99cf0bc35e4b68872",
    },
    ExpectedCard {
        id: "skulltap",
        name: "Skulltap",
        mana_cost: "{1}{B}",
        types: &["Sorcery"],
        draw: 2,
        oracle_text_sha256: "c35fcea4edc49b0a5e42bbaec5073eb29a951b12fda9f6f99cf0bc35e4b68872",
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
    assert!(face.activated_abilities.is_empty(), "{id}");
    assert!(face.triggered_abilities.is_empty(), "{id}");
    assert!(face.static_abilities.is_empty(), "{id}");
    assert!(face.targeting.is_none(), "{id}");

    let [cost] = face.additional_costs.as_slice() else {
        panic!("{id}: exactly one additional cost");
    };
    let AdditionalCost::SacrificePermanent { filter } = cost else {
        panic!("{id}: a creature-sacrifice additional cost");
    };
    assert_eq!(
        *filter,
        ObjectFilter {
            kind: TargetKind::Creature,
            controller: TargetController::You,
            ..ObjectFilter::default()
        },
        "{id}: sacrifice a creature the caster controls"
    );

    let [SpellEffectKind::Draw { who, count }] = face.spell_effect.as_slice() else {
        panic!("{id}: one draw instruction");
    };
    assert_eq!(
        *who,
        tricerules_cards::primitives::PlayerRecipient::Controller,
        "{id}: only the caster draws"
    );
    assert_eq!(
        *count,
        tricerules_cards::primitives::Amount::Fixed(expected.draw),
        "{id}: printed draw"
    );
}

#[test]
fn sacrifice_draw_batch_canary_cards_match_their_typed_shape() {
    for expected in CARDS
        .iter()
        .filter(|card| matches!(card.id, "altars_reap" | "vivisection"))
    {
        check_card(expected);
    }
}

#[test]
fn sacrifice_draw_batch_maps_definitions() {
    for expected in CARDS {
        check_card(expected);
    }
}

#[test]
fn sacrifice_draw_batch_fingerprints_match_the_pinned_oracle_text() {
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
