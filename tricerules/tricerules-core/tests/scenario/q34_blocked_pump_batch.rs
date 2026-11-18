//! Actual-card scenarios for the queue-v34 "becomes blocked by a creature" pump cohort.
//!
//! Expectations are printed Oracle values. CR 509.3d creates one trigger for
//! each blocking creature; CR 611.2a/514.2 and 613.4c apply the self pump.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_proto::ruled::v1::BlockPair;

struct Case {
    id: &'static str,
    body: (u32, u32),
    pump: (u32, u32),
}

fn cases() -> [Case; 3] {
    [
        Case {
            id: "cave_tiger",
            body: (2, 2),
            pump: (1, 1),
        },
        Case {
            id: "rabid_wolverines",
            body: (4, 4),
            pump: (1, 1),
        },
        Case {
            id: "viashino_weaponsmith",
            body: (2, 2),
            pump: (2, 2),
        },
    ]
}

fn engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![
        deck_with("forest", &[card, "grizzly_bears"]),
        deck_with("forest", &["grizzly_bears", "grizzly_bears"]),
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

fn setup_block(case: &Case, seed: u64, blockers: usize) -> (GameEngine, u32, Vec<u32>) {
    let mut e = engine(seed, case.id);
    let attacker = move_ready_to_battlefield(&mut e, 0, case.id);
    let blockers: Vec<u32> = (0..blockers)
        .map(|_| move_ready_to_battlefield(&mut e, 1, "grizzly_bears"))
        .collect();
    e.apply_scenario_command(0, &declare_attackers(vec![attacker]))
        .expect("declare the attacker");
    pass_both_players(&mut e);
    e.apply_scenario_command(
        1,
        &declare_blockers(
            blockers
                .iter()
                .map(|blocker| BlockPair {
                    attacker_id: attacker,
                    blocker_id: *blocker,
                })
                .collect(),
        ),
    )
    .expect("declare the blockers");
    (e, attacker, blockers)
}

#[test]
fn q34_blocked_pump_canary_simplest() {
    let case = &cases()[0];
    let (mut e, attacker, blockers) = setup_block(case, 34_070_001, 1);
    assert_eq!(
        e.state.stack.len(),
        1,
        "one blocking creature makes one trigger"
    );
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        (e.effective_power(attacker), e.effective_toughness(attacker)),
        (
            Some(case.body.0 + case.pump.0),
            Some(case.body.1 + case.pump.1)
        ),
        "Cave Tiger gets +1/+1"
    );
    assert_eq!(
        (
            e.effective_power(blockers[0]),
            e.effective_toughness(blockers[0])
        ),
        (Some(2), Some(2)),
        "the blocker is unaffected"
    );
}

#[test]
fn q34_blocked_pump_canary_hardest() {
    let case = &cases()[2];
    let (mut e, attacker, _blockers) = setup_block(case, 34_070_002, 1);
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        (e.effective_power(attacker), e.effective_toughness(attacker)),
        (Some(4), Some(4)),
        "Viashino Weaponsmith gets +2/+2"
    );
}

#[test]
fn q34_blocked_pump_cards_apply_the_printed_pump_per_blocking_creature() {
    for (index, case) in cases().iter().enumerate() {
        let (mut e, attacker, _blockers) = setup_block(case, 34_071_000 + index as u64, 2);
        answer_trigger_order_in_engine_order(&mut e);
        assert_eq!(
            e.state.stack.len(),
            2,
            "{}: two blocking creatures make two triggers",
            case.id
        );
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            (e.effective_power(attacker), e.effective_toughness(attacker)),
            (
                Some(case.body.0 + 2 * case.pump.0),
                Some(case.body.1 + 2 * case.pump.1)
            ),
            "{}: one pump occurrence per blocker",
            case.id
        );
    }
}

#[test]
fn q34_blocked_pump_cards_do_not_pump_when_unblocked() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(34_072_000 + index as u64, case.id);
        let attacker = move_ready_to_battlefield(&mut e, 0, case.id);
        e.apply_scenario_command(0, &declare_attackers(vec![attacker]))
            .expect("declare the attacker");
        pass_both_players(&mut e);
        assert!(
            e.state.stack.is_empty(),
            "{}: no blocking creature means no trigger",
            case.id
        );
        assert_eq!(
            (e.effective_power(attacker), e.effective_toughness(attacker)),
            (Some(case.body.0), Some(case.body.1)),
            "{}: unchanged printed body",
            case.id
        );
    }
}
