use crate::helpers::*;
use tricerules_proto::ruled::v1 as rv1;

fn damage_mana_engine(seed: u64, life: i32, card_id: &str) -> (GameEngine, u32) {
    let mut engine = GameEngine::new(seed, &[0, 1], 20, None, true).expect("new engine");
    advance_to_main1_from_game_start(&mut engine);
    engine.state.players[0].life = life;
    let source = inject_permanent_on_battlefield(&mut engine, 0, card_id);
    (engine, source)
}

fn mana_snapshot(engine: &GameEngine, player: usize) -> [u32; 6] {
    let pool = engine.state.players[player].mana_pool;
    [
        pool.colorless,
        pool.white,
        pool.blue,
        pool.black,
        pool.red,
        pool.green,
    ]
}

fn activate_mana_option(
    engine: &mut GameEngine,
    player: i32,
    source: u32,
    ability_index: u32,
    option_index: u32,
) -> RuledEventBatch {
    let mut command = activate_ability_for(engine, source, ability_index, vec![]);
    let Some(Cmd::ActivateAbility(activation)) = command.cmd.as_mut() else {
        unreachable!("mana activation command")
    };
    activation.mana_option_index = option_index;
    engine
        .apply_command(player, &command)
        .expect("selected mana ability resolves")
}

fn begin_spell_payment(engine: &mut GameEngine, player: i32, card_id: &str) -> u64 {
    inject_card_into_hand(engine, player as usize, card_id);
    let slot = hand_index_for_card(engine, player as usize, card_id);
    let Some(Cmd::CastSpell(cast)) = cast_spell(slot, vec![]).cmd else {
        unreachable!("cast announcement")
    };
    let announcement = rv1::SpellCastAnnouncement {
        targets: cast.targets,
        x_value: cast.x_value,
        flex_payments: cast.flex_payments,
        face_index: cast.face_index,
        selected_modes: cast.selected_modes,
        source: cast.source,
        cost_selections: cast.cost_selections,
        cast_cost_group_selections: cast.cast_cost_group_selections,
        cast_method: cast.cast_method,
        casting_permission_id: cast.casting_permission_id,
    };
    engine
        .apply_command(
            player,
            &RuledCommand {
                cmd: Some(Cmd::BeginSpellCast(rv1::BeginSpellCast {
                    announcement: Some(announcement),
                })),
            },
        )
        .expect("begin spell payment");
    engine
        .state
        .pending_spell_cast
        .as_ref()
        .expect("spell payment remains open")
        .transaction_id
}

fn commit_green_spell(
    engine: &mut GameEngine,
    player: i32,
    transaction_id: u64,
) -> RuledEventBatch {
    let proposed = rv1::CommitSpellCast {
        transaction_id,
        payment: Some(rv1::PaymentSelection {
            mana: Some(rv1::PaymentMana {
                g: 1,
                ..Default::default()
            }),
            ..Default::default()
        }),
        ..Default::default()
    };
    let preview = engine.preview_payment(
        player,
        &rv1::PreviewPayment {
            transaction_id,
            revision: engine.state.command_index,
            commit_spell_cast: Some(proposed),
            ..Default::default()
        },
    );
    assert!(preview.valid && preview.complete, "{preview:?}");
    engine
        .apply_command(
            player,
            &RuledCommand {
                cmd: Some(Cmd::CommitSpellCast(rv1::CommitSpellCast {
                    transaction_id,
                    payment: preview.selection,
                    restricted_mana: preview.restricted_mana,
                })),
            },
        )
        .expect("commit spell payment")
}

