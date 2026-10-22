//! Exact deck-corpus coverage for Garruk's Packleader.
//!
//! Oracle text and Scryfall rulings were checked on 2026-09-26. The rulings confirm simultaneous
//! entry triggers and that power is checked as the creature enters. CR 603.3/603.5 cover putting
//! the trigger on the stack and choosing the optional draw on resolution; CR 603.6a covers the
//! entry event, and CR 121.1 covers drawing a card.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::{
    ruled_command::Cmd, ruled_event::Ev, ChoiceKind, ChooseTriggerTarget, ResolutionChoiceDecision,
    RuledCommand, RuledEventBatch,
};

const GARRUKS_PACKLEADER: &str = "garruks_packleader";

fn engine(seed: u64) -> GameEngine {
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1],
        20,
        Some(vec![forest_only_deck(), forest_only_deck()]),
        true,
    )
    .expect("new Garruk's Packleader engine");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn three_player_engine(seed: u64) -> GameEngine {
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1, 2],
        20,
        Some(vec![
            forest_only_deck(),
            forest_only_deck(),
            forest_only_deck(),
        ]),
        true,
    )
    .expect("new three-player Garruk's Packleader engine");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn four_player_engine(seed: u64) -> GameEngine {
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1, 2, 3],
        20,
        Some(vec![forest_only_deck(); 4]),
        false,
    )
    .expect("new four-player Garruk's Packleader engine");
    let chooser = engine.state.opening.as_ref().unwrap().chooser;
    engine
        .apply_scenario_command(
            chooser,
            &RuledCommand {
                cmd: Some(Cmd::ChooseStartingPlayer(
                    tricerules_proto::ruled::v1::ChooseStartingPlayer {
                        starting_player_id: 2,
                    },
                )),
            },
        )
        .expect("P2 chooses to start");
    while let Some(actor) = engine
        .state
        .opening
        .as_ref()
        .and_then(|opening| opening.mulligan_actor)
    {
        engine
            .apply_scenario_command(
                actor,
                &RuledCommand {
                    cmd: Some(Cmd::Mulligan(
                        tricerules_proto::ruled::v1::MulliganDecision { keep: true },
                    )),
                },
            )
            .expect("keep opening hand");
    }
    advance_to_main1_from_game_start(&mut engine);
    assert_eq!(engine.state.active_player_id(), 2);
    engine
}

fn choose_trigger_target(object_id: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
            decline: false,
            selected_modes: Vec::new(),
            targets: target_object(object_id),
        })),
    }
}

fn resolve_top_stack_item(engine: &mut GameEngine) -> RuledEventBatch {
    answer_trigger_order_in_engine_order(engine);
    let first = engine.state.priority_player_id();
    let second = if first == engine.state.players[0].id {
        engine.state.players[1].id
    } else {
        engine.state.players[0].id
    };
    engine
        .apply_scenario_command(first, &pass())
        .expect("first player passes priority");
    engine
        .apply_scenario_command(second, &pass())
        .expect("second player passes priority and the top item resolves")
}

fn advance_to_player_main1(engine: &mut GameEngine, player: i32) {
    for _ in 0..160 {
        if engine.state.turn_step == tricerules_core::TurnStep::Main1
            && engine.state.active_player_id() == player
        {
            return;
        }
        let (actor, command) = match engine.state.cleanup_discard_player {
            Some(player) => {
                let player_index = engine.state.player_idx(player).expect("cleanup player");
                let excess = engine.state.players[player_index].hand.len() - 7;
                (player, discard_cleanup_batch((0..excess as u32).collect()))
            }
            None => (engine.state.priority_player_id(), pass()),
        };
        engine
            .apply_scenario_command(actor, &command)
            .expect("pass priority toward the requested player's main phase");
    }
    panic!("game did not reach player {player}'s main phase");
}

fn cast_permanent(engine: &mut GameEngine, player: i32, card_id: &str) -> u32 {
    let card = inject_card_into_hand(engine, player as usize, card_id);
    grant_pool(engine, player as usize);
    let slot = hand_index_for_card(engine, player as usize, card_id);
    engine
        .apply_scenario_command(player, &cast_spell(slot, vec![]))
        .unwrap_or_else(|error| panic!("cast {card_id}: {error:?}"));
    let _ = resolve_top_stack_item(engine);
    assert_eq!(engine.state.objects[&card].zone, Zone::Battlefield);
    card
}

