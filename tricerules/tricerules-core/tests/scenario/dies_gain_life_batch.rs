//! Actual-card scenarios for the reviewed "dies, you gain N life" cohort.
//!
//! Expectations are printed Oracle values. CR 700.4 makes dies mean leaving the
//! battlefield for the graveyard; CR 603.6c triggers from that event; CR
//! 119.3 adjusts the controller's life total; 608.2c has that player follow the
//! trigger's instruction.
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
            id: "anodet_lurker",
            life: 3,
        },
        Case {
            id: "enatu_golem",
            life: 4,
        },
        Case {
            id: "guardian_automaton",
            life: 3,
        },
        Case {
            id: "onulet",
            life: 2,
        },
        Case {
            id: "tarpan",
            life: 1,
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

fn kill_with_murder(e: &mut GameEngine, controller: usize, source: u32) {
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
    let _ = controller;
}

#[test]
fn dies_gain_life_batch_cards_gain_their_printed_life_when_they_die() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(10_800_000 + index as u64, case.id);
        let source = move_ready_to_battlefield(&mut e, 1, case.id);
        kill_with_murder(&mut e, 1, source);
        assert_eq!(
            e.state.objects[&source].zone,
            Zone::Graveyard,
            "{}",
            case.id
        );
        assert_eq!(
            e.state.players[1].life,
            20 + i32::try_from(case.life).expect("small printed life"),
            "{}: the source's controller gains the printed life",
            case.id
        );
        assert_eq!(
            e.state.players[0].life, 20,
            "{}: the destroying player gains nothing",
            case.id
        );
    }
}

#[test]
fn dies_gain_life_batch_cards_do_not_gain_life_when_exiled() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(10_810_000 + index as u64, case.id);
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
        assert_eq!(
            e.state.players[0].life, 20,
            "{}: leaving for exile is not dying",
            case.id
        );
        assert_eq!(e.state.players[1].life, 20, "{}", case.id);
    }
}

#[test]
fn dies_gain_life_batch_cards_scope_the_gain_to_their_controller() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(10_820_000 + index as u64, case.id);
        let source = move_ready_to_battlefield(&mut e, 1, case.id);
        kill_with_murder(&mut e, 1, source);
        assert_eq!(
            e.state.players[1].life,
            20 + i32::try_from(case.life).expect("small printed life"),
            "{}: only the source's controller gains life",
            case.id
        );
        assert_eq!(e.state.players[0].life, 20, "{}", case.id);
    }
}
