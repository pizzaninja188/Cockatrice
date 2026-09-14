//! Issue #281 — the generated creature ETB recursion ability targets only a controller-owned
//! instant or sorcery card and revalidates that exact graveyard object at resolution.

use super::helpers::*;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::TargetRefKind;

fn engine_with_shipwreck(seed: u64) -> GameEngine {
    let mut engine = GameEngine::new(
        seed,
        &[0, 1],
        20,
        Some(vec![
            deck_with("island", &["shipwreck_dowser"]),
            forest_only_deck(),
        ]),
        true,
    )
    .expect("new Shipwreck Dowser engine");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn choose_graveyard_target(engine: &mut GameEngine, object_id: u32) {
    engine
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
                    decline: false,
                    selected_modes: Vec::new(),
                    targets: vec![TargetRef {
                        object_id,
                        damage_amount: 0,
                        group_index: 0,
                        kind: TargetRefKind::Graveyard as i32,
                    }],
                })),
            },
        )
        .expect("choose Shipwreck graveyard target");
}

fn pending_graveyard_targets(engine: &GameEngine, batch: &RuledEventBatch) -> Vec<u32> {
    let pending = engine
        .state
        .pending_triggers
        .front()
        .expect("Shipwreck target pending");
    let key = (pending.source_permanent_id as u64) << 32 | pending.ability_index as u64;
    batch
        .legal_by_player
        .get(&0)
        .expect("controller legal actions")
        .valid_targets_by_ability
        .get(&key)
        .expect("Shipwreck legal target group")
        .groups[0]
        .valid_graveyard_ids
        .clone()
}

fn move_graveyard_object_to_exile(engine: &mut GameEngine, player: usize, object_id: u32) {
    engine.state.players[player]
        .graveyard
        .retain(|id| *id != object_id);
    engine.state.players[player].exile.push(object_id);
    engine
        .state
        .objects
        .get_mut(&object_id)
        .expect("graveyard object")
        .zone = Zone::Exile;
    *engine
        .state
        .zone_change_generation
        .entry(object_id)
        .or_default() += 1;
}

fn return_exiled_object_to_graveyard(engine: &mut GameEngine, player: usize, object_id: u32) {
    engine.state.players[player]
        .exile
        .retain(|id| *id != object_id);
    engine.state.players[player].graveyard.push(object_id);
    engine
        .state
        .objects
        .get_mut(&object_id)
        .expect("exiled object")
        .zone = Zone::Graveyard;
    *engine
        .state
        .zone_change_generation
        .entry(object_id)
        .or_default() += 1;
}

#[test]
fn issue_281_targets_only_controller_owned_instants_and_sorceries() {
    let mut engine = engine_with_shipwreck(281_001);
    let instant = inject_graveyard_card(&mut engine, 0, "lightning_bolt");
    let sorcery = inject_graveyard_card(&mut engine, 0, "divination");
    let artifact = inject_graveyard_card(&mut engine, 0, "bonesplitter");
    let creature = inject_graveyard_card(&mut engine, 0, "grizzly_bears");
    let opponent_instant = inject_graveyard_card(&mut engine, 1, "counterspell");

    let source = move_ready_to_battlefield(&mut engine, 0, "shipwreck_dowser");
    let batch = engine.initial_response_batch();
    assert!(
        engine
            .state
            .pending_triggers
            .iter()
            .any(|pending| pending.source_permanent_id == source),
        "Shipwreck ETB trigger should await its target"
    );
    let candidates = pending_graveyard_targets(&engine, &batch);
    assert_eq!(candidates, vec![instant, sorcery]);
    for illegal in [artifact, creature, opponent_instant] {
        let error = engine
            .apply_command(
                0,
                &RuledCommand {
                    cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
                        decline: false,
                        selected_modes: Vec::new(),
                        targets: vec![TargetRef {
                            object_id: illegal,
                            damage_amount: 0,
                            group_index: 0,
                            kind: TargetRefKind::Graveyard as i32,
                        }],
                    })),
                },
            )
            .expect_err("nonmatching or opponent graveyard card must be illegal");
        assert!(matches!(error, tricerules_core::EngineError::Illegal(_)));
        assert!(!engine.state.stack.is_empty() || !engine.state.pending_triggers.is_empty());
    }

    choose_graveyard_target(&mut engine, instant);
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&instant].zone, Zone::Hand);
    assert!(engine.state.players[0].hand.contains(&instant));
    assert_eq!(engine.state.objects[&sorcery].zone, Zone::Graveyard);
}

#[test]
fn issue_281_revalidates_the_exact_graveyard_generation_on_resolution() {
    let mut engine = engine_with_shipwreck(281_002);
    let target = inject_graveyard_card(&mut engine, 0, "lightning_bolt");
    move_ready_to_battlefield(&mut engine, 0, "shipwreck_dowser");
    let batch = engine.initial_response_batch();
    let _ = pending_graveyard_targets(&engine, &batch);
    choose_graveyard_target(&mut engine, target);

    move_graveyard_object_to_exile(&mut engine, 0, target);
    return_exiled_object_to_graveyard(&mut engine, 0, target);
    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(engine.state.objects[&target].zone, Zone::Graveyard);
    assert!(!engine.state.players[0].hand.contains(&target));
    assert!(engine.state.stack.is_empty());
}

#[test]
fn issue_281_sole_target_removal_fizzles_without_moving_the_card() {
    let mut engine = engine_with_shipwreck(281_003);
    let target = inject_graveyard_card(&mut engine, 0, "lightning_bolt");
    move_ready_to_battlefield(&mut engine, 0, "shipwreck_dowser");
    let batch = engine.initial_response_batch();
    let _ = pending_graveyard_targets(&engine, &batch);
    choose_graveyard_target(&mut engine, target);

    move_graveyard_object_to_exile(&mut engine, 0, target);
    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(engine.state.objects[&target].zone, Zone::Exile);
    assert!(!engine.state.players[0].hand.contains(&target));
    assert!(engine.state.stack.is_empty());
}
