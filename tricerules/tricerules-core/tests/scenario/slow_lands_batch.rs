//! Actual-card scenarios for the reviewed slow-land cycle.
//!
//! Expectations are printed Oracle values. CR 614.1d and 614.12 make the
//! conditional entry a replacement effect; CR 109.2 keeps the source itself out
//! of the "other lands" count; CR 605.1a and 605.3b cover the mana ability.
//! The pinned ruling states that lands entering at the same time are not counted.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;

type ManaPool = (u32, u32, u32, u32, u32, u32);

struct Case {
    id: &'static str,
    options: [ManaPool; 2],
}

const CASES: [Case; 5] = [
    Case {
        id: "deserted_beach",
        options: [(1, 0, 0, 0, 0, 0), (0, 1, 0, 0, 0, 0)],
    },
    Case {
        id: "haunted_ridge",
        options: [(0, 0, 1, 0, 0, 0), (0, 0, 0, 1, 0, 0)],
    },
    Case {
        id: "overgrown_farmland",
        options: [(0, 0, 0, 0, 1, 0), (1, 0, 0, 0, 0, 0)],
    },
    Case {
        id: "rockfall_vale",
        options: [(0, 0, 0, 1, 0, 0), (0, 0, 0, 0, 1, 0)],
    },
    Case {
        id: "shipwreck_marsh",
        options: [(0, 1, 0, 0, 0, 0), (0, 0, 1, 0, 0, 0)],
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

fn play_with_other_lands(seed: u64, id: &str, other_lands: usize) -> (GameEngine, u32) {
    let mut engine = engine(seed);
    for _ in 0..other_lands {
        relocate_to_battlefield(&mut engine, 0, "forest", false);
    }
    let land = inject_card_into_hand(&mut engine, 0, id);
    let slot = hand_index_for_card(&engine, 0, id);
    engine
        .apply_scenario_command(0, &play_land(slot))
        .unwrap_or_else(|error| panic!("play {id}: {error:?}"));
    (engine, land)
}

fn activate_mana_option(engine: &GameEngine, land: u32, option: u32) -> RuledCommand {
    let mut command = activate_ability_for(engine, land, 0, vec![]);
    let Some(Cmd::ActivateAbility(activation)) = command.cmd.as_mut() else {
        unreachable!()
    };
    activation.mana_option_index = option;
    command
}

#[test]
fn slow_lands_batch_cards_enter_tapped_with_zero_or_one_other_land() {
    for (case_index, case) in CASES.iter().enumerate() {
        for other_lands in [0usize, 1] {
            let (engine, land) = play_with_other_lands(
                25_010_000 + (case_index * 2 + other_lands) as u64,
                case.id,
                other_lands,
            );
            assert!(
                engine.state.objects[&land].tapped,
                "{} enters tapped with {other_lands} other land(s)",
                case.id
            );
        }
    }
}

#[test]
fn slow_lands_batch_cards_enter_untapped_with_two_other_lands() {
    for (case_index, case) in CASES.iter().enumerate() {
        let (engine, land) = play_with_other_lands(25_011_000 + case_index as u64, case.id, 2);
        assert!(
            !engine.state.objects[&land].tapped,
            "{} enters untapped with two other lands",
            case.id
        );
    }
}

#[test]
fn slow_lands_batch_cards_produce_each_printed_color() {
    for (case_index, case) in CASES.iter().enumerate() {
        for (option, expected) in case.options.iter().enumerate() {
            let mut engine = engine(25_012_000 + (case_index * 2 + option) as u64);
            inject_card_into_hand(&mut engine, 0, case.id);
            let land = relocate_to_battlefield(&mut engine, 0, case.id, false);

            engine
                .apply_scenario_command(0, &activate_mana_option(&engine, land, option as u32))
                .unwrap_or_else(|error| panic!("{} option {option}: {error:?}", case.id));

            assert_eq!(mana_pool(&engine), *expected, "{} option {option}", case.id);
            assert!(engine.state.objects[&land].tapped, "{}", case.id);
            assert!(
                engine.state.stack.is_empty(),
                "{}: mana abilities resolve immediately",
                case.id
            );
        }
    }
}

#[test]
fn slow_lands_batch_cards_ignore_lands_controlled_by_opponents() {
    for (case_index, case) in CASES.iter().enumerate() {
        let mut engine = engine(25_013_000 + case_index as u64);
        for _ in 0..2 {
            relocate_to_battlefield(&mut engine, 1, "island", false);
        }
        let land = inject_card_into_hand(&mut engine, 0, case.id);
        let slot = hand_index_for_card(&engine, 0, case.id);
        engine
            .apply_scenario_command(0, &play_land(slot))
            .unwrap_or_else(|error| panic!("play {}: {error:?}", case.id));
        assert!(
            engine.state.objects[&land].tapped,
            "{} counts only its controller's other lands",
            case.id
        );
    }
}
