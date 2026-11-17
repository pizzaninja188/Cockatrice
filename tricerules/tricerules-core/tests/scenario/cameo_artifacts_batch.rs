//! Actual-card scenarios for the reviewed Cameo mana-artifact cycle.
//!
//! Expectations are printed Oracle values. CR 605.1a and 605.3a-b make the
//! targetless artifact mana ability resolve immediately without using the
//! stack; CR 118.3 makes the tap cost require an untapped artifact.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;

type ManaPool = (u32, u32, u32, u32, u32, u32);

struct Case {
    id: &'static str,
    options: [ManaPool; 2],
}

const CASES: [Case; 5] = [
    Case {
        id: "bloodstone_cameo",
        options: [(0, 0, 1, 0, 0, 0), (0, 0, 0, 1, 0, 0)],
    },
    Case {
        id: "drake-skull_cameo",
        options: [(0, 1, 0, 0, 0, 0), (0, 0, 1, 0, 0, 0)],
    },
    Case {
        id: "seashell_cameo",
        options: [(1, 0, 0, 0, 0, 0), (0, 1, 0, 0, 0, 0)],
    },
    Case {
        id: "tigereye_cameo",
        options: [(0, 0, 0, 0, 1, 0), (1, 0, 0, 0, 0, 0)],
    },
    Case {
        id: "troll-horn_cameo",
        options: [(0, 0, 0, 1, 0, 0), (0, 0, 0, 0, 1, 0)],
    },
];

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

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("forest", &[]), deck_with("island", &[])]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn activate_mana_option(engine: &GameEngine, artifact: u32, option: u32) -> RuledCommand {
    let mut command = activate_ability_for(engine, artifact, 0, vec![]);
    let Some(Cmd::ActivateAbility(activation)) = command.cmd.as_mut() else {
        unreachable!()
    };
    activation.mana_option_index = option;
    command
}

#[test]
fn cameo_artifacts_batch_cards_produce_each_printed_color_and_tap() {
    for (case_index, case) in CASES.iter().enumerate() {
        for (option, expected) in case.options.iter().enumerate() {
            let mut engine = engine(25_070_000 + (case_index * 2 + option) as u64);
            let artifact = inject_permanent_on_battlefield(&mut engine, 0, case.id);

            engine
                .apply_scenario_command(0, &activate_mana_option(&engine, artifact, option as u32))
                .unwrap_or_else(|error| panic!("{} option {option}: {error:?}", case.id));

            assert_eq!(mana_pool(&engine), *expected, "{} option {option}", case.id);
            assert!(engine.state.objects[&artifact].tapped, "{}", case.id);
            assert!(
                engine.state.stack.is_empty(),
                "{}: artifact mana abilities resolve immediately",
                case.id
            );
        }
    }
}

#[test]
fn cameo_artifacts_batch_cards_reject_a_second_activation_while_tapped() {
    for (case_index, case) in CASES.iter().enumerate() {
        let mut engine = engine(25_071_000 + case_index as u64);
        let artifact = inject_permanent_on_battlefield(&mut engine, 0, case.id);

        engine
            .apply_scenario_command(0, &activate_mana_option(&engine, artifact, 0))
            .expect("first activation");
        let before = engine.state.command_index;
        engine
            .apply_scenario_command(0, &activate_mana_option(&engine, artifact, 1))
            .expect_err("a tapped artifact cannot pay the tap cost again");

        assert_eq!(engine.state.command_index, before, "{}", case.id);
        assert!(engine.state.objects[&artifact].tapped, "{}", case.id);
        assert_eq!(mana_pool(&engine), case.options[0], "{}", case.id);
    }
}
