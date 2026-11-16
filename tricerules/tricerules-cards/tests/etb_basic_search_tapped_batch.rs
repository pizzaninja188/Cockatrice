//! Registry conformance for the queue-v22 optional basic-land entry search
//! cohort (search onto the battlefield tapped).
//!
//! Every included card is a normal-layout creature whose only ability triggers
//! when it enters and runs a typed optional private basic-land search. Each
//! fingerprint is SHA-256 of the exact pinned normalized Oracle text.

use tricerules_cards::primitives::{
    CardTypeFilter, InstructionKind, SearchDestination, SpellEffectKind,
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
        id: "farhaven_elf",
        name: "Farhaven Elf",
        mana_cost: "{2}{G}",
        types: &["Creature", "Elf", "Druid"],
        power: 1,
        toughness: 1,
        oracle_text_sha256: "a887e61e5cd8cf9a072287d9a3b87721e0a164cfec1e1c8f4ecf400b037af677",
    },
    ExpectedCard {
        id: "ondu_giant",
        name: "Ondu Giant",
        mana_cost: "{3}{G}",
        types: &["Creature", "Giant", "Druid"],
        power: 2,
        toughness: 4,
        oracle_text_sha256: "a887e61e5cd8cf9a072287d9a3b87721e0a164cfec1e1c8f4ecf400b037af677",
    },
    ExpectedCard {
        id: "wild_wanderer",
        name: "Wild Wanderer",
        mana_cost: "{3}{G}",
        types: &["Creature", "Elf", "Druid"],
        power: 3,
        toughness: 2,
        oracle_text_sha256: "a887e61e5cd8cf9a072287d9a3b87721e0a164cfec1e1c8f4ecf400b037af677",
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
    assert!(triggered.effect.is_empty(), "{id}");
    let program = triggered
        .program
        .as_ref()
        .expect("typed optional entry-search program");
    let [step] = program.body.steps.as_slice() else {
        panic!("{id}: one typed entry-search step");
    };
    assert!(
        matches!(
            &step.kind,
            InstructionKind::Choose { optional: true, branches, otherwise, .. }
                if branches.len() == 1
                    && otherwise.steps.is_empty()
                    && matches!(branches[0].body.steps.as_slice(),
                        [step] if matches!(&step.kind,
                            InstructionKind::Perform(effect)
                                if matches!(effect.as_ref(), SpellEffectKind::SearchLibrary {
                                    filter: Some(filter),
                                    destination: SearchDestination::Battlefield { tapped: true },
                                    shuffle: true, reveal: false, ..
                                } if filter.card_type == Some(CardTypeFilter::BasicLand))))
        ),
        "{id}: optional basic-land search onto the battlefield tapped"
    );
}

#[test]
fn etb_basic_search_tapped_batch_canary_cards_match_their_typed_shape() {
    for expected in CARDS
        .iter()
        .filter(|card| matches!(card.id, "farhaven_elf" | "ondu_giant"))
    {
        check_card(expected);
    }
}

#[test]
fn etb_basic_search_tapped_batch_maps_definitions() {
    for expected in CARDS {
        check_card(expected);
    }
}

#[test]
fn etb_basic_search_tapped_batch_fingerprints_match_the_pinned_oracle_text() {
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
