//! Exact deck-corpus coverage for Chromatic Star.
//!
//! Oracle and rulings checked 2026-09-25; Scryfall returned no rulings for this Oracle identity.
//! CR 605.1a and 605.3b govern its activated mana ability. CR 603.6c and 603.10a govern its
//! battlefield-to-graveyard trigger.

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

fn chromatic_star_engine(seed: u64) -> (GameEngine, u32) {
    let decks = Some(vec![deck_with("island", &[]), deck_with("forest", &[])]);
    let mut engine = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    let star = inject_permanent_on_battlefield(&mut engine, 0, "chromatic_star");
    (engine, star)
}

fn activate_with_mana_option(engine: &GameEngine, star: u32, option: u32) -> RuledCommand {
    let mut command = activate_ability_for(engine, star, 0, vec![]);
    let Some(Cmd::ActivateAbility(activation)) = command.cmd.as_mut() else {
        unreachable!()
    };
    activation.mana_option_index = option;
    command
}

#[test]
fn chromatic_star_pays_for_each_color_and_draws_when_sacrificed() {
    let colors = [
        (1, 0, 0, 0, 0, 0),
        (0, 1, 0, 0, 0, 0),
        (0, 0, 1, 0, 0, 0),
        (0, 0, 0, 1, 0, 0),
        (0, 0, 0, 0, 1, 0),
    ];

    for (option, expected_pool) in colors.into_iter().enumerate() {
        let (mut engine, star) = chromatic_star_engine(20_260_960 + option as u64);
        let hand_before = engine.state.players[0].hand.len();
        let opponent_hand_before = engine.state.players[1].hand.len();
        let unpaid = activate_with_mana_option(&engine, star, option as u32);
        let command_index_before = engine.state.command_index;
        engine
            .apply_command(0, &unpaid)
            .expect_err("Chromatic Star requires one generic mana");
        assert_eq!(engine.state.command_index, command_index_before);
        assert_eq!(mana_pool(&engine), (0, 0, 0, 0, 0, 0));
        assert_eq!(engine.state.objects[&star].zone, Zone::Battlefield);
        assert!(!engine.state.objects[&star].tapped);
        assert!(engine.state.pending_triggers.is_empty());
        assert!(engine.state.stack.is_empty());

        give_mana(
            &mut engine,
            0,
            ManaGift {
                c: 1,
                ..Default::default()
            },
        );
        let activation = activate_with_mana_option(&engine, star, option as u32);
        semantic::accepted(&mut engine, 0, &activation);

        assert_eq!(mana_pool(&engine), expected_pool, "mana option {option}");
        assert_eq!(engine.state.objects[&star].zone, Zone::Graveyard);
        assert_eq!(
            engine.state.stack.len(),
            1,
            "only the triggered ability is waiting on the stack"
        );
        assert!(engine.state.stack[0].is_triggered);
        assert!(engine.state.stack[0].activated_ability.is_none());
        assert_eq!(engine.state.stack[0].controller, 0);
        assert_eq!(
            engine.state.players[0].hand.len(),
            hand_before,
            "the draw waits for the triggered ability to resolve"
        );
        assert_eq!(engine.state.players[1].hand.len(), opponent_hand_before);

        pass_both_players(&mut engine);
        resolve_entire_stack_two_player(&mut engine);
        assert_eq!(engine.state.players[0].hand.len(), hand_before + 1);
        assert_eq!(engine.state.players[1].hand.len(), opponent_hand_before);
    }
}
