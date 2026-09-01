use super::helpers::*;
use tricerules_proto::ruled::v1 as rv1;
use tricerules_proto::ruled::v1::{
    cost_selection::Selection, ruled_command::Cmd, CostChoiceKind, CostObjectRef, CostObjectRefs,
    CostSelection, ResolutionChoiceDecision, RuledCommand, SubmitResolutionChoice,
};

#[test]
fn waterbend_vinebender_all_mana_activation_adds_one_counter() {
    let mut engine = GameEngine::new(
        146001,
        &[0, 1],
        20,
        Some(vec![
            deck_with("forest", &["foggy_swamp_vinebender"]),
            deck_with("island", &[]),
        ]),
        true,
    )
    .unwrap();
    advance_to_main1_from_game_start(&mut engine);
    let source = relocate_to_battlefield(&mut engine, 0, "foggy_swamp_vinebender", false);
    engine.state.players[0].mana_pool.colorless = 5;
    engine
        .apply_command(0, &activate_ability_with_costs(source, 0, vec![], vec![]))
        .expect("Waterbend can be paid entirely with mana");
    assert_eq!(engine.state.players[0].mana_pool.colorless, 0);
    assert!(!engine.state.objects[&source].tapped);
    assert_eq!(engine.state.stack.len(), 1);
    engine.apply_command(0, &pass()).unwrap();
    engine.apply_command(1, &pass()).unwrap();
    assert_eq!(engine.characteristics(source).unwrap().power, Some(5));
    assert_eq!(engine.state.stack.len(), 0);
}

fn tap_selection(cost_index: u32, objects: &[(u32, u64)]) -> CostSelection {
    CostSelection {
        cost_index,
        selection: Some(Selection::BattlefieldObjects(CostObjectRefs {
            objects: objects
                .iter()
                .map(|&(object_id, zone_change_generation)| CostObjectRef {
                    object_id,
                    zone_change_generation,
                })
                .collect(),
        })),
    }
}

fn select_branch(index: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
            decision: ResolutionChoiceDecision::SelectBranch as i32,
            selected_branch_index: index,
            ..Default::default()
        })),
    }
}

fn waterbend_ref(engine: &GameEngine, object_id: u32) -> CostObjectRef {
    CostObjectRef {
        object_id,
        zone_change_generation: engine
            .state
            .zone_change_generation
            .get(&object_id)
            .copied()
            .unwrap_or(0),
    }
}

#[test]
fn waterbend_payload_cannot_be_silently_ignored_by_a_mana_ability() {
    let mut engine = GameEngine::new(
        146010,
        &[0, 1],
        20,
        Some(vec![deck_with("island", &[]), deck_with("forest", &[])]),
        true,
    )
    .unwrap();
    advance_to_main1_from_game_start(&mut engine);
    let source = relocate_to_battlefield(&mut engine, 0, "island", false);
    let mut command = activate_ability_with_costs(source, 0, vec![], vec![]);
    let Some(Cmd::ActivateAbility(activation)) = command.cmd.as_mut() else {
        unreachable!()
    };
    activation.payment = Some(rv1::PaymentSelection::default());
    let before = format!("{:?}", engine.state);
    assert!(engine.apply_command(0, &command).is_err());
    assert_eq!(format!("{:?}", engine.state), before);
}

