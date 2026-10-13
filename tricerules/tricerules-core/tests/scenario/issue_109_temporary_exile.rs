use crate::helpers::*;
use tricerules_proto::ruled::v1::{
    dev_command, ruled_command::Cmd, ChoiceKind, ChooseTriggerTarget, DevCommand, DevMoveCard,
    DevZone, RuledCommand, SubmitResolutionChoice,
};

fn choose_trigger_target(object_id: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
            decline: false,
            selected_modes: Vec::new(),
            targets: target_object(object_id),
        })),
    }
}

fn setup_banishing_light() -> (GameEngine, u32) {
    let decks = Some(vec![
        deck_with("plains", &["banishing_light"]),
        deck_with("forest", &["broken_wings"]),
    ]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        10901,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("Issue #109 cards must validate");
    advance_to_main1_from_game_start(&mut engine);
    let target = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    ensure_card_in_hand(&mut engine, 0, "banishing_light");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            w: 1,
            c: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "banishing_light");
    engine
        .apply_command(0, &cast_spell(slot, Vec::new()))
        .expect("cast Banishing Light");
    pass_both_players(&mut engine);
    assert_eq!(engine.state.pending_triggers.len(), 1);
    engine
        .apply_command(0, &choose_trigger_target(target))
        .expect("choose Banishing Light target");
    (engine, target)
}

fn banishing_light_id(engine: &GameEngine) -> u32 {
    engine
        .state
        .objects
        .values()
        .find(|object| {
            object.card_id == "banishing_light" && object.zone == tricerules_core::Zone::Battlefield
        })
        .expect("Banishing Light on battlefield")
        .id
}

fn cast_broken_wings(engine: &mut GameEngine, source: u32) {
    ensure_card_in_hand(engine, 1, "broken_wings");
    give_mana(
        engine,
        1,
        ManaGift {
            g: 1,
            c: 2,
            ..Default::default()
        },
    );
    engine
        .apply_command(0, &pass())
        .expect("P0 passes priority");
    let slot = hand_index_for_card(engine, 1, "broken_wings");
    engine
        .apply_command(1, &cast_spell(slot, target_object(source)))
        .expect("cast Broken Wings");
}

fn dev_move(engine: &mut GameEngine, target_player: i32, card_name: &str, zone: DevZone) {
    engine
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::DevCommand(DevCommand {
                    target_player_id: target_player,
                    dev: Some(dev_command::Dev::MoveCard(DevMoveCard {
                        card_name: card_name.into(),
                        zone: zone as i32,
                        ready: true,
                    })),
                })),
            },
        )
        .expect("dev move");
}

#[test]
fn banishing_light_exiles_the_exact_target_generation() {
    let (mut engine, target) = setup_banishing_light();
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        engine.state.objects[&target].zone,
        tricerules_core::Zone::Exile
    );
    assert_eq!(engine.state.active_event_observers.len(), 1);
}

#[test]
fn source_leaving_returns_the_card_under_its_owners_control() {
    let (mut engine, target) = setup_banishing_light();
    resolve_entire_stack_two_player(&mut engine);
    let source = banishing_light_id(&engine);

    cast_broken_wings(&mut engine, source);
    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(
        engine.state.objects[&source].zone,
        tricerules_core::Zone::Graveyard
    );
    assert_eq!(
        engine.state.objects[&target].zone,
        tricerules_core::Zone::Battlefield
    );
    assert_eq!(engine.state.objects[&target].controller, 1);
    assert!(engine.state.active_event_observers.is_empty());
}

#[test]
fn source_leaving_before_the_etb_trigger_resolves_exiles_nothing() {
    let (mut engine, target) = setup_banishing_light();
    let source = banishing_light_id(&engine);
    cast_broken_wings(&mut engine, source);
    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(
        engine.state.objects[&source].zone,
        tricerules_core::Zone::Graveyard
    );
    assert_eq!(
        engine.state.objects[&target].zone,
        tricerules_core::Zone::Battlefield
    );
    assert!(engine.state.active_event_observers.is_empty());
}

