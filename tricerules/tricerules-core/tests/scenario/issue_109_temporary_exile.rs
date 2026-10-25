use crate::helpers::ScenarioCommandExt;
use crate::helpers::*;
use tricerules_proto::ruled::v1::{
    dev_command, ruled_command::Cmd, CastCostGroupSelection, CastMethod, CastSpell, ChoiceKind,
    ChooseTriggerTarget, DevCommand, DevMoveCard, DevZone, ResolutionChoiceRequired, RuledCommand,
    SelectedSpellMode, SubmitResolutionChoice,
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

fn setup_three_player_observer_return(
    seed: u64,
    p0_cards: &[&str],
    p1_cards: &[&str],
    p2_cards: &[&str],
) -> (GameEngine, u32) {
    let decks = Some(vec![
        deck_with("forest", p0_cards),
        deck_with("forest", p1_cards),
        deck_with("plains", p2_cards),
    ]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1, 2],
        20,
        decks,
        false,
    )
    .expect("three-player opening starts");
    let opening_chooser = engine.state.opening.as_ref().unwrap().chooser;
    engine
        .apply_scenario_command(
            opening_chooser,
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

    let returned_creature = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    ensure_card_in_hand(&mut engine, 2, "banishing_light");
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
        .apply_scenario_command(2, &cast_spell(slot, Vec::new()))
        .expect("P2 casts Banishing Light");
    pass_priority_round(&mut engine);
    assert_eq!(engine.state.pending_triggers.len(), 1);
    engine
        .apply_scenario_command(2, &choose_trigger_target(returned_creature))
        .expect("Banishing Light targets P1's creature");
    pass_priority_round(&mut engine);
    assert_eq!(
        engine.state.objects[&returned_creature].zone,
        tricerules_core::Zone::Exile
    );

    for _ in 0..16 {
        if engine.state.active_player_id() == 0
            && engine.state.turn_step == tricerules_core::TurnStep::Main1
        {
            assert_eq!(engine.state.priority_player_id(), 0);
            return (engine, returned_creature);
        }
        pass_priority_round(&mut engine);
        resolve_cleanup_discards_if_any(&mut engine);
    }
    panic!("P2's turn advances to P0's first main phase");
}

fn mind_control_p2_creature(engine: &mut GameEngine, creature: u32) {
    ensure_card_in_hand(engine, 0, "mind_control");
    give_mana(
        engine,
        0,
        ManaGift {
            u: 2,
            c: 3,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(engine, 0, "mind_control");
    engine
        .apply_scenario_command(0, &cast_spell(slot, target_object(creature)))
        .expect("P0 casts Mind Control on P2's permanent");
    pass_priority_round(engine);
    assert_eq!(
        engine
            .characteristics(creature)
            .expect("enchanted creature remains on the battlefield")
            .controller,
        0
    );
}

fn assert_stale_prompt_rejected(
    engine: &mut GameEngine,
    chooser: i32,
    prompt_id: u64,
    otherwise_legal_candidate: u32,
) {
    let before = format!("{:?}", engine.state);
    let before_index = engine.state.command_index;
    let stale = RuledCommand {
        cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
            resolution_prompt_id: prompt_id,
            chosen_object_ids: vec![otherwise_legal_candidate],
            ..Default::default()
        })),
    };
    assert!(engine.apply_command(chooser, &stale).is_err());
    assert_eq!(format!("{:?}", engine.state), before);
    assert_eq!(engine.state.command_index, before_index);
}

fn concede_p2_and_refresh_offer(
    engine: &mut GameEngine,
    old_offer: &ResolutionChoiceRequired,
    removed_candidate: u32,
) -> ResolutionChoiceRequired {
    let batch = engine
        .apply_command(2, &concede())
        .expect("P2 concession returns its Banishing Light card and restores the parent choice");
    assert!(!engine.state.objects.contains_key(&removed_candidate));
    let offer = find_resolution_choice(&batch)
        .expect("restored choice is rebuilt and published in the concession batch")
        .clone();
    assert_ne!(offer.resolution_prompt_id, old_offer.resolution_prompt_id);
    assert!(!offer.candidate_object_ids.contains(&removed_candidate));
    assert!(batch.resolution_prompt_state.as_ref().is_some_and(
        |state| state.active && state.resolution_prompt_id == Some(offer.resolution_prompt_id)
    ));
    offer
}

fn pass_until_resolution_choice(engine: &mut GameEngine) -> ResolutionChoiceRequired {
    for _ in 0..8 {
        if engine.state.pending_resolution.is_some() {
            panic!("resolution choice was parked without its event in the captured pass batch");
        }
        answer_simultaneous_entry_order_in_engine_order(engine);
        answer_trigger_order_in_engine_order(engine);
        let count = engine
            .state
            .players
            .iter()
            .filter(|player| !player.has_lost)
            .count()
            .checked_sub(engine.state.passes_since_stack_change as usize)
            .expect("consistent priority pass count");
        for _ in 0..count {
            let actor = engine.state.priority_player_id();
            let batch = engine
                .apply_scenario_command(actor, &pass())
                .expect("pass priority while resolving fixture spell");
            if let Some(choice) = find_resolution_choice(&batch) {
                return choice.clone();
            }
            if engine.state.pending_resolution.is_some() {
                panic!("parked offer is present in state but missing from the emitted batch");
            }
        }
        answer_simultaneous_entry_order_in_engine_order(engine);
    }
    panic!("fixture spell or triggered ability produces its expected resolution choice");
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
        .apply_scenario_command(0, &cast_spell(slot, Vec::new()))
        .expect("cast Banishing Light");
    pass_both_players(&mut engine);
    assert_eq!(engine.state.pending_triggers.len(), 1);
    engine
        .apply_scenario_command(0, &choose_trigger_target(target))
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
        .apply_scenario_command(0, &pass())
        .expect("P0 passes priority");
    let slot = hand_index_for_card(engine, 1, "broken_wings");
    engine
        .apply_scenario_command(1, &cast_spell(slot, target_object(source)))
        .expect("cast Broken Wings");
}

