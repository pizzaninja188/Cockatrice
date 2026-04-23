//! Scripted command sequences (M2).

use tricerules_proto::ruled::v1::ruled_command::Cmd;
use tricerules_proto::ruled::v1::{PassPriority, RuledCommand};

use tricerules_core::GameEngine;

fn pass() -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::PassPriority(PassPriority {})),
    }
}

#[test]
fn two_player_passes_empty_stack_advances_toward_combat() {
    let mut e = GameEngine::new(99, &[0, 1], 20, None).expect("new");
    assert_eq!(e.state.turn_step, tricerules_core::TurnStep::Main1);
    e.apply_command(0, &pass()).expect("p0");
    e.apply_command(1, &pass()).expect("p1");
    // After two passes, should leave main1 to begin combat
    assert_eq!(e.state.turn_step, tricerules_core::TurnStep::BeginCombat);
}

#[test]
fn new_with_custom_deck_length() {
    let decks = Some(vec![vec!["mountain".into(); 30], vec!["forest".into(); 30]]);
    let e = GameEngine::new(1, &[0, 1], 20, decks).expect("new");
    assert_eq!(
        e.state.players[0].library.len() + e.state.players[0].hand.len(),
        30
    );
}
