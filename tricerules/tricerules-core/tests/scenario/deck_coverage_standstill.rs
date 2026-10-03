//! Actual Standstill: successful source sacrifice and frozen caster-relative draw.
use super::helpers::*;
use tricerules_cards::{CardRegistry, ContinuousEffectKind, ControllerReference, EffectDuration};
use tricerules_core::state::ActiveDeathReplacement;
use tricerules_core::Zone;
use tricerules_core::{AffectedScope, ContinuousEffect};
use tricerules_proto::ruled::v1 as rv1;

fn start() -> (GameEngine, u32) {
    let deck = deck_with("forest", &[]);
    let mut engine = GameEngine::new(
        512_001,
        &[10, 20, 30],
        20,
        Some(vec![deck.clone(), deck.clone(), deck]),
        true,
    )
    .expect("complete Standstill registration");
    advance_to_main1_from_game_start(&mut engine);
    let source = inject_permanent_on_battlefield(&mut engine, 1, "standstill");
    engine.apply_command(10, &pass()).unwrap();
    engine.apply_command(20, &pass()).unwrap();
    give_mana(
        &mut engine,
        30,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    inject_card_into_hand(&mut engine, 2, "lightning_bolt");
    (engine, source)
}

fn pending() -> (GameEngine, u32) {
    let (mut engine, source) = start();
    let slot = hand_index_for_card(&engine, 2, "lightning_bolt");
    engine
        .apply_command(30, &cast_spell(slot, target_player(10)))
        .unwrap();
    assert_eq!(engine.state.stack.len(), 2);
    let trigger = engine.state.stack.last().unwrap();
    assert!(trigger.is_triggered);
    assert_eq!(trigger.card_id, "standstill");
    assert_eq!(trigger.controller, 20);
    assert_eq!(trigger.source_permanent_id, Some(source));
    assert_eq!(trigger.trigger_context.affected_player, Some(30));
    (engine, source)
}

#[test]
fn standstill_sacrifices_then_draws_three_for_casters_opponents_before_spell() {
    let (mut engine, source) = pending();
    let hands: Vec<_> = engine
        .state
        .players
        .iter()
        .map(|p| p.hand.clone())
        .collect();
    let libraries: Vec<_> = engine
        .state
        .players
        .iter()
        .map(|p| p.library.clone())
        .collect();
    pass_priority_round(&mut engine);
    assert_eq!(engine.state.objects[&source].zone, Zone::Graveyard);
    assert!(engine.state.players[1].graveyard.contains(&source));
    assert_eq!(
        engine.state.stack.len(),
        1,
        "original Bolt remains below resolved trigger"
    );
    assert_eq!(engine.state.stack[0].card_id, "lightning_bolt");
    assert_eq!(engine.state.players[0].life, 20, "Bolt has not resolved");
    for seat in [0, 1] {
        assert_eq!(engine.state.players[seat].hand.len(), hands[seat].len() + 3);
        assert_eq!(
            engine.state.players[seat].library.len(),
            libraries[seat].len() - 3
        );
        assert_eq!(
            &engine.state.players[seat].hand[hands[seat].len()..],
            &libraries[seat].iter().take(3).copied().collect::<Vec<_>>()
        );
    }
    assert_eq!(engine.state.players[2].hand, hands[2]);
    assert_eq!(engine.state.players[2].library, libraries[2]);
    assert!(
        engine.state.pending_resolution.is_none(),
        "automatic branches never prompt"
    );
    pass_priority_round(&mut engine);
    assert_eq!(engine.state.players[0].life, 17);
}

fn move_source(engine: &mut GameEngine, zone: rv1::DevZone) {
    engine.enable_dev_commands();
    engine
        .apply_command(
            20,
            &RuledCommand {
                cmd: Some(Cmd::DevCommand(rv1::DevCommand {
                    target_player_id: 20,
                    dev: Some(rv1::dev_command::Dev::MoveCard(rv1::DevMoveCard {
                        card_name: "Standstill".into(),
                        zone: zone as i32,
                        ready: false,
                    })),
                })),
            },
        )
        .unwrap();
}

fn control(engine: &mut GameEngine, source: u32, controller: i32) {
    engine.state.continuous_effects.push(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: None,
        affected: AffectedScope::Single(source),
        kind: ContinuousEffectKind::Layer2Control {
            controller: ControllerReference::Fixed(controller),
        },
        condition: None,
        duration: EffectDuration::Indefinite,
        timestamp: engine.state.command_index + engine.state.continuous_effects.len() as u64 + 1,
    });
    engine.initial_response_batch();
}