fn dev_move(engine: &mut GameEngine, target_player: i32, card_name: &str, zone: DevZone) {
    engine
        .apply_scenario_command(
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
        .apply_scenario_command(0, &cast_spell(slot, Vec::new()))
        .expect("cast Banishing Light");
    pass_both_players(&mut engine);
    engine
        .apply_scenario_command(0, &choose_trigger_target(aura))
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
        .apply_scenario_command(
            1,
            &RuledCommand {
                cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
                    resolution_prompt_id: 0,
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
        .apply_scenario_command(0, &cast_spell(slot, Vec::new()))
        .expect("cast Banishing Light");
    pass_both_players(&mut engine);
    engine
        .apply_scenario_command(0, &choose_trigger_target(token))
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
        .expect("choose P2 to start");
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
        .apply_scenario_command(2, &cast_spell(slot, Vec::new()))
        .expect("cast Banishing Light");
    pass_priority_round(&mut engine);
    assert_eq!(engine.state.pending_triggers.len(), 1);
    engine
        .apply_scenario_command(2, &choose_trigger_target(pacifism))
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
            .apply_scenario_command(priority, &pass())
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
        .apply_scenario_command(0, &cast_spell(slot, Vec::new()))
        .expect("cast Brainstorm");
    pass_priority_round(&mut engine);
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("Brainstorm's private ordered choice");
    assert_eq!(pending.deciding_player, 0);
    assert_eq!(pending.presentation.choice_kind, ChoiceKind::HandCards);

    engine
        .apply_scenario_command(2, &concede())
        .expect("P2 concedes");
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

    engine
        .apply_scenario_command(0, &concede())
        .expect("P0 concedes");
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
        .apply_scenario_command(1, &submit_resolution_choice(vec![eligible_creature]))
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
        .expect("choose P2 to start");
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
        .apply_scenario_command(2, &cast_spell(slot, Vec::new()))
        .expect("cast Banishing Light");
    pass_priority_round(&mut engine);
    assert_eq!(engine.state.pending_triggers.len(), 1);
    engine
        .apply_scenario_command(2, &choose_trigger_target(pacifism))
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
            .apply_scenario_command(priority, &pass())
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
        .apply_scenario_command(0, &cast_spell(slot, Vec::new()))
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

    engine
        .apply_scenario_command(2, &concede())
        .expect("P2 concedes");
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
        .apply_scenario_command(1, &submit_resolution_choice(vec![eligible_creature]))
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
        .apply_scenario_command(0, &submit_resolution_choice(selected.clone()))
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
fn non_aura_observer_return_republishes_suspended_library_search() {
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
        .expect("choose P2 to start");
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
    for _ in 0..2 {
        pass_priority_round(&mut engine);
    }
    assert_eq!(engine.state.active_player_id(), 2);
    assert_eq!(engine.state.turn_step, tricerules_core::TurnStep::Main1);

    // P2's actual Banishing Light creates a source-bound return observer for a non-Aura
    // permanent. Its later concession must finish this one-shot return and resume P0's unrelated
    // Library search without inventing a child choice.
    let returned_creature = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
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
        .apply_scenario_command(2, &cast_spell(slot, Vec::new()))
        .expect("cast Banishing Light");
    pass_priority_round(&mut engine);
    assert_eq!(engine.state.pending_triggers.len(), 1);
    engine
        .apply_scenario_command(2, &choose_trigger_target(returned_creature))
        .expect("target P1's Grizzly Bears");
    pass_priority_round(&mut engine);
    assert_eq!(
        engine.state.objects[&returned_creature].zone,
        tricerules_core::Zone::Exile
    );
    assert_eq!(engine.state.active_event_observers.len(), 1);

    while engine.state.priority_player_id() != 0 {
        let priority = engine.state.priority_player_id();
        engine
            .apply_scenario_command(priority, &pass())
            .expect("pass priority to P0");
    }
    let tutor = inject_card_into_hand(&mut engine, 0, "mystical_tutor");
    inject_library_card(&mut engine, 0, "lightning_bolt");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "mystical_tutor");
    engine
        .apply_scenario_command(0, &cast_spell(slot, Vec::new()))
        .expect("cast Mystical Tutor");
    let mut initial_search_batch = None;
    while engine.state.pending_resolution.is_none() {
        let actor = engine.state.priority_player_id();
        let batch = engine
            .apply_scenario_command(actor, &pass())
            .expect("pass priority to resolve Mystical Tutor");
        if find_resolution_choice(&batch).is_some() {
            initial_search_batch = Some(batch);
        }
    }
    let initial_search_batch = initial_search_batch.expect("SearchLibrary choice was published");
    let mut original_offer =
        find_resolution_choice(&initial_search_batch).expect("Mystical Tutor search offer");
    assert_eq!(original_offer.deciding_player_id, 0);
    assert_eq!(original_offer.choice_kind(), ChoiceKind::LibrarySearch);
    assert!(!original_offer.candidate_object_ids.is_empty());
    let original_id = original_offer.resolution_prompt_id;
    assert_ne!(original_id, 0);

    let concession = engine
        .apply_command(2, &concede())
        .expect("unrelated concession completes the non-Aura observer return");
    assert!(!engine.state.objects.contains_key(&banishing_light));
    assert_eq!(
        engine.state.objects[&returned_creature].zone,
        tricerules_core::Zone::Battlefield
    );
    let resumed_offer = find_resolution_choice(&concession)
        .expect("suspended SearchLibrary offer must be republished");
    assert_eq!(resumed_offer.deciding_player_id, 0);
    assert_eq!(resumed_offer.choice_kind(), ChoiceKind::LibrarySearch);
    assert_ne!(resumed_offer.resolution_prompt_id, 0);
    assert_ne!(resumed_offer.resolution_prompt_id, original_id);
    original_offer.resolution_prompt_id = 0;
    let mut normalized_resumed_offer = resumed_offer.clone();
    normalized_resumed_offer.resolution_prompt_id = 0;
    assert_eq!(normalized_resumed_offer, original_offer);
    let prompt_state = concession
        .resolution_prompt_state
        .as_ref()
        .expect("concession batch has final prompt state");
    assert!(prompt_state.active);
    assert_eq!(prompt_state.deciding_player_id, 0);
    assert_eq!(
        prompt_state.resolution_prompt_id,
        Some(resumed_offer.resolution_prompt_id)
    );

    let chosen = resumed_offer.candidate_object_ids[0];
    let mut stale_answer = submit_resolution_choice(vec![chosen]);
    let Some(Cmd::SubmitResolutionChoice(answer)) = stale_answer.cmd.as_mut() else {
        unreachable!("choice helper returns a resolution answer")
    };
    answer.resolution_prompt_id = original_id;
    let before_stale_answer = format!("{:?}", engine.state);
    assert!(engine.apply_command(0, &stale_answer).is_err());
    assert_eq!(format!("{:?}", engine.state), before_stale_answer);

    let mut current_answer = submit_resolution_choice(vec![chosen]);
    let Some(Cmd::SubmitResolutionChoice(answer)) = current_answer.cmd.as_mut() else {
        unreachable!("choice helper returns a resolution answer")
    };
    answer.resolution_prompt_id = resumed_offer.resolution_prompt_id;
    engine
        .apply_command(0, &current_answer)
        .expect("current SearchLibrary offer remains answerable");
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine
        .diagnostic_snapshot()
        .expect("server diagnostic state")["state"]["pending_observer_return_batch"]
        .is_null());
    let final_snapshot = engine.initial_response_batch();
    assert!(
        !final_snapshot
            .resolution_prompt_state
            .expect("reconnect has final prompt status")
            .active
    );
    assert!(engine.state.objects.contains_key(&tutor));
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
        .expect("choose P2 to start");
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
        .apply_scenario_command(2, &cast_spell(slot, Vec::new()))
        .expect("cast Banishing Light");
    pass_priority_round(&mut engine);
    assert_eq!(engine.state.pending_triggers.len(), 1);
    engine
        .apply_scenario_command(2, &choose_trigger_target(pacifism))
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
                .apply_scenario_command(priority, &pass())
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
        .apply_scenario_command(0, &cast_spell(slot, Vec::new()))
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

    engine
        .apply_scenario_command(2, &concede())
        .expect("P2 concedes");
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
        .apply_scenario_command(1, &submit_resolution_choice(vec![eligible_creature]))
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
        .apply_scenario_command(0, &submit_resolution_choice(selected.clone()))
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
        .apply_scenario_command(0, &cast_spell(slot, Vec::new()))
        .expect("cast Banishing Light");
    pass_both_players(&mut engine);
    engine
        .apply_scenario_command(0, &choose_trigger_target(aura))
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
        .apply_scenario_command(0, &cast_spell(slot, Vec::new()))
        .expect("cast Banishing Light");
    pass_both_players(&mut engine);
    engine
        .apply_scenario_command(0, &choose_trigger_target(aura))
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
        .apply_scenario_command(
            1,
            &RuledCommand {
                cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
                    resolution_prompt_id: 0,
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
            .apply_scenario_command(0, &cast_spell(slot, Vec::new()))
            .expect("cast Banishing Light");
        pass_both_players(&mut engine);
        engine
            .apply_scenario_command(0, &choose_trigger_target(target))
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
        .apply_scenario_command(0, &cast_spell(slot, Vec::new()))
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

#[test]
fn observer_restore_rebuilds_program_permanent_choice_after_candidate_owner_concedes() {
    let decks = Some(vec![
        deck_with("forest", &["ray_of_command", "final_showdown"]),
        deck_with("forest", &["grizzly_bears"]),
        deck_with("forest", &["banishing_light"]),
    ]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        10907,
        &[0, 1, 2],
        20,
        decks,
        false,
    )
    .expect("three-player opening starts");
    let opening_chooser = engine.state.opening.as_ref().unwrap().chooser;
    engine
        .apply_scenario_command(
            opening_chooser,
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

    let stolen_creature = inject_creature_on_battlefield(&mut engine, 2, "hill_giant");
    let surviving_creature = inject_creature_on_battlefield(&mut engine, 0, "wind_drake");
    let another_surviving_creature =
        inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let returned_creature = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");

    ensure_card_in_hand(&mut engine, 2, "banishing_light");
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
        .apply_scenario_command(2, &cast_spell(slot, Vec::new()))
        .expect("P2 casts Banishing Light");
    pass_priority_round(&mut engine);
    assert_eq!(engine.state.pending_triggers.len(), 1);
    engine
        .apply_scenario_command(2, &choose_trigger_target(returned_creature))
        .expect("Banishing Light targets P1's creature");
    pass_priority_round(&mut engine);
    assert_eq!(
        engine.state.objects[&returned_creature].zone,
        tricerules_core::Zone::Exile
    );

    // On P2's turn, P0 uses the real instant Ray of Command to take P2's creature. This keeps
    // both that temporary-control effect and P2's source-bound Banishing Light return live while
    // Final Showdown parks its non-program permanent choice.
    engine
        .apply_scenario_command(2, &pass())
        .expect("P2 passes priority to P0");
    assert_eq!(engine.state.priority_player_id(), 0);
    ensure_card_in_hand(&mut engine, 0, "ray_of_command");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            c: 3,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "ray_of_command");
    engine
        .apply_scenario_command(0, &cast_spell(slot, target_object(stolen_creature)))
        .expect("P0 casts Ray of Command on P2's creature");
    pass_priority_round(&mut engine);
    assert_eq!(
        engine
            .characteristics(stolen_creature)
            .expect("stolen creature remains on the battlefield")
            .controller,
        0
    );
    engine
        .apply_scenario_command(2, &pass())
        .expect("P2 passes priority to P0 again");

    ensure_card_in_hand(&mut engine, 0, "final_showdown");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            w: 1,
            c: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "final_showdown");
    engine
        .apply_scenario_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::CastSpell(CastSpell {
                    cast_method: CastMethod::Normal as i32,
                    source: Some(hand_cast_source(slot)),
                    selected_modes: vec![SelectedSpellMode {
                        mode_index: 1,
                        targets: Vec::new(),
                    }],
                    cast_cost_group_selections: vec![CastCostGroupSelection {
                        repetitions: None,
                        group_index: 0,
                        option_index: 1,
                        selected_object: None,
                        expected_zone_change_generation: 0,
                        battlefield_objects: None,
                    }],
                    ..Default::default()
                })),
            },
        )
        .expect("P0 casts Final Showdown's indestructible mode");
    let mut original_offer = None;
    while engine.state.pending_resolution.is_none() {
        let actor = engine.state.priority_player_id();
        let batch = engine
            .apply_scenario_command(actor, &pass())
            .expect("resolve Final Showdown");
        if let Some(offer) = find_resolution_choice(&batch) {
            original_offer = Some(offer.clone());
        }
    }
    let original_offer = original_offer.expect("Final Showdown publishes its choice");
    assert_eq!(original_offer.choice_kind(), ChoiceKind::PermanentObjects);
    assert!(original_offer
        .candidate_object_ids
        .contains(&stolen_creature));
    assert!(original_offer
        .candidate_object_ids
        .contains(&surviving_creature));
    assert!(original_offer
        .candidate_object_ids
        .contains(&another_surviving_creature));
    assert_ne!(original_offer.resolution_prompt_id, 0);

    let concession = engine
        .apply_command(2, &concede())
        .expect("P2 concession returns its Banishing Light card and restores P0's choice");
    assert!(!engine.state.objects.contains_key(&stolen_creature));
    assert_eq!(
        engine.state.objects[&returned_creature].zone,
        tricerules_core::Zone::Battlefield
    );
    let prompt_state = concession
        .resolution_prompt_state
        .as_ref()
        .expect("concession batch publishes final prompt state");
    assert!(prompt_state.active);
    assert_eq!(prompt_state.deciding_player_id, 0);
    let resumed_prompt_id = prompt_state
        .resolution_prompt_id
        .expect("refreshed Program choice has an active prompt ID");
    assert_ne!(resumed_prompt_id, 0);
    let resumed_offer = concession
        .events
        .iter()
        .rev()
        .find_map(|event| match &event.ev {
            Some(Ev::ResolutionChoiceRequired(choice))
                if choice.resolution_prompt_id == resumed_prompt_id =>
            {
                Some(choice.clone())
            }
            _ => None,
        })
        .unwrap_or_else(|| {
            panic!(
                "restored Final Showdown choice is republished; pending={:#?}; events={:#?}",
                engine.state.pending_resolution, concession.events
            )
        });
    assert_ne!(
        resumed_offer.resolution_prompt_id,
        original_offer.resolution_prompt_id
    );
    let mut resumed_candidates = resumed_offer.candidate_object_ids.clone();
    resumed_candidates.sort_unstable();
    let mut expected_candidates = vec![surviving_creature, another_surviving_creature];
    expected_candidates.sort_unstable();
    assert_eq!(
        resumed_candidates, expected_candidates,
        "the offer is rebuilt from current legality rather than reusing a departed candidate"
    );
    let before_stale_answer = format!("{:?}", engine.state);
    let before_stale_answer_index = engine.state.command_index;
    let stale = RuledCommand {
        cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
            resolution_prompt_id: original_offer.resolution_prompt_id,
            chosen_object_ids: vec![surviving_creature],
            ..Default::default()
        })),
    };
    assert!(engine.apply_command(0, &stale).is_err());
    assert_eq!(
        format!("{:?}", engine.state),
        before_stale_answer,
        "the retired offer ID must not mutate the refreshed choice"
    );
    assert_eq!(engine.state.command_index, before_stale_answer_index);

    let before_invalid_candidate = format!("{:?}", engine.state);
    let before_invalid_candidate_index = engine.state.command_index;
    let invalid_candidate = RuledCommand {
        cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
            resolution_prompt_id: resumed_offer.resolution_prompt_id,
            chosen_object_ids: vec![stolen_creature],
            ..Default::default()
        })),
    };
    assert!(engine.apply_command(0, &invalid_candidate).is_err());
    assert_eq!(
        format!("{:?}", engine.state),
        before_invalid_candidate,
        "a current ID cannot submit a departed object absent from the refreshed offer"
    );
    assert_eq!(engine.state.command_index, before_invalid_candidate_index);

    let current = RuledCommand {
        cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
            resolution_prompt_id: resumed_offer.resolution_prompt_id,
            chosen_object_ids: vec![surviving_creature],
            ..Default::default()
        })),
    };
    engine
        .apply_command(0, &current)
        .expect("P0 answers the refreshed offer");
    assert!(engine
        .characteristics(surviving_creature)
        .expect("surviving creature")
        .has_keyword(tricerules_cards::Keyword::Indestructible));
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.players[2].has_lost);
}

