use super::helpers::*;
use tricerules_core::{GameEngine, Zone};

fn winged_words_action_cost(engine: &mut GameEngine) -> String {
    let hand_index = hand_index_for_card(engine, 0, "winged_words") as u32;
    engine.initial_response_batch().legal_by_player[&0]
        .hand_actions
        .iter()
        .find(|action| action.hand_index == hand_index)
        .expect("Winged Words cast action")
        .cost
        .clone()
}

#[test]
fn winged_words_uses_the_flying_reduction_for_preview_and_payment() {
    let decks = Some(vec![
        deck_with("island", &["winged_words", "cloudkin_seer"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(56_001, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    ensure_in_hand(&mut engine, 0, "winged_words");
    relocate_to_battlefield(&mut engine, 0, "cloudkin_seer", false);

    assert_eq!(winged_words_action_cost(&mut engine), "{1}{U}");

    engine.state.players[0].mana_pool.blue = 1;
    engine.state.players[0].mana_pool.colorless = 1;
    let hand_index = hand_index_for_card(&engine, 0, "winged_words");
    engine
        .apply_command(0, &cast_spell(hand_index, vec![]))
        .expect("reduced Winged Words cast");

    assert_eq!(engine.state.players[0].mana_pool.blue, 0);
    assert_eq!(engine.state.players[0].mana_pool.colorless, 0);
    let hand_size_after_cast = engine.state.players[0].hand.len();
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.players[0].hand.len(), hand_size_after_cast + 2);
}

#[test]
fn winged_words_counts_only_its_controllers_flyers_and_reduces_only_once() {
    let decks = Some(vec![
        deck_with(
            "island",
            &["winged_words", "cloudkin_seer", "cloudkin_seer"],
        ),
        deck_with("forest", &["cloudkin_seer"]),
    ]);
    let mut engine = GameEngine::new(56_002, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    ensure_in_hand(&mut engine, 0, "winged_words");

    assert_eq!(winged_words_action_cost(&mut engine), "{2}{U}");
    relocate_to_battlefield(&mut engine, 1, "cloudkin_seer", false);
    assert_eq!(winged_words_action_cost(&mut engine), "{2}{U}");
    relocate_to_battlefield(&mut engine, 0, "cloudkin_seer", false);
    assert_eq!(winged_words_action_cost(&mut engine), "{1}{U}");
    relocate_to_battlefield(&mut engine, 0, "cloudkin_seer", false);
    assert_eq!(winged_words_action_cost(&mut engine), "{1}{U}");
}

#[test]
fn winged_words_tracks_gained_and_lost_flying_and_revalidates_payment_atomically() {
    let decks = Some(vec![
        deck_with("island", &["winged_words", "grizzly_bears", "flight"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(56_003, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    ensure_in_hand(&mut engine, 0, "winged_words");
    ensure_in_hand(&mut engine, 0, "flight");
    let bear = relocate_to_battlefield(&mut engine, 0, "grizzly_bears", false);

    assert_eq!(winged_words_action_cost(&mut engine), "{2}{U}");
    grant_pool(&mut engine, 0);
    let flight_index = hand_index_for_card(&engine, 0, "flight");
    engine
        .apply_command(0, &cast_spell(flight_index, target_object(bear)))
        .expect("cast Flight");
    resolve_entire_stack_two_player(&mut engine);
    engine.state.players[0].mana_pool.clear();
    assert_eq!(winged_words_action_cost(&mut engine), "{1}{U}");

    let battlefield_pos = engine.state.players[0]
        .battlefield
        .iter()
        .position(|oid| *oid == bear)
        .expect("bear remains on battlefield");
    engine.state.players[0].battlefield.remove(battlefield_pos);
    engine.state.players[0].graveyard.push(bear);
    engine
        .state
        .objects
        .get_mut(&bear)
        .expect("bear object")
        .zone = Zone::Graveyard;
    assert_eq!(winged_words_action_cost(&mut engine), "{2}{U}");

    engine.state.players[0].mana_pool.blue = 1;
    engine.state.players[0].mana_pool.colorless = 1;
    let hand_before = engine.state.players[0].hand.clone();
    let command_index_before = engine.state.command_index;
    let hand_index = hand_index_for_card(&engine, 0, "winged_words");
    assert!(engine
        .apply_command(0, &cast_spell(hand_index, vec![]))
        .is_err());
    assert_eq!(engine.state.players[0].hand, hand_before);
    assert_eq!(engine.state.command_index, command_index_before);
    assert_eq!(engine.state.players[0].mana_pool.blue, 1);
    assert_eq!(engine.state.players[0].mana_pool.colorless, 1);
}
