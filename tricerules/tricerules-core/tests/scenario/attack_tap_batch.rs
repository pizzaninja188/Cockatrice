//! Actual-card scenarios for the queue-v28 attack-tap cohort.
//!
//! Expectations are printed Oracle values. CR 508.3a puts the attack trigger on the stack
//! before blockers; CR 115.1 and the pinned rulings restrict the target to a creature
//! controlled by the defending player; CR 701.26 taps it.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::TurnStep;
use tricerules_proto::ruled::v1::{
    AttackAssignment, ChooseTriggerTarget, DeclareAttackers, TargetRef, TargetRefKind,
};

struct Case {
    id: &'static str,
    power: u32,
    toughness: u32,
}

fn cases() -> [Case; 3] {
    [
        Case {
            id: "fiend_binder",
            power: 3,
            toughness: 2,
        },
        Case {
            id: "master_of_diversion",
            power: 2,
            toughness: 2,
        },
        Case {
            id: "star-crowned_stag",
            power: 3,
            toughness: 3,
        },
    ]
}

fn choose_trigger_target(object_id: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
            decline: false,
            selected_modes: Vec::new(),
            targets: vec![TargetRef {
                object_id,
                group_index: 0,
                kind: TargetRefKind::Permanent as i32,
                ..Default::default()
            }],
        })),
    }
}

fn engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![
        deck_with("plains", &[card]),
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

fn advance_to_declare_attackers(e: &mut GameEngine) {
    e.apply_scenario_command(0, &primitive_yield())
        .expect("main phase to beginning of combat");
    for _ in 0..e.state.players.len() {
        if e.state.turn_step == TurnStep::DeclareAttackers {
            return;
        }
        let player = e.state.priority_player_id();
        e.apply_scenario_command(player, &pass())
            .expect("pass in beginning of combat");
    }
    assert_eq!(e.state.turn_step, TurnStep::DeclareAttackers);
}

#[test]
fn attack_tap_batch_cards_tap_a_creature_controlled_by_the_defending_player() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(28_070_000 + index as u64, case.id);
        let attacker = move_ready_to_battlefield(&mut e, 0, case.id);
        let blocker = move_ready_to_battlefield(&mut e, 1, "grizzly_bears");
        assert_eq!(
            (e.effective_power(attacker), e.effective_toughness(attacker)),
            (Some(case.power), Some(case.toughness)),
            "{}: printed body",
            case.id
        );
        advance_to_declare_attackers(&mut e);
        e.apply_scenario_command(0, &declare_attackers(vec![attacker]))
            .expect("declare attacker");
        assert_eq!(e.state.pending_triggers.len(), 1, "{}", case.id);
        e.apply_scenario_command(0, &choose_trigger_target(blocker))
            .expect("choose the defending player's creature");
        resolve_entire_stack_two_player(&mut e);
        assert!(
            e.state.objects[&blocker].tapped,
            "{}: the chosen creature is tapped",
            case.id
        );
        assert!(
            e.state.objects[&attacker].tapped,
            "{}: attacking tapped the attacker",
            case.id
        );
    }
}

#[test]
fn attack_tap_batch_cards_offer_no_target_when_the_defender_has_no_creature() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(28_071_000 + index as u64, case.id);
        let attacker = move_ready_to_battlefield(&mut e, 0, case.id);
        advance_to_declare_attackers(&mut e);
        e.apply_scenario_command(0, &declare_attackers(vec![attacker]))
            .expect("declare attacker");
        assert_eq!(
            e.state.pending_triggers.len(),
            0,
            "{}: a mandatory target with no legal choice is not put on the stack",
            case.id
        );
    }
}

#[test]
fn attack_tap_batch_cards_use_defending_player_scope_in_a_three_player_game() {
    let decks = Some(vec![
        deck_with("plains", &["fiend_binder"]),
        deck_with("forest", &["grizzly_bears"]),
        deck_with("forest", &["grizzly_bears"]),
    ]);
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        28_072_000,
        &[0, 1, 2],
        20,
        decks,
        true,
    )
    .expect("engine");
    advance_to_main1_from_game_start(&mut e);
    let attacker = move_ready_to_battlefield(&mut e, 0, "fiend_binder");
    let defender_creature = move_ready_to_battlefield(&mut e, 1, "grizzly_bears");
    let bystander = move_ready_to_battlefield(&mut e, 2, "grizzly_bears");
    advance_to_declare_attackers(&mut e);
    let command = RuledCommand {
        cmd: Some(Cmd::DeclareAttackers(DeclareAttackers {
            assignments: vec![AttackAssignment {
                attacker_object_id: attacker,
                attacker_zone_change_generation: e
                    .state
                    .zone_change_generation
                    .get(&attacker)
                    .copied()
                    .unwrap_or(0),
                defender: Some(TargetRef {
                    object_id: e.state.players[1].id as u32,
                    kind: TargetRefKind::Player as i32,
                    ..Default::default()
                }),
                defending_player_id: e.state.players[1].id,
                ..Default::default()
            }],
        })),
    };
    e.apply_scenario_command(0, &command)
        .expect("attack player 1");
    assert_eq!(e.state.pending_triggers.len(), 1);
    assert!(
        e.apply_scenario_command(0, &choose_trigger_target(bystander))
            .is_err(),
        "a creature controlled by a non-defending opponent is not a legal target"
    );
    e.apply_scenario_command(0, &choose_trigger_target(defender_creature))
        .expect("the defending player's creature is legal");
    while !e.state.stack.is_empty() {
        pass_priority_round(&mut e);
    }
    assert!(e.state.objects[&defender_creature].tapped);
    assert!(!e.state.objects[&bystander].tapped);
}
