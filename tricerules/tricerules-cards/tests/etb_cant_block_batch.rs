//! Registry conformance for the queue-v22 "entry, target creature can't block
//! this turn" cohort.
//!
//! Every included card is a normal-layout creature with exactly one entry trigger.
//! Each fingerprint is SHA-256 of the exact pinned normalized Oracle text.

use tricerules_cards::primitives::{
    CombatRestrictionScope, ObjectFilter, SpellEffectKind, TargetKind,
};
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
        id: "crossway_vampire",
        name: "Crossway Vampire",
        mana_cost: "{1}{R}{R}",
        types: &["Creature", "Vampire"],
        power: 3,
        toughness: 2,
        oracle_text_sha256: "6036e27a33b7ce99796f35f181fc6128df78193e107f412f5329e13cc8d18588",
    },
    ExpectedCard {
        id: "goblin_shortcutter",
        name: "Goblin Shortcutter",
        mana_cost: "{1}{R}",
        types: &["Creature", "Goblin", "Scout"],
        power: 2,
        toughness: 1,
        oracle_text_sha256: "6036e27a33b7ce99796f35f181fc6128df78193e107f412f5329e13cc8d18588",
    },
    ExpectedCard {
        id: "unstoppable_ogre",
        name: "Unstoppable Ogre",
        mana_cost: "{2}{R}",
        types: &["Artifact", "Creature", "Ogre", "Warrior"],
        power: 4,
        toughness: 1,
        oracle_text_sha256: "6036e27a33b7ce99796f35f181fc6128df78193e107f412f5329e13cc8d18588",
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
    let [SpellEffectKind::ApplyCombatRestriction { scope, restriction }] =
        triggered.effect.as_slice()
    else {
        panic!("{id}: one combat restriction instruction");
    };
    assert_eq!(
        *scope,
        CombatRestrictionScope::Chosen(ObjectFilter {
            kind: TargetKind::Creature,
            ..ObjectFilter::default()
        }),
        "{id}: target creature"
    );
    assert!(restriction.cant_block, "{id}: cannot block");
    assert!(!restriction.cant_be_blocked, "{id}");
    assert!(restriction.cant_block_creatures_matching.is_empty(), "{id}");
}

#[test]
fn etb_cant_block_batch_canary_cards_match_their_typed_shape() {
    for expected in CARDS
        .iter()
        .filter(|card| matches!(card.id, "goblin_shortcutter" | "crossway_vampire"))
    {
        check_card(expected);
    }
}

#[test]
fn etb_cant_block_batch_maps_definitions() {
    for expected in CARDS {
        check_card(expected);
    }
}

#[test]
fn etb_cant_block_batch_fingerprints_match_the_pinned_oracle_text() {
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