#[test]
fn observer_restore_rebuilds_behold_choice_after_candidate_owner_concedes() {
    let decks = Some(vec![
        deck_with(
            "forest",
            &["elven_passage", "safewright_cavalry", "ray_of_command"],
        ),
        deck_with("forest", &["grizzly_bears"]),
        deck_with("plains", &["banishing_light", "safewright_cavalry"]),
    ]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        10908,
        &[0, 1, 2],
        20,
        decks,
        false,
    )
    .expect("three-player opening starts");
    let opening_chooser = engine.state.opening.as_ref().unwrap().chooser;
    engine
        .apply_scenario_command(
            opening_chooser,
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

    let stolen_elf = inject_creature_on_battlefield(&mut engine, 2, "safewright_cavalry");
    let surviving_elf = inject_creature_on_battlefield(&mut engine, 0, "safewright_cavalry");
    let returned_creature = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let passage = relocate_to_battlefield(&mut engine, 0, "elven_passage", false);

    ensure_card_in_hand(&mut engine, 2, "banishing_light");
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
        .apply_scenario_command(2, &cast_spell(slot, Vec::new()))
        .expect("P2 casts Banishing Light");
    pass_priority_round(&mut engine);
    assert_eq!(engine.state.pending_triggers.len(), 1);
    engine
        .apply_scenario_command(2, &choose_trigger_target(returned_creature))
        .expect("Banishing Light targets P1's creature");
    pass_priority_round(&mut engine);
    assert_eq!(
        engine.state.objects[&returned_creature].zone,
        tricerules_core::Zone::Exile
    );

    engine
        .apply_scenario_command(2, &pass())
        .expect("P2 passes priority to P0");
    assert_eq!(engine.state.priority_player_id(), 0);
    ensure_card_in_hand(&mut engine, 0, "ray_of_command");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            c: 3,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "ray_of_command");
    engine
        .apply_scenario_command(0, &cast_spell(slot, target_object(stolen_elf)))
        .expect("P0 casts Ray of Command on P2's Elf");
    pass_priority_round(&mut engine);
    assert_eq!(
        engine
            .characteristics(stolen_elf)
            .expect("stolen Elf remains on the battlefield")
            .controller,
        0
    );
    engine
        .apply_scenario_command(2, &pass())
        .expect("P2 passes priority to P0 again");

    engine
        .apply_scenario_command(0, &activate_ability(passage, 0, Vec::new()))
        .expect("P0 activates Elven Passage");
    pass_priority_round(&mut engine);
    let pending_search = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("Elven Passage searches its library");
    assert_eq!(
        pending_search.presentation.choice_kind,
        ChoiceKind::LibrarySearch
    );
    let found_land = pending_search.presentation.candidates[0];
    let behold_batch = engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![found_land]))
        .expect("find a basic land and reach Behold");
    let original_offer = find_resolution_choice(&behold_batch).expect("Behold offer");
    assert_eq!(original_offer.choice_kind(), ChoiceKind::Behold);
    assert_eq!(original_offer.deciding_player_id, 0);
    assert!(original_offer.candidate_object_ids.contains(&stolen_elf));
    assert!(original_offer.candidate_object_ids.contains(&surviving_elf));
    assert_ne!(original_offer.resolution_prompt_id, 0);

    let concession = engine
        .apply_command(2, &concede())
        .expect("P2 concession restores Elven Passage's Behold choice");
    assert!(!engine.state.objects.contains_key(&stolen_elf));
    assert_eq!(
        engine.state.objects[&returned_creature].zone,
        tricerules_core::Zone::Battlefield
    );
    let resumed_offer = find_resolution_choice(&concession).unwrap_or_else(|| {
        panic!(
            "restored Behold choice is rebuilt from current legality; pending={:#?}; events={:#?}",
            engine.state.pending_resolution, concession.events
        )
    });
    assert_eq!(resumed_offer.choice_kind(), ChoiceKind::Behold);
    assert_eq!(
        resumed_offer.deciding_player_id, 0,
        "the refreshed offer remains with Elven Passage's controller"
    );
    assert_ne!(
        resumed_offer.resolution_prompt_id,
        original_offer.resolution_prompt_id
    );
    let mut expected_candidates = original_offer.candidate_object_ids.clone();
    expected_candidates.retain(|candidate| *candidate != stolen_elf);
    assert_eq!(resumed_offer.candidate_object_ids, expected_candidates);
    assert!(resumed_offer.candidate_object_ids.contains(&surviving_elf));
    let surviving_elf_index = resumed_offer
        .candidate_object_ids
        .iter()
        .position(|candidate| *candidate == surviving_elf)
        .expect("the surviving battlefield Elf remains an offered candidate");
    assert_eq!(
        resumed_offer.candidate_source_zones[surviving_elf_index],
        tricerules_proto::ruled::v1::ChoiceCandidateSourceZone::Battlefield as i32
    );
    assert!(!concession.events.iter().any(|event| matches!(
        &event.ev,
        Some(tricerules_proto::ruled::v1::ruled_event::Ev::CardsRevealed(
            _
        ))
    )));
    assert!(!concession.events.iter().any(|event| matches!(
        &event.ev,
        Some(tricerules_proto::ruled::v1::ruled_event::Ev::Log(log))
            if log.text == "P0 shuffles their library."
    )));
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("the refreshed Behold remains parked");
    let serialized_pending =
        serde_json::to_value(pending).expect("the refreshed continuation is serializable");
    let search_receipt = &serialized_pending["continuation"]["BeholdChoice"]["stack"]["program"]
        ["slots"][0]["PermanentSelection"][0];
    assert_eq!(
        search_receipt["object_id"].as_u64(),
        Some(found_land as u64)
    );
    assert_eq!(
        search_receipt["zone_change_generation"].as_u64(),
        Some(
            engine
                .state
                .zone_change_generation
                .get(&found_land)
                .copied()
                .unwrap_or(0),
        ),
        "observer restoration preserves the typed search result's generation"
    );
    assert_eq!(search_receipt["controller_at_event"].as_i64(), Some(0));

    let before_stale_answer = format!("{:?}", engine.state);
    let before_stale_answer_index = engine.state.command_index;
    let stale = RuledCommand {
        cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
            resolution_prompt_id: original_offer.resolution_prompt_id,
            chosen_object_ids: vec![surviving_elf],
            ..Default::default()
        })),
    };
    assert!(engine.apply_command(0, &stale).is_err());
    assert_eq!(format!("{:?}", engine.state), before_stale_answer);
    assert_eq!(engine.state.command_index, before_stale_answer_index);

    let current = RuledCommand {
        cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
            resolution_prompt_id: resumed_offer.resolution_prompt_id,
            chosen_object_ids: vec![surviving_elf],
            ..Default::default()
        })),
    };
    engine
        .apply_command(0, &current)
        .expect("P0 may behold its remaining Elf");
    assert!(!engine.state.objects[&found_land].tapped);
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.players[2].has_lost);
}

