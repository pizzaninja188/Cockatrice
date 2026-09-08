use super::helpers::*;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1 as rv1;

fn prepared_healer() -> (GameEngine, u32) {
    let mut engine = GameEngine::new(
        18002,
        &[0, 1],
        20,
        Some(vec![vec!["forest".into(); 20], vec!["forest".into(); 20]]),
        true,
    )
    .unwrap();
    advance_to_main1_from_game_start(&mut engine);
    inject_card_into_hand(&mut engine, 0, "infirmary_healer_stream_of_life");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            g: 2,
            ..Default::default()
        },
    );
    let index = hand_index_for_card(&engine, 0, "infirmary_healer_stream_of_life");
    engine
        .apply_command(0, &cast_spell_face(index, vec![], 0))
        .unwrap();
    let permanent = engine.state.stack.last().unwrap().id;
    resolve_entire_stack_two_player(&mut engine);
    (engine, permanent)
}

fn inset_cast(engine: &mut GameEngine, copy: u32, x: u32) -> rv1::RuledCommand {
    let batch = engine.initial_response_batch();
    let action = batch.legal_by_player[&0]
        .zone_cast_actions
        .iter()
        .find(|a| a.object_id == copy)
        .unwrap();
    rv1::RuledCommand {
        cmd: Some(rv1::ruled_command::Cmd::CastSpell(rv1::CastSpell {
            source: Some(rv1::CastSource {
                location: Some(rv1::cast_source::Location::ExileObjectId(copy)),
                expected_zone_change_generation: Some(action.zone_change_generation),
            }),
            face_index: action.face_index,
            casting_permission_id: action.casting_permission_id,
            cast_method: action.cast_method,
            targets: target_player(1),
            x_value: x,
            ..Default::default()
        })),
    }
}

#[test]
fn preparation_logged_instructions_reprepare_once_and_invalidate_old_copy() {
    let (mut engine, permanent) = prepared_healer();
    let old_copy = engine.state.prepared_permanents[&permanent];
    let stale_cast = inset_cast(&mut engine, old_copy, 0);
    let definition = tricerules_cards::CardRegistry::global()
        .get("infirmary_healer_stream_of_life")
        .unwrap();
    let mut face = definition.primary_face().clone();
    for prepared in [false, true] {
        face.activated_abilities.push(
            serde_json::from_value(serde_json::json!({
                "ability_id": if prepared { "prepare" } else { "unprepare" },
                "presentation": "Fallback", "costs": [],
                "effect": [{"SetPrepared": {"subject": "Source", "prepared": prepared}}]
            }))
            .expect("typed preparation instruction"),
        );
    }
    engine
        .state
        .objects
        .get_mut(&permanent)
        .unwrap()
        .copiable_values = Some(tricerules_core::state::CopiableValues {
        source_card_id: definition.id.clone(),
        source_face_index: 0,
        display_name: definition.name.clone(),
        face,
        room_faces: None,
    });
    let unprepare = activate_ability_for(&engine, permanent, 0, vec![]);
    engine.apply_command(0, &unprepare).unwrap();
    resolve_entire_stack_two_player(&mut engine);
    assert!(!engine.state.prepared_permanents.contains_key(&permanent));
    assert!(!engine.state.objects.contains_key(&old_copy));
    let prepare = activate_ability_for(&engine, permanent, 1, vec![]);
    engine.apply_command(0, &prepare).unwrap();
    resolve_entire_stack_two_player(&mut engine);
    let new_copy = engine.state.prepared_permanents[&permanent];
    assert_ne!(new_copy, old_copy);
    engine.apply_command(0, &prepare).unwrap();
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.prepared_permanents[&permanent], new_copy);
    assert!(engine.apply_command(0, &stale_cast).is_err());
}