fn optional_draw_choice(
    engine: &mut GameEngine,
) -> tricerules_proto::ruled::v1::ResolutionChoiceRequired {
    let batch = resolve_top_stack_item(engine);
    let choice = find_resolution_choice(&batch).expect("optional draw resolution branch");
    assert_eq!(choice.deciding_player_id, 0);
    assert_eq!(choice.choice_kind(), ChoiceKind::ResolutionBranch);
    assert_eq!((choice.min, choice.max), (0, 1));
    choice
}

fn select_branch(index: u32) -> RuledCommand {
    let mut command = submit_resolution_decision(ResolutionChoiceDecision::SelectBranch);
    if let Some(Cmd::SubmitResolutionChoice(answer)) = command.cmd.as_mut() {
        answer.selected_branch_index = index;
    }
    command
}

fn stack_resolved_count(batch: &RuledEventBatch, stack_id: u32) -> usize {
    batch
        .events
        .iter()
        .filter(|event| {
            matches!(event.ev.as_ref(), Some(Ev::StackResolved(resolved)) if resolved.object_id == stack_id)
        })
        .count()
}

#[test]
fn garruks_packleader_draws_for_a_controlled_creature_with_power_three_or_greater() {
    let mut engine = engine(202_609_263);
    let packleader = cast_permanent(&mut engine, 0, GARRUKS_PACKLEADER);
    assert!(
        engine.state.stack.is_empty(),
        "Packleader is not another creature"
    );

    cast_permanent(&mut engine, 0, "grizzly_bears");
    assert!(
        engine.state.stack.is_empty(),
        "a 2/2 does not qualify at entry"
    );

    let library_before = engine.state.players[0].library.len();
    let expected_draw = *engine.state.players[0]
        .library
        .front()
        .expect("a library card to draw");
    cast_permanent(&mut engine, 0, "air_elemental");
    assert_eq!(
        engine.state.stack.len(),
        1,
        "the qualifying entry triggers once"
    );

    let choice = optional_draw_choice(&mut engine);
    engine
        .apply_scenario_command(
            choice.deciding_player_id,
            &submit_resolution_decision(ResolutionChoiceDecision::SelectBranch),
        )
        .expect("choose to draw");

    assert_eq!(engine.state.players[0].library.len(), library_before - 1);
    assert!(engine.state.players[0].hand.contains(&expected_draw));
    assert!(engine.state.players[1]
        .library
        .iter()
        .all(|card| *card != expected_draw));
    assert!(engine.state.stack.is_empty());
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.objects[&packleader].zone == Zone::Battlefield);

    advance_to_player_main1(&mut engine, 1);
    cast_permanent(&mut engine, 1, "air_elemental");
    assert!(
        engine.state.stack.is_empty(),
        "an opponent's creature does not qualify"
    );
}

#[test]
fn garruks_packleader_can_decline_its_optional_draw() {
    let mut engine = engine(202_609_264);
    cast_permanent(&mut engine, 0, GARRUKS_PACKLEADER);
    cast_permanent(&mut engine, 0, "air_elemental");

    let library_before = engine.state.players[0].library.len();
    let expected_draw = *engine.state.players[0]
        .library
        .front()
        .expect("a library card to remain");
    let _choice = optional_draw_choice(&mut engine);
    engine
        .apply_scenario_command(
            0,
            &submit_resolution_decision(ResolutionChoiceDecision::Decline),
        )
        .expect("decline to draw");

    assert_eq!(engine.state.players[0].library.len(), library_before);
    assert!(engine.state.players[0].library.front() == Some(&expected_draw));
    assert!(!engine.state.players[0].hand.contains(&expected_draw));
    assert!(engine.state.stack.is_empty());
    assert!(engine.state.pending_resolution.is_none());
}

#[test]
fn garruks_packleader_controller_concession_preserves_three_player_optional_choice() {
    for (seed, accept) in [(2026_1021, true), (2026_1024, false)] {
        run_packleader_controller_concession(seed, accept);
    }
}