#[test]
fn waterbend_own_turn_timing_and_serialized_replay_ignore_preview_queries() {
    use prost::Message;
    fn run(previews: bool) -> (Vec<rv1::RuledEventBatch>, String) {
        let mut engine = GameEngine::new(
            146012,
            &[0, 1],
            20,
            Some(vec![
                deck_with("forest", &["foggy_swamp_vinebender"]),
                deck_with("island", &[]),
            ]),
            true,
        )
        .unwrap();
        advance_to_main1_from_game_start(&mut engine);
        let source = relocate_to_battlefield(&mut engine, 0, "foggy_swamp_vinebender", false);
        engine.state.players[0].mana_pool.colorless = 4;
        let mut command = rv1::ActivateAbility {
            source_object_id: source,
            expected_zone_change_generation: waterbend_ref(&engine, source).zone_change_generation,
            payment: Some(rv1::PaymentSelection {
                expected_state_revision: engine.state.command_index,
                source: Some(waterbend_ref(&engine, source)),
                waterbend: vec![waterbend_ref(&engine, source)],
                mana: Some(rv1::PaymentMana {
                    c: 4,
                    ..Default::default()
                }),
                ..Default::default()
            }),
            ..Default::default()
        };
        engine.state.active_player_idx = 1;
        let before = format!("{:?}", engine.state);
        assert!(engine
            .apply_command(
                0,
                &RuledCommand {
                    cmd: Some(Cmd::ActivateAbility(command.clone()))
                }
            )
            .is_err());
        assert_eq!(format!("{:?}", engine.state), before);
        engine.state.active_player_idx = 0;
        engine.state.turn_step = tricerules_core::TurnStep::BeginCombat;
        if previews {
            for _ in 0..3 {
                let response = engine.preview_payment(
                    0,
                    &rv1::PreviewPayment {
                        activate_ability: Some(command.clone()),
                        ..Default::default()
                    },
                );
                assert!(response.valid && response.complete, "{response:?}");
                command.payment = response.selection;
            }
        }
        let encoded = RuledCommand {
            cmd: Some(Cmd::ActivateAbility(command)),
        }
        .encode_to_vec();
        let replayed = RuledCommand::decode(encoded.as_slice()).unwrap();
        let batches = vec![
            engine.apply_command(0, &replayed).unwrap(),
            engine.apply_command(0, &pass()).unwrap(),
            engine.apply_command(1, &pass()).unwrap(),
        ];
        assert_eq!(engine.characteristics(source).unwrap().power, Some(5));
        // Debug maps are not ordered; compare stable public batches and the serialized command.
        (batches, format!("{encoded:?}"))
    }
    assert_eq!(run(true), run(false));
}

#[test]
fn waterbend_lesson_draws_before_payment_and_resumes_once() {
    for taps in 0..=2 {
        let mut engine = GameEngine::new(
            146003,
            &[0, 1],
            20,
            Some(vec![
                deck_with("island", &["waterbending_lesson"]),
                deck_with("forest", &[]),
            ]),
            true,
        )
        .expect("Waterbending Lesson must be registered");
        advance_to_main1_from_game_start(&mut engine);
        let objects = [
            inject_creature_on_battlefield(&mut engine, 0, "goldvein_pick"),
            inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears"),
        ];
        ensure_in_hand(&mut engine, 0, "waterbending_lesson");
        engine.state.players[0].mana_pool.blue = 4;
        let slot = hand_index_for_card(&engine, 0, "waterbending_lesson");
        engine.apply_command(0, &cast_spell(slot, vec![])).unwrap();
        let before_draw = engine.state.players[0].hand.len();
        engine.apply_command(0, &pass()).unwrap();
        engine.apply_command(1, &pass()).unwrap();
        assert_eq!(engine.state.players[0].hand.len(), before_draw + 3);
        engine.apply_command(0, &select_branch(0)).unwrap();
        assert!(
            engine
                .state
                .pending_resolution
                .as_ref()
                .unwrap()
                .continuation
                .mana_payment()
                .unwrap()
                .waterbend
        );
        engine.state.players[0].mana_pool.blue = 2;
        let mut answer = rv1::SubmitResolutionChoice {
            decision: rv1::ResolutionChoiceDecision::PayMana as i32,
            payment: Some(rv1::PaymentSelection {
                waterbend: objects[..taps]
                    .iter()
                    .map(|id| waterbend_ref(&engine, *id))
                    .collect(),
                mana: Some(rv1::PaymentMana {
                    u: 2 - taps as u32,
                    ..Default::default()
                }),
                ..Default::default()
            }),
            ..Default::default()
        };
        let before = format!("{:?}", engine.state);
        let preview = engine.preview_payment(
            0,
            &rv1::PreviewPayment {
                resolution_choice: Some(answer.clone()),
                ..Default::default()
            },
        );
        assert!(preview.valid && preview.complete, "{preview:?}");
        assert_eq!(format!("{:?}", engine.state), before);
        answer.payment = preview.selection;
        let mut stale = answer.clone();
        stale.payment.as_mut().unwrap().expected_state_revision += 1;
        assert!(engine
            .apply_command(
                0,
                &RuledCommand {
                    cmd: Some(Cmd::SubmitResolutionChoice(stale))
                }
            )
            .is_err());
        assert_eq!(
            format!("{:?}", engine.state),
            before,
            "rejection must preserve the parked choice"
        );
        let command = RuledCommand {
            cmd: Some(Cmd::SubmitResolutionChoice(answer)),
        };
        engine.apply_command(0, &command).unwrap();
        assert_eq!(engine.state.players[0].hand.len(), before_draw + 3);
        assert!(engine.state.pending_resolution.is_none());
        assert!(engine.state.stack.is_empty());
        assert!(engine.apply_command(0, &command).is_err());
    }
}

