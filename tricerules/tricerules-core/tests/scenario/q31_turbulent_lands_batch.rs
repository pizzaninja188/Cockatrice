//! Actual-card scenarios for the queue-v31 turbulent land cycle.
//!
//! Expectations are printed Oracle values. CR 614.1d/614.12 make the entry
//! replacement effect conditional on the live opponent land count; CR 109.2
//! keeps the entering land out of that count; CR 305.6/305.8 give the printed
//! basic land types; CR 605.1a/605.3b cover the intrinsic mana ability.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::ruled_command::Cmd;

struct Case {
    id: &'static str,
    options: [(u32, u32, u32, u32, u32, u32); 2],
}

const CASES: [Case; 5] = [
    Case {
        id: "turbulent_fen",
        options: [(0, 0, 1, 0, 0, 0), (0, 0, 0, 0, 1, 0)],
    },
    Case {
        id: "turbulent_moor",
        options: [(1, 0, 0, 0, 0, 0), (0, 0, 1, 0, 0, 0)],
    },
    Case {
        id: "turbulent_springs",
        options: [(0, 1, 0, 0, 0, 0), (0, 0, 0, 1, 0, 0)],
    },
    Case {
        id: "turbulent_wilderness",
        options: [(0, 0, 0, 0, 1, 0), (0, 1, 0, 0, 0, 0)],
    },
    Case {
        id: "turbulent_steppe",
        options: [(0, 0, 0, 1, 0, 0), (1, 0, 0, 0, 0, 0)],
    },
];

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("forest", &[]), deck_with("island", &[])]);
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

fn mana_pool(e: &GameEngine, player: usize) -> (u32, u32, u32, u32, u32, u32) {
    let pool = &e.state.players[player].mana_pool;
    (
        pool.white,
        pool.blue,
        pool.black,
        pool.red,
        pool.green,
        pool.colorless,
    )
}

fn play_with_opponent_lands(seed: u64, case: &Case, opponent_lands: usize) -> (GameEngine, u32) {
    let mut e = engine(seed);
    for _ in 0..opponent_lands {
        relocate_to_battlefield(&mut e, 1, "island", false);
    }
    inject_card_into_hand(&mut e, 0, case.id);
    let slot = hand_index_for_card(&e, 0, case.id);
    e.apply_scenario_command(0, &play_land(slot))
        .unwrap_or_else(|error| panic!("{}: play land: {error:?}", case.id));
    let land = battlefield_object_for_card(&e, 0, case.id);
    (e, land)
}

#[test]
fn q31_turbulent_lands_canary_simplest() {
    let (e, land) = play_with_opponent_lands(31_030_001, &CASES[0], 0);
    assert_eq!(e.state.objects[&land].zone, Zone::Battlefield);
    assert!(e.state.objects[&land].tapped, "no opponent lands");
}

#[test]
fn q31_turbulent_lands_canary_hardest() {
    let (e, land) = play_with_opponent_lands(31_030_002, &CASES[4], 8);
    assert!(!e.state.objects[&land].tapped, "eight opponent lands");
}

#[test]
fn q31_turbulent_lands_enter_tapped_below_eight_opponent_lands() {
    for (index, case) in CASES.iter().enumerate() {
        for count in [0usize, 1, 7] {
            let (e, land) = play_with_opponent_lands(
                31_031_000 + (index * 3 + count.min(2)) as u64,
                case,
                count,
            );
            assert!(
                e.state.objects[&land].tapped,
                "{}: {count} opponent land(s)",
                case.id
            );
        }
    }
}

#[test]
fn q31_turbulent_lands_enter_untapped_with_eight_opponent_lands() {
    for (index, case) in CASES.iter().enumerate() {
        let (e, land) = play_with_opponent_lands(31_032_000 + index as u64, case, 8);
        assert!(
            !e.state.objects[&land].tapped,
            "{}: eight opponent lands",
            case.id
        );
    }
}

#[test]
fn q31_turbulent_lands_ignore_your_own_lands() {
    for (index, case) in CASES.iter().enumerate() {
        let mut e = engine(31_033_000 + index as u64);
        for _ in 0..8 {
            relocate_to_battlefield(&mut e, 0, "forest", false);
        }
        inject_card_into_hand(&mut e, 0, case.id);
        let slot = hand_index_for_card(&e, 0, case.id);
        e.apply_scenario_command(0, &play_land(slot))
            .unwrap_or_else(|error| panic!("{}: {error:?}", case.id));
        let land = battlefield_object_for_card(&e, 0, case.id);
        assert!(
            e.state.objects[&land].tapped,
            "{}: the controller's own lands do not count",
            case.id
        );
    }
}

#[test]
fn q31_turbulent_lands_produce_each_printed_pair() {
    for (index, case) in CASES.iter().enumerate() {
        for (option, expected) in case.options.iter().enumerate() {
            let mut e = engine(31_034_000 + (index * 2 + option) as u64);
            let land = inject_permanent_on_battlefield(&mut e, 0, case.id);
            let mut command = activate_ability_for(&e, land, 0, vec![]);
            let Some(Cmd::ActivateAbility(activation)) = command.cmd.as_mut() else {
                unreachable!()
            };
            activation.mana_option_index = option as u32;
            e.apply_scenario_command(0, &command)
                .unwrap_or_else(|error| panic!("{} option {option}: {error:?}", case.id));
            assert_eq!(mana_pool(&e, 0), *expected, "{} option {option}", case.id);
            assert!(e.state.objects[&land].tapped, "{}", case.id);
            assert!(e.state.stack.is_empty(), "{}", case.id);
        }
    }
}
