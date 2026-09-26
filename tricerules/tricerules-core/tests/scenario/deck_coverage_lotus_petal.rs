//! Exact deck-corpus coverage for Lotus Petal.
//!
//! Oracle and rulings checked against the physical Tempest printing on 2026-09-26; Scryfall
//! returned no rulings. CR 105.4/106.1a define the color choice, CR 107.5 and 701.21a govern its
//! tap and sacrifice costs, and CR 605.1a and 605.3a-b govern its immediate mana-ability resolution.

use super::helpers::*;
use tricerules_core::{GameEngine, Zone};
use tricerules_proto::ruled::v1::ruled_command::Cmd;

type ManaPool = (u32, u32, u32, u32, u32, u32);

fn mana_pool(engine: &GameEngine) -> ManaPool {
    let pool = &engine.state.players[0].mana_pool;
    (
        pool.white,
        pool.blue,
        pool.black,
        pool.red,
        pool.green,
        pool.colorless,
    )
}

fn lotus_petal_engine(seed: u64) -> (GameEngine, u32) {
    let decks = Some(vec![deck_with("island", &[]), deck_with("forest", &[])]);
    let mut engine = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    let petal = inject_permanent_on_battlefield(&mut engine, 0, "lotus_petal");
    (engine, petal)
}

fn activate_with_mana_option(engine: &GameEngine, petal: u32, option: u32) -> RuledCommand {
    let mut command = activate_ability_for(engine, petal, 0, vec![]);
    let Some(Cmd::ActivateAbility(activation)) = command.cmd.as_mut() else {
        unreachable!()
    };
    activation.mana_option_index = option;
    command
}

#[test]
fn lotus_petal_sacrifices_and_adds_the_chosen_color_without_the_stack() {
    let colors = [
        (1, 0, 0, 0, 0, 0),
        (0, 1, 0, 0, 0, 0),
        (0, 0, 1, 0, 0, 0),
        (0, 0, 0, 1, 0, 0),
        (0, 0, 0, 0, 1, 0),
    ];

    for (option, expected_pool) in colors.into_iter().enumerate() {
        let (mut engine, petal) = lotus_petal_engine(20_260_926 + option as u64);
        let command = activate_with_mana_option(&engine, petal, option as u32);
        engine
            .apply_command(0, &command)
            .expect("pay the tap and self-sacrifice costs and choose one color");

        assert_eq!(mana_pool(&engine), expected_pool, "mana option {option}");
        assert_eq!(engine.state.objects[&petal].zone, Zone::Graveyard);
        assert!(
            engine.state.stack.is_empty(),
            "Lotus Petal's mana ability resolves without using the stack"
        );
        assert!(engine.state.pending_triggers.is_empty());
    }
}

#[test]
fn lotus_petal_rejects_an_out_of_range_color_without_paying_its_costs() {
    let (mut engine, petal) = lotus_petal_engine(20_260_932);
    let command_index_before = engine.state.command_index;
    let invalid = activate_with_mana_option(&engine, petal, 5);

    let error = engine
        .apply_command(0, &invalid)
        .expect_err("the ability offers only the five colors");
    assert!(matches!(
        error,
        tricerules_core::EngineError::Illegal("invalid mana option")
    ));

    assert_eq!(engine.state.command_index, command_index_before);
    assert_eq!(mana_pool(&engine), (0, 0, 0, 0, 0, 0));
    assert_eq!(engine.state.objects[&petal].zone, Zone::Battlefield);
    assert!(!engine.state.objects[&petal].tapped);
    assert!(engine.state.stack.is_empty());
}
