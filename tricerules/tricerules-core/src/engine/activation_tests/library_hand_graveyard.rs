use super::*;

// Capability fixture: Onyx's +1 sentence plus a synthetic life-gain tail. This does not
// register Professor Onyx or establish whole-card admission.
const LOOK_SOURCE: &str = r#"(
    id: "synthetic_onyx_look", name: "Synthetic Onyx Look",
    face_id: "synthetic_onyx_look", types: ["Planeswalker"], loyalty: 5,
    activated_abilities: [(ability_id: "look", presentation: Fallback,
        costs: [Loyalty(1)], effect: [LoseLife(amount: Fixed(1)),
            LookChooseOneToHandRestToGraveyard(count: 3), GainLife(amount: 2),
            GainLife(amount: 3, who: EachOpponent)],
    )],
)"#;

// Exact Ransack sentence and mana cost, without out-of-corpus registration.
const RANSACK_SENTENCE: &str = r#"(
    id: "synthetic_ransack_sentence", name: "Synthetic Ransack Sentence",
    face_id: "synthetic_ransack_sentence", types: ["Sorcery"], mana_cost: "{1}{B}",
    spell_effect: [LookChooseOneToHandRestToGraveyard(count: 3)],
)"#;

fn fixture(library_size: usize) -> (GameEngine, ObjectId, Vec<ObjectId>) {
    let registry = CardRegistry::from_chunks_and_tokens(
        &[
            include_str!("../../../../tricerules-cards/data/forest.ron"),
            LOOK_SOURCE,
            RANSACK_SENTENCE,
        ],
        &[],
    )
    .unwrap();
    let mut engine = GameEngine::new_with_registry(
        602_451,
        &[17, 29, 41],
        20,
        Some(
            (0..3)
                .map(|_| EngineDeck {
                    mainboard: vec!["forest".into(); 40],
                    commanders: Vec::new(),
                })
                .collect(),
        ),
        true,
        Box::leak(Box::new(registry)),
    )
    .unwrap();
    engine.state.turn_step = TurnStep::Main1;
    engine.state.priority_idx = 0;
    let source = place_ready(&mut engine, 17, "Synthetic Onyx Look");
    while engine.state.players[0].library.len() > library_size {
        let id = engine.state.players[0].library.pop_back().unwrap();
        engine.state.objects.remove(&id);
    }
    let looked = engine.state.players[0]
        .library
        .iter()
        .take(3)
        .copied()
        .collect();
    (engine, source, looked)
}

fn begin(engine: &mut GameEngine, source: ObjectId) -> rv1::RuledEventBatch {
    let generation = engine.payment_object_ref(source).zone_change_generation;
    engine
        .apply_command(
            17,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::ActivateAbility(
                    rv1::ActivateAbility {
                        source_object_id: source,
                        ability_index: 0,
                        expected_zone_change_generation: generation,
                        ..Default::default()
                    },
                )),
            },
        )
        .unwrap();
    let mut batch = None;
    for player in [17, 29, 41] {
        batch = Some(
            engine
                .apply_command(
                    player,
                    &RuledCommand {
                        cmd: Some(rv1::ruled_command::Cmd::PassPriority(rv1::PassPriority {})),
                    },
                )
                .expect("the typed private partition resolves through ordinary priority"),
        );
    }
    batch.unwrap()
}

fn answer(engine: &mut GameEngine, chosen: Vec<ObjectId>) -> rv1::RuledEventBatch {
    let prompt = engine
        .state
        .pending_resolution
        .as_ref()
        .unwrap()
        .resolution_prompt_id
        .get();
    engine
        .apply_command(
            17,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(
                    rv1::SubmitResolutionChoice {
                        resolution_prompt_id: prompt,
                        chosen_object_ids: chosen,
                        ..Default::default()
                    },
                )),
            },
        )
        .unwrap()
}

