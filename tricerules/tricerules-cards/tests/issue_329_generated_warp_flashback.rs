//! Registry and presentation conformance for issue #329.
//!
//! The two cast-method templates must register the five reviewed Standard identities with their
//! complete typed payloads: CR 702.185 the hand alternative Warp cost on permanent faces;
//! CR 702.34 the graveyard alternative Flashback cost on an instant face; and the preserved
//! printed keywords, draw, ETB gain life, and ETB surveil effects.

use tricerules_cards::primitives::PlayerRecipient;
use tricerules_cards::{
    AbilityPresentation, Amount, CardRegistry, Color, Keyword, Layout, LibraryPartitionKind,
    SpellEffectKind, TriggerCondition,
};

#[test]
fn issue_329_registers_the_five_reviewed_identities() {
    let registry = CardRegistry::global();
    for (id, name) in [
        ("bygone_colossus", "Bygone Colossus"),
        ("germinating_wurm", "Germinating Wurm"),
        ("red_tiger_mechan", "Red Tiger Mechan"),
        ("starbreach_whale", "Starbreach Whale"),
        ("think_twice", "Think Twice"),
    ] {
        let definition = registry
            .get(id)
            .unwrap_or_else(|| panic!("missing reviewed card {id}"));
        assert_eq!(definition.name, name);
        assert_eq!(registry.id_for_name(name), Some(id));
        assert_eq!(definition.layout, Layout::Normal);
        assert_eq!(definition.face_count(), 1);
        assert_eq!(definition.primary_face().face_id.as_str(), id);
    }
}

#[test]
fn issue_329_bygone_colossus_has_a_generic_warp_cost() {
    let definition = CardRegistry::global()
        .get("bygone_colossus")
        .expect("Bygone Colossus");
    let face = definition.primary_face();
    assert_eq!(face.mana_cost.to_string(), "{9}");
    assert_eq!(
        face.warp_cost.as_ref().map(ToString::to_string),
        Some("{3}".to_string())
    );
    assert!(face.flashback_cost.is_none());
    assert_eq!(face.types, ["Artifact", "Creature", "Robot", "Giant"]);
    assert_eq!((face.power, face.toughness), (Some(9), Some(9)));
    assert!(face.colors().is_empty());
    assert!(face.spell_effect.is_empty());
    assert!(face.triggered_abilities.is_empty());
}

#[test]
fn issue_329_germinating_wurm_keeps_its_etb_gain_life() {
    let definition = CardRegistry::global()
        .get("germinating_wurm")
        .expect("Germinating Wurm");
    let face = definition.primary_face();
    assert_eq!(face.mana_cost.to_string(), "{4}{G}");
    assert_eq!(
        face.warp_cost.as_ref().map(ToString::to_string),
        Some("{1}{G}".to_string())
    );
    assert_eq!(face.types, ["Creature", "Plant", "Wurm"]);
    assert_eq!((face.power, face.toughness), (Some(5), Some(5)));
    assert_eq!(face.colors(), vec![Color::Green]);

    let [ability] = face.triggered_abilities.as_slice() else {
        panic!("Germinating Wurm must keep exactly one triggered ability");
    };
    assert_eq!(ability.trigger, TriggerCondition::WhenSelfEntersBattlefield);
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert_eq!(
        ability.effect,
        [SpellEffectKind::GainLife {
            amount: Amount::Fixed(2),
        }]
    );
    assert!(ability.targeting.is_none());
}

#[test]
fn issue_329_red_tiger_mechan_keeps_haste() {
    let definition = CardRegistry::global()
        .get("red_tiger_mechan")
        .expect("Red Tiger Mechan");
    let face = definition.primary_face();
    assert_eq!(face.mana_cost.to_string(), "{3}{R}");
    assert_eq!(
        face.warp_cost.as_ref().map(ToString::to_string),
        Some("{1}{R}".to_string())
    );
    assert_eq!(face.types, ["Artifact", "Creature", "Robot", "Cat"]);
    assert_eq!((face.power, face.toughness), (Some(3), Some(3)));
    assert_eq!(face.colors(), vec![Color::Red]);
    assert_eq!(face.keywords, [Keyword::Haste]);
    assert!(face.triggered_abilities.is_empty());
}

