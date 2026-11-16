//! Registry conformance for the queue-v22 "dies, each opponent loses N life and
//! you gain N life" cohort.
//!
//! Every included card is a normal-layout creature with exactly one dies trigger.
//! Each fingerprint is SHA-256 of the exact pinned normalized Oracle text.

use tricerules_cards::{AbilityPresentation, Keyword, Layout, SpellEffectKind, TriggerCondition};

struct ExpectedCard {
    id: &'static str,
    name: &'static str,
    mana_cost: &'static str,
    types: &'static [&'static str],
    power: u32,
    toughness: u32,
    keywords: &'static [Keyword],
    line: u16,
    life: u32,
    oracle_text_sha256: &'static str,
}

const CARDS: &[ExpectedCard] = &[
    ExpectedCard {
        id: "brotherhood_patriarch",
        name: "Brotherhood Patriarch",
        mana_cost: "{3}{B}",
        types: &["Creature", "Human", "Assassin"],
        power: 4,
        toughness: 1,
        keywords: &[],
        line: 1,
        life: 2,
        oracle_text_sha256: "4cc0ee694f85fc0306b2c78a651b3ed7af5faacc4bba6a10ca778d2bf504bb45",
    },
    ExpectedCard {
        id: "spirit_of_malevolence",
        name: "Spirit of Malevolence",
        mana_cost: "{1}{B}",
        types: &["Creature", "Spirit"],
        power: 2,
        toughness: 1,
        keywords: &[],
        line: 1,
        life: 1,
        oracle_text_sha256: "325e78665ec780c43e1cdb26175fc2dc8f81cf43e3d33c4b0c20251c73fa9f4a",
    },
    ExpectedCard {
        id: "symbiote_spawn",
        name: "Symbiote Spawn",
        mana_cost: "{2}{B}",
        types: &["Creature", "Symbiote", "Villain"],
        power: 3,
        toughness: 2,
        keywords: &[],
        line: 1,
        life: 2,
        oracle_text_sha256: "4cc0ee694f85fc0306b2c78a651b3ed7af5faacc4bba6a10ca778d2bf504bb45",
    },
    ExpectedCard {
        id: "etched_familiar",
        name: "Etched Familiar",
        mana_cost: "{2}{B}",
        types: &["Artifact", "Creature", "Phyrexian", "Fox"],
        power: 3,
        toughness: 2,
        keywords: &[],
        line: 1,
        life: 2,
        oracle_text_sha256: "4cc0ee694f85fc0306b2c78a651b3ed7af5faacc4bba6a10ca778d2bf504bb45",
    },
    ExpectedCard {
        id: "nocturnal_feeder",
        name: "Nocturnal Feeder",
        mana_cost: "{2}{B}",
        types: &["Creature", "Vampire", "Rogue"],
        power: 2,
        toughness: 1,
        keywords: &[Keyword::Flying],
        line: 2,
        life: 2,
        oracle_text_sha256: "f231e22d79aacda1e743b227a4e652ff7e0dbfffb29d1bd1f45bef0b4f394adb",
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
    assert_eq!(face.keywords, expected.keywords, "{id} keywords");
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
        AbilityPresentation::OracleLines(vec![expected.line]),
        "{id}"
    );
    assert_eq!(triggered.trigger, TriggerCondition::WhenSelfDies, "{id}");
    let [SpellEffectKind::LoseLife { amount, who }, SpellEffectKind::GainLife {
        amount: gain,
        who: gain_who,
    }] = triggered.effect.as_slice()
    else {
        panic!("{id}: fixed loss then fixed gain");
    };
    assert_eq!(
        *amount,
        tricerules_cards::primitives::LifeAmount::Fixed(expected.life),
        "{id}: each opponent loses the printed life"
    );
    assert_eq!(
        *who,
        tricerules_cards::primitives::PlayerRecipient::EachOpponent,
        "{id}: every opponent loses"
    );
    assert_eq!(
        *gain_who,
        tricerules_cards::primitives::PlayerRecipient::Controller,
        "{id}: only the controller gains"
    );
    assert_eq!(
        *gain,
        tricerules_cards::primitives::Amount::Fixed(expected.life),
        "{id}: the controller gains the printed life"
    );
}

#[test]
fn dies_drain_batch_canary_cards_match_their_typed_shape() {
    for expected in CARDS
        .iter()
        .filter(|card| matches!(card.id, "spirit_of_malevolence" | "nocturnal_feeder"))
    {
        check_card(expected);
    }
}

#[test]
fn dies_drain_batch_maps_definitions() {
    for expected in CARDS {
        check_card(expected);
    }
}

#[test]
fn dies_drain_batch_fingerprints_match_the_pinned_oracle_text() {
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