#[test]
fn waterbend_lesson_decline_restores_branch_without_drawing_again() {
    let mut engine = GameEngine::new(
        146011,
        &[0, 1],
        20,
        Some(vec![
            deck_with("island", &["waterbending_lesson"]),
            deck_with("forest", &[]),
        ]),
        true,
    )
    .unwrap();
    advance_to_main1_from_game_start(&mut engine);
    let island = relocate_to_battlefield(&mut engine, 0, "island", false);
    ensure_in_hand(&mut engine, 0, "waterbending_lesson");
    engine.state.players[0].mana_pool.blue = 4;
    let slot = hand_index_for_card(&engine, 0, "waterbending_lesson");
    engine.apply_command(0, &cast_spell(slot, vec![])).unwrap();
    let before_draw = engine.state.players[0].hand.len();
    engine.apply_command(0, &pass()).unwrap();
    engine.apply_command(1, &pass()).unwrap();
    engine.apply_command(0, &select_branch(0)).unwrap();
    engine
        .apply_command(0, &activate_ability_with_costs(island, 0, vec![], vec![]))
        .unwrap();
    assert!(engine.state.objects[&island].tapped);
    assert_eq!(engine.state.players[0].mana_pool.blue, 1);
    let decline = RuledCommand {
        cmd: Some(Cmd::SubmitResolutionChoice(rv1::SubmitResolutionChoice {
            decision: rv1::ResolutionChoiceDecision::Decline as i32,
            ..Default::default()
        })),
    };
    let branch = engine.apply_command(0, &decline).unwrap();
    let branch = find_resolution_choice(&branch).unwrap();
    assert_eq!(branch.choice_kind(), rv1::ChoiceKind::ResolutionBranch);
    assert_eq!(branch.resolution_branches.len(), 2);
    for (option, expected_id) in branch
        .resolution_branches
        .iter()
        .zip(["waterbend_2", "discard_a_card"])
    {
        let presentation = option
            .presentation
            .as_ref()
            .expect("stable resolution branch identity");
        assert_eq!(presentation.card_id, "waterbending_lesson");
        assert_eq!(presentation.face_id, "waterbending_lesson");
        assert!(presentation.oracle_line_indices.is_empty());
        assert_eq!(presentation.path.last().unwrap().id, expected_id);
    }
    assert!(!engine.state.objects[&island].tapped);
    assert_eq!(engine.state.players[0].mana_pool.blue, 0);
    assert_eq!(engine.state.players[0].hand.len(), before_draw + 3);
    let discard = engine.apply_command(0, &select_branch(1)).unwrap();
    let choice = find_resolution_choice(&discard).unwrap();
    assert_eq!(choice.choice_kind(), rv1::ChoiceKind::HandCards);
    let chosen = choice.candidate_object_ids[0];
    engine
        .apply_command(0, &submit_resolution_choice(vec![chosen]))
        .unwrap();
    assert_eq!(engine.state.players[0].hand.len(), before_draw + 2);
    assert!(engine.state.pending_resolution.is_none());
}

