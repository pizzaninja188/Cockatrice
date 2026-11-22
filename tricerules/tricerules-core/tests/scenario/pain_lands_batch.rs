//! Actual-card scenarios for the reviewed pain-land cycle.
//!
//! Expectations are printed Oracle values. CR 605.1a/605.3a-b make the targetless
//! mana abilities resolve immediately; CR 120.3a deals the printed 1 damage to
//! the ability's controller; CR 118.3 requires the tap cost.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_proto::ruled::v1::ruled_command::Cmd;

type Pool = (u32, u32, u32, u32, u32, u32);

struct Case {
    id: &'static str,
    first: char,
    second: char,
}

fn cases() -> [Case; 8] {
    [
        Case {
            id: "shivan_reef",
            first: 'u',
            second: 'r',
        },
        Case {
            id: "llanowar_wastes",
            first: 'b',
            second: 'g',
        },
        Case {
            id: "caves_of_koilos",
            first: 'w',
            second: 'b',
        },
        Case {
            id: "brushland",
            first: 'g',
            second: 'w',
        },
        Case {
            id: "battlefield_forge",
            first: 'r',
            second: 'w',
        },
        Case {
            id: "underground_river",
            first: 'u',
            second: 'b',
        },
        Case {
            id: "adarkar_wastes",
            first: 'w',
            second: 'u',
        },
        Case {
            id: "sulfurous_springs",
            first: 'b',
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
fn pain_lands_batch_cards_add_colorless_or_each_color_and_deal_one_damage() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut colorless = engine(10_200_000 + index as u64, case.id);
        let source = inject_permanent_on_battlefield(&mut colorless, 0, case.id);
        activate(&mut colorless, source, 0, 0);
        assert_eq!(
            pool(&colorless, 0),
            (0, 0, 0, 0, 0, 1),
            "{}: one colorless mana",
            case.id
        );
        assert_eq!(
            colorless.state.players[0].life, 20,
            "{}: the colorless ability deals no damage",
            case.id
        );
        assert!(colorless.state.stack.is_empty(), "{}", case.id);

        for (offset, (option, color)) in [(0, case.first), (1, case.second)].into_iter().enumerate()
        {
            let mut e = engine(10_210_000 + index as u64 * 10 + offset as u64, case.id);
            let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
            activate(&mut e, source, 1, option);
            assert_eq!(
                pool(&e, 0),
                mask(color),
                "{} option {option}: exact printed color",
                case.id
            );
            assert_eq!(
                e.state.players[0].life, 19,
                "{} option {option}: exactly one self-damage",
                case.id
            );
            assert_eq!(
                e.state.players[1].life, 20,
                "{} option {option}: the opponent is not damaged",
                case.id
            );
            assert!(e.state.objects[&source].tapped, "{}", case.id);
            assert!(
                e.state.stack.is_empty(),
                "{}: the pain mana ability uses no stack",
                case.id
            );
            assert_eq!(e.state.objects[&source].controller, 0, "{}", case.id);
        }
    }
}

#[test]
fn pain_lands_batch_cards_require_tap_and_untapped_source() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(10_220_000 + index as u64, case.id);
        let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
        activate(&mut e, source, 0, 0);
        assert!(
            !try_activate(&mut e, source, 0, 0),
            "{}: a tapped source cannot pay {{T}} again",
            case.id
        );
        assert!(
            !try_activate(&mut e, source, 1, 0),
            "{}: a tapped source cannot use the pain ability",
            case.id
        );
        assert_eq!(pool(&e, 0), (0, 0, 0, 0, 0, 1), "{}", case.id);
        assert_eq!(e.state.players[0].life, 20, "{}", case.id);

        let mut foreign = engine(10_230_000 + index as u64, case.id);
        let foreign_source = inject_permanent_on_battlefield(&mut foreign, 0, case.id);
        assert!(
            foreign
                .apply_scenario_command(1, &command(&foreign, foreign_source, 1, 0))
                .is_err(),
            "{}: another player cannot activate it",
            case.id
        );
        assert_eq!(pool(&foreign, 1), (0, 0, 0, 0, 0, 0), "{}", case.id);
        assert_eq!(foreign.state.players[1].life, 20, "{}", case.id);
        assert!(
            !foreign.state.objects[&foreign_source].tapped,
            "{}",
            case.id
        );
    }
}

#[test]
fn pain_lands_batch_cards_do_not_damage_when_activation_is_rejected() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(10_240_000 + index as u64, case.id);
        let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
        activate(&mut e, source, 1, 0);
        assert_eq!(e.state.players[0].life, 19, "{}", case.id);
        assert_eq!(pool(&e, 0), mask(case.first), "{}", case.id);
        assert!(
            !try_activate(&mut e, source, 1, 1),
            "{}: the tapped source cannot deal damage again",
            case.id
        );
        assert_eq!(
            e.state.players[0].life, 19,
            "{}: a rejected activation deals no damage",
            case.id
        );
        assert_eq!(
            pool(&e, 0),
            mask(case.first),
            "{}: a rejected activation adds no mana",
            case.id
        );
    }
}