#[test]
fn returning_aura_owner_chooses_a_legal_permanent_to_enchant() {
    let decks = Some(vec![
        deck_with("plains", &["banishing_light"]),
        deck_with("forest", &["broken_wings"]),
    ]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        10902,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("new");
    advance_to_main1_from_game_start(&mut engine);
    let first_creature = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let chosen_creature = inject_creature_on_battlefield(&mut engine, 1, "hill_giant");
    let aura = inject_permanent_on_battlefield(&mut engine, 1, "capture_sphere");
    engine.state.objects.get_mut(&aura).unwrap().attached_to =
        Some(tricerules_core::AttachmentRecipient::Object(first_creature));
    ensure_card_in_hand(&mut engine, 0, "banishing_light");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            w: 1,
            c: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "banishing_light");
    engine
        .apply_command(0, &cast_spell(slot, Vec::new()))
        .expect("cast Banishing Light");
    pass_both_players(&mut engine);
    engine
        .apply_command(0, &choose_trigger_target(aura))
        .expect("target Aura");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        engine.state.objects[&aura].zone,
        tricerules_core::Zone::Exile
    );

    let source = banishing_light_id(&engine);
    cast_broken_wings(&mut engine, source);
    pass_both_players(&mut engine);
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("Aura recipient choice");
    assert_eq!(pending.deciding_player, 1, "the Aura's owner chooses");
    assert_eq!(pending.presentation.choice_kind, ChoiceKind::AuraPermanent);
    assert!(pending.presentation.candidates.contains(&chosen_creature));

    engine
        .apply_command(
            1,
            &RuledCommand {
                cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
                    chosen_object_ids: vec![chosen_creature],
                    ..Default::default()
                })),
            },
        )
        .expect("choose Aura recipient");

    assert_eq!(
        engine.state.objects[&aura].zone,
        tricerules_core::Zone::Battlefield
    );
    assert_eq!(
        engine.state.objects[&aura].attached_to,
        Some(tricerules_core::AttachmentRecipient::Object(
            chosen_creature
        ))
    );
}

#[test]
fn exiled_card_that_moves_and_returns_is_not_the_linked_generation() {
    let (mut engine, target) = setup_banishing_light();
    resolve_entire_stack_two_player(&mut engine);
    let linked_generation = engine.state.zone_change_generation[&target];
    engine.enable_dev_commands();
    dev_move(&mut engine, 1, "Grizzly Bears", DevZone::Hand);
    dev_move(&mut engine, 1, "Grizzly Bears", DevZone::Battlefield);
    assert!(engine.state.zone_change_generation[&target] > linked_generation);

    let source = banishing_light_id(&engine);
    cast_broken_wings(&mut engine, source);
    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(
        engine.state.objects[&target].zone,
        tricerules_core::Zone::Battlefield
    );
    assert!(engine.state.active_event_observers.is_empty());
}

#[test]
fn exiled_token_ceases_to_exist_and_never_returns() {
    let decks = Some(vec![
        deck_with("plains", &["banishing_light"]),
        deck_with("forest", &["broken_wings"]),
    ]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        10903,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("new");
    advance_to_main1_from_game_start(&mut engine);
    let token = inject_creature_on_battlefield(&mut engine, 1, "soldier_w_1_1");
    ensure_card_in_hand(&mut engine, 0, "banishing_light");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            w: 1,
            c: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "banishing_light");
    engine
        .apply_command(0, &cast_spell(slot, Vec::new()))
        .expect("cast Banishing Light");
    pass_both_players(&mut engine);
    engine
        .apply_command(0, &choose_trigger_target(token))
        .expect("target token");
    resolve_entire_stack_two_player(&mut engine);
    assert!(!engine.state.objects.contains_key(&token));

    let source = banishing_light_id(&engine);
    cast_broken_wings(&mut engine, source);
    resolve_entire_stack_two_player(&mut engine);
    assert!(!engine.state.objects.contains_key(&token));
    assert!(engine.state.active_event_observers.is_empty());
}

