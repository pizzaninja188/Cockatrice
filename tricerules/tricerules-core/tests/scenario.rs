//! Scripted command sequences (M2).

use tricerules_proto::ruled::v1::ruled_command::Cmd;
use tricerules_proto::ruled::v1::{PassPriority, PrimitiveYieldStructured, RuledCommand};

use tricerules_core::GameEngine;

fn pass() -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::PassPriority(PassPriority {})),
    }
}

fn primitive_yield() -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::PrimitiveYieldStructured(PrimitiveYieldStructured {})),
    }
}

#[test]
fn primitive_yield_active_skips_double_pass_main1() {
    let mut e = GameEngine::new(99, &[0, 1], 20, None).expect("new");
    assert_eq!(e.state.turn_step, tricerules_core::TurnStep::Main1);
    e.apply_command(0, &primitive_yield()).expect("active primitive");
    assert_eq!(e.state.turn_step, tricerules_core::TurnStep::BeginCombat);
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
fn mana_pools_empty_on_step_change() {
    let mut e = GameEngine::new(99, &[0, 1], 20, None).expect("new");
    e.state.players[0].mana_pool.red = 2;
    e.state.players[1].mana_pool.green = 1;

    e.apply_command(0, &primitive_yield()).expect("active primitive");

    assert_eq!(e.state.turn_step, tricerules_core::TurnStep::BeginCombat);
    assert_eq!(e.state.players[0].mana_pool.red, 0);
    assert_eq!(e.state.players[0].mana_pool.green, 0);
    assert_eq!(e.state.players[0].mana_pool.blue, 0);
    assert_eq!(e.state.players[0].mana_pool.colorless, 0);
    assert_eq!(e.state.players[1].mana_pool.red, 0);
    assert_eq!(e.state.players[1].mana_pool.green, 0);
    assert_eq!(e.state.players[1].mana_pool.blue, 0);
    assert_eq!(e.state.players[1].mana_pool.colorless, 0);
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