#[test]
fn library_hand_graveyard_partition_keeps_the_full_cohort_until_both_choices_commit() {
    for selected in 0..3 {
        for reverse in [false, true] {
            let (mut engine, source, looked) = fixture(5);
            let hand_before = engine.state.players[0].hand.clone();
            begin(&mut engine, source);
            assert_eq!(engine.state.players[0].life, 19);
            assert_eq!(
                engine
                    .state
                    .pending_resolution
                    .as_ref()
                    .unwrap()
                    .presentation
                    .candidates,
                looked
            );
            answer(&mut engine, vec![looked[selected]]);
            assert_eq!(engine.state.players[0].hand, hand_before);
            assert!(engine.state.players[0].graveyard.is_empty());
            assert_eq!(
                engine.state.players[0]
                    .library
                    .iter()
                    .take(3)
                    .copied()
                    .collect::<Vec<_>>(),
                looked
            );
            let mut rest: Vec<_> = looked
                .iter()
                .copied()
                .filter(|id| *id != looked[selected])
                .collect();
            if reverse {
                rest.reverse();
            }
            let pending = engine.state.pending_resolution.as_ref().unwrap();
            assert!(pending.presentation.ordered);
            assert_eq!(pending.presentation.min, 2);
            answer(&mut engine, rest.clone());
            assert!(engine.state.players[0].hand.contains(&looked[selected]));
            assert_eq!(engine.state.players[0].graveyard, rest);
            assert_eq!(engine.state.players[0].library.len(), 2);
            assert_eq!(
                engine.state.players[0].life, 21,
                "synthetic tail executes exactly once"
            );
            assert!(engine.state.pending_resolution.is_none());
            assert!(engine.state.stack.is_empty());
        }
    }
}

#[test]
fn library_hand_graveyard_partition_clamps_short_libraries_and_finishes_before_sba() {
    for size in 0..=3 {
        let (mut engine, source, looked) = fixture(size);
        engine.state.players[0].life = 1;
        begin(&mut engine, source);
        if size > 0 {
            assert_eq!(engine.state.players[0].life, 0);
            assert!(
                !engine.state.players[0].has_lost,
                "no SBA within the resolving ability"
            );
            answer(&mut engine, vec![looked[size - 1]]);
            if size == 3 {
                answer(&mut engine, looked[..2].to_vec());
            }
            assert!(engine.state.players[0].hand.contains(&looked[size - 1]));
            assert_eq!(engine.state.players[0].graveyard, looked[..size - 1]);
        }
        assert_eq!(engine.state.players[0].life, 2);
        assert!(!engine.state.players[0].has_lost);
        assert!(engine.state.pending_resolution.is_none());
        assert!(engine.state.stack.is_empty());
    }
}

#[test]
fn library_hand_graveyard_partition_rejects_invalid_and_stale_answers_atomically() {
    for ordering in [false, true] {
        for mutation in 0..11 {
            let (mut engine, source, looked) = fixture(5);
            begin(&mut engine, source);
            if ordering {
                answer(&mut engine, vec![looked[1]]);
            }
            let mut choice = rv1::SubmitResolutionChoice {
                resolution_prompt_id: engine
                    .state
                    .pending_resolution
                    .as_ref()
                    .unwrap()
                    .resolution_prompt_id
                    .get(),
                chosen_object_ids: if ordering {
                    vec![looked[0], looked[2]]
                } else {
                    vec![looked[1]]
                },
                ..Default::default()
            };
            let mut actor = 17;
            match mutation {
                0 => actor = 29,
                1 => choice.resolution_prompt_id += 1,
                2 => choice.chosen_object_ids = vec![],
                3 => choice.chosen_object_ids = vec![looked[0], looked[0]],
                4 => choice.chosen_object_ids[0] = source,
                5 => choice.chosen_player_ids = vec![29],
                6 => choice.decision = rv1::ResolutionChoiceDecision::Decline as i32,
                7 => {
                    *engine
                        .state
                        .zone_change_generation
                        .entry(looked[1])
                        .or_default() += 1
                }
                8 => {
                    *engine
                        .state
                        .zone_change_generation
                        .entry(looked[2])
                        .or_default() += 1
                }
                9 => engine.state.players[0].library.swap(0, 1),
                _ => engine.state.objects.get_mut(&looked[1]).unwrap().owner = 29,
            }
            let before = engine.diagnostic_snapshot().unwrap();
            assert!(
                engine
                    .apply_command(
                        actor,
                        &RuledCommand {
                            cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(choice)),
                        }
                    )
                    .is_err(),
                "ordering={ordering}, mutation={mutation}"
            );
            assert_eq!(
                engine.diagnostic_snapshot().unwrap(),
                before,
                "ordering={ordering}, mutation={mutation}"
            );
        }
    }
}