#[test]
fn preparation_x_cast_consumes_designation_and_resolves_without_moving_source() {
    let (mut engine, permanent) = prepared_healer();
    let copy = engine.state.prepared_permanents[&permanent];
    let command = inset_cast(&mut engine, copy, 3);
    assert!(
        engine.apply_command(0, &command).is_err(),
        "unpaid cast must fail"
    );
    assert_eq!(engine.state.prepared_permanents[&permanent], copy);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            g: 4,
            ..Default::default()
        },
    );
    engine.apply_command(0, &command).unwrap();
    assert!(!engine.state.prepared_permanents.contains_key(&permanent));
    let spell = engine.state.stack.last().unwrap();
    assert!(spell.is_copy);
    assert!(
        spell.cast_occurrence.is_some(),
        "a cast copy counts as cast"
    );
    assert_eq!(spell.chosen_x, 3);
    assert_eq!(engine.state.objects[&permanent].zone, Zone::Battlefield);
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.players[1].life, 23);
    assert_eq!(engine.state.objects[&permanent].zone, Zone::Battlefield);
    assert!(!engine.state.objects.contains_key(&copy));
    assert!(engine
        .state
        .players
        .iter()
        .all(|p| !p.graveyard.contains(&copy)));
    assert!(
        engine
            .state
            .turn_history
            .current
            .permanent_cards_entered_graveyard
            .is_empty(),
        "a spell copy is not a permanent card entering a graveyard"
    );
    assert!(engine.apply_command(0, &command).is_err());
}

#[test]
fn preparation_palette_copies_the_next_cast_even_after_original_is_countered() {
    let (mut engine, _) = prepared_healer();
    inject_card_into_hand(&mut engine, 0, "pigment_wrangler_striking_palette");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            r: 7,
            ..Default::default()
        },
    );
    let index = hand_index_for_card(&engine, 0, "pigment_wrangler_striking_palette");
    engine
        .apply_command(0, &cast_spell_face(index, vec![], 0))
        .unwrap();
    let source = engine.state.stack.last().unwrap().id;
    resolve_entire_stack_two_player(&mut engine);
    let copy = engine.state.prepared_permanents[&source];
    let mut palette = inset_cast(&mut engine, copy, 0);
    if let Some(Cmd::CastSpell(cast)) = &mut palette.cmd {
        cast.targets.clear();
    }
    engine.apply_command(0, &palette).unwrap();
    resolve_entire_stack_two_player(&mut engine);
    inject_card_into_hand(&mut engine, 0, "lightning_bolt");
    let index = hand_index_for_card(&engine, 0, "lightning_bolt");
    engine
        .apply_command(0, &cast_spell(index, target_player(1)))
        .unwrap();
    assert_eq!(engine.state.stack.len(), 2, "copy trigger above Bolt");
    let bolt = engine.state.stack[0].id;
    engine.apply_command(0, &pass()).unwrap();
    inject_card_into_hand(&mut engine, 1, "counterspell");
    give_mana(
        &mut engine,
        1,
        ManaGift {
            u: 2,
            ..Default::default()
        },
    );
    let index = hand_index_for_card(&engine, 1, "counterspell");
    engine
        .apply_command(1, &cast_spell(index, target_object(bolt)))
        .unwrap();
    pass_both_players(&mut engine);
    assert!(!engine.state.stack.iter().any(|item| item.id == bolt));
    pass_both_players(&mut engine);
    assert!(
        engine.state.pending_resolution.is_some(),
        "retarget the copied Bolt"
    );
    engine
        .apply_command(0, &submit_resolution_choice(vec![1]))
        .unwrap();
    assert!(engine.state.stack.last().unwrap().is_copy);
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.players[1].life, 17);
}

#[test]
fn preparation_rejoinder_decline_still_draws_and_twofold_intent_pumps() {
    let (mut engine, healer) = prepared_healer();
    for card_id in [
        "elite_interceptor_rejoinder",
        "quill-blade_laureate_twofold_intent",
    ] {
        inject_card_into_hand(&mut engine, 0, card_id);
        give_mana(
            &mut engine,
            0,
            ManaGift {
                w: 4,
                ..Default::default()
            },
        );
        let index = hand_index_for_card(&engine, 0, card_id);
        engine
            .apply_command(0, &cast_spell_face(index, vec![], 0))
            .unwrap();
        let permanent = engine.state.stack.last().unwrap().id;
        resolve_entire_stack_two_player(&mut engine);
        let copy = engine.state.prepared_permanents[&permanent];
        let mut cast = inset_cast(&mut engine, copy, 0);
        if let Some(Cmd::CastSpell(cast)) = &mut cast.cmd {
            cast.targets = target_object(healer);
        }
        let hand_size = engine.state.players[0].hand.len();
        engine.apply_command(0, &cast).unwrap();
        pass_both_players(&mut engine);
        if card_id == "elite_interceptor_rejoinder" {
            assert!(engine.state.pending_resolution.is_some());
            engine
                .apply_command(
                    0,
                    &submit_resolution_decision(rv1::ResolutionChoiceDecision::Decline),
                )
                .unwrap();
            assert_eq!(engine.state.players[0].hand.len(), hand_size + 1);
            assert!(!engine.state.objects[&healer].tapped);
        } else {
            assert_eq!(engine.effective_power(healer), Some(3));
            assert!(engine.effective_has_keyword(healer, tricerules_cards::Keyword::DoubleStrike));
        }
    }
}

