//! Actual Maze of Ith: targeted untap and prospective prevention in both directions.
use super::helpers::*;
use tricerules_core::state::{
    DamagePreventionProhibition, DamagePreventionScope, ResolutionContinuation,
};
use tricerules_core::{TurnStep, Zone};

fn reject(engine: &mut GameEngine, actor: i32, command: &RuledCommand) {
    let before = engine.diagnostic_snapshot().unwrap();
    engine
        .apply_command(actor, command)
        .expect_err("illegal input");
    assert_eq!(engine.diagnostic_snapshot().unwrap(), before);
}

fn blocked_maze(seed: u64) -> (GameEngine, u32, u32, u32, u32) {
    let mut engine = GameEngine::new(
        seed,
        &[0, 1],
        20,
        Some(vec![
            deck_with("forest", &["maze_of_ith", "grizzly_bears", "grizzly_bears"]),
            deck_with("forest", &["grizzly_bears"]),
        ]),
        true,
    )
    .expect("complete Maze of Ith registration");
    advance_to_main1_from_game_start(&mut engine);
    let maze = relocate_to_battlefield(&mut engine, 0, "maze_of_ith", false);
    let attacker = relocate_to_battlefield(&mut engine, 0, "grizzly_bears", false);
    let unrelated = relocate_to_battlefield(&mut engine, 0, "grizzly_bears", false);
    let blocker = relocate_to_battlefield(&mut engine, 1, "grizzly_bears", false);
    engine.apply_command(0, &primitive_yield()).unwrap();
    pass_both_players(&mut engine);
    engine
        .apply_command(0, &declare_attackers(vec![attacker, unrelated]))
        .unwrap();
    pass_both_players(&mut engine);
    engine
        .apply_command(
            1,
            &declare_blockers(vec![BlockPair {
                attacker_id: attacker,
                blocker_id: blocker,
            }]),
        )
        .unwrap();
    (engine, maze, attacker, blocker, unrelated)
}

#[test]
fn maze_untaps_without_removing_attacker_and_prevents_both_combat_directions() {
    let (mut engine, maze, attacker, blocker, unrelated) = blocked_maze(511_001);
    assert!(engine.state.objects[&attacker].tapped);
    apply_ability(&mut engine, 0, maze, 0, target_object(attacker)).unwrap();
    assert!(engine.state.objects[&maze].tapped);
    assert!(
        engine.state.objects[&attacker].tapped,
        "untap waits for normal stack resolution"
    );
    assert_eq!(engine.state.stack.len(), 1);
    resolve_entire_stack_two_player(&mut engine);
    assert!(!engine.state.objects[&attacker].tapped);
    assert!(engine
        .state
        .combat
        .as_ref()
        .unwrap()
        .attacking
        .contains(&attacker));
    let labels = zone_view_rules_annotation_labels(&mut engine, 0, attacker);
    assert!(labels.contains(&"Prevent all combat damage".into()));
    assert!(labels.contains(&"Prevent all combat damage dealt by this creature".into()));
    pass_both_players(&mut engine);
    assert_eq!(engine.state.objects[&attacker].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&blocker].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&attacker].damage, 0);
    assert_eq!(engine.state.objects[&blocker].damage, 0);
    assert_eq!(
        engine.state.players[1].life, 18,
        "unrelated attacker still deals combat damage"
    );
    assert!(engine.state.objects[&unrelated].tapped);
    assert!(engine.state.pending_resolution.is_none());
}

