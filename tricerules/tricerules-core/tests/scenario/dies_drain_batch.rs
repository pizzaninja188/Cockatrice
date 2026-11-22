//! Actual-card scenarios for the queue-v22 "dies, each opponent loses N life and
//! you gain N life" cohort.
//!
//! Expectations are printed Oracle values. CR 700.4 makes dies mean leaving the
//! battlefield for the graveyard; CR 603.6c triggers from that event; CR 119.3
//! adjusts each opponent's and the controller's life total; CR 608.2c makes the
//! source's controller follow the instruction.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;

struct Case {
    id: &'static str,
    life: u32,
}

fn cases() -> [Case; 5] {
    [
        Case {
            id: "brotherhood_patriarch",
            life: 2,
        },
        Case {
            id: "spirit_of_malevolence",
            life: 1,
        },
        Case {
            id: "symbiote_spawn",
            life: 2,
        },
        Case {
            id: "etched_familiar",
            life: 2,
        },
        Case {
            id: "nocturnal_feeder",
            life: 2,
        },
    ]
}

fn engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![
        deck_with("swamp", &["murder", "unmake"]),
        deck_with("forest", &[card]),
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

fn resolve_entire_stack(e: &mut GameEngine) {
    loop {
        answer_simultaneous_entry_order_in_engine_order(e);
        answer_trigger_order_in_engine_order(e);
        if e.state.stack.is_empty() {
            break;
        }
        pass_priority_round(e);
    }
}

fn kill_source(e: &mut GameEngine, source: u32) {
    ensure_in_hand(e, 0, "murder");
    give_mana(
        e,
        0,
        ManaGift {
            b: 2,
            c: 1,
            ..Default::default()
        },
    );
    let murder = hand_index_for_card(e, 0, "murder");
    e.apply_scenario_command(0, &cast_spell(murder, target_object(source)))
        .expect("destroy the source");
    resolve_entire_stack_two_player(e);
}

#[test]
fn dies_drain_batch_canary_cards_drain_each_opponent_and_credit_their_controller() {
    for case in cases()
        .into_iter()
        .filter(|case| matches!(case.id, "spirit_of_malevolence" | "nocturnal_feeder"))
    {
        let mut e = engine(11_000_000, case.id);
        let source = move_ready_to_battlefield(&mut e, 1, case.id);
        kill_source(&mut e, source);
        assert_eq!(
            e.state.objects[&source].zone,
            Zone::Graveyard,
            "{}",
            case.id
        );
        assert_eq!(
            e.state.players[0].life,
            20 - i32::try_from(case.life).expect("small printed life"),
            "{}: the opponent loses the printed life",
            case.id
        );
        assert_eq!(
            e.state.players[1].life,
            20 + i32::try_from(case.life).expect("small printed life"),
            "{}: only the source's controller gains life",
            case.id
        );
    }
}

#[test]
fn dies_drain_batch_cards_use_their_printed_amounts() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(11_010_000 + index as u64, case.id);
        let source = move_ready_to_battlefield(&mut e, 1, case.id);
        kill_source(&mut e, source);
        assert_eq!(
            e.state.players[0].life,
            20 - i32::try_from(case.life).expect("small printed life"),
            "{}",
            case.id
        );
        assert_eq!(
            e.state.players[1].life,
            20 + i32::try_from(case.life).expect("small printed life"),
            "{}",
            case.id
        );
    }
}

#[test]
fn dies_drain_batch_cards_hit_every_opponent_in_a_three_player_game() {
    for (index, case) in cases().into_iter().enumerate() {
        let decks = Some(vec![
            deck_with("swamp", &["murder"]),
            deck_with("forest", &[case.id]),
            deck_with("island", &[]),
        ]);
        let mut e = GameEngine::new(
            tricerules_cards::registry::global(),
            11_020_000 + index as u64,
            &[0, 1, 2],
            20,
            decks,
            true,
        )
        .expect("three-player engine");
        advance_to_main1_from_game_start(&mut e);
        let source = move_ready_to_battlefield(&mut e, 1, case.id);
        ensure_in_hand(&mut e, 0, "murder");
        give_mana(
            &mut e,
            0,
            ManaGift {
                b: 2,
                c: 1,
                ..Default::default()
            },
        );
        let murder = hand_index_for_card(&e, 0, "murder");
        e.apply_scenario_command(0, &cast_spell(murder, target_object(source)))
            .expect("destroy the source");
        resolve_entire_stack(&mut e);
        assert_eq!(
            e.state.objects[&source].zone,
            Zone::Graveyard,
            "{}",
            case.id
        );
        let amount = i32::try_from(case.life).expect("small printed life");
        assert_eq!(
            e.state.players[0].life,
            20 - amount,
            "{}: first opponent",
            case.id
        );
        assert_eq!(
            e.state.players[1].life,
            20 + amount,
            "{}: controller",
            case.id
        );
        assert_eq!(
            e.state.players[2].life,
            20 - amount,
            "{}: second opponent",
            case.id
        );
    }
}

#[test]
fn dies_drain_batch_cards_do_not_trigger_when_exiled() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(11_030_000 + index as u64, case.id);
        let source = move_ready_to_battlefield(&mut e, 1, case.id);
        ensure_in_hand(&mut e, 0, "unmake");
        give_mana(
            &mut e,
            0,
            ManaGift {
                w: 2,
                b: 1,
                ..Default::default()
            },
        );
        let unmake = hand_index_for_card(&e, 0, "unmake");
        e.apply_scenario_command(0, &cast_spell(unmake, target_object(source)))
            .expect("exile the source");
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(e.state.objects[&source].zone, Zone::Exile, "{}", case.id);
        assert_eq!(e.state.players[0].life, 20, "{}", case.id);
        assert_eq!(e.state.players[1].life, 20, "{}", case.id);
    }
}

#[test]
fn dies_drain_batch_cards_can_kill_an_opponent_at_low_life() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(11_040_000 + index as u64, case.id);
        let source = move_ready_to_battlefield(&mut e, 1, case.id);
        e.state.players[0].life = i32::try_from(case.life).expect("small printed life");
        kill_source(&mut e, source);
        assert_eq!(e.state.players[0].life, 0, "{}", case.id);
        assert!(e.state.players[0].has_lost, "{}: zero life loses", case.id);
        assert_eq!(
            e.state.players[1].life,
            20 + i32::try_from(case.life).expect("small printed life"),
            "{}",
            case.id
        );
    }
}
