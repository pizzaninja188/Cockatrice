//! Actual-card scenarios for the reviewed once-each-turn self-pump family.
//!
//! Expectations are printed Oracle values. CR 602.1/602.2 define the activated ability and its cost; CR 611.2c/613.4c apply the pump until end of turn; the printed once-each-turn limit is the typed activation limit; CR 514.2 ends the pump during cleanup.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::{TurnStep, Zone};

struct Case {
    id: &'static str,
    cast: ManaGift,
    activation: ManaGift,
    power: u32,
    toughness: u32,
    pump_power: i32,
    pump_toughness: i32,
}

fn cases() -> [Case; 7] {
    [
        Case {
            id: "darkthicket_wolf",
            cast: ManaGift {
                g: 1,
                c: 1,
                ..Default::default()
            },
            activation: ManaGift {
                g: 1,
                c: 2,
                ..Default::default()
            },
            power: 2,
            toughness: 2,
            pump_power: 2,
            pump_toughness: 2,
        },
        Case {
            id: "frilled_oculus",
            cast: ManaGift {
                u: 1,
                c: 1,
                ..Default::default()
            },
            activation: ManaGift {
                g: 1,
                c: 1,
                ..Default::default()
            },
            power: 1,
            toughness: 3,
            pump_power: 2,
            pump_toughness: 2,
        },
        Case {
            id: "frilled_sandwalla",
            cast: ManaGift {
                g: 1,
                ..Default::default()
            },
            activation: ManaGift {
                g: 1,
                c: 1,
                ..Default::default()
            },
            power: 1,
            toughness: 1,
            pump_power: 2,
            pump_toughness: 2,
        },
        Case {
            id: "knight_of_the_skyward_eye",
            cast: ManaGift {
                w: 1,
                c: 1,
                ..Default::default()
            },
            activation: ManaGift {
                g: 1,
                c: 3,
                ..Default::default()
            },
            power: 2,
            toughness: 2,
            pump_power: 3,
            pump_toughness: 3,
        },
        Case {
            id: "plated_rootwalla",
            cast: ManaGift {
                g: 1,
                c: 4,
                ..Default::default()
            },
            activation: ManaGift {
                g: 1,
                c: 2,
                ..Default::default()
            },
            power: 3,
            toughness: 3,
            pump_power: 3,
            pump_toughness: 3,
        },
        Case {
            id: "rootwalla",
            cast: ManaGift {
                g: 1,
                c: 2,
                ..Default::default()
            },
            activation: ManaGift {
                g: 1,
                c: 1,
                ..Default::default()
            },
            power: 2,
            toughness: 2,
            pump_power: 2,
            pump_toughness: 2,
        },
        Case {
            id: "snarling_wolf",
            cast: ManaGift {
                g: 1,
                ..Default::default()
            },
            activation: ManaGift {
                g: 1,
                c: 1,
                ..Default::default()
            },
            power: 1,
            toughness: 1,
            pump_power: 2,
            pump_toughness: 2,
        },
    ]
}

fn engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![
        deck_with("forest", &[card, "grizzly_bears"]),
        deck_with("island", &[]),
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

fn pump(e: &mut GameEngine, source: u32, case: &Case) {
    give_mana(
        e,
        0,
        ManaGift {
            w: case.activation.w,
            u: case.activation.u,
            b: case.activation.b,
            r: case.activation.r,
            g: case.activation.g,
            c: case.activation.c,
        },
    );
    e.apply_scenario_command(0, &activate_ability_for(e, source, 0, vec![]))
        .unwrap_or_else(|error| panic!("{}: activate: {error}", case.id));
    resolve_entire_stack_two_player(e);
}

fn advance_to_next_upkeep(e: &mut GameEngine) {
    let starting_turn = e.state.turn_instance;
    for _ in 0..240 {
        if e.state.turn_instance > starting_turn && e.state.turn_step == TurnStep::Upkeep {
            return;
        }
        resolve_cleanup_discards_if_any(e);
        let priority = e.state.priority_player_id();
        e.apply_scenario_command(priority, &pass())
            .expect("advance to the next upkeep");
    }
    panic!("turn advancement stalled before the next upkeep");
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

#[test]
fn self_pump_once_batch_cards_cast_with_exact_printed_characteristics() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(11_890_000 + index as u64, case.id);
        give_mana(
            &mut e,
            0,
            ManaGift {
                w: case.cast.w,
                u: case.cast.u,
                b: case.cast.b,
                r: case.cast.r,
                g: case.cast.g,
                c: case.cast.c,
            },
        );
        let slot = inject_card_into_hand(&mut e, 0, case.id);
        let hand_slot = e.state.players[0]
            .hand
            .iter()
            .position(|oid| *oid == slot)
            .expect("the injected card is in hand");
        e.apply_scenario_command(0, &cast_spell(hand_slot, vec![]))
            .unwrap_or_else(|error| panic!("{}: cast: {error}", case.id));
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
    }
}