#[test]
fn second_departure_does_not_restore_departed_brainstorm_choice_after_aura_return() {
    let decks = Some(vec![vec!["forest".into(); 30]; 4]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        10904,
        &[0, 1, 2, 3],
        20,
        decks,
        false,
    )
    .expect("four-player opening");
    let chooser = engine.state.opening.as_ref().unwrap().chooser;
    engine
        .apply_command(
            chooser,
            &RuledCommand {
                cmd: Some(Cmd::ChooseStartingPlayer(
                    tricerules_proto::ruled::v1::ChooseStartingPlayer {
                        starting_player_id: 2,
                    },
                )),
            },
        )
        .expect("choose P2 to start");
    while let Some(actor) = engine
        .state
        .opening
        .as_ref()
        .and_then(|opening| opening.mulligan_actor)
    {
        engine
            .apply_command(
                actor,
                &RuledCommand {
                    cmd: Some(Cmd::Mulligan(
                        tricerules_proto::ruled::v1::MulliganDecision { keep: true },
                    )),
                },
            )
            .expect("keep opening hand");
    }
    for _ in 0..2 {
        pass_priority_round(&mut engine);
    }
    assert_eq!(engine.state.active_player_id(), 2);
    assert_eq!(engine.state.turn_step, tricerules_core::TurnStep::Main1);

    let original_creature = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let eligible_creature = inject_creature_on_battlefield(&mut engine, 1, "hill_giant");
    let pacifism = inject_permanent_on_battlefield(&mut engine, 1, "pacifism");
    engine.state.objects.get_mut(&pacifism).unwrap().attached_to = Some(
        tricerules_core::AttachmentRecipient::Object(original_creature),
    );

    let banishing_light = inject_card_into_hand(&mut engine, 2, "banishing_light");
    give_mana(
        &mut engine,
        2,
        ManaGift {
            w: 1,
            c: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 2, "banishing_light");
    engine
        .apply_command(2, &cast_spell(slot, Vec::new()))
        .expect("cast Banishing Light");
    pass_priority_round(&mut engine);
    assert_eq!(engine.state.pending_triggers.len(), 1);
    engine
        .apply_command(2, &choose_trigger_target(pacifism))
        .expect("target P1's Pacifism");
    pass_priority_round(&mut engine);
    assert_eq!(
        engine.state.objects[&pacifism].zone,
        tricerules_core::Zone::Exile
    );
    assert_eq!(engine.state.active_event_observers.len(), 1);

    while engine.state.priority_player_id() != 0 {
        let priority = engine.state.priority_player_id();
        engine
            .apply_command(priority, &pass())
            .expect("pass priority to P0");
    }
    let brainstorm = inject_card_into_hand(&mut engine, 0, "brainstorm");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "brainstorm");
    engine
        .apply_command(0, &cast_spell(slot, Vec::new()))
        .expect("cast Brainstorm");
    pass_priority_round(&mut engine);
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("Brainstorm's private ordered choice");
    assert_eq!(pending.deciding_player, 0);
    assert_eq!(pending.presentation.choice_kind, ChoiceKind::HandCards);

    engine.apply_command(2, &concede()).expect("P2 concedes");
    assert!(!engine.state.objects.contains_key(&banishing_light));
    let aura_prompt = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("P1 chooses where Pacifism returns");
    assert_eq!(aura_prompt.deciding_player, 1);
    assert_eq!(
        aura_prompt.presentation.choice_kind,
        ChoiceKind::AuraPermanent
    );
    let snapshot = engine
        .diagnostic_snapshot()
        .expect("server-only diagnostic state");
    let suspended = &snapshot["state"]["pending_observer_return_batch"]["suspended_resolution"];
    assert_eq!(suspended["deciding_player"], 0);
    assert_eq!(suspended["continuation"]["variant"], "HandChoice");
    let hand_choice = &suspended["continuation"]["value"];
    assert_eq!(hand_choice["stack"]["item"]["id"], brainstorm);
    assert_eq!(hand_choice["hand_choice"]["program_selection"], true);

    engine.apply_command(0, &concede()).expect("P0 concedes");
    assert!(!engine.state.objects.contains_key(&brainstorm));
    let live_prompt = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("Aura return remains the active choice");
    assert_eq!(live_prompt.deciding_player, 1);
    assert_eq!(
        live_prompt.presentation.choice_kind,
        ChoiceKind::AuraPermanent
    );

    engine
        .apply_command(1, &submit_resolution_choice(vec![eligible_creature]))
        .expect("attach returning Pacifism to P1's creature");

    assert_eq!(
        engine.state.objects[&pacifism].zone,
        tricerules_core::Zone::Battlefield
    );
    assert_eq!(engine.state.objects[&pacifism].controller, 1);
    assert_eq!(
        engine.state.objects[&pacifism].attached_to,
        Some(tricerules_core::AttachmentRecipient::Object(
            eligible_creature
        ))
    );
    assert!(engine.state.pending_resolution.is_none());
    assert!(
        engine.diagnostic_snapshot().unwrap()["state"]["pending_observer_return_batch"].is_null()
    );
    assert!(engine.state.stack.is_empty());
    assert!(engine.state.active_event_observers.is_empty());
    assert!(!engine.state.players[1].has_lost && !engine.state.players[3].has_lost);
    assert!(engine.state.players[0].has_lost && engine.state.players[2].has_lost);
    assert!(engine.state.winner().is_none());
}

