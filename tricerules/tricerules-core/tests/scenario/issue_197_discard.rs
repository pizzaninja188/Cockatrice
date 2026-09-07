use crate::helpers::*;
use tricerules_proto::ruled::v1 as rv1;

#[test]
fn issue_197_megrim_observes_each_discard_after_mind_rot_finishes() {
    let mut e = GameEngine::new(
        197,
        &[0, 1],
        20,
        Some(vec![
            deck_with("swamp", &["mind_rot", "megrim"]),
            deck_with("forest", &["grizzly_bears"]),
        ]),
        true,
    )
    .unwrap();
    advance_to_main1_from_game_start(&mut e);
    relocate_to_battlefield(&mut e, 0, "megrim", false);
    relocate_to_hand(&mut e, 0, "mind_rot");
    give_mana(
        &mut e,
        0,
        ManaGift {
            b: 1,
            c: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&e, 0, "mind_rot");
    e.apply_command(0, &cast_spell(slot, target_player(1)))
        .unwrap();
    e.apply_command(0, &pass()).unwrap();
    e.apply_command(1, &pass()).unwrap();
    let chosen = e.state.players[1].hand.iter().take(2).copied().collect();
    e.apply_command(1, &submit_resolution_choice(chosen))
        .unwrap();
    assert!(
        e.state.pending_trigger_order.is_some(),
        "both discards share one trigger-order window"
    );
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.players[1].life, 16);
}

#[test]
fn issue_197_library_discard_is_private_but_still_triggers_megrim() {
    let mut e = GameEngine::new(
        19702,
        &[0, 1],
        20,
        Some(vec![
            deck_with("swamp", &["mind_rot", "megrim", "waste_not"]),
            deck_with("forest", &["library_of_leng", "grizzly_bears"]),
        ]),
        true,
    )
    .unwrap();
    advance_to_main1_from_game_start(&mut e);
    relocate_to_battlefield(&mut e, 0, "megrim", false);
    relocate_to_battlefield(&mut e, 0, "waste_not", false);
    relocate_to_battlefield(&mut e, 1, "library_of_leng", false);
    relocate_to_hand(&mut e, 0, "mind_rot");
    let creature = relocate_to_hand(&mut e, 1, "grizzly_bears");
    let land = relocate_to_hand(&mut e, 1, "forest");
    give_mana(
        &mut e,
        0,
        ManaGift {
            b: 1,
            c: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&e, 0, "mind_rot");
    e.apply_command(0, &cast_spell(slot, target_player(1)))
        .unwrap();
    e.apply_command(0, &pass()).unwrap();
    e.apply_command(1, &pass()).unwrap();
    e.apply_command(1, &submit_resolution_choice(vec![creature, land]))
        .unwrap();
    // Opaque destination choices: 0 graveyard, 1 library.
    e.apply_command(1, &submit_resolution_choice(vec![1]))
        .unwrap();
    e.apply_command(1, &submit_resolution_choice(vec![1]))
        .unwrap();
    e.apply_command(1, &submit_resolution_choice(vec![land, creature]))
        .unwrap();
    assert_eq!(e.state.players[1].library.front(), Some(&land));
    assert_eq!(e.state.players[1].library.get(1), Some(&creature));
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.players[1].life, 16);

    assert!(!e.state.players[0]
        .battlefield
        .iter()
        .any(|oid| e.state.objects[oid].card_id == "zombie_b_2_2"));
}