#[test]
fn library_hand_graveyard_partition_publishes_original_positions_for_same_name_cards() {
    let (mut engine, source, looked) = fixture(5);
    let first = begin(&mut engine, source);
    let choice = first
        .events
        .iter()
        .find_map(|event| match &event.ev {
            Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(choice)) => Some(choice),
            _ => None,
        })
        .unwrap();
    assert_eq!(choice.choice_kind, rv1::ChoiceKind::LibraryLook as i32);
    assert_eq!(choice.deciding_player_id, 17);
    assert!(choice.public_reveal.is_none());
    assert!(choice.candidate_names.iter().all(|name| name == "Forest"));
    let second = answer(&mut engine, vec![looked[1]]);
    assert!(!second
        .events
        .iter()
        .any(|event| matches!(event.ev, Some(rv1::ruled_event::Ev::PermanentMoved(_)))));
    let final_batch = answer(&mut engine, vec![looked[2], looked[0]]);
    let moved: Vec<_> = final_batch
        .events
        .iter()
        .filter_map(|event| match &event.ev {
            Some(rv1::ruled_event::Ev::PermanentMoved(moved))
                if looked.contains(&moved.object_id) =>
            {
                Some(moved)
            }
            _ => None,
        })
        .collect();
    assert_eq!(moved.len(), 3);
    assert_eq!(
        moved.iter().map(|m| m.object_id).collect::<Vec<_>>(),
        [looked[1], looked[2], looked[0]]
    );
    assert_eq!(
        moved
            .iter()
            .map(|m| m.source_library_position)
            .collect::<Vec<_>>(),
        [Some(1), Some(2), Some(0)]
    );
    assert_eq!(
        moved[0].destination,
        rv1::permanent_moved::Destination::Hand as i32
    );
    assert!(!moved[0].object_id_publicly_observable);
    assert!(moved[1..]
        .iter()
        .all(|m| m.destination == rv1::permanent_moved::Destination::Graveyard as i32));
    assert!(moved[1..].iter().all(|m| m.object_id_publicly_observable));
    let view = engine.ev_zone_view_sync();
    let Some(rv1::ruled_event::Ev::ZoneView(sync)) = view.ev else {
        panic!("zone view");
    };
    let owner = sync
        .per_player
        .iter()
        .find(|view| view.player_id == 17)
        .unwrap();
    assert!(owner
        .hand_cards
        .iter()
        .any(|card| card.object_id == looked[1]));
    for opponent in sync.per_player.iter().filter(|view| view.player_id != 17) {
        assert!(opponent
            .hand_cards
            .iter()
            .all(|card| card.object_id != looked[1]));
        assert!(opponent
            .library_cards
            .iter()
            .all(|card| !looked.contains(&card.object_id)));
    }
}

#[test]
fn library_hand_graveyard_partition_survives_source_owner_departure_and_retires_owner_departure() {
    for ordering in [false, true] {
        for departing in [17, 29] {
            let (mut engine, source, looked) = fixture(5);
            engine.state.objects.get_mut(&source).unwrap().owner = 29;
            begin(&mut engine, source);
            if ordering {
                answer(&mut engine, vec![looked[1]]);
            }
            engine
                .apply_command(
                    departing,
                    &RuledCommand {
                        cmd: Some(rv1::ruled_command::Cmd::Concede(rv1::Concede {})),
                    },
                )
                .unwrap();
            if departing == 17 {
                assert!(engine.state.pending_resolution.is_none());
                assert!(engine.state.stack.is_empty());
                assert!(looked
                    .iter()
                    .all(|id| !engine.state.objects.contains_key(id)));
                assert_eq!(
                    (engine.state.players[1].life, engine.state.players[2].life),
                    (23, 23),
                    "the departed controller's synthetic opponent tail still executes once"
                );
            } else {
                assert!(engine.state.pending_resolution.is_some());
                assert!(!engine.state.objects.contains_key(&source));
                if !ordering {
                    answer(&mut engine, vec![looked[1]]);
                }
                answer(&mut engine, vec![looked[2], looked[0]]);
                assert_eq!(engine.state.players[0].graveyard, [looked[2], looked[0]]);
                assert_eq!(engine.state.players[0].life, 21);
                assert!(engine.state.pending_resolution.is_none());
            }
        }
    }
}

