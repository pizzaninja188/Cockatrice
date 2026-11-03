use super::super::replacement::PendingReplacementEvent;
use super::*;

const DISTRIBUTION: &str = r#"(
    id: "synthetic_counter_distribution", name: "Synthetic Counter Distribution",
    face_id: "synthetic_counter_distribution", types: ["Planeswalker", "Ajani"], loyalty: 4,
    activated_abilities: [(ability_id: "distribute", presentation: Fallback,
        costs: [Loyalty(-3)], effect: [
            DistributeCounters(counter: PlusOnePlusOne,count: 3,subject: Chosen((kind: Creature))),
            GrantKeywords(subject: Chosen((kind: Creature)),keywords: [Vigilance]),
            GainLife(amount: 7,who: EachPlayer),
        ],
        targeting: Some((groups: [(min: 0,max: 3,prompt: "Choose creatures",effect_indices: [0,1])])),
    )],
)"#;

fn parked_distribution() -> GameEngine {
    parked_distribution_with_targets(false)
}

fn parked_distribution_with_targets(split: bool) -> GameEngine {
    let registry = CardRegistry::from_chunks_and_tokens(
        &[
            include_str!("../../../../tricerules-cards/data/forest.ron"),
            include_str!("../../../../tricerules-cards/data/grizzly_bears.ron"),
            include_str!("../../../../tricerules-cards/data/doubling_season.ron"),
            include_str!("../../../../tricerules-cards/data/pacifism.ron"),
            DISTRIBUTION,
        ],
        &[],
    )
    .unwrap();
    let mut engine = GameEngine::new_with_registry(
        731_301,
        &[17, 29, 41],
        20,
        Some(
            (0..3)
                .map(|_| EngineDeck {
                    mainboard: vec!["forest".into(); 40],
                    commanders: vec![],
                })
                .collect(),
        ),
        true,
        Box::leak(Box::new(registry)),
    )
    .unwrap();
    engine.state.turn_step = TurnStep::Main1;
    engine.state.priority_idx = 0;
    let source = place_ready(&mut engine, 17, "Synthetic Counter Distribution");
    let target = place_ready(&mut engine, 29, "Grizzly Bears");
    place_ready(&mut engine, 29, "Doubling Season");
    place_ready(&mut engine, 29, "Doubling Season");
    let borrowed = place_ready(&mut engine, 17, "Doubling Season");
    engine
        .state
        .objects
        .get_mut(&borrowed)
        .unwrap()
        .base_controller = 29;
    engine.reindex_battlefield_control(&mut Vec::new()).unwrap();
    let mut targets = vec![rv1::TargetRef {
        object_id: target,
        counter_amount: if split { 2 } else { 3 },
        kind: rv1::TargetRefKind::Permanent as i32,
        expected_zone_change_generation: Some(
            engine.payment_object_ref(target).zone_change_generation,
        ),
        ..Default::default()
    }];
    if split {
        let other = place_ready(&mut engine, 41, "Grizzly Bears");
        targets.push(rv1::TargetRef {
            object_id: other,
            counter_amount: 1,
            kind: rv1::TargetRefKind::Permanent as i32,
            expected_zone_change_generation: Some(
                engine.payment_object_ref(other).zone_change_generation,
            ),
            ..Default::default()
        });
    }
    let command = rv1::ActivateAbility {
        source_object_id: source,
        expected_zone_change_generation: engine.payment_object_ref(source).zone_change_generation,
        targets,
        ..Default::default()
    };
    engine
        .apply_command(
            17,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::ActivateAbility(command)),
            },
        )
        .unwrap();
    for player in [17, 29, 41] {
        engine
            .apply_command(
                player,
                &RuledCommand {
                    cmd: Some(rv1::ruled_command::Cmd::PassPriority(rv1::PassPriority {})),
                },
            )
            .unwrap();
    }
    assert!(engine.state.pending_resolution.is_some());
    engine
}

fn choose_distribution_replacement(engine: &mut GameEngine, source: Option<ObjectId>) {
    let pending = engine.state.pending_resolution.as_ref().unwrap();
    let Some(PendingReplacementEvent::Counters(batch)) = &engine.state.pending_replacement_event
    else {
        panic!("replacement choice");
    };
    let application = batch
        .applications
        .iter()
        .find(|application| {
            source.is_none_or(|source| match &application.replacement {
                super::super::replacement::CounterPlacementReplacement::Battlefield {
                    instance,
                    ..
                } => instance.source_object_id == source,
                _ => false,
            })
        })
        .unwrap()
        .choice_id;
    let prompt_id = pending.resolution_prompt_id.get();
    let actor = pending.deciding_player;
    engine
        .apply_command(
            actor,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(
                    rv1::SubmitResolutionChoice {
                        chosen_object_ids: vec![application],
                        resolution_prompt_id: prompt_id,
                        ..Default::default()
                    },
                )),
            },
        )
        .unwrap();
}

