//! Exact deck-corpus coverage for Mind Stone.
//!
//! Oracle and rulings checked against the pinned MBC printing on 2026-09-26; Scryfall returned
//! no rulings. CR 602.2b/601.2h and 701.21a cover cost payment and sacrifice, CR 605.1a and 605.3b
//! classify and immediately resolve the mana ability, and CR 121.1 covers drawing.

use super::helpers::*;
use tricerules_core::{GameEngine, Zone};

fn mind_stone_engine(seed: u64) -> (GameEngine, u32) {
    let decks = Some(vec![deck_with("island", &[]), deck_with("forest", &[])]);
    let mut engine = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    let stone = inject_permanent_on_battlefield(&mut engine, 0, "mind_stone");
    (engine, stone)
}

#[test]
fn mind_stone_taps_for_colorless_without_using_the_stack() {
    let (mut engine, stone) = mind_stone_engine(20_260_927);

    apply_ability(&mut engine, 0, stone, 0, vec![]).expect("activate Mind Stone's mana ability");

    assert_eq!(engine.state.players[0].mana_pool.colorless, 1);
    assert!(engine.state.objects[&stone].tapped);
    assert_eq!(engine.state.objects[&stone].zone, Zone::Battlefield);
    assert!(
        engine.state.stack.is_empty(),
        "the mana ability resolves immediately"
    );
}

#[test]
fn mind_stone_pays_its_cost_before_drawing_one_card() {
    let (mut engine, stone) = mind_stone_engine(20_260_928);
    let top_card = *engine.state.players[0]
        .library
        .front()
        .expect("library has a card to draw");
    let hand_before = engine.state.players[0].hand.len();
    let library_before = engine.state.players[0].library.len();

    apply_ability(&mut engine, 0, stone, 1, vec![])
        .expect_err("Mind Stone's draw ability requires one mana");
    assert_eq!(engine.state.objects[&stone].zone, Zone::Battlefield);
    assert!(!engine.state.objects[&stone].tapped);
    assert_eq!(engine.state.players[0].hand.len(), hand_before);
    assert_eq!(engine.state.players[0].library.len(), library_before);
    assert!(engine.state.stack.is_empty());

    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );
    apply_ability(&mut engine, 0, stone, 1, vec![])
        .expect("pay {1}, tap, and sacrifice Mind Stone");

    assert_eq!(engine.state.players[0].mana_pool.colorless, 0);
    assert_eq!(engine.state.objects[&stone].zone, Zone::Graveyard);
    assert_eq!(engine.state.players[0].hand.len(), hand_before);
    assert_eq!(engine.state.players[0].library.len(), library_before);
    assert_eq!(engine.state.stack.len(), 1, "the draw waits for resolution");

    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(engine.state.players[0].hand.len(), hand_before + 1);
    assert!(engine.state.players[0].hand.contains(&top_card));
    assert_eq!(engine.state.players[0].library.len(), library_before - 1);
    assert!(engine.state.stack.is_empty());
}
