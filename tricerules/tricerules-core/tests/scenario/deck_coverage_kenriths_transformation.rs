//! Exact pinned-deck coverage for Kenrith's Transformation.
//!
//! Oracle and rulings checked 2026-09-30. CR 303.4a governs the Aura spell's creature target;
//! CR 608.2b governs an illegal target at resolution. CR 613.1d-g govern the attached type,
//! color, ability-removal, and base power/toughness effects.

use crate::helpers::*;
use tricerules_cards::{Color, CounterKind, Keyword};
use tricerules_core::Zone;

const TRANSFORMATION: &str = "kenriths_transformation";

fn transformation_engine(seed: u64) -> GameEngine {
    let deck = deck_with(
        "forest",
        &[
            TRANSFORMATION,
            TRANSFORMATION,
            "zetalpa,_primal_dawn",
            "clockwork_percussionist",
            "flight",
            "forest",
        ],
    );
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1],
        20,
        Some(vec![deck.clone(), deck]),
        true,
    )
    .expect("new Kenrith's Transformation game");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

#[test]
fn kenrith_transformation_draws_and_sets_the_enchanted_creature_characteristics() {
    let mut engine = transformation_engine(20_260_930);
    let legendary = relocate_to_battlefield(&mut engine, 0, "zetalpa,_primal_dawn", false);
    let artifact = relocate_to_battlefield(&mut engine, 0, "clockwork_percussionist", false);
    engine
        .state
        .objects
        .get_mut(&legendary)
        .expect("Zetalpa")
        .add_counters(CounterKind::PlusOnePlusOne, 1, engine.state.command_index);
    let drawn = authoring_fixture::library_top(&mut engine, 0, &["island", "mountain"]);
    ensure_card_in_hand(&mut engine, 0, TRANSFORMATION);
    take_card_from_library_to_hand(&mut engine, 0, TRANSFORMATION);
    let hand_before = engine.state.players[0].hand.len();
    give_mana(
        &mut engine,
        0,
        ManaGift {
            g: 2,
            c: 2,
            ..Default::default()
        },
    );

    for target in [legendary, artifact] {
        let slot = hand_index_for_card(&engine, 0, TRANSFORMATION);
        engine
            .apply_command(0, &cast_spell(slot, target_object(target)))
            .expect("cast Kenrith's Transformation at a creature");
        resolve_entire_stack_two_player(&mut engine);
    }

    assert_eq!(engine.state.players[0].hand.len(), hand_before);
    assert!(drawn
        .iter()
        .all(|object| engine.state.players[0].hand.contains(object)));
    let transformed_legendary = engine.characteristics(legendary).expect("Zetalpa");
    assert_eq!(transformed_legendary.names, vec!["Zetalpa, Primal Dawn"]);
    assert_eq!(transformed_legendary.supertypes, vec!["Legendary"]);
    assert_eq!(transformed_legendary.types, vec!["Creature", "Elk"]);
    assert_eq!(transformed_legendary.colors, vec![Color::Green]);
    assert!(transformed_legendary.keywords.is_empty());
    assert_eq!(
        zone_view_rules_annotation_labels(&mut engine, 0, legendary),
        vec!["Loses all abilities"]
    );
    assert_eq!(
        (transformed_legendary.power, transformed_legendary.toughness),
        (Some(4), Some(4))
    );
    assert_eq!(
        engine.state.objects[&legendary].counter_count(CounterKind::PlusOnePlusOne),
        1
    );

    let transformed_artifact = engine
        .characteristics(artifact)
        .expect("Clockwork Percussionist");
    assert_eq!(transformed_artifact.types, vec!["Creature", "Elk"]);
    assert_eq!(transformed_artifact.colors, vec![Color::Green]);
    assert!(transformed_artifact.keywords.is_empty());
    assert_eq!(
        zone_view_rules_annotation_labels(&mut engine, 0, artifact),
        vec!["Loses all abilities"]
    );
    assert!(!transformed_artifact
        .types
        .iter()
        .any(|kind| kind == "Artifact"));
    assert_eq!(
        (transformed_artifact.power, transformed_artifact.toughness),
        (Some(3), Some(3))
    );
    for target in [legendary, artifact] {
        assert!(engine.state.objects.values().any(|object| {
            object.card_id == TRANSFORMATION
                && object.zone == Zone::Battlefield
                && object.attached_to == Some(tricerules_core::AttachmentRecipient::Object(target))
        }));
    }

    ensure_card_in_hand(&mut engine, 0, "flight");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    let flight_slot = hand_index_for_card(&engine, 0, "flight");
    engine
        .apply_command(0, &cast_spell(flight_slot, target_object(legendary)))
        .expect("grant an ability after Kenrith's Transformation");
    resolve_entire_stack_two_player(&mut engine);
    assert!(engine.effective_has_keyword(legendary, Keyword::Flying));
}

#[test]
fn kenrith_transformation_does_not_enter_or_draw_if_its_target_becomes_illegal() {
    let mut engine = transformation_engine(20_260_931);
    let target = relocate_to_battlefield(&mut engine, 0, "zetalpa,_primal_dawn", false);
    let drawn = authoring_fixture::library_top(&mut engine, 0, &["island"]);
    ensure_card_in_hand(&mut engine, 0, TRANSFORMATION);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            g: 1,
            c: 1,
            ..Default::default()
        },
    );
    let hand_before = engine.state.players[0].hand.len();
    let slot = hand_index_for_card(&engine, 0, TRANSFORMATION);
    engine
        .apply_command(0, &cast_spell(slot, target_object(target)))
        .expect("cast Kenrith's Transformation at a legal creature");
    engine.state.players[0]
        .battlefield
        .retain(|object| *object != target);
    engine.state.players[0].graveyard.push(target);
    engine.state.objects.get_mut(&target).unwrap().zone = Zone::Graveyard;
    *engine
        .state
        .zone_change_generation
        .entry(target)
        .or_default() += 1;

    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(engine.state.objects[&target].zone, Zone::Graveyard);
    assert_eq!(engine.state.players[0].hand.len(), hand_before - 1);
    assert!(drawn
        .iter()
        .all(|object| !engine.state.players[0].hand.contains(object)));
    assert!(engine
        .state
        .objects
        .values()
        .any(|object| { object.card_id == TRANSFORMATION && object.zone == Zone::Graveyard }));
}

#[test]
fn kenrith_transformation_rejects_a_noncreature_spell_target() {
    let mut engine = transformation_engine(20_260_932);
    let land = relocate_to_battlefield(&mut engine, 0, "forest", false);
    ensure_card_in_hand(&mut engine, 0, TRANSFORMATION);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            g: 1,
            c: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, TRANSFORMATION);

    engine
        .apply_command(0, &cast_spell(slot, target_object(land)))
        .expect_err("a land is not a legal Enchant creature target");

    assert_eq!(engine.state.objects[&land].zone, Zone::Battlefield);
    assert!(engine.state.players[0]
        .hand
        .iter()
        .any(|object| engine.state.objects[object].card_id == TRANSFORMATION));
    assert!(engine.state.stack.is_empty());
}