fn run_packleader_controller_concession(seed: u64, accept: bool) {
    let mut engine = three_player_engine(seed);
    for card_id in [GARRUKS_PACKLEADER, "air_elemental"] {
        inject_card_into_hand(&mut engine, 0, card_id);
        grant_pool(&mut engine, 0);
        let slot = hand_index_for_card(&engine, 0, card_id);
        engine
            .apply_scenario_command(0, &cast_spell(slot, vec![]))
            .unwrap_or_else(|error| panic!("cast {card_id}: {error:?}"));
        pass_priority_round(&mut engine);
    }
    assert_eq!(engine.state.stack.len(), 1, "the creature entry triggers");

    pass_priority_round(&mut engine);
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("the optional trigger reaches its resolution choice");
    assert_eq!(pending.deciding_player, 0);
    assert!(matches!(
        &pending.continuation,
        ResolutionContinuation::OptionalTriggeredAbility { .. }
    ));
    let ResolutionContinuation::OptionalTriggeredAbility { stack, .. } = &pending.continuation
    else {
        unreachable!("checked above");
    };
    let original = stack.item.clone();
    let original_source = original.source_permanent_id;
    assert!(original_source.is_some());
    let draws_before: Vec<_> = engine
        .state
        .players
        .iter()
        .map(|player| {
            engine
                .state
                .turn_history
                .current
                .player(player.id)
                .cards_drawn
        })
        .collect();

    let departure = engine
        .apply_scenario_command(0, &concede())
        .expect("the trigger controller may concede during the optional choice");
    assert!(engine.state.players[0].has_lost);
    assert!(!engine.state.players[1].has_lost && !engine.state.players[2].has_lost);
    assert!(engine.state.outcome.is_none(), "the survivors keep playing");
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("the begun optional trigger remains pending for delegation");
    let ResolutionContinuation::OptionalTriggeredAbility { stack, .. } = &pending.continuation
    else {
        panic!("the original optional trigger continuation is retained");
    };
    assert_eq!(
        stack.item.controller, 0,
        "the original chooser is preserved"
    );
    assert_eq!(stack.item.id, original.id);
    assert_eq!(stack.item.source_permanent_id, original_source);
    assert_eq!(stack.item.source_zone_change, original.source_zone_change);
    assert_eq!(stack.item.targets, original.targets);
    assert!(find_resolution_choice(&departure).is_some());
    for (player, before) in engine.state.players.iter().zip(&draws_before) {
        assert_eq!(
            engine
                .state
                .turn_history
                .current
                .player(player.id)
                .cards_drawn,
            *before,
            "departure and delegation do not themselves draw cards"
        );
    }

    let survivor_libraries: Vec<_> = engine.state.players[1..]
        .iter()
        .map(|player| player.library.len())
        .collect();
    let selector = pending.deciding_player;
    let selector_prompt_id = pending.resolution_prompt_id.get();
    assert_ne!(selector, 0);
    engine
        .apply_scenario_command(selector, &select_branch(0))
        .expect("a survivor selects the delegate for Packleader's draw");
    let delegated = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("the delegate receives the original optional draw");
    let delegate = delegated.deciding_player;
    assert!(!engine.state.players[engine.state.player_idx(delegate).unwrap()].has_lost);
    assert_ne!(
        (delegate, delegated.resolution_prompt_id.get()),
        (selector, selector_prompt_id),
        "selector and delegate answer stages have distinct actor-and-prompt identities"
    );
    let completed = engine
        .apply_scenario_command(
            delegate,
            &submit_resolution_decision(if accept {
                ResolutionChoiceDecision::SelectBranch
            } else {
                ResolutionChoiceDecision::Decline
            }),
        )
        .expect("the delegate may apply or decline the begun optional draw");
    assert_eq!(stack_resolved_count(&completed, original.id), 1);
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
    assert_eq!(
        engine.state.players[1..]
            .iter()
            .map(|player| player.library.len())
            .collect::<Vec<_>>(),
        survivor_libraries,
        "the original controller's draw is not redirected to a surviving player"
    );
    for player in &engine.state.players {
        assert_eq!(
            engine
                .state
                .turn_history
                .current
                .player(player.id)
                .cards_drawn,
            draws_before[engine.state.player_idx(player.id).unwrap()],
            "no player receives a redirected Packleader draw after controller departure"
        );
    }
}

