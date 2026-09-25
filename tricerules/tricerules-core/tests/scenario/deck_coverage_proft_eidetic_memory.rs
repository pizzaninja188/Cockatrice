//! Complete-card scenarios for Proft's Eidetic Memory in the pinned deck corpus.
//!
//! The ability counts committed draws from earlier in the turn and reads its counter amount on
//! resolution (CR 603.4, 603.3d). Vision Skeins is the target-deck interaction that adds draws
//! while the targeted Proft trigger waits on the stack.

use super::helpers::*;
use tricerules_cards::CounterKind;
use tricerules_core::{GameEngine, Zone};
use tricerules_proto::ruled::v1::ruled_command::Cmd;
use tricerules_proto::ruled::v1::{ChooseTriggerTarget, RuledCommand};

fn start_player_zero_second_main(engine: &mut GameEngine) {
    advance_to_main1_from_game_start(engine);
    end_active_turn(engine, 0);
    pass_both_players(engine); // P1 upkeep to draw; P1 takes the turn-based draw.
    pass_both_players(engine); // P1 draw to main one.
    end_active_turn(engine, 1);
    pass_both_players(engine); // P0 upkeep to draw; P0 takes its first turn-based draw.
    pass_both_players(engine); // P0 draw to main one.
    assert_eq!(engine.state.active_player_id(), 0);
    assert_eq!(engine.state.turn_history.current.player(0).cards_drawn, 1);
}

fn cast_proft(engine: &mut GameEngine) -> u32 {
    let source = inject_card_into_hand(engine, 0, "profts_eidetic_memory");
    give_mana(
        engine,
        0,
        ManaGift {
            u: 2,
            c: 4,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(engine, 0, "profts_eidetic_memory");
    engine
        .apply_command(0, &cast_spell(slot, Vec::new()))
        .expect("cast Proft's Eidetic Memory");
    resolve_entire_stack_two_player(engine);
    assert_eq!(engine.state.objects[&source].zone, Zone::Battlefield);
    source
}

fn choose_proft_target(engine: &mut GameEngine, target: u32) {
    engine
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
                    decline: false,
                    selected_modes: Vec::new(),
                    targets: target_object(target),
                })),
            },
        )
        .expect("choose the target as Proft's triggered ability is put on the stack");
}

#[test]
fn proft_counts_pre_entry_draws_and_reads_x_when_the_trigger_resolves() {
    let decks = Some(vec![deck_with("island", &[]), deck_with("forest", &[])]);
    let mut engine = GameEngine::new(20_260_957, &[0, 1], 20, decks, true).expect("new game");
    start_player_zero_second_main(&mut engine);
    let creature = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    inject_card_into_hand(&mut engine, 0, "vision_skeins");
    inject_card_into_hand(&mut engine, 1, "vision_skeins");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 4,
            c: 4,
            ..Default::default()
        },
    );

    let first_skeins = hand_index_for_card(&engine, 0, "vision_skeins");
    engine
        .apply_command(0, &cast_spell(first_skeins, Vec::new()))
        .expect("cast Vision Skeins before Proft enters");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.turn_history.current.player(0).cards_drawn, 3);
    assert_eq!(engine.state.turn_history.current.player(1).cards_drawn, 2);

    cast_proft(&mut engine);
    assert_eq!(engine.state.turn_history.current.player(0).cards_drawn, 4);
    assert_eq!(engine.state.turn_history.current.player(1).cards_drawn, 2);

    engine
        .apply_command(0, &primitive_yield())
        .expect("enter the beginning of combat step");
    assert_eq!(engine.state.pending_triggers.len(), 1);
    assert_eq!(engine.state.pending_triggers[0].controller, 0);
    choose_proft_target(&mut engine, creature);
    assert!(engine.state.pending_triggers.is_empty());
    assert_eq!(engine.state.stack.len(), 1);
    assert_eq!(
        engine.state.objects[&creature].counter_count(CounterKind::PlusOnePlusOne),
        0
    );

    engine
        .apply_command(0, &pass())
        .expect("active player passes priority");
    assert_eq!(engine.state.priority_player_id(), 1);
    let response = hand_index_for_card(&engine, 1, "vision_skeins");
    give_mana(
        &mut engine,
        1,
        ManaGift {
            u: 2,
            c: 2,
            ..Default::default()
        },
    );
    engine
        .apply_command(1, &cast_spell(response, Vec::new()))
        .expect("opponent responds with Vision Skeins");
    pass_both_players(&mut engine);

    assert_eq!(engine.state.turn_history.current.player(0).cards_drawn, 6);
    assert_eq!(engine.state.turn_history.current.player(1).cards_drawn, 4);
    assert_eq!(
        engine.state.stack.len(),
        1,
        "Proft's trigger is still waiting"
    );
    assert_eq!(
        engine.state.objects[&creature].counter_count(CounterKind::PlusOnePlusOne),
        0
    );

    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        engine.state.objects[&creature].counter_count(CounterKind::PlusOnePlusOne),
        5
    );
}

#[test]
fn proft_does_not_trigger_after_only_one_draw_this_turn() {
    let decks = Some(vec![deck_with("island", &[]), deck_with("forest", &[])]);
    let mut engine = GameEngine::new(20_260_958, &[0, 1], 20, decks, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    cast_proft(&mut engine);
    assert_eq!(engine.state.turn_history.current.player(0).cards_drawn, 1);

    engine
        .apply_command(0, &primitive_yield())
        .expect("enter beginning of combat");
    assert!(engine.state.pending_triggers.is_empty());
    assert!(engine.state.stack.is_empty());
}

#[test]
fn proft_removes_only_its_controllers_hand_size_limit() {
    let decks = Some(vec![deck_with("island", &[]), deck_with("forest", &[])]);
    let mut engine = GameEngine::new(20_260_959, &[0, 1], 20, decks, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    cast_proft(&mut engine);
    while engine.state.players[0].hand.len() < 9 {
        inject_card_into_hand(&mut engine, 0, "forest");
    }
    while engine.state.players[1].hand.len() < 9 {
        inject_card_into_hand(&mut engine, 1, "forest");
    }

    end_active_turn(&mut engine, 0);
    assert_eq!(engine.state.players[0].hand.len(), 9);
    assert_eq!(engine.state.players[1].hand.len(), 9);

    pass_both_players(&mut engine); // P1 upkeep to draw.
    pass_both_players(&mut engine); // P1 draw to main one.
    end_active_turn(&mut engine, 1);
    assert_eq!(engine.state.players[1].hand.len(), 7);
}
