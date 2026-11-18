//! Actual-card scenarios for the queue-v33 upkeep self-damage cohort.
//!
//! Expectations are printed Oracle values. CR 503.1 gives each turn an upkeep
//! step; CR 603.2 puts the trigger on the stack and CR 120.3 makes the source's
//! controller the damaged player.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;

struct Case {
    id: &'static str,
    flying: bool,
}

const CASES: [Case; 3] = [
    Case {
        id: "fledgling_djinn",
        flying: true,
    },
    Case {
        id: "nettletooth_djinn",
        flying: false,
    },
    Case {
        id: "serendib_efreet",
        flying: true,
    },
];

fn engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![deck_with("forest", &[card]), deck_with("forest", &[])]);
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

fn advance_to_active_player_upkeep(e: &mut GameEngine, player: i32) {
    for _ in 0..60 {
        let (actor, command) = match e.state.cleanup_discard_player {
            Some(cleanup_player) => {
                let player_index = e.state.player_idx(cleanup_player).expect("cleanup player");
                let excess = e.state.players[player_index].hand.len() - 7;
                (
                    cleanup_player,
                    discard_cleanup_batch((0..excess as u32).collect()),
                )
            }
            None => (e.state.priority_player_id(), pass()),
        };
        e.apply_scenario_command(actor, &command)
            .expect("pass through turn");
        if e.state.active_player_id() == player
            && e.state.turn_step == tricerules_core::TurnStep::Upkeep
        {
            return;
        }
    }
    panic!("game did not reach player {player}'s upkeep");
}

#[test]
fn q33_upkeep_damage_canary_simplest() {
    let case = &CASES[1];
    let mut e = engine(33_010_001, case.id);
    move_ready_to_battlefield(&mut e, 0, case.id);
    advance_to_active_player_upkeep(&mut e, 0);
    resolve_entire_stack_two_player(&mut e);
    assert_eq!((e.state.players[0].life, e.state.players[1].life), (19, 20));
}

#[test]
fn q33_upkeep_damage_canary_hardest() {
    let case = &CASES[2];
    let mut e = engine(33_010_002, case.id);
    let source = move_ready_to_battlefield(&mut e, 0, case.id);
    assert!(e.effective_has_keyword(source, tricerules_cards::Keyword::Flying));
    advance_to_active_player_upkeep(&mut e, 0);
    resolve_entire_stack_two_player(&mut e);
    assert_eq!((e.state.players[0].life, e.state.players[1].life), (19, 20));
}

#[test]
fn q33_upkeep_damage_cards_damage_only_their_controller_once_per_upkeep() {
    for (index, case) in CASES.iter().enumerate() {
        let mut e = engine(33_011_000 + index as u64, case.id);
        let source = move_ready_to_battlefield(&mut e, 0, case.id);
        assert_eq!(
            e.effective_has_keyword(source, tricerules_cards::Keyword::Flying),
            case.flying,
            "{}: printed flying",
            case.id
        );
        advance_to_active_player_upkeep(&mut e, 0);
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            (e.state.players[0].life, e.state.players[1].life),
            (19, 20),
            "{}: exactly one point to the controller",
            case.id
        );
        advance_to_active_player_upkeep(&mut e, 1);
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            (e.state.players[0].life, e.state.players[1].life),
            (19, 20),
            "{}: an opponent's upkeep does not trigger it",
            case.id
        );
    }
}