#[test]
fn observer_return_republishes_suspended_brainstorm_hand_choice() {
    let decks = Some(vec![vec!["forest".into(); 30]; 4]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        10905,
        &[0, 1, 2, 3],
        20,
        decks,
        false,
    )
    .expect("four-player opening");
    let chooser = engine.state.opening.as_ref().unwrap().chooser;
    engine
        .apply_command(
            chooser,
            &RuledCommand {
                cmd: Some(Cmd::ChooseStartingPlayer(
                    tricerules_proto::ruled::v1::ChooseStartingPlayer {
                        starting_player_id: 2,
                    },
                )),
            },
        )
        .expect("choose P2 to start");
    while let Some(actor) = engine
        .state
        .opening
        .as_ref()
        .and_then(|opening| opening.mulligan_actor)
    {
        engine
            .apply_command(
                actor,
                &RuledCommand {
                    cmd: Some(Cmd::Mulligan(
                        tricerules_proto::ruled::v1::MulliganDecision { keep: true },
                    )),
                },
            )
            .expect("keep opening hand");
    }
    for _ in 0..2 {
        pass_priority_round(&mut engine);
    }
    assert_eq!(engine.state.active_player_id(), 2);
    assert_eq!(engine.state.turn_step, tricerules_core::TurnStep::Main1);

    let original_creature = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let eligible_creature = inject_creature_on_battlefield(&mut engine, 1, "hill_giant");
    let pacifism = inject_permanent_on_battlefield(&mut engine, 1, "pacifism");
    engine.state.objects.get_mut(&pacifism).unwrap().attached_to = Some(
        tricerules_core::AttachmentRecipient::Object(original_creature),
    );

    let banishing_light = inject_card_into_hand(&mut engine, 2, "banishing_light");
    give_mana(
        &mut engine,
        2,
        ManaGift {
            w: 1,
            c: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 2, "banishing_light");
    engine
        .apply_command(2, &cast_spell(slot, Vec::new()))
        .expect("cast Banishing Light");
    pass_priority_round(&mut engine);
    assert_eq!(engine.state.pending_triggers.len(), 1);
    engine
        .apply_command(2, &choose_trigger_target(pacifism))
        .expect("target P1's Pacifism");
    pass_priority_round(&mut engine);
    assert_eq!(
        engine.state.objects[&pacifism].zone,
        tricerules_core::Zone::Exile
    );
    assert_eq!(engine.state.active_event_observers.len(), 1);

    while engine.state.priority_player_id() != 0 {
        let priority = engine.state.priority_player_id();
        engine
            .apply_command(priority, &pass())
            .expect("pass priority to P0");
    }
    let brainstorm = inject_card_into_hand(&mut engine, 0, "brainstorm");
    let draws_before = engine.state.turn_history.current.player(0).cards_drawn;
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "brainstorm");
    engine
        .apply_command(0, &cast_spell(slot, Vec::new()))
        .expect("cast Brainstorm");
    pass_priority_round(&mut engine);
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("Brainstorm's private ordered choice");
    assert_eq!(pending.deciding_player, 0);
    assert_eq!(pending.presentation.choice_kind, ChoiceKind::HandCards);
    let stack_generation = engine.state.zone_change_generation[&brainstorm];

    engine.apply_command(2, &concede()).expect("P2 concedes");
    assert!(!engine.state.objects.contains_key(&banishing_light));
    let aura_prompt = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("P1 chooses where Pacifism returns");
    assert_eq!(aura_prompt.deciding_player, 1);
    assert_eq!(
        aura_prompt.presentation.choice_kind,
        ChoiceKind::AuraPermanent
    );
    let snapshot = engine
        .diagnostic_snapshot()
        .expect("server-only diagnostic state");
    let suspended = &snapshot["state"]["pending_observer_return_batch"]["suspended_resolution"];
    assert_eq!(suspended["deciding_player"], 0);
    assert_eq!(suspended["continuation"]["variant"], "HandChoice");
    let hand_choice = &suspended["continuation"]["value"];
    assert_eq!(hand_choice["stack"]["item"]["id"], brainstorm);
    assert_eq!(hand_choice["hand_choice"]["program_selection"], true);

    let aura_completed = engine
        .apply_command(1, &submit_resolution_choice(vec![eligible_creature]))
        .expect("attach returning Pacifism to P1's creature");
    assert_eq!(
        engine.state.objects[&pacifism].zone,
        tricerules_core::Zone::Battlefield
    );
    assert_eq!(engine.state.objects[&pacifism].controller, 1);
    assert_eq!(
        engine.state.objects[&pacifism].attached_to,
        Some(tricerules_core::AttachmentRecipient::Object(
            eligible_creature
        ))
    );

    let live_prompt = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("Brainstorm's private choice is restored");
    assert_eq!(live_prompt.deciding_player, 0);
    assert_eq!(live_prompt.presentation.choice_kind, ChoiceKind::HandCards);
    assert_eq!(live_prompt.presentation.source_object_id, brainstorm);
    assert_eq!(
        live_prompt.presentation.candidates,
        engine.state.players[0].hand
    );
    let restored_candidates = live_prompt.presentation.candidates.clone();
    let ResolutionContinuation::HandChoice { stack, hand_choice } = &live_prompt.continuation
    else {
        panic!("the restored prompt retains its ordinary private hand-choice continuation");
    };
    assert_eq!(stack.item.id, brainstorm);
    assert!(hand_choice.program_selection);
    assert_eq!(hand_choice.affected_player, 0);
    assert_eq!(
        hand_choice.candidate_generations.len(),
        engine.state.players[0].hand.len()
    );
    for (object_id, generation) in &hand_choice.candidate_generations {
        assert!(engine.state.players[0].hand.contains(object_id));
        assert_eq!(
            engine
                .state
                .zone_change_generation
                .get(object_id)
                .copied()
                .unwrap_or(0),
            *generation,
            "the restored private offer binds each current hand incarnation"
        );
    }

    let prompts: Vec<_> = aura_completed
        .events
        .iter()
        .filter_map(|event| match &event.ev {
            Some(tricerules_proto::ruled::v1::ruled_event::Ev::ResolutionChoiceRequired(
                choice,
            )) if choice.choice_kind == ChoiceKind::HandCards as i32 => Some(choice),
            _ => None,
        })
        .collect();
    assert_eq!(prompts.len(), 1, "the resumed prompt is published once");
    let prompt = prompts[0];
    assert_eq!(prompt.deciding_player_id, 0);
    assert_eq!(prompt.source_object_id, brainstorm);
    assert_eq!(prompt.candidate_object_ids, engine.state.players[0].hand);
    assert!(prompt.ordered);
    assert_eq!((prompt.min, prompt.max), (2, 2));

    let selected = restored_candidates[..2].to_vec();
    engine
        .apply_command(0, &submit_resolution_choice(selected.clone()))
        .expect("finish Brainstorm's restored private choice");
    assert_eq!(engine.state.players[0].library[0], selected[1]);
    assert_eq!(engine.state.players[0].library[1], selected[0]);
    assert_eq!(
        engine.state.turn_history.current.player(0).cards_drawn,
        draws_before + 3,
        "restoring the prompt does not repeat Brainstorm's draw"
    );
    assert_eq!(
        engine.state.objects[&brainstorm].zone,
        tricerules_core::Zone::Graveyard
    );
    assert_eq!(
        engine.state.zone_change_generation[&brainstorm],
        stack_generation + 1
    );
    assert_eq!(count_card_id_in_graveyard(&engine, 0, "brainstorm"), 1);
    assert!(engine.state.stack.is_empty());
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.active_event_observers.is_empty());
    assert!(!engine.state.players[0].has_lost && !engine.state.players[1].has_lost);
    assert!(engine.state.players[2].has_lost && !engine.state.players[3].has_lost);
}

