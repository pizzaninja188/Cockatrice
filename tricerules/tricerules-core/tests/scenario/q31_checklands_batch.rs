//! Actual-card scenarios for the queue-v31 checkland cycle.
//!
//! Expectations are printed Oracle values. CR 614.1d/614.12 make the conditional
//! entry a replacement effect; CR 305.6 identifies the basic land subtypes; CR
//! 109.2 keeps the source itself out of the other-permanent check because it has
//! not entered yet; CR 605.1a/605.3b cover the intrinsic mana ability.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::ruled_command::Cmd;

struct Case {
    id: &'static str,
    first_basic: &'static str,
    second_basic: &'static str,
    options: [(u32, u32, u32, u32, u32, u32); 2],
}

const CASES: [Case; 8] = [
    Case {
        id: "glacial_fortress",
        first_basic: "plains",
        second_basic: "island",
        options: [(1, 0, 0, 0, 0, 0), (0, 1, 0, 0, 0, 0)],
    },
    Case {
        id: "drowned_catacomb",
        first_basic: "island",
        second_basic: "swamp",
        options: [(0, 1, 0, 0, 0, 0), (0, 0, 1, 0, 0, 0)],
    },
    Case {
        id: "dragonskull_summit",
        first_basic: "swamp",
        second_basic: "mountain",
        options: [(0, 0, 1, 0, 0, 0), (0, 0, 0, 1, 0, 0)],
    },
    Case {
        id: "isolated_chapel",
        first_basic: "plains",
        second_basic: "swamp",
        options: [(1, 0, 0, 0, 0, 0), (0, 0, 1, 0, 0, 0)],
    },
    Case {
        id: "woodland_cemetery",
        first_basic: "swamp",
        second_basic: "forest",
        options: [(0, 0, 1, 0, 0, 0), (0, 0, 0, 0, 1, 0)],
    },
    Case {
        id: "clifftop_retreat",
        first_basic: "mountain",
        second_basic: "plains",
        options: [(0, 0, 0, 1, 0, 0), (1, 0, 0, 0, 0, 0)],
    },
    Case {
        id: "sunpetal_grove",
        first_basic: "forest",
        second_basic: "plains",
        options: [(0, 0, 0, 0, 1, 0), (1, 0, 0, 0, 0, 0)],
    },
    Case {
        id: "sulfur_falls",
        first_basic: "island",
        second_basic: "mountain",
        options: [(0, 1, 0, 0, 0, 0), (0, 0, 0, 1, 0, 0)],
    },
];

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![
        deck_with("plains", &["island", "swamp", "mountain", "forest"]),
        deck_with("island", &["plains", "swamp", "mountain", "forest"]),
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

fn mana_pool(e: &GameEngine) -> (u32, u32, u32, u32, u32, u32) {
    let pool = &e.state.players[0].mana_pool;
    (
        pool.white,
        pool.blue,
        pool.black,
        pool.red,
        pool.green,
        pool.colorless,
    )
}

fn play_with_controlled_basic(
    seed: u64,
    case: &Case,
    basic: Option<&str>,
    controller: usize,
) -> (GameEngine, u32) {
    let mut e = engine(seed);
    if let Some(basic) = basic {
        relocate_to_battlefield(&mut e, controller, basic, false);
    }
    inject_card_into_hand(&mut e, 0, case.id);
    let slot = hand_index_for_card(&e, 0, case.id);
    e.apply_scenario_command(0, &play_land(slot))
        .unwrap_or_else(|error| panic!("{}: play land: {error:?}", case.id));
    let land = battlefield_object_for_card(&e, 0, case.id);
    (e, land)
}

fn activate_mana_option(e: &mut GameEngine, land: u32, option: u32) {
    let mut command = activate_ability_for(e, land, 0, vec![]);
    let Some(Cmd::ActivateAbility(activation)) = command.cmd.as_mut() else {
        unreachable!()
    };
    activation.mana_option_index = option;
    e.apply_scenario_command(0, &command)
        .unwrap_or_else(|error| panic!("mana option {option}: {error:?}"));
}

#[test]
fn q31_checklands_canary_simplest() {
    let (e, land) = play_with_controlled_basic(31_010_001, &CASES[0], Some("plains"), 0);
    assert_eq!(e.state.objects[&land].zone, Zone::Battlefield);
    assert!(!e.state.objects[&land].tapped, "glacial fortress: Plains");
    assert_eq!(mana_pool(&e), (0, 0, 0, 0, 0, 0));
}

#[test]
fn q31_checklands_canary_hardest() {
    let (e, land) = play_with_controlled_basic(31_010_002, &CASES[7], Some("mountain"), 0);
    assert_eq!(e.state.objects[&land].zone, Zone::Battlefield);
    assert!(!e.state.objects[&land].tapped, "sulfur falls: Mountain");
}

#[test]
fn q31_checklands_enter_untapped_with_either_printed_basic_type() {
    for (index, case) in CASES.iter().enumerate() {
        for (which, basic) in [case.first_basic, case.second_basic]
            .into_iter()
            .enumerate()
        {
            let (e, land) = play_with_controlled_basic(
                31_011_000 + (index * 2 + which) as u64,
                case,
                Some(basic),
                0,
            );
            assert!(
                !e.state.objects[&land].tapped,
                "{}: controls {basic}",
                case.id
            );
        }
    }
}

#[test]
fn q31_checklands_enter_tapped_without_either_printed_type() {
    for (index, case) in CASES.iter().enumerate() {
        let (e, land) = play_with_controlled_basic(31_012_000 + index as u64, case, None, 0);
        assert!(
            e.state.objects[&land].tapped,
            "{}: no matching basic land",
            case.id
        );
    }
}

#[test]
fn q31_checklands_ignore_opponents_lands() {
    for (index, case) in CASES.iter().enumerate() {
        let (e, land) =
            play_with_controlled_basic(31_013_000 + index as u64, case, Some(case.first_basic), 1);
        assert!(
            e.state.objects[&land].tapped,
            "{}: an opponent's basic land does not satisfy the condition",
            case.id
        );
    }
}

#[test]
fn q31_checklands_produce_each_printed_color() {
    for (index, case) in CASES.iter().enumerate() {
        for (option, expected) in case.options.iter().enumerate() {
            let mut e = engine(31_014_000 + (index * 2 + option) as u64);
            let land = inject_permanent_on_battlefield(&mut e, 0, case.id);
            activate_mana_option(&mut e, land, option as u32);
            assert_eq!(mana_pool(&e), *expected, "{} option {option}", case.id);
            assert!(e.state.objects[&land].tapped, "{}", case.id);
            assert!(
                e.state.stack.is_empty(),
                "{}: the mana ability does not use the stack",
                case.id
            );
        }
    }
}

#[test]
fn q31_checklands_mana_credits_the_activating_player() {
    for (index, case) in CASES.iter().enumerate() {
        for option in 0..2u32 {
            let mut e = engine(31_016_000 + (index * 2 + option as usize) as u64);
            let land = inject_permanent_on_battlefield(&mut e, 1, case.id);
            e.apply_scenario_command(0, &pass())
                .expect("pass to player 1");
            let mut command = activate_ability_for(&e, land, 0, vec![]);
            let Some(Cmd::ActivateAbility(activation)) = command.cmd.as_mut() else {
                unreachable!()
            };
            activation.mana_option_index = option;
            e.apply_scenario_command(1, &command)
                .unwrap_or_else(|error| panic!("{} option {option}: {error:?}", case.id));
            let player_one = {
                let pool = &e.state.players[1].mana_pool;
                (
                    pool.white,
                    pool.blue,
                    pool.black,
                    pool.red,
                    pool.green,
                    pool.colorless,
                )
            };
            assert_eq!(
                player_one, case.options[option as usize],
                "{} option {option}: activating player receives the mana",
                case.id
            );
            assert_eq!(
                mana_pool(&e),
                (0, 0, 0, 0, 0, 0),
                "{} option {option}: the other player receives nothing",
                case.id
            );
        }
    }
}

#[test]
fn q31_checklands_reject_a_second_activation_while_tapped() {
    for (index, case) in CASES.iter().enumerate() {
        let mut e = engine(31_015_000 + index as u64);
        let land = inject_permanent_on_battlefield(&mut e, 0, case.id);
        activate_mana_option(&mut e, land, 0);
        let mut command = activate_ability_for(&e, land, 0, vec![]);
        let Some(Cmd::ActivateAbility(activation)) = command.cmd.as_mut() else {
            unreachable!()
        };
        activation.mana_option_index = 0;
        assert!(
            e.apply_scenario_command(0, &command).is_err(),
            "{}: a tapped land cannot produce again",
            case.id
        );
    }
}
