//! Actual-card coverage for Prosperity's X-based draw for every player.

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

fn advance_three_player_game_to_main1(engine: &mut GameEngine) {
    assert_eq!(engine.state.turn_step, TurnStep::Upkeep);
    pass_all_active_players(engine);
    pass_all_active_players(engine);
    assert_eq!(engine.state.turn_step, TurnStep::Main1);
}

fn resolve_three_player_stack(engine: &mut GameEngine) -> Vec<String> {
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

fn three_player_engine(seed: u64) -> GameEngine {
    let decks = Some(vec![
        deck_with("island", &["prosperity"]),
        island_only_deck(),
        island_only_deck(),
    ]);
    GameEngine::new(seed, &[0, 1, 2], 40, decks, true).expect("new three-player game")
}

fn resolve_two_player_stack_and_capture_logs(engine: &mut GameEngine) -> Vec<String> {
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

#[test]
fn prosperity_pays_x_plus_blue_and_each_player_draws_x_in_apnap_order() {
    let mut engine = three_player_engine(202_609_261);
    ensure_card_in_hand(&mut engine, 0, "prosperity");
    advance_three_player_game_to_main1(&mut engine);

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
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            c: 2,
            ..Default::default()
        },
    );

    let hand_index = hand_index_for_card(&engine, 0, "prosperity");
    let spell = engine
        .apply_command(0, &cast_spell_x(hand_index, Vec::new(), 2))
        .expect("cast Prosperity with X=2");
    let stack_item = engine.state.stack.last().expect("Prosperity on stack");
    assert_eq!(stack_item.chosen_x, 2);
    assert_eq!(engine.state.players[0].mana_pool.blue, 0);
    assert_eq!(engine.state.players[0].mana_pool.colorless, 0);
    let stack_push = spell
        .events
        .iter()
        .find_map(|event| match &event.ev {
            Some(tricerules_proto::ruled::v1::ruled_event::Ev::StackPushed(pushed)) => Some(pushed),
            _ => None,
        })
        .expect("Prosperity stack event");
    assert_eq!(stack_push.ability_annotation, "X = 2");

    let logs = resolve_three_player_stack(&mut engine);

    let draw_logs: Vec<_> = logs
        .iter()
        .filter(|log| log.contains("draws 2 cards") && log.contains("Prosperity"))
        .map(String::as_str)
        .collect();
    assert_eq!(
        draw_logs,
        [
            "P0 draws 2 cards (Prosperity).",
            "P1 draws 2 cards (Prosperity).",
            "P2 draws 2 cards (Prosperity).",
        ],
        "each player completes their draws in APNAP order"
    );

    for index in 0..3 {
        assert_eq!(
            engine.state.players[index].library.len(),
            library_sizes[index] - 2,
            "player {index} draws X cards"
        );
        let expected_hand = hand_sizes[index] + if index == 0 { 1 } else { 2 };
        assert_eq!(
            engine.state.players[index].hand.len(),
            expected_hand,
            "player {index} receives the drawn cards"
        );
    }
}

#[test]
fn prosperity_with_x_zero_draws_no_cards_but_still_pays_blue() {
    let decks = Some(vec![
        deck_with("island", &["prosperity"]),
        island_only_deck(),
    ]);
    let mut engine =
        GameEngine::new(202_609_262, &[0, 1], 20, decks, true).expect("new two-player game");
    ensure_card_in_hand(&mut engine, 0, "prosperity");
    advance_to_main1_from_game_start(&mut engine);
    let libraries: Vec<_> = engine
        .state
        .players
        .iter()
        .map(|player| player.library.len())
        .collect();
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );

    let hand_index = hand_index_for_card(&engine, 0, "prosperity");
    engine
        .apply_command(0, &cast_spell_x(hand_index, Vec::new(), 0))
        .expect("cast Prosperity with X=0");
    assert_eq!(engine.state.stack.last().unwrap().chosen_x, 0);
    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(engine.state.players[0].mana_pool.blue, 0);
    assert_eq!(engine.state.players[0].mana_pool.colorless, 0);
    for (index, library_size) in libraries.into_iter().enumerate() {
        assert_eq!(engine.state.players[index].library.len(), library_size);
    }
}

#[test]
fn prosperity_can_make_all_remaining_players_lose_simultaneously() {
    let decks = Some(vec![
        deck_with("island", &["prosperity"]),
        island_only_deck(),
    ]);
    let mut engine =
        GameEngine::new(202_609_263, &[0, 1], 20, decks, true).expect("new two-player game");
    ensure_card_in_hand(&mut engine, 0, "prosperity");
    advance_to_main1_from_game_start(&mut engine);
    for player in 0..2 {
        engine.state.players[player].library.clear();
        inject_library_card(&mut engine, player, "island");
    }
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            c: 2,
            ..Default::default()
        },
    );

    let hand_index = hand_index_for_card(&engine, 0, "prosperity");
    engine
        .apply_command(0, &cast_spell_x(hand_index, Vec::new(), 2))
        .expect("cast Prosperity with X=2");
    let logs = resolve_two_player_stack_and_capture_logs(&mut engine);

    assert!(engine.state.players.iter().all(|player| player.has_lost));
    assert!(engine.state.winner.is_none(), "the game is a draw");
    for player in 0..2 {
        assert!(logs.iter().any(|log| {
            log.starts_with(&format!("P{player} draws 1 card")) && log.contains("Prosperity")
        }));
        assert!(logs
            .iter()
            .any(|log| log.starts_with(&format!("P{player} attempted to draw more cards"))));
    }
    assert!(engine
        .state
        .players
        .iter()
        .all(|player| !player.pending_library_loss));
}
