//! Actual-card scenarios for Clock of Omens' selected artifact taps and untap target.
//!
//! CR 602.2b and 601.2c/601.2h govern activation, target selection, and atomic cost payment;
//! CR 115.1c and 701.26 govern its target and untap effect. The generic
//! `selected_tap_payment_does_not_require_a_ready_source` engine regression covers summoning-sick
//! objects paying a verbal tap cost, so this card suite focuses on Clock's artifact filter and
//! exact two-object payment rather than duplicating that readiness test.

use super::helpers::*;
use tricerules_core::GameEngine;
use tricerules_proto::ruled::v1::{
    cost_selection::Selection, CostObjectRef, CostObjectRefs, CostSelection,
};

fn clock_engine(seed: u64) -> (GameEngine, u32) {
    let decks = Some(vec![
        deck_with("island", &["clock_of_omens", "sol_ring", "mind_stone"]),
        deck_with("forest", &["voltaic_key"]),
    ]);
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
    let clock = relocate_to_battlefield(&mut engine, 0, "clock_of_omens", false);
    (engine, clock)
}

fn tap_selection(engine: &GameEngine, objects: &[u32]) -> CostSelection {
    CostSelection {
        cost_index: 0,
        selection: Some(Selection::BattlefieldObjects(CostObjectRefs {
            objects: objects
                .iter()
                .map(|object_id| CostObjectRef {
                    object_id: *object_id,
                    zone_change_generation: engine
                        .state
                        .zone_change_generation
                        .get(object_id)
                        .copied()
                        .unwrap_or(0),
                })
                .collect(),
        })),
    }
}

#[test]
fn clock_taps_two_artifacts_as_cost_then_untaps_a_tapped_opponents_artifact() {
    let (mut engine, clock) = clock_engine(202_609_271);
    let ring = relocate_to_battlefield(&mut engine, 0, "sol_ring", false);
    let stone = relocate_to_battlefield(&mut engine, 0, "mind_stone", false);
    let opposing_artifact = relocate_to_battlefield(&mut engine, 1, "voltaic_key", true);
    engine.state.players[0].mana_pool.colorless = 3;

    let costs = tap_selection(&engine, &[ring, stone]);
    let command =
        activate_ability_with_costs(clock, 0, target_object(opposing_artifact), vec![costs]);
    engine
        .apply_command(0, &command)
        .expect("two untapped controlled artifacts pay the activation");

    assert!(engine.state.objects[&ring].tapped);
    assert!(engine.state.objects[&stone].tapped);
    assert!(
        !engine.state.objects[&clock].tapped,
        "Clock is not tapped as a separate source cost"
    );
    assert!(
        engine.state.objects[&opposing_artifact].tapped,
        "the target remains tapped while the ability is pending"
    );
    assert_eq!(engine.state.players[0].mana_pool.colorless, 3);
    assert_eq!(engine.state.stack.len(), 1);

    resolve_entire_stack_two_player(&mut engine);

    assert!(engine.state.objects[&ring].tapped);
    assert!(engine.state.objects[&stone].tapped);
    assert!(
        !engine.state.objects[&opposing_artifact].tapped,
        "resolution untaps the opponent's artifact"
    );
}

#[test]
fn clock_can_be_one_of_its_two_cost_artifacts_and_its_own_target() {
    let (mut engine, clock) = clock_engine(202_609_272);
    let ring = relocate_to_battlefield(&mut engine, 0, "sol_ring", false);

    let costs = tap_selection(&engine, &[clock, ring]);
    let command = activate_ability_with_costs(clock, 0, target_object(clock), vec![costs]);
    engine
        .apply_command(0, &command)
        .expect("Clock can be tapped for its cost and chosen as the artifact target");

    assert!(engine.state.objects[&clock].tapped);
    assert!(engine.state.objects[&ring].tapped);
    assert_eq!(engine.state.stack.len(), 1);

    resolve_entire_stack_two_player(&mut engine);

    assert!(
        !engine.state.objects[&clock].tapped,
        "the resolving ability untaps its source as the chosen target"
    );
    assert!(engine.state.objects[&ring].tapped);
}

