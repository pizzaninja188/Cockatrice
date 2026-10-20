//! Actual Standstill: successful source sacrifice and frozen caster-relative draw.
use super::helpers::*;
use crate::helpers::ScenarioCommandExt;
use tricerules_cards::{ContinuousEffectKind, ControllerReference, EffectDuration};
use tricerules_core::state::ActiveDeathReplacement;
use tricerules_core::Zone;
use tricerules_core::{AffectedScope, ContinuousEffect};
use tricerules_proto::ruled::v1 as rv1;

fn start() -> (GameEngine, u32) {
    start_with_abundance(false)
}

fn start_with_abundance(include_abundance: bool) -> (GameEngine, u32) {
    let plain_deck = deck_with("forest", &[]);
    let decks = if include_abundance {
        Some(vec![
            deck_with("forest", &["abundance"]),
            plain_deck.clone(),
            plain_deck,
        ])
    } else {
        Some(vec![plain_deck.clone(), plain_deck.clone(), plain_deck])
    };
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        if include_abundance { 512_004 } else { 512_001 },
        &[10, 20, 30],
        20,
        decks,
        true,
    )
    .expect("complete Standstill registration");
    advance_to_main1_from_game_start(&mut engine);
    if include_abundance {
        move_ready_to_battlefield(&mut engine, 0, "abundance");
    }
    let source = inject_permanent_on_battlefield(&mut engine, 1, "standstill");
    engine.apply_scenario_command(10, &pass()).unwrap();
    engine.apply_scenario_command(20, &pass()).unwrap();
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

fn put_cards_on_top(engine: &mut GameEngine, player: usize, card_ids: &[&str]) -> Vec<u32> {
    let object_ids = card_ids
        .iter()
        .map(|card_id| inject_library_card(engine, player, card_id))
        .collect::<Vec<_>>();
    engine.state.players[player]
        .library
        .retain(|object_id| !object_ids.contains(object_id));
    for object_id in object_ids.iter().rev() {
        engine.state.players[player].library.push_front(*object_id);
    }
    object_ids
}

fn pending() -> (GameEngine, u32) {
    let (mut engine, source) = start();
    let slot = hand_index_for_card(&engine, 2, "lightning_bolt");
    engine
        .apply_scenario_command(30, &cast_spell(slot, target_player(10)))
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

fn pending_with_abundance() -> (GameEngine, u32, Vec<u32>) {
    let (mut engine, source) = start_with_abundance(true);
    let replacement_cards = put_cards_on_top(
        &mut engine,
        0,
        &["grizzly_bears", "storm_crow", "llanowar_elves"],
    );
    let slot = hand_index_for_card(&engine, 2, "lightning_bolt");
    engine
        .apply_scenario_command(30, &cast_spell(slot, target_player(10)))
        .unwrap();
    assert_eq!(engine.state.stack.len(), 2);
    let trigger = engine.state.stack.last().unwrap();
    assert!(trigger.is_triggered);
    assert_eq!(trigger.card_id, "standstill");
    assert_eq!(trigger.controller, 20);
    assert_eq!(trigger.source_permanent_id, Some(source));
    assert_eq!(trigger.trigger_context.affected_player, Some(30));
    (engine, source, replacement_cards)
}

fn choose_standstill_branch(
    engine: &mut GameEngine,
    player: i32,
    branch_index: u32,
) -> Result<rv1::RuledEventBatch, tricerules_core::EngineError> {
    engine.apply_scenario_command(player, &standstill_branch_command(branch_index))
}

fn standstill_branch_command(branch_index: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::SubmitResolutionChoice(rv1::SubmitResolutionChoice {
            resolution_prompt_id: 0,
            decision: rv1::ResolutionChoiceDecision::SelectBranch as i32,
            selected_branch_index: branch_index,
            ..Default::default()
        })),
    }
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

#[test]
fn standstill_missing_captured_caster_fails_before_program_instruction_zero_atomically() {
    let (mut engine, source) = pending();
    let trigger = engine.state.stack.last_mut().unwrap();
    assert_eq!(trigger.trigger_context.affected_player, Some(30));
    trigger.trigger_context.affected_player = None;

    let active_players = engine
        .state
        .players
        .iter()
        .filter(|player| !player.has_lost)
        .count();
    for _ in 1..active_players {
        let actor = engine.state.priority_player_id();
        engine
            .apply_scenario_command(actor, &pass())
            .expect("earlier players in the priority round pass");
    }

    let actor = engine.state.priority_player_id();
    let before = engine
        .diagnostic_snapshot()
        .expect("full pre-resolution diagnostic snapshot");
    let error = engine
        .apply_scenario_command(actor, &pass())
        .expect_err("missing captured caster must fail before resolving Standstill's Program");
    assert!(matches!(
        error,
        tricerules_core::EngineError::ProgramInvariant(reason)
            if reason == "resolution program is missing its captured affected player"
    ));
    assert_eq!(
        engine
            .diagnostic_snapshot()
            .expect("post-rejection diagnostic snapshot"),
        before,
        "failed preflight must be atomic"
    );
    assert_eq!(engine.state.objects[&source].zone, Zone::Battlefield);
    assert_eq!(engine.state.stack.len(), 2);
    assert!(engine.state.pending_resolution.is_none());
}

