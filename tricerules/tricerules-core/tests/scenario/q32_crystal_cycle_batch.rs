//! Actual-card scenarios for the queue-v32 Crystal artifact cycle.
//!
//! Expectations are printed Oracle values. CR 605.1a/605.3b resolve the
//! mana ability immediately; CR 702.29a-b define cycling as a hand-zone
//! activated ability; CR 118.3 requires the printed costs.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::{
    ruled_command::Cmd, AbilitySourceZone, ActivateAbility, RuledCommand,
};

struct Case {
    id: &'static str,
    colors: [char; 3],
}

const CASES: [Case; 5] = [
    Case {
        id: "indatha_crystal",
        colors: ['w', 'b', 'g'],
    },
    Case {
        id: "ketria_crystal",
        colors: ['g', 'u', 'r'],
    },
    Case {
        id: "raugrin_crystal",
        colors: ['u', 'r', 'w'],
    },
    Case {
        id: "savai_crystal",
        colors: ['r', 'w', 'b'],
    },
    Case {
        id: "zagoth_crystal",
        colors: ['b', 'g', 'u'],
    },
];

type ManaPool = (u32, u32, u32, u32, u32, u32);

fn pool(e: &GameEngine, player: usize) -> ManaPool {
    let p = &e.state.players[player].mana_pool;
    (p.white, p.blue, p.black, p.red, p.green, p.colorless)
}

fn mask(color: char) -> ManaPool {
    match color {
        'w' => (1, 0, 0, 0, 0, 0),
        'u' => (0, 1, 0, 0, 0, 0),
        'b' => (0, 0, 1, 0, 0, 0),
        'r' => (0, 0, 0, 1, 0, 0),
        'g' => (0, 0, 0, 0, 1, 0),
        _ => unreachable!("printed color"),
    }
}

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

fn cast_crystal(e: &mut GameEngine, case: &Case) -> u32 {
    give_mana(
        e,
        0,
        ManaGift {
            c: 3,
            ..Default::default()
        },
    );
    inject_card_into_hand(e, 0, case.id);
    let slot = hand_index_for_card(e, 0, case.id);
    e.apply_scenario_command(0, &cast_spell(slot, vec![]))
        .unwrap_or_else(|error| panic!("cast {}: {error:?}", case.id));
    resolve_entire_stack_two_player(e);
    let source = battlefield_object_for_card(e, 0, case.id);
    assert_eq!(
        e.state.objects[&source].zone,
        Zone::Battlefield,
        "{}",
        case.id
    );
    assert_eq!(pool(e, 0), (0, 0, 0, 0, 0, 0), "{}: exact cost", case.id);
    source
}

fn mana_command(e: &GameEngine, source: u32, option: u32) -> RuledCommand {
    let mut command = activate_ability_for(e, source, 0, vec![]);
    let Some(Cmd::ActivateAbility(activation)) = command.cmd.as_mut() else {
        unreachable!()
    };
    activation.mana_option_index = option;
    command
}

fn hand_cycling(e: &GameEngine, source: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ActivateAbility(ActivateAbility {
            source_object_id: source,
            source_zone: AbilitySourceZone::Hand as i32,
            expected_zone_change_generation: e
                .state
                .zone_change_generation
                .get(&source)
                .copied()
                .unwrap_or(0),
            ability_index: 1,
            ..Default::default()
        })),
    }
}

fn put_on_top(e: &mut GameEngine, player: usize, card: &str) -> u32 {
    let object = inject_library_card(e, player, card);
    e.state.players[player]
        .library
        .retain(|candidate| *candidate != object);
    e.state.players[player].library.push_front(object);
    object
}

