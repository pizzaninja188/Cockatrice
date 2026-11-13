//! Actual-card scenarios for the reviewed attack-triggered +2/+0 family.
//!
//! Expectations are printed Oracle values. CR 508.1m/603.2c put the trigger on
//! the stack when the source is declared as an attacker; CR 611.2c and 613.4c
//! apply the pump to the source only; CR 514.2 ends it at cleanup.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::TurnStep;

struct Case {
    id: &'static str,
    power: u32,
    toughness: u32,
}

fn cases() -> [Case; 7] {
    [
        Case {
            id: "lurking_nightstalker",
            power: 1,
            toughness: 1,
        },
        Case {
            id: "wei_ambush_force",
            power: 1,
            toughness: 1,
        },
        Case {
            id: "borderland_marauder",
            power: 1,
            toughness: 2,
        },
        Case {
            id: "brazen_wolves",
            power: 2,
            toughness: 3,
        },
        Case {
            id: "vicious_kavu",
            power: 2,
            toughness: 2,
        },
        Case {
            id: "charging_bandits",
            power: 3,
            toughness: 3,
        },
        Case {
            id: "hollow_dogs",
            power: 3,
            toughness: 3,
        },
    ]
}

fn engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![
        deck_with("mountain", &[card]),
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
    advance_to_declare_attackers(&mut e);
    e
}

#[test]
fn attack_pump_batch_cards_apply_the_printed_effect() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(3_030_000 + index as u64, case.id);
        let source = move_ready_to_battlefield(&mut e, 0, case.id);
        assert_eq!(e.effective_power(source), Some(case.power), "{}", case.id);
        assert_eq!(
            e.effective_toughness(source),
            Some(case.toughness),
            "{}",
            case.id
        );
        e.apply_scenario_command(0, &declare_attackers(vec![source]))
            .expect("declare the source as an attacker");
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.effective_power(source),
            Some(case.power + 2),
            "{}: the attack trigger pumps power",
            case.id
        );
        assert_eq!(
            e.effective_toughness(source),
            Some(case.toughness),
            "{}: toughness is unchanged",
            case.id
        );
    }
}

#[test]
fn attack_pump_batch_cards_scope_the_trigger_to_the_attacking_source() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(3_031_000 + index as u64, case.id);
        let source = move_ready_to_battlefield(&mut e, 0, case.id);
        let other_attacker = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        e.apply_scenario_command(0, &declare_attackers(vec![other_attacker]))
            .expect("declare another creature as the only attacker");
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.effective_power(source),
            Some(case.power),
            "{}: the source did not attack and does not pump",
            case.id
        );
        assert_eq!(
            e.effective_toughness(source),
            Some(case.toughness),
            "{}",
            case.id
        );
        assert_eq!(
            e.effective_power(other_attacker),
            Some(2),
            "{}: the unrelated attacker is unchanged",
            case.id
        );

        let mut both = engine(3_031_500 + index as u64, case.id);
        let both_source = move_ready_to_battlefield(&mut both, 0, case.id);
        let both_other = inject_creature_on_battlefield(&mut both, 0, "grizzly_bears");
        both.apply_scenario_command(0, &declare_attackers(vec![both_source, both_other]))
            .expect("declare the source and another creature as attackers");
        resolve_entire_stack_two_player(&mut both);
        assert_eq!(
            both.effective_power(both_source),
            Some(case.power + 2),
            "{}: only the attacking source pumps",
            case.id
        );
        assert_eq!(
            both.effective_toughness(both_source),
            Some(case.toughness),
            "{}",
            case.id
        );
        assert_eq!(
            both.effective_power(both_other),
            Some(2),
            "{}: a second attacker is not pumped",
            case.id
        );
    }
}

fn advance_to_postcombat_main(engine: &mut GameEngine) {
    let starting_turn = engine.state.turn_instance;
    for _ in 0..160 {
        if engine.state.turn_instance == starting_turn
            && engine.state.active_player_id() == 0
            && engine.state.turn_step == TurnStep::Main2
            && engine.state.priority_player_id() == 0
        {
            return;
        }
        resolve_cleanup_discards_if_any(engine);
        let priority = engine.state.priority_player_id();
        engine
            .apply_scenario_command(priority, &pass())
            .expect("advance to the postcombat main");
    }
    panic!("turn advancement stalled before the postcombat main");
}

fn advance_to_next_upkeep(engine: &mut GameEngine) {
    let starting_turn = engine.state.turn_instance;
    for _ in 0..240 {
        if engine.state.turn_instance > starting_turn && engine.state.turn_step == TurnStep::Upkeep
        {
            return;
        }
        resolve_cleanup_discards_if_any(engine);
        let priority = engine.state.priority_player_id();
        engine
            .apply_scenario_command(priority, &pass())
            .expect("advance to the next turn's upkeep");
    }
    panic!("turn advancement stalled before the next upkeep");
}

#[test]
fn attack_pump_batch_cards_expire_at_end_of_turn() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(3_032_000 + index as u64, case.id);
        let source = move_ready_to_battlefield(&mut e, 0, case.id);
        e.apply_scenario_command(0, &declare_attackers(vec![source]))
            .expect("declare the source as an attacker");
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.effective_power(source),
            Some(case.power + 2),
            "{}: the pump is active",
            case.id
        );
        advance_to_postcombat_main(&mut e);
        assert_eq!(
            e.effective_power(source),
            Some(case.power + 2),
            "{}: the pump survives end of combat",
            case.id
        );
        assert_eq!(
            e.effective_toughness(source),
            Some(case.toughness),
            "{}",
            case.id
        );
        advance_to_next_upkeep(&mut e);
        assert_eq!(
            e.effective_power(source),
            Some(case.power),
            "{}: cleanup restores power before the next turn's upkeep",
            case.id
        );
        assert_eq!(
            e.effective_toughness(source),
            Some(case.toughness),
            "{}: cleanup restores toughness",
            case.id
        );
    }
}
