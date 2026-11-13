//! Actual-card scenarios for the reviewed Zendikar refuge land cycle.
//!
//! Expectations are printed Oracle values. CR 603.6d and 614.1d/614.12 make the land enter tapped; CR 603.6a stages the entry trigger and CR 119.3 gains exactly one life; CR 605.1a/605.3b make each printed mana ability resolve without the stack.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_proto::ruled::v1::ruled_command::Cmd;

struct Case {
    id: &'static str,
    first: char,
    second: char,
}

fn cases() -> [Case; 5] {
    [
        Case {
            id: "akoum_refuge",
            first: 'b',
            second: 'r',
        },
        Case {
            id: "graypelt_refuge",
            first: 'g',
            second: 'w',
        },
        Case {
            id: "jwar_isle_refuge",
            first: 'u',
            second: 'b',
        },
        Case {
            id: "kazandu_refuge",
            first: 'r',
            second: 'g',
        },
        Case {
            id: "sejiri_refuge",
            first: 'w',
            second: 'u',
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

fn command(e: &GameEngine, source: u32, option: u32) -> tricerules_proto::ruled::v1::RuledCommand {
    let mut command = activate_ability_for(e, source, 0, vec![]);
    let Some(Cmd::ActivateAbility(activation)) = command.cmd.as_mut() else {
        unreachable!("activate_ability_for builds an ActivateAbility command")
    };
    activation.mana_option_index = option;
    command
}

fn try_activate(e: &mut GameEngine, source: u32, option: u32) -> bool {
    e.apply_scenario_command(0, &command(e, source, option))
        .is_ok()
}

fn pool(e: &GameEngine, player: usize) -> (u32, u32, u32, u32, u32, u32) {
    let p = &e.state.players[player].mana_pool;
    (p.white, p.blue, p.black, p.red, p.green, p.colorless)
}

fn mask(color: char) -> (u32, u32, u32, u32, u32, u32) {
    match color {
        'w' => (1, 0, 0, 0, 0, 0),
        'u' => (0, 1, 0, 0, 0, 0),
        'b' => (0, 0, 1, 0, 0, 0),
        'r' => (0, 0, 0, 1, 0, 0),
        'g' => (0, 0, 0, 0, 1, 0),
        _ => unreachable!("printed color"),
    }
}

#[test]
fn refuge_lands_batch_cards_enter_tapped_gain_one_life_and_tap_for_each_printed_color() {
    for (index, case) in cases().into_iter().enumerate() {
        for (offset, (option, color)) in [(0, case.first), (1, case.second)].into_iter().enumerate()
        {
            let mut e = engine(11_100_000 + index as u64 * 10 + offset as u64, case.id);
            let card = inject_card_into_hand(&mut e, 0, case.id);
            let slot = e.state.players[0]
                .hand
                .iter()
                .position(|oid| *oid == card)
                .expect("the injected card is in hand");
            e.apply_scenario_command(0, &play_land(slot))
                .unwrap_or_else(|error| panic!("play {}: {error}", case.id));
            let land = battlefield_object_for_card(&e, 0, case.id);
            assert_eq!(land, card, "{}: the played object", case.id);
            assert!(e.state.objects[&land].tapped, "{}: enters tapped", case.id);
            assert_eq!(
                e.state.players[0].life, 20,
                "{}: no life before the trigger resolves",
                case.id
            );
            pass_both_players(&mut e);
            assert_eq!(
                e.state.players[0].life, 21,
                "{}: entry gains exactly one life",
                case.id
            );
            assert_eq!(
                e.state.players[1].life, 20,
                "{}: the opponent does not gain",
                case.id
            );

            e.state.objects.get_mut(&land).expect("land").tapped = false;
            e.apply_scenario_command(0, &command(&e, land, option))
                .unwrap_or_else(|error| panic!("{} option {option}: {error}", case.id));
            assert_eq!(
                pool(&e, 0),
                mask(color),
                "{} option {option}: exact printed color",
                case.id
            );
            assert!(
                e.state.objects[&land].tapped,
                "{}: the ability taps the land",
                case.id
            );
            assert!(
                e.state.stack.is_empty(),
                "{}: the mana ability uses no stack",
                case.id
            );
            assert_eq!(
                e.state.players[0].life, 21,
                "{}: the mana ability does not change life",
                case.id
            );
        }
    }
}

#[test]
fn refuge_lands_batch_cards_require_tap_and_untapped_source() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(11_200_000 + index as u64, case.id);
        let land = inject_permanent_on_battlefield(&mut e, 0, case.id);
        try_activate(&mut e, land, 0);
        assert_eq!(pool(&e, 0), mask(case.first), "{}", case.id);
        assert!(
            !try_activate(&mut e, land, 1),
            "{}: a tapped land cannot produce mana again",
            case.id
        );
        assert_eq!(pool(&e, 0), mask(case.first), "{}", case.id);

        let mut foreign = engine(11_210_000 + index as u64, case.id);
        let foreign_land = inject_permanent_on_battlefield(&mut foreign, 0, case.id);
        assert!(
            foreign
                .apply_scenario_command(1, &command(&foreign, foreign_land, 0))
                .is_err(),
            "{}: another player cannot activate it",
            case.id
        );
        assert_eq!(pool(&foreign, 1), (0, 0, 0, 0, 0, 0), "{}", case.id);
        assert!(!foreign.state.objects[&foreign_land].tapped, "{}", case.id);
    }
}

#[test]
fn refuge_lands_batch_cards_do_not_gain_life_when_they_do_not_enter() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(11_220_000 + index as u64, case.id);
        inject_graveyard_card(&mut e, 0, case.id);
        pass_both_players(&mut e);
        assert_eq!(
            e.state.players[0].life, 20,
            "{}: no entry, no life",
            case.id
        );
        assert_eq!(
            e.state.players[1].life, 20,
            "{}: opponent unchanged",
            case.id
        );
    }
}