#[test]
fn waterbend_preview_and_commit_share_exact_mixed_payment_and_reject_stale_input() {
    for tap_count in 0..=5 {
        let mut engine = GameEngine::new(
            146002,
            &[0, 1],
            20,
            Some(vec![
                deck_with("forest", &["foggy_swamp_vinebender"]),
                deck_with("island", &[]),
            ]),
            true,
        )
        .unwrap();
        advance_to_main1_from_game_start(&mut engine);
        let source = relocate_to_battlefield(&mut engine, 0, "foggy_swamp_vinebender", false);
        let objects = [
            source,
            inject_creature_on_battlefield(&mut engine, 0, "goldvein_pick"),
            inject_creature_on_battlefield(&mut engine, 0, "ornithopter"),
            inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears"),
            inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears"),
        ];
        let opponent = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
        engine.state.players[0].mana_pool.colorless = 5;
        let mut activation = rv1::ActivateAbility {
            source_object_id: source,
            expected_zone_change_generation: waterbend_ref(&engine, source).zone_change_generation,
            payment: Some(rv1::PaymentSelection {
                waterbend: objects[..tap_count]
                    .iter()
                    .map(|oid| waterbend_ref(&engine, *oid))
                    .collect(),
                mana: Some(rv1::PaymentMana {
                    c: 5 - tap_count as u32,
                    ..Default::default()
                }),
                ..Default::default()
            }),
            ..Default::default()
        };
        let before = format!("{:?}", engine.state);
        let preview = engine.preview_payment(
            0,
            &rv1::PreviewPayment {
                activate_ability: Some(activation.clone()),
                ..Default::default()
            },
        );
        assert!(
            preview.valid && preview.complete,
            "tap count {tap_count}: {preview:?}"
        );
        assert_eq!(format!("{:?}", engine.state), before);
        assert!(!preview
            .candidates
            .iter()
            .any(|c| c.object.as_ref().unwrap().object_id == opponent));
        activation.payment = preview.selection;
        let valid = RuledCommand {
            cmd: Some(Cmd::ActivateAbility(activation.clone())),
        };
        for corruption in 0..6 {
            let mut invalid = activation.clone();
            let payment = invalid.payment.as_mut().unwrap();
            match corruption {
                0 => payment.expected_state_revision += 1,
                1 => payment.waterbend.push(waterbend_ref(&engine, opponent)),
                2 => payment.mana.as_mut().unwrap().c += 1,
                3 => {
                    payment.waterbend = vec![waterbend_ref(&engine, source); 2];
                    payment.mana.as_mut().unwrap().c = 3;
                }
                4 => {
                    let mut stale = waterbend_ref(&engine, source);
                    stale.zone_change_generation += 1;
                    payment.waterbend = vec![stale];
                    payment.mana.as_mut().unwrap().c = 4;
                }
                _ => {
                    payment.convoke.push(rv1::ObjectPaymentContribution {
                        object: Some(waterbend_ref(&engine, source)),
                        kind: rv1::ObjectPaymentKind::Generic as i32,
                    });
                }
            }
            assert!(engine
                .apply_command(
                    0,
                    &RuledCommand {
                        cmd: Some(Cmd::ActivateAbility(invalid))
                    }
                )
                .is_err());
            assert_eq!(format!("{:?}", engine.state), before);
        }
        engine.apply_command(0, &valid).unwrap();
        for (index, oid) in objects.into_iter().enumerate() {
            assert_eq!(engine.state.objects[&oid].tapped, index < tap_count);
        }
        assert_eq!(
            engine.state.players[0].mana_pool.colorless,
            tap_count as u32
        );
        assert!(!engine.state.objects[&opponent].tapped);
        assert_eq!(engine.state.stack.len(), 1);
        engine.apply_command(0, &pass()).unwrap();
        engine.apply_command(1, &pass()).unwrap();
        assert_eq!(engine.characteristics(source).unwrap().power, Some(5));
    }
}

