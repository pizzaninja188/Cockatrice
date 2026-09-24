//! CR 121.2: each successfully drawn card creates its own event and trigger.

use crate::helpers::*;
use tricerules_cards::CounterKind;
use tricerules_core::Zone;

fn prepared(seed: u64, card: &str) -> (GameEngine, u32) {
    let decks = Some(vec![
        deck_with("island", &[card, "divination"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(seed, &[0, 1], 20, decks, true).unwrap();
    advance_to_main1_from_game_start(&mut engine);
    let source = relocate_to_battlefield(&mut engine, 0, card, false);
    ensure_in_hand(&mut engine, 0, "divination");
    (engine, source)
}

fn draw_two(engine: &mut GameEngine) {
    give_mana(
        engine,
        0,
        ManaGift {
            u: 1,
            c: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(engine, 0, "divination");
    engine.apply_command(0, &cast_spell(slot, vec![])).unwrap();
    resolve_entire_stack_two_player(engine);
}

#[test]
fn both_complete_cards_trigger_once_per_card_in_a_two_card_draw() {
    for (index, card) in ["ravenhill_flock", "clinquant_skymage"]
        .into_iter()
        .enumerate()
    {
        let (mut engine, source) = prepared(296_101 + index as u64, card);
        draw_two(&mut engine);
        assert_eq!(engine.state.turn_history.current.player(0).cards_drawn, 2);
        assert_eq!(
            engine.state.objects[&source].counter_count(CounterKind::PlusOnePlusOne),
            2,
            "{card}"
        );
        assert_eq!(engine.state.objects[&source].zone, Zone::Battlefield);
    }
}

#[test]
fn a_single_effect_draw_triggers_once_and_an_opponents_draw_does_not() {
    let (mut engine, source) = prepared(296_103, "ravenhill_flock");
    inject_card_into_hand(&mut engine, 0, "elvish_visionary");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            g: 1,
            c: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "elvish_visionary");
    engine.apply_command(0, &cast_spell(slot, vec![])).unwrap();
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::PlusOnePlusOne),
        1
    );

    end_active_turn(&mut engine, 0);
    pass_both_players(&mut engine); // P1's turn-based draw.
    assert_eq!(engine.state.turn_history.current.player(1).cards_drawn, 1);
    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::PlusOnePlusOne),
        1
    );
}

#[test]
fn pending_draw_triggers_do_not_put_counters_on_a_new_source_generation() {
    let (mut engine, source) = prepared(296_104, "clinquant_skymage");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            c: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "divination");
    engine.apply_command(0, &cast_spell(slot, vec![])).unwrap();
    pass_both_players(&mut engine); // Resolve Divination; draw triggers remain pending/on stack.
    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::PlusOnePlusOne),
        0
    );
    engine.state.players[0]
        .battlefield
        .retain(|&id| id != source);
    engine.state.players[0].graveyard.push(source);
    engine.state.objects.get_mut(&source).unwrap().zone = Zone::Graveyard;
    *engine
        .state
        .zone_change_generation
        .entry(source)
        .or_default() += 1;
    engine.state.players[0].graveyard.retain(|&id| id != source);
    engine.state.players[0].battlefield.push(source);
    engine.state.objects.get_mut(&source).unwrap().zone = Zone::Battlefield;
    *engine
        .state
        .zone_change_generation
        .entry(source)
        .or_default() += 1;
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::PlusOnePlusOne),
        0
    );
}

#[test]
fn an_empty_library_draw_attempt_creates_no_every_draw_trigger() {
    let (mut engine, source) = prepared(296_105, "ravenhill_flock");
    engine.state.players[0].library.clear();
    draw_two(&mut engine);
    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::PlusOnePlusOne),
        0
    );
    assert_eq!(engine.state.turn_history.current.player(0).cards_drawn, 0);
}

#[test]
fn own_turn_based_draw_triggers_after_the_opponents_turn() {
    let (mut engine, source) = prepared(296_106, "clinquant_skymage");
    end_active_turn(&mut engine, 0);
    pass_both_players(&mut engine); // P1 draws.
    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::PlusOnePlusOne),
        0
    );
    pass_both_players(&mut engine); // P1 reaches main phase.
    end_active_turn(&mut engine, 1);
    pass_both_players(&mut engine); // P0 draws.
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::PlusOnePlusOne),
        1
    );
}