#[test]
fn garruks_packleader_unrelated_departure_keeps_original_controller_choice() {
    let mut engine = three_player_engine(2026_1023);
    for card_id in [GARRUKS_PACKLEADER, "air_elemental"] {
        inject_card_into_hand(&mut engine, 0, card_id);
        grant_pool(&mut engine, 0);
        let slot = hand_index_for_card(&engine, 0, card_id);
        engine
            .apply_scenario_command(0, &cast_spell(slot, vec![]))
            .unwrap_or_else(|error| panic!("cast {card_id}: {error:?}"));
        pass_priority_round(&mut engine);
    }
    pass_priority_round(&mut engine);
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("Packleader's optional draw is pending");
    let original_prompt_id = pending.resolution_prompt_id.get();
    let ResolutionContinuation::OptionalTriggeredAbility { stack, .. } = &pending.continuation
    else {
        panic!("Packleader uses the optional-trigger continuation");
    };
    let original = stack.item.clone();
    let draws_before = engine.state.turn_history.current.player(0).cards_drawn;

    engine
        .apply_scenario_command(2, &concede())
        .expect("an unrelated player may concede while the original choice is pending");
    assert!(engine.state.players[2].has_lost);
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("the unrelated departure leaves Packleader's choice pending");
    assert_eq!(pending.deciding_player, 0);
    assert_eq!(pending.resolution_prompt_id.get(), original_prompt_id);
    let ResolutionContinuation::OptionalTriggeredAbility { stack, .. } = &pending.continuation
    else {
        panic!("unrelated departure does not replace the original continuation");
    };
    assert_eq!(stack.item.id, original.id);
    assert_eq!(stack.item.controller, 0);
    assert_eq!(stack.item.source_permanent_id, original.source_permanent_id);

    let applied = engine
        .apply_scenario_command(
            0,
            &submit_resolution_decision(ResolutionChoiceDecision::SelectBranch),
        )
        .expect("the original controller applies Packleader's draw");
    assert_eq!(stack_resolved_count(&applied, original.id), 1);
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(
        engine.state.turn_history.current.player(0).cards_drawn,
        draws_before + 1,
        "the choice remains with its live original controller"
    );
}