#[test]
fn standstill_absent_and_returned_incarnations_do_not_pay_or_prompt() {
    for return_source in [false, true] {
        let (mut engine, source) = pending();
        let old_generation = semantic::generation(&engine, source);
        move_source(&mut engine, rv1::DevZone::Exile);
        if return_source {
            move_source(&mut engine, rv1::DevZone::Battlefield);
        }
        assert!(semantic::generation(&engine, source) > old_generation);
        let hands: Vec<_> = engine
            .state
            .players
            .iter()
            .map(|p| p.hand.clone())
            .collect();
        pass_priority_round(&mut engine);
        assert_eq!(engine.state.stack.len(), 1);
        assert_eq!(
            engine.state.objects[&source].zone,
            if return_source {
                Zone::Battlefield
            } else {
                Zone::Exile
            }
        );
        for (seat, hand) in hands.iter().enumerate() {
            assert_eq!(&engine.state.players[seat].hand, hand);
        }
        assert!(engine.state.pending_resolution.is_none());
    }
}

#[test]
fn standstill_requires_captured_controllers_payment_and_can_regain_control() {
    for regain in [false, true] {
        let (mut engine, source) = pending();
        control(&mut engine, source, 10);
        assert_eq!(engine.state.stack.last().unwrap().controller, 20);
        if regain {
            control(&mut engine, source, 20);
        }
        let hands: Vec<_> = engine.state.players.iter().map(|p| p.hand.len()).collect();
        pass_priority_round(&mut engine);
        assert_eq!(
            engine.state.objects[&source].zone,
            if regain {
                Zone::Graveyard
            } else {
                Zone::Battlefield
            }
        );
        for (seat, hand) in hands.iter().enumerate() {
            assert_eq!(
                engine.state.players[seat].hand.len(),
                hand + if regain && seat != 2 { 3 } else { 0 }
            );
        }
        assert!(engine.state.pending_resolution.is_none());
    }
}

#[test]
fn standstill_uses_controller_for_payment_and_owner_for_destination() {
    let (mut engine, source) = pending();
    engine.state.objects.get_mut(&source).unwrap().owner = 30;
    let hands: Vec<_> = engine.state.players.iter().map(|p| p.hand.len()).collect();
    pass_priority_round(&mut engine);
    assert!(engine.state.players[2].graveyard.contains(&source));
    assert!(!engine.state.players[1].graveyard.contains(&source));
    assert_eq!(engine.state.players[0].hand.len(), hands[0] + 3);
    assert_eq!(engine.state.players[1].hand.len(), hands[1] + 3);
    assert_eq!(engine.state.players[2].hand.len(), hands[2]);
}

#[test]
fn standstill_first_successful_of_two_cast_triggers_draws_only_that_casters_opponents() {
    let (mut engine, source) = pending();
    engine.apply_command(30, &pass()).unwrap();
    inject_card_into_hand(&mut engine, 0, "lightning_bolt");
    give_mana(
        &mut engine,
        10,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "lightning_bolt");
    engine
        .apply_command(10, &cast_spell(slot, target_player(30)))
        .unwrap();
    assert_eq!(engine.state.stack.len(), 4);
    assert_eq!(
        engine.state.stack[1].trigger_context.affected_player,
        Some(30)
    );
    assert_eq!(
        engine.state.stack[3].trigger_context.affected_player,
        Some(10)
    );
    let hands: Vec<_> = engine.state.players.iter().map(|p| p.hand.len()).collect();
    pass_priority_round(&mut engine);
    assert_eq!(engine.state.objects[&source].zone, Zone::Graveyard);
    assert_eq!(engine.state.players[0].hand.len(), hands[0]);
    assert_eq!(engine.state.players[1].hand.len(), hands[1] + 3);
    assert_eq!(engine.state.players[2].hand.len(), hands[2] + 3);
    let drawn: Vec<_> = engine
        .state
        .players
        .iter()
        .map(|p| p.hand.clone())
        .collect();
    pass_priority_round(&mut engine); // The second cast's Bolt.
    pass_priority_round(&mut engine); // The older trigger cannot sacrifice again.
    assert_eq!(engine.state.stack.len(), 1);
    for (seat, hand) in drawn.iter().enumerate() {
        assert_eq!(&engine.state.players[seat].hand, hand);
    }
    assert!(engine.state.pending_resolution.is_none());
}