#[test]
fn observer_restore_reenters_only_the_uncommitted_proliferate_leaf() {
    let (mut engine, _) = setup_three_player_observer_return(
        10_909,
        &["mutational_advantage", "mind_control"],
        &[],
        &["banishing_light"],
    );
    let stolen = inject_creature_on_battlefield(&mut engine, 2, "hill_giant");
    let survivor = inject_creature_on_battlefield(&mut engine, 0, "hill_giant");
    for object_id in [stolen, survivor] {
        engine
            .state
            .objects
            .get_mut(&object_id)
            .expect("fixture creature")
            .set_counter(tricerules_cards::CounterKind::PlusOnePlusOne, 1);
    }
    mind_control_p2_creature(&mut engine, stolen);
    ensure_card_in_hand(&mut engine, 0, "mutational_advantage");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            g: 1,
            c: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "mutational_advantage");
    engine
        .apply_scenario_command(0, &cast_spell(slot, Vec::new()))
        .expect("P0 casts Mutational Advantage");
    let original = pass_until_resolution_choice(&mut engine);
    assert_eq!(original.choice_kind(), ChoiceKind::Proliferate);
    assert!(original.candidate_object_ids.contains(&stolen));
    assert!(original.candidate_object_ids.contains(&survivor));
    let protection_prefix_count = engine
        .state
        .continuous_effects
        .iter()
        .filter(|effect| {
            effect.source_id == Some(original.source_object_id)
                && effect.affected == tricerules_core::state::AffectedScope::Single(survivor)
        })
        .count();
    assert_eq!(protection_prefix_count, 2);

    let refreshed = concede_p2_and_refresh_offer(&mut engine, &original, stolen);
    assert!(refreshed.candidate_object_ids.contains(&survivor));
    assert_eq!(
        engine
            .state
            .continuous_effects
            .iter()
            .filter(|effect| {
                effect.source_id == Some(original.source_object_id)
                    && effect.affected == tricerules_core::state::AffectedScope::Single(survivor)
            })
            .count(),
        protection_prefix_count,
        "the already-resolved protection prefix remains committed exactly once"
    );
    assert_stale_prompt_rejected(&mut engine, 0, original.resolution_prompt_id, survivor);
    engine
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
                    resolution_prompt_id: refreshed.resolution_prompt_id,
                    chosen_object_ids: vec![survivor],
                    ..Default::default()
                })),
            },
        )
        .expect("P0 proliferates the surviving permanent");
    assert_eq!(
        engine.state.objects[&survivor]
            .counter_count(tricerules_cards::CounterKind::PlusOnePlusOne),
        2
    );
    assert!(engine.state.pending_resolution.is_none());
}

