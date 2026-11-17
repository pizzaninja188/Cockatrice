//! Actual-card scenarios for the queue-v26 flying once-each-turn self-pump cohort.
//!
//! Expectations are printed Oracle values. CR 702.9a-b grants flying; CR 602.1/602.2
//! governs the activated ability; CR 611.2c/613.4c apply the source-only pump; the
//! printed once-each-turn limit is the typed activation limit; CR 514.2 ends the
//! pump during cleanup.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::{TurnStep, Zone};

struct Case {
    id: &'static str,
    cast: [u32; 6],
    activation: [u32; 6],
    power: u32,
    toughness: u32,
    pump_power: u32,
    pump_toughness: u32,
}

fn cases() -> [Case; 4] {
    [
        Case {
            id: "azimaet_drake",
            cast: [0, 1, 0, 0, 0, 2],
            activation: [0, 1, 0, 0, 0, 0],
            power: 1,
            toughness: 3,
            pump_power: 1,
            pump_toughness: 0,
        },
        Case {
            id: "drake_hatchling",
            cast: [0, 1, 0, 0, 0, 2],
            activation: [0, 1, 0, 0, 0, 0],
            power: 1,
            toughness: 3,
            pump_power: 1,
            pump_toughness: 0,
        },
        Case {
            id: "spitting_drake",
            cast: [0, 0, 0, 1, 0, 3],
            activation: [0, 0, 0, 1, 0, 0],
            power: 2,
            toughness: 2,
            pump_power: 1,
            pump_toughness: 0,
        },
        Case {
            id: "fire_drake",
            cast: [0, 0, 0, 2, 0, 1],
            activation: [0, 0, 0, 1, 0, 0],
            power: 1,
            toughness: 2,
            pump_power: 1,
            pump_toughness: 0,
        },
    ]
}

fn gift(mask: [u32; 6]) -> ManaGift {
    ManaGift {
        w: mask[0],
        u: mask[1],
        b: mask[2],
        r: mask[3],
        g: mask[4],
        c: mask[5],
    }
}

fn engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![
        deck_with("island", &[card, "grizzly_bears"]),
        deck_with("forest", &[]),
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

fn advance_to_next_own_main1(e: &mut GameEngine) {
    let starting_turn = e.state.turn_instance;
    for _ in 0..320 {
        if e.state.turn_instance > starting_turn
            && e.state.active_player_id() == 0
            && e.state.turn_step == TurnStep::Main1
            && e.state.priority_player_id() == 0
        {
            return;
        }
        resolve_cleanup_discards_if_any(e);
        let priority = e.state.priority_player_id();
        e.apply_scenario_command(priority, &pass())
            .expect("advance to the next own main phase");
    }
    panic!("turn advancement stalled before the next own main phase");
}

fn activate(e: &mut GameEngine, case: &Case, source: u32) {
    give_mana(e, 0, gift(case.activation));
    e.apply_scenario_command(0, &activate_ability_for(e, source, 0, vec![]))
        .unwrap_or_else(|error| panic!("{}: activate: {error}", case.id));
    resolve_entire_stack_two_player(e);
}

#[test]
fn flying_once_pump_batch_cards_cast_with_exact_characteristics_and_flying() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(26_060_000 + index as u64, case.id);
        give_mana(&mut e, 0, gift(case.cast));
        let slot = inject_card_into_hand(&mut e, 0, case.id);
        let hand_slot = e.state.players[0]
            .hand
            .iter()
            .position(|oid| *oid == slot)
            .expect("the injected card is in hand");
        e.apply_scenario_command(0, &cast_spell(hand_slot, vec![]))
            .unwrap_or_else(|error| panic!("{}: cast: {error}", case.id));
        resolve_entire_stack_two_player(&mut e);

        assert_eq!(
            e.state.objects[&slot].zone,
            Zone::Battlefield,
            "{}",
            case.id
        );
        assert_eq!(e.effective_power(slot), Some(case.power), "{}", case.id);
        assert_eq!(
            e.effective_toughness(slot),
            Some(case.toughness),
            "{}",
            case.id
        );
        assert!(
            e.effective_has_keyword(slot, tricerules_cards::Keyword::Flying),
            "{}: printed flying",
            case.id
        );
        let pool = &e.state.players[0].mana_pool;
        assert_eq!(
            (
                pool.white,
                pool.blue,
                pool.black,
                pool.red,
                pool.green,
                pool.colorless
            ),
            (0, 0, 0, 0, 0, 0),
            "{}: exact printed cost consumed",
            case.id
        );
    }
}

#[test]
fn flying_once_pump_batch_cards_activate_once_each_turn_and_reset_on_the_next_turn() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(26_061_000 + index as u64, case.id);
        let source = move_ready_to_battlefield(&mut e, 0, case.id);
        let other = move_ready_to_battlefield(&mut e, 0, "grizzly_bears");

        activate(&mut e, case, source);
        assert_eq!(
            e.effective_power(source),
            Some(case.power + case.pump_power),
            "{}: first activation pumps",
            case.id
        );
        let before = e.state.command_index;
        give_mana(&mut e, 0, gift(case.activation));
        e.apply_scenario_command(0, &activate_ability_for(&e, source, 0, vec![]))
            .expect_err("the printed limit allows only one activation each turn");
        assert_eq!(e.state.command_index, before, "{}", case.id);

        advance_to_next_own_main1(&mut e);
        assert_eq!(
            e.effective_power(source),
            Some(case.power),
            "{}: the pump ended during cleanup",
            case.id
        );
        activate(&mut e, case, source);
        assert_eq!(
            e.effective_power(source),
            Some(case.power + case.pump_power),
            "{}: the next turn has a fresh activation",
            case.id
        );
        assert_eq!(
            e.effective_toughness(source),
            Some(case.toughness + case.pump_toughness),
            "{}",
            case.id
        );
        assert_eq!(
            e.effective_power(other),
            Some(2),
            "{}: only the source pumps",
            case.id
        );
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}

#[test]
fn flying_once_pump_batch_cards_require_the_printed_activation_cost() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(26_062_000 + index as u64, case.id);
        let source = move_ready_to_battlefield(&mut e, 0, case.id);
        let before = e.state.command_index;
        e.apply_scenario_command(0, &activate_ability_for(&e, source, 0, vec![]))
            .expect_err("the printed mana cost must be paid");
        assert_eq!(e.state.command_index, before, "{}", case.id);
        assert_eq!(e.effective_power(source), Some(case.power), "{}", case.id);
        assert_eq!(
            e.effective_toughness(source),
            Some(case.toughness),
            "{}",
            case.id
        );
        assert_eq!(
            e.state.objects[&source].zone,
            Zone::Battlefield,
            "{}",
            case.id
        );
        assert!(e.state.stack.is_empty(), "{}", case.id);
        let pool = &e.state.players[0].mana_pool;
        assert_eq!(
            (
                pool.white,
                pool.blue,
                pool.black,
                pool.red,
                pool.green,
                pool.colorless
            ),
            (0, 0, 0, 0, 0, 0),
            "{}",
            case.id
        );
    }
}