#[test]
fn standstill_replaced_exile_still_pays_and_fires_sacrifice_but_not_graveyard_observer() {
    let (mut engine, source) = pending();
    let observer = inject_creature_on_battlefield(&mut engine, 1, "pirate_peddlers");
    let ability = CardRegistry::global()
        .get("ichor_wellspring")
        .unwrap()
        .primary_face()
        .triggered_abilities[0]
        .clone();
    engine.state.add_triggered_ability_grant(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: None,
        affected: AffectedScope::Single(source),
        kind: ContinuousEffectKind::GrantTriggeredAbility(Box::new(ability)),
        condition: None,
        duration: EffectDuration::UntilEndOfTurn,
        timestamp: engine.state.command_index,
    });
    engine
        .state
        .death_replacement_effects
        .push(ActiveDeathReplacement {
            object_id: source,
            zone_change_generation: semantic::generation(&engine, source),
        });
    let hands: Vec<_> = engine.state.players.iter().map(|p| p.hand.len()).collect();
    pass_priority_round(&mut engine);
    assert_eq!(engine.state.objects[&source].zone, Zone::Exile);
    assert!(!engine.state.players[1].graveyard.contains(&source));
    assert_eq!(engine.state.players[0].hand.len(), hands[0] + 3);
    assert_eq!(engine.state.players[1].hand.len(), hands[1] + 3);
    assert_eq!(engine.state.players[2].hand.len(), hands[2]);
    assert_eq!(
        engine.state.stack.len(),
        2,
        "Bolt and exactly one sacrifice observer; no graveyard observer"
    );
    assert_eq!(
        engine.state.stack.last().unwrap().card_id,
        "pirate_peddlers"
    );
    pass_priority_round(&mut engine);
    assert_eq!(
        engine.state.objects[&observer]
            .counter_count(tricerules_cards::CounterKind::PlusOnePlusOne),
        1
    );
    assert_eq!(engine.state.players[1].hand.len(), hands[1] + 3);
}

#[test]
fn standstill_caster_and_other_departure_preserve_cohort_but_controller_departure_removes_trigger()
{
    for departed in [10, 20, 30] {
        let (mut engine, source) = pending();
        engine.apply_command(departed, &concede()).unwrap();
        let hands: Vec<_> = engine.state.players.iter().map(|p| p.hand.len()).collect();
        if departed == 20 {
            assert!(engine.state.stack.iter().all(|item| !item.is_triggered));
            assert!(!engine
                .state
                .objects
                .get(&source)
                .is_some_and(|object| object.zone == Zone::Battlefield));
        } else {
            assert_eq!(
                engine
                    .state
                    .stack
                    .last()
                    .unwrap()
                    .trigger_context
                    .affected_player,
                Some(30)
            );
            pass_priority_round(&mut engine);
            assert_eq!(engine.state.objects[&source].zone, Zone::Graveyard);
            for (seat, hand) in hands.iter().enumerate() {
                let player = &engine.state.players[seat];
                let draws = !player.has_lost && player.id != 30;
                assert_eq!(player.hand.len(), hand + if draws { 3 } else { 0 });
            }
        }
        assert!(engine.state.pending_resolution.is_none());
    }
}