fn move_source(engine: &mut GameEngine, zone: rv1::DevZone) {
    engine.enable_dev_commands();
    engine
        .apply_scenario_command(
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
    engine.apply_scenario_command(30, &pass()).unwrap();
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
        .apply_scenario_command(10, &cast_spell(slot, target_player(30)))
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
fn standstill_abundance_draw_replacement_parks_resumes_and_replays_once() {
    let (mut engine, source, replacement_cards) = pending_with_abundance();
    let (mut replay, replay_source, replay_cards) = pending_with_abundance();
    assert_eq!(source, replay_source);
    assert_eq!(replacement_cards, replay_cards);
    let spell_id = engine.state.stack[0].id;
    let hands: Vec<_> = engine
        .state
        .players
        .iter()
        .map(|player| player.hand.clone())
        .collect();
    let libraries: Vec<_> = engine
        .state
        .players
        .iter()
        .map(|player| player.library.clone())
        .collect();
    let assert_later_recipients_and_caster_unchanged = |engine: &GameEngine| {
        for seat in [1, 2] {
            assert_eq!(engine.state.players[seat].hand, hands[seat]);
            assert_eq!(engine.state.players[seat].library, libraries[seat]);
        }
    };

    let active_players = engine
        .state
        .players
        .iter()
        .filter(|player| !player.has_lost)
        .count();
    let mut accepted_commands = Vec::new();
    let mut accepted_batches = Vec::new();
    let mut batch = None;
    for _ in 0..active_players {
        let actor = engine.state.priority_player_id();
        let command = pass();
        let result = engine
            .apply_scenario_command(actor, &command)
            .expect("pass until Standstill's actual trigger begins resolving");
        accepted_commands.push((actor, command));
        accepted_batches.push(result.clone());
        batch = Some(result);
    }
    let mut batch = batch.expect("the priority round produces a resolution batch");
    let first_replacement = find_resolution_choice(&batch)
        .expect("the first caster-excluded recipient parks on Abundance");
    assert_eq!(first_replacement.deciding_player_id, 10);
    assert_eq!(
        first_replacement.choice_kind(),
        rv1::ChoiceKind::ResolutionBranch
    );
    assert!(first_replacement.public_reveal.is_none());
    assert_eq!(engine.state.objects[&source].zone, Zone::Graveyard);
    assert!(engine.state.players[1].graveyard.contains(&source));
    assert_eq!(
        engine.state.stack[0].id, spell_id,
        "the original Bolt remains below the trigger"
    );
    assert_later_recipients_and_caster_unchanged(&engine);

    let before_wrong_actor = engine
        .diagnostic_snapshot()
        .expect("pre-rejection diagnostic snapshot");
    assert!(choose_standstill_branch(&mut engine, 30, 0).is_err());
    assert_eq!(
        engine
            .diagnostic_snapshot()
            .expect("post-rejection diagnostic snapshot"),
        before_wrong_actor,
        "another player cannot answer the recipient's private replacement choice"
    );

    for (draw_index, expected_card) in replacement_cards.iter().copied().enumerate() {
        let replacement = find_resolution_choice(&batch)
            .expect("each of the recipient's three draws receives its own replacement choice");
        assert_eq!(replacement.deciding_player_id, 10);
        assert_eq!(replacement.choice_kind(), rv1::ChoiceKind::ResolutionBranch);
        assert!(replacement.public_reveal.is_none());

        let replace_command = standstill_branch_command(0);
        batch = engine
            .apply_scenario_command(10, &replace_command)
            .expect("choose Abundance for this draw");
        accepted_commands.push((10, replace_command));
        accepted_batches.push(batch.clone());

        let kind_choice = find_resolution_choice(&batch).expect("choose land or nonland");
        assert_eq!(kind_choice.deciding_player_id, 10);
        assert_eq!(kind_choice.choice_kind(), rv1::ChoiceKind::ResolutionBranch);
        assert!(kind_choice.public_reveal.is_none());
        let nonland_command = standstill_branch_command(1);
        batch = engine
            .apply_scenario_command(10, &nonland_command)
            .expect("take the first nonland revealed by Abundance");
        accepted_commands.push((10, nonland_command));
        accepted_batches.push(batch.clone());

        let revealed = batch
            .events
            .iter()
            .find_map(|event| match &event.ev {
                Some(Ev::CardsRevealed(revealed)) => Some(revealed),
                _ => None,
            })
            .expect("Abundance reveals through the requested nonland");
        assert_eq!(
            revealed
                .cards
                .iter()
                .map(|card| card.object_id)
                .collect::<Vec<_>>(),
            vec![expected_card]
        );
        assert!(engine.state.players[0].hand.contains(&expected_card));
        if draw_index + 1 < replacement_cards.len() {
            assert_later_recipients_and_caster_unchanged(&engine);
            let next = find_resolution_choice(&batch)
                .expect("the next draw parks independently on Abundance");
            assert_eq!(next.deciding_player_id, 10);
            assert_eq!(next.choice_kind(), rv1::ChoiceKind::ResolutionBranch);
        }
    }

    assert!(find_resolution_choice(&batch).is_none());
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(engine.state.stack.len(), 1);
    assert_eq!(engine.state.stack[0].id, spell_id);
    assert_eq!(engine.state.objects[&source].zone, Zone::Graveyard);
    for (seat, before_hand) in hands.iter().enumerate() {
        let expected_draws = match seat {
            0 | 1 => 3,
            _ => 0,
        };
        assert_eq!(
            engine.state.players[seat].hand.len(),
            before_hand.len() + expected_draws
        );
    }
    for object_id in &replacement_cards {
        assert!(engine.state.players[0].hand.contains(object_id));
    }
    for seat in [0, 1] {
        assert_eq!(
            engine.state.players[seat].library.len(),
            libraries[seat].len() - 3
        );
    }
    assert_eq!(engine.state.players[2].hand, hands[2]);
    assert_eq!(engine.state.players[2].library, libraries[2]);

    for (index, (actor, command)) in accepted_commands.iter().enumerate() {
        let replayed = replay
            .apply_scenario_command(*actor, command)
            .expect("replay every accepted Standstill and Abundance choice command");
        assert_eq!(replayed, accepted_batches[index]);
    }
    assert_eq!(
        engine.diagnostic_snapshot().unwrap(),
        replay.diagnostic_snapshot().unwrap()
    );
}

#[test]
fn standstill_replaced_exile_still_pays_and_fires_sacrifice_but_not_graveyard_observer() {
    let (mut engine, source) = pending();
    let observer = inject_creature_on_battlefield(&mut engine, 1, "pirate_peddlers");
    let ability = tricerules_cards::registry::global()
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
        engine.apply_scenario_command(departed, &concede()).unwrap();
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
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        512_002,
        &[10, 20],
        20,
        Some(vec![deck.clone(), deck]),
        true,
    )
    .unwrap();
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
    engine
        .apply_scenario_command(10, &cast_spell(slot, vec![]))
        .unwrap();
    assert_eq!(engine.state.stack.len(), 1);
    assert!(!engine.state.stack[0].is_triggered);
    pass_priority_round(&mut engine);
    let source = battlefield_object_for_card(&engine, 0, "standstill");
    let artifact = inject_permanent_on_battlefield(&mut engine, 0, "codex_shredder");
    engine
        .apply_scenario_command(10, &activate_ability(artifact, 0, target_player(20)))
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
        assert!(engine.apply_scenario_command(actor, &command).is_err());
        assert_eq!(engine.diagnostic_snapshot().unwrap(), before);
    }
    let mut commands = vec![(30, cast_spell(slot, target_player(10)))];
    let mut batches = vec![engine
        .apply_scenario_command(commands[0].0, &commands[0].1)
        .unwrap()];
    while !engine.state.stack.is_empty() {
        let actor = engine.state.priority_player_id();
        let command = pass();
        batches.push(engine.apply_scenario_command(actor, &command).unwrap());
        commands.push((actor, command));
    }
    let repeated: Vec<_> = commands
        .iter()
        .map(|(actor, cmd)| replay.apply_scenario_command(*actor, cmd).unwrap())
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
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        512_003,
        &[10, 20],
        20,
        Some(vec![deck.clone(), deck]),
        true,
    )
    .unwrap();
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
        .apply_scenario_command(10, &cast_spell(bolt_slot, target_player(20)))
        .unwrap();
    let bolt = engine.state.stack[0].id;
    inject_card_into_hand(&mut engine, 0, "twincast");
    let slot = hand_index_for_card(&engine, 0, "twincast");
    engine
        .apply_scenario_command(10, &cast_spell(slot, target_object(bolt)))
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
        .apply_scenario_command(10, &submit_resolution_player_choice(vec![20]))
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
