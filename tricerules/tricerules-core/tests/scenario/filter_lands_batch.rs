//! Actual-card scenarios for the reviewed Shadowmoor filter-land cycle.
//!
//! Expectations are printed Oracle values. CR 107.4e makes each hybrid symbol
//! payable with either half; CR 605.1a and 605.3b classify and immediately
//! resolve the targetless mana abilities; CR 106.4 puts the chosen mana in the
//! controller's pool; CR 118.3 requires both printed costs.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_proto::ruled::v1::ruled_command::Cmd;

type Pool = (u32, u32, u32, u32, u32, u32);

struct Case {
    id: &'static str,
    first: char,
    second: char,
}

fn cases() -> [Case; 9] {
    [
        Case {
            id: "fetid_heath",
            first: 'w',
            second: 'b',
        },
        Case {
            id: "graven_cairns",
            first: 'b',
            second: 'r',
        },
        Case {
            id: "wooded_bastion",
            first: 'g',
            second: 'w',
        },
        Case {
            id: "rugged_prairie",
            first: 'r',
            second: 'w',
        },
        Case {
            id: "twilight_mire",
            first: 'b',
            second: 'g',
        },
        Case {
            id: "flooded_grove",
            first: 'g',
            second: 'u',
        },
        Case {
            id: "sunken_ruins",
            first: 'u',
            second: 'b',
        },
        Case {
            id: "mystic_gate",
            first: 'w',
            second: 'u',
        },
        Case {
            id: "cascade_bluffs",
            first: 'u',
            second: 'r',
        },
    ]
}

fn engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![deck_with("forest", &[card]), deck_with("island", &[])]);
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

fn pool(e: &GameEngine, player: usize) -> Pool {
    let p = &e.state.players[player].mana_pool;
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

fn twice(color: char) -> Pool {
    let m = mask(color);
    (m.0 * 2, m.1 * 2, m.2 * 2, m.3 * 2, m.4 * 2, m.5 * 2)
}

fn mixed(first: char, second: char) -> Pool {
    let a = mask(first);
    let b = mask(second);
    (a.0 + b.0, a.1 + b.1, a.2 + b.2, a.3 + b.3, a.4 + b.4, 0)
}

fn gift(color: char) -> ManaGift {
    match color {
        'w' => ManaGift {
            w: 1,
            ..Default::default()
        },
        'u' => ManaGift {
            u: 1,
            ..Default::default()
        },
        'b' => ManaGift {
            b: 1,
            ..Default::default()
        },
        'r' => ManaGift {
            r: 1,
            ..Default::default()
        },
        'g' => ManaGift {
            g: 1,
            ..Default::default()
        },
        _ => unreachable!("printed color"),
    }
}

fn command(
    e: &GameEngine,
    source: u32,
    ability: u32,
    option: u32,
) -> tricerules_proto::ruled::v1::RuledCommand {
    let mut command = activate_ability_for(e, source, ability, vec![]);
    let Some(Cmd::ActivateAbility(activation)) = command.cmd.as_mut() else {
        unreachable!("activate_ability_for builds an ActivateAbility command")
    };
    activation.mana_option_index = option;
    command
}

fn try_activate(e: &mut GameEngine, source: u32, ability: u32, option: u32) -> bool {
    e.apply_scenario_command(0, &command(e, source, ability, option))
        .is_ok()
}

fn activate(e: &mut GameEngine, source: u32, ability: u32, option: u32) {
    e.apply_scenario_command(0, &command(e, source, ability, option))
        .expect("activate the printed mana ability");
}

#[test]
fn filter_lands_batch_cards_add_colorless_and_each_filtered_pair() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut colorless = engine(10_100_000 + index as u64, case.id);
        let source = inject_permanent_on_battlefield(&mut colorless, 0, case.id);
        activate(&mut colorless, source, 0, 0);
        assert_eq!(
            pool(&colorless, 0),
            (0, 0, 0, 0, 0, 1),
            "{}: one colorless mana",
            case.id
        );
        assert!(colorless.state.objects[&source].tapped, "{}", case.id);
        assert!(
            colorless.state.stack.is_empty(),
            "{}: the mana ability uses no stack",
            case.id
        );

        let activations = [
            (0, twice(case.first), case.first),
            (1, mixed(case.first, case.second), case.first),
            (2, twice(case.second), case.second),
        ];
        for (offset, (option, expected, paid)) in activations.into_iter().enumerate() {
            let mut e = engine(10_110_000 + index as u64 * 10 + offset as u64, case.id);
            let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
            give_mana(&mut e, 0, gift(paid));
            activate(&mut e, source, 1, option);
            assert_eq!(
                pool(&e, 0),
                expected,
                "{} option {option}: exact printed output",
                case.id
            );
            assert_eq!(
                pool(&e, 1),
                (0, 0, 0, 0, 0, 0),
                "{} option {option}: opponent pool unchanged",
                case.id
            );
            assert!(e.state.objects[&source].tapped, "{}", case.id);
            assert!(
                e.state.stack.is_empty(),
                "{}: the filtered mana ability uses no stack",
                case.id
            );
            assert_eq!(e.state.objects[&source].controller, 0, "{}", case.id);
        }
    }
}