#[test]
fn issue_494_each_in_scope_pain_mana_choice_resolves_without_stack_or_undo() {
    let cases = [
        ("talisman_of_impulse", 0, 0, [1, 0, 0, 0, 0, 0], 20, 1),
        ("talisman_of_impulse", 1, 0, [0, 0, 0, 0, 1, 0], 19, 0),
        ("talisman_of_impulse", 1, 1, [0, 0, 0, 0, 0, 1], 19, 0),
        ("karplusan_forest", 0, 0, [1, 0, 0, 0, 0, 0], 20, 1),
        ("karplusan_forest", 1, 0, [0, 0, 0, 0, 1, 0], 19, 0),
        ("karplusan_forest", 1, 1, [0, 0, 0, 0, 0, 1], 19, 0),
        ("yavimaya_coast", 0, 0, [1, 0, 0, 0, 0, 0], 20, 1),
        ("yavimaya_coast", 1, 0, [0, 0, 0, 0, 0, 1], 19, 0),
        ("yavimaya_coast", 1, 1, [0, 0, 1, 0, 0, 0], 19, 0),
    ];

    for (index, (card_id, ability, option, expected_mana, expected_life, expected_undo)) in
        cases.into_iter().enumerate()
    {
        let (mut engine, source) = damage_mana_engine(494_001 + index as u64, 20, card_id);
        let mut command = activate_ability_for(&engine, source, ability, vec![]);
        let Some(Cmd::ActivateAbility(activation)) = command.cmd.as_mut() else {
            unreachable!("mana activation command")
        };
        activation.mana_option_index = option;

        let batch = engine
            .apply_command(0, &command)
            .expect("the selected mana option is legal");

        assert_eq!(
            mana_snapshot(&engine, 0),
            expected_mana,
            "{card_id} option {option}"
        );
        assert_eq!(
            engine.state.players[0].life, expected_life,
            "life after {card_id} ability {ability}"
        );
        let source_generation = engine
            .state
            .zone_change_generation
            .get(&source)
            .copied()
            .unwrap_or(0);
        assert_eq!(
            engine
                .state
                .turn_history
                .current
                .dealt_damage_objects
                .contains(&(source, source_generation)),
            expected_life == 19,
            "only unprevented self-damage reaches committed damage history"
        );
        let life_changes = batch
            .events
            .iter()
            .filter_map(|event| match &event.ev {
                Some(Ev::LifeChanged(change)) if change.player_id == 0 => Some(change.delta),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            life_changes,
            if expected_life == 19 {
                &[-1][..]
            } else {
                &[][..]
            },
            "the shared damage pipeline commits only the printed damage"
        );
        assert!(
            engine.state.objects[&source].tapped,
            "{card_id} pays its tap cost"
        );
        assert!(
            engine.state.stack.is_empty(),
            "mana ability never enters the stack"
        );
        assert_eq!(engine.state.priority_player_id(), 0);
        assert!(engine.state.pending_resolution.is_none());
        assert_eq!(
            batch.legal_by_player[&0].undoable_mana_abilities,
            expected_undo
        );
        assert!(batch.events.iter().all(|event| !matches!(
            event.ev,
            Some(Ev::StackPushed(_)) | Some(Ev::StackResolved(_))
        )));
    }
}

#[test]
fn issue_494_pure_talisman_mana_remains_undoable_and_damage_activation_invalidates_prior_float() {
    let (mut engine, talisman) = damage_mana_engine(494_010, 20, "talisman_of_impulse");
    activate_mana_option(&mut engine, 0, talisman, 0, 0);
    assert_eq!(mana_snapshot(&engine, 0), [1, 0, 0, 0, 0, 0]);
    assert_eq!(engine.state.undoable_mana_abilities.len(), 1);
    engine
        .apply_command(0, &undo_mana_ability())
        .expect("pure colorless Talisman mana remains undoable");
    assert_eq!(mana_snapshot(&engine, 0), [0, 0, 0, 0, 0, 0]);
    assert!(!engine.state.objects[&talisman].tapped);

    let float_source = inject_permanent_on_battlefield(&mut engine, 0, "mountain");
    activate_mana_option(&mut engine, 0, float_source, 0, 0);
    assert_eq!(engine.state.undoable_mana_abilities.len(), 1);
    activate_mana_option(&mut engine, 0, talisman, 1, 1);
    assert_eq!(engine.state.players[0].life, 19);
    assert!(
        engine.state.undoable_mana_abilities.is_empty(),
        "the consequential mana ability invalidates earlier float undo"
    );
    assert!(engine
        .apply_command(0, &undo_mana_ability())
        .expect_err("no mana activation remains undoable")
        .to_string()
        .contains("no mana ability to undo"));
}

#[test]
fn issue_494_prevention_choice_preserves_source_identity_and_resumes_once() {
    let (mut engine, source) = damage_mana_engine(494_020, 20, "talisman_of_impulse");
    engine.state.zone_change_generation.insert(source, 7);
    engine.state.add_damage_prevention_shield(0, 1);
    engine.state.add_damage_prevention_shield(0, 1);

    let activation = activate_mana_option(&mut engine, 0, source, 1, 0);
    assert_eq!(mana_snapshot(&engine, 0), [0, 0, 0, 0, 1, 0]);
    assert_eq!(
        engine.state.players[0].life, 20,
        "damage waits for the prevention choice"
    );
    assert_eq!(engine.state.priority_player_id(), 0);
    assert!(activation
        .events
        .iter()
        .all(|event| !matches!(event.ev, Some(Ev::LifeChanged(_)))));
    assert!(engine.state.stack.is_empty());
    assert!(activation.events.iter().all(|event| !matches!(
        event.ev,
        Some(Ev::StackPushed(_)) | Some(Ev::StackResolved(_))
    )));

    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("prevention choice");
    assert_eq!(pending.deciding_player, 0);
    assert_eq!(pending.presentation.source_object_id, source);
    let ResolutionContinuation::ManaAbilityDamageReplacement {
        actor,
        source_object_id,
        source_zone_change_generation,
        ..
    } = &pending.continuation
    else {
        panic!("stackless mana-damage continuation")
    };
    assert_eq!(*actor, 0);
    assert_eq!(*source_object_id, source);
    assert_eq!(*source_zone_change_generation, 7);
    let choice_id = pending.presentation.candidates[0];

    let answer = RuledCommand {
        cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
            chosen_object_ids: vec![choice_id],
            ..Default::default()
        })),
    };
    assert!(
        engine.apply_command(1, &answer).is_err(),
        "only the affected player chooses"
    );
    let resolved = engine
        .apply_command(0, &answer)
        .expect("resume the stackless damage event");
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(
        engine.state.players[0].life, 20,
        "the shield prevents this damage once"
    );
    let source_generation = engine
        .state
        .zone_change_generation
        .get(&source)
        .copied()
        .unwrap_or(0);
    assert!(
        !engine
            .state
            .turn_history
            .current
            .dealt_damage_objects
            .contains(&(source, source_generation)),
        "fully prevented damage does not enter the damage-trigger/history pipeline"
    );
    assert!(resolved.events.iter().all(|event| match &event.ev {
        Some(Ev::LifeChanged(change)) => change.delta == 0,
        _ => true,
    }));
    assert_eq!(engine.state.remaining_damage_prevention(0), 1);
    assert!(engine.state.stack.is_empty());
    assert!(resolved.events.iter().all(|event| !matches!(
        event.ev,
        Some(Ev::StackPushed(_)) | Some(Ev::StackResolved(_))
    )));
    assert!(engine
        .apply_command(0, &answer)
        .expect_err("the completed damage choice cannot be replayed")
        .to_string()
        .contains("no resolution awaiting a choice"));
}