#[test]
fn self_pump_once_batch_cards_apply_the_printed_pump_once_each_turn() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(11_900_000 + index as u64, case.id);
        let source = move_ready_to_battlefield(&mut e, 0, case.id);
        pump(&mut e, source, &case);
        assert_eq!(
            (e.effective_power(source), e.effective_toughness(source)),
            (
                Some(case.power + case.pump_power as u32),
                Some(case.toughness + case.pump_toughness as u32)
            ),
            "{}: first activation pumps",
            case.id
        );
        give_mana(
            &mut e,
            0,
            ManaGift {
                w: case.activation.w,
                u: case.activation.u,
                b: case.activation.b,
                r: case.activation.r,
                g: case.activation.g,
                c: case.activation.c,
            },
        );
        assert!(
            e.apply_scenario_command(0, &activate_ability_for(&e, source, 0, vec![]))
                .is_err(),
            "{}: a second activation in the same turn is rejected",
            case.id
        );
        assert_eq!(
            (e.effective_power(source), e.effective_toughness(source)),
            (
                Some(case.power + case.pump_power as u32),
                Some(case.toughness + case.pump_toughness as u32)
            ),
            "{}: the rejected activation does not double the pump",
            case.id
        );
        advance_to_next_own_main1(&mut e);
        assert_eq!(
            (e.effective_power(source), e.effective_toughness(source)),
            (Some(case.power), Some(case.toughness)),
            "{}: the first pump expired at end of turn",
            case.id
        );
        pump(&mut e, source, &case);
        assert_eq!(
            (e.effective_power(source), e.effective_toughness(source)),
            (
                Some(case.power + case.pump_power as u32),
                Some(case.toughness + case.pump_toughness as u32)
            ),
            "{}: the next turn allows another activation",
            case.id
        );
    }
}

#[test]
fn self_pump_once_batch_cards_require_the_printed_activation_cost() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(11_910_000 + index as u64, case.id);
        let source = move_ready_to_battlefield(&mut e, 0, case.id);
        assert!(
            e.apply_scenario_command(0, &activate_ability_for(&e, source, 0, vec![]))
                .is_err(),
            "{}: an unpaid activation is rejected",
            case.id
        );
        assert_eq!(
            (e.effective_power(source), e.effective_toughness(source)),
            (Some(case.power), Some(case.toughness)),
            "{}: rejected activation leaves the printed body",
            case.id
        );
    }
}

#[test]
fn self_pump_once_batch_cards_expire_at_end_of_turn() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(11_920_000 + index as u64, case.id);
        let source = move_ready_to_battlefield(&mut e, 0, case.id);
        pump(&mut e, source, &case);
        assert_eq!(
            e.effective_power(source),
            Some(case.power + case.pump_power as u32),
            "{}",
            case.id
        );
        advance_to_next_upkeep(&mut e);
        assert_eq!(
            e.effective_power(source),
            Some(case.power),
            "{}: power expires",
            case.id
        );
        assert_eq!(
            e.effective_toughness(source),
            Some(case.toughness),
            "{}: toughness expires",
            case.id
        );
    }
}