#[test]
fn preparation_entry_creates_a_distinct_castable_exile_copy() {
    let mut engine = GameEngine::new(
        18001,
        &[0, 1],
        20,
        Some(vec![vec!["forest".into(); 20], vec!["forest".into(); 20]]),
        true,
    )
    .unwrap();
    advance_to_main1_from_game_start(&mut engine);
    inject_card_into_hand(&mut engine, 0, "infirmary_healer_stream_of_life");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            g: 2,
            ..Default::default()
        },
    );
    let index = hand_index_for_card(&engine, 0, "infirmary_healer_stream_of_life");
    engine
        .apply_command(0, &cast_spell_face(index, vec![], 0))
        .unwrap();
    let permanent = engine.state.stack.last().unwrap().id;
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&permanent].zone, Zone::Battlefield);
    let batch = engine.initial_response_batch();
    let action = batch.legal_by_player[&0]
        .zone_cast_actions
        .iter()
        .find(|action| action.card_name == "Stream of Life")
        .expect("prepared inset has a cast action");
    assert_ne!(action.object_id, permanent);
    assert_eq!(action.source_zone, rv1::CastSourceZone::Exile as i32);
    assert_eq!(engine.state.objects[&action.object_id].zone, Zone::Exile);
    assert!(batch.legal_by_player[&1].zone_cast_actions.is_empty());
}

#[test]
fn preparation_clone_applies_enters_prepared_and_copies_have_separate_lifetimes() {
    let (mut engine, source) = prepared_healer();
    let original_copy = engine.state.prepared_permanents[&source];
    inject_card_into_hand(&mut engine, 0, "clone");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 4,
            ..Default::default()
        },
    );
    let index = hand_index_for_card(&engine, 0, "clone");
    engine.apply_command(0, &cast_spell(index, vec![])).unwrap();
    let clone = engine.state.stack.last().unwrap().id;
    pass_both_players(&mut engine);
    assert!(engine.state.pending_resolution.is_some());
    engine
        .apply_command(0, &submit_resolution_choice(vec![source]))
        .unwrap();
    let cloned_copy = engine.state.prepared_permanents[&clone];
    assert_ne!(cloned_copy, original_copy);
    assert_eq!(
        engine.state.objects[&cloned_copy].card_id,
        "infirmary_healer_stream_of_life"
    );
    give_mana(
        &mut engine,
        0,
        ManaGift {
            g: 1,
            ..Default::default()
        },
    );
    let command = inset_cast(&mut engine, cloned_copy, 0);
    engine.apply_command(0, &command).unwrap();
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.players[1].life, 20);
    assert_eq!(engine.state.prepared_permanents[&source], original_copy);
    assert!(!engine.state.prepared_permanents.contains_key(&clone));
}

#[test]
fn preparation_copied_spell_preserves_x_and_classification_without_a_second_cast() {
    let (mut engine, source) = prepared_healer();
    let copy = engine.state.prepared_permanents[&source];
    give_mana(
        &mut engine,
        0,
        ManaGift {
            g: 4,
            u: 2,
            ..Default::default()
        },
    );
    let command = inset_cast(&mut engine, copy, 3);
    let batch = engine.apply_command(0, &command).unwrap();
    assert!(batch.events.iter().any(|e| matches!(&e.ev, Some(rv1::ruled_event::Ev::StackPushed(p)) if p.object_id == copy && p.is_prepare_spell && p.is_copy)));
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 2,
            ..Default::default()
        },
    );
    inject_card_into_hand(&mut engine, 0, "twincast");
    let index = hand_index_for_card(&engine, 0, "twincast");
    engine
        .apply_command(0, &cast_spell(index, target_object(copy)))
        .unwrap();
    pass_both_players(&mut engine);
    let batch = engine
        .apply_command(0, &submit_resolution_choice(vec![0]))
        .unwrap();
    assert!(batch.events.iter().any(|e| matches!(&e.ev, Some(rv1::ruled_event::Ev::StackPushed(p)) if p.is_prepare_spell && p.is_copy && p.copy_source_object_id == copy)));
    let second = engine.state.stack.last().unwrap();
    assert!(second.cast_occurrence.is_none());
    assert_eq!(second.chosen_x, 3);
    assert_eq!(second.face_index, 1);
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.players[0].life, 23);
    assert_eq!(engine.state.players[1].life, 23);
    assert_eq!(engine.state.objects[&source].zone, Zone::Battlefield);
}

