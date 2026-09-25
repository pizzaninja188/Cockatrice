//! Exact deck-corpus coverage for Ichor Wellspring.
//!
//! Oracle and rulings checked 2026-09-25. Its single triggered ability triggers on both entry
//! and battlefield-to-graveyard events; CR 603.6a, 603.6c, 603.10a, and 603.3a govern those events
//! and the event-time controller. CR 121.1 governs the draw instructions.

use super::helpers::*;
use tricerules_core::Zone;

const ICHOR_WELLSPRING: &str = "ichor_wellspring";
const DISENCHANT: &str = "disenchant";

#[test]
fn ichor_wellspring_draws_for_entry_and_graveyard_events_on_one_ability() {
    let decks = Some(vec![deck_with("island", &[]), deck_with("forest", &[])]);
    let mut engine = GameEngine::new(20_260_961, &[0, 1], 20, decks, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);

    let wellspring = inject_card_into_hand(&mut engine, 0, ICHOR_WELLSPRING);
    inject_card_into_hand(&mut engine, 0, DISENCHANT);
    let initial_hand = engine.state.players[0].hand.len();
    let opponent_hand = engine.state.players[1].hand.len();

    let unpaid = cast_spell(hand_index_for_card(&engine, 0, ICHOR_WELLSPRING), vec![]);
    let command_index = engine.state.command_index;
    engine
        .apply_command(0, &unpaid)
        .expect_err("Ichor Wellspring requires {2}");
    assert_eq!(engine.state.command_index, command_index);
    assert_eq!(engine.state.objects[&wellspring].zone, Zone::Hand);
    assert!(engine.state.stack.is_empty());

    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );
    let cast_wellspring = cast_spell(hand_index_for_card(&engine, 0, ICHOR_WELLSPRING), vec![]);
    semantic::accepted(&mut engine, 0, &cast_wellspring);
    assert_eq!(engine.state.objects[&wellspring].zone, Zone::Stack);
    assert_eq!(engine.state.stack.len(), 1);
    assert!(!engine.state.stack[0].is_triggered);

    pass_both_players(&mut engine);
    assert_eq!(engine.state.objects[&wellspring].zone, Zone::Battlefield);
    assert_eq!(
        engine.state.stack.len(),
        1,
        "entry trigger waits on the stack"
    );
    assert!(engine.state.stack[0].is_triggered);
    assert_eq!(engine.state.players[0].hand.len(), initial_hand - 1);
    assert_eq!(engine.state.players[1].hand.len(), opponent_hand);

    // The exact Scryfall ruling says the graveyard event still triggers this ability while its
    // entry trigger is waiting on the stack. Disenchant is cast at instant speed to exercise that
    // ordering on one source ability identity.
    give_mana(
        &mut engine,
        0,
        ManaGift {
            w: 1,
            c: 1,
            ..Default::default()
        },
    );
    let destroy_wellspring = cast_spell(
        hand_index_for_card(&engine, 0, DISENCHANT),
        target_object(wellspring),
    );
    semantic::accepted(&mut engine, 0, &destroy_wellspring);
    assert_eq!(engine.state.stack.len(), 2);
    assert!(engine.state.stack[0].is_triggered);
    assert!(!engine.state.stack[1].is_triggered);

    pass_both_players(&mut engine);
    assert_eq!(engine.state.objects[&wellspring].zone, Zone::Graveyard);
    assert_eq!(
        engine.state.stack.len(),
        2,
        "both event triggers are pending"
    );
    assert!(engine.state.stack.iter().all(|item| item.is_triggered));
    let entry_trigger = engine.state.stack[0]
        .triggered_ability
        .as_ref()
        .expect("entry trigger definition");
    let graveyard_trigger = engine.state.stack[1]
        .triggered_ability
        .as_ref()
        .expect("graveyard trigger definition");
    assert_eq!(entry_trigger.ability_id.as_str(), "triggered_01");
    assert_eq!(graveyard_trigger.ability_id, entry_trigger.ability_id);
    assert_eq!(engine.state.players[0].hand.len(), initial_hand - 2);
    assert_eq!(engine.state.players[1].hand.len(), opponent_hand);

    pass_both_players(&mut engine);
    assert_eq!(engine.state.stack.len(), 1);
    assert_eq!(engine.state.players[0].hand.len(), initial_hand - 1);
    assert_eq!(engine.state.players[1].hand.len(), opponent_hand);

    pass_both_players(&mut engine);
    assert!(engine.state.stack.is_empty());
    assert_eq!(engine.state.players[0].hand.len(), initial_hand);
    assert_eq!(engine.state.players[1].hand.len(), opponent_hand);
}
