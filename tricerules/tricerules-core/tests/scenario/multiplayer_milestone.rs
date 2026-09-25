use crate::helpers::*;
use tricerules_core::TurnStep;
use tricerules_proto::ruled::v1::{
    ChooseStartingPlayer, Concede, DeclareAttackers, MulliganDecision,
};

fn take_priority_action(engine: &mut GameEngine) {
    if engine.state.cleanup_discard_player.is_some() {
        resolve_cleanup_discards_if_any(engine);
        return;
    }
    let actor = engine.state.priority_player_id();
    assert!(
        !engine.state.players[engine.state.player_idx(actor).unwrap()].has_lost,
        "lost player must never receive priority"
    );
    engine.apply_command(actor, &pass()).expect("priority pass");
}

fn advance_until(engine: &mut GameEngine, active: i32, step: TurnStep) {
    for _ in 0..120 {
        if engine.state.active_player_id() == active && engine.state.turn_step == step {
            return;
        }
        take_priority_action(engine);
    }
    panic!("did not reach P{active} {step:?}");
}

#[test]
fn three_player_opening_split_combat_elimination_and_final_victory() {
    let decks = Some(vec![vec!["forest".into(); 30]; 3]);
    let mut engine = GameEngine::new(3001, &[0, 1, 2], 20, decks, false).expect("three seats");
    let chooser = engine.state.opening.as_ref().unwrap().chooser;
    engine
        .apply_command(
            chooser,
            &RuledCommand {
                cmd: Some(Cmd::ChooseStartingPlayer(ChooseStartingPlayer {
                    starting_player_id: 0,
                })),
            },
        )
        .expect("choose P0 first");
    engine
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::Mulligan(MulliganDecision { keep: false })),
            },
        )
        .expect("first free-for-all mulligan");
    for seat in [1, 2, 0] {
        assert_eq!(
            engine.state.opening.as_ref().unwrap().mulligan_actor,
            Some(seat)
        );
        engine
            .apply_command(
                seat,
                &RuledCommand {
                    cmd: Some(Cmd::Mulligan(MulliganDecision { keep: true })),
                },
            )
            .expect("keep opening hand");
    }
    assert!(engine.state.opening.is_none());
    assert_eq!(
        engine.state.players[0].hand.len(),
        7,
        "first mulligan is free"
    );
    assert_eq!(engine.state.turn_step, TurnStep::Upkeep);
    advance_until(&mut engine, 0, TurnStep::Main1);
    assert_eq!(
        engine.state.players[0].hand.len(),
        8,
        "first player draws in FFA"
    );

    let attacker_one = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let attacker_two = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let blocker_one = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let blocker_two = inject_creature_on_battlefield(&mut engine, 2, "grizzly_bears");
    engine
        .apply_command(0, &primitive_yield())
        .expect("begin combat");
    advance_until(&mut engine, 0, TurnStep::DeclareAttackers);
    let legal = engine.initial_response_batch().legal_by_player[&0]
        .legal_attack_assignments
        .clone();
    let attack_one = *legal
        .iter()
        .find(|a| a.attacker_object_id == attacker_one && a.defending_player_id == 1)
        .unwrap();
    let attack_two = *legal
        .iter()
        .find(|a| a.attacker_object_id == attacker_two && a.defending_player_id == 2)
        .unwrap();
    engine
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::DeclareAttackers(DeclareAttackers {
                    assignments: vec![attack_one, attack_two],
                })),
            },
        )
        .expect("split attacks");
    advance_until(&mut engine, 0, TurnStep::DeclareBlockers);
    assert_eq!(engine.state.priority_player_id(), 1);
    let wrong = engine.apply_command(
        1,
        &declare_blockers(vec![BlockPair {
            attacker_id: attacker_two,
            blocker_id: blocker_one,
        }]),
    );
    assert!(wrong.is_err(), "P1 cannot block an attacker aimed at P2");
    let first_blocks = engine
        .apply_command(
            1,
            &declare_blockers(vec![BlockPair {
                attacker_id: attacker_one,
                blocker_id: blocker_one,
            }]),
        )
        .expect("P1 blocks");
    assert!(
        blockers_declared_in(&first_blocks).is_empty(),
        "blockers publish after every defender acts"
    );
    assert_eq!(
        engine.state.priority_player_id(),
        2,
        "P2 gets their declaration"
    );
    assert!(engine.initial_response_batch().legal_by_player[&1]
        .legal_block_pairs
        .is_empty());
    let final_blocks = engine
        .apply_command(2, &declare_blockers(vec![]))
        .expect("P2 declines block");
    assert_eq!(
        blockers_declared_in(&final_blocks)[0].block_pairs,
        vec![BlockPair {
            attacker_id: attacker_one,
            blocker_id: blocker_one,
        }]
    );
    assert_eq!(engine.state.priority_player_id(), 0);
    advance_until(&mut engine, 0, TurnStep::EndCombat);
    assert_eq!(engine.state.players[1].life, 20);
    assert_eq!(
        engine.state.players[2].life, 18,
        "unblocked attacker damages P2"
    );
    assert!(engine.state.objects.contains_key(&blocker_two));

    let departure = engine
        .apply_command(
            1,
            &RuledCommand {
                cmd: Some(Cmd::Concede(Concede {})),
            },
        )
        .expect("P1 concedes");
    assert!(engine.state.winner.is_none(), "two players remain");
    assert!(!departure.legal_by_player.contains_key(&1));
    assert!(
        !engine.state.objects.contains_key(&blocker_one),
        "departed player's objects leave the game"
    );
    assert!(
        engine.apply_command(1, &pass()).is_err(),
        "departed player cannot act"
    );
    advance_until(&mut engine, 2, TurnStep::Main1);
    assert_eq!(engine.state.active_player_id(), 2, "turn skips departed P1");
    advance_until(&mut engine, 0, TurnStep::Main1);
    assert_eq!(engine.state.turn_instance, 3, "P2 completed a full turn");
    engine
        .apply_command(
            2,
            &RuledCommand {
                cmd: Some(Cmd::Concede(Concede {})),
            },
        )
        .expect("P2 concedes");
    assert_eq!(engine.state.winner, Some(0));
}