#[test]
fn gene_pollinator_publishes_and_atomically_pays_another_untapped_permanent() {
    let decks = Some(vec![
        deck_with("forest", &["gene_pollinator", "grizzly_bears"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(144_001, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    let gene = relocate_to_battlefield(&mut engine, 0, "gene_pollinator", false);
    let bear = relocate_to_battlefield(&mut engine, 0, "grizzly_bears", false);
    engine.state.objects.get_mut(&gene).unwrap().summoning_sick = false;
    engine.state.objects.get_mut(&bear).unwrap().summoning_sick = true;
    let generation = engine
        .state
        .zone_change_generation
        .get(&bear)
        .copied()
        .unwrap_or(0);

    let legal = engine.initial_response_batch();
    let key = u64::from(gene) << 32;
    let choices = &legal.legal_by_player[&0].cost_choices_by_ability[&key];
    assert!(choices.non_mana_costs_payable);
    assert_eq!(choices.choices.len(), 1);
    assert_eq!(choices.choices[0].kind(), CostChoiceKind::Tap);
    assert_eq!(choices.choices[0].candidate_ids, [bear]);
    let candidate = choices.choices[0].candidate_objects[0]
        .object
        .as_ref()
        .expect("generation-bound tap candidate");
    assert_eq!(candidate.object_id, bear);
    assert_eq!(candidate.zone_change_generation, generation);

    engine
        .apply_command(
            0,
            &activate_ability_with_costs(
                gene,
                0,
                vec![],
                vec![tap_selection(1, &[(bear, generation)])],
            ),
        )
        .expect("tap Gene and a summoning-sick permanent as the separate payment");
    assert!(engine.state.objects[&gene].tapped);
    assert!(engine.state.objects[&bear].tapped);
    let pool = engine.state.players[0].mana_pool;
    assert_eq!(
        pool.white + pool.blue + pool.black + pool.red + pool.green + pool.colorless,
        1
    );
}

#[test]
fn stale_or_duplicate_tap_selection_rejects_without_partial_taps() {
    let decks = Some(vec![
        deck_with("forest", &["gene_pollinator", "grizzly_bears"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(144_002, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    let gene = relocate_to_battlefield(&mut engine, 0, "gene_pollinator", false);
    let bear = relocate_to_battlefield(&mut engine, 0, "grizzly_bears", false);
    engine.state.objects.get_mut(&gene).unwrap().summoning_sick = false;
    let generation = engine
        .state
        .zone_change_generation
        .get(&bear)
        .copied()
        .unwrap_or(0);

    let stale = activate_ability_with_costs(
        gene,
        0,
        vec![],
        vec![tap_selection(1, &[(bear, generation + 1)])],
    );
    engine
        .apply_command(0, &stale)
        .expect_err("stale generation must fail");
    assert!(!engine.state.objects[&gene].tapped);
    assert!(!engine.state.objects[&bear].tapped);

    let duplicate = activate_ability_with_costs(
        gene,
        0,
        vec![],
        vec![tap_selection(1, &[(bear, generation), (bear, generation)])],
    );
    engine
        .apply_command(0, &duplicate)
        .expect_err("duplicate object must fail");
    assert!(!engine.state.objects[&gene].tapped);
    assert!(!engine.state.objects[&bear].tapped);
}

#[test]
fn gravelgill_scoundrel_uses_a_private_generation_bound_resolution_payment() {
    let decks = Some(vec![
        deck_with("island", &["gravelgill_scoundrel", "grizzly_bears"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(144_003, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    let scoundrel = relocate_to_battlefield(&mut engine, 0, "gravelgill_scoundrel", false);
    let bear = relocate_to_battlefield(&mut engine, 0, "grizzly_bears", false);
    engine
        .state
        .objects
        .get_mut(&scoundrel)
        .unwrap()
        .summoning_sick = false;
    engine.state.objects.get_mut(&bear).unwrap().summoning_sick = true;
    engine
        .apply_command(0, &primitive_yield())
        .expect("main phase to beginning of combat");
    engine.apply_command(0, &pass()).expect("attacker passes");
    engine.apply_command(1, &pass()).expect("defender passes");
    engine
        .apply_command(0, &declare_attackers(vec![scoundrel]))
        .expect("attack and stage trigger");
    engine.apply_command(0, &pass()).expect("attacker passes");
    let branch_batch = engine.apply_command(1, &pass()).expect("trigger resolves");
    let branch = find_resolution_choice(&branch_batch).expect("resolution branch");
    assert_eq!(
        branch.choice_kind(),
        tricerules_proto::ruled::v1::ChoiceKind::ResolutionBranch
    );

    let payment_batch = engine
        .apply_command(0, &select_branch(0))
        .expect("select tap-payment branch");
    let payment = find_resolution_choice(&payment_batch).expect("tap payment");
    assert_eq!(
        payment.choice_kind(),
        tricerules_proto::ruled::v1::ChoiceKind::CostObjects
    );
    assert_eq!(payment.candidate_object_ids, [bear]);
    assert_eq!(payment.min, 0);
    assert_eq!(payment.max, 1);
    assert!(payment.prompt_text.contains("or decline"));

    engine
        .apply_command(0, &submit_resolution_choice(vec![bear]))
        .expect("tap the other creature and resume");
    assert!(engine.state.objects[&bear].tapped);
    assert!(
        !engine.state.objects[&scoundrel].tapped,
        "vigilance keeps attacker untapped"
    );
}

#[test]
fn command_bridge_taps_a_physical_permanent_or_sacrifices_itself() {
    let decks = Some(vec![
        deck_with("forest", &["command_bridge", "grizzly_bears"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(144_004, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    let bear = relocate_to_battlefield(&mut engine, 0, "grizzly_bears", false);
    engine.state.objects.get_mut(&bear).unwrap().summoning_sick = true;
    ensure_in_hand(&mut engine, 0, "command_bridge");
    let slot = hand_index_for_card(&engine, 0, "command_bridge");
    let bridge = engine.state.players[0].hand[slot];
    engine
        .apply_command(0, &play_land(slot))
        .expect("play Command Bridge");
    assert!(engine.state.objects[&bridge].tapped);
    engine.apply_command(0, &pass()).expect("controller passes");
    let branch_batch = engine.apply_command(1, &pass()).expect("ETB resolves");
    let branch = find_resolution_choice(&branch_batch).expect("tap or sacrifice branch");
    assert_eq!(branch.resolution_branches.len(), 2);
    let payment_batch = engine
        .apply_command(0, &select_branch(0))
        .expect("choose tap payment");
    let payment = find_resolution_choice(&payment_batch).expect("tap candidate prompt");
    assert_eq!(payment.candidate_object_ids, [bear]);
    assert_eq!(payment.min, 1);
    assert!(!payment.prompt_text.contains("decline"));
    engine
        .apply_command(0, &submit_resolution_choice(vec![bear]))
        .expect("tap creature");
    assert!(engine.state.objects[&bear].tapped);
    assert_eq!(
        engine.state.objects[&bridge].zone,
        tricerules_core::Zone::Battlefield
    );

    let decks = Some(vec![
        deck_with("forest", &["command_bridge"]),
        deck_with("forest", &[]),
    ]);
    let mut fallback = GameEngine::new(144_005, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut fallback);
    ensure_in_hand(&mut fallback, 0, "command_bridge");
    let slot = hand_index_for_card(&fallback, 0, "command_bridge");
    let bridge = fallback.state.players[0].hand[slot];
    fallback
        .apply_command(0, &play_land(slot))
        .expect("play Command Bridge");
    fallback
        .apply_command(0, &pass())
        .expect("controller passes");
    fallback
        .apply_command(1, &pass())
        .expect("fallback sacrifices Bridge");
    assert_eq!(
        fallback.state.objects[&bridge].zone,
        tricerules_core::Zone::Graveyard
    );
}

#[test]
fn untapped_command_bridge_can_pay_for_itself_or_remain_untapped() {
    for choose_bridge in [true, false] {
        let decks = Some(vec![
            deck_with("forest", &["command_bridge", "grizzly_bears"]),
            deck_with("forest", &[]),
        ]);
        let mut engine = GameEngine::new(144_006, &[0, 1], 20, decks, true).expect("engine");
        advance_to_main1_from_game_start(&mut engine);
        let bear = relocate_to_battlefield(&mut engine, 0, "grizzly_bears", false);
        ensure_in_hand(&mut engine, 0, "command_bridge");
        let slot = hand_index_for_card(&engine, 0, "command_bridge");
        let bridge = engine.state.players[0].hand[slot];
        engine
            .apply_command(0, &play_land(slot))
            .expect("play Bridge");
        // Model an untap before the entry trigger resolves. Its text does not say "another".
        engine.state.objects.get_mut(&bridge).unwrap().tapped = false;
        engine.apply_command(0, &pass()).expect("controller passes");
        engine
            .apply_command(1, &pass())
            .expect("entry trigger resolves");
        let payment_batch = engine
            .apply_command(0, &select_branch(0))
            .expect("tap branch");
        let payment = find_resolution_choice(&payment_batch).expect("payment choice");
        assert!(payment.candidate_object_ids.contains(&bridge));
        assert!(payment.candidate_object_ids.contains(&bear));
        let chosen = if choose_bridge { bridge } else { bear };
        engine
            .apply_command(0, &submit_resolution_choice(vec![chosen]))
            .expect("pay only the chosen tap cost");
        assert_eq!(
            engine.state.objects[&bridge].zone,
            tricerules_core::Zone::Battlefield
        );
        assert_eq!(engine.state.objects[&bridge].tapped, choose_bridge);
        assert_eq!(engine.state.objects[&bear].tapped, !choose_bridge);
    }
}
