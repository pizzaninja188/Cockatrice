//! Complete-card scenarios for Anvil of Bogardan in the pinned deck corpus.
//!
//! CR 402.2 sets the default maximum hand size, CR 504.2 puts the normal draw before draw-step
//! triggers, and CR 514.1 applies maximum hand size during cleanup.

use super::helpers::*;
use tricerules_core::{GameEngine, TurnStep, Zone};

fn two_player_engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("plains", &[]), deck_with("island", &[])]);
    let mut engine = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn cast_anvil(engine: &mut GameEngine) -> u32 {
    let source = inject_card_into_hand(engine, 0, "anvil_of_bogardan");
    let slot = hand_index_for_card(engine, 0, "anvil_of_bogardan");
    engine.state.players[0].mana_pool.colorless = 2;
    engine
        .apply_command(0, &cast_spell(slot, vec![]))
        .expect("cast Anvil of Bogardan");
    resolve_entire_stack_two_player(engine);
    assert_eq!(engine.state.objects[&source].zone, Zone::Battlefield);
    source
}

#[test]
fn anvil_of_bogardan_removes_both_players_hand_limits_at_cleanup() {
    let mut engine = two_player_engine(20_260_955);
    cast_anvil(&mut engine);

    for player in 0..2 {
        while engine.state.players[player].hand.len() < 9 {
            inject_card_into_hand(&mut engine, player, "forest");
        }
    }

    engine.state.turn_step = TurnStep::EndStep;
    engine
        .apply_command(0, &pass())
        .expect("P0 passes end step");
    engine
        .apply_command(1, &pass())
        .expect("P1 passes end step");
    assert_eq!(engine.state.cleanup_discard_player, None);
    assert_eq!(engine.state.players[0].hand.len(), 9);
    assert_eq!(engine.state.active_player_id(), 1);

    engine.state.turn_step = TurnStep::EndStep;
    engine
        .apply_command(1, &pass())
        .expect("P1 passes end step");
    engine
        .apply_command(0, &pass())
        .expect("P0 passes end step");
    assert_eq!(engine.state.cleanup_discard_player, None);
    assert_eq!(engine.state.players[1].hand.len(), 9);
}

#[test]
fn anvil_of_bogardan_draws_then_each_triggered_player_discards() {
    let mut engine = two_player_engine(20_260_956);
    cast_anvil(&mut engine);
    inject_permanent_on_battlefield(&mut engine, 0, "anvil_of_bogardan");

    let initial_hand = engine.state.players[1].hand.len();
    let first_discard = engine.state.players[1].hand[0];
    let second_discard = engine.state.players[1].hand[1];
    let library_before = engine.state.players[1].library.len();

    end_active_turn(&mut engine, 0);
    pass_both_players(&mut engine); // P1's normal draw happens before its two Anvil triggers.
    assert_eq!(engine.state.active_player_id(), 1);
    assert_eq!(engine.state.players[1].hand.len(), initial_hand + 1);
    assert_eq!(engine.state.players[1].library.len(), library_before - 1);
    assert!(engine.state.pending_trigger_order.is_some());
    answer_trigger_order_in_engine_order(&mut engine);
    assert_eq!(
        engine.state.stack.len(),
        2,
        "both Anvil triggers are placed after the order prompt"
    );

    pass_both_players(&mut engine);
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .deciding_player,
        1
    );
    assert!(engine
        .state
        .pending_resolution
        .as_ref()
        .unwrap()
        .presentation
        .candidates
        .contains(&first_discard));
    engine
        .apply_command(1, &submit_resolution_choice(vec![first_discard]))
        .expect("P1 chooses the first discard");
    assert_eq!(engine.state.objects[&first_discard].zone, Zone::Graveyard);
    assert_eq!(engine.state.players[1].hand.len(), initial_hand + 1);

    pass_both_players(&mut engine);
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .deciding_player,
        1
    );
    engine
        .apply_command(1, &submit_resolution_choice(vec![second_discard]))
        .expect("P1 chooses the second discard");
    assert_eq!(engine.state.objects[&second_discard].zone, Zone::Graveyard);
    assert_eq!(engine.state.players[1].hand.len(), initial_hand + 1);
    assert_eq!(engine.state.players[1].library.len(), library_before - 3);
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
}