#[test]
fn observer_restore_rebuilds_populate_candidates_after_candidate_owner_concedes() {
    let (mut engine, _) = setup_three_player_observer_return(
        10_910,
        &["wake_the_reflections", "mind_control"],
        &[],
        &["banishing_light"],
    );
    let stolen = inject_creature_on_battlefield(&mut engine, 2, "saproling_g_1_1");
    let survivor = inject_creature_on_battlefield(&mut engine, 0, "saproling_g_1_1");
    mind_control_p2_creature(&mut engine, stolen);
    ensure_card_in_hand(&mut engine, 0, "wake_the_reflections");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            w: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "wake_the_reflections");
    engine
        .apply_scenario_command(0, &cast_spell(slot, Vec::new()))
        .expect("P0 casts Wake the Reflections");
    let original = pass_until_resolution_choice(&mut engine);
    assert_eq!(original.choice_kind(), ChoiceKind::CopySource);
    assert!(original.candidate_object_ids.contains(&stolen));
    assert!(original.candidate_object_ids.contains(&survivor));

    let refreshed = concede_p2_and_refresh_offer(&mut engine, &original, stolen);
    let mut expected_candidates = original.candidate_object_ids.clone();
    expected_candidates.retain(|candidate| *candidate != stolen);
    assert_eq!(refreshed.candidate_object_ids, expected_candidates);
    assert!(refreshed.candidate_object_ids.contains(&survivor));
    assert_stale_prompt_rejected(&mut engine, 0, original.resolution_prompt_id, survivor);
    let result = engine
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
                    resolution_prompt_id: refreshed.resolution_prompt_id,
                    chosen_object_ids: vec![survivor],
                    ..Default::default()
                })),
            },
        )
        .expect("P0 populates from its remaining Saproling");
    assert_eq!(token_created_events(&result).len(), 1);
    assert!(engine.state.pending_resolution.is_none());
}

