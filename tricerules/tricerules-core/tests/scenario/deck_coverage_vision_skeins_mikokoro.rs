//! Exact deck-corpus coverage for Vision Skeins and Mikokoro, Center of the Sea.
//!
//! Oracle and rulings checked 2026-09-25. CR 121.2c orders each player's draws active-player first.
//! Mikokoro's {T}: Add {C} ability is a mana ability under CR 605.1a/605.3b; its draw ability uses
//! the stack under CR 602.2.

use super::helpers::*;
use tricerules_core::{GameEngine, Zone};

fn two_player_engine(seed: u64) -> GameEngine {
    let mut engine = GameEngine::new(
        seed,
        &[0, 1],
        20,
        Some(vec![deck_with("island", &[]), deck_with("forest", &[])]),
        true,
    )
    .expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn play_mikokoro(engine: &mut GameEngine) -> u32 {
    let source = inject_card_into_hand(engine, 0, "mikokoro,_center_of_the_sea");
    let slot = hand_index_for_card(engine, 0, "mikokoro,_center_of_the_sea");
    engine
        .apply_command(0, &play_land(slot))
        .expect("play Mikokoro as a land");
    assert_eq!(engine.state.objects[&source].zone, Zone::Battlefield);
    source
}

fn library_top(engine: &GameEngine, player: usize, count: usize) -> Vec<u32> {
    engine.state.players[player]
        .library
        .iter()
        .take(count)
        .copied()
        .collect()
}

#[test]
fn vision_skeins_makes_each_player_draw_two_cards() {
    let mut engine = two_player_engine(20_260_945);
    let source = inject_card_into_hand(&mut engine, 0, "vision_skeins");
    let expected_draws = [library_top(&engine, 0, 2), library_top(&engine, 1, 2)];
    let hands_before = [
        engine.state.players[0].hand.len(),
        engine.state.players[1].hand.len(),
    ];
    engine.state.players[0].mana_pool.colorless = 1;
    engine.state.players[0].mana_pool.blue = 1;
    let slot = hand_index_for_card(&engine, 0, "vision_skeins");

    engine
        .apply_command(0, &cast_spell(slot, vec![]))
        .expect("cast Vision Skeins for {1}{U}");
    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(engine.state.objects[&source].zone, Zone::Graveyard);
    for player in 0..2 {
        assert_eq!(
            engine.state.players[player].hand.len(),
            hands_before[player] + if player == 0 { 1 } else { 2 },
            "player {player} draws two while P0's spell leaves their hand"
        );
        for object in &expected_draws[player] {
            assert!(engine.state.players[player].hand.contains(object));
        }
    }
    assert!(engine.state.stack.is_empty());
}

#[test]
fn mikokoro_mana_ability_adds_colorless_without_the_stack() {
    let mut engine = two_player_engine(20_260_946);
    let mikokoro = play_mikokoro(&mut engine);

    apply_ability(&mut engine, 0, mikokoro, 0, vec![])
        .expect("Mikokoro's first ability adds colorless mana");
    assert_eq!(engine.state.players[0].mana_pool.colorless, 1);
    assert!(engine.state.objects[&mikokoro].tapped);
    assert!(engine.state.stack.is_empty());
}

#[test]
fn mikokoro_pays_two_and_each_player_draws_one_on_the_stack() {
    let mut engine = two_player_engine(20_260_947);
    let mikokoro = play_mikokoro(&mut engine);
    let expected_draws = [library_top(&engine, 0, 1), library_top(&engine, 1, 1)];
    let command_index = engine.state.command_index;

    assert!(apply_ability(&mut engine, 0, mikokoro, 1, vec![]).is_err());
    assert_eq!(engine.state.command_index, command_index);
    assert!(!engine.state.objects[&mikokoro].tapped);

    engine.state.players[0].mana_pool.colorless = 2;
    apply_ability(&mut engine, 0, mikokoro, 1, vec![])
        .expect("pay {2} and tap to activate Mikokoro's draw ability");
    assert_eq!(engine.state.players[0].mana_pool.colorless, 0);
    assert!(engine.state.objects[&mikokoro].tapped);
    assert_eq!(engine.state.stack.len(), 1, "draw ability uses the stack");

    resolve_entire_stack_two_player(&mut engine);
    for (player, player_draws) in expected_draws.iter().enumerate() {
        assert_eq!(engine.state.players[player].hand.len(), 8);
        assert!(engine.state.players[player].hand.contains(&player_draws[0]));
    }
    assert!(engine.state.stack.is_empty());
}
