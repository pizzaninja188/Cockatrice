//! Registry conformance for the queue-v21 activated loot cohort.
//!
//! Every included card is a normal-layout permanent with one activated ability that
//! draws a card and then discards a card, with no other ability. Each fingerprint is
//! SHA-256 of the exact pinned normalized Oracle text.

use tricerules_cards::primitives::{
    AbilityCost, Amount, DiscardQuantity, InstructionKind, SpellEffectKind,
};
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
        id: "captain_of_umbar",
        name: "Captain of Umbar",
        mana_cost: "{2}{U}",
        types: &["Creature", "Human", "Pirate"],
        body: Some((2, 3)),
        activation: "{1}",
        oracle_text_sha256: "deaebede08bec767260159fb2996d341a79ab10388d8e55544c321c06bfb5d5c",
    },
    ExpectedCard {
        id: "dragonborn_looter",
        name: "Dragonborn Looter",
        mana_cost: "{1}{U}",
        types: &["Creature", "Dragon", "Rogue"],
        body: Some((1, 2)),
        activation: "{1}",
        oracle_text_sha256: "deaebede08bec767260159fb2996d341a79ab10388d8e55544c321c06bfb5d5c",
    },
    ExpectedCard {
        id: "facet_reader",
        name: "Facet Reader",
        mana_cost: "{1}{U}",
        types: &["Creature", "Human", "Wizard"],
        body: Some((1, 2)),
        activation: "{1}",
        oracle_text_sha256: "deaebede08bec767260159fb2996d341a79ab10388d8e55544c321c06bfb5d5c",
    },
    ExpectedCard {
        id: "jalum_tome",
        name: "Jalum Tome",
        mana_cost: "{3}",
        types: &["Artifact", "Book"],
        body: None,
        activation: "{2}",
        oracle_text_sha256: "503f75a35a230129ca9a61461c07aee6354e8fa3ec14731d0fa02eafb4edb379",
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
    assert!(ability.effect.is_empty(), "{id}: program-owned effects");
    let program = ability.program.as_ref().expect("draw/discard program");
    let [draw, discard] = program.body.steps.as_slice() else {
        panic!("{id}: exactly two program steps");
    };
    assert_eq!(draw.id, "draw_one", "{id}");
    assert!(
        matches!(
            &draw.kind,
            InstructionKind::Perform(op)
                if matches!(**op, SpellEffectKind::Draw { count: Amount::Fixed(1), .. })
        ),
        "{id}: draw one"
    );
    assert_eq!(discard.id, "discard_one", "{id}");
    assert!(
        matches!(
            &discard.kind,
            InstructionKind::Perform(op)
                if matches!(
                    **op,
                    SpellEffectKind::Discard {
                        quantity: DiscardQuantity::Exact(1),
                        ..
                    }
                )
        ),
        "{id}: discard one"
    );
}

#[test]
fn loot_activated_batch_canary_cards_match_their_typed_shape() {
    for expected in CARDS
        .iter()
        .filter(|card| matches!(card.id, "dragonborn_looter" | "jalum_tome"))
    {
        check_card(expected);
    }
}

#[test]
fn loot_activated_batch_maps_definitions() {
    for expected in CARDS {
        check_card(expected);
    }
}

#[test]
fn loot_activated_batch_fingerprints_match_the_pinned_oracle_text() {
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
