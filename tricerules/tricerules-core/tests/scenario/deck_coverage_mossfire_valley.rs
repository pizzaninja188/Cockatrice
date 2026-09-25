//! Exact deck-corpus coverage for Mossfire Valley.
//!
//! Oracle and rulings checked 2026-09-25. CR 605.1a classifies its untargeted mana-producing
//! activated ability as a mana ability, and CR 605.3b makes it resolve immediately without using
//! the stack.

use super::helpers::*;
use tricerules_core::GameEngine;

#[test]
fn mossfire_valley_pays_one_generic_and_produces_red_and_green_together() {
    let mut engine = GameEngine::new(
        20_260_944,
        &[0, 1],
        20,
        Some(vec![deck_with("island", &[]), deck_with("forest", &[])]),
        true,
    )
    .expect("new game");
    advance_to_main1_from_game_start(&mut engine);

    inject_card_into_hand(&mut engine, 0, "mossfire_valley");
    let slot = hand_index_for_card(&engine, 0, "mossfire_valley");
    engine
        .apply_command(0, &play_land(slot))
        .expect("play Mossfire Valley as a land");
    let valley = battlefield_object_for_card(&engine, 0, "mossfire_valley");
    assert!(!engine.state.objects[&valley].tapped);

    let before_unpaid_activation = engine.state.command_index;
    assert!(apply_ability(&mut engine, 0, valley, 0, vec![]).is_err());
    assert_eq!(engine.state.command_index, before_unpaid_activation);
    assert!(!engine.state.objects[&valley].tapped);

    engine.state.players[0].mana_pool.colorless = 1;
    apply_ability(&mut engine, 0, valley, 0, vec![])
        .expect("pay {1} and tap to activate Mossfire Valley");
    let pool = &engine.state.players[0].mana_pool;
    assert_eq!(
        (
            pool.white,
            pool.blue,
            pool.black,
            pool.red,
            pool.green,
            pool.colorless,
        ),
        (0, 0, 0, 1, 1, 0)
    );
    assert!(engine.state.objects[&valley].tapped);
    assert!(
        engine.state.stack.is_empty(),
        "mana ability does not use the stack"
    );

    let mana_after_activation = (
        pool.white,
        pool.blue,
        pool.black,
        pool.red,
        pool.green,
        pool.colorless,
    );
    assert!(apply_ability(&mut engine, 0, valley, 0, vec![]).is_err());
    let pool = &engine.state.players[0].mana_pool;
    assert_eq!(
        (
            pool.white,
            pool.blue,
            pool.black,
            pool.red,
            pool.green,
            pool.colorless,
        ),
        mana_after_activation
    );
}
