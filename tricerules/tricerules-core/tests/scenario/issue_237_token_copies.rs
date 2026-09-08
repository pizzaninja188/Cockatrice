use crate::helpers::*;
use tricerules_cards::primitives::{CounterKind, Keyword};
use tricerules_proto::ruled::v1::{dev_command, DevCommand, DevMoveCard, DevZone};

fn stallion_game() -> (GameEngine, u32) {
    let mut engine = GameEngine::new(
        237_001,
        &[0, 1],
        20,
        Some(vec![
            deck_with("island", &["colorstorm_stallion", "unexpected_assistance"]),
            deck_with("forest", &[]),
        ]),
        true,
    )
    .unwrap();
    advance_to_main1_from_game_start(&mut engine);
    let source = relocate_to_battlefield(&mut engine, 0, "colorstorm_stallion", false);
    (engine, source)
}

fn cast_five(engine: &mut GameEngine) {
    ensure_in_hand(engine, 0, "unexpected_assistance");
    grant_pool(engine, 0);
    let slot = hand_index_for_card(engine, 0, "unexpected_assistance");
    let batch = engine.apply_command(0, &cast_spell(slot, vec![])).unwrap();
    assert_eq!(engine.state.stack.len(), 2);
    assert!(!batch
        .events
        .iter()
        .any(|event| matches!(event.ev, Some(Ev::TriggerNeedsTarget(_)))));
}

fn resolve_top(engine: &mut GameEngine) -> RuledEventBatch {
    engine.apply_command(0, &pass()).unwrap();
    engine.apply_command(1, &pass()).unwrap()
}

fn move_source(engine: &mut GameEngine, zone: DevZone) {
    engine.enable_dev_commands();
    engine
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::DevCommand(DevCommand {
                    target_player_id: 0,
                    dev: Some(dev_command::Dev::MoveCard(DevMoveCard {
                        zone: zone as i32,
                        card_name: "Colorstorm Stallion".into(),
                        ready: false,
                    })),
                })),
            },
        )
        .unwrap();
}

#[test]
fn issue_237_stallion_copies_before_spell_without_counters_or_pump() {
    let (mut engine, source) = stallion_game();
    engine
        .state
        .objects
        .get_mut(&source)
        .unwrap()
        .set_counter(CounterKind::PlusOnePlusOne, 2);
    cast_five(&mut engine);
    let batch = resolve_top(&mut engine);
    let created = token_created_events(&batch);
    assert_eq!(created.len(), 1);
    let token = created[0].object_id;
    assert_eq!(
        engine.state.stack.len(),
        1,
        "Opus resolves before its spell"
    );
    assert_eq!(engine.effective_power(source), Some(6));
    assert_eq!(engine.effective_power(token), Some(3));
    assert!(engine
        .characteristics(token)
        .unwrap()
        .has_keyword(Keyword::Haste));
    assert_eq!(engine.state.objects[&token].owner, 0);
    assert_eq!(engine.state.objects[&token].controller, 0);
}

#[test]
fn issue_237_stallion_uses_departed_generation_without_pumping_returned_source() {
    for returned in [false, true] {
        let (mut engine, source) = stallion_game();
        cast_five(&mut engine);
        move_source(&mut engine, DevZone::Graveyard);
        if returned {
            move_source(&mut engine, DevZone::Battlefield);
        }
        let batch = resolve_top(&mut engine);
        let created = token_created_events(&batch);
        assert_eq!(
            created.len(),
            1,
            "the departed source's last values remain available"
        );
        assert_eq!(engine.effective_power(created[0].object_id), Some(3));
        if returned {
            assert_eq!(
                engine.effective_power(source),
                Some(3),
                "old pump cannot affect new generation"
            );
        }
    }
}

#[test]
fn issue_237_double_faced_permanent_spell_copy_becomes_double_faced_token() {
    let mut engine = GameEngine::new(
        237_002,
        &[0, 1],
        20,
        Some(vec![
            deck_with("mountain", &["reckless_waif_merciless_predator"]),
            deck_with("forest", &[]),
        ]),
        true,
    )
    .unwrap();
    advance_to_main1_from_game_start(&mut engine);
    ensure_in_hand(&mut engine, 0, "reckless_waif_merciless_predator");
    grant_pool(&mut engine, 0);
    let slot = hand_index_for_card(&engine, 0, "reckless_waif_merciless_predator");
    engine.apply_command(0, &cast_spell(slot, vec![])).unwrap();
    // A permanent spell copy uses the existing stack-copy representation.
    let mut copy = engine.state.stack.last().unwrap().clone();
    copy.id = engine.state.next_object_id;
    engine.state.next_object_id += 1;
    copy.is_copy = true;
    let copy_id = copy.id;
    engine.state.stack.push(copy);
    let batch = resolve_top(&mut engine);
    assert_eq!(token_created_events(&batch)[0].object_id, copy_id);
    assert!(engine.state.objects[&copy_id].token_faces.is_some());
}