#[test]
fn clock_rejects_incomplete_duplicate_foreign_and_nonartifact_cost_selections_atomically() {
    let (mut engine, clock) = clock_engine(202_609_273);
    let ring = relocate_to_battlefield(&mut engine, 0, "sol_ring", false);
    let stone = relocate_to_battlefield(&mut engine, 0, "mind_stone", false);
    let island = relocate_to_battlefield(&mut engine, 0, "island", false);
    let opposing_artifact = relocate_to_battlefield(&mut engine, 1, "voltaic_key", false);

    let invalid_payments = [
        tap_selection(&engine, &[ring]),
        tap_selection(&engine, &[ring, ring]),
        tap_selection(&engine, &[ring, opposing_artifact]),
        tap_selection(&engine, &[ring, island]),
        tap_selection(&engine, &[clock, ring, stone]),
    ];
    for costs in invalid_payments {
        let command =
            activate_ability_with_costs(clock, 0, target_object(opposing_artifact), vec![costs]);
        let error = engine
            .apply_command(0, &command)
            .expect_err("the payment must contain two distinct untapped artifacts you control");
        assert!(matches!(error, tricerules_core::EngineError::Illegal(_)));
        assert!(!engine.state.objects[&ring].tapped);
        assert!(!engine.state.objects[&stone].tapped);
        assert!(!engine.state.objects[&clock].tapped);
        assert!(!engine.state.objects[&opposing_artifact].tapped);
        assert!(engine.state.stack.is_empty());
    }

    engine
        .state
        .objects
        .get_mut(&stone)
        .expect("Mind Stone")
        .tapped = true;
    let costs = tap_selection(&engine, &[ring, stone]);
    let command =
        activate_ability_with_costs(clock, 0, target_object(opposing_artifact), vec![costs]);
    let error = engine
        .apply_command(0, &command)
        .expect_err("a tapped artifact cannot pay the tap-permanents cost");
    assert!(matches!(error, tricerules_core::EngineError::Illegal(_)));
    assert!(!engine.state.objects[&ring].tapped);
    assert!(engine.state.objects[&stone].tapped);
    assert!(!engine.state.objects[&clock].tapped);
    assert!(engine.state.stack.is_empty());
}

#[test]
fn clock_rejects_a_nonartifact_target_without_tapping_its_cost_artifacts() {
    let (mut engine, clock) = clock_engine(202_609_274);
    let ring = relocate_to_battlefield(&mut engine, 0, "sol_ring", false);
    let stone = relocate_to_battlefield(&mut engine, 0, "mind_stone", false);
    let opposing_land = relocate_to_battlefield(&mut engine, 1, "forest", false);

    let costs = tap_selection(&engine, &[ring, stone]);
    let command = activate_ability_with_costs(clock, 0, target_object(opposing_land), vec![costs]);
    let error = engine
        .apply_command(0, &command)
        .expect_err("a nonartifact permanent is not a legal target");

    assert!(matches!(error, tricerules_core::EngineError::Illegal(_)));
    assert!(!engine.state.objects[&ring].tapped);
    assert!(!engine.state.objects[&stone].tapped);
    assert!(!engine.state.objects[&clock].tapped);
    assert!(engine.state.stack.is_empty());
}

#[test]
fn clock_can_target_an_already_untapped_artifact() {
    let (mut engine, clock) = clock_engine(202_609_275);
    let ring = relocate_to_battlefield(&mut engine, 0, "sol_ring", false);
    let stone = relocate_to_battlefield(&mut engine, 0, "mind_stone", false);
    let opposing_artifact = relocate_to_battlefield(&mut engine, 1, "voltaic_key", false);

    let costs = tap_selection(&engine, &[ring, stone]);
    let command =
        activate_ability_with_costs(clock, 0, target_object(opposing_artifact), vec![costs]);
    engine
        .apply_command(0, &command)
        .expect("an untapped artifact remains a legal target");
    assert!(engine.state.objects[&ring].tapped);
    assert!(engine.state.objects[&stone].tapped);
    assert!(!engine.state.objects[&opposing_artifact].tapped);
    assert_eq!(engine.state.stack.len(), 1);

    resolve_entire_stack_two_player(&mut engine);

    assert!(!engine.state.objects[&opposing_artifact].tapped);
}
