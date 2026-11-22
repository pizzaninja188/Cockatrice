//! Actual-card scenarios for the queue-v26 entered-tapped pain-land cycle.
//!
//! Expectations are printed Oracle values. CR 614.1d/614.12 applies the
//! entered-tapped replacement; CR 605.1a/605.3a-b make each targetless mana
//! ability resolve immediately; CR 120.3a turns the printed 1 damage into 1
//! life lost by the ability's controller; CR 118.3 requires the tap cost.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_proto::ruled::v1::ruled_command::Cmd;

type Pool = (u32, u32, u32, u32, u32, u32);

struct Case {
    id: &'static str,
    first: char,
    second: char,
}

const CASES: [Case; 5] = [
    Case {
        id: "skyshroud_forest",
        first: 'g',
        second: 'u',
    },
    Case {
        id: "scabland",
        first: 'r',
        second: 'w',
    },
    Case {
        id: "pine_barrens",
        first: 'b',
        second: 'g',
    },
    Case {
        id: "salt_flats",
        first: 'w',
        second: 'b',
    },
    Case {
        id: "caldera_lake",
        first: 'u',
        second: 'r',
    },
];

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

#[test]
fn pain_taplands_batch_cards_enter_tapped() {
    for (index, case) in CASES.iter().enumerate() {
        let mut e = engine(26_010_000 + index as u64, case.id);
        let source = move_ready_to_battlefield(&mut e, 0, case.id);
        assert!(e.state.objects[&source].tapped, "{} enters tapped", case.id);
    }
}

#[test]
fn pain_taplands_batch_cards_add_colorless_or_each_color_and_deal_one_damage() {
    for (index, case) in CASES.iter().enumerate() {
        let mut colorless = engine(26_011_000 + index as u64, case.id);
        let source = inject_permanent_on_battlefield(&mut colorless, 0, case.id);
        colorless
            .apply_scenario_command(0, &command(&colorless, source, 0, 0))
            .unwrap_or_else(|error| panic!("{} colorless: {error:?}", case.id));
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
            let mut e = engine(26_012_000 + index as u64 * 10 + offset as u64, case.id);
            let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
            e.apply_scenario_command(0, &command(&e, source, 1, option))
                .unwrap_or_else(|error| panic!("{} option {option}: {error:?}", case.id));
            assert_eq!(
                pool(&e, 0),
                mask(color),
                "{}: option {option} produces its printed color",
                case.id
            );
            assert_eq!(
                e.state.players[0].life, 19,
                "{}: option {option} deals exactly one damage",
                case.id
            );
            assert_eq!(
                e.state.players[1].life, 20,
                "{}: the opponent is untouched",
                case.id
            );
            assert!(e.state.objects[&source].tapped, "{}", case.id);
            assert!(e.state.stack.is_empty(), "{}", case.id);
        }
    }
}

#[test]
fn pain_taplands_batch_cards_reject_a_second_activation_while_tapped() {
    for (index, case) in CASES.iter().enumerate() {
        let mut e = engine(26_013_000 + index as u64, case.id);
        let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
        e.apply_scenario_command(0, &command(&e, source, 0, 0))
            .expect("first activation");
        let before = e.state.command_index;
        e.apply_scenario_command(0, &command(&e, source, 1, 0))
            .expect_err("a tapped land cannot pay the tap cost again");
        assert_eq!(e.state.command_index, before, "{}", case.id);
        assert_eq!(
            pool(&e, 0),
            (0, 0, 0, 0, 0, 1),
            "{}: the rejected activation produces no mana",
            case.id
        );
        assert_eq!(e.state.players[0].life, 20, "{}", case.id);
    }
}