#[test]
fn accepted_distribution_history_survives_controller_and_replacement_source_departure() {
    let mut engine = parked_distribution();
    let borrowed = engine
        .state
        .objects
        .values()
        .find(|object| object.card_id == "doubling_season" && object.owner == 17)
        .unwrap()
        .id;
    let target = engine
        .state
        .objects
        .values()
        .find(|object| object.card_id == "grizzly_bears")
        .unwrap()
        .id;
    choose_distribution_replacement(&mut engine, Some(borrowed));
    let Some(PendingReplacementEvent::Counters(batch)) = &engine.state.pending_replacement_event
    else {
        panic!("still parked")
    };
    assert_eq!(batch.events[0].counters[&CounterKind::PlusOnePlusOne], 6);
    assert_eq!(batch.events[0].applied_replacements.len(), 1);
    concede(&mut engine, 17);
    assert!(!engine.state.objects.contains_key(&borrowed));
    choose_distribution_replacement(&mut engine, None);
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(
        engine.state.objects[&target].counter_count(CounterKind::PlusOnePlusOne),
        24
    );
    assert!(engine
        .characteristics(target)
        .unwrap()
        .has_keyword(Keyword::Vigilance));
    assert_eq!(engine.state.players[1].life, 27);
    assert_eq!(engine.state.players[2].life, 27);
}

#[test]
fn departed_recipient_loses_only_its_share_and_the_flat_tail_runs_once() {
    let mut engine = parked_distribution_with_targets(true);
    let survivor = engine
        .state
        .objects
        .values()
        .find(|object| object.card_id == "grizzly_bears" && object.owner == 41)
        .unwrap()
        .id;
    choose_distribution_replacement(&mut engine, None);
    concede(&mut engine, 29);
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(
        engine.state.objects[&survivor].counter_count(CounterKind::PlusOnePlusOne),
        1
    );
    assert!(engine
        .characteristics(survivor)
        .unwrap()
        .has_keyword(Keyword::Vigilance));
    assert_eq!(engine.state.players[0].life, 27);
    assert_eq!(engine.state.players[2].life, 27);
}