#[test]
fn library_hand_graveyard_partition_restores_observer_stage_and_rejects_stale_full_cohort() {
    // Synthetic observer seam: no claim this private look itself creates an observer interrupt.
    for ordering in [false, true] {
        for stale in [false, true] {
            let (mut engine, source, looked) = fixture(5);
            begin(&mut engine, source);
            if ordering {
                answer(&mut engine, vec![looked[1]]);
            }
            let saved = engine.state.pending_resolution.take().unwrap();
            let mut saved = saved;
            let crate::state::ResolutionContinuation::LibraryLook { stack, .. } =
                &mut saved.continuation
            else {
                unreachable!();
            };
            *stack =
                stack
                    .clone()
                    .with_previous_result(crate::state::OperationResults::from_value(
                        crate::state::ProgramValue::Count(37),
                    ));
            let saved_previous = serde_json::to_value(stack.previous_result()).unwrap();
            let previous_prompt = saved.resolution_prompt_id.get();
            if stale {
                engine.state.players[0].library.swap(1, 2);
            }
            let before = engine.diagnostic_snapshot().unwrap();
            let mut events = Vec::new();
            let restored =
                engine.restore_suspended_observer_resolution(Some(saved), None, &mut events);
            if stale {
                assert!(restored.is_err());
                assert_eq!(engine.diagnostic_snapshot().unwrap(), before);
                assert!(events.is_empty());
            } else {
                assert!(restored.unwrap());
                engine.publish_resolution_prompt_ids(&mut events).unwrap();
                let pending = engine.state.pending_resolution.as_ref().unwrap();
                assert_ne!(pending.resolution_prompt_id.get(), previous_prompt);
                assert_eq!(pending.presentation.ordered, ordering);
                let crate::state::ResolutionContinuation::LibraryLook { stack, .. } =
                    &pending.continuation
                else {
                    unreachable!();
                };
                assert_eq!(
                    serde_json::to_value(stack.previous_result()).unwrap(),
                    saved_previous
                );
                assert_eq!(stack.resume_effect_index(), Some(2));
                if !ordering {
                    answer(&mut engine, vec![looked[1]]);
                }
                answer(&mut engine, vec![looked[2], looked[0]]);
                assert_eq!(engine.state.players[0].life, 21);
                assert!(engine.state.pending_resolution.is_none());
            }
        }
    }
}

#[test]
fn library_hand_graveyard_partition_retires_departed_owner_on_observer_restoration() {
    for ordering in [false, true] {
        let (mut engine, source, looked) = fixture(5);
        begin(&mut engine, source);
        if ordering {
            answer(&mut engine, vec![looked[1]]);
        }
        let saved = engine.state.pending_resolution.take().unwrap();
        engine
            .apply_command(
                17,
                &RuledCommand {
                    cmd: Some(rv1::ruled_command::Cmd::Concede(rv1::Concede {})),
                },
            )
            .unwrap();
        let mut events = Vec::new();
        assert!(engine
            .restore_suspended_observer_resolution(Some(saved), None, &mut events)
            .unwrap());
        assert!(engine.state.pending_resolution.is_none());
        assert!(engine.state.stack.is_empty());
        assert_eq!(
            (engine.state.players[1].life, engine.state.players[2].life),
            (23, 23)
        );
        assert!(!events.iter().any(|event| matches!(
            event.ev,
            Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(_))
        )));
    }
}

#[test]
fn library_hand_graveyard_partition_resolves_the_ransack_spell_sentence_without_drawing() {
    let (mut engine, _, looked) = fixture(5);
    engine
        .apply_dev_command(
            17,
            &rv1::DevCommand {
                target_player_id: 17,
                dev: Some(rv1::dev_command::Dev::PutCardInZone(
                    rv1::DevPutCardInZone {
                        card_name: "Synthetic Ransack Sentence".into(),
                        zone: rv1::DevZone::Hand as i32,
                        ..Default::default()
                    },
                )),
            },
        )
        .unwrap();
    let spell = engine.state.next_object_id - 1;
    let hand_index = engine.state.players[0]
        .hand
        .iter()
        .position(|&id| id == spell)
        .unwrap() as u32;
    engine.state.players[0].mana_pool.black = 1;
    engine.state.players[0].mana_pool.colorless = 1;
    let draws_before = engine.state.turn_history.current.player(17).cards_drawn;
    engine
        .apply_scenario_command(
            17,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::CastSpell(rv1::CastSpell {
                    cast_method: rv1::CastMethod::Normal as i32,
                    source: Some(rv1::CastSource {
                        expected_zone_change_generation: Some(
                            engine
                                .state
                                .zone_change_generation
                                .get(&spell)
                                .copied()
                                .unwrap_or(0),
                        ),
                        location: Some(rv1::cast_source::Location::HandIndex(hand_index)),
                    }),
                    ..Default::default()
                })),
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
    answer(&mut engine, vec![looked[2]]);
    answer(&mut engine, vec![looked[1], looked[0]]);
    assert_eq!(
        engine.state.players[0].graveyard,
        [looked[1], looked[0], spell],
        "the resolving sorcery is placed on top only after its partition finishes"
    );
    assert!(engine.state.players[0].hand.contains(&looked[2]));
    assert_eq!(engine.state.players[0].life, 20);
    assert_eq!(
        engine.state.turn_history.current.player(17).cards_drawn,
        draws_before
    );
    assert!(engine.state.stack.is_empty());
}
