//! Exact deck-corpus coverage for Words of Wisdom.
//!
//! Oracle and rulings checked against the physical Odyssey printing on 2026-09-26; Scryfall
//! returned no rulings. CR 121.1-121.2c govern sequential draws and multiplayer draw order; CR
//! 121.4 and 704.5b govern an attempted draw from an empty library.

use super::helpers::*;
use tricerules_core::{GameEngine, TurnStep};

fn pass_all_active_players(engine: &mut GameEngine) {
    let active_players = engine
        .state
        .players
        .iter()
        .filter(|player| !player.has_lost)
        .count();
    for _ in 0..active_players {
        answer_trigger_order_in_engine_order(engine);
        let player = engine.state.priority_player_id();
        engine
            .apply_command(player, &pass())
            .expect("active player passes priority");
    }
}

fn advance_four_player_game_to_main1(engine: &mut GameEngine) {
    assert_eq!(engine.state.turn_step, TurnStep::Upkeep);
    pass_all_active_players(engine);
    pass_all_active_players(engine);
    assert_eq!(engine.state.turn_step, TurnStep::Main1);
}

fn resolve_four_player_stack(engine: &mut GameEngine) -> Vec<String> {
    let mut logs = Vec::new();
    while !engine.state.stack.is_empty() {
        answer_trigger_order_in_engine_order(engine);
        let player = engine.state.priority_player_id();
        let batch = engine
            .apply_command(player, &pass())
            .expect("active player passes priority");
        logs.extend(batch.events.into_iter().filter_map(|event| match event.ev {
            Some(tricerules_proto::ruled::v1::ruled_event::Ev::Log(log)) => Some(log.text),
            _ => None,
        }));
    }
    logs
}

fn four_player_engine(seed: u64) -> GameEngine {
    let decks = Some(vec![
        deck_with("island", &["words_of_wisdom"]),
        island_only_deck(),
        island_only_deck(),
        island_only_deck(),
    ]);
    GameEngine::new(seed, &[0, 1, 2, 3], 20, decks, true).expect("new four-player game")
}

#[test]
fn words_of_wisdom_draws_two_then_each_other_player_draws_one_in_order() {
    let mut engine = four_player_engine(202_609_263);
    ensure_card_in_hand(&mut engine, 0, "words_of_wisdom");
    advance_four_player_game_to_main1(&mut engine);

    let library_sizes: Vec<_> = engine
        .state
        .players
        .iter()
        .map(|player| player.library.len())
        .collect();
    let hand_sizes: Vec<_> = engine
        .state
        .players
        .iter()
        .map(|player| player.hand.len())
        .collect();
    let expected_draws: Vec<Vec<_>> = engine
        .state
        .players
        .iter()
        .enumerate()
        .map(|(index, player)| {
            player
                .library
                .iter()
                .take(if index == 0 { 2 } else { 1 })
                .copied()
                .collect()
        })
        .collect();
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            c: 1,
            ..Default::default()
        },
    );

    let hand_index = hand_index_for_card(&engine, 0, "words_of_wisdom");
    engine
        .apply_command(0, &cast_spell(hand_index, Vec::new()))
        .expect("cast Words of Wisdom");
    let logs = resolve_four_player_stack(&mut engine);

    let draw_logs: Vec<_> = logs
        .iter()
        .filter(|log| log.contains("draws ") && log.contains("(Words of Wisdom)"))
        .map(String::as_str)
        .collect();
    assert_eq!(
        draw_logs,
        [
            "P0 draws 2 cards (Words of Wisdom).",
            "P1 draws 1 card (Words of Wisdom).",
            "P2 draws 1 card (Words of Wisdom).",
            "P3 draws 1 card (Words of Wisdom).",
        ],
        "the controller finishes their two draws before each opponent draws in APNAP order"
    );

    for index in 0..4 {
        let draw_count = if index == 0 { 2 } else { 1 };
        let spent_spell = usize::from(index == 0);
        assert_eq!(
            engine.state.players[index].library.len(),
            library_sizes[index] - draw_count,
            "player {index} draws the instructed quantity"
        );
        let expected_hand = hand_sizes[index] + draw_count - spent_spell;
        assert_eq!(
            engine.state.players[index].hand.len(),
            expected_hand,
            "player {index} receives only their own draws"
        );
        for object_id in &expected_draws[index] {
            assert!(
                engine.state.players[index].hand.contains(object_id),
                "player {index} receives their own library card {object_id}"
            );
        }
        assert!(!engine.state.players[index].has_lost);
    }
    assert!(engine.state.stack.is_empty());
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(count_card_id_in_graveyard(&engine, 0, "words_of_wisdom"), 1);
}

#[test]
fn words_of_wisdom_rejects_a_cast_without_enough_mana() {
    let mut engine = four_player_engine(202_609_264);
    ensure_card_in_hand(&mut engine, 0, "words_of_wisdom");
    advance_four_player_game_to_main1(&mut engine);

    let hand_before = engine.state.players[0].hand.clone();
    let libraries_before: Vec<_> = engine
        .state
        .players
        .iter()
        .map(|player| player.library.clone())
        .collect();
    let command_index_before = engine.state.command_index;
    let hand_index = hand_index_for_card(&engine, 0, "words_of_wisdom");
    let error = engine
        .apply_command(0, &cast_spell(hand_index, Vec::new()))
        .expect_err("the {1}{U} spell needs both colored and generic mana");

    assert!(matches!(error, tricerules_core::EngineError::Illegal(_)));
    assert_eq!(engine.state.command_index, command_index_before);
    assert_eq!(engine.state.players[0].hand, hand_before);
    for (index, library) in libraries_before.iter().enumerate() {
        assert_eq!(engine.state.players[index].library, *library);
    }
    assert!(engine.state.stack.is_empty());
}