#[test]
fn issue_237_stallion_uses_reduced_and_alternative_actual_payments() {
    let (mut engine, source) = stallion_game();
    inject_card_into_hand(&mut engine, 0, "mocking_sprite");
    move_ready_to_battlefield(&mut engine, 0, "mocking_sprite");
    cast_five(&mut engine);
    assert_eq!(
        engine
            .state
            .turn_history
            .current
            .spell_casts
            .last()
            .unwrap()
            .mana_spent,
        4
    );
    let batch = resolve_top(&mut engine);
    assert!(token_created_events(&batch).is_empty());
    assert_eq!(engine.effective_power(source), Some(4));

    let (mut engine, source) = stallion_game();
    let spell = inject_card_into_hand(&mut engine, 0, "cackling_counterpart");
    engine.state.players[0].hand.retain(|oid| *oid != spell);
    engine.state.players[0].graveyard.push(spell);
    engine.state.objects.get_mut(&spell).unwrap().zone = tricerules_core::Zone::Graveyard;
    grant_pool(&mut engine, 0);
    engine
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::CastSpell(CastSpell {
                    source: Some(graveyard_cast_source(spell, 0)),
                    cast_method: CastMethod::Flashback as i32,
                    targets: target_object(source),
                    ..Default::default()
                })),
            },
        )
        .unwrap();
    assert_eq!(
        engine
            .state
            .turn_history
            .current
            .spell_casts
            .last()
            .unwrap()
            .mana_spent,
        7
    );
    assert_eq!(token_created_events(&resolve_top(&mut engine)).len(), 1);
}

#[test]
fn issue_237_stallion_copy_uses_current_values_or_the_last_values_before_departure() {
    for departed in [false, true] {
        let (mut engine, source) = stallion_game();
        cast_five(&mut engine);
        let face = tricerules_cards::CardRegistry::global()
            .get("grizzly_bears")
            .unwrap()
            .primary_face()
            .clone();
        engine
            .state
            .objects
            .get_mut(&source)
            .unwrap()
            .copiable_values = Some(tricerules_core::state::CopiableValues {
            source_card_id: "grizzly_bears".into(),
            source_face_index: 0,
            display_name: face.name.clone(),
            face,
            room_faces: None,
        });
        if departed {
            move_source(&mut engine, DevZone::Graveyard);
        }
        let batch = resolve_top(&mut engine);
        let created = token_created_events(&batch);
        assert_eq!(created.len(), 1);
        assert_eq!(created[0].identity.as_ref().unwrap().name, "Grizzly Bears");
        assert_eq!(engine.effective_power(created[0].object_id), Some(2));
    }
}

#[test]
fn issue_237_stallion_trigger_survives_countering_the_spell() {
    let (mut engine, _) = stallion_game();
    cast_five(&mut engine);
    let original = engine.state.stack[0].id;
    inject_card_into_hand(&mut engine, 1, "counterspell");
    grant_pool(&mut engine, 1);
    engine.apply_command(0, &pass()).unwrap();
    let slot = hand_index_for_card(&engine, 1, "counterspell");
    engine
        .apply_command(
            1,
            &cast_spell(
                slot,
                vec![TargetRef {
                    object_id: original,
                    kind: TargetRefKind::Stack as i32,
                    ..Default::default()
                }],
            ),
        )
        .unwrap();
    pass_both_players(&mut engine);
    assert_eq!(engine.state.stack.len(), 1);
    assert!(engine.state.stack[0].is_triggered);
    assert_eq!(token_created_events(&resolve_top(&mut engine)).len(), 1);
}