#[test]
fn issue_197_madness_casts_from_exile_during_trigger_resolution() {
    let mut e = GameEngine::new(
        19703,
        &[0, 1],
        20,
        Some(vec![
            deck_with("swamp", &["mind_rot"]),
            deck_with("mountain", &["fiery_temper"]),
        ]),
        true,
    )
    .unwrap();
    advance_to_main1_from_game_start(&mut e);
    relocate_to_hand(&mut e, 0, "mind_rot");
    let fiery = relocate_to_hand(&mut e, 1, "fiery_temper");
    let land = relocate_to_hand(&mut e, 1, "mountain");
    give_mana(
        &mut e,
        0,
        ManaGift {
            b: 1,
            c: 2,
            ..Default::default()
        },
    );
    give_mana(
        &mut e,
        1,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&e, 0, "mind_rot");
    e.apply_command(0, &cast_spell(slot, target_player(1)))
        .unwrap();
    e.apply_command(0, &pass()).unwrap();
    e.apply_command(1, &pass()).unwrap();
    e.apply_command(1, &submit_resolution_choice(vec![fiery, land]))
        .unwrap();
    assert_eq!(e.state.objects[&fiery].zone, tricerules_core::Zone::Exile);
    e.apply_command(0, &pass()).unwrap();
    e.apply_command(1, &pass()).unwrap();
    let offer = e.state.pending_resolution.as_ref().expect("madness offer");
    let offer_id = offer.continuation.stack().unwrap().item.id as u64;
    let mut command = submit_resolution_decision(rv1::ResolutionChoiceDecision::CastSpell);
    if let Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(answer)) = &mut command.cmd {
        answer.cast_spell = Some(rv1::CastSpell {
            source: Some(rv1::CastSource {
                location: Some(rv1::cast_source::Location::ExileObjectId(fiery)),
                expected_zone_change_generation: Some(e.state.zone_change_generation[&fiery]),
            }),
            cast_method: 8,
            casting_permission_id: Some(offer_id),
            targets: target_player(0),
            ..Default::default()
        });
    }
    e.apply_command(1, &command).unwrap();
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.players[0].life, 17);
    assert_eq!(
        e.state.objects[&fiery].zone,
        tricerules_core::Zone::Graveyard
    );
}

fn madness_offer(card: &str) -> (GameEngine, u32, rv1::RuledEventBatch) {
    let mut e = GameEngine::new(
        19704,
        &[0, 1],
        20,
        Some(vec![
            deck_with("swamp", &["mind_rot"]),
            deck_with("mountain", &[card]),
        ]),
        true,
    )
    .unwrap();
    advance_to_main1_from_game_start(&mut e);
    relocate_to_hand(&mut e, 0, "mind_rot");
    let oid = relocate_to_hand(&mut e, 1, card);
    let land = relocate_to_hand(&mut e, 1, "mountain");
    give_mana(
        &mut e,
        0,
        ManaGift {
            b: 1,
            c: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&e, 0, "mind_rot");
    e.apply_command(0, &cast_spell(slot, target_player(1)))
        .unwrap();
    e.apply_command(0, &pass()).unwrap();
    e.apply_command(1, &pass()).unwrap();
    e.apply_command(1, &submit_resolution_choice(vec![oid, land]))
        .unwrap();
    e.apply_command(0, &pass()).unwrap();
    let batch = e.apply_command(1, &pass()).unwrap();
    (e, oid, batch)
}

fn offered_cast(e: &GameEngine, oid: u32, targets: Vec<rv1::TargetRef>) -> rv1::CastSpell {
    rv1::CastSpell {
        source: Some(rv1::CastSource {
            location: Some(rv1::cast_source::Location::ExileObjectId(oid)),
            expected_zone_change_generation: Some(e.state.zone_change_generation[&oid]),
        }),
        cast_method: rv1::CastMethod::Madness as i32,
        casting_permission_id: Some(
            e.state
                .pending_resolution
                .as_ref()
                .unwrap()
                .continuation
                .stack()
                .unwrap()
                .item
                .id as u64,
        ),
        targets,
        ..Default::default()
    }
}
fn accept_madness(cast: rv1::CastSpell) -> rv1::RuledCommand {
    rv1::RuledCommand {
        cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(
            rv1::SubmitResolutionChoice {
                decision: rv1::ResolutionChoiceDecision::CastSpell as i32,
                cast_spell: Some(cast),
                ..Default::default()
            },
        )),
    }
}

