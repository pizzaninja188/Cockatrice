//! Actual-card scenarios for the queue-v22 "entry, draw a card and lose 1 life"
//! cohort.
//!
//! Expectations are printed Oracle values. CR 603.6a makes the entry trigger;
//! CR 121.1 draws one card; CR 119.3 loses exactly one life; CR 608.2c resolves
//! the written instructions in order.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;

struct Case {
    id: &'static str,
    generic: u32,
}

fn cases() -> [Case; 3] {
    [
        Case {
            id: "dusk_legion_zealot",
            generic: 1,
        },
        Case {
            id: "phyrexian_rager",
            generic: 2,
        },
        Case {
            id: "tithebearer_giant",
            generic: 5,
        },
    ]
}

fn engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![
        deck_with("swamp", &[card]),
        deck_with("forest", &["grizzly_bears"]),
    ]);
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("engine");
    advance_to_main1_from_game_start(&mut e);
    e
}

fn cast_case(e: &mut GameEngine, case: &Case) {
    give_mana(
        e,
        0,
        ManaGift {
            b: 1,
            c: case.generic,
            ..Default::default()
        },
    );
    ensure_in_hand(e, 0, case.id);
    let slot = hand_index_for_card(e, 0, case.id);
    e.apply_scenario_command(0, &cast_spell(slot, vec![]))
        .expect("cast the entry creature");
    resolve_entire_stack_two_player(e);
}

#[test]
fn etb_draw_lose_life_batch_canary_cards_draw_one_and_lose_one() {
    for case in cases()
        .into_iter()
        .filter(|case| matches!(case.id, "dusk_legion_zealot" | "tithebearer_giant"))
    {
        let mut e = engine(11_400_000, case.id);
        ensure_in_hand(&mut e, 0, case.id);
        let hand_before = e.state.players[0].hand.len();
        cast_case(&mut e, &case);
        assert_eq!(
            e.state.players[0].hand.len(),
            hand_before,
            "{}: the cast card leaves and exactly one is drawn",
            case.id
        );
        assert_eq!(e.state.players[0].life, 19, "{}", case.id);
        assert_eq!(e.state.players[1].life, 20, "{}", case.id);
    }
}

#[test]
fn etb_draw_lose_life_batch_cards_scope_the_draw_and_loss_to_their_controller() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(11_410_000 + index as u64, case.id);
        let opponent_hand = e.state.players[1].hand.len();
        let opponent_library = e.state.players[1].library.len();
        cast_case(&mut e, &case);
        assert_eq!(e.state.players[1].hand.len(), opponent_hand, "{}", case.id);
        assert_eq!(
            e.state.players[1].library.len(),
            opponent_library,
            "{}: the opponent neither draws nor mills",
            case.id
        );
        assert_eq!(e.state.players[1].life, 20, "{}", case.id);
        assert_eq!(e.state.players[0].life, 19, "{}", case.id);
    }
}

#[test]
fn etb_draw_lose_life_batch_cards_draw_exactly_one() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(11_420_000 + index as u64, case.id);
        ensure_in_hand(&mut e, 0, case.id);
        let hand_before = e.state.players[0].hand.len();
        let library_before = e.state.players[0].library.len();
        cast_case(&mut e, &case);
        assert_eq!(
            e.state.players[0].hand.len(),
            hand_before,
            "{}: cast one, draw one",
            case.id
        );
        assert_eq!(
            e.state.players[0].library.len(),
            library_before - 1,
            "{}: exactly one card left the library",
            case.id
        );
        assert_eq!(e.state.players[0].life, 19, "{}", case.id);
    }
}

#[test]
fn etb_draw_lose_life_batch_cards_lose_the_game_at_one_life() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(11_430_000 + index as u64, case.id);
        e.state.players[0].life = 1;
        cast_case(&mut e, &case);
        assert_eq!(e.state.players[0].life, 0, "{}", case.id);
        assert!(e.state.players[0].has_lost, "{}: zero life loses", case.id);
    }
}

#[test]
fn etb_draw_lose_life_batch_cards_enter_the_battlefield() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(11_440_000 + index as u64, case.id);
        cast_case(&mut e, &case);
        let source = battlefield_object_for_card(&e, 0, case.id);
        assert_eq!(
            e.state.objects[&source].zone,
            Zone::Battlefield,
            "{}",
            case.id
        );
    }
}