#[test]
fn issue_329_starbreach_whale_keeps_flying_and_etb_surveil_two() {
    let definition = CardRegistry::global()
        .get("starbreach_whale")
        .expect("Starbreach Whale");
    let face = definition.primary_face();
    assert_eq!(face.mana_cost.to_string(), "{4}{U}");
    assert_eq!(
        face.warp_cost.as_ref().map(ToString::to_string),
        Some("{1}{U}".to_string())
    );
    assert_eq!(face.types, ["Creature", "Whale"]);
    assert_eq!((face.power, face.toughness), (Some(3), Some(5)));
    assert_eq!(face.colors(), vec![Color::Blue]);
    assert_eq!(face.keywords, [Keyword::Flying]);

    let [ability] = face.triggered_abilities.as_slice() else {
        panic!("Starbreach Whale must keep exactly one triggered ability");
    };
    assert_eq!(ability.trigger, TriggerCondition::WhenSelfEntersBattlefield);
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    assert_eq!(
        ability.effect,
        [SpellEffectKind::LibraryPartition {
            count: 2,
            top_min: 0,
            top_max: None,
            kind: LibraryPartitionKind::Surveil,
        }]
    );
    assert!(ability.targeting.is_none());
}

#[test]
fn issue_329_think_twice_has_a_flashback_cost_and_keeps_the_draw() {
    let definition = CardRegistry::global()
        .get("think_twice")
        .expect("Think Twice");
    let face = definition.primary_face();
    assert_eq!(face.mana_cost.to_string(), "{1}{U}");
    assert_eq!(
        face.flashback_cost.as_ref().map(ToString::to_string),
        Some("{2}{U}".to_string())
    );
    assert!(face.warp_cost.is_none());
    assert_eq!(face.types, ["Instant"]);
    assert_eq!(face.colors(), vec![Color::Blue]);
    assert_eq!(
        face.spell_effect,
        [SpellEffectKind::Draw {
            who: PlayerRecipient::Controller,
            count: Amount::Fixed(1),
        }]
    );
    assert!(face.targeting.is_none());
}

#[test]
fn issue_329_fingerprint_rows_match_the_presentation_registry() {
    let registry = CardRegistry::global();
    let cases: &[(&str, &str, &str, &str)] = &[
        (
            "bygone_colossus",
            "bygone_colossus",
            "Bygone Colossus",
            "Bygone Colossus",
        ),
        (
            "germinating_wurm",
            "germinating_wurm",
            "Germinating Wurm",
            "Germinating Wurm",
        ),
        (
            "red_tiger_mechan",
            "red_tiger_mechan",
            "Red Tiger Mechan",
            "Red Tiger Mechan",
        ),
        (
            "starbreach_whale",
            "starbreach_whale",
            "Starbreach Whale",
            "Starbreach Whale",
        ),
        ("think_twice", "think_twice", "Think Twice", "Think Twice"),
    ];
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for (id, face_id, card_name, face_name) in cases {
        let presentation = registry
            .presentation_face(id, face_id)
            .unwrap_or_else(|| panic!("missing presentation metadata for {id}/{face_id}"));
        assert_eq!(presentation.card_name, *card_name);
        assert_eq!(presentation.face_name, *face_name);
        assert_eq!(presentation.oracle_text_sha256.len(), 64);

        let row = fingerprints
            .lines()
            .find(|line| {
                line.starts_with(&format!("{id}\t")) && line.contains(&format!("\t{face_id}\t"))
            })
            .unwrap_or_else(|| panic!("missing fingerprint row for {id}/{face_id}"));
        assert!(
            row.ends_with(&presentation.oracle_text_sha256),
            "fingerprint drift for {id}/{face_id}: {row}"
        );
    }
}