#[test]
fn observer_return_republishes_suspended_looting_discard_choice() {
    let decks = Some(vec![vec!["forest".into(); 30]; 4]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        10906,
        &[0, 1, 2, 3],
        20,
        decks,
        false,
    )
    .expect("four-player opening");
    let chooser = engine.state.opening.as_ref().unwrap().chooser;
    engine
        .apply_command(
            chooser,
            &RuledCommand {
                cmd: Some(Cmd::ChooseStartingPlayer(
                    tricerules_proto::ruled::v1::ChooseStartingPlayer {
                        starting_player_id: 2,
                    },
                )),
            },
        )
        .expect("choose P2 to start");
    while let Some(actor) = engine
        .state
        .opening
        .as_ref()
        .and_then(|opening| opening.mulligan_actor)
    {
        engine
            .apply_command(
                actor,
                &RuledCommand {
                    cmd: Some(Cmd::Mulligan(
                        tricerules_proto::ruled::v1::MulliganDecision { keep: true },
                    )),
                },
            )
            .expect("keep opening hand");
    }
    for _ in 0..2 {
        pass_priority_round(&mut engine);
    }
    assert_eq!(engine.state.active_player_id(), 2);
    assert_eq!(engine.state.turn_step, tricerules_core::TurnStep::Main1);

    let original_creature = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let eligible_creature = inject_creature_on_battlefield(&mut engine, 1, "hill_giant");
    let pacifism = inject_permanent_on_battlefield(&mut engine, 1, "pacifism");
    engine.state.objects.get_mut(&pacifism).unwrap().attached_to = Some(
        tricerules_core::AttachmentRecipient::Object(original_creature),
    );

    let banishing_light = inject_card_into_hand(&mut engine, 2, "banishing_light");
    give_mana(
        &mut engine,
        2,
        ManaGift {
            w: 1,
            c: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 2, "banishing_light");
    engine
        .apply_command(2, &cast_spell(slot, Vec::new()))
        .expect("cast Banishing Light");
    pass_priority_round(&mut engine);
    assert_eq!(engine.state.pending_triggers.len(), 1);
    engine
        .apply_command(2, &choose_trigger_target(pacifism))
        .expect("target P1's Pacifism");
    pass_priority_round(&mut engine);
    assert_eq!(
        engine.state.objects[&pacifism].zone,
        tricerules_core::Zone::Exile
    );
    assert_eq!(engine.state.active_event_observers.len(), 1);

    for _ in 0..120 {
        if engine.state.active_player_id() == 0
            && engine.state.turn_step == tricerules_core::TurnStep::Main1
        {
            break;
        }
        if engine.state.cleanup_discard_player.is_some() {
            resolve_cleanup_discards_if_any(&mut engine);
        } else {
            let priority = engine.state.priority_player_id();
            engine
                .apply_command(priority, &pass())
                .expect("advance priority toward P0 Main1");
        }
    }
    assert_eq!(engine.state.active_player_id(), 0);
    assert_eq!(engine.state.turn_step, tricerules_core::TurnStep::Main1);

    let looting = inject_card_into_hand(&mut engine, 0, "faithless_looting");
    let draws_before = engine.state.turn_history.current.player(0).cards_drawn;
    give_mana(
        &mut engine,
        0,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "faithless_looting");
    engine
        .apply_command(0, &cast_spell(slot, Vec::new()))
        .expect("cast Faithless Looting at sorcery timing");
    pass_priority_round(&mut engine);
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("Looting's private discard choice");
    assert_eq!(pending.deciding_player, 0);
    let ResolutionContinuation::PlayerSetDiscard { stack, discard } = &pending.continuation else {
        panic!("authored Discard parks its controller choice in PlayerSetDiscard");
    };
    assert_eq!(stack.item.id, looting);
    assert_eq!(discard.current, 0);
    assert!(discard.selections.is_empty());
    let initial_generations = discard.choices[0].candidate_generations.clone();
    let stack_generation = engine.state.zone_change_generation[&looting];

    engine.apply_command(2, &concede()).expect("P2 concedes");
    assert!(!engine.state.objects.contains_key(&banishing_light));
    let aura_prompt = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("P1 chooses where Pacifism returns");
    assert_eq!(aura_prompt.deciding_player, 1);
    assert_eq!(
        aura_prompt.presentation.choice_kind,
        ChoiceKind::AuraPermanent
    );
    let snapshot = engine
        .diagnostic_snapshot()
        .expect("server-only diagnostic state");
    let suspended = &snapshot["state"]["pending_observer_return_batch"]["suspended_resolution"];
    assert_eq!(suspended["deciding_player"], 0);
    assert_eq!(suspended["continuation"]["variant"], "PlayerSetDiscard");
    assert_eq!(
        suspended["continuation"]["value"]["stack"]["item"]["id"],
        looting
    );

    let aura_completed = engine
        .apply_command(1, &submit_resolution_choice(vec![eligible_creature]))
        .expect("attach returning Pacifism to P1's creature");
    assert_eq!(
        engine.state.objects[&pacifism].zone,
        tricerules_core::Zone::Battlefield
    );
    assert_eq!(engine.state.objects[&pacifism].controller, 1);
    assert_eq!(
        engine.state.objects[&pacifism].attached_to,
        Some(tricerules_core::AttachmentRecipient::Object(
            eligible_creature
        ))
    );

    let live_prompt = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("Looting's private choice is restored");
    assert_eq!(live_prompt.deciding_player, 0);
    assert_eq!(live_prompt.presentation.choice_kind, ChoiceKind::HandCards);
    assert_eq!(live_prompt.presentation.source_object_id, looting);
    assert_eq!(
        live_prompt.presentation.candidates,
        engine.state.players[0].hand
    );
    let ResolutionContinuation::PlayerSetDiscard { stack, discard } = &live_prompt.continuation
    else {
        panic!("the restored offer retains its player-set discard state");
    };
    assert_eq!(stack.item.id, looting);
    assert_eq!(discard.current, 0);
    assert!(discard.selections.is_empty());
    assert_eq!(discard.choices[0].player, 0);
    assert_eq!(
        discard.choices[0]
            .candidate_generations
            .iter()
            .map(|(object_id, _)| *object_id)
            .collect::<Vec<_>>(),
        engine.state.players[0].hand
    );
    for (object_id, generation) in &discard.choices[0].candidate_generations {
        assert_eq!(
            engine
                .state
                .zone_change_generation
                .get(object_id)
                .copied()
                .unwrap_or(0),
            *generation,
            "the refreshed discard offer binds current hand incarnations"
        );
    }

    let prompts: Vec<_> = aura_completed
        .events
        .iter()
        .filter_map(|event| match &event.ev {
            Some(tricerules_proto::ruled::v1::ruled_event::Ev::ResolutionChoiceRequired(
                choice,
            )) if choice.choice_kind == ChoiceKind::HandCards as i32 => Some(choice),
            _ => None,
        })
        .collect();
    assert_eq!(prompts.len(), 1, "the resumed prompt is published once");
    let prompt = prompts[0];
    assert_eq!(prompt.deciding_player_id, 0);
    assert_eq!(prompt.source_object_id, looting);
    assert_eq!(prompt.candidate_object_ids, engine.state.players[0].hand);
    assert_eq!(
        prompt.candidate_selectable.len(),
        engine.state.players[0].hand.len()
    );
    assert!(prompt
        .candidate_selectable
        .iter()
        .all(|selectable| *selectable));
    assert!(
        prompt.public_reveal.is_none(),
        "the discard prompt remains private"
    );
    assert_eq!((prompt.min, prompt.max), (2, 2));
    assert_eq!(
        discard.choices[0].candidate_generations, initial_generations,
        "restoration rebuilds the same live hand cohort and generations"
    );

    let selected = engine.state.players[0].hand[..2].to_vec();
    engine
        .apply_command(0, &submit_resolution_choice(selected.clone()))
        .expect("finish Looting's restored private choice");
    assert_eq!(
        engine.state.turn_history.current.player(0).cards_drawn,
        draws_before + 2,
        "restoring the discard prompt does not repeat Looting's draw"
    );
    assert!(selected
        .iter()
        .all(|object_id| engine.state.objects[object_id].zone == tricerules_core::Zone::Graveyard));
    assert_eq!(
        engine.state.objects[&looting].zone,
        tricerules_core::Zone::Graveyard
    );
    assert_eq!(
        engine.state.zone_change_generation[&looting],
        stack_generation + 1
    );
    assert_eq!(
        count_card_id_in_graveyard(&engine, 0, "faithless_looting"),
        1
    );
    assert!(engine.state.stack.is_empty());
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.active_event_observers.is_empty());
    assert!(!engine.state.players[0].has_lost && !engine.state.players[1].has_lost);
    assert!(engine.state.players[2].has_lost && !engine.state.players[3].has_lost);
}

