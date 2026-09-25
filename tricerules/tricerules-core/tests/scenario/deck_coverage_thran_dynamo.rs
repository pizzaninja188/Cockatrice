//! Exact deck-corpus coverage for Thran Dynamo.
//!
//! Oracle and rulings checked 2026-09-25. CR 605.1a defines its tap ability as a mana ability,
//! CR 605.3b makes it resolve immediately without using the stack, and CR 106.1b defines the
//! six mana types, including colorless.

use super::helpers::*;
use tricerules_core::GameEngine;

#[test]
fn thran_dynamo_casts_then_adds_three_colorless_mana_without_using_the_stack() {
    let mut engine = GameEngine::new(
        20_260_943,
        &[0, 1],
        20,
        Some(vec![
            deck_with("island", &["thran_dynamo"]),
            forest_only_deck(),
        ]),
        true,
    )
    .expect("Thran Dynamo is registered");
    advance_to_main1_from_game_start(&mut engine);
    ensure_card_in_hand(&mut engine, 0, "thran_dynamo");
    engine.state.players[0].mana_pool.colorless = 4;

    let dynamo_in_hand = hand_index_for_card(&engine, 0, "thran_dynamo");
    engine
        .apply_command(0, &cast_spell(dynamo_in_hand, vec![]))
        .expect("cast Thran Dynamo for {4}");
    resolve_entire_stack_two_player(&mut engine);
    let dynamo = battlefield_object_for_card(&engine, 0, "thran_dynamo");
    assert_eq!(engine.state.players[0].mana_pool.colorless, 0);

    apply_ability(&mut engine, 0, dynamo, 0, vec![])
        .expect("Thran Dynamo's mana ability is legal while untapped");
    assert_eq!(engine.state.players[0].mana_pool.colorless, 3);
    assert!(engine.state.objects[&dynamo].tapped);
    assert!(
        engine.state.stack.is_empty(),
        "mana ability does not use the stack"
    );

    let mana_after_first_activation = engine.state.players[0].mana_pool.colorless;
    assert!(apply_ability(&mut engine, 0, dynamo, 0, vec![]).is_err());
    assert_eq!(
        engine.state.players[0].mana_pool.colorless, mana_after_first_activation,
        "a rejected activation while tapped produces no mana"
    );
}