#[test]
fn issue_494_yavimaya_coast_damage_source_is_colorless() {
    let (mut engine, source) = damage_mana_engine(494_025, 20, "yavimaya_coast");
    engine.state.add_damage_prevention_shield(0, 1);
    engine.state.add_damage_prevention_shield(0, 1);

    activate_mana_option(&mut engine, 0, source, 1, 0);

    let state = serde_json::to_value(&engine.state).expect("serialize pending game state");
    let damage = &state["pending_replacement_event"]["Damage"]["damage"][0];
    let damage_source = &damage["spec"]["event"]["source"];
    assert_eq!(damage_source["object_id"], source);
    assert_eq!(damage_source["colors"], serde_json::json!([]));
    assert_eq!(damage_source["types"], serde_json::json!(["Land"]));

    let choice_id = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("prevention choice")
        .presentation
        .candidates[0];
    engine
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
                    chosen_object_ids: vec![choice_id],
                    ..Default::default()
                })),
            },
        )
        .expect("resolve the colorless source's damage");
    assert_eq!(engine.state.players[0].life, 20);
    assert!(engine.state.stack.is_empty());
}

#[test]
fn issue_494_prevention_choice_keeps_spell_payment_open_until_damage_finishes() {
    let (mut engine, source) = damage_mana_engine(494_030, 20, "talisman_of_impulse");
    engine.state.add_damage_prevention_shield(0, 1);
    engine.state.add_damage_prevention_shield(0, 1);
    let transaction_id = begin_spell_payment(&mut engine, 0, "elvish_mystic");
    let activation = activate_mana_option(&mut engine, 0, source, 1, 1);
    assert!(engine.state.pending_spell_cast.is_some());
    assert_eq!(engine.state.players[0].mana_pool.green, 1);
    assert_eq!(engine.state.players[0].life, 20);
    assert_eq!(engine.state.priority_player_id(), 0);
    assert!(engine.state.stack.is_empty());
    let choice_id = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("damage prevention choice")
        .presentation
        .candidates[0];

    let proposal = rv1::CommitSpellCast {
        transaction_id,
        payment: Some(rv1::PaymentSelection {
            mana: Some(rv1::PaymentMana {
                g: 1,
                ..Default::default()
            }),
            ..Default::default()
        }),
        ..Default::default()
    };
    let preview = engine.preview_payment(
        0,
        &rv1::PreviewPayment {
            transaction_id,
            revision: engine.state.command_index,
            commit_spell_cast: Some(proposal),
            ..Default::default()
        },
    );
    assert!(preview.valid && preview.complete, "{preview:?}");
    let commit = RuledCommand {
        cmd: Some(Cmd::CommitSpellCast(rv1::CommitSpellCast {
            transaction_id,
            payment: preview.selection,
            restricted_mana: preview.restricted_mana,
        })),
    };
    let cancel = RuledCommand {
        cmd: Some(Cmd::CancelSpellCast(rv1::CancelSpellCast {
            transaction_id,
        })),
    };
    let pending_choice = format!("{:?}", engine.state.pending_resolution);
    assert!(engine.apply_command(0, &commit).is_err());
    assert_eq!(
        format!("{:?}", engine.state.pending_resolution),
        pending_choice
    );
    assert!(engine.apply_command(0, &cancel).is_err());
    assert_eq!(
        format!("{:?}", engine.state.pending_resolution),
        pending_choice
    );
    assert_eq!(
        engine
            .state
            .pending_spell_cast
            .as_ref()
            .unwrap()
            .transaction_id,
        transaction_id
    );

    assert!(activation.events.iter().all(|event| !matches!(
        event.ev,
        Some(Ev::StackPushed(_)) | Some(Ev::StackResolved(_))
    )));

    engine
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
                    chosen_object_ids: vec![choice_id],
                    ..Default::default()
                })),
            },
        )
        .expect("resolve prevention without closing spell payment");
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(
        engine
            .state
            .pending_spell_cast
            .as_ref()
            .unwrap()
            .transaction_id,
        transaction_id
    );
    assert!(engine.state.stack.is_empty());

    let committed = commit_green_spell(&mut engine, 0, transaction_id);
    assert!(engine.state.pending_spell_cast.is_none());
    assert_eq!(engine.state.stack.len(), 1);
    assert_eq!(engine.state.stack[0].card_id, "elvish_mystic");
    assert_eq!(engine.state.players[0].life, 20);
    assert!(committed
        .events
        .iter()
        .any(|event| matches!(event.ev, Some(Ev::StackPushed(_)))));
}