#[test]
fn observer_restore_completes_amass_when_departure_leaves_one_army() {
    let (mut engine, _) = setup_three_player_observer_return(
        10_911,
        &["goblin-town_flunkies", "mind_control"],
        &[],
        &["banishing_light"],
    );
    let stolen = inject_creature_on_battlefield(&mut engine, 2, "goblin_army_b_0_0");
    let survivor = inject_creature_on_battlefield(&mut engine, 0, "goblin_army_b_0_0");
    for object_id in [stolen, survivor] {
        engine
            .state
            .objects
            .get_mut(&object_id)
            .expect("fixture Army")
            .set_counter(tricerules_cards::CounterKind::PlusOnePlusOne, 1);
    }
    mind_control_p2_creature(&mut engine, stolen);
    ensure_card_in_hand(&mut engine, 0, "goblin-town_flunkies");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            r: 1,
            c: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "goblin-town_flunkies");
    engine
        .apply_scenario_command(0, &cast_spell(slot, Vec::new()))
        .expect("P0 casts Goblin-town Flunkies");
    let original = pass_until_resolution_choice(&mut engine);
    assert_eq!(original.choice_kind(), ChoiceKind::PermanentObjects);
    assert!(original.candidate_object_ids.contains(&stolen));
    assert!(original.candidate_object_ids.contains(&survivor));

    let batch = engine
        .apply_command(2, &concede())
        .expect("P2 concession removes one Army and restores the Amass instruction");
    assert!(!engine.state.objects.contains_key(&stolen));
    assert!(engine.state.pending_resolution.is_none());
    assert!(batch
        .resolution_prompt_state
        .as_ref()
        .is_some_and(|state| !state.active));
    assert_eq!(
        engine.state.objects[&survivor]
            .counter_count(tricerules_cards::CounterKind::PlusOnePlusOne),
        2,
        "the remaining Army is selected automatically by the production Amass leaf"
    );
    assert_stale_prompt_rejected(&mut engine, 0, original.resolution_prompt_id, survivor);
}