#[test]
fn opening_continues_when_chooser_or_mulligan_actor_leaves() {
    let decks = Some(vec![vec!["forest".into(); 30]; 3]);
    let mut engine = GameEngine::new(3002, &[0, 1, 2], 20, decks, false).unwrap();
    let chooser = engine.state.opening.as_ref().unwrap().chooser;
    engine.apply_command(chooser, &concede()).unwrap();
    let next = engine.state.opening.as_ref().unwrap().chooser;
    assert_ne!(next, chooser);
    for participant in engine.state.players.iter().filter(|p| !p.has_lost) {
        assert!(
            participant.library.len() >= 7,
            "P{} library {}",
            participant.id,
            participant.library.len()
        );
    }
    engine
        .apply_command(
            next,
            &RuledCommand {
                cmd: Some(Cmd::ChooseStartingPlayer(ChooseStartingPlayer {
                    starting_player_id: next,
                })),
            },
        )
        .unwrap();
    for _ in 0..2 {
        let actor = engine
            .state
            .opening
            .as_ref()
            .unwrap()
            .mulligan_actor
            .unwrap();
        engine
            .apply_command(
                actor,
                &RuledCommand {
                    cmd: Some(Cmd::Mulligan(MulliganDecision { keep: true })),
                },
            )
            .unwrap();
    }
    assert!(engine.state.opening.is_none());

    let decks = Some(vec![vec!["forest".into(); 30]; 3]);
    let mut engine = GameEngine::new(3004, &[0, 1, 2], 20, decks, false).unwrap();
    let chooser = engine.state.opening.as_ref().unwrap().chooser;
    engine
        .apply_command(
            chooser,
            &RuledCommand {
                cmd: Some(Cmd::ChooseStartingPlayer(ChooseStartingPlayer {
                    starting_player_id: 0,
                })),
            },
        )
        .unwrap();
    let actor = engine
        .state
        .opening
        .as_ref()
        .unwrap()
        .mulligan_actor
        .unwrap();
    engine.apply_command(actor, &concede()).unwrap();
    assert!(engine.state.opening.is_some());
    for _ in 0..2 {
        let actor = engine
            .state
            .opening
            .as_ref()
            .unwrap()
            .mulligan_actor
            .unwrap();
        assert_ne!(actor, 0);
        engine
            .apply_command(
                actor,
                &RuledCommand {
                    cmd: Some(Cmd::Mulligan(MulliganDecision { keep: true })),
                },
            )
            .unwrap();
    }
    assert!(engine.state.opening.is_none());
    assert_eq!(engine.state.active_player_id(), 1);
}