#[test]
fn issue_494_replacement_choice_resumes_resolution_time_mana_payment() {
    let mut engine = GameEngine::new(494_035, &[0, 1], 20, None, true).expect("new engine");
    advance_to_main1_from_game_start(&mut engine);
    let source = inject_permanent_on_battlefield(&mut engine, 0, "talisman_of_impulse");
    let island = inject_permanent_on_battlefield(&mut engine, 0, "island");
    let ward_creature =
        inject_permanent_on_battlefield(&mut engine, 1, "dirgur_island_dragon_skimming_strike");
    engine.state.add_damage_prevention_shield(0, 1);
    engine.state.add_damage_prevention_shield(0, 1);
    inject_card_into_hand(&mut engine, 0, "unsummon");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    let unsummon = hand_index_for_card(&engine, 0, "unsummon");
    engine
        .apply_command(0, &cast_spell(unsummon, target_object(ward_creature)))
        .expect("cast Unsummon and trigger Ward");
    pass_both_players(&mut engine);
    assert!(engine
        .state
        .pending_resolution
        .as_ref()
        .is_some_and(|pending| pending.continuation.mana_payment().is_some()));

    activate_mana_option(&mut engine, 0, island, 0, 0);
    let activation = activate_mana_option(&mut engine, 0, source, 1, 1);
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("damage prevention choice");
    let ResolutionContinuation::ManaAbilityDamageReplacement {
        resume_resolution: Some(parent),
        ..
    } = &pending.continuation
    else {
        panic!("damage choice retains the interrupted resolution payment")
    };
    assert!(parent.continuation.mana_payment().is_some());
    let choice_id = pending.presentation.candidates[0];
    let resolved_damage = engine
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
                    chosen_object_ids: vec![choice_id],
                    ..Default::default()
                })),
            },
        )
        .expect("resolve damage and restore the Ward payment choice");
    assert!(resolved_damage.events.iter().any(|event| matches!(
        &event.ev,
        Some(Ev::ResolutionChoiceRequired(choice))
            if choice.choice_kind == rv1::ChoiceKind::ManaPayment as i32
    )));
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("Ward payment is restored");
    assert!(pending.continuation.mana_payment().is_some());
    assert_eq!(engine.state.players[0].life, 20);
    assert!(engine
        .state
        .stack
        .iter()
        .any(|item| item.card_id == "unsummon"));
    assert!(activation.events.iter().all(|event| !matches!(
        event.ev,
        Some(Ev::StackPushed(_)) | Some(Ev::StackResolved(_))
    )));

    submit_mana_resolution(
        &mut engine,
        0,
        SubmitResolutionChoice {
            decision: rv1::ResolutionChoiceDecision::PayMana as i32,
            ..Default::default()
        },
    )
    .expect("finish the original Ward payment");
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine
        .state
        .stack
        .iter()
        .any(|item| item.card_id == "unsummon"));
    assert_eq!(
        engine.state.objects[&ward_creature].card_id,
        "dirgur_island_dragon_skimming_strike"
    );
    assert!(engine.state.objects[&island].tapped);
}

