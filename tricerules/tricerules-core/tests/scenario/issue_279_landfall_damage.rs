//! Issue #279 — Landfall damage is controller-scoped and damages each opponent exactly once.
//! Oracle and rulings were checked 2026-09-14. CR 207.2c, 603.6a, 120.2b, and 120.3a govern
//! the ability word, land entering triggers, and damage rather than life loss.

use super::helpers::*;
use tricerules_core::{TurnStep, Zone};

fn three_player_main1(seed: u64) -> GameEngine {
    let mut engine = GameEngine::new(seed, &[0, 1], 20, None, true).expect("engine");
    engine
        .state
        .players
        .push(tricerules_core::state::PlayerState::new(2, 20));
    while engine.state.turn_step != TurnStep::Main1 {
        let player = engine.state.priority_player_id();
        engine
            .apply_command(player, &pass())
            .expect("advance three-player turn");
    }
    engine
}

fn resolve_entire_stack_three_player(engine: &mut GameEngine) {
    loop {
        answer_trigger_order_in_engine_order(engine);
        if engine.state.stack.is_empty() {
            break;
        }
        let player_count = engine.state.players.len();
        for _ in 0..player_count {
            let player = engine.state.priority_player_id();
            engine
                .apply_command(player, &pass())
                .expect("three-player priority pass");
            if engine.state.stack.is_empty() {
                break;
            }
        }
    }
}

#[test]
fn issue_279_controller_landfall_damages_both_opponents_not_controller() {
    let mut engine = three_player_main1(279_001);
    inject_card_into_hand(&mut engine, 0, "sabotender");
    let source = move_ready_to_battlefield(&mut engine, 0, "sabotender");

    inject_card_into_hand(&mut engine, 0, "mountain");
    let controller_land = move_ready_to_battlefield(&mut engine, 0, "mountain");
    resolve_entire_stack_three_player(&mut engine);

    assert_eq!(engine.state.players[0].life, 20);
    assert_eq!(engine.state.players[1].life, 19);
    assert_eq!(engine.state.players[2].life, 19);
    assert_eq!(engine.state.objects[&source].zone, Zone::Battlefield);
    assert_eq!(
        engine.state.objects[&controller_land].zone,
        Zone::Battlefield
    );

    inject_card_into_hand(&mut engine, 1, "mountain");
    let opponent_land = move_ready_to_battlefield(&mut engine, 1, "mountain");
    resolve_entire_stack_three_player(&mut engine);

    assert_eq!(engine.state.players[0].life, 20);
    assert_eq!(engine.state.players[1].life, 19);
    assert_eq!(engine.state.players[2].life, 19);
    assert_eq!(engine.state.objects[&opponent_land].zone, Zone::Battlefield);
}
