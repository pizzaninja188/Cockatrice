//! Actual-card scenarios for the queue-v31 battle cry cohort.
//!
//! Expectations are printed Oracle values. CR 508.1m/603.2c put the trigger on the
//! stack when the source is declared as an attacker; CR 611.2a/613.4c apply the
//! pump to each other attacking creature; CR 514.2 ends it at cleanup.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::TurnStep;

fn put_card(e: &mut GameEngine, player: i32, card_name: &str, card_id: &str) -> u32 {
    use tricerules_proto::ruled::v1::dev_command::Dev;
    use tricerules_proto::ruled::v1::{DevCommand, DevPutCardInZone, DevZone, RuledCommand};
    e.enable_dev_commands();
    e.apply_scenario_command(
        player,
        &RuledCommand {
            cmd: Some(tricerules_proto::ruled::v1::ruled_command::Cmd::DevCommand(
                DevCommand {
                    target_player_id: player,
                    dev: Some(Dev::PutCardInZone(DevPutCardInZone {
                        card_name: card_name.to_string(),
                        zone: DevZone::Battlefield as i32,
                        ready: true,
                    })),
                },
            )),
        },
    )
    .unwrap_or_else(|error| panic!("put {card_name}: {error:?}"));
    battlefield_object_for_card(e, player as usize, card_id)
}

struct Case {
    id: &'static str,
    name: &'static str,
    body: (u32, u32),
}

const CASES: [Case; 3] = [
    Case {
        id: "accorder_paladin",
        name: "Accorder Paladin",
        body: (3, 1),
    },
    Case {
        id: "goblin_wardriver",
        name: "Goblin Wardriver",
        body: (2, 2),
    },
    Case {
        id: "loxodon_partisan",
        name: "Loxodon Partisan",
        body: (3, 4),
    },
];

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("mountain", &[]), deck_with("forest", &[])]);
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

fn advance_to_postcombat_main(e: &mut GameEngine) {
    let starting_turn = e.state.turn_instance;
    for _ in 0..160 {
        if e.state.turn_instance == starting_turn
            && e.state.active_player_id() == 0
            && e.state.turn_step == TurnStep::Main2
            && e.state.priority_player_id() == 0
        {
            return;
        }
        resolve_cleanup_discards_if_any(e);
        let priority = e.state.priority_player_id();
        e.apply_scenario_command(priority, &pass())
            .expect("advance to the postcombat main");
    }
    panic!("turn advancement stalled");
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
    panic!("turn advancement stalled");
}

#[test]
fn q31_battle_cry_canary_simplest() {
    let case = &CASES[1];
    let mut e = engine(31_070_001);
    let source = put_card(&mut e, 0, case.name, case.id);
    let other = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    e.apply_scenario_command(0, &declare_attackers(vec![source, other]))
        .expect("declare attackers");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.effective_power(source), Some(case.body.0));
    assert_eq!(e.effective_power(other), Some(3));
}

#[test]
fn q31_battle_cry_canary_hardest() {
    let case = &CASES[2];
    let mut e = engine(31_070_002);
    let source = put_card(&mut e, 0, case.name, case.id);
    let other = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    e.apply_scenario_command(0, &declare_attackers(vec![source, other]))
        .expect("declare attackers");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.effective_power(source), Some(case.body.0));
    assert_eq!(e.effective_power(other), Some(3));
}

#[test]
fn q31_battle_cry_cards_pump_each_other_attacking_creature() {
    for (index, case) in CASES.iter().enumerate() {
        let mut e = engine(31_071_000 + index as u64);
        let source = put_card(&mut e, 0, case.name, case.id);
        let other = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        let bystander = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        e.apply_scenario_command(0, &declare_attackers(vec![source, other]))
            .expect("declare attackers");
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(e.effective_power(source), Some(case.body.0), "{}", case.id);
        assert_eq!(
            e.effective_toughness(source),
            Some(case.body.1),
            "{}",
            case.id
        );
        assert_eq!(e.effective_power(other), Some(3), "{}", case.id);
        assert_eq!(e.effective_power(bystander), Some(2), "{}", case.id);
    }
}

#[test]
fn q31_battle_cry_cards_stack_from_multiple_sources() {
    for index in 0..CASES.len() {
        let first_case = &CASES[index];
        let second_case = &CASES[(index + 1) % CASES.len()];
        let mut e = engine(31_072_000 + index as u64);
        let first = put_card(&mut e, 0, first_case.name, first_case.id);
        let second = put_card(&mut e, 0, second_case.name, second_case.id);
        let other = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        e.apply_scenario_command(0, &declare_attackers(vec![first, second, other]))
            .expect("declare attackers");
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.effective_power(first),
            Some(first_case.body.0 + 1),
            "{}",
            first_case.id
        );
        assert_eq!(
            e.effective_power(second),
            Some(second_case.body.0 + 1),
            "{}",
            second_case.id
        );
        assert_eq!(
            e.effective_power(other),
            Some(4),
            "{} and {}",
            first_case.id,
            second_case.id
        );
    }
}

#[test]
fn q31_battle_cry_does_not_trigger_when_the_source_does_not_attack() {
    for (index, case) in CASES.iter().enumerate() {
        let mut e = engine(31_073_000 + index as u64);
        let source = put_card(&mut e, 0, case.name, case.id);
        let other = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        e.apply_scenario_command(0, &declare_attackers(vec![other]))
            .expect("declare the other attacker");
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(e.effective_power(source), Some(case.body.0), "{}", case.id);
        assert_eq!(e.effective_power(other), Some(2), "{}", case.id);
    }
}

#[test]
fn q31_battle_cry_pumps_nothing_when_attacking_alone() {
    for (index, case) in CASES.iter().enumerate() {
        let mut e = engine(31_074_000 + index as u64);
        let source = put_card(&mut e, 0, case.name, case.id);
        e.apply_scenario_command(0, &declare_attackers(vec![source]))
            .expect("declare the lone attacker");
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(e.effective_power(source), Some(case.body.0), "{}", case.id);
    }
}

#[test]
fn q31_battle_cry_pump_expires_at_cleanup() {
    for (index, case) in CASES.iter().enumerate() {
        let mut e = engine(31_075_000 + index as u64);
        let source = put_card(&mut e, 0, case.name, case.id);
        let other = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        e.apply_scenario_command(0, &declare_attackers(vec![source, other]))
            .expect("declare attackers");
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(e.effective_power(other), Some(3), "{}", case.id);
        advance_to_postcombat_main(&mut e);
        assert_eq!(e.effective_power(other), Some(3), "{}", case.id);
        advance_to_next_upkeep(&mut e);
        assert_eq!(e.effective_power(other), Some(2), "{}", case.id);
    }
}