#[test]
fn issue_197_madness_publishes_recipient_scoped_targets_and_allows_mana_and_retry() {
    let (mut e, fiery, batch) = madness_offer("fiery_temper");
    let offered = &batch.legal_by_player[&1].zone_cast_actions;
    assert_eq!(offered.len(), 1);
    assert_eq!(offered[0].cast_method, rv1::CastMethod::Madness as i32);
    assert_eq!(
        offered[0].zone_change_generation,
        e.state.zone_change_generation[&fiery]
    );
    assert!(batch.legal_by_player[&0].zone_cast_actions.is_empty());
    assert!(e.state.active_exile_play_permissions.is_empty());
    let cast = offered_cast(&e, fiery, target_player(0));
    let before = format!("{:?}", e.state);
    assert!(e.apply_command(0, &accept_madness(cast.clone())).is_err());
    assert!(
        e.apply_command(1, &accept_madness(cast.clone())).is_err(),
        "cannot pay yet"
    );
    assert_eq!(format!("{:?}", e.state), before);
    let mountain = relocate_to_battlefield(&mut e, 1, "mountain", false);
    let batch = apply_ability(&mut e, 1, mountain, 0, vec![]).unwrap();
    assert_eq!(e.state.players[1].mana_pool.red, 1);
    assert_eq!(batch.legal_by_player[&1].zone_cast_actions.len(), 1);
    let undo = rv1::RuledCommand {
        cmd: Some(rv1::ruled_command::Cmd::UndoManaAbility(
            rv1::UndoManaAbility {},
        )),
    };
    e.apply_command(1, &undo).unwrap();
    assert_eq!(e.state.players[1].mana_pool.red, 0);
    apply_ability(&mut e, 1, mountain, 0, vec![]).unwrap();
    let mut missing_target = cast.clone();
    missing_target.targets.clear();
    let before = format!("{:?}", e.state);
    assert!(e.apply_command(1, &accept_madness(missing_target)).is_err());
    let mut stale = cast.clone();
    stale
        .source
        .as_mut()
        .unwrap()
        .expected_zone_change_generation = Some(0);
    assert!(e.apply_command(1, &accept_madness(stale)).is_err());
    assert_eq!(format!("{:?}", e.state), before);
    let mut contaminated = accept_madness(cast.clone());
    if let Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(choice)) = contaminated.cmd.as_mut()
    {
        choice.chosen_object_ids.push(fiery);
    }
    assert!(
        e.apply_command(1, &contaminated).is_err(),
        "special casts reject unrelated choice data"
    );
    assert_eq!(format!("{:?}", e.state), before);
    e.apply_command(1, &accept_madness(cast)).unwrap();
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.players[0].life, 17);
}

#[test]
fn issue_197_madness_creature_cast_ignores_normal_timing_and_decline_is_final() {
    let (mut e, wurm, _) = madness_offer("arrogant_wurm");
    give_mana(
        &mut e,
        1,
        ManaGift {
            g: 1,
            c: 2,
            ..Default::default()
        },
    );
    let cast = offered_cast(&e, wurm, vec![]);
    e.apply_command(1, &accept_madness(cast)).unwrap();
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.state.objects[&wurm].zone,
        tricerules_core::Zone::Battlefield
    );
    assert_eq!(e.state.objects[&wurm].power, Some(4));

    let (mut e, fiery, _) = madness_offer("fiery_temper");
    let cast = offered_cast(&e, fiery, target_player(0));
    e.apply_command(
        1,
        &submit_resolution_decision(rv1::ResolutionChoiceDecision::Decline),
    )
    .unwrap();
    assert_eq!(
        e.state.objects[&fiery].zone,
        tricerules_core::Zone::Graveyard
    );
    assert!(e.apply_command(1, &accept_madness(cast)).is_err());
}

#[test]
fn issue_197_madness_cost_discard_places_trigger_above_the_paid_spell() {
    let mut e = GameEngine::new(
        19705,
        &[0, 1],
        20,
        Some(vec![
            deck_with(
                "mountain",
                &["tormenting_voice", "fiery_temper", "library_of_leng"],
            ),
            deck_with("forest", &[]),
        ]),
        true,
    )
    .unwrap();
    advance_to_main1_from_game_start(&mut e);
    relocate_to_battlefield(&mut e, 0, "library_of_leng", false);
    let fiery = relocate_to_hand(&mut e, 0, "fiery_temper");
    relocate_to_hand(&mut e, 0, "tormenting_voice");
    give_mana(
        &mut e,
        0,
        ManaGift {
            r: 1,
            c: 1,
            ..Default::default()
        },
    );
    let cast = cast_spell_with_costs(
        hand_index_for_card(&e, 0, "tormenting_voice"),
        vec![],
        vec![hand_cost_selection(
            0,
            hand_index_for_card(&e, 0, "fiery_temper") as u32,
        )],
    );
    e.apply_command(0, &cast).unwrap();
    assert_eq!(e.state.objects[&fiery].zone, tricerules_core::Zone::Exile);
    assert!(
        e.state.pending_resolution.is_none(),
        "Library cannot replace a cost discard"
    );
    assert_eq!(e.state.stack.len(), 2);
    assert!(e.state.stack.last().unwrap().is_triggered);
    e.apply_command(0, &pass()).unwrap();
    e.apply_command(1, &pass()).unwrap();
    e.apply_command(
        0,
        &submit_resolution_decision(rv1::ResolutionChoiceDecision::Decline),
    )
    .unwrap();
    assert_eq!(
        e.state.objects[&fiery].zone,
        tricerules_core::Zone::Graveyard
    );
    assert_eq!(e.state.stack.len(), 1);
    resolve_entire_stack_two_player(&mut e);
}

