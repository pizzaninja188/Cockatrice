//! Registry conformance for the queue-v22 "entry, draw a card and lose 1 life"
//! cohort.
//!
//! Every included card is a normal-layout creature with exactly one entry trigger.
//! Each fingerprint is SHA-256 of the exact pinned normalized Oracle text.

use tricerules_cards::primitives::{Amount, LifeAmount, PlayerRecipient, SpellEffectKind};
use tricerules_cards::{AbilityPresentation, Layout, TriggerCondition};

struct ExpectedCard {
    id: &'static str,
    name: &'static str,
    mana_cost: &'static str,
    types: &'static [&'static str],
    power: u32,
    toughness: u32,
    oracle_text_sha256: &'static str,
}

const CARDS: &[ExpectedCard] = &[
    ExpectedCard {
        id: "dusk_legion_zealot",
        name: "Dusk Legion Zealot",
        mana_cost: "{1}{B}",
        types: &["Creature", "Vampire", "Soldier"],
        power: 1,
        toughness: 1,
        oracle_text_sha256: "bdf839b555c64392c0552feb2cb1c92219f01571e8537119cb928cf8dde6b977",
    },
    ExpectedCard {
        id: "phyrexian_rager",
        name: "Phyrexian Rager",
        mana_cost: "{2}{B}",
        types: &["Creature", "Phyrexian", "Horror"],
        power: 2,
        toughness: 2,
        oracle_text_sha256: "bdf839b555c64392c0552feb2cb1c92219f01571e8537119cb928cf8dde6b977",
    },
    ExpectedCard {
        id: "tithebearer_giant",
        name: "Tithebearer Giant",
        mana_cost: "{5}{B}",
        types: &["Creature", "Giant", "Warrior"],
        power: 4,
        toughness: 5,
        oracle_text_sha256: "bdf839b555c64392c0552feb2cb1c92219f01571e8537119cb928cf8dde6b977",
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
    assert!(face.supertypes.is_empty(), "{id}");
    assert!(face.keywords.is_empty(), "{id}");
    assert!(face.spell_effect.is_empty(), "{id}");
    assert!(face.activated_abilities.is_empty(), "{id}");
    assert!(face.static_abilities.is_empty(), "{id}");
    assert!(face.targeting.is_none(), "{id}");

    let [triggered] = face.triggered_abilities.as_slice() else {
        panic!("{id}: exactly one triggered ability");
    };
    assert_eq!(triggered.ability_id.as_str(), "triggered_01", "{id}");
    assert_eq!(
        triggered.presentation,
        AbilityPresentation::OracleLines(vec![1]),
        "{id}"
    );
    assert_eq!(
        triggered.trigger,
        TriggerCondition::WhenSelfEntersBattlefield,
        "{id}"
    );
    let [SpellEffectKind::Draw {
        who: draw_who,
        count,
    }, SpellEffectKind::LoseLife { amount, who }] = triggered.effect.as_slice()
    else {
        panic!("{id}: draw then fixed life loss");
    };
    assert_eq!(*draw_who, PlayerRecipient::Controller, "{id}");
    assert_eq!(*count, Amount::Fixed(1), "{id}: exactly one card");
    assert_eq!(*amount, LifeAmount::Fixed(1), "{id}: exactly one life");
    assert_eq!(*who, PlayerRecipient::Controller, "{id}");
}

#[test]
fn etb_draw_lose_life_batch_canary_cards_match_their_typed_shape() {
    for expected in CARDS
        .iter()
        .filter(|card| matches!(card.id, "dusk_legion_zealot" | "tithebearer_giant"))
    {
        check_card(expected);
    }
}

#[test]
fn etb_draw_lose_life_batch_maps_definitions() {
    for expected in CARDS {
        check_card(expected);
    }
}

#[test]
fn etb_draw_lose_life_batch_fingerprints_match_the_pinned_oracle_text() {
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