#[test]
fn ordinary_return_runs_through_battlefield_entry_replacements() {
    let (mut engine, target) = setup_banishing_light();
    resolve_entire_stack_two_player(&mut engine);
    inject_permanent_on_battlefield(&mut engine, 0, "orb_of_dreams");
    let source = banishing_light_id(&engine);

    cast_broken_wings(&mut engine, source);
    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(
        engine.state.objects[&target].zone,
        tricerules_core::Zone::Battlefield
    );
    assert!(engine.state.objects[&target].tapped);
}

#[test]
fn returning_aura_with_no_legal_recipient_stays_in_exile() {
    let decks = Some(vec![
        deck_with("plains", &["banishing_light"]),
        deck_with("forest", &["broken_wings"]),
    ]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        10904,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("new");
    advance_to_main1_from_game_start(&mut engine);
    let first_creature = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let second_creature = inject_creature_on_battlefield(&mut engine, 1, "hill_giant");
    let aura = inject_permanent_on_battlefield(&mut engine, 1, "capture_sphere");
    engine.state.objects.get_mut(&aura).unwrap().attached_to =
        Some(tricerules_core::AttachmentRecipient::Object(first_creature));
    ensure_card_in_hand(&mut engine, 0, "banishing_light");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            w: 1,
            c: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "banishing_light");
    engine
        .apply_command(0, &cast_spell(slot, Vec::new()))
        .expect("cast Banishing Light");
    pass_both_players(&mut engine);
    engine
        .apply_command(0, &choose_trigger_target(aura))
        .expect("target Aura");
    resolve_entire_stack_two_player(&mut engine);

    engine.enable_dev_commands();
    dev_move(&mut engine, 1, "Grizzly Bears", DevZone::Graveyard);
    dev_move(&mut engine, 1, "Hill Giant", DevZone::Graveyard);
    assert_eq!(
        engine.state.objects[&first_creature].zone,
        tricerules_core::Zone::Graveyard
    );
    assert_eq!(
        engine.state.objects[&second_creature].zone,
        tricerules_core::Zone::Graveyard
    );

    let source = banishing_light_id(&engine);
    cast_broken_wings(&mut engine, source);
    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(
        engine.state.objects[&aura].zone,
        tricerules_core::Zone::Exile
    );
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.active_event_observers.is_empty());
}