#[test]
fn maze_rejects_nonattacker_noncreature_wrong_actor_tap_stale_source_and_extra_targets_purely() {
    let (mut engine, maze, attacker, blocker, unrelated) = blocked_maze(511_002);
    for targets in [
        target_object(blocker),
        target_object(maze),
        vec![],
        [target_object(attacker), target_object(unrelated)].concat(),
    ] {
        let command = activate_ability_for(&engine, maze, 0, targets);
        reject(&mut engine, 0, &command);
    }
    let mut command = activate_ability_for(&engine, maze, 0, target_object(attacker));
    reject(&mut engine, 1, &command);
    let Some(Cmd::ActivateAbility(activation)) = &mut command.cmd else {
        unreachable!()
    };
    activation.expected_zone_change_generation += 1;
    reject(&mut engine, 0, &command);
    apply_ability(&mut engine, 0, maze, 0, target_object(attacker)).unwrap();
    let command = activate_ability_for(&engine, maze, 0, target_object(attacker));
    reject(&mut engine, 0, &command);
}

#[test]
fn maze_accepts_an_already_untapped_attacker_and_suppresses_unblocked_player_damage() {
    let (mut engine, maze, _attacker, _blocker, unrelated) = blocked_maze(511_003);
    engine.state.objects.get_mut(&unrelated).unwrap().tapped = false;
    apply_ability(&mut engine, 0, maze, 0, target_object(unrelated)).unwrap();
    resolve_entire_stack_two_player(&mut engine);
    pass_both_players(&mut engine);
    assert_eq!(engine.state.players[1].life, 20);
    assert!(!engine.state.objects[&unrelated].tapped);
}

#[test]
fn maze_can_untap_at_end_combat_without_undoing_damage() {
    let (mut engine, maze, _attacker, _blocker, unrelated) = blocked_maze(511_004);
    pass_both_players(&mut engine);
    assert_eq!(engine.state.turn_step, TurnStep::CombatDamage);
    assert_eq!(engine.state.players[1].life, 18);
    pass_both_players(&mut engine);
    assert_eq!(engine.state.turn_step, TurnStep::EndCombat);
    let history = engine
        .state
        .turn_history
        .current
        .dealt_damage_objects
        .clone();
    apply_ability(&mut engine, 0, maze, 0, target_object(unrelated)).unwrap();
    resolve_entire_stack_two_player(&mut engine);
    assert!(!engine.state.objects[&unrelated].tapped);
    assert!(engine
        .state
        .combat
        .as_ref()
        .unwrap()
        .attacking
        .contains(&unrelated));
    assert_eq!(engine.state.players[1].life, 18);
    assert_eq!(
        engine.state.turn_history.current.dealt_damage_objects,
        history
    );
    pass_both_players(&mut engine);
    assert_eq!(engine.state.turn_step, TurnStep::Main2);
    assert!(engine.state.combat.is_none());
    assert_eq!(engine.state.players[1].life, 18);
}

#[test]
fn maze_fizzles_on_a_new_target_incarnation() {
    let (mut engine, maze, attacker, _blocker, _unrelated) = blocked_maze(511_005);
    apply_ability(&mut engine, 0, maze, 0, target_object(attacker)).unwrap();
    // Bounded same-physical-id leave/return fixture: captured target receipt is now stale.
    *engine
        .state
        .zone_change_generation
        .entry(attacker)
        .or_default() += 2;
    resolve_entire_stack_two_player(&mut engine);
    assert!(engine.state.objects[&attacker].tapped);
    assert!(zone_view_rules_annotation_labels(&mut engine, 0, attacker).is_empty());
    assert!(engine.state.damage_prevention_effects.is_empty());
}

#[test]
fn maze_ability_and_resolved_shields_survive_its_source_departure_and_expire_at_cleanup() {
    let (mut engine, maze, attacker, blocker, _unrelated) = blocked_maze(511_006);
    apply_ability(&mut engine, 0, maze, 0, target_object(attacker)).unwrap();
    engine.state.players[0]
        .battlefield
        .retain(|&oid| oid != maze);
    engine.state.players[0].graveyard.push(maze);
    engine.state.objects.get_mut(&maze).unwrap().zone = Zone::Graveyard;
    *engine.state.zone_change_generation.entry(maze).or_default() += 1;
    resolve_entire_stack_two_player(&mut engine);
    pass_both_players(&mut engine);
    assert_eq!(engine.state.objects[&attacker].damage, 0);
    assert_eq!(engine.state.objects[&blocker].damage, 0);
    pass_both_players(&mut engine); // end combat
    pass_both_players(&mut engine); // main2
    pass_both_players(&mut engine); // end step
    pass_both_players(&mut engine); // cleanup and roll
    resolve_cleanup_discards_if_any(&mut engine);
    assert!(engine.state.damage_prevention_effects.is_empty());
    assert!(zone_view_rules_annotation_labels(&mut engine, 0, attacker).is_empty());
}