#[test]
fn garruks_packleader_restores_departure_choice_after_nested_aura_return() {
    let mut engine = four_player_engine(2026_1026);
    let first_creature = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let eligible_creature = inject_creature_on_battlefield(&mut engine, 1, "hill_giant");
    let aura = inject_permanent_on_battlefield(&mut engine, 1, "pacifism");
    engine.state.objects.get_mut(&aura).unwrap().attached_to =
        Some(tricerules_core::AttachmentRecipient::Object(first_creature));

    inject_card_into_hand(&mut engine, 2, "banishing_light");
    grant_pool(&mut engine, 2);
    let banishing_light_slot = hand_index_for_card(&engine, 2, "banishing_light");
    engine
        .apply_scenario_command(2, &cast_spell(banishing_light_slot, vec![]))
        .expect("P2 casts Banishing Light");
    pass_priority_round(&mut engine);
    assert_eq!(engine.state.pending_triggers.len(), 1);
    engine
        .apply_scenario_command(2, &choose_trigger_target(aura))
        .expect("Banishing Light targets P1's Pacifism");
    pass_priority_round(&mut engine);
    assert_eq!(engine.state.objects[&aura].zone, Zone::Exile);
    assert_eq!(engine.state.active_event_observers.len(), 1);

    advance_to_player_main1(&mut engine, 0);
    for card_id in [GARRUKS_PACKLEADER, "air_elemental"] {
        inject_card_into_hand(&mut engine, 0, card_id);
        grant_pool(&mut engine, 0);
        let slot = hand_index_for_card(&engine, 0, card_id);
        engine
            .apply_scenario_command(0, &cast_spell(slot, vec![]))
            .unwrap_or_else(|error| panic!("P0 casts {card_id}: {error:?}"));
        pass_priority_round(&mut engine);
    }
    pass_priority_round(&mut engine);

    let initial_pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("Packleader's optional draw is pending");
    assert_eq!(initial_pending.deciding_player, 0);
    let initial_prompt_id = initial_pending.resolution_prompt_id.get();
    let ResolutionContinuation::OptionalTriggeredAbility { stack, .. } =
        &initial_pending.continuation
    else {
        panic!("Packleader's optional trigger is the suspended root");
    };
    let original = stack.item.clone();
    assert_eq!(original.controller, 0);
    let original_source = original.source_permanent_id;
    assert!(original_source.is_some());
    let original_source_generation = original.source_zone_change;
    let original_targets = original.targets.clone();
    let root_id = original.id;
    let draws_before: Vec<_> = engine
        .state
        .players
        .iter()
        .map(|player| {
            engine
                .state
                .turn_history
                .current
                .player(player.id)
                .cards_drawn
        })
        .collect();

    engine
        .apply_scenario_command(2, &concede())
        .expect("P2's departure returns its exiled Aura during Packleader's choice");
    assert!(engine.state.players[2].has_lost);
    let aura_return = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("P1 receives the interactive Aura return");
    assert_eq!(aura_return.deciding_player, 1);
    assert_eq!(
        aura_return.presentation.choice_kind,
        ChoiceKind::AuraPermanent
    );
    assert!(aura_return
        .presentation
        .candidates
        .contains(&eligible_creature));
    let observer_state = engine
        .diagnostic_snapshot()
        .expect("server-only observer continuation state");
    let suspended =
        &observer_state["state"]["pending_observer_return_batch"]["suspended_resolution"];
    assert_eq!(suspended["deciding_player"], 0);
    assert_eq!(
        suspended["continuation"]["variant"],
        "OptionalTriggeredAbility"
    );
    assert_eq!(
        suspended["continuation"]["value"]["stack"]["item"]["id"],
        root_id
    );

    engine
        .apply_scenario_command(0, &concede())
        .expect("P0 may concede while P1 chooses the Aura recipient");
    assert!(engine.state.players[0].has_lost);
    assert!(!engine.state.players[1].has_lost && !engine.state.players[3].has_lost);
    assert!(engine.state.outcome.is_none());

    let aura_completed = engine
        .apply_scenario_command(1, &submit_resolution_choice(vec![eligible_creature]))
        .expect("P1 completes the valid Aura return");
    assert_eq!(engine.state.objects[&aura].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&aura].controller, 1);
    assert_eq!(
        engine.state.objects[&aura].attached_to,
        Some(tricerules_core::AttachmentRecipient::Object(
            eligible_creature
        ))
    );
    assert!(engine.state.active_event_observers.is_empty());

    let restored = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("the Packleader optional trigger is restored after AuraReturn");
    let ResolutionContinuation::OptionalTriggeredAbility { stack, .. } = &restored.continuation
    else {
        panic!("the exact optional trigger remains the suspended root");
    };
    assert_eq!(stack.item.id, root_id);
    assert_eq!(stack.item.controller, 0);
    assert_eq!(stack.item.source_permanent_id, original_source);
    assert_eq!(stack.item.source_zone_change, original_source_generation);
    assert_eq!(stack.item.targets, original_targets);
    let restored_offer = find_resolution_choice(&aura_completed);
    assert!(
        !engine.state.players[engine.state.player_idx(restored.deciding_player).unwrap()].has_lost
            && restored_offer.as_ref().is_some_and(|offer| {
                offer.deciding_player_id == restored.deciding_player
                    && offer.choice_kind() == ChoiceKind::ResolutionBranch
            }),
        "after the valid AuraReturn, the restored optional root must have a live chooser and a published offer; got departed chooser P{} and offer={}",
        restored.deciding_player,
        restored_offer.is_some()
    );

    // The rest of this acceptance path exercises the delegated chooser. Before the restoration
    // refresh is added, the assertion above fails with P0 still holding an unpublished prompt.
    let selector = restored.deciding_player;
    assert_eq!(selector, 1, "the next survivor after P0 selects a delegate");
    let selector_prompt_id = restored.resolution_prompt_id.get();
    assert_ne!(selector_prompt_id, initial_prompt_id);
    let original_draws: Vec<_> = engine
        .state
        .players
        .iter()
        .map(|player| {
            engine
                .state
                .turn_history
                .current
                .player(player.id)
                .cards_drawn
        })
        .collect();

    let stale_command = submit_resolution_decision(ResolutionChoiceDecision::SelectBranch);
    assert!(
        engine.apply_scenario_command(0, &stale_command).is_err(),
        "the departed controller cannot answer the restored selector prompt"
    );
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .resolution_prompt_id
            .get(),
        selector_prompt_id,
        "rejected stale actor leaves the published prompt unchanged"
    );

    engine
        .apply_scenario_command(selector, &select_branch(1))
        .expect("P1 selects the distinct surviving delegate P3");
    let delegated = engine.state.pending_resolution.as_ref().unwrap();
    assert_eq!(delegated.deciding_player, 3);
    assert_ne!(delegated.resolution_prompt_id.get(), selector_prompt_id);
    let delegate_prompt_id = delegated.resolution_prompt_id.get();
    let before_stale = engine.diagnostic_snapshot().unwrap();
    let mut stale_prompt = submit_resolution_decision(ResolutionChoiceDecision::SelectBranch);
    if let Some(Cmd::SubmitResolutionChoice(answer)) = stale_prompt.cmd.as_mut() {
        answer.resolution_prompt_id = selector_prompt_id;
    }
    assert!(
        engine.apply_scenario_command(3, &stale_prompt).is_err(),
        "P3's new delegate prompt rejects the stale selector prompt ID"
    );
    assert_eq!(engine.diagnostic_snapshot().unwrap(), before_stale);
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .resolution_prompt_id
            .get(),
        delegate_prompt_id
    );

    let applied = engine
        .apply_scenario_command(
            3,
            &submit_resolution_decision(ResolutionChoiceDecision::SelectBranch),
        )
        .expect("P3 applies the original controller's optional draw");
    assert_eq!(stack_resolved_count(&applied, root_id), 1);
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
    for player in &engine.state.players {
        assert_eq!(
            engine
                .state
                .turn_history
                .current
                .player(player.id)
                .cards_drawn,
            draws_before[engine.state.player_idx(player.id).unwrap()],
            "Packleader's draw is never redirected to a survivor"
        );
        assert_eq!(
            engine
                .state
                .turn_history
                .current
                .player(player.id)
                .cards_drawn,
            original_draws[engine.state.player_idx(player.id).unwrap()]
        );
    }
}