#[test]
fn distribution_commit_requires_its_captured_frame_and_authentic_baseline() {
    let mut engine = parked_distribution();
    let pending = engine.state.pending_resolution.as_ref().unwrap();
    let ResolutionContinuation::CounterReplacement {
        stack: Some(frame),
        completion,
    } = &pending.continuation
    else {
        panic!("parked distribution");
    };
    let frame = frame.clone();
    let completion = completion.clone();
    let Some(PendingReplacementEvent::Counters(batch)) = &engine.state.pending_replacement_event
    else {
        panic!("counter batch");
    };
    let proposal = batch.events[0].clone();
    let baseline_replacement = batch.applications[0].replacement.clone();
    let before = engine.diagnostic_snapshot().unwrap();
    let mut output = Vec::new();
    assert!(
        engine
            .commit_counter_placement_batch(
                std::slice::from_ref(&proposal),
                &completion,
                None,
                &mut output
            )
            .is_err(),
        "distribution must reject a missing captured frame"
    );
    assert!(output.is_empty());
    assert_eq!(engine.diagnostic_snapshot().unwrap(), before);
    let mut changed = proposal.clone();
    changed
        .original_counters
        .insert(CounterKind::PlusOnePlusOne, 2);
    assert!(
        engine
            .commit_counter_placement_batch(&[changed], &completion, Some(&frame), &mut output)
            .is_err(),
        "distribution must reject a changed baseline"
    );
    assert!(output.is_empty());
    assert_eq!(engine.diagnostic_snapshot().unwrap(), before);
    for mutation in 0..11 {
        let mut changed = proposal.clone();
        match mutation {
            0 => changed.actor = 41,
            1 => changed.origin = super::super::continuous::CounterPlacementOrigin::Cost,
            2 => changed.cost_debit_index = Some(0),
            3 => {
                changed.counters.insert(CounterKind::PlusOnePlusOne, 4);
            }
            4 => changed.effect_created_counters = false,
            5 => {
                changed.recipient =
                    super::super::replacement::CounterPlacementRecipient::Player { player_id: 29 }
            }
            6 => {
                let super::super::replacement::CounterPlacementRecipient::Object {
                    zone_change_generation,
                    ..
                } = &mut changed.recipient
                else {
                    unreachable!()
                };
                *zone_change_generation += 1;
            }
            7 => {
                changed.counter_move_context =
                    Some(super::super::replacement::CounterMoveReplacementContext {
                        source_object_id: frame.item.id,
                        source_zone_change_generation: 0,
                        kind: CounterKind::PlusOnePlusOne,
                    })
            }
            8..=10 => {
                let mut replacement = baseline_replacement.clone();
                let super::super::replacement::CounterPlacementReplacement::Battlefield {
                    instance,
                    ..
                } = &mut replacement
                else {
                    unreachable!()
                };
                if mutation == 8 {
                    instance.ability_id.clear();
                }
                changed.applied_replacements.push(instance.clone());
                if mutation == 9 {
                    changed.applied_replacements.push(instance.clone());
                }
                // Even a well-shaped history must explain the modified counter value.
            }
            _ => unreachable!(),
        }
        assert!(
            engine
                .commit_counter_placement_batch(&[changed], &completion, Some(&frame), &mut output)
                .is_err(),
            "proposal mutation {mutation}"
        );
        assert!(output.is_empty());
        assert_eq!(engine.diagnostic_snapshot().unwrap(), before);
    }
    for mutation in 0..6 {
        let mut changed = frame.clone();
        match mutation {
            0 => changed.item.id += 1,
            1 => changed.item.controller = 41,
            2 => changed.item.targets[0].group_index = 1,
            3 => changed.item.targets[0].counter_amount = 2,
            4 => changed.item.targets[0].zone_change_generation = None,
            5 => {
                changed = crate::state::ParkedStackResolution::for_flat_effects(
                    changed.item,
                    Some(2),
                    Default::default(),
                )
            }
            _ => unreachable!(),
        }
        assert!(
            engine
                .commit_counter_placement_batch(
                    std::slice::from_ref(&proposal),
                    &completion,
                    Some(&changed),
                    &mut output
                )
                .is_err(),
            "frame mutation {mutation}"
        );
        assert!(output.is_empty());
        assert_eq!(engine.diagnostic_snapshot().unwrap(), before);
    }
}

#[test]
fn nested_aura_return_prunes_a_blinked_distribution_recipient_without_reallocation() {
    let mut engine = parked_distribution_with_targets(true);
    choose_distribution_replacement(&mut engine, None);
    let departed = engine
        .state
        .objects
        .values()
        .find(|object| object.card_id == "grizzly_bears" && object.owner == 29)
        .unwrap()
        .id;
    let survivor = engine
        .state
        .objects
        .values()
        .find(|object| object.card_id == "grizzly_bears" && object.owner == 41)
        .unwrap()
        .id;
    let pacifism = engine.state.players[2].hand[0];
    engine.state.objects.get_mut(&pacifism).unwrap().card_id = "pacifism".into();
    super::super::resolution::move_object_to_zone(
        &mut engine.state,
        engine.registry,
        pacifism,
        Zone::Exile,
        None,
    )
    .unwrap();
    let generation = engine.state.zone_change_generation[&pacifism];
    engine.state.pending_immediate_observer_actions.push(
        crate::state::ImmediateObserverAction::ReturnExiledObject {
            exiled: crate::state::TriggerObjectRef {
                object_id: pacifism,
                zone_change_generation: generation,
                controller_at_event: 41,
            },
        },
    );
    let mut events = Vec::new();
    assert!(engine
        .drain_immediate_observer_actions(None, &mut events)
        .unwrap());
    engine.publish_resolution_prompt_ids(&mut events).unwrap();
    assert!(engine.state.pending_observer_return_batch.is_some());
    super::super::resolution::move_object_to_zone(
        &mut engine.state,
        engine.registry,
        departed,
        Zone::Exile,
        None,
    )
    .unwrap();
    super::super::resolution::move_object_to_zone(
        &mut engine.state,
        engine.registry,
        departed,
        Zone::Battlefield,
        Some(29),
    )
    .unwrap();
    let pending = engine.state.pending_resolution.as_ref().unwrap();
    let prompt = pending.resolution_prompt_id.get();
    engine
        .apply_command(
            41,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(
                    rv1::SubmitResolutionChoice {
                        chosen_object_ids: vec![survivor],
                        resolution_prompt_id: prompt,
                        ..Default::default()
                    },
                )),
            },
        )
        .unwrap();
    assert!(engine.state.pending_observer_return_batch.is_none());
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(
        engine.state.objects[&departed].counter_count(CounterKind::PlusOnePlusOne),
        0
    );
    assert!(!engine
        .characteristics(departed)
        .unwrap()
        .has_keyword(Keyword::Vigilance));
    assert_eq!(
        engine.state.objects[&survivor].counter_count(CounterKind::PlusOnePlusOne),
        1
    );
    assert!(engine
        .characteristics(survivor)
        .unwrap()
        .has_keyword(Keyword::Vigilance));
    assert_eq!(
        engine
            .state
            .players
            .iter()
            .map(|player| player.life)
            .collect::<Vec<_>>(),
        vec![27; 3]
    );
}

