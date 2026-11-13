//! Actual-card scenarios for the reviewed attack-trigger self-pump family.
//!
//! Expectations are printed Oracle values. CR 508.1m/603.2c put the trigger on the stack when the source is declared as an attacker; CR 611.2c/613.4c apply the pump only to the source until end of turn; CR 514.2 ends it during cleanup.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::TurnStep;

struct Case {
    id: &'static str,
    power: u32,
    toughness: u32,
    pump_power: i32,
    pump_toughness: i32,
}

fn cases() -> [Case; 7] {
    [
        Case {
            id: "benalish_veteran",
            power: 2,
            toughness: 2,
            pump_power: 1,
            pump_toughness: 1,
        },
        Case {
            id: "bramble_creeper",
            power: 0,
            toughness: 3,
            pump_power: 5,
            pump_toughness: 0,
        },
        Case {
            id: "charging_paladin",
            power: 2,
            toughness: 2,
            pump_power: 0,
            pump_toughness: 3,
        },
        Case {
            id: "graceful_cat",
            power: 2,
            toughness: 2,
            pump_power: 1,
            pump_toughness: 1,
        },
        Case {
            id: "kiln_walker",
            power: 0,
            toughness: 3,
            pump_power: 3,
            pump_toughness: 0,
        },
        Case {
            id: "reckless_pangolin",
            power: 2,
            toughness: 2,
            pump_power: 1,
            pump_toughness: 1,
        },
        Case {
            id: "steadfast_cathar",
            power: 2,
            toughness: 1,
            pump_power: 0,
            pump_toughness: 2,
        },
    ]
}

fn engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![
        deck_with("plains", &[card, "savannah_lions", "grizzly_bears"]),
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
    advance_to_declare_attackers(&mut e);
    e
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
            .expect("advance to the next upkeep");
    }
    panic!("turn advancement stalled before the next upkeep");
}

#[test]
fn attack_pump_values_batch_cards_apply_the_printed_pump_when_they_attack() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(11_800_000 + index as u64, case.id);
        let source = move_ready_to_battlefield(&mut e, 0, case.id);
        e.apply_scenario_command(0, &declare_attackers(vec![source]))
            .expect("declare the source as an attacker");
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            (e.effective_power(source), e.effective_toughness(source)),
            (
                Some(case.power + case.pump_power as u32),
                Some(case.toughness + case.pump_toughness as u32)
            ),
            "{}: exact printed pump",
            case.id
        );
        assert_eq!(e.state.players[0].life, 20, "{}: no life change", case.id);
    }
}

#[test]
fn attack_pump_values_batch_cards_pump_only_the_attacking_source() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(11_810_000 + index as u64, case.id);
        let source = move_ready_to_battlefield(&mut e, 0, case.id);
        let other_attacker = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        e.apply_scenario_command(0, &declare_attackers(vec![other_attacker]))
            .expect("declare another creature as the only attacker");
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.effective_power(source),
            Some(case.power),
            "{}: source did not attack",
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
            "{}: unrelated attacker unchanged",
            case.id
        );

        let mut both = engine(11_815_000 + index as u64, case.id);
        let both_source = move_ready_to_battlefield(&mut both, 0, case.id);
        let both_other = inject_creature_on_battlefield(&mut both, 0, "grizzly_bears");
        both.apply_scenario_command(0, &declare_attackers(vec![both_source, both_other]))
            .expect("declare both attackers");
        resolve_entire_stack_two_player(&mut both);
        assert_eq!(
            both.effective_power(both_source),
            Some(case.power + case.pump_power as u32),
            "{}: only the attacking source pumps",
            case.id
        );
        assert_eq!(
            both.effective_toughness(both_source),
            Some(case.toughness + case.pump_toughness as u32),
            "{}",
            case.id
        );
        assert_eq!(
            both.effective_power(both_other),
            Some(2),
            "{}: second attacker unchanged",
            case.id
        );
    }
}

#[test]
fn attack_pump_values_batch_cards_expire_at_end_of_turn() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(11_820_000 + index as u64, case.id);
        let source = move_ready_to_battlefield(&mut e, 0, case.id);
        e.apply_scenario_command(0, &declare_attackers(vec![source]))
            .expect("declare the source as an attacker");
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.effective_power(source),
            Some(case.power + case.pump_power as u32),
            "{}",
            case.id
        );
        advance_to_postcombat_main(&mut e);
        assert_eq!(
            e.effective_power(source),
            Some(case.power + case.pump_power as u32),
            "{}: survives end of combat",
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