#[test]
fn daggerfang_duo_fixed_mill_controller_concession_preserves_the_original_recipient() {
    for (seed, accept) in [(2026_1022, true), (2026_1025, false)] {
        run_daggerfang_controller_concession(seed, accept);
    }
}

fn run_daggerfang_controller_concession(seed: u64, accept: bool) {
    let mut engine = three_player_engine(seed);
    let daggerfang = inject_card_into_hand(&mut engine, 0, "daggerfang_duo");
    grant_pool(&mut engine, 0);
    let slot = hand_index_for_card(&engine, 0, "daggerfang_duo");
    engine
        .apply_scenario_command(0, &cast_spell(slot, vec![]))
        .expect("cast actual Daggerfang Duo");
    pass_priority_round(&mut engine);
    assert_eq!(engine.state.objects[&daggerfang].zone, Zone::Battlefield);
    pass_priority_round(&mut engine);

    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("Daggerfang's real optional Mill trigger begins resolving");
    let ResolutionContinuation::OptionalTriggeredAbility { stack, .. } = &pending.continuation
    else {
        panic!("the fixed Mill profile uses the optional-trigger continuation");
    };
    let original = stack.item.clone();
    assert_eq!(stack.item.controller, 0);
    assert_eq!(stack.item.source_permanent_id, Some(daggerfang));
    let survivor_libraries: Vec<_> = engine.state.players[1..]
        .iter()
        .map(|player| player.library.len())
        .collect();

    let departure = engine
        .apply_scenario_command(0, &concede())
        .expect("the Mill trigger controller may concede during its choice");
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("the fixed Mill choice remains available after controller departure");
    assert!(matches!(
        &pending.continuation,
        ResolutionContinuation::OptionalTriggeredAbility { .. }
    ));
    let ResolutionContinuation::OptionalTriggeredAbility { stack, .. } = &pending.continuation
    else {
        unreachable!("checked above");
    };
    let delegated_root = stack.item.clone();
    assert_eq!(delegated_root.id, original.id);
    assert_eq!(delegated_root.controller, original.controller);
    assert_eq!(delegated_root.source_owner, original.source_owner);
    assert_eq!(stack.item.source_permanent_id, Some(daggerfang));
    assert_eq!(stack.item.source_zone_change, original.source_zone_change);
    assert_eq!(stack.item.targets, original.targets);
    assert!(find_resolution_choice(&departure).is_some());
    assert_eq!(
        engine.state.players[1..]
            .iter()
            .map(|player| player.library.len())
            .collect::<Vec<_>>(),
        survivor_libraries,
        "a departed recipient never redirects Mill to a survivor"
    );
    let selector = pending.deciding_player;
    engine
        .apply_scenario_command(selector, &select_branch(0))
        .expect("a survivor selects the delegate for Daggerfang's Mill");
    let delegate = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("the delegate receives the original fixed Mill")
        .deciding_player;
    let completed = engine
        .apply_scenario_command(
            delegate,
            &submit_resolution_decision(if accept {
                ResolutionChoiceDecision::SelectBranch
            } else {
                ResolutionChoiceDecision::Decline
            }),
        )
        .expect("the delegate may apply or decline the fixed Mill");
    assert_eq!(stack_resolved_count(&completed, original.id), 1);
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
    assert_eq!(
        engine.state.players[1..]
            .iter()
            .map(|player| player.library.len())
            .collect::<Vec<_>>(),
        survivor_libraries,
        "delegated Mill never moves cards from a surviving player's library"
    );
}