#[test]
fn copied_occurrences_keep_their_quantities_across_old_and_new_generations() {
    // Prepared copy consumer; existing Rings tests cover the copy producer. No card admission.
    let mut engine = parked_distribution_with_targets(true);
    let ResolutionContinuation::CounterReplacement {
        stack: Some(frame), ..
    } = &engine
        .state
        .pending_resolution
        .as_ref()
        .unwrap()
        .continuation
    else {
        unreachable!()
    };
    let original = frame.item.clone();
    let first = original.targets[0].object_id;
    let old_generation = original.targets[0].zone_change_generation.unwrap();
    engine.state.pending_resolution = None;
    engine.state.pending_replacement_event = None;
    // Remove replacement sources so this fixture distinguishes target shares directly.
    let seasons = engine
        .state
        .objects
        .values()
        .filter(|object| object.card_id == "doubling_season")
        .map(|object| object.id)
        .collect::<Vec<_>>();
    for source in seasons {
        super::super::resolution::move_object_to_zone(
            &mut engine.state,
            engine.registry,
            source,
            Zone::Exile,
            None,
        )
        .unwrap();
    }
    super::super::resolution::move_object_to_zone(
        &mut engine.state,
        engine.registry,
        first,
        Zone::Exile,
        None,
    )
    .unwrap();
    super::super::resolution::move_object_to_zone(
        &mut engine.state,
        engine.registry,
        first,
        Zone::Battlefield,
        Some(29),
    )
    .unwrap();
    let new_generation = engine.state.zone_change_generation[&first];
    assert_ne!(new_generation, old_generation);
    let mut copy = original.clone();
    copy.id = engine.state.next_object_id;
    engine.state.next_object_id += 1;
    copy.is_copy = true;
    let mut events = Vec::new();
    engine
        .begin_copy_target_choices(
            vec![crate::state::ParkedStackResolution::dedicated(copy)],
            original.id,
            None,
            &mut events,
        )
        .unwrap();
    engine.publish_resolution_prompt_ids(&mut events).unwrap();
    let mut published = None;
    for selection in [vec![], vec![first]] {
        let prompt_id = engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .resolution_prompt_id
            .get();
        published = Some(
            engine
                .apply_command(
                    17,
                    &RuledCommand {
                        cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(
                            rv1::SubmitResolutionChoice {
                                chosen_object_ids: selection,
                                resolution_prompt_id: prompt_id,
                                ..Default::default()
                            },
                        )),
                    },
                )
                .unwrap(),
        );
    }
    let copied = engine.state.stack.last().unwrap();
    assert_eq!(
        copied
            .targets
            .iter()
            .map(|target| (
                target.object_id,
                target.zone_change_generation.unwrap(),
                target.counter_amount
            ))
            .collect::<Vec<_>>(),
        vec![(first, old_generation, 2), (first, new_generation, 1)]
    );
    let push = published
        .unwrap()
        .events
        .into_iter()
        .find_map(|event| match event.ev {
            Some(rv1::ruled_event::Ev::StackPushed(push)) => Some(push),
            _ => None,
        })
        .unwrap();
    assert_eq!(
        push.targets
            .iter()
            .map(|target| (
                target.object_id,
                target.expected_zone_change_generation.unwrap(),
                target.counter_amount
            ))
            .collect::<Vec<_>>(),
        vec![(first, old_generation, 2), (first, new_generation, 1)]
    );
    for _ in 0..3 {
        let actor = engine.state.priority_player_id();
        engine
            .apply_command(
                actor,
                &RuledCommand {
                    cmd: Some(rv1::ruled_command::Cmd::PassPriority(rv1::PassPriority {})),
                },
            )
            .unwrap();
    }
    assert_eq!(
        engine.state.objects[&first].counter_count(CounterKind::PlusOnePlusOne),
        1
    );
    assert!(engine
        .characteristics(first)
        .unwrap()
        .has_keyword(Keyword::Vigilance));
    assert_eq!(engine.state.players[0].life, 27);
}