#[test]
fn returning_player_aura_uses_the_typed_player_choice_surface() {
    let decks = Some(vec![
        deck_with("plains", &["banishing_light"]),
        deck_with("forest", &["broken_wings"]),
    ]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        10905,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("new");
    advance_to_main1_from_game_start(&mut engine);
    let aura = inject_permanent_on_battlefield(&mut engine, 1, "curse_of_disturbance");
    engine.state.objects.get_mut(&aura).unwrap().attached_to =
        Some(tricerules_core::AttachmentRecipient::Player(0));
    ensure_card_in_hand(&mut engine, 0, "banishing_light");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            w: 1,
            c: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "banishing_light");
    engine
        .apply_command(0, &cast_spell(slot, Vec::new()))
        .expect("cast Banishing Light");
    pass_both_players(&mut engine);
    engine
        .apply_command(0, &choose_trigger_target(aura))
        .expect("target player Aura");
    resolve_entire_stack_two_player(&mut engine);

    let source = banishing_light_id(&engine);
    cast_broken_wings(&mut engine, source);
    pass_both_players(&mut engine);
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("player Aura choice");
    assert_eq!(pending.deciding_player, 1);
    assert_eq!(pending.presentation.choice_kind, ChoiceKind::AuraPlayer);
    assert!(pending.presentation.candidates.contains(&0));

    engine
        .apply_command(
            1,
            &RuledCommand {
                cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
                    chosen_object_ids: vec![0],
                    ..Default::default()
                })),
            },
        )
        .expect("choose enchanted player");
    assert_eq!(
        engine.state.objects[&aura].zone,
        tricerules_core::Zone::Battlefield
    );
    assert_eq!(
        engine.state.objects[&aura].attached_to,
        Some(tricerules_core::AttachmentRecipient::Player(0))
    );
}