#[test]
fn observer_restore_reenters_chaos_spewer_otherwise_blight_leaf() {
    let (mut engine, _) = setup_three_player_observer_return(
        10_912,
        &["chaos_spewer", "mind_control", "hill_giant"],
        &[],
        &["banishing_light"],
    );
    let stolen = inject_creature_on_battlefield(&mut engine, 2, "hill_giant");
    let survivor = relocate_to_battlefield(&mut engine, 0, "hill_giant", false);
    mind_control_p2_creature(&mut engine, stolen);
    ensure_card_in_hand(&mut engine, 0, "chaos_spewer");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            b: 1,
            r: 1,
            c: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "chaos_spewer");
    engine
        .apply_scenario_command(0, &cast_spell(slot, Vec::new()))
        .expect("P0 casts Chaos Spewer");
    let branch_offer = pass_until_resolution_choice(&mut engine);
    assert_eq!(branch_offer.choice_kind(), ChoiceKind::ResolutionBranch);
    assert_eq!(branch_offer.resolution_branches.len(), 1);
    let payment_batch = engine
        .apply_scenario_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
                    resolution_prompt_id: 0,
                    decision: tricerules_proto::ruled::v1::ResolutionChoiceDecision::SelectBranch
                        as i32,
                    selected_branch_index: 0,
                    ..Default::default()
                })),
            },
        )
        .expect("P0 selects Chaos Spewer's optional payment");
    assert_eq!(
        find_resolution_choice(&payment_batch)
            .expect("the selected payment branch opens mana payment")
            .choice_kind(),
        ChoiceKind::ManaPayment
    );
    let blight_batch = submit_mana_resolution_decision(
        &mut engine,
        0,
        tricerules_proto::ruled::v1::ResolutionChoiceDecision::Decline,
    )
    .expect("declining the payment enters the Otherwise Blight block");
    let original = find_resolution_choice(&blight_batch)
        .expect("Otherwise parks Blight")
        .clone();
    assert_eq!(original.choice_kind(), ChoiceKind::CostObjects);
    assert!(original.candidate_object_ids.contains(&stolen));
    assert!(original.candidate_object_ids.contains(&survivor));

    let refreshed = concede_p2_and_refresh_offer(&mut engine, &original, stolen);
    let mut expected_candidates = original.candidate_object_ids.clone();
    expected_candidates.retain(|candidate| *candidate != stolen);
    assert_eq!(refreshed.candidate_object_ids, expected_candidates);
    assert!(refreshed.candidate_object_ids.contains(&survivor));
    assert_stale_prompt_rejected(&mut engine, 0, original.resolution_prompt_id, survivor);
    let result = engine
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
                    resolution_prompt_id: refreshed.resolution_prompt_id,
                    chosen_object_ids: vec![survivor],
                    ..Default::default()
                })),
            },
        )
        .expect("P0 chooses its surviving creature for Blight");
    assert_eq!(
        engine.state.objects[&survivor]
            .counter_count(tricerules_cards::CounterKind::MinusOneMinusOne),
        2,
        "answer events: {:#?}; state counters: {:?}",
        result.events,
        engine.state.objects[&survivor].counters
    );
    assert!(engine.state.pending_resolution.is_none());
}

#[test]
fn observer_restore_finishes_blight_when_departure_removes_last_borrowed_candidate() {
    let (mut engine, _) = setup_three_player_observer_return(
        10_914,
        &["chaos_spewer", "mind_control", "hill_giant"],
        &[],
        &["banishing_light"],
    );
    let borrowed = inject_creature_on_battlefield(&mut engine, 2, "hill_giant");
    mind_control_p2_creature(&mut engine, borrowed);
    ensure_card_in_hand(&mut engine, 0, "chaos_spewer");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            r: 1,
            c: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "chaos_spewer");
    engine
        .apply_scenario_command(0, &cast_spell(slot, Vec::new()))
        .expect("P0 casts Chaos Spewer");
    pass_priority_round(&mut engine);
    let spewer = engine
        .state
        .objects
        .values()
        .find(|object| {
            object.card_id == "chaos_spewer" && object.zone == tricerules_core::Zone::Battlefield
        })
        .expect("Chaos Spewer enters and puts its trigger on the stack")
        .id;
    // The source's trigger remains independent after its source leaves; removing the Spewer
    // leaves only P2's borrowed creature as a possible Blight recipient.
    engine.state.players[0]
        .battlefield
        .retain(|object_id| *object_id != spewer);
    engine.state.players[0].graveyard.push(spewer);
    engine.state.objects.get_mut(&spewer).unwrap().zone = tricerules_core::Zone::Graveyard;
    *engine
        .state
        .zone_change_generation
        .entry(spewer)
        .or_default() += 1;

    let branch_offer = pass_until_resolution_choice(&mut engine);
    assert_eq!(branch_offer.choice_kind(), ChoiceKind::ResolutionBranch);
    let payment = engine
        .apply_scenario_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
                    resolution_prompt_id: branch_offer.resolution_prompt_id,
                    decision: tricerules_proto::ruled::v1::ResolutionChoiceDecision::SelectBranch
                        as i32,
                    selected_branch_index: 0,
                    ..Default::default()
                })),
            },
        )
        .expect("P0 selects the optional payment branch");
    assert_eq!(
        find_resolution_choice(&payment)
            .expect("the branch starts the fixed payment")
            .choice_kind(),
        ChoiceKind::ManaPayment
    );
    let blight_batch = submit_mana_resolution_decision(
        &mut engine,
        0,
        tricerules_proto::ruled::v1::ResolutionChoiceDecision::Decline,
    )
    .expect("declining payment enters Otherwise Blight");
    let blight = find_resolution_choice(&blight_batch)
        .expect("Blight parks on P2's borrowed creature")
        .clone();
    assert_eq!(blight.choice_kind(), ChoiceKind::CostObjects);
    assert_eq!(blight.candidate_object_ids, vec![borrowed]);
    let mana_before_departure = engine.state.players[0].mana_pool;
    let draws_before = engine.state.turn_history.current.player(0).cards_drawn;

    let completion = engine
        .apply_command(2, &concede())
        .expect("P2 departure removes the last borrowed Blight candidate");
    assert!(!engine.state.objects.contains_key(&borrowed));
    assert!(find_resolution_choice(&completion).is_none());
    assert!(completion
        .resolution_prompt_state
        .as_ref()
        .is_some_and(|state| !state.active));
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
    assert_eq!(engine.state.players[0].mana_pool, mana_before_departure);
    assert_eq!(
        engine.state.turn_history.current.player(0).cards_drawn,
        draws_before,
        "concession must not replay the declined payment or unrelated Draw work"
    );
    assert_eq!(
        completion
            .events
            .iter()
            .filter(|event| matches!(&event.ev, Some(tricerules_proto::ruled::v1::ruled_event::Ev::Log(log)) if log.text == "P0 blights 2; no creature can receive counters."))
            .count(),
        1,
        "the resumed Blight occurrence completes once"
    );
}

