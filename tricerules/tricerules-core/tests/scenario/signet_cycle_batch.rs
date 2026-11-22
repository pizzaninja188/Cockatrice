//! Actual-card scenarios for the reviewed Signet artifact cycle.
//!
//! Expectations are printed Oracle values. CR 605.1a/605.3a-b classify the
//! untargeted mana ability as a mana ability that resolves immediately without
//! the stack; CR 118.3 requires every activation cost to be payable.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;

struct Case {
    id: &'static str,
    first: [u32; 6],
    second: [u32; 6],
}

fn cases() -> [Case; 10] {
    [
        Case {
            id: "azorius_signet",
            first: [1, 0, 0, 0, 0, 0],
            second: [0, 1, 0, 0, 0, 0],
        },
        Case {
            id: "boros_signet",
            first: [0, 0, 0, 1, 0, 0],
            second: [1, 0, 0, 0, 0, 0],
        },
        Case {
            id: "dimir_signet",
            first: [0, 1, 0, 0, 0, 0],
            second: [0, 0, 1, 0, 0, 0],
        },
        Case {
            id: "golgari_signet",
            first: [0, 0, 1, 0, 0, 0],
            second: [0, 0, 0, 0, 1, 0],
        },
        Case {
            id: "gruul_signet",
            first: [0, 0, 0, 1, 0, 0],
            second: [0, 0, 0, 0, 1, 0],
        },
        Case {
            id: "izzet_signet",
            first: [0, 1, 0, 0, 0, 0],
            second: [0, 0, 0, 1, 0, 0],
        },
        Case {
            id: "orzhov_signet",
            first: [1, 0, 0, 0, 0, 0],
            second: [0, 0, 1, 0, 0, 0],
        },
        Case {
            id: "rakdos_signet",
            first: [0, 0, 1, 0, 0, 0],
            second: [0, 0, 0, 1, 0, 0],
        },
        Case {
            id: "selesnya_signet",
            first: [0, 0, 0, 0, 1, 0],
            second: [1, 0, 0, 0, 0, 0],
        },
        Case {
            id: "simic_signet",
            first: [0, 0, 0, 0, 1, 0],
            second: [0, 1, 0, 0, 0, 0],
        },
    ]
}

fn engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![deck_with("island", &[card]), deck_with("forest", &[])]);
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

fn cast_signet(e: &mut GameEngine, card: &str) -> u32 {
    let slot = inject_card_into_hand(e, 0, card);
    give_mana(
        e,
        0,
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );
    let hand_slot = e.state.players[0]
        .hand
        .iter()
        .position(|oid| *oid == slot)
        .expect("the injected card is in hand");
    e.apply_scenario_command(0, &cast_spell(hand_slot, vec![]))
        .unwrap_or_else(|error| panic!("cast {card}: {error}"));
    resolve_entire_stack_two_player(e);
    assert_eq!(
        e.state.objects[&slot].zone,
        Zone::Battlefield,
        "{card}: resolves"
    );
    assert_eq!(pool(e), [0, 0, 0, 0, 0, 0], "{card}: cast cost consumed");
    slot
}

#[test]
fn signet_cycle_batch_cards_add_each_printed_color_pair() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(940_000 + index as u64, case.id);
        let source = cast_signet(&mut e, case.id);
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
fn signet_cycle_batch_cards_require_the_generic_mana_and_tap_cost() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(941_000 + index as u64, case.id);
        let source = cast_signet(&mut e, case.id);
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
fn signet_cycle_batch_cards_do_not_add_mana_when_activation_is_rejected() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(942_000 + index as u64, case.id);
        let source = cast_signet(&mut e, case.id);
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
