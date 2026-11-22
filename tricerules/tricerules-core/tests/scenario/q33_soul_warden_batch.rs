//! Actual-card scenarios for the queue-v33 another-creature entry life-gain cohort.
//!
//! Expectations are printed Oracle values. CR 603.6a creates the enters-the-
//! battlefield event; the source's own entry is excluded and only creatures
//! controlled by the source's controller are observed.

use super::helpers::*;

struct Case {
    id: &'static str,
    amount: i32,
}

const CASES: [Case; 3] = [
    Case {
        id: "healer_of_the_pride",
        amount: 2,
    },
    Case {
        id: "impassioned_orator",
        amount: 1,
    },
    Case {
        id: "social_climber",
        amount: 1,
    },
];

fn engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![
        deck_with(
            "plains",
            &[card, "grizzly_bears", "grizzly_bears", "short_sword"],
        ),
        deck_with("forest", &["grizzly_bears", "short_sword"]),
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
fn q33_soul_warden_canary_simplest() {
    let case = &CASES[1];
    let mut e = engine(33_030_001, case.id);
    let source = move_ready_to_battlefield(&mut e, 0, case.id);
    assert!(e.state.stack.is_empty(), "its own entry is excluded");
    move_ready_to_battlefield(&mut e, 0, "grizzly_bears");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.players[0].life, 20 + case.amount);
    assert_eq!(e.state.players[1].life, 20);
    assert_eq!(
        e.state.objects[&source].zone,
        tricerules_core::Zone::Battlefield
    );
}

#[test]
fn q33_soul_warden_canary_hardest() {
    let case = &CASES[0];
    let mut e = engine(33_030_002, case.id);
    move_ready_to_battlefield(&mut e, 0, case.id);
    move_ready_to_battlefield(&mut e, 0, "grizzly_bears");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.players[0].life, 22);
}

#[test]
fn q33_soul_warden_cards_ignore_opponents_noncreatures_and_their_own_entry() {
    for (index, case) in CASES.iter().enumerate() {
        let mut e = engine(33_031_000 + index as u64, case.id);
        move_ready_to_battlefield(&mut e, 0, case.id);
        assert!(
            e.state.stack.is_empty(),
            "{}: the source's own entry is excluded",
            case.id
        );
        move_ready_to_battlefield(&mut e, 1, "grizzly_bears");
        assert!(
            e.state.stack.is_empty(),
            "{}: an opponent's creature does not trigger it",
            case.id
        );
        move_ready_to_battlefield(&mut e, 0, "short_sword");
        assert!(
            e.state.stack.is_empty(),
            "{}: a noncreature permanent does not trigger it",
            case.id
        );
        move_ready_to_battlefield(&mut e, 0, "grizzly_bears");
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            (e.state.players[0].life, e.state.players[1].life),
            (20 + case.amount, 20),
            "{}: exactly the printed life",
            case.id
        );
    }
}

#[test]
fn q33_soul_warden_cards_stack_one_trigger_per_entering_creature() {
    for (index, case) in CASES.iter().enumerate() {
        let mut e = engine(33_032_000 + index as u64, case.id);
        move_ready_to_battlefield(&mut e, 0, case.id);
        move_ready_to_battlefield(&mut e, 0, "grizzly_bears");
        assert_eq!(e.state.stack.len(), 1, "{}", case.id);
        move_ready_to_battlefield(&mut e, 0, "grizzly_bears");
        assert_eq!(e.state.stack.len(), 2, "{}", case.id);
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.players[0].life,
            20 + 2 * case.amount,
            "{}: one trigger per entering creature",
            case.id
        );
    }
}
