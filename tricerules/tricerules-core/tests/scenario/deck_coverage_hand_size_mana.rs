//! Exact deck-corpus coverage for four cards sharing the existing NoMaximumHandSize ability.
//!
//! Oracle text is from the pinned Scryfall corpus. CR 514.1 applies the maximum hand size during
//! cleanup; CR 604.1 describes static abilities as continuously true while they apply.

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

fn verify_no_maximum_hand_size(card_id: &str, seed: u64, is_land: bool, mana_cost: u32) {
    let mut engine = two_player_engine(seed);
    let source = inject_card_into_hand(&mut engine, 0, card_id);
    let slot = hand_index_for_card(&engine, 0, card_id);

    if is_land {
        engine
            .apply_command(0, &play_land(slot))
            .expect("play Reliquary Tower as a land");
    } else {
        engine.state.players[0].mana_pool.colorless = mana_cost;
        engine
            .apply_command(0, &cast_spell(slot, vec![]))
            .expect("cast the hand-size artifact");
        resolve_entire_stack_two_player(&mut engine);
    }
    assert_eq!(engine.state.objects[&source].zone, Zone::Battlefield);

    for player in 0..2 {
        while engine.state.players[player].hand.len() < 9 {
            inject_card_into_hand(&mut engine, player, "forest");
        }
    }

    engine.state.turn_step = tricerules_core::TurnStep::EndStep;
    engine
        .apply_command(0, &pass())
        .expect("pass the active player's end step");
    engine
        .apply_command(1, &pass())
        .expect("pass the opponent's end step");

    assert_eq!(engine.state.cleanup_discard_player, None);
    assert_eq!(engine.state.players[0].hand.len(), 9);
    assert_eq!(engine.state.active_player_id(), 1);

    engine.state.turn_step = tricerules_core::TurnStep::EndStep;
    engine
        .apply_command(1, &pass())
        .expect("pass the next active player's end step");
    engine
        .apply_command(0, &pass())
        .expect("pass priority to the next active player");
    assert_eq!(engine.state.cleanup_discard_player, Some(1));
}

#[test]
fn spellbook_removes_its_controllers_hand_limit_at_cleanup() {
    verify_no_maximum_hand_size("spellbook", 20_260_951, false, 0);
}

#[test]
fn decanter_of_endless_water_removes_its_controllers_hand_limit_at_cleanup() {
    verify_no_maximum_hand_size("decanter_of_endless_water", 20_260_952, false, 3);
}

#[test]
fn thought_vessel_removes_its_controllers_hand_limit_at_cleanup() {
    verify_no_maximum_hand_size("thought_vessel", 20_260_953, false, 2);
}

#[test]
fn reliquary_tower_removes_its_controllers_hand_limit_at_cleanup() {
    verify_no_maximum_hand_size("reliquary_tower", 20_260_954, true, 0);
}