#[test]
fn standstill_own_cast_and_activated_ability_are_not_its_spell_cast_trigger() {
    let deck = deck_with("forest", &[]);
    let mut engine =
        GameEngine::new(512_002, &[10, 20], 20, Some(vec![deck.clone(), deck]), true).unwrap();
    advance_to_main1_from_game_start(&mut engine);
    inject_card_into_hand(&mut engine, 0, "standstill");
    give_mana(
        &mut engine,
        10,
        ManaGift {
            c: 1,
            u: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "standstill");
    engine.apply_command(10, &cast_spell(slot, vec![])).unwrap();
    assert_eq!(engine.state.stack.len(), 1);
    assert!(!engine.state.stack[0].is_triggered);
    pass_priority_round(&mut engine);
    let source = battlefield_object_for_card(&engine, 0, "standstill");
    let artifact = inject_permanent_on_battlefield(&mut engine, 0, "codex_shredder");
    engine
        .apply_command(10, &activate_ability(artifact, 0, target_player(20)))
        .unwrap();
    assert_eq!(engine.state.stack.len(), 1);
    assert!(!engine.state.stack[0].is_triggered);
    pass_priority_round(&mut engine);
    assert_eq!(engine.state.objects[&source].zone, Zone::Battlefield);
}

#[test]
fn standstill_logged_resolution_replays_identical_batches_and_state() {
    let (mut engine, _) = start();
    let (mut replay, _) = start();
    let slot = hand_index_for_card(&engine, 2, "lightning_bolt");
    for (actor, command) in [
        (20, cast_spell(slot, target_player(10))),
        (30, cast_spell(slot, target_object(u32::MAX))),
    ] {
        let before = engine.diagnostic_snapshot().unwrap();
        assert!(engine.apply_command(actor, &command).is_err());
        assert_eq!(engine.diagnostic_snapshot().unwrap(), before);
    }
    let mut commands = vec![(30, cast_spell(slot, target_player(10)))];
    let mut batches = vec![engine.apply_command(commands[0].0, &commands[0].1).unwrap()];
    while !engine.state.stack.is_empty() {
        let actor = engine.state.priority_player_id();
        let command = pass();
        batches.push(engine.apply_command(actor, &command).unwrap());
        commands.push((actor, command));
    }
    let repeated: Vec<_> = commands
        .iter()
        .map(|(actor, cmd)| replay.apply_command(*actor, cmd).unwrap())
        .collect();
    assert_eq!(repeated, batches);
    assert_eq!(
        engine.diagnostic_snapshot().unwrap(),
        replay.diagnostic_snapshot().unwrap()
    );
}

#[test]
fn standstill_on_battlefield_during_twincast_copy_is_not_triggered_by_copy_creation() {
    let deck = deck_with("forest", &[]);
    let mut engine =
        GameEngine::new(512_003, &[10, 20], 20, Some(vec![deck.clone(), deck]), true).unwrap();
    advance_to_main1_from_game_start(&mut engine);
    give_mana(
        &mut engine,
        10,
        ManaGift {
            r: 1,
            u: 2,
            ..Default::default()
        },
    );
    inject_card_into_hand(&mut engine, 0, "lightning_bolt");
    let bolt_slot = hand_index_for_card(&engine, 0, "lightning_bolt");
    engine
        .apply_command(10, &cast_spell(bolt_slot, target_player(20)))
        .unwrap();
    let bolt = engine.state.stack[0].id;
    inject_card_into_hand(&mut engine, 0, "twincast");
    let slot = hand_index_for_card(&engine, 0, "twincast");
    engine
        .apply_command(10, &cast_spell(slot, target_object(bolt)))
        .unwrap();
    // The source exists during actual copy creation, but was absent for the two real casts.
    let source = inject_permanent_on_battlefield(&mut engine, 1, "standstill");
    let history = engine.state.turn_history.current.spell_casts.clone();
    let hands: Vec<_> = engine
        .state
        .players
        .iter()
        .map(|p| p.hand.clone())
        .collect();
    pass_priority_round(&mut engine);
    assert!(engine.state.pending_resolution.is_some());
    engine
        .apply_command(10, &submit_resolution_choice(vec![20]))
        .unwrap();
    assert!(engine.state.stack.last().unwrap().is_copy);
    assert_eq!(engine.state.stack.last().unwrap().cast_occurrence, None);
    assert!(engine.state.stack.iter().all(|item| !item.is_triggered));
    assert_eq!(engine.state.turn_history.current.spell_casts, history);
    assert_eq!(engine.state.objects[&source].zone, Zone::Battlefield);
    for (seat, hand) in hands.iter().enumerate() {
        assert_eq!(&engine.state.players[seat].hand, hand);
    }
}
