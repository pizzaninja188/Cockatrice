//! Registry conformance for the queue-v22 "dies, put a +1/+1 counter on target
//! creature you control" cohort.
//!
//! Every included card is a normal-layout creature with exactly one dies trigger
//! that puts one +1/+1 counter on a chosen creature its controller controls.
//! Each fingerprint is SHA-256 of the exact pinned normalized Oracle text.

use tricerules_cards::primitives::{
    Amount, CounterKind, EffectSubject, ObjectFilter, TargetController, TargetKind,
};
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
    oracle_text_sha256: &'static str,
}

const CARDS: &[ExpectedCard] = &[
    ExpectedCard {
        id: "spinal_centipede",
        name: "Spinal Centipede",
        mana_cost: "{2}{B}",
        types: &["Creature", "Insect"],
        power: 3,
        toughness: 2,
        keywords: &[],
        line: 1,
        oracle_text_sha256: "0caa81d1582ea693b91cc827f00d4d336c5d8c4be7d0be45588b45594bab6008",
    },
    ExpectedCard {
        id: "guul_draz_mucklord",
        name: "Guul Draz Mucklord",
        mana_cost: "{2}{B}",
        types: &["Creature", "Crocodile"],
        power: 2,
        toughness: 3,
        keywords: &[],
        line: 1,
        oracle_text_sha256: "0caa81d1582ea693b91cc827f00d4d336c5d8c4be7d0be45588b45594bab6008",
    },
    ExpectedCard {
        id: "sparring_construct",
        name: "Sparring Construct",
        mana_cost: "{1}",
        types: &["Artifact", "Creature", "Construct"],
        power: 1,
        toughness: 1,
        keywords: &[],
        line: 1,
        oracle_text_sha256: "0caa81d1582ea693b91cc827f00d4d336c5d8c4be7d0be45588b45594bab6008",
    },
    ExpectedCard {
        id: "lawless_broker",
        name: "Lawless Broker",
        mana_cost: "{2}{B}",
        types: &["Creature", "Aetherborn", "Rogue"],
        power: 3,
        toughness: 2,
        keywords: &[],
        line: 1,
        oracle_text_sha256: "0caa81d1582ea693b91cc827f00d4d336c5d8c4be7d0be45588b45594bab6008",
    },
    ExpectedCard {
        id: "goblin_assault_team",
        name: "Goblin Assault Team",
        mana_cost: "{3}{R}",
        types: &["Creature", "Goblin", "Warrior"],
        power: 4,
        toughness: 1,
        keywords: &[Keyword::Haste],
        line: 2,
        oracle_text_sha256: "c7ee23085239817be823a7526849c496d2c4e59784d6927e1580fb1abb49ee2d",
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
    let [SpellEffectKind::PutCounters {
        counter,
        count,
        subject,
    }] = triggered.effect.as_slice()
    else {
        panic!("{id}: one counter instruction");
    };
    assert_eq!(*counter, CounterKind::PlusOnePlusOne, "{id}");
    assert_eq!(*count, Amount::Fixed(1), "{id}: exactly one counter");
    assert_eq!(
        *subject,
        EffectSubject::Chosen(Box::new(ObjectFilter {
            kind: TargetKind::Creature,
            controller: TargetController::You,
            ..ObjectFilter::default()
        })),
        "{id}: a creature its controller controls"
    );
}

#[test]
fn dies_counter_target_batch_canary_cards_match_their_typed_shape() {
    for expected in CARDS
        .iter()
        .filter(|card| matches!(card.id, "sparring_construct" | "goblin_assault_team"))
    {
        check_card(expected);
    }
}

#[test]
fn dies_counter_target_batch_maps_definitions() {
    for expected in CARDS {
        check_card(expected);
    }
}

#[test]
fn dies_counter_target_batch_fingerprints_match_the_pinned_oracle_text() {
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