#[test]
fn preparation_rejoinder_tap_and_untap_choices_preserve_target_and_draw() {
    for (branch, initially_tapped, expected_tapped) in [(0, false, true), (1, true, false)] {
        let (mut engine, healer) = prepared_healer();
        inject_card_into_hand(&mut engine, 0, "elite_interceptor_rejoinder");
        let source = move_ready_to_battlefield(&mut engine, 0, "elite_interceptor_rejoinder");
        let copy = engine.state.prepared_permanents[&source];
        engine.state.objects.get_mut(&healer).unwrap().tapped = initially_tapped;
        give_mana(
            &mut engine,
            0,
            ManaGift {
                w: 2,
                ..Default::default()
            },
        );
        let mut command = inset_cast(&mut engine, copy, 0);
        if let Some(Cmd::CastSpell(cast)) = &mut command.cmd {
            cast.targets = target_object(healer);
        }
        engine.apply_command(0, &command).unwrap();
        let hand_before = engine.state.players[0].hand.len();
        pass_both_players(&mut engine);
        let mut choice = submit_resolution_decision(rv1::ResolutionChoiceDecision::SelectBranch);
        if let Some(Cmd::SubmitResolutionChoice(choice)) = &mut choice.cmd {
            choice.selected_branch_index = branch;
        }
        engine.apply_command(0, &choice).unwrap();
        assert_eq!(engine.state.objects[&healer].tapped, expected_tapped);
        assert_eq!(engine.state.players[0].hand.len(), hand_before + 1);
    }
}

#[test]
fn preparation_rejoinder_illegal_target_fizzles_without_choice_or_draw() {
    let (mut engine, healer) = prepared_healer();
    inject_card_into_hand(&mut engine, 0, "elite_interceptor_rejoinder");
    let source = move_ready_to_battlefield(&mut engine, 0, "elite_interceptor_rejoinder");
    let copy = engine.state.prepared_permanents[&source];
    give_mana(
        &mut engine,
        0,
        ManaGift {
            w: 2,
            ..Default::default()
        },
    );
    let mut command = inset_cast(&mut engine, copy, 0);
    if let Some(Cmd::CastSpell(cast)) = &mut command.cmd {
        cast.targets = target_object(healer);
    }
    engine.apply_command(0, &command).unwrap();
    let hand_before = engine.state.players[0].hand.len();
    engine.apply_command(0, &pass()).unwrap();
    inject_card_into_hand(&mut engine, 1, "lightning_bolt");
    give_mana(
        &mut engine,
        1,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    let index = hand_index_for_card(&engine, 1, "lightning_bolt");
    engine
        .apply_command(1, &cast_spell(index, target_object(healer)))
        .unwrap();
    resolve_entire_stack_two_player(&mut engine);
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(engine.state.players[0].hand.len(), hand_before);
    assert!(!engine.state.objects.contains_key(&copy));
    assert_eq!(engine.state.objects[&source].zone, Zone::Battlefield);
}

#[test]
fn preparation_countered_copy_never_moves_or_reprepares_the_source() {
    let (mut engine, source) = prepared_healer();
    let copy = engine.state.prepared_permanents[&source];
    give_mana(
        &mut engine,
        0,
        ManaGift {
            g: 1,
            ..Default::default()
        },
    );
    let command = inset_cast(&mut engine, copy, 0);
    engine.apply_command(0, &command).unwrap();
    engine.apply_command(0, &pass()).unwrap();
    inject_card_into_hand(&mut engine, 1, "counterspell");
    give_mana(
        &mut engine,
        1,
        ManaGift {
            u: 2,
            ..Default::default()
        },
    );
    let index = hand_index_for_card(&engine, 1, "counterspell");
    engine
        .apply_command(1, &cast_spell(index, target_object(copy)))
        .unwrap();
    resolve_entire_stack_two_player(&mut engine);
    assert!(!engine.state.objects.contains_key(&copy));
    assert!(!engine.state.prepared_permanents.contains_key(&source));
    assert_eq!(engine.state.objects[&source].zone, Zone::Battlefield);
    assert_eq!(engine.state.players[1].life, 20);
}

fn cast_palette(engine: &mut GameEngine) {
    inject_card_into_hand(engine, 0, "pigment_wrangler_striking_palette");
    let source = move_ready_to_battlefield(engine, 0, "pigment_wrangler_striking_palette");
    let copy = engine.state.prepared_permanents[&source];
    give_mana(
        engine,
        0,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    let mut command = inset_cast(engine, copy, 0);
    if let Some(Cmd::CastSpell(cast)) = &mut command.cmd {
        cast.targets.clear();
    }
    engine.apply_command(0, &command).unwrap();
}

#[test]
fn preparation_palette_multiple_one_shot_triggers_use_shared_order_and_retargeting() {
    let (mut engine, _) = prepared_healer();
    cast_palette(&mut engine);
    let palette = engine.state.stack.last().unwrap().id;
    inject_card_into_hand(&mut engine, 0, "twincast");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 2,
            ..Default::default()
        },
    );
    let index = hand_index_for_card(&engine, 0, "twincast");
    engine
        .apply_command(0, &cast_spell(index, target_object(palette)))
        .unwrap();
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.active_event_observers.len(), 2);
    inject_card_into_hand(&mut engine, 0, "lightning_bolt");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    let index = hand_index_for_card(&engine, 0, "lightning_bolt");
    engine
        .apply_command(0, &cast_spell(index, target_player(1)))
        .unwrap();
    answer_trigger_order_in_engine_order(&mut engine);
    assert_eq!(engine.state.stack.len(), 3);
    for _ in 0..2 {
        pass_both_players(&mut engine);
        assert!(engine.state.pending_resolution.is_some());
        engine
            .apply_command(0, &submit_resolution_choice(vec![1]))
            .unwrap();
        pass_both_players(&mut engine);
    }
    pass_both_players(&mut engine);
    assert_eq!(engine.state.players[1].life, 11);
    assert!(engine.state.active_event_observers.is_empty());
    assert!(engine.state.captured_spell_copies.is_empty());
}