#[test]
fn issue_237_copied_stallion_keeps_ward_and_opus() {
    let (mut engine, _) = stallion_game();
    cast_five(&mut engine);
    let token = token_created_events(&resolve_top(&mut engine))[0].object_id;
    resolve_entire_stack_two_player(&mut engine);
    let discard = engine
        .state
        .pending_resolution
        .as_ref()
        .unwrap()
        .presentation
        .candidates[0];
    engine
        .apply_command(0, &submit_resolution_choice(vec![discard]))
        .unwrap();
    inject_card_into_hand(&mut engine, 0, "unexpected_assistance");
    grant_pool(&mut engine, 0);
    let slot = hand_index_for_card(&engine, 0, "unexpected_assistance");
    engine.apply_command(0, &cast_spell(slot, vec![])).unwrap();
    answer_trigger_order_in_engine_order(&mut engine);
    assert_eq!(
        engine.state.stack.len(),
        3,
        "both existing Stallions trigger"
    );
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        engine.state.players[0].battlefield.len(),
        4,
        "new copies do not trigger retroactively"
    );
    let discard = engine
        .state
        .pending_resolution
        .as_ref()
        .unwrap()
        .presentation
        .candidates[0];
    engine
        .apply_command(0, &submit_resolution_choice(vec![discard]))
        .unwrap();

    inject_card_into_hand(&mut engine, 1, "unsummon");
    grant_pool(&mut engine, 1);
    engine.apply_command(0, &pass()).unwrap();
    let slot = hand_index_for_card(&engine, 1, "unsummon");
    engine
        .apply_command(1, &cast_spell(slot, target_object(token)))
        .unwrap();
    assert_eq!(
        engine.state.stack.len(),
        2,
        "copied ward triggers for an opponent's spell"
    );
    let spell_id = engine.state.stack[0].id;
    pass_both_players(&mut engine);
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .deciding_player,
        1
    );
    engine
        .apply_command(
            1,
            &submit_resolution_decision(
                tricerules_proto::ruled::v1::ResolutionChoiceDecision::Decline,
            ),
        )
        .unwrap();
    assert!(engine.state.stack.iter().all(|item| item.id != spell_id));
    assert!(engine.state.players[0].battlefield.contains(&token));
}

#[test]
fn issue_237_source_copy_command_replay_and_three_seat_ownership() {
    fn setup() -> GameEngine {
        let (mut engine, source) = stallion_game();
        engine
            .state
            .players
            .push(tricerules_core::state::PlayerState::new(7, 20));
        engine.state.objects.get_mut(&source).unwrap().owner = 7;
        grant_pool(&mut engine, 0);
        ensure_in_hand(&mut engine, 0, "unexpected_assistance");
        engine.enable_dev_commands();
        engine
    }
    let mut engine = setup();
    let slot = hand_index_for_card(&engine, 0, "unexpected_assistance");
    let commands = [
        (0, cast_spell(slot, vec![])),
        (
            0,
            RuledCommand {
                cmd: Some(Cmd::DevCommand(DevCommand {
                    target_player_id: 0,
                    dev: Some(dev_command::Dev::MoveCard(DevMoveCard {
                        card_name: "Colorstorm Stallion".into(),
                        zone: DevZone::Graveyard as i32,
                        ready: false,
                    })),
                })),
            },
        ),
        (0, pass()),
        (1, pass()),
        (7, pass()),
    ];
    let batches: Vec<_> = commands
        .iter()
        .map(|(player, cmd)| engine.apply_command(*player, cmd).unwrap())
        .collect();
    let tokens = token_created_events(batches.last().unwrap());
    assert_eq!(tokens.len(), 1);
    let token = &engine.state.objects[&tokens[0].object_id];
    assert_eq!(
        token.owner, 0,
        "ability controller creates and owns the copy, not original owner"
    );
    assert_eq!(token.controller, 0);
    let mut replay = setup();
    let replayed: Vec<_> = commands
        .iter()
        .map(|(player, cmd)| replay.apply_command(*player, cmd).unwrap())
        .collect();
    assert_eq!(batches, replayed);
    assert_eq!(engine.state.command_index, replay.state.command_index);
}

#[test]
fn issue_237_departed_token_source_survives_cessation_as_last_known_values() {
    let (mut engine, source) = stallion_game();
    let face = tricerules_cards::CardRegistry::global()
        .get("colorstorm_stallion")
        .unwrap()
        .primary_face()
        .clone();
    engine.state.objects.get_mut(&source).unwrap().token_origin =
        Some(tricerules_core::state::CopiableValues {
            source_card_id: "colorstorm_stallion".into(),
            source_face_index: 0,
            display_name: face.name.clone(),
            face,
            room_faces: None,
        });
    cast_five(&mut engine);
    move_source(&mut engine, DevZone::Graveyard);
    assert!(
        !engine.state.objects.contains_key(&source),
        "departed token has ceased to exist"
    );
    let batch = resolve_top(&mut engine);
    let tokens = token_created_events(&batch);
    assert_eq!(tokens.len(), 1);
    assert_eq!(engine.effective_power(tokens[0].object_id), Some(3));
}