#[test]
fn issue_197_madness_cleanup_opens_priority_then_requires_another_cleanup() {
    let mut e = GameEngine::new(
        19706,
        &[0, 1],
        20,
        Some(vec![
            deck_with("mountain", &["fiery_temper"]),
            deck_with("forest", &[]),
        ]),
        true,
    )
    .unwrap();
    advance_to_main1_from_game_start(&mut e);
    let fiery = relocate_to_hand(&mut e, 0, "fiery_temper");
    while e.state.players[0].hand.len() < 8 {
        relocate_to_hand(&mut e, 0, "mountain");
    }
    e.state.turn_step = tricerules_core::TurnStep::Cleanup;
    e.state.cleanup_discard_player = Some(0);
    let slot = hand_index_for_card(&e, 0, "fiery_temper") as u32;
    e.apply_command(0, &discard_cleanup(slot)).unwrap();
    assert_eq!(e.state.objects[&fiery].zone, tricerules_core::Zone::Exile);
    assert!(e.state.cleanup_priority_active);
    e.apply_command(0, &pass()).unwrap();
    e.apply_command(1, &pass()).unwrap();
    e.apply_command(
        0,
        &submit_resolution_decision(rv1::ResolutionChoiceDecision::Decline),
    )
    .unwrap();
    assert_eq!(e.state.turn_step, tricerules_core::TurnStep::Cleanup);
    e.apply_command(0, &pass()).unwrap();
    e.apply_command(1, &pass()).unwrap();
    assert_eq!(e.state.active_player_id(), 1);
}

#[test]
fn issue_197_library_and_madness_compete_before_any_discard_commits() {
    for destination in [1, 2] {
        let mut e = GameEngine::new(
            19707,
            &[0, 1],
            20,
            Some(vec![
                deck_with("swamp", &["mind_rot", "megrim"]),
                deck_with("mountain", &["fiery_temper", "library_of_leng"]),
            ]),
            true,
        )
        .unwrap();
        advance_to_main1_from_game_start(&mut e);
        relocate_to_battlefield(&mut e, 0, "megrim", false);
        relocate_to_battlefield(&mut e, 1, "library_of_leng", false);
        relocate_to_hand(&mut e, 0, "mind_rot");
        let fiery = relocate_to_hand(&mut e, 1, "fiery_temper");
        let land = relocate_to_hand(&mut e, 1, "mountain");
        give_mana(
            &mut e,
            0,
            ManaGift {
                b: 1,
                c: 2,
                ..Default::default()
            },
        );
        let slot = hand_index_for_card(&e, 0, "mind_rot");
        e.apply_command(0, &cast_spell(slot, target_player(1)))
            .unwrap();
        e.apply_command(0, &pass()).unwrap();
        e.apply_command(1, &pass()).unwrap();
        e.apply_command(1, &submit_resolution_choice(vec![fiery, land]))
            .unwrap();
        assert_eq!(
            e.state
                .pending_resolution
                .as_ref()
                .unwrap()
                .presentation
                .candidates,
            vec![2, 1]
        );
        let before = format!("{:?}", e.state);
        assert!(
            e.apply_command(1, &submit_resolution_choice(vec![0]))
                .is_err(),
            "madness cannot discard directly to graveyard"
        );
        assert!(e
            .apply_command(0, &submit_resolution_choice(vec![1]))
            .is_err());
        assert_eq!(format!("{:?}", e.state), before);
        e.apply_command(1, &submit_resolution_choice(vec![destination]))
            .unwrap();
        assert_eq!(e.state.objects[&fiery].zone, tricerules_core::Zone::Hand);
        assert_eq!(e.state.objects[&land].zone, tricerules_core::Zone::Hand);
        e.apply_command(1, &submit_resolution_choice(vec![0]))
            .unwrap();
        assert_eq!(
            e.state.objects[&fiery].zone,
            if destination == 1 {
                tricerules_core::Zone::Library
            } else {
                tricerules_core::Zone::Exile
            }
        );
        // Megrim is controlled by the active player. Its two triggers order first; madness is
        // the nonactive player's trigger and must be above them after APNAP ordering.
        while let Some(order) = e.state.pending_trigger_order.as_ref() {
            let player = order.deciding_player;
            let id = order.candidates[0].object_id;
            e.apply_command(
                player,
                &rv1::RuledCommand {
                    cmd: Some(rv1::ruled_command::Cmd::SubmitTriggerOrder(
                        rv1::SubmitTriggerOrder {
                            trigger_object_id: id,
                        },
                    )),
                },
            )
            .unwrap();
        }
        if destination == 2 {
            assert_eq!(e.state.stack.last().unwrap().controller, 1);
            e.apply_command(0, &pass()).unwrap();
            e.apply_command(1, &pass()).unwrap();
            e.apply_command(
                1,
                &submit_resolution_decision(rv1::ResolutionChoiceDecision::Decline),
            )
            .unwrap();
        }
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(e.state.players[1].life, 16);
    }
}