#[test]
fn maze_cannot_prevent_unpreventable_combat_damage() {
    let (mut engine, maze, attacker, blocker, _unrelated) = blocked_maze(511_007);
    apply_ability(&mut engine, 0, maze, 0, target_object(attacker)).unwrap();
    resolve_entire_stack_two_player(&mut engine);
    engine
        .state
        .damage_prevention_prohibitions
        .push(DamagePreventionProhibition { source_id: None });
    pass_both_players(&mut engine);
    assert_eq!(engine.state.objects[&attacker].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&blocker].zone, Zone::Graveyard);
    assert_eq!(engine.state.players[1].life, 18);
}

#[test]
fn maze_outgoing_overlap_parks_the_entire_combat_batch_and_replays_choices() {
    let (mut first, maze, attacker, blocker, _) = blocked_maze(511_008);
    let (mut replay, _, _, _, _) = blocked_maze(511_008);
    first.state.add_damage_prevention_shield(blocker, 1);
    replay.state.add_damage_prevention_shield(blocker, 1);
    let command = activate_ability_for(&first, maze, 0, target_object(attacker));
    assert_eq!(
        first.apply_command(0, &command).unwrap(),
        replay.apply_command(0, &command).unwrap()
    );
    for _ in 0..4 {
        let actor = first.state.priority_player_id();
        assert_eq!(
            first.apply_command(actor, &pass()).unwrap(),
            replay.apply_command(actor, &pass()).unwrap()
        );
    }
    let pending = first.state.pending_resolution.as_ref().unwrap();
    assert_eq!(
        pending.deciding_player, 1,
        "recipient's controller chooses outgoing versus incoming shield"
    );
    assert_eq!(
        first.state.players[1].life, 20,
        "unrelated damage has not committed"
    );
    assert_eq!(first.state.objects[&attacker].damage, 0);
    let outgoing = first.state.damage_prevention_effects.iter().find(|effect|
        matches!(effect.scope, DamagePreventionScope::CombatSource { object_id, .. } if object_id == attacker)).unwrap().id;
    let ResolutionContinuation::CombatDamageReplacement { effect_ids } = &pending.continuation
    else {
        panic!("private combat continuation")
    };
    let choice =
        pending.presentation.candidates[effect_ids.iter().position(|&id| id == outgoing).unwrap()];
    let command = submit_resolution_choice(vec![choice]);
    reject(&mut first, 0, &command);
    reject(&mut first, 1, &submit_resolution_choice(vec![u32::MAX]));
    assert_eq!(
        first.apply_command(1, &command).unwrap(),
        replay.apply_command(1, &command).unwrap()
    );
    assert_eq!(
        first.diagnostic_snapshot().unwrap(),
        replay.diagnostic_snapshot().unwrap()
    );
    assert!(first.state.pending_resolution.is_none());
    assert_eq!(first.state.players[1].life, 18);
    assert_eq!(first.state.objects[&blocker].damage, 0);
    assert_eq!(
        first.state.remaining_damage_prevention(blocker),
        1,
        "outgoing shield chosen first preserves finite shield"
    );
}