#[test]
fn q32_crystal_cycle_canary_simplest() {
    let case = &CASES[0];
    let mut e = engine(32_040_001);
    cast_crystal(&mut e, case);
    for (option, color) in case.colors.into_iter().enumerate() {
        let probe = inject_permanent_on_battlefield(&mut e, 0, case.id);
        e.apply_scenario_command(0, &mana_command(&e, probe, option as u32))
            .expect("activate the printed color");
        assert_eq!(pool(&e, 0), mask(color), "option {option}");
        assert!(e.state.objects[&probe].tapped);
        e.state.players[0].mana_pool = Default::default();
    }
}

#[test]
fn q32_crystal_cycle_canary_hardest() {
    let case = &CASES[4];
    let mut e = engine(32_040_002);
    let source = cast_crystal(&mut e, case);
    e.apply_scenario_command(0, &mana_command(&e, source, 1))
        .expect("green");
    assert_eq!(pool(&e, 0), mask('g'));
    assert!(e.state.stack.is_empty());
}

#[test]
fn q32_crystal_cycle_cards_offer_three_colors_and_cycle_for_two() {
    for (index, case) in CASES.iter().enumerate() {
        let mut e = engine(32_041_000 + index as u64);
        cast_crystal(&mut e, case);
        for (option, color) in case.colors.into_iter().enumerate() {
            let probe = inject_permanent_on_battlefield(&mut e, 0, case.id);
            e.apply_scenario_command(0, &mana_command(&e, probe, option as u32))
                .unwrap_or_else(|error| panic!("{} option {option}: {error:?}", case.id));
            assert_eq!(pool(&e, 0), mask(color), "{} option {option}", case.id);
            assert!(e.state.objects[&probe].tapped, "{}", case.id);
            assert!(e.state.stack.is_empty(), "{}", case.id);
            e.state.players[0].mana_pool = Default::default();
        }

        let mut cycling = engine(32_042_000 + index as u64);
        inject_card_into_hand(&mut cycling, 0, case.id);
        let card = cycling.state.players[0].hand[hand_index_for_card(&cycling, 0, case.id)];
        let drawn = put_on_top(&mut cycling, 0, "grizzly_bears");
        let hand_before = cycling.state.players[0].hand.len();
        let library_before = cycling.state.players[0].library.len();
        let opponent_hand = cycling.state.players[1].hand.clone();
        give_mana(
            &mut cycling,
            0,
            ManaGift {
                c: 2,
                ..Default::default()
            },
        );
        cycling
            .apply_scenario_command(0, &hand_cycling(&cycling, card))
            .unwrap_or_else(|error| panic!("{}: cycle: {error:?}", case.id));
        assert_eq!(
            cycling.state.objects[&card].zone,
            Zone::Graveyard,
            "{}",
            case.id
        );
        assert_eq!(pool(&cycling, 0), (0, 0, 0, 0, 0, 0), "{}", case.id);
        resolve_entire_stack_two_player(&mut cycling);
        assert_eq!(
            cycling.state.objects[&drawn].zone,
            Zone::Hand,
            "{}",
            case.id
        );
        assert_eq!(
            cycling.state.players[0].hand.len(),
            hand_before,
            "{}",
            case.id
        );
        assert_eq!(
            cycling.state.players[0].library.len(),
            library_before - 1,
            "{}",
            case.id
        );
        assert_eq!(cycling.state.players[1].hand, opponent_hand, "{}", case.id);
    }
}

#[test]
fn q32_crystal_cycle_cards_reject_online_cycling_without_the_exact_cost() {
    for (index, case) in CASES.iter().enumerate() {
        let mut e = engine(32_043_000 + index as u64);
        let card = inject_card_into_hand(&mut e, 0, case.id);
        let hand_before = e.state.players[0].hand.len();
        assert!(
            e.apply_scenario_command(0, &hand_cycling(&e, card))
                .is_err(),
            "{}: two generic mana are required",
            case.id
        );
        assert_eq!(e.state.objects[&card].zone, Zone::Hand, "{}", case.id);
        assert_eq!(e.state.players[0].hand.len(), hand_before, "{}", case.id);
    }
}