#[test]
fn filter_lands_batch_cards_require_the_printed_hybrid_and_tap_costs() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(10_120_000 + index as u64, case.id);
        let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
        assert!(
            !try_activate(&mut e, source, 1, 0),
            "{}: the hybrid cost cannot be skipped",
            case.id
        );
        assert!(!e.state.objects[&source].tapped, "{}", case.id);
        assert_eq!(pool(&e, 0), (0, 0, 0, 0, 0, 0), "{}", case.id);

        give_mana(
            &mut e,
            0,
            ManaGift {
                c: 1,
                ..Default::default()
            },
        );
        assert!(
            !try_activate(&mut e, source, 1, 0),
            "{}: colorless mana cannot pay a hybrid symbol",
            case.id
        );
        assert!(!e.state.objects[&source].tapped, "{}", case.id);
        assert_eq!(pool(&e, 0), (0, 0, 0, 0, 0, 1), "{}", case.id);

        give_mana(&mut e, 0, gift(case.first));
        assert!(
            try_activate(&mut e, source, 1, 0),
            "{}: the first hybrid half is payable",
            case.id
        );
        assert!(e.state.objects[&source].tapped, "{}", case.id);
        let output = twice(case.first);
        assert_eq!(
            pool(&e, 0),
            (output.0, output.1, output.2, output.3, output.4, 1),
            "{}: hybrid payment and exact output",
            case.id
        );
        assert!(e.state.stack.is_empty(), "{}", case.id);

        let mut tapped = engine(10_130_000 + index as u64, case.id);
        let tapped_source = inject_permanent_on_battlefield(&mut tapped, 0, case.id);
        activate(&mut tapped, tapped_source, 0, 0);
        assert!(
            !try_activate(&mut tapped, tapped_source, 0, 0),
            "{}: a tapped source cannot pay {{T}} again",
            case.id
        );
        assert_eq!(pool(&tapped, 0), (0, 0, 0, 0, 0, 1), "{}", case.id);
    }
}

#[test]
fn filter_lands_batch_cards_reject_a_tapped_or_foreign_source() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(10_140_000 + index as u64, case.id);
        let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
        assert!(
            e.apply_scenario_command(1, &command(&e, source, 0, 0))
                .is_err(),
            "{}: another player cannot activate it",
            case.id
        );
        assert!(!e.state.objects[&source].tapped, "{}", case.id);
        assert_eq!(pool(&e, 1), (0, 0, 0, 0, 0, 0), "{}", case.id);
        assert_eq!(e.state.objects[&source].controller, 0, "{}", case.id);
        activate(&mut e, source, 0, 0);
        assert_eq!(pool(&e, 0), (0, 0, 0, 0, 0, 1), "{}", case.id);
    }
}
