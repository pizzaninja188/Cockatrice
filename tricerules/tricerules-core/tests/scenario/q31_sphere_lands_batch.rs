//! Actual-card scenarios for the queue-v31 Sphere land cycle.
//!
//! Expectations are printed Oracle values. CR 614.1d/614.12 make the entry
//! replacement effect tap the land; CR 605.1a/605.3b cover the intrinsic mana
//! ability; CR 118.3/701.21a pay the tap-and-sacrifice cost and CR 121 draws.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;

struct Case {
    id: &'static str,
    color: char,
}

const CASES: [Case; 5] = [
    Case {
        id: "the_dross_pits",
        color: 'b',
    },
    Case {
        id: "the_fair_basilica",
        color: 'w',
    },
    Case {
        id: "the_hunter_maze",
        color: 'g',
    },
    Case {
        id: "the_surgical_bay",
        color: 'u',
    },
    Case {
        id: "the_autonomous_furnace",
        color: 'r',
    },
];

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("swamp", &[]), deck_with("island", &[])]);
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

fn expected(case: &Case) -> (u32, u32, u32, u32, u32, u32) {
    match case.color {
        'w' => (1, 0, 0, 0, 0, 0),
        'u' => (0, 1, 0, 0, 0, 0),
        'b' => (0, 0, 1, 0, 0, 0),
        'r' => (0, 0, 0, 1, 0, 0),
        'g' => (0, 0, 0, 0, 1, 0),
        _ => unreachable!(),
    }
}

fn play_sphere(e: &mut GameEngine, case: &Case) -> u32 {
    inject_card_into_hand(e, 0, case.id);
    let slot = hand_index_for_card(e, 0, case.id);
    e.apply_scenario_command(0, &play_land(slot))
        .unwrap_or_else(|error| panic!("{}: play: {error:?}", case.id));
    battlefield_object_for_card(e, 0, case.id)
}

#[test]
fn q31_sphere_lands_canary_simplest() {
    let mut e = engine(31_040_001);
    let land = play_sphere(&mut e, &CASES[0]);
    assert!(e.state.objects[&land].tapped);
    let probe = inject_permanent_on_battlefield(&mut e, 0, CASES[0].id);
    e.apply_scenario_command(0, &activate_ability_for(&e, probe, 0, vec![]))
        .expect("tap for black");
    assert_eq!(mana_pool(&e), expected(&CASES[0]));
}

#[test]
fn q31_sphere_lands_canary_hardest() {
    let mut e = engine(31_040_002);
    let land = play_sphere(&mut e, &CASES[4]);
    assert!(e.state.objects[&land].tapped);
    let probe = inject_permanent_on_battlefield(&mut e, 0, CASES[4].id);
    e.apply_scenario_command(0, &activate_ability_for(&e, probe, 0, vec![]))
        .expect("tap for red");
    assert_eq!(mana_pool(&e), expected(&CASES[4]));
}