#[test]
fn observer_restore_rebuilds_targeted_player_sacrifice_candidates() {
    let (mut engine, _) = setup_three_player_observer_return(
        10_913,
        &["mind_control"],
        &["diabolic_edict"],
        &["banishing_light"],
    );
    let stolen = inject_creature_on_battlefield(&mut engine, 2, "hill_giant");
    let first_survivor = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let second_survivor = inject_creature_on_battlefield(&mut engine, 0, "wind_drake");
    mind_control_p2_creature(&mut engine, stolen);

    engine
        .apply_scenario_command(0, &pass())
        .expect("P0 passes priority to P1");
    ensure_card_in_hand(&mut engine, 1, "diabolic_edict");
    give_mana(
        &mut engine,
        1,
        ManaGift {
            b: 1,
            c: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 1, "diabolic_edict");
    engine
        .apply_scenario_command(1, &cast_spell(slot, target_player(0)))
        .expect("P1 casts Diabolic Edict targeting P0");
    let original = pass_until_resolution_choice(&mut engine);
    assert_eq!(original.deciding_player_id, 0);
    assert_eq!(original.choice_kind(), ChoiceKind::Revealed);
    assert!(original.candidate_object_ids.contains(&stolen));
    assert!(original.candidate_object_ids.contains(&first_survivor));
    assert!(original.candidate_object_ids.contains(&second_survivor));

    let refreshed = concede_p2_and_refresh_offer(&mut engine, &original, stolen);
    assert_eq!(refreshed.candidate_object_ids.len(), 2);
    assert!(refreshed.candidate_object_ids.contains(&first_survivor));
    assert!(refreshed.candidate_object_ids.contains(&second_survivor));
    assert_stale_prompt_rejected(
        &mut engine,
        0,
        original.resolution_prompt_id,
        first_survivor,
    );
    engine
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
                    resolution_prompt_id: refreshed.resolution_prompt_id,
                    chosen_object_ids: vec![first_survivor],
                    ..Default::default()
                })),
            },
        )
        .expect("P0 sacrifices a creature from the refreshed offer");
    assert_eq!(
        engine.state.objects[&first_survivor].zone,
        tricerules_core::Zone::Graveyard
    );
    assert!(engine.state.pending_resolution.is_none());
}

#[test]
fn observer_restore_reenters_vow_equipment_choice_after_target_owner_concedes() {
    let (mut engine, _) = setup_three_player_observer_return(
        10_914,
        &["mind_control", "vow_to_erebor"],
        &[],
        &["banishing_light"],
    );
    let dwarf = inject_creature_on_battlefield(&mut engine, 2, "dwarven_priest");
    let equipment = inject_permanent_on_battlefield(&mut engine, 0, "bonesplitter");
    mind_control_p2_creature(&mut engine, dwarf);
    engine
        .state
        .objects
        .get_mut(&dwarf)
        .expect("P2-owned Dwarf")
        .tapped = true;

    ensure_card_in_hand(&mut engine, 0, "vow_to_erebor");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            w: 1,
            c: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "vow_to_erebor");
    engine
        .apply_scenario_command(0, &cast_spell(slot, target_object(dwarf)))
        .expect("P0 casts Vow to Erebor targeting the stolen Dwarf");
    let original = pass_until_resolution_choice(&mut engine);
    assert_eq!(original.choice_kind(), ChoiceKind::PermanentObjects);
    assert_eq!(original.candidate_object_ids, vec![equipment]);
    assert!(!engine.state.objects[&dwarf].tapped);
    assert_eq!(engine.effective_power(dwarf), Some(4));
    assert_eq!(engine.effective_toughness(dwarf), Some(4));

    let batch = engine
        .apply_command(2, &concede())
        .expect("P2 concession restores the already-entered Vow chooser");
    assert!(!engine.state.objects.contains_key(&dwarf));
    assert!(engine.state.pending_resolution.is_none());
    assert!(batch
        .resolution_prompt_state
        .as_ref()
        .is_some_and(|state| !state.active));
    assert_stale_prompt_rejected(&mut engine, 0, original.resolution_prompt_id, equipment);
    assert_eq!(engine.state.objects[&equipment].attached_to, None);
}
