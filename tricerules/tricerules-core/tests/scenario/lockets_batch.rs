//! Actual-card scenarios for the queue-v26 Locket sacrifice-draw cycle.
//!
//! Expectations are printed Oracle values. CR 107.4e lets each hybrid symbol be
//! paid with either half; CR 605.1a/605.3a-b make the targetless mana ability
//! resolve immediately; CR 118.3 and 701.21a require the tap and sacrifice
//! costs; CR 121.1 draws exactly two cards.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::ruled_command::Cmd;

type Pool = (u32, u32, u32, u32, u32, u32);

struct Case {
    id: &'static str,
    first: char,
    second: char,
}

const CASES: [Case; 10] = [
    Case {
        id: "azorius_locket",
        first: 'w',
        second: 'u',
    },
    Case {
        id: "boros_locket",
        first: 'r',
        second: 'w',
    },
    Case {
        id: "dimir_locket",
        first: 'u',
        second: 'b',
    },
    Case {
        id: "golgari_locket",
        first: 'b',
        second: 'g',
    },
    Case {
        id: "gruul_locket",
        first: 'r',
        second: 'g',
    },
    Case {
        id: "izzet_locket",
        first: 'u',
        second: 'r',
    },
    Case {
        id: "orzhov_locket",
        first: 'w',
        second: 'b',
    },
    Case {
        id: "rakdos_locket",
        first: 'b',
        second: 'r',
    },
    Case {
        id: "selesnya_locket",
        first: 'g',
        second: 'w',
    },
    Case {
        id: "simic_locket",
        first: 'g',
        second: 'u',
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

fn pool(e: &GameEngine) -> Pool {
    let p = &e.state.players[0].mana_pool;
    (p.white, p.blue, p.black, p.red, p.green, p.colorless)
}

fn mask(color: char) -> Pool {
    match color {
        'w' => (1, 0, 0, 0, 0, 0),
        'u' => (0, 1, 0, 0, 0, 0),
        'b' => (0, 0, 1, 0, 0, 0),
        'r' => (0, 0, 0, 1, 0, 0),
        'g' => (0, 0, 0, 0, 1, 0),
        _ => unreachable!("printed color"),
    }
}

fn two_of(color: char) -> ManaGift {
    match color {
        'w' => ManaGift {
            w: 2,
            ..ManaGift::default()
        },
        'u' => ManaGift {
            u: 2,
            ..ManaGift::default()
        },
        'b' => ManaGift {
            b: 2,
            ..ManaGift::default()
        },
        'r' => ManaGift {
            r: 2,
            ..ManaGift::default()
        },
        'g' => ManaGift {
            g: 2,
            ..ManaGift::default()
        },
        _ => unreachable!("printed color"),
    }
}

fn mana_command(
    e: &GameEngine,
    artifact: u32,
    option: u32,
) -> tricerules_proto::ruled::v1::RuledCommand {
    let mut command = activate_ability_for(e, artifact, 0, vec![]);
    let Some(Cmd::ActivateAbility(activation)) = command.cmd.as_mut() else {
        unreachable!()
    };
    activation.mana_option_index = option;
    command
}

#[test]
fn lockets_batch_cards_produce_each_printed_color_and_tap() {
    for (case_index, case) in CASES.iter().enumerate() {
        for (option, color) in [(0, case.first), (1, case.second)] {
            let mut e = engine(26_030_000 + (case_index * 2 + option) as u64);
            let artifact = inject_permanent_on_battlefield(&mut e, 0, case.id);
            e.apply_scenario_command(0, &mana_command(&e, artifact, option as u32))
                .unwrap_or_else(|error| panic!("{} option {option}: {error:?}", case.id));
            assert_eq!(pool(&e), mask(color), "{} option {option}", case.id);
            assert!(e.state.objects[&artifact].tapped, "{}", case.id);
            assert!(e.state.stack.is_empty(), "{}", case.id);
        }
    }
}

#[test]
fn lockets_batch_cards_pay_four_hybrid_symbols_and_draw_two() {
    for (index, case) in CASES.iter().enumerate() {
        let mut e = engine(26_031_000 + index as u64);
        let artifact = inject_permanent_on_battlefield(&mut e, 0, case.id);
        give_mana(&mut e, 0, two_of(case.first));
        give_mana(&mut e, 0, two_of(case.second));
        let hand_before = e.state.players[0].hand.len();
        let library_before = e.state.players[0].library.len();

        e.apply_scenario_command(0, &activate_ability_for(&e, artifact, 1, vec![]))
            .unwrap_or_else(|error| panic!("{}: activate draw: {error:?}", case.id));
        resolve_entire_stack_two_player(&mut e);

        assert_eq!(
            pool(&e),
            (0, 0, 0, 0, 0, 0),
            "{}: four symbols consumed",
            case.id
        );
        assert_eq!(
            e.state.players[0].hand.len(),
            hand_before + 2,
            "{}: draws exactly two",
            case.id
        );
        assert_eq!(
            e.state.players[0].library.len(),
            library_before - 2,
            "{}",
            case.id
        );
        assert_eq!(
            e.state.objects[&artifact].zone,
            Zone::Graveyard,
            "{}: the artifact sacrifices itself",
            case.id
        );
    }
}

#[test]
fn lockets_batch_cards_reject_the_draw_with_too_little_mana() {
    for (index, case) in CASES.iter().enumerate() {
        let mut e = engine(26_032_000 + index as u64);
        let artifact = inject_permanent_on_battlefield(&mut e, 0, case.id);
        give_mana(&mut e, 0, two_of(case.first));
        give_mana(
            &mut e,
            0,
            ManaGift {
                c: 1,
                ..ManaGift::default()
            },
        );
        let hand_before = e.state.players[0].hand.len();
        let before = e.state.command_index;
        e.apply_scenario_command(0, &activate_ability_for(&e, artifact, 1, vec![]))
            .expect_err("three mana cannot pay four hybrid symbols");
        assert_eq!(e.state.command_index, before, "{}", case.id);
        assert_eq!(e.state.players[0].hand.len(), hand_before, "{}", case.id);
        assert_eq!(
            e.state.objects[&artifact].zone,
            Zone::Battlefield,
            "{}",
            case.id
        );
    }
}
