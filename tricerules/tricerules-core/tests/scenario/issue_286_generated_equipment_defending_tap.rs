//! Issue #286 — Captain America's Shield and Thunder Lasso trigger from their attached creature's
//! attack and tap exactly one creature controlled by the event-time defending player.
//!
//! Oracle/rulings checked 2026-09-14. CR 301.5, 508.1b/508.1m, 603.2c/603.3d, 608.2b, 609.3,
//! and 701.26 govern Equipment attachment, event-time defending-player context, target
//! publication/revalidation, and tapping.

use super::helpers::*;
use tricerules_core::state::CombatDefenderTarget;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::{ChooseTriggerTarget, TargetRef, TargetRefKind};

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

fn append_third_player(engine: &mut GameEngine) {
    // GameEngine::new intentionally remains a two-seat constructor. This test extends the state
    // after setup, as the engine's targeting and combat snapshots are seat-generic.
    let mut third = engine.state.players[1].clone();
    third.id = 2;
    third.battlefield.clear();
    third.hand.clear();
    third.library.clear();
    third.graveyard.clear();
    third.exile.clear();
    engine.state.players.push(third);
}

fn resolve_entire_stack_three_player(engine: &mut GameEngine) {
    for _ in 0..128 {
        if engine.state.stack.is_empty() {
            return;
        }
        for _ in 0..engine.state.players.len() {
            if engine.state.stack.is_empty() {
                return;
            }
            let priority = engine.state.priority_player_id();
            engine
                .apply_command(priority, &pass())
                .expect("all seats pass priority");
        }
    }
    panic!("three-player stack did not resolve");
}

fn stage_equipment_attack_trigger(seed: u64) -> (GameEngine, u32, u32, u32, u32) {
    let mut engine = GameEngine::new(seed, &[0, 1], 20, None, true).expect("engine");
    advance_to_declare_attackers(&mut engine);

    let attacker = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let equipment = inject_permanent_on_battlefield(&mut engine, 0, "captain_americas_shield");
    let defending_creature = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    engine
        .state
        .objects
        .get_mut(&equipment)
        .expect("Equipment")
        .attached_to = Some(AttachmentRecipient::Object(attacker));

    engine
        .apply_command(0, &declare_attackers(vec![attacker]))
        .expect("declare equipped attacker");
    assert_eq!(engine.state.pending_triggers.len(), 1);
    assert_eq!(
        engine.state.pending_triggers[0]
            .trigger_context
            .defending_player,
        Some(1),
        "the trigger captures the event-time defender"
    );

    append_third_player(&mut engine);
    let different_opponent = inject_creature_on_battlefield(&mut engine, 2, "grizzly_bears");
    let assignment = engine
        .state
        .combat
        .as_mut()
        .expect("combat")
        .attack_assignments
        .get_mut(&attacker)
        .expect("attacker assignment");
    assignment.defending_player = 2;
    assignment.defender = CombatDefenderTarget::Player(2);

    (
        engine,
        equipment,
        attacker,
        defending_creature,
        different_opponent,
    )
}

fn move_to_hand_and_advance_generation(engine: &mut GameEngine, player: usize, object_id: u32) {
    engine.state.players[player]
        .battlefield
        .retain(|candidate| *candidate != object_id);
    engine.state.players[player].hand.push(object_id);
    engine
        .state
        .objects
        .get_mut(&object_id)
        .expect("object")
        .zone = Zone::Hand;
    *engine
        .state
        .zone_change_generation
        .entry(object_id)
        .or_insert(0) += 1;
}

fn move_hand_object_back_to_battlefield(engine: &mut GameEngine, player: usize, object_id: u32) {
    engine.state.players[player]
        .hand
        .retain(|candidate| *candidate != object_id);
    engine.state.players[player].battlefield.push(object_id);
    engine
        .state
        .objects
        .get_mut(&object_id)
        .expect("object")
        .zone = Zone::Battlefield;
    engine
        .state
        .objects
        .get_mut(&object_id)
        .expect("object")
        .tapped = false;
    *engine
        .state
        .zone_change_generation
        .entry(object_id)
        .or_insert(0) += 1;
}

