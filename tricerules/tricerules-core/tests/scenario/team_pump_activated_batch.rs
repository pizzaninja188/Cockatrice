//! Actual-card scenarios for the reviewed "{4}{W}: creatures you control get
//! +1/+1 until end of turn" cohort.
//!
//! Expectations are printed Oracle values. CR 602.1/602.2 activate the ability
//! and require its mana cost; CR 611.2c/613.4c fix the affected set at
//! resolution; CR 514.2 ends the pump during cleanup.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::TurnStep;

struct Case {
    id: &'static str,
    power: u32,
    toughness: u32,
}

fn cases() -> [Case; 3] {
    [
        Case {
            id: "charismatic_vanguard",
            power: 3,
            toughness: 2,
        },
        Case {
            id: "cliffside_lookout",
            power: 1,
            toughness: 1,
        },
        Case {
            id: "sea_gate_banneret",
            power: 1,
            toughness: 2,
        },
    ]
}

fn engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![
        deck_with("plains", &[card, "grizzly_bears", "savannah_lions"]),
        deck_with("forest", &["grizzly_bears"]),
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

fn activate(e: &mut GameEngine, source: u32) {
    give_mana(
        e,
        0,
        ManaGift {
            w: 1,
            c: 4,
            ..Default::default()
        },
    );
    apply_ability(e, 0, source, 0, vec![]).expect("activate the team pump");
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

#[test]
fn team_pump_activated_batch_cards_pump_only_their_controllers_creatures() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(10_900_000 + index as u64, case.id);
        let source = move_ready_to_battlefield(&mut e, 0, case.id);
        let own = move_ready_to_battlefield(&mut e, 0, "grizzly_bears");
        let opposing = move_ready_to_battlefield(&mut e, 1, "grizzly_bears");
        activate(&mut e, source);
        assert_eq!(
            (e.effective_power(source), e.effective_toughness(source)),
            (Some(case.power + 1), Some(case.toughness + 1)),
            "{}: the source is pumped",
            case.id
        );
        assert_eq!(
            (e.effective_power(own), e.effective_toughness(own)),
            (Some(3), Some(3)),
            "{}: each other controlled creature is pumped",
            case.id
        );
        assert_eq!(
            (e.effective_power(opposing), e.effective_toughness(opposing)),
            (Some(2), Some(2)),
            "{}: the opponent's creature is unchanged",
            case.id
        );

        let later = move_ready_to_battlefield(&mut e, 0, "savannah_lions");
        assert_eq!(
            (e.effective_power(later), e.effective_toughness(later)),
            (Some(2), Some(1)),
            "{}: a later entrant is not pumped",
            case.id
        );
    }
}

#[test]
fn team_pump_activated_batch_cards_require_the_printed_mana_cost() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(10_910_000 + index as u64, case.id);
        let source = move_ready_to_battlefield(&mut e, 0, case.id);
        let own = move_ready_to_battlefield(&mut e, 0, "grizzly_bears");
        give_mana(
            &mut e,
            0,
            ManaGift {
                w: 1,
                c: 3,
                ..Default::default()
            },
        );
        assert!(
            apply_ability(&mut e, 0, source, 0, vec![]).is_err(),
            "{}: four generic mana plus {{W}} is required",
            case.id
        );
        assert_eq!(
            (e.effective_power(source), e.effective_toughness(source)),
            (Some(case.power), Some(case.toughness)),
            "{}",
            case.id
        );
        assert_eq!(
            (e.effective_power(own), e.effective_toughness(own)),
            (Some(2), Some(2)),
            "{}",
            case.id
        );
        assert_eq!(
            (
                e.state.players[0].mana_pool.white,
                e.state.players[0].mana_pool.colorless
            ),
            (1, 3),
            "{}: a rejected activation spends nothing",
            case.id
        );
        give_mana(
            &mut e,
            0,
            ManaGift {
                c: 1,
                ..Default::default()
            },
        );
        apply_ability(&mut e, 0, source, 0, vec![]).expect("the exact printed cost is payable");
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            (e.effective_power(source), e.effective_toughness(source)),
            (Some(case.power + 1), Some(case.toughness + 1)),
            "{}",
            case.id
        );
        assert_eq!(
            (
                e.state.players[0].mana_pool.white,
                e.state.players[0].mana_pool.colorless
            ),
            (0, 0),
            "{}: the exact cost was consumed",
            case.id
        );
    }
}

#[test]
fn team_pump_activated_batch_cards_expire_at_end_of_turn() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(10_920_000 + index as u64, case.id);
        let source = move_ready_to_battlefield(&mut e, 0, case.id);
        activate(&mut e, source);
        assert_eq!(
            (e.effective_power(source), e.effective_toughness(source)),
            (Some(case.power + 1), Some(case.toughness + 1)),
            "{}: the pump is active",
            case.id
        );
        advance_to_next_upkeep(&mut e);
        assert_eq!(
            (e.effective_power(source), e.effective_toughness(source)),
            (Some(case.power), Some(case.toughness)),
            "{}: the pump expires during cleanup",
            case.id
        );
    }
}