#[test]
fn issue_197_waste_not_uses_public_discard_types_and_land_mana_uses_the_stack() {
    for creature in [true, false] {
        let card = if creature {
            "grizzly_bears"
        } else {
            "mind_rot"
        };
        let mut e = GameEngine::new(
            19708,
            &[0, 1],
            20,
            Some(vec![
                deck_with("swamp", &["mind_rot", "waste_not"]),
                deck_with("forest", &[card]),
            ]),
            true,
        )
        .unwrap();
        advance_to_main1_from_game_start(&mut e);
        relocate_to_battlefield(&mut e, 0, "waste_not", false);
        relocate_to_hand(&mut e, 0, "mind_rot");
        let selected = relocate_to_hand(&mut e, 1, card);
        let land = relocate_to_hand(&mut e, 1, "forest");
        give_mana(
            &mut e,
            0,
            ManaGift {
                b: 1,
                c: 2,
                ..Default::default()
            },
        );
        let slot = hand_index_for_card(&e, 0, "mind_rot");
        e.apply_command(0, &cast_spell(slot, target_player(1)))
            .unwrap();
        e.apply_command(0, &pass()).unwrap();
        e.apply_command(1, &pass()).unwrap();
        let hand_before = e.state.players[0].hand.len();
        e.apply_command(1, &submit_resolution_choice(vec![selected, land]))
            .unwrap();
        assert_eq!(
            e.state.players[0].mana_pool.black, 0,
            "Waste Not is not a triggered mana ability"
        );
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(e.state.players[0].mana_pool.black, 2);
        assert_eq!(
            e.state.players[0].hand.len(),
            hand_before + usize::from(!creature)
        );
        let zombies = e.state.players[0]
            .battlefield
            .iter()
            .filter(|oid| e.state.objects[oid].card_id == "zombie_b_2_2")
            .count();
        assert_eq!(zombies, usize::from(creature));
    }
}

#[test]
fn issue_197_library_removal_restores_cleanup_hand_limit() {
    for keep in [true, false] {
        let mut e = GameEngine::new(
            19709,
            &[0, 1],
            20,
            Some(vec![
                deck_with("forest", &["library_of_leng"]),
                deck_with("forest", &[]),
            ]),
            true,
        )
        .unwrap();
        advance_to_main1_from_game_start(&mut e);
        let library = relocate_to_battlefield(&mut e, 0, "library_of_leng", false);
        while e.state.players[0].hand.len() < 9 {
            relocate_to_hand(&mut e, 0, "forest");
        }
        if !keep {
            e.enable_dev_commands();
            let cmd = rv1::RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::DevCommand(rv1::DevCommand {
                    target_player_id: 0,
                    dev: Some(rv1::dev_command::Dev::MoveCard(rv1::DevMoveCard {
                        card_name: "Library of Leng".into(),
                        zone: rv1::DevZone::Graveyard as i32,
                        ..Default::default()
                    })),
                })),
            };
            e.apply_command(0, &cmd).unwrap();
            assert_eq!(
                e.state.objects[&library].zone,
                tricerules_core::Zone::Graveyard
            );
        }
        e.state.turn_step = tricerules_core::TurnStep::EndStep;
        e.apply_command(0, &pass()).unwrap();
        e.apply_command(1, &pass()).unwrap();
        assert_eq!(e.state.cleanup_discard_player.is_some(), !keep);
        if keep {
            assert_eq!(e.state.active_player_id(), 1);
        }
    }
}