#[test]
fn preparation_palette_expires_at_cleanup_and_does_not_copy_next_turn() {
    let (mut engine, _) = prepared_healer();
    cast_palette(&mut engine);
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.active_event_observers.len(), 1);
    let turn = engine.state.turn_instance;
    for _ in 0..40 {
        if engine.state.turn_instance != turn {
            break;
        }
        engine
            .apply_command(engine.state.priority_player_id(), &pass())
            .unwrap();
    }
    assert_ne!(engine.state.turn_instance, turn);
    assert!(engine.state.active_event_observers.is_empty());
    if engine.state.priority_player_id() != 0 {
        engine.apply_command(1, &pass()).unwrap();
    }
    inject_card_into_hand(&mut engine, 0, "lightning_bolt");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    let index = hand_index_for_card(&engine, 0, "lightning_bolt");
    engine
        .apply_command(0, &cast_spell(index, target_player(1)))
        .unwrap();
    assert_eq!(engine.state.stack.len(), 1);
}

#[test]
fn preparation_restricted_mana_and_bad_target_rejections_preserve_the_copy() {
    let (mut engine, source) = prepared_healer();
    let copy = engine.state.prepared_permanents[&source];
    let peeper = inject_permanent_on_battlefield(&mut engine, 0, "creeping_peeper");
    engine
        .apply_command(0, &activate_ability_for(&engine, peeper, 0, vec![]))
        .unwrap();
    let group = engine.state.players[0].restricted_mana[0].restriction_group_id;
    give_mana(
        &mut engine,
        0,
        ManaGift {
            g: 1,
            ..Default::default()
        },
    );
    let mut command = inset_cast(&mut engine, copy, 1);
    if let Some(Cmd::CastSpell(cast)) = &mut command.cmd {
        cast.restricted_mana.push(rv1::ManaSpendSelection {
            restriction_group_id: group,
            u: 1,
            ..Default::default()
        });
    }
    let before = serde_json::to_string(&engine.state.players[0]).unwrap();
    assert!(engine.apply_command(0, &command).is_err());
    assert_eq!(
        serde_json::to_string(&engine.state.players[0]).unwrap(),
        before
    );
    assert_eq!(engine.state.prepared_permanents[&source], copy);
    if let Some(Cmd::CastSpell(cast)) = &mut command.cmd {
        cast.restricted_mana.clear();
        cast.targets = target_object(source);
        cast.x_value = 0;
    }
    assert!(engine.apply_command(0, &command).is_err());
    assert_eq!(
        serde_json::to_string(&engine.state.players[0]).unwrap(),
        before
    );
    assert_eq!(engine.state.prepared_permanents[&source], copy);
}