#[test]
fn maze_parked_damage_terminal_completion_publishes_no_priority_or_remaining_choice() {
    let (mut engine, maze, attacker, blocker, _) = blocked_maze(511_011);
    engine.state.players[1].life = 2;
    engine.state.add_damage_prevention_shield(blocker, 1);
    apply_ability(&mut engine, 0, maze, 0, target_object(attacker)).unwrap();
    resolve_entire_stack_two_player(&mut engine);
    pass_both_players(&mut engine);
    let pending = engine.state.pending_resolution.as_ref().unwrap();
    let choice = pending.presentation.candidates[0];
    let result = engine
        .apply_command(1, &submit_resolution_choice(vec![choice]))
        .unwrap();
    assert!(engine.state.is_terminal());
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.diagnostic_snapshot().unwrap()["state"]["pending_replacement_event"].is_null());
    assert!(priority_changes_in(&result).is_empty());
    assert!(result.events.iter().any(
        |event| matches!(&event.ev, Some(Ev::Log(log)) if log.text == "Game over. Winner: 0")
    ));
}

#[test]
fn third_player_maze_targets_an_opponents_attacker_against_another_defender() {
    let mut engine = GameEngine::new(
        511_009,
        &[0, 1, 2],
        20,
        Some(vec![
            deck_with("forest", &["grizzly_bears"]),
            deck_with("forest", &["grizzly_bears"]),
            deck_with("forest", &["maze_of_ith"]),
        ]),
        true,
    )
    .unwrap();
    advance_to_main1_from_game_start(&mut engine);
    let attacker = relocate_to_battlefield(&mut engine, 0, "grizzly_bears", false);
    let blocker = relocate_to_battlefield(&mut engine, 1, "grizzly_bears", false);
    let maze = relocate_to_battlefield(&mut engine, 2, "maze_of_ith", false);
    engine.apply_command(0, &primitive_yield()).unwrap();
    pass_priority_round(&mut engine);
    let assignment = *engine.initial_response_batch().legal_by_player[&0]
        .legal_attack_assignments
        .iter()
        .find(|assignment| {
            assignment.attacker_object_id == attacker && assignment.defending_player_id == 1
        })
        .unwrap();
    engine
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::DeclareAttackers(DeclareAttackers {
                    assignments: vec![assignment],
                })),
            },
        )
        .unwrap();
    pass_priority_round(&mut engine);
    engine
        .apply_command(
            1,
            &declare_blockers(vec![BlockPair {
                attacker_id: attacker,
                blocker_id: blocker,
            }]),
        )
        .unwrap();
    while engine.state.priority_player_id() != 2 {
        let actor = engine.state.priority_player_id();
        engine.apply_command(actor, &pass()).unwrap();
    }
    apply_ability(&mut engine, 2, maze, 0, target_object(attacker)).unwrap();
    pass_priority_round(&mut engine); // resolve Maze
    pass_priority_round(&mut engine); // deal combat damage
    assert_eq!(engine.state.objects[&attacker].damage, 0);
    assert_eq!(engine.state.objects[&blocker].damage, 0);
    assert_eq!(
        engine
            .state
            .players
            .iter()
            .map(|player| player.life)
            .collect::<Vec<_>>(),
        [20, 20, 20]
    );
    assert!(!engine.state.objects[&attacker].tapped);
    assert!(engine.state.objects[&maze].tapped);
}

