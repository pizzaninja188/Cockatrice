//! Actual-card scenarios for the reviewed `{T}: You gain 1 life.` family.
//!
//! Expectations are printed Oracle values. CR 107.5 and 118.3 require an
//! untapped source to pay the tap cost; CR 602.2 activates the ability; CR 119.3
//! adjusts only its controller's life total.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;

struct Case {
    id: &'static str,
}

fn cases() -> [Case; 4] {
    [
        Case {
            id: "braidwood_cup",
        },
        Case {
            id: "marble_chalice",
        },
        Case {
            id: "silent_attendant",
        },
        Case { id: "soulmender" },
    ]
}

fn engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![
        deck_with("plains", &[card]),
        deck_with("plains", &[card]),
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
fn tap_gain_life_batch_cards_activate_to_gain_one_life() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_200_000 + index as u64, case.id);
        let source = move_ready_to_battlefield(&mut e, 0, case.id);
        apply_ability(&mut e, 0, source, 0, vec![]).expect("activate the life ability");
        assert!(
            e.state.objects[&source].tapped,
            "{}: the tap cost is paid on activation",
            case.id
        );
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.players[0].life, 21,
            "{}: controller gains one",
            case.id
        );
        assert_eq!(
            e.state.players[1].life, 20,
            "{}: the opponent does not gain",
            case.id
        );
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}

#[test]
fn tap_gain_life_batch_cards_scope_the_life_gain_to_their_controller() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_201_000 + index as u64, case.id);
        let source = move_ready_to_battlefield(&mut e, 1, case.id);
        e.apply_scenario_command(0, &pass())
            .expect("the active player passes priority");
        apply_ability(&mut e, 1, source, 0, vec![]).expect("the opponent activates their own");
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(e.state.players[1].life, 21, "{}: owner gains", case.id);
        assert_eq!(
            e.state.players[0].life, 20,
            "{}: noncontroller unchanged",
            case.id
        );
    }
}

#[test]
fn tap_gain_life_batch_cards_require_an_untapped_source_and_controller() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_202_000 + index as u64, case.id);
        let source = move_ready_to_battlefield(&mut e, 0, case.id);
        apply_ability(&mut e, 0, source, 0, vec![]).expect("first activation");
        assert!(
            apply_ability(&mut e, 0, source, 0, vec![]).is_err(),
            "{}: a tapped source cannot pay the tap cost again",
            case.id
        );
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.players[0].life, 21,
            "{}: only one activation",
            case.id
        );

        let mut foreign = engine(9_203_000 + index as u64, case.id);
        let foreign_source = move_ready_to_battlefield(&mut foreign, 0, case.id);
        foreign
            .apply_scenario_command(0, &pass())
            .expect("the controller passes priority");
        assert!(
            apply_ability(&mut foreign, 1, foreign_source, 0, vec![]).is_err(),
            "{}: another player cannot activate it",
            case.id
        );
        assert!(
            !foreign.state.objects[&foreign_source].tapped,
            "{}",
            case.id
        );
    }
}