#[test]
fn two_sources_destroyed_together_return_both_linked_cards() {
    let decks = Some(vec![
        deck_with(
            "plains",
            &["banishing_light", "banishing_light", "tranquility"],
        ),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        10906,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("new");
    advance_to_main1_from_game_start(&mut engine);
    let first_target = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let second_target = inject_creature_on_battlefield(&mut engine, 1, "hill_giant");

    for target in [first_target, second_target] {
        ensure_card_in_hand(&mut engine, 0, "banishing_light");
        give_mana(
            &mut engine,
            0,
            ManaGift {
                w: 1,
                c: 2,
                ..Default::default()
            },
        );
        let slot = hand_index_for_card(&engine, 0, "banishing_light");
        engine
            .apply_command(0, &cast_spell(slot, Vec::new()))
            .expect("cast Banishing Light");
        pass_both_players(&mut engine);
        engine
            .apply_command(0, &choose_trigger_target(target))
            .expect("choose linked target");
        resolve_entire_stack_two_player(&mut engine);
    }
    assert_eq!(engine.state.active_event_observers.len(), 2);
    assert_eq!(
        engine.state.objects[&first_target].zone,
        tricerules_core::Zone::Exile
    );
    assert_eq!(
        engine.state.objects[&second_target].zone,
        tricerules_core::Zone::Exile
    );

    ensure_card_in_hand(&mut engine, 0, "tranquility");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            g: 1,
            c: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "tranquility");
    engine
        .apply_command(0, &cast_spell(slot, Vec::new()))
        .expect("cast Tranquility");
    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(
        engine.state.objects[&first_target].zone,
        tricerules_core::Zone::Battlefield
    );
    assert_eq!(
        engine.state.objects[&second_target].zone,
        tricerules_core::Zone::Battlefield
    );
    assert!(engine.state.active_event_observers.is_empty());
}