#[test]
fn issue_197_library_discard_then_draw_returns_the_ordered_top_card() {
    let mut e = GameEngine::new(
        19713,
        &[0, 1],
        20,
        Some(vec![
            deck_with("forest", &["library_of_leng", "keldon_raider"]),
            deck_with("forest", &[]),
        ]),
        true,
    )
    .unwrap();
    advance_to_main1_from_game_start(&mut e);
    relocate_to_battlefield(&mut e, 0, "library_of_leng", false);
    let forest = relocate_to_hand(&mut e, 0, "forest");
    let generation = e
        .state
        .zone_change_generation
        .get(&forest)
        .copied()
        .unwrap_or(0);
    move_ready_to_battlefield(&mut e, 0, "keldon_raider");
    e.apply_command(0, &pass()).unwrap();
    e.apply_command(1, &pass()).unwrap();
    e.apply_command(0, &submit_resolution_choice(vec![forest]))
        .unwrap();
    e.apply_command(0, &submit_resolution_choice(vec![1]))
        .unwrap();
    assert!(e.state.players[0].hand.contains(&forest));
    assert_eq!(e.state.zone_change_generation[&forest], generation + 2);
    assert!(e.state.pending_resolution.is_none());
}

#[test]
fn issue_197_random_library_replacements_replay_with_private_inspection_and_order() {
    fn setup() -> GameEngine {
        let mut e = GameEngine::new(
            19714,
            &[0, 1],
            20,
            Some(vec![
                deck_with("swamp", &["hymn_to_tourach"]),
                deck_with("forest", &["library_of_leng"]),
            ]),
            true,
        )
        .unwrap();
        advance_to_main1_from_game_start(&mut e);
        relocate_to_battlefield(&mut e, 1, "library_of_leng", false);
        relocate_to_hand(&mut e, 0, "hymn_to_tourach");
        give_mana(
            &mut e,
            0,
            ManaGift {
                b: 2,
                ..Default::default()
            },
        );
        e
    }
    let mut e = setup();
    let mut replay = setup();
    let slot = hand_index_for_card(&e, 0, "hymn_to_tourach");
    for (actor, command) in [
        (0, cast_spell(slot, target_player(1))),
        (0, pass()),
        (1, pass()),
    ] {
        assert_eq!(
            e.apply_command(actor, &command).unwrap(),
            replay.apply_command(actor, &command).unwrap()
        );
    }
    assert_eq!(
        e.state
            .pending_resolution
            .as_ref()
            .unwrap()
            .presentation
            .choice_kind,
        rv1::ChoiceKind::PrivateReplacement
    );
    for _ in 0..2 {
        let command = submit_resolution_choice(vec![1]);
        let batch = e.apply_command(1, &command).unwrap();
        assert_eq!(batch, replay.apply_command(1, &command).unwrap());
        assert!(batch.events.iter().any(|event| matches!(&event.ev,Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(choice)) if choice.candidate_names.iter().any(|name|name.contains("Forest")))));
    }
    let mut ordered = e
        .state
        .pending_resolution
        .as_ref()
        .unwrap()
        .presentation
        .candidates
        .clone();
    ordered.reverse();
    let command = submit_resolution_choice(ordered.clone());
    assert_eq!(
        e.apply_command(1, &command).unwrap(),
        replay.apply_command(1, &command).unwrap()
    );
    assert_eq!(
        e.state.players[1]
            .library
            .iter()
            .take(2)
            .copied()
            .collect::<Vec<_>>(),
        ordered
    );
}