#[test]
fn combat_damage_settles_player_departure_before_creature_deaths_and_targeted_triggers() {
    for (parked, automatic) in [(false, false), (true, false), (false, true), (true, true)] {
        let mut engine = GameEngine::new(
            511_010,
            &[0, 1, 2],
            20,
            Some(vec![
                deck_with(
                    "forest",
                    &["grizzly_bears", "grizzly_bears", "blood_artist"],
                ),
                deck_with("forest", &["grizzly_bears"]),
                deck_with("forest", &[]),
            ]),
            true,
        )
        .unwrap();
        advance_to_main1_from_game_start(&mut engine);
        let attacker = relocate_to_battlefield(&mut engine, 0, "grizzly_bears", false);
        let unblocked = relocate_to_battlefield(&mut engine, 0, "grizzly_bears", false);
        let blocker = relocate_to_battlefield(&mut engine, 1, "grizzly_bears", false);
        relocate_to_battlefield(&mut engine, 0, "blood_artist", false);
        engine.state.players[1].life = 2;
        if parked {
            engine.state.objects.get_mut(&unblocked).unwrap().power = Some(4);
            engine.state.add_damage_prevention_shield(1, 1);
            engine.state.add_damage_prevention_shield(1, 1);
        }
        engine.apply_command(0, &primitive_yield()).unwrap();
        pass_priority_round(&mut engine);
        let offers = engine.initial_response_batch().legal_by_player[&0]
            .legal_attack_assignments
            .clone();
        let assignments = [attacker, unblocked]
            .iter()
            .map(|&oid| {
                *offers
                    .iter()
                    .find(|offer| offer.attacker_object_id == oid && offer.defending_player_id == 1)
                    .unwrap()
            })
            .collect();
        engine
            .apply_command(
                0,
                &RuledCommand {
                    cmd: Some(Cmd::DeclareAttackers(DeclareAttackers { assignments })),
                },
            )
            .unwrap();
        pass_priority_round(&mut engine);
        engine
            .apply_command(
                1,
                &declare_blockers(vec![BlockPair {
                    attacker_id: attacker,
                    blocker_id: blocker,
                }]),
            )
            .unwrap();
        let mut final_batch = None;
        let passes = if automatic { 2 } else { 3 };
        for index in 0..passes {
            let actor = engine.state.priority_player_id();
            let command = if automatic && index == 1 {
                use prost::Message;
                use tricerules_proto::ruled::v1::{
                    AutoPassPolicy, CanonicalGameplayCommand, PhaseId,
                };
                RuledCommand {
                    cmd: Some(Cmd::CanonicalGameplay(CanonicalGameplayCommand {
                        command: pass().encode_to_vec(),
                        auto_pass_policies: [0, 1, 2]
                            .into_iter()
                            .map(|player_id| AutoPassPolicy {
                                player_id,
                                stop_on_own_turn: vec![PhaseId::CombatDamage as i32],
                                stop_on_opponent_turn: vec![PhaseId::CombatDamage as i32],
                            })
                            .collect(),
                    })),
                }
            } else {
                pass()
            };
            final_batch = Some(engine.apply_command(actor, &command).unwrap());
        }
        if parked {
            assert_eq!(engine.state.players[1].life, 2);
            assert_eq!(engine.state.objects[&blocker].zone, Zone::Battlefield);
            let pending = engine.state.pending_resolution.as_ref().unwrap();
            assert_eq!(pending.deciding_player, 1);
            let chosen = pending.presentation.candidates[0];
            final_batch = Some(
                engine
                    .apply_command(1, &submit_resolution_choice(vec![chosen]))
                    .unwrap(),
            );
        }
        assert!(engine.state.players[1].has_lost);
        assert!(!engine.state.objects.contains_key(&blocker));
        assert_eq!(engine.state.objects[&attacker].zone, Zone::Graveyard);
        assert_eq!(engine.state.pending_triggers.len(), 1,
        "departed player's blocker leaves the game before a death SBA; only the surviving player's attacker dies");
        assert!(
            priority_changes_in(final_batch.as_ref().unwrap()).is_empty(),
            "targeted death trigger blocks priority publication"
        );
        let chosen = engine
            .apply_command(
                0,
                &RuledCommand {
                    cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
                        targets: target_player(2),
                        ..Default::default()
                    })),
                },
            )
            .unwrap();
        assert_eq!(
            priority_changes_in(&chosen),
            [0],
            "priority is published once after targeting finishes"
        );
        assert_eq!(engine.state.stack.len(), 1);
        assert_eq!(
            engine.state.players[2].life, 20,
            "trigger is on stack, not prematurely resolved"
        );
    }
}
