//! Frozen four-deck coverage for Liquimetal Torque.
//!
//! Oracle and rulings checked against Scryfall on 2026-09-28; the rulings endpoint returned no
//! entries. CR 605.1a and 605.3b govern the colorless mana ability, CR 205.1b and 613.1d govern
//! adding Artifact while retaining other types, and CR 514.2 governs the end-of-turn duration.

use super::helpers::*;
use tricerules_core::{GameEngine, Zone};

fn torque_engine(seed: u64) -> (GameEngine, u32) {
    let decks = Some(vec![deck_with("island", &[]), deck_with("forest", &[])]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    let torque = inject_permanent_on_battlefield(&mut engine, 0, "liquimetal_torque");
    (engine, torque)
}

#[test]
fn liquimetal_torque_taps_for_colorless_without_using_the_stack() {
    let (mut engine, torque) = torque_engine(20_260_928);

    apply_ability(&mut engine, 0, torque, 0, vec![]).expect("activate Torque's mana ability");

    assert_eq!(engine.state.players[0].mana_pool.colorless, 1);
    assert!(engine.state.objects[&torque].tapped);
    assert_eq!(engine.state.objects[&torque].zone, Zone::Battlefield);
    assert!(
        engine.state.stack.is_empty(),
        "the mana ability resolves immediately"
    );
}

#[test]
fn liquimetal_torque_targets_nonland_and_adds_artifact_type() {
    let (mut engine, torque) = torque_engine(20_260_929);
    let bear = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let land = inject_permanent_on_battlefield(&mut engine, 1, "mountain");

    apply_ability(&mut engine, 0, torque, 1, target_object(land))
        .expect_err("Liquimetal Torque cannot target a land");
    assert!(
        !engine.state.objects[&torque].tapped,
        "an illegal target does not pay the tap cost"
    );
    assert!(engine.state.stack.is_empty());
    assert!(!engine
        .characteristics(land)
        .expect("land")
        .has_type("Artifact"));

    apply_ability(&mut engine, 0, torque, 1, target_object(bear))
        .expect("activate Liquimetal Torque targeting a nonland permanent");
    assert!(engine.state.objects[&torque].tapped);
    assert_eq!(
        engine.state.stack.len(),
        1,
        "the type-changing ability uses the stack"
    );
    resolve_entire_stack_two_player(&mut engine);

    let modified = engine.characteristics(bear).expect("modified creature");
    assert!(modified.has_type("Creature"));
    assert!(modified.has_type("Artifact"));
    assert!(modified.has_type("Bear"));
    assert!(!engine
        .characteristics(land)
        .expect("land")
        .has_type("Artifact"));
}
