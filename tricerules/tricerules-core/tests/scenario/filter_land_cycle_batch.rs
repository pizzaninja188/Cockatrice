//! Actual-card scenarios for the reviewed `{1}, {T}: Add {A}{B}` land cycle.
//!
//! Expectations are printed Oracle values. CR 605.1a/605.3a-b classify the
//! untargeted mana ability as a mana ability that resolves immediately without
//! the stack; CR 118.3 requires every activation cost to be payable.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;

struct Case {
    id: &'static str,
    first: [u32; 6],
    second: [u32; 6],
}

fn cases() -> [Case; 9] {
    [
        Case {
            id: "darkwater_catacombs",
            first: [0, 1, 0, 0, 0, 0],
            second: [0, 0, 1, 0, 0, 0],
        },
        Case {
            id: "desolate_mire",
            first: [1, 0, 0, 0, 0, 0],
            second: [0, 0, 1, 0, 0, 0],
        },
        Case {
            id: "ferrous_lake",
            first: [0, 1, 0, 0, 0, 0],
            second: [0, 0, 0, 1, 0, 0],
        },
        Case {
            id: "overflowing_basin",
            first: [0, 0, 0, 0, 1, 0],
            second: [0, 1, 0, 0, 0, 0],
        },
        Case {
            id: "shadowblood_ridge",
            first: [0, 0, 1, 0, 0, 0],
            second: [0, 0, 0, 1, 0, 0],
        },
        Case {
            id: "skycloud_expanse",
            first: [1, 0, 0, 0, 0, 0],
            second: [0, 1, 0, 0, 0, 0],
        },
        Case {
            id: "sungrass_prairie",
            first: [0, 0, 0, 0, 1, 0],
            second: [1, 0, 0, 0, 0, 0],
        },
        Case {
            id: "sunscorched_divide",
            first: [0, 0, 0, 1, 0, 0],
            second: [1, 0, 0, 0, 0, 0],
        },
        Case {
            id: "viridescent_bog",
            first: [0, 0, 1, 0, 0, 0],
            second: [0, 0, 0, 0, 1, 0],
        },
    ]
}

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("island", &[]), deck_with("forest", &[])]);
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

fn pool(e: &GameEngine) -> [u32; 6] {
    let pool = &e.state.players[0].mana_pool;
    [
        pool.white,
        pool.blue,
        pool.black,
        pool.red,
        pool.green,
        pool.colorless,
    ]
}

fn play_land_card(e: &mut GameEngine, card: &str) -> u32 {
    inject_card_into_hand(e, 0, card);
    let slot = hand_index_for_card(e, 0, card);
    e.apply_scenario_command(0, &play_land(slot))
        .unwrap_or_else(|error| panic!("play {card}: {error}"));
    battlefield_object_for_card(e, 0, card)
}

#[test]
fn filter_land_cycle_batch_cards_add_each_printed_color_pair() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(950_000 + index as u64);
        let source = play_land_card(&mut e, case.id);
        give_mana(
            &mut e,
            0,
            ManaGift {
                c: 1,
                ..Default::default()
            },
        );
        apply_ability(&mut e, 0, source, 0, vec![])
            .unwrap_or_else(|error| panic!("{}: activation failed: {error}", case.id));
        assert_eq!(
            pool(&e),
            [
                case.first[0] + case.second[0],
                case.first[1] + case.second[1],
                case.first[2] + case.second[2],
                case.first[3] + case.second[3],
                case.first[4] + case.second[4],
                case.first[5] + case.second[5],
            ],
            "{}: printed color pair",
            case.id
        );
        assert!(e.state.objects[&source].tapped, "{}: tap paid", case.id);
        assert!(
            e.state.stack.is_empty(),
            "{}: mana ability does not use the stack",
            case.id
        );
    }
}

#[test]
fn filter_land_cycle_batch_cards_require_the_generic_mana_and_tap_cost() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(951_000 + index as u64);
        let source = play_land_card(&mut e, case.id);
        let before = e.state.command_index;
        assert!(
            apply_ability(&mut e, 0, source, 0, vec![]).is_err(),
            "{}: {{1}} is required",
            case.id
        );
        assert_eq!(
            e.state.command_index, before,
            "{}: rejected activation changes nothing",
            case.id
        );
        assert_eq!(
            pool(&e),
            [0, 0, 0, 0, 0, 0],
            "{}: no mana produced",
            case.id
        );
        assert!(
            !e.state.objects[&source].tapped,
            "{}: source stays untapped",
            case.id
        );
    }
}

#[test]
fn filter_land_cycle_batch_cards_do_not_add_mana_when_activation_is_rejected() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(952_000 + index as u64);
        let source = play_land_card(&mut e, case.id);
        give_mana(
            &mut e,
            0,
            ManaGift {
                c: 1,
                ..Default::default()
            },
        );
        apply_ability(&mut e, 0, source, 0, vec![]).expect("first activation");
        let after = pool(&e);
        assert!(
            apply_ability(&mut e, 0, source, 0, vec![]).is_err(),
            "{}: tapped source cannot pay Tap again",
            case.id
        );
        assert_eq!(
            pool(&e),
            after,
            "{}: rejected activation adds nothing",
            case.id
        );
    }
}