#[test]
fn issue_286_only_event_time_defender_creatures_are_legal_and_tap() {
    let (mut engine, equipment, attacker, defending_creature, different_opponent) =
        stage_equipment_attack_trigger(286_001);
    let response = engine.initial_response_batch();
    let target_groups = response.legal_by_player[&0]
        .valid_targets_by_ability
        .values()
        .collect::<Vec<_>>();
    assert_eq!(
        target_groups.len(),
        1,
        "only the pending Equipment trigger targets"
    );
    assert_eq!(target_groups[0].groups.len(), 1);
    assert_eq!(
        target_groups[0].groups[0].valid_permanent_ids,
        [defending_creature],
        "the trigger keeps the event-time defender after combat changes"
    );

    assert!(
        engine
            .apply_command(0, &choose_trigger_target(attacker))
            .is_err(),
        "the attacking creature cannot satisfy DefendingPlayer"
    );
    assert!(
        engine
            .apply_command(0, &choose_trigger_target(different_opponent))
            .is_err(),
        "a different opponent cannot satisfy the event-time defender"
    );
    engine
        .apply_command(0, &choose_trigger_target(defending_creature))
        .expect("choose the event-time defender's creature");
    assert_eq!(engine.state.stack.len(), 1);
    assert_eq!(engine.state.pending_triggers.len(), 0);
    assert_eq!(
        engine.state.stack[0].source_permanent_id,
        Some(equipment),
        "the Equipment remains the trigger source"
    );

    resolve_entire_stack_three_player(&mut engine);
    assert!(engine.state.objects[&defending_creature].tapped);
    assert!(
        engine.state.objects[&attacker].tapped,
        "the attacker was tapped by combat declaration, not by the chosen target effect"
    );
    assert!(!engine.state.objects[&different_opponent].tapped);
}

#[test]
fn issue_286_already_tapped_defender_remains_legal_and_tapped() {
    let (mut engine, _equipment, _attacker, defending_creature, _different_opponent) =
        stage_equipment_attack_trigger(286_004);
    engine
        .state
        .objects
        .get_mut(&defending_creature)
        .expect("defending creature")
        .tapped = true;

    let response = engine.initial_response_batch();
    let target_groups = response.legal_by_player[&0]
        .valid_targets_by_ability
        .values()
        .collect::<Vec<_>>();
    assert_eq!(target_groups.len(), 1);
    assert_eq!(
        target_groups[0].groups[0].valid_permanent_ids,
        [defending_creature],
        "an already-tapped creature still satisfies the target filter"
    );

    engine
        .apply_command(0, &choose_trigger_target(defending_creature))
        .expect("choose the already-tapped defending creature");
    resolve_entire_stack_three_player(&mut engine);
    assert!(
        engine.state.objects[&defending_creature].tapped,
        "the mandatory tap effect succeeds as a no-op on an already-tapped target"
    );
}

#[test]
fn issue_286_removed_target_is_rejected_at_resolution() {
    let (mut engine, _equipment, _attacker, defending_creature, _different_opponent) =
        stage_equipment_attack_trigger(286_002);
    engine
        .apply_command(0, &choose_trigger_target(defending_creature))
        .expect("choose initially legal defender creature");
    move_to_hand_and_advance_generation(&mut engine, 1, defending_creature);

    resolve_entire_stack_three_player(&mut engine);
    assert_eq!(engine.state.objects[&defending_creature].zone, Zone::Hand);
    assert!(!engine.state.objects[&defending_creature].tapped);
}

#[test]
fn issue_286_same_object_new_generation_is_rejected_at_resolution() {
    let (mut engine, _equipment, _attacker, defending_creature, _different_opponent) =
        stage_equipment_attack_trigger(286_003);
    engine
        .apply_command(0, &choose_trigger_target(defending_creature))
        .expect("choose initially legal defender creature");
    move_to_hand_and_advance_generation(&mut engine, 1, defending_creature);
    move_hand_object_back_to_battlefield(&mut engine, 1, defending_creature);

    resolve_entire_stack_three_player(&mut engine);
    assert_eq!(
        engine.state.objects[&defending_creature].zone,
        Zone::Battlefield
    );
    assert!(!engine.state.objects[&defending_creature].tapped);
}