#[test]
fn issue_494_spell_payment_defers_state_based_actions_until_commit() {
    let (mut engine, source) = damage_mana_engine(494_040, 1, "talisman_of_impulse");
    let transaction_id = begin_spell_payment(&mut engine, 0, "elvish_mystic");

    let activation = activate_mana_option(&mut engine, 0, source, 1, 1);
    assert_eq!(engine.state.players[0].life, 0);
    assert!(engine.state.pending_spell_cast.is_some());
    assert_eq!(
        engine.state.winner(),
        None,
        "SBA is deferred while payment is open"
    );
    assert!(!engine.state.players[0].has_lost);
    assert!(
        engine.state.stack.is_empty(),
        "mana ability has no stack item"
    );
    assert!(activation.events.iter().all(|event| !matches!(
        event.ev,
        Some(Ev::StackPushed(_)) | Some(Ev::StackResolved(_))
    )));

    commit_green_spell(&mut engine, 0, transaction_id);
    assert!(engine.state.pending_spell_cast.is_none());
    assert!(engine.state.players[0].has_lost);
    assert_eq!(
        engine.state.winner(),
        Some(1),
        "the deferred SBA runs after payment commits"
    );
}

#[test]
fn issue_494_activation_and_prevention_choice_replay_deterministically() {
    let (mut original, source) = damage_mana_engine(494_050, 20, "talisman_of_impulse");
    let (mut replay, replay_source) = damage_mana_engine(494_050, 20, "talisman_of_impulse");
    assert_eq!(source, replay_source);
    original.state.zone_change_generation.insert(source, 7);
    replay.state.zone_change_generation.insert(replay_source, 7);
    for engine in [&mut original, &mut replay] {
        engine.state.add_damage_prevention_shield(0, 1);
        engine.state.add_damage_prevention_shield(0, 1);
    }

    let activation = activate_ability_for(&original, source, 1, vec![]);
    let activated = original
        .apply_command(0, &activation)
        .expect("record the logged damage-mana activation");
    let replayed_activation = replay
        .apply_command(0, &activation)
        .expect("replay the same activation command");
    assert_eq!(activated, replayed_activation);

    let choice_id = original
        .state
        .pending_resolution
        .as_ref()
        .expect("prevention choice was recorded")
        .presentation
        .candidates[0];
    let answer = RuledCommand {
        cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
            chosen_object_ids: vec![choice_id],
            ..Default::default()
        })),
    };
    let resolved = original
        .apply_command(0, &answer)
        .expect("record the logged damage-prevention choice");
    let replayed_resolution = replay
        .apply_command(0, &answer)
        .expect("replay the same damage-prevention choice");
    assert_eq!(resolved, replayed_resolution);
    assert_eq!(mana_snapshot(&original, 0), mana_snapshot(&replay, 0));
    assert_eq!(original.state.players[0].life, replay.state.players[0].life);
    assert_eq!(original.state.command_index, replay.state.command_index);
    assert!(original.state.pending_resolution.is_none());
    assert!(replay.state.pending_resolution.is_none());
}