#[test]
fn departed_active_players_turn_completes_and_next_player_acts() {
    let decks = Some(vec![vec!["forest".into(); 30]; 3]);
    let mut engine = GameEngine::new(3003, &[0, 1, 2], 20, decks, true).unwrap();
    let active = engine.state.active_player_id();
    let next = (active + 1) % 3;
    engine.apply_command(active, &concede()).unwrap();
    assert_eq!(engine.state.active_player_id(), active);
    advance_until(&mut engine, next, TurnStep::Main1);
    assert_eq!(engine.state.active_player_id(), next);
}

#[test]
fn unrelated_concession_does_not_erase_parked_resolution() {
    let mut first = vec!["brainstorm".to_string()];
    first.extend(vec!["island".to_string(); 29]);
    let decks = Some(vec![
        first,
        vec!["forest".into(); 30],
        vec!["forest".into(); 30],
    ]);
    let mut engine = GameEngine::new(3005, &[0, 1, 2], 20, decks, true).unwrap();
    advance_until(&mut engine, 0, TurnStep::Main1);
    ensure_in_hand(&mut engine, 0, "brainstorm");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    let index = hand_index_for_card(&engine, 0, "brainstorm");
    engine.apply_command(0, &cast_spell(index, vec![])).unwrap();
    for player in [0, 1, 2] {
        engine.apply_command(player, &pass()).unwrap();
    }
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .deciding_player,
        0
    );
    engine.apply_command(2, &concede()).unwrap();
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .deciding_player,
        0
    );
    let chosen = engine.state.players[0]
        .hand
        .iter()
        .take(2)
        .copied()
        .collect();
    engine
        .apply_command(0, &submit_resolution_choice(chosen))
        .unwrap();
    assert!(engine.state.pending_resolution.is_none());
}

#[test]
fn departing_spell_owner_clears_its_parked_resolution() {
    let mut first = vec!["brainstorm".to_string()];
    first.extend(vec!["island".to_string(); 29]);
    let decks = Some(vec![
        first,
        vec!["forest".into(); 30],
        vec!["forest".into(); 30],
    ]);
    let mut engine = GameEngine::new(3010, &[0, 1, 2], 20, decks, true).unwrap();
    advance_until(&mut engine, 0, TurnStep::Main1);
    ensure_in_hand(&mut engine, 0, "brainstorm");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    let index = hand_index_for_card(&engine, 0, "brainstorm");
    engine.apply_command(0, &cast_spell(index, vec![])).unwrap();
    for player in [0, 1, 2] {
        engine.apply_command(player, &pass()).unwrap();
    }
    assert!(engine.state.pending_resolution.is_some());
    engine.apply_command(0, &concede()).unwrap();
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.winner.is_none());
    advance_until(&mut engine, 1, TurnStep::Main1);
}

#[test]
fn departure_returns_stolen_permanents_and_exiles_unowned_objects_with_no_controller() {
    use tricerules_cards::primitives::{ContinuousEffectKind, ControllerReference, EffectDuration};
    use tricerules_core::state::{AffectedScope, ContinuousEffect, Zone};

    let decks = Some(vec![vec!["forest".into(); 30]; 3]);
    let mut engine = GameEngine::new(3006, &[0, 1, 2], 20, decks, true).unwrap();
    let stolen = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    engine.state.continuous_effects.push(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: None,
        affected: AffectedScope::Single(stolen),
        kind: ContinuousEffectKind::Layer2Control {
            controller: ControllerReference::Fixed(1),
        },
        condition: None,
        duration: EffectDuration::UntilEndOfTurn,
        timestamp: engine.state.command_index,
    });
    engine.apply_command(0, &pass()).unwrap();
    assert_eq!(engine.state.objects[&stolen].controller, 1);

    let exiled = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    engine
        .state
        .objects
        .get_mut(&exiled)
        .unwrap()
        .base_controller = 1;
    engine.state.objects.get_mut(&exiled).unwrap().controller = 1;
    engine.state.players[0]
        .battlefield
        .retain(|id| *id != exiled);
    engine.state.players[1].battlefield.push(exiled);

    engine.apply_command(1, &concede()).unwrap();
    assert_eq!(engine.state.objects[&stolen].controller, 0);
    assert!(engine.state.players[0].battlefield.contains(&stolen));
    assert_eq!(engine.state.objects[&exiled].zone, Zone::Exile);
    assert!(engine.state.players[0].exile.contains(&exiled));
}