#[test]
fn q31_sphere_lands_enter_tapped_and_tap_for_the_printed_color() {
    for (index, case) in CASES.iter().enumerate() {
        let mut e = engine(31_041_000 + index as u64);
        let land = play_sphere(&mut e, case);
        assert!(e.state.objects[&land].tapped, "{}", case.id);
        // A second land is injected untapped so the mana ability is exercised independent
        // of the entry replacement.
        let probe = inject_permanent_on_battlefield(&mut e, 0, case.id);
        e.apply_scenario_command(0, &activate_ability_for(&e, probe, 0, vec![]))
            .unwrap_or_else(|error| panic!("{}: {error:?}", case.id));
        assert_eq!(mana_pool(&e), expected(case), "{}", case.id);
        assert!(e.state.objects[&probe].tapped, "{}", case.id);
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}

#[test]
fn q31_sphere_lands_sacrifice_for_a_card_with_exact_cost() {
    for (index, case) in CASES.iter().enumerate() {
        let mut e = engine(31_042_000 + index as u64);
        let land = inject_permanent_on_battlefield(&mut e, 0, case.id);
        give_mana(
            &mut e,
            0,
            ManaGift {
                w: u32::from(case.color == 'w'),
                u: u32::from(case.color == 'u'),
                b: u32::from(case.color == 'b'),
                r: u32::from(case.color == 'r'),
                g: u32::from(case.color == 'g'),
                c: 1,
            },
        );
        let hand_before = e.state.players[0].hand.len();
        e.apply_scenario_command(0, &activate_ability_for(&e, land, 1, vec![]))
            .unwrap_or_else(|error| panic!("{}: {error:?}", case.id));
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.objects[&land].zone,
            Zone::Graveyard,
            "{}: sacrificed as a cost",
            case.id
        );
        assert_eq!(
            e.state.players[0].hand.len(),
            hand_before + 1,
            "{}: draws one",
            case.id
        );
        assert_eq!(mana_pool(&e), (0, 0, 0, 0, 0, 0), "{}", case.id);
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}

#[test]
fn q31_sphere_lands_credit_the_activating_player() {
    for (index, case) in CASES.iter().enumerate() {
        let mut e = engine(31_044_000 + index as u64);
        let land = inject_permanent_on_battlefield(&mut e, 1, case.id);
        e.apply_scenario_command(0, &pass())
            .expect("pass to player 1");
        e.apply_scenario_command(1, &activate_ability_for(&e, land, 0, vec![]))
            .unwrap_or_else(|error| panic!("{}: {error:?}", case.id));
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
        assert_eq!(player_one, expected(case), "{}: activating player", case.id);
        assert_eq!(
            mana_pool(&e),
            (0, 0, 0, 0, 0, 0),
            "{}: other player",
            case.id
        );
    }
}

#[test]
fn q31_sphere_lands_draw_goes_to_the_activating_player() {
    for (index, case) in CASES.iter().enumerate() {
        let mut e = engine(31_045_000 + index as u64);
        let land = inject_permanent_on_battlefield(&mut e, 1, case.id);
        give_mana(
            &mut e,
            1,
            ManaGift {
                w: u32::from(case.color == 'w'),
                u: u32::from(case.color == 'u'),
                b: u32::from(case.color == 'b'),
                r: u32::from(case.color == 'r'),
                g: u32::from(case.color == 'g'),
                c: 1,
            },
        );
        let player_one_hand = e.state.players[1].hand.len();
        let player_zero_hand = e.state.players[0].hand.len();
        e.apply_scenario_command(0, &pass())
            .expect("pass to player 1");
        e.apply_scenario_command(1, &activate_ability_for(&e, land, 1, vec![]))
            .unwrap_or_else(|error| panic!("{}: {error:?}", case.id));
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.players[1].hand.len(),
            player_one_hand + 1,
            "{}: activating player draws",
            case.id
        );
        assert_eq!(
            e.state.players[0].hand.len(),
            player_zero_hand,
            "{}: other player does not draw",
            case.id
        );
        assert!(
            e.state.players[1].graveyard.contains(&land),
            "{}: owner's graveyard",
            case.id
        );
    }
}

#[test]
fn q31_sphere_lands_reject_the_sacrifice_ability_while_tapped() {
    for (index, case) in CASES.iter().enumerate() {
        let mut e = engine(31_046_000 + index as u64);
        let land = inject_permanent_on_battlefield(&mut e, 0, case.id);
        e.apply_scenario_command(0, &activate_ability_for(&e, land, 0, vec![]))
            .unwrap_or_else(|error| panic!("{}: mana: {error:?}", case.id));
        // Fund the colorless part so the attempted sacrifice fails on the tap cost,
        // not on an unaffordable {1}{X}.
        give_mana(
            &mut e,
            0,
            ManaGift {
                c: 1,
                ..Default::default()
            },
        );
        let pool_before = mana_pool(&e);
        let hand_before = e.state.players[0].hand.len();
        assert!(
            e.apply_scenario_command(0, &activate_ability_for(&e, land, 1, vec![]))
                .is_err(),
            "{}: a tapped land cannot pay the tap cost",
            case.id
        );
        assert!(e.state.objects[&land].tapped, "{}: still tapped", case.id);
        assert_eq!(
            e.state.objects[&land].zone,
            Zone::Battlefield,
            "{}",
            case.id
        );
        assert_eq!(e.state.players[0].hand.len(), hand_before, "{}", case.id);
        assert_eq!(mana_pool(&e), pool_before, "{}: pool unchanged", case.id);
    }
}

#[test]
fn q31_sphere_lands_reject_the_sacrifice_ability_without_mana() {
    for (index, case) in CASES.iter().enumerate() {
        let mut e = engine(31_043_000 + index as u64);
        let land = inject_permanent_on_battlefield(&mut e, 0, case.id);
        assert!(
            e.apply_scenario_command(0, &activate_ability_for(&e, land, 1, vec![]))
                .is_err(),
            "{}: the mana cost cannot be skipped",
            case.id
        );
        assert_eq!(
            e.state.objects[&land].zone,
            Zone::Battlefield,
            "{}",
            case.id
        );
    }
}
