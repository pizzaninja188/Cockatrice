//! Actual-card scenarios for the reviewed "when this creature enters, you gain
//! 4 life" family.
//!
//! Expectations are printed Oracle values. CR 603.6a stages the entry trigger;
//! the life gain belongs only to the creature's controller and another
//! creature entering never triggers this source.
use super::helpers::*;

struct Case {
    id: &'static str,
}

fn cases() -> [Case; 4] {
    [
        Case {
            id: "shu_soldier-farmers",
        },
        Case {
            id: "spiritual_guardian",
        },
        Case {
            id: "staunch_defenders",
        },
        Case {
            id: "terohs_faithful",
        },
    ]
}

fn engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![
        deck_with("plains", &[card, "grizzly_bears"]),
        deck_with("forest", &[card]),
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

#[test]
fn etb_gain_four_life_batch_cards_gain_four_life_on_entry() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_080_000 + index as u64, case.id);
        let life_before = e.state.players[0].life;
        move_ready_to_battlefield(&mut e, 0, case.id);
        assert_eq!(
            e.state.stack.len(),
            1,
            "{}: one entry trigger is staged",
            case.id
        );
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.players[0].life,
            life_before + 4,
            "{}: the controller gains exactly four life",
            case.id
        );
        assert_eq!(
            e.state.players[1].life, 20,
            "{}: the opponent's life is unchanged",
            case.id
        );
    }
}

#[test]
fn etb_gain_four_life_batch_cards_gain_only_for_their_controller() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_081_000 + index as u64, case.id);
        let p0_life = e.state.players[0].life;
        let p1_life = e.state.players[1].life;
        move_ready_to_battlefield(&mut e, 1, case.id);
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.players[1].life,
            p1_life + 4,
            "{}: the creature's controller gains the life",
            case.id
        );
        assert_eq!(
            e.state.players[0].life, p0_life,
            "{}: the other player does not gain",
            case.id
        );
    }
}

#[test]
fn etb_gain_four_life_batch_cards_do_not_trigger_from_other_entries() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_090_000 + index as u64, case.id);
        move_ready_to_battlefield(&mut e, 0, case.id);
        resolve_entire_stack_two_player(&mut e);
        let life_after_self = e.state.players[0].life;
        move_ready_to_battlefield(&mut e, 0, "grizzly_bears");
        assert_eq!(
            e.state.stack.len(),
            0,
            "{}: another creature's entry stages no trigger",
            case.id
        );
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.players[0].life, life_after_self,
            "{}: another creature does not cause life gain",
            case.id
        );
    }
}