#[test]
fn departing_defender_does_not_strand_block_declarations() {
    let decks = Some(vec![vec!["forest".into(); 30]; 3]);
    let mut engine = GameEngine::new(3007, &[0, 1, 2], 20, decks, true).unwrap();
    advance_until(&mut engine, 0, TurnStep::Main1);
    let attacker_one = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let attacker_two = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    inject_creature_on_battlefield(&mut engine, 2, "grizzly_bears");
    engine.apply_command(0, &primitive_yield()).unwrap();
    advance_until(&mut engine, 0, TurnStep::DeclareAttackers);
    let legal = engine.initial_response_batch().legal_by_player[&0]
        .legal_attack_assignments
        .clone();
    let one = *legal
        .iter()
        .find(|a| a.attacker_object_id == attacker_one && a.defending_player_id == 1)
        .unwrap();
    let two = *legal
        .iter()
        .find(|a| a.attacker_object_id == attacker_two && a.defending_player_id == 2)
        .unwrap();
    engine
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::DeclareAttackers(DeclareAttackers {
                    assignments: vec![one, two],
                })),
            },
        )
        .unwrap();
    advance_until(&mut engine, 0, TurnStep::DeclareBlockers);
    assert_eq!(engine.state.priority_player_id(), 1);
    engine.apply_command(1, &concede()).unwrap();
    assert_eq!(engine.state.priority_player_id(), 2);
    engine.apply_command(2, &declare_blockers(vec![])).unwrap();
    assert!(engine.state.combat.as_ref().unwrap().blockers_declared);
    assert_eq!(engine.state.priority_player_id(), 0);
    advance_until(&mut engine, 0, TurnStep::EndCombat);
}

#[test]
fn sole_attacked_defender_departure_finishes_block_declaration() {
    let decks = Some(vec![vec!["forest".into(); 30]; 3]);
    let mut engine = GameEngine::new(3008, &[0, 1, 2], 20, decks, true).unwrap();
    advance_until(&mut engine, 0, TurnStep::Main1);
    let attacker = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    engine.apply_command(0, &primitive_yield()).unwrap();
    advance_until(&mut engine, 0, TurnStep::DeclareAttackers);
    let assignment = *engine.initial_response_batch().legal_by_player[&0]
        .legal_attack_assignments
        .iter()
        .find(|a| a.attacker_object_id == attacker && a.defending_player_id == 1)
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
    advance_until(&mut engine, 0, TurnStep::DeclareBlockers);
    assert_eq!(engine.state.priority_player_id(), 1);
    engine.apply_command(1, &concede()).unwrap();
    assert!(engine.state.combat.as_ref().unwrap().blockers_declared);
    advance_until(&mut engine, 0, TurnStep::EndCombat);
}

#[test]
fn departing_stack_controller_exiles_survivor_owned_card() {
    use tricerules_core::state::Zone;
    let mut first = vec!["brainstorm".to_string()];
    first.extend(vec!["island".to_string(); 29]);
    let decks = Some(vec![
        first,
        vec!["forest".into(); 30],
        vec!["forest".into(); 30],
    ]);
    let mut engine = GameEngine::new(3009, &[0, 1, 2], 20, decks, true).unwrap();
    advance_until(&mut engine, 0, TurnStep::Main1);
    ensure_in_hand(&mut engine, 0, "brainstorm");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    let index = hand_index_for_card(&engine, 0, "brainstorm");
    engine.apply_command(0, &cast_spell(index, vec![])).unwrap();
    let id = engine.state.stack.last().unwrap().id;
    engine.state.stack.last_mut().unwrap().controller = 1;
    engine.apply_command(1, &concede()).unwrap();
    assert!(engine.state.stack.iter().all(|item| item.id != id));
    assert_eq!(engine.state.objects[&id].zone, Zone::Exile);
    assert!(engine.state.players[0].exile.contains(&id));
}
