//! Actual-card scenarios for the reviewed Battlebond opponent-count land cycle.
//!
//! Expectations are printed Oracle values. CR 614.1d and 614.12 make the
//! conditional entry a replacement effect; CR 102.2-102.3 define opponents and,
//! per CR 800.4 and the pinned rulings, only opponents still in the game count.
//! CR 605.1a and 605.3b cover the mana ability.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;

type ManaPool = (u32, u32, u32, u32, u32, u32);

struct Case {
    id: &'static str,
    options: [ManaPool; 2],
}

const CASES: [Case; 10] = [
    Case {
        id: "bountiful_promenade",
        options: [(0, 0, 0, 0, 1, 0), (1, 0, 0, 0, 0, 0)],
    },
    Case {
        id: "luxury_suite",
        options: [(0, 0, 1, 0, 0, 0), (0, 0, 0, 1, 0, 0)],
    },
    Case {
        id: "morphic_pool",
        options: [(0, 1, 0, 0, 0, 0), (0, 0, 1, 0, 0, 0)],
    },
    Case {
        id: "rejuvenating_springs",
        options: [(0, 0, 0, 0, 1, 0), (0, 1, 0, 0, 0, 0)],
    },
    Case {
        id: "sea_of_clouds",
        options: [(1, 0, 0, 0, 0, 0), (0, 1, 0, 0, 0, 0)],
    },
    Case {
        id: "spectator_seating",
        options: [(0, 0, 0, 1, 0, 0), (1, 0, 0, 0, 0, 0)],
    },
    Case {
        id: "spire_garden",
        options: [(0, 0, 0, 1, 0, 0), (0, 0, 0, 0, 1, 0)],
    },
    Case {
        id: "training_center",
        options: [(0, 1, 0, 0, 0, 0), (0, 0, 0, 1, 0, 0)],
    },
    Case {
        id: "undergrowth_stadium",
        options: [(0, 0, 1, 0, 0, 0), (0, 0, 0, 0, 1, 0)],
    },
    Case {
        id: "vault_of_champions",
        options: [(1, 0, 0, 0, 0, 0), (0, 0, 1, 0, 0, 0)],
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

fn game(seed: u64, players: i32) -> GameEngine {
    let ids: Vec<i32> = (0..players).collect();
    let decks = Some(vec![deck_with("forest", &[]); players as usize]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &ids,
        20,
        decks,
        true,
    )
    .expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn play(engine: &mut GameEngine, id: &str) -> u32 {
    let land = inject_card_into_hand(engine, 0, id);
    let slot = hand_index_for_card(engine, 0, id);
    engine
        .apply_scenario_command(0, &play_land(slot))
        .unwrap_or_else(|error| panic!("play {id}: {error:?}"));
    assert!(
        engine.state.players[0].battlefield.contains(&land),
        "{id} is on the battlefield"
    );
    land
}

fn concede_player(engine: &mut GameEngine, player: i32, id: &str) {
    engine
        .apply_scenario_command(player, &concede())
        .unwrap_or_else(|error| panic!("{id}: player {player} concedes: {error:?}"));
    assert!(engine.state.players[player as usize].has_lost, "{id}");
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
fn battlebond_lands_enter_tapped_with_one_opponent() {
    for (case_index, case) in CASES.iter().enumerate() {
        let mut engine = game(51_900_000 + case_index as u64, 2);
        let land = play(&mut engine, case.id);
        assert!(
            engine.state.objects[&land].tapped,
            "{} enters tapped with a single opponent",
            case.id
        );
    }
}

#[test]
fn battlebond_lands_enter_untapped_with_two_or_more_opponents() {
    for (case_index, case) in CASES.iter().enumerate() {
        for players in [3i32, 4] {
            let mut engine = game(51_901_000 + case_index as u64 * 4 + players as u64, players);
            let land = play(&mut engine, case.id);
            assert!(
                !engine.state.objects[&land].tapped,
                "{} enters untapped with {} opponents",
                case.id,
                players - 1
            );
        }
    }
}

#[test]
fn battlebond_lands_count_only_opponents_still_in_the_game() {
    for (case_index, case) in CASES.iter().enumerate() {
        // Four players and one concedes: two opponents remain, so the land enters untapped.
        let mut engine = game(51_902_000 + case_index as u64, 4);
        concede_player(&mut engine, 3, case.id);
        let land = play(&mut engine, case.id);
        assert!(
            !engine.state.objects[&land].tapped,
            "{} counts the two opponents still in the game",
            case.id
        );

        // Three players and one concedes: one opponent remains, so the land enters tapped.
        let mut engine = game(51_903_000 + case_index as u64, 3);
        concede_player(&mut engine, 2, case.id);
        let land = play(&mut engine, case.id);
        assert!(
            engine.state.objects[&land].tapped,
            "{} does not count an opponent who left the game",
            case.id
        );
    }
}

#[test]
fn battlebond_lands_produce_each_printed_color() {
    for (case_index, case) in CASES.iter().enumerate() {
        for (option, expected) in case.options.iter().enumerate() {
            let mut engine = game(51_904_000 + (case_index * 2 + option) as u64, 2);
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
