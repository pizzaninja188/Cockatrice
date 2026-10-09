//! Actual-card Font of Mythos behavior, reviewed against its Scryfall record and ruling.
//!
//! Source: https://scryfall.com/card/con/136/font-of-mythos (Scryfall card object
//! c198caf8-27ab-4300-841b-507e1b0ce9b3; Oracle ID 7194a262-5e7a-4c12-b271-2bc5e0799477).
//! Rulings: https://api.scryfall.com/cards/c198caf8-27ab-4300-841b-507e1b0ce9b3/rulings .
//! The 2013-04-15 ruling says: “The triggered ability is put onto the stack after you have already
//! drawn your card for the turn.” Current Comprehensive Rules PDF, 2026-06-19:
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260619.pdf . CR 504.1 and 703.4d
//! put the turn-based draw first; CR 603.2b and 603.3 govern creating and putting the beginning-of-
//! step trigger onto the stack; CR 121.1–121.2 govern the three individual draws asserted here.

use super::helpers::*;
use tricerules_core::TurnStep;

fn draw_step_engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("plains", &[]), deck_with("island", &[])]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("new engine");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

/// CR 504.1 / 703.4d and the cited ruling require the normal draw before the trigger resolves.
/// Font differs from Howling Mine: it still triggers while tapped. The resolving effect benefits
/// the player whose draw step it is, including an opponent of Font's controller.
#[test]
fn font_of_mythos_draws_two_after_normal_draw_even_when_tapped() {
    let mut engine = draw_step_engine(926_001);
    let font = inject_permanent_on_battlefield(&mut engine, 0, "font_of_mythos");
    engine.state.objects.get_mut(&font).expect("Font").tapped = true;

    let expected_top: Vec<_> = engine.state.players[1]
        .library
        .iter()
        .take(3)
        .copied()
        .collect();
    assert_eq!(expected_top.len(), 3, "fixture has three cards to draw");
    let p0_hand_before = engine.state.players[0].hand.len();
    let p1_hand_before = engine.state.players[1].hand.len();
    let p0_library_before = engine.state.players[0].library.len();
    let p1_library_before = engine.state.players[1].library.len();

    end_active_turn(&mut engine, 0);
    assert_eq!(engine.state.active_player_id(), 1, "opponent's turn");
    pass_both_players(&mut engine); // P1 upkeep -> draw step, with its turn-based draw first.

    assert_eq!(engine.state.turn_step, TurnStep::Draw);
    assert_eq!(
        engine.state.stack.len(),
        1,
        "Font's trigger waits on the stack"
    );
    assert!(
        engine.state.objects[&font].tapped,
        "tapped Font still triggers"
    );
    assert_eq!(engine.state.players[1].library.len(), p1_library_before - 1);
    assert_eq!(engine.state.players[1].hand.len(), p1_hand_before + 1);
    assert!(
        engine.state.players[1].hand.contains(&expected_top[0]),
        "the normal turn-based draw is already in hand before Font resolves"
    );
    assert_eq!(engine.state.players[0].hand.len(), p0_hand_before);

    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(engine.state.players[1].library.len(), p1_library_before - 3);
    assert_eq!(engine.state.players[1].hand.len(), p1_hand_before + 3);
    assert!(expected_top
        .iter()
        .all(|id| engine.state.players[1].hand.contains(id)));
    assert_eq!(engine.state.players[0].library.len(), p0_library_before);
    assert_eq!(engine.state.players[0].hand.len(), p0_hand_before);
    assert!(engine.state.stack.is_empty());
}
