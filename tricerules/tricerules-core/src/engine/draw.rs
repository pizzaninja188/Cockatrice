//! CR 121/614/616: one resumable transaction for actual draw instructions.
use super::characteristics::printed_static_source_is_available;
use super::events::{ev_log, ev_priority_changed, finish_with_events};
use super::replacement::PendingReplacementEvent;
use super::triggers::ability_definition_from;
use super::*;
use tricerules_cards::primitives::{DrawReplacementCondition, LibraryDrawReplacement};
mod library_actions;

#[derive(serde::Serialize, Debug, Clone, PartialEq, Eq)]
struct DrawReplacementIdentity {
    source: ObjectId,
    generation: u64,
    definition: AbilityDefinitionId,
}

#[cfg(test)]
mod library_replacement_tests {
    use super::*;

    fn fixture(kind: &str, size: usize) -> GameEngine {
        let source = format!(
            r#"(id: "replacement_fixture", name: "Replacement fixture", face_id: "replacement_fixture", types: ["Creature"], power: 2, toughness: 3, static_abilities: [(ability_id: "static_01", presentation: Fallback, definition: ReplaceControllerDrawWithLibraryChoice(kind: {kind}))])"#
        );
        let registry = CardRegistry::from_chunks_and_tokens(&[
            r#"(id: "forest", name: "Forest", face_id: "forest", types: ["Basic", "Land", "Forest"])"#,
            r#"(id: "nonland_fixture", name: "Nonland fixture", face_id: "nonland_fixture", types: ["Creature"], power: 1, toughness: 1)"#,
            r#"(id: "double_fixture", name: "Double fixture", face_id: "double_fixture", types: ["Enchantment"], static_abilities: [(ability_id: "static_01", presentation: Fallback, definition: DoubleControllerDraws(condition: Always))])"#,
            &source,
        ], &[]).expect("admit the real draw replacement operation");
        let mut engine = GameEngine::new(
            12106,
            &[0, 1, 2],
            20,
            Some(vec![vec!["forest".into(); 24]; 3]),
            true,
        )
        .unwrap();
        engine.registry = Box::leak(Box::new(registry));
        engine.state.opening = None;
        engine.state.turn_step = TurnStep::Draw;
        engine.state.draw_step_progress = Some((0, 0, 0));
        let source = engine.state.players[0].library[0];
        resolution::move_object_to_zone(
            &mut engine.state,
            engine.registry,
            source,
            Zone::Battlefield,
            None,
        )
        .unwrap();
        engine.state.objects.get_mut(&source).unwrap().card_id = "replacement_fixture".into();
        let excess: Vec<_> = engine.state.players[0]
            .library
            .iter()
            .skip(size)
            .copied()
            .collect();
        for oid in excess {
            resolution::move_object_to_zone(
                &mut engine.state,
                engine.registry,
                oid,
                Zone::Exile,
                None,
            )
            .unwrap();
        }
        engine
    }

    fn start(engine: &mut GameEngine) -> (DrawProgress, Vec<rv1::RuledEvent>) {
        let mut events = Vec::new();
        let progress = engine
            .start_draw_transaction(
                vec![(0, 1)],
                DrawCompletion::FinishDrawStep {
                    active_player: 0,
                    occurrence: 0,
                },
                "fixture",
                &mut events,
            )
            .unwrap();
        (progress, events)
    }

    #[test]
    fn authoring_tomorrow_replacement_handles_short_and_empty_libraries_without_drawing() {
        for size in 0..=3 {
            let mut engine = fixture("LookAtTopThree", size);
            let hand = engine.state.players[0].hand.len();
            let looked: Vec<_> = engine.state.players[0].library.iter().copied().collect();
            let (progress, events) = start(&mut engine);
            if size == 0 {
                let DrawProgress::Complete(done) = progress else {
                    panic!("empty replacement completes")
                };
                assert!(done.receipts.is_empty());
                assert!(events.iter().all(|event| !matches!(
                    event.ev,
                    Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(_))
                )));
            } else {
                assert!(matches!(progress, DrawProgress::Parked));
                let pending = engine.state.pending_resolution.as_ref().unwrap();
                assert_eq!((pending.presentation.min, pending.presentation.max), (1, 1));
                assert_eq!(pending.presentation.candidates, looked);
                assert!(pending.continuation.stack().is_none());
                engine
                    .submit_resolution_choice(
                        0,
                        &rv1::SubmitResolutionChoice {
                            chosen_object_ids: vec![looked[0]],
                            ..Default::default()
                        },
                    )
                    .unwrap();
                if size == 3 {
                    engine
                        .submit_resolution_choice(
                            0,
                            &rv1::SubmitResolutionChoice {
                                chosen_object_ids: vec![looked[2], looked[1]],
                                ..Default::default()
                            },
                        )
                        .unwrap();
                }
                assert_eq!(engine.state.players[0].hand.len(), hand + 1);
                assert_eq!(
                    engine.state.players[0]
                        .library
                        .iter()
                        .copied()
                        .collect::<Vec<_>>(),
                    if size == 3 {
                        vec![looked[2], looked[1]]
                    } else {
                        looked[1..].to_vec()
                    }
                );
            }
            assert!(!engine.state.players[0].pending_library_loss);
            assert_eq!(
                engine.resolve_amount(
                    &Amount::Count(CountExpression::CardsDrawnThisTurn {
                        players: RelativePlayerSet::Controller,
                    }),
                    AmountContext::from_condition(ConditionContext {
                        controller: 0,
                        source_object_id: 0,
                        source_zone_change: 0,
                        resolving_spell_id: None,
                        stack_item: None,
                        previous_effect_result: None,
                    })
                ),
                0
            );
        }
    }

    fn branch(engine: &mut GameEngine, index: u32) -> RuledEventBatch {
        engine
            .apply_command(
                0,
                &rv1::RuledCommand {
                    cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(
                        rv1::SubmitResolutionChoice {
                            selected_branch_index: index,
                            decision: rv1::ResolutionChoiceDecision::SelectBranch as i32,
                            ..Default::default()
                        },
                    )),
                },
            )
            .unwrap()
    }

    #[test]
    fn authoring_draw_branch_commands_reject_atomically_and_repeat_deterministically() {
        let run = || {
            let mut engine = fixture("RevealUntilLandOrNonland", 4);
            let cards: Vec<_> = engine.state.players[0].library.iter().copied().collect();
            for oid in &cards[..2] {
                engine.state.objects.get_mut(oid).unwrap().card_id = "nonland_fixture".into();
            }
            let (_, initial) = start(&mut engine);
            let mut batches = vec![format!("{initial:?}")];
            for stage in 0..2 {
                let before = serde_json::to_value(&engine.state).unwrap();
                for answer in [
                    rv1::SubmitResolutionChoice::default(),
                    rv1::SubmitResolutionChoice {
                        decision: rv1::ResolutionChoiceDecision::Decline as i32,
                        ..Default::default()
                    },
                    rv1::SubmitResolutionChoice {
                        decision: rv1::ResolutionChoiceDecision::SelectBranch as i32,
                        chosen_object_ids: vec![cards[0]],
                        ..Default::default()
                    },
                    rv1::SubmitResolutionChoice {
                        decision: rv1::ResolutionChoiceDecision::SelectBranch as i32,
                        selected_branch_index: 2,
                        ..Default::default()
                    },
                ] {
                    assert!(engine
                        .apply_command(
                            0,
                            &rv1::RuledCommand {
                                cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(answer)),
                            }
                        )
                        .is_err());
                    assert_eq!(
                        serde_json::to_value(&engine.state).unwrap(),
                        before,
                        "stage {stage}: rejection changed continuation or command index"
                    );
                }
                batches.push(format!("{:?}", branch(&mut engine, 0)));
            }
            assert!(engine.state.pending_resolution.is_some());
            let done = engine
                .apply_command(
                    0,
                    &rv1::RuledCommand {
                        cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(
                            rv1::SubmitResolutionChoice {
                                chosen_object_ids: vec![cards[1], cards[0]],
                                ..Default::default()
                            },
                        )),
                    },
                )
                .unwrap();
            batches.push(format!("{done:?}"));
            assert!(engine.state.pending_resolution.is_none());
            (batches, serde_json::to_value(&engine.state).unwrap())
        };
        assert_eq!(
            run(),
            run(),
            "same seed and stock commands must reproduce every event batch and final state"
        );
    }

    #[test]
    fn authoring_abundance_reveals_matching_prefix_and_keeps_bottom_order_private() {
        let mut engine = fixture("RevealUntilLandOrNonland", 4);
        let cards: Vec<_> = engine.state.players[0].library.iter().copied().collect();
        for oid in &cards[..2] {
            engine.state.objects.get_mut(oid).unwrap().card_id = "nonland_fixture".into();
        }
        let hand = engine.state.players[0].hand.len();
        assert!(matches!(start(&mut engine).0, DrawProgress::Parked));
        branch(&mut engine, 0); // replace
        let batch = branch(&mut engine, 0); // land
        let reveal = batch
            .events
            .iter()
            .find_map(|event| match &event.ev {
                Some(rv1::ruled_event::Ev::CardsRevealed(reveal)) => Some(reveal.clone()),
                _ => None,
            })
            .unwrap();
        assert_eq!(
            reveal
                .cards
                .iter()
                .map(|card| card.object_id)
                .collect::<Vec<_>>(),
            cards[..3]
        );
        assert!(!reveal.reveal_id.is_empty());
        assert_eq!(engine.state.players[0].hand.len(), hand + 1);
        assert!(engine.state.players[0].hand.contains(&cards[2]));
        let event = engine.draw_replacement_choice_event().unwrap();
        let Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(choice)) = event.ev else {
            panic!("ordering choice")
        };
        assert!(choice.public_reveal.is_none());
        assert_eq!(choice.candidate_object_ids, cards[..2]);
        assert!(choice.ordered);
        assert_eq!(engine.draw_action_reveal(), Some(reveal.clone()));
        assert_eq!(
            engine.draw_replacement_choice_event(),
            Some(rv1::RuledEvent {
                ev: Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(choice))
            })
        );
        engine
            .submit_resolution_choice(
                0,
                &rv1::SubmitResolutionChoice {
                    chosen_object_ids: vec![cards[1], cards[0]],
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(
            engine.state.players[0]
                .library
                .iter()
                .copied()
                .collect::<Vec<_>>(),
            vec![cards[3], cards[1], cards[0]]
        );
        assert!(engine.draw_action_reveal().is_none());
        assert_eq!(engine.state.draw_step_progress, Some((0, 0, 0)));
    }

    #[test]
    fn authoring_abundance_decline_no_match_and_empty_do_not_restart_the_replacement() {
        for size in [0, 1, 3] {
            let mut engine = fixture("RevealUntilLandOrNonland", size);
            let hand = engine.state.players[0].hand.len();
            let cards: Vec<_> = engine.state.players[0].library.iter().copied().collect();
            start(&mut engine);
            branch(&mut engine, 0);
            branch(&mut engine, 1); // no nonlands: consume the draw, order the entire revealed cohort
            if size > 1 {
                engine
                    .submit_resolution_choice(
                        0,
                        &rv1::SubmitResolutionChoice {
                            chosen_object_ids: cards.iter().rev().copied().collect(),
                            ..Default::default()
                        },
                    )
                    .unwrap();
            }
            assert!(engine.state.pending_resolution.is_none());
            assert_eq!(engine.state.players[0].hand.len(), hand);
            assert!(!engine.state.players[0].pending_library_loss);
            assert_eq!(engine.state.draw_step_progress, Some((0, 0, 0)));
        }
        let mut engine = fixture("RevealUntilLandOrNonland", 2);
        let hand = engine.state.players[0].hand.len();
        start(&mut engine);
        branch(&mut engine, 1);
        assert!(engine.state.pending_resolution.is_none());
        assert_eq!(engine.state.players[0].hand.len(), hand + 1);
        assert_eq!(engine.state.draw_step_progress, Some((0, 0, 1)));
    }

    #[test]
    fn authoring_draw_action_rejects_bad_answers_atomically_and_survives_source_departure() {
        let mut engine = fixture("LookAtTopThree", 3);
        start(&mut engine);
        let cards = engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .presentation
            .candidates
            .clone();
        let answer = rv1::SubmitResolutionChoice {
            chosen_object_ids: vec![cards[0]],
            ..Default::default()
        };
        let before = format!("{:?}", engine.state.pending_resolution);
        let work_before = format!("{:?}", engine.state.pending_replacement_event);
        assert!(engine.submit_resolution_choice(1, &answer).is_err());
        for bad in [
            rv1::SubmitResolutionChoice {
                chosen_object_ids: vec![],
                ..Default::default()
            },
            rv1::SubmitResolutionChoice {
                chosen_object_ids: vec![cards[0], cards[0]],
                ..Default::default()
            },
            rv1::SubmitResolutionChoice {
                chosen_player_ids: vec![1],
                ..answer.clone()
            },
        ] {
            assert!(engine.submit_resolution_choice(0, &bad).is_err());
            assert_eq!(format!("{:?}", engine.state.pending_resolution), before);
            assert_eq!(
                format!("{:?}", engine.state.pending_replacement_event),
                work_before
            );
        }
        *engine
            .state
            .zone_change_generation
            .entry(cards[0])
            .or_default() += 1;
        assert!(engine.submit_resolution_choice(0, &answer).is_err());
        *engine
            .state
            .zone_change_generation
            .get_mut(&cards[0])
            .unwrap() -= 1;
        let source = engine.state.players[0].battlefield[0];
        engine.state.players[0]
            .battlefield
            .retain(|oid| *oid != source);
        engine.state.players[1].battlefield.push(source);
        engine.state.objects.get_mut(&source).unwrap().owner = 1;
        engine.concede_batch(1).unwrap();
        assert!(!engine.state.objects.contains_key(&source));
        assert_eq!(
            engine
                .state
                .pending_resolution
                .as_ref()
                .unwrap()
                .presentation
                .candidates,
            cards
        );
        engine.submit_resolution_choice(0, &answer).unwrap();
        engine
            .submit_resolution_choice(
                0,
                &rv1::SubmitResolutionChoice {
                    chosen_object_ids: vec![cards[2], cards[1]],
                    ..Default::default()
                },
            )
            .unwrap();
        assert!(engine.state.pending_resolution.is_none());
    }

    #[test]
    fn authoring_drawer_departure_retires_only_their_action_and_continues_other_requests() {
        let mut engine = fixture("LookAtTopThree", 3);
        let hand = engine.state.players[2].hand.len();
        engine
            .start_draw_transaction(
                vec![(0, 1), (2, 1)],
                DrawCompletion::FinishDrawStep {
                    active_player: 0,
                    occurrence: 0,
                },
                "fixture",
                &mut Vec::new(),
            )
            .unwrap();
        engine.concede_batch(0).unwrap();
        assert!(engine.state.pending_resolution.is_none());
        assert!(engine.state.pending_replacement_event.is_none());
        assert_eq!(engine.state.players[2].hand.len(), hand + 1);
    }

    #[test]
    fn authoring_doubling_and_library_replacement_finish_each_child_before_the_next() {
        for double_first in [false, true] {
            let mut engine = fixture("LookAtTopThree", 7);
            let double = engine.state.players[0].library[6];
            resolution::move_object_to_zone(
                &mut engine.state,
                engine.registry,
                double,
                Zone::Battlefield,
                None,
            )
            .unwrap();
            engine.state.objects.get_mut(&double).unwrap().card_id = "double_fixture".into();
            let hand = engine.state.players[0].hand.len();
            start(&mut engine);
            let Some(PendingReplacementEvent::Draw(work)) = &engine.state.pending_replacement_event
            else {
                panic!("replacement order")
            };
            let selected = work
                .applications
                .iter()
                .find(|application| {
                    matches!(application.action, DrawReplacementAction::Double) == double_first
                })
                .unwrap()
                .handle;
            engine
                .submit_resolution_choice(
                    0,
                    &rv1::SubmitResolutionChoice {
                        chosen_object_ids: vec![selected],
                        ..Default::default()
                    },
                )
                .unwrap();
            for _ in 0..if double_first { 2 } else { 1 } {
                let candidates = engine
                    .state
                    .pending_resolution
                    .as_ref()
                    .unwrap()
                    .presentation
                    .candidates
                    .clone();
                engine
                    .submit_resolution_choice(
                        0,
                        &rv1::SubmitResolutionChoice {
                            chosen_object_ids: vec![candidates[0]],
                            ..Default::default()
                        },
                    )
                    .unwrap();
                let bottom = engine
                    .state
                    .pending_resolution
                    .as_ref()
                    .unwrap()
                    .presentation
                    .candidates
                    .clone();
                engine
                    .submit_resolution_choice(
                        0,
                        &rv1::SubmitResolutionChoice {
                            chosen_object_ids: bottom.into_iter().rev().collect(),
                            ..Default::default()
                        },
                    )
                    .unwrap();
            }
            assert!(engine.state.pending_resolution.is_none());
            assert_eq!(
                engine.state.players[0].hand.len(),
                hand + if double_first { 2 } else { 1 }
            );
            assert_eq!(engine.state.draw_step_progress, Some((0, 0, 0)));
        }
    }
}

#[cfg(test)]
mod zurs_weirding_tests {
    use super::*;

    fn fixture(players: &[PlayerId], drawer: PlayerId) -> GameEngine {
        let mut engine = GameEngine::new(
            801204,
            players,
            20,
            Some(vec![vec!["forest".into(); 24]; players.len()]),
            true,
        )
        .unwrap();
        engine.registry = CardRegistry::global();
        engine.state.opening = None;

        let source_owner = players[0];
        add_permanent(&mut engine, source_owner, "zurs_weirding");
        assert_ne!(drawer, source_owner);
        engine
    }

    fn add_permanent(engine: &mut GameEngine, owner: PlayerId, card_id: &str) -> ObjectId {
        let index = engine.state.player_idx(owner).unwrap();
        let oid = engine.state.players[index].library[0];
        resolution::move_object_to_zone(
            &mut engine.state,
            engine.registry,
            oid,
            Zone::Battlefield,
            None,
        )
        .unwrap();
        engine.state.objects.get_mut(&oid).unwrap().card_id = card_id.into();
        oid
    }

    fn start(engine: &mut GameEngine, drawer: PlayerId) -> Vec<rv1::RuledEvent> {
        let mut events = Vec::new();
        let progress = engine
            .start_draw_transaction(
                vec![(drawer, 1)],
                DrawCompletion::FinishDrawStep {
                    active_player: drawer,
                    occurrence: 0,
                },
                "Zur's Weirding regression",
                &mut events,
            )
            .unwrap();
        assert!(matches!(progress, DrawProgress::Parked));
        events
    }

    fn choose(engine: &mut GameEngine, player: PlayerId, branch: u32) -> RuledEventBatch {
        engine
            .submit_resolution_choice(
                player,
                &rv1::SubmitResolutionChoice {
                    decision: rv1::ResolutionChoiceDecision::SelectBranch as i32,
                    selected_branch_index: branch,
                    ..Default::default()
                },
            )
            .unwrap()
    }

    fn choose_replacement(
        engine: &mut GameEngine,
        player: PlayerId,
        action: DrawReplacementAction,
    ) {
        let Some(PendingReplacementEvent::Draw(work)) = &engine.state.pending_replacement_event
        else {
            panic!("draw replacement order")
        };
        let handle = work
            .applications
            .iter()
            .find(|application| {
                std::mem::discriminant(&application.action) == std::mem::discriminant(&action)
            })
            .map(|application| application.handle)
            .unwrap();
        engine
            .submit_resolution_choice(
                player,
                &rv1::SubmitResolutionChoice {
                    chosen_object_ids: vec![handle],
                    ..Default::default()
                },
            )
            .unwrap();
    }

    fn pending_choice(engine: &GameEngine) -> rv1::ResolutionChoiceRequired {
        let Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(choice)) = engine
            .draw_replacement_choice_event()
            .and_then(|event| event.ev)
        else {
            panic!("Zur's Weirding payer choice")
        };
        choice
    }

    #[test]
    fn zurs_weirding_global_draw_is_replaced_for_an_opponent() {
        let mut engine = fixture(&[0, 1, 2], 1);
        let events = start(&mut engine, 1);
        assert!(events
            .iter()
            .any(|event| matches!(event.ev, Some(rv1::ruled_event::Ev::CardsRevealed(_)))));
        assert!(engine.state.pending_resolution.is_some());
        assert_eq!(engine.state.players[1].hand.len(), 7);
        let choice = pending_choice(&engine);
        assert_eq!(choice.deciding_player_id, 0);
        assert_eq!(choice.resolution_branches.len(), 2);
        assert!(choice.public_reveal.is_none());
        assert!(engine.draw_action_reveal().is_some());
    }

    #[test]
    fn zurs_weirding_zone_views_publish_each_current_hand_and_clear_when_the_source_leaves() {
        let mut engine = fixture(&[0, 1, 2], 1);
        let Some(rv1::ruled_event::Ev::ZoneView(first)) = engine.ev_zone_view_sync_tracked().ev
        else {
            panic!("zone view")
        };
        for player in &engine.state.players {
            let view = first
                .per_player
                .iter()
                .find(|view| view.player_id == player.id)
                .unwrap();
            let public_hand = view.public_hand.as_ref().expect("active public hand");
            assert_eq!(public_hand.cards.len(), player.hand.len());
            for (card, &oid) in public_hand.cards.iter().zip(player.hand.iter()) {
                assert_eq!(card.object_id, oid);
                assert_eq!(card.card_id, engine.state.objects[&oid].card_id);
                assert!(!card.card_name.is_empty());
            }
        }

        engine.state.players[2].hand.clear();
        let Some(rv1::ruled_event::Ev::ZoneView(empty_hand)) =
            engine.ev_zone_view_sync_tracked().ev
        else {
            panic!("updated zone view")
        };
        assert!(empty_hand.battlefields_unchanged);
        let player_two = empty_hand
            .per_player
            .iter()
            .find(|view| view.player_id == 2)
            .unwrap();
        assert!(player_two.public_hand.is_some());
        assert!(player_two.public_hand.as_ref().unwrap().cards.is_empty());

        let source = engine.state.players[0]
            .battlefield
            .iter()
            .copied()
            .find(|oid| engine.state.objects[oid].card_id == "zurs_weirding")
            .unwrap();
        resolution::move_object_to_zone(
            &mut engine.state,
            engine.registry,
            source,
            Zone::Graveyard,
            None,
        )
        .unwrap();
        let Some(rv1::ruled_event::Ev::ZoneView(cleared)) = engine.ev_zone_view_sync_tracked().ev
        else {
            panic!("cleared zone view")
        };
        assert!(cleared
            .per_player
            .iter()
            .all(|view| view.public_hand.is_none()));
    }

    #[test]
    fn zurs_weirding_collects_public_apnap_choices_before_simultaneous_payments() {
        let mut engine = fixture(&[0, 1, 2, 3], 1);
        let before_hand = engine.state.players[1].hand.len();
        let before_library = engine.state.players[1].library.len();
        let top = engine.state.players[1].library[0];
        let events = start(&mut engine, 1);
        let reveal = engine.draw_action_reveal().unwrap();
        assert_eq!(reveal.cards[0].object_id, top);
        assert_eq!(pending_choice(&engine).deciding_player_id, 0);

        let first = choose(&mut engine, 0, 0);
        let first_pay_log = first
            .events
            .iter()
            .position(|event| {
                matches!(
                    event.ev.as_ref(),
                    Some(rv1::ruled_event::Ev::Log(log)) if log.text.contains("P0 chooses to pay")
                )
            })
            .unwrap();
        let next_prompt = first
            .events
            .iter()
            .position(|event| {
                matches!(
                    event.ev,
                    Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(_))
                )
            })
            .unwrap();
        assert!(
            first_pay_log < next_prompt,
            "the public choice precedes the next prompt"
        );
        assert_eq!(pending_choice(&engine).deciding_player_id, 2);
        assert_eq!(engine.state.players[0].life, 20);
        assert_eq!(engine.state.players[2].life, 20);
        assert_eq!(engine.state.players[3].life, 20);
        assert_eq!(engine.state.players[1].library.front(), Some(&top));
        assert_eq!(engine.draw_action_reveal(), Some(reveal.clone()));

        choose(&mut engine, 2, 0);
        assert_eq!(pending_choice(&engine).deciding_player_id, 3);
        let Some(PendingReplacementEvent::Draw(work)) = &engine.state.pending_replacement_event
        else {
            panic!("parked payer transaction")
        };
        assert!(matches!(
            work.action.as_ref().map(|action| &action.stage),
            Some(DrawLibraryStage::ZurPayment { pay_intents, .. }) if pay_intents == &[0, 2]
        ));
        let card = &work.action.as_ref().unwrap().reveal.as_ref().unwrap().cards[0];
        assert_eq!(card.object_id, top);
        assert_eq!(engine.state.players[1].library.front(), Some(&top));
        assert_eq!(engine.state.objects[&top].zone, Zone::Library);
        assert_eq!(
            engine
                .state
                .zone_change_generation
                .get(&top)
                .copied()
                .unwrap_or(0),
            card.zone_change_generation
        );
        assert_eq!(engine.state.players[0].life, 20);
        assert_eq!(engine.state.players[2].life, 20);
        let final_batch = choose(&mut engine, 3, 1);

        assert_eq!(engine.state.players[0].life, 18);
        assert_eq!(engine.state.players[2].life, 18);
        assert_eq!(engine.state.players[3].life, 20);
        assert_eq!(engine.state.players[1].hand.len(), before_hand);
        assert_eq!(engine.state.players[1].library.len(), before_library - 1);
        assert_eq!(engine.state.objects[&top].zone, Zone::Graveyard);
        assert_eq!(engine.state.players[1].graveyard.last(), Some(&top));
        assert!(engine.draw_action_reveal().is_none());
        let life_events = final_batch
            .events
            .iter()
            .filter_map(|event| match event.ev.as_ref() {
                Some(rv1::ruled_event::Ev::LifeChanged(change)) => Some(change.player_id),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert!(life_events.contains(&0));
        assert!(life_events.contains(&2));
        assert!(!life_events.contains(&3));
        assert!(events
            .iter()
            .any(|event| matches!(event.ev, Some(rv1::ruled_event::Ev::CardsRevealed(_)))));
    }

    #[test]
    fn zurs_weirding_all_declines_draw_and_an_empty_library_payment_prevents_loss() {
        let mut engine = fixture(&[0, 1, 2], 1);
        let hand = engine.state.players[1].hand.len();
        let library = engine.state.players[1].library.len();
        let top = engine.state.players[1].library[0];
        start(&mut engine, 1);
        choose(&mut engine, 0, 1);
        let done = choose(&mut engine, 2, 1);
        assert_eq!(engine.state.players[1].hand.len(), hand + 1);
        assert_eq!(engine.state.objects[&top].zone, Zone::Hand);
        assert_eq!(engine.state.players[1].library.len(), library - 1);
        assert!(!engine.state.players[1].pending_library_loss);
        assert!(engine.draw_action_reveal().is_none());
        assert!(!done.events.iter().any(|event| matches!(
            event.ev.as_ref(),
            Some(rv1::ruled_event::Ev::LifeChanged(_))
        )));

        let mut empty = fixture(&[0, 1, 2], 1);
        let library = std::mem::take(&mut empty.state.players[1].library);
        for oid in library {
            resolution::move_object_to_zone(
                &mut empty.state,
                empty.registry,
                oid,
                Zone::Exile,
                None,
            )
            .unwrap();
        }
        start(&mut empty, 1);
        assert!(empty.draw_action_reveal().is_none());
        choose(&mut empty, 0, 0);
        choose(&mut empty, 2, 1);
        assert_eq!(empty.state.players[0].life, 18);
        assert!(empty.state.players[1].library.is_empty());
        assert!(!empty.state.players[1].pending_library_loss);

        let mut empty_unpaid = fixture(&[0, 1, 2], 1);
        let library = std::mem::take(&mut empty_unpaid.state.players[1].library);
        for oid in library {
            resolution::move_object_to_zone(
                &mut empty_unpaid.state,
                empty_unpaid.registry,
                oid,
                Zone::Exile,
                None,
            )
            .unwrap();
        }
        start(&mut empty_unpaid, 1);
        choose(&mut empty_unpaid, 0, 1);
        choose(&mut empty_unpaid, 2, 1);
        assert!(empty_unpaid.state.players[1].has_lost);
        assert!(!empty_unpaid.state.players[1].pending_library_loss);
        assert_eq!(empty_unpaid.state.players[0].life, 20);
        assert_eq!(empty_unpaid.state.players[2].life, 20);
        assert!(empty_unpaid.state.players[1].library.is_empty());
    }

    #[test]
    fn zurs_weirding_revalidates_every_payment_before_the_group_debit() {
        for another_payer_pays in [false, true] {
            let mut engine = fixture(&[0, 1, 2], 1);
            engine.state.players[0].life = 2;
            let top = engine.state.players[1].library[0];
            let before_hand = engine.state.players[1].hand.len();
            start(&mut engine, 1);
            choose(&mut engine, 0, 0);
            engine.state.players[0].life = 1;
            choose(&mut engine, 2, u32::from(!another_payer_pays));

            assert_eq!(engine.state.players[0].life, 1);
            assert_eq!(
                engine.state.players[2].life,
                if another_payer_pays { 18 } else { 20 }
            );
            if another_payer_pays {
                assert_eq!(engine.state.objects[&top].zone, Zone::Graveyard);
                assert_eq!(engine.state.players[1].hand.len(), before_hand);
            } else {
                assert_eq!(engine.state.objects[&top].zone, Zone::Hand);
                assert_eq!(engine.state.players[1].hand.len(), before_hand + 1);
            }
        }
    }

    #[test]
    fn zurs_weirding_skips_a_departed_payer_and_does_not_charge_a_selected_payer_who_concedes() {
        let mut current_leaves = fixture(&[0, 1, 2], 1);
        let top = current_leaves.state.players[1].library[0];
        start(&mut current_leaves, 1);
        let same_reveal = current_leaves.draw_action_reveal();
        current_leaves.concede_batch(0).unwrap();
        assert_eq!(pending_choice(&current_leaves).deciding_player_id, 2);
        assert_eq!(current_leaves.draw_action_reveal(), same_reveal);
        choose(&mut current_leaves, 2, 1);
        assert_eq!(current_leaves.state.players[0].life, 20);
        assert_eq!(current_leaves.state.objects[&top].zone, Zone::Hand);

        let mut selected_leaves = fixture(&[0, 1, 2], 1);
        let selected_top = selected_leaves.state.players[1].library[0];
        start(&mut selected_leaves, 1);
        choose(&mut selected_leaves, 0, 0);
        selected_leaves.concede_batch(0).unwrap();
        assert_eq!(pending_choice(&selected_leaves).deciding_player_id, 2);
        choose(&mut selected_leaves, 2, 1);
        assert_eq!(selected_leaves.state.players[0].life, 20);
        assert_eq!(
            selected_leaves.state.objects[&selected_top].zone,
            Zone::Hand
        );
    }

    #[test]
    fn zurs_weirding_and_abundance_follow_the_drawers_selected_replacement_order() {
        for zur_first in [false, true] {
            let mut engine = fixture(&[0, 1, 2], 1);
            let before_hand = engine.state.players[1].hand.len();
            add_permanent(&mut engine, 1, "abundance");
            start(&mut engine, 1);

            if zur_first {
                choose_replacement(&mut engine, 1, DrawReplacementAction::ZurWeirding);
                choose(&mut engine, 0, 1);
                choose(&mut engine, 2, 1);
            } else {
                choose_replacement(
                    &mut engine,
                    1,
                    DrawReplacementAction::Library(
                        LibraryDrawReplacement::RevealUntilLandOrNonland,
                    ),
                );
            }

            let optional = pending_choice(&engine);
            assert_eq!(optional.deciding_player_id, 1);
            assert_eq!(
                optional.choice_kind,
                rv1::ChoiceKind::ResolutionBranch as i32
            );
            choose(&mut engine, 1, 0); // Replace the draw with Abundance.
            choose(&mut engine, 1, 0); // Choose a land; this fixture's library contains Forests.
            assert_eq!(engine.state.players[1].hand.len(), before_hand + 1);
            assert!(engine.state.pending_resolution.is_none());
            assert!(engine.draw_action_reveal().is_none());
            assert_eq!(engine.state.players[0].life, 20);
            assert_eq!(engine.state.players[2].life, 20);
        }
    }

    #[test]
    fn zurs_weirding_instances_apply_once_each_to_the_same_draw_event() {
        for first_instance_pays in [false, true] {
            let mut engine = fixture(&[0, 1, 2], 1);
            let top = engine.state.players[1].library[0];
            add_permanent(&mut engine, 2, "zurs_weirding");
            start(&mut engine, 1);
            choose_replacement(&mut engine, 1, DrawReplacementAction::ZurWeirding);

            if !first_instance_pays {
                choose(&mut engine, 0, 1);
                choose(&mut engine, 2, 1);
                assert_eq!(pending_choice(&engine).deciding_player_id, 0);
                assert!(engine.draw_action_reveal().is_some());
                choose(&mut engine, 0, 0);
                choose(&mut engine, 2, 1);
            } else {
                choose(&mut engine, 0, 0);
                choose(&mut engine, 2, 1);
            }

            assert_eq!(engine.state.objects[&top].zone, Zone::Graveyard);
            assert_eq!(engine.state.players[1].hand.len(), 7);
            assert_eq!(engine.state.players[0].life, 18);
            assert_eq!(engine.state.players[2].life, 20);
            assert!(engine.state.pending_resolution.is_none());
            assert!(engine.draw_action_reveal().is_none());
        }
    }

    #[test]
    fn zurs_weirding_logged_payer_choices_replay_identically() {
        fn branch(index: u32) -> rv1::RuledCommand {
            rv1::RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(
                    rv1::SubmitResolutionChoice {
                        decision: rv1::ResolutionChoiceDecision::SelectBranch as i32,
                        selected_branch_index: index,
                        ..Default::default()
                    },
                )),
            }
        }

        let mut engine = fixture(&[0, 1, 2, 3], 1);
        let mut replay = fixture(&[0, 1, 2, 3], 1);
        let first_start = start(&mut engine, 1);
        let replay_start = start(&mut replay, 1);
        assert_eq!(first_start, replay_start);

        let commands = [(0, branch(0)), (2, branch(0)), (3, branch(1))];
        for (actor, command) in &commands {
            let recorded = engine.apply_command(*actor, command).unwrap();
            let replayed = replay.apply_command(*actor, command).unwrap();
            assert_eq!(recorded, replayed);
            assert_eq!(engine.state.command_index, replay.state.command_index);
            assert_eq!(
                engine.diagnostic_snapshot().unwrap(),
                replay.diagnostic_snapshot().unwrap()
            );
        }
        assert_eq!(engine.state.players[0].life, 18);
        assert_eq!(engine.state.players[2].life, 18);
        assert_eq!(engine.state.players[1].hand.len(), 7);
        assert_eq!(engine.state.players[1].graveyard.len(), 1);
    }
}

#[derive(serde::Serialize, Debug, Clone)]
struct DrawApplication {
    handle: u32,
    identity: DrawReplacementIdentity,
    action: DrawReplacementAction,
    presentation: crate::state::ReplacementSourcePresentation,
}

#[derive(serde::Serialize, Debug, Clone, Copy)]
enum DrawReplacementAction {
    Double,
    WinInstead,
    Library(LibraryDrawReplacement),
    ZurWeirding,
}

enum AppliedDrawProgress {
    Continue,
    Parked,
    GameEnded,
}

#[derive(serde::Serialize, Debug, Clone)]
enum DrawLibraryStage {
    Optional,
    ChooseKind,
    ChooseToHand(Vec<(ObjectId, u64)>),
    OrderBottom(Vec<(ObjectId, u64)>),
    ZurPayment {
        payers: Vec<PlayerId>,
        cursor: usize,
        pay_intents: Vec<PlayerId>,
    },
}

#[derive(serde::Serialize, Debug, Clone)]
struct DrawLibraryAction {
    source: ObjectId,
    source_label: String,
    stage: DrawLibraryStage,
    reveal: Option<rv1::CardsRevealed>,
}

#[derive(serde::Serialize, Debug, Clone)]
struct DrawNode {
    applied: Vec<DrawReplacementIdentity>,
}

#[derive(serde::Serialize, Debug, Clone)]
pub(crate) enum DrawCompletion {
    ResumeEffects {
        stack: ParkedStackResolution,
        result: EffectResult,
    },
    BeginDiscard {
        stack: ParkedStackResolution,
        player: PlayerId,
        count: u32,
        result: EffectResult,
    },
    FinishDrawStep {
        active_player: PlayerId,
        occurrence: u64,
    },
    BrainstormPutBack {
        stack: ParkedStackResolution,
        step: u32,
        scratch: Vec<ObjectId>,
    },
}

impl DrawCompletion {
    fn stack(&self) -> Option<&ParkedStackResolution> {
        match self {
            Self::ResumeEffects { stack, .. }
            | Self::BeginDiscard { stack, .. }
            | Self::BrainstormPutBack { stack, .. } => Some(stack),
            Self::FinishDrawStep { .. } => None,
        }
    }
    fn transfer_stack(&mut self, parent: Option<ParkedStackResolution>) {
        if let Some(parent) = parent {
            match self {
                Self::ResumeEffects { stack, .. }
                | Self::BeginDiscard { stack, .. }
                | Self::BrainstormPutBack { stack, .. } => *stack = parent,
                Self::FinishDrawStep { .. } => {}
            }
        }
    }
}

#[derive(serde::Serialize, Debug, Clone)]
struct DrawRequest {
    player: PlayerId,
    remaining: u32,
    drawn: u32,
    failed: bool,
}

#[derive(serde::Serialize, Debug, Clone)]
pub(crate) struct PendingDrawTransaction {
    requests: VecDeque<DrawRequest>,
    nodes: Vec<DrawNode>,
    applications: Vec<DrawApplication>,
    completion: DrawCompletion,
    label: String,
    receipts: Vec<TriggerObjectRef>,
    action: Option<DrawLibraryAction>,
}

pub(super) struct CompletedDraw {
    pub completion: DrawCompletion,
    pub receipts: Vec<TriggerObjectRef>,
}
pub(super) enum DrawProgress {
    Complete(Box<CompletedDraw>),
    Parked,
    GameEnded,
}

pub(super) fn controller_library_empty(state: &GameState, controller: PlayerId) -> bool {
    state.player_idx(controller).is_some_and(|index| {
        !state.players[index].has_lost && state.players[index].library.is_empty()
    })
}

impl GameEngine {
    fn draw_candidates(
        &self,
        drawer: PlayerId,
        node: &DrawNode,
    ) -> Vec<(DrawReplacementIdentity, DrawReplacementAction)> {
        let first_own_step = self.state.turn_step == TurnStep::Draw
            && self.state.active_player_id() == drawer
            && self
                .state
                .draw_step_progress
                .is_some_and(|(player, _, count)| player == drawer && count == 0);
        let mut sources = self.state.objects.keys().copied().collect::<Vec<_>>();
        sources.sort_unstable();
        let mut result = Vec::new();
        for source in sources {
            if !printed_static_source_is_available(&self.state, self.registry, source) {
                continue;
            }
            let source_controller = self.characteristics(source).map(|value| value.controller);
            for ability in self.active_static_abilities(source) {
                let action = match ability.definition {
                    StaticAbilityDef::ZurWeirding => DrawReplacementAction::ZurWeirding,
                    StaticAbilityDef::DoubleControllerDraws { condition }
                        if source_controller == Some(drawer) =>
                    {
                        if condition
                            == DrawReplacementCondition::ExceptFirstSuccessfulDrawInOwnDrawStep
                            && first_own_step
                        {
                            continue;
                        }
                        DrawReplacementAction::Double
                    }
                    StaticAbilityDef::WinControllerInsteadOfEmptyLibraryDraw
                        if source_controller == Some(drawer)
                            && controller_library_empty(&self.state, drawer) =>
                    {
                        DrawReplacementAction::WinInstead
                    }
                    StaticAbilityDef::ReplaceControllerDrawWithLibraryChoice { kind }
                        if source_controller == Some(drawer) =>
                    {
                        DrawReplacementAction::Library(kind)
                    }
                    _ => continue,
                };
                let identity = DrawReplacementIdentity {
                    source,
                    generation: self
                        .state
                        .zone_change_generation
                        .get(&source)
                        .copied()
                        .unwrap_or(0),
                    definition: ability_definition_from(
                        &self.state,
                        self.registry,
                        source,
                        self.state.objects[&source].face_up_index,
                        vec![ability.ability_id.clone()],
                    ),
                };
                if !node.applied.contains(&identity) {
                    result.push((identity, action));
                }
            }
        }
        result
    }

    pub(super) fn start_draw_transaction(
        &mut self,
        requests: Vec<(PlayerId, u32)>,
        completion: DrawCompletion,
        label: &str,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<DrawProgress, EngineError> {
        self.advance_draw_transaction(
            PendingDrawTransaction {
                requests: requests
                    .into_iter()
                    .map(|(player, remaining)| DrawRequest {
                        player,
                        remaining,
                        drawn: 0,
                        failed: false,
                    })
                    .collect(),
                nodes: Vec::new(),
                applications: Vec::new(),
                completion,
                label: label.to_string(),
                receipts: Vec::new(),
                action: None,
            },
            events,
        )
    }

    fn advance_draw_transaction(
        &mut self,
        mut work: PendingDrawTransaction,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<DrawProgress, EngineError> {
        if self.state.is_terminal() {
            return Ok(DrawProgress::GameEnded);
        }
        while !work.requests.is_empty() {
            let player = work.requests.front().expect("current draw request").player;
            let Some(index) = self
                .state
                .player_idx(player)
                .filter(|index| !self.state.players[*index].has_lost)
            else {
                work.requests.pop_front();
                work.nodes.clear();
                work.action = None;
                continue;
            };
            if work.action.is_some() {
                self.advance_zur_payment_choice(&mut work, events)?;
                if work.action.is_none() {
                    continue;
                }
                return Ok(self.park_draw_library_action(work, events));
            }
            let request = work.requests.front_mut().expect("current draw request");
            if work.nodes.is_empty() {
                if request.remaining == 0 {
                    events.push(ev_log(format!(
                        "P{} draws {} {} ({}).",
                        request.player,
                        request.drawn,
                        if request.drawn == 1 { "card" } else { "cards" },
                        work.label
                    )));
                    if request.failed {
                        events.push(ev_log(format!("P{} attempted to draw more cards than remained in the library; loss pending until the next state-based-action check (CR 121.4/704.5b; CR 704.4).", request.player)));
                    }
                    work.requests.pop_front();
                    continue;
                }
                request.remaining -= 1;
                work.nodes.push(DrawNode {
                    applied: Vec::new(),
                });
            }
            let node = work.nodes.last().expect("current draw node");
            let candidates = self.draw_candidates(request.player, node);
            if candidates.len() > 1 {
                work.applications.clear();
                for (identity, action) in candidates {
                    let handle = self.state.next_replacement_application_id;
                    self.state.next_replacement_application_id = handle.checked_add(1).ok_or(
                        EngineError::Illegal("replacement application ids exhausted"),
                    )?;
                    work.applications.push(DrawApplication {
                        handle,
                        presentation: self.replacement_source_presentation(identity.source),
                        identity,
                        action,
                    });
                }
                let player = request.player;
                let stack = work.completion.stack().cloned();
                let source_object_id = stack.as_ref().map_or(0, |stack| stack.item.id);
                let candidates = work.applications.iter().map(|value| value.handle).collect();
                self.state.pending_replacement_event =
                    Some(PendingReplacementEvent::Draw(Box::new(work)));
                self.state.pending_resolution = Some(PendingResolution {
                    deciding_player: player,
                    presentation: PendingResolutionPresentation {
                        source_object_id,
                        candidates,
                        min: 1,
                        max: 1,
                        ordered: false,
                        unique_names: false,
                        prompt: "Choose which draw replacement applies next.".to_string(),
                        choice_kind: rv1::ChoiceKind::ReplacementEffect,
                    },
                    continuation: ResolutionContinuation::DrawReplacement { stack },
                });
                events.push(
                    self.draw_replacement_choice_event()
                        .expect("parked draw choice"),
                );
                return Ok(DrawProgress::Parked);
            }
            if let Some((identity, action)) = candidates.into_iter().next() {
                match self.apply_draw_replacement(&mut work, identity, action, events)? {
                    AppliedDrawProgress::GameEnded => return Ok(DrawProgress::GameEnded),
                    AppliedDrawProgress::Parked => {
                        return Ok(self.park_draw_library_action(work, events))
                    }
                    AppliedDrawProgress::Continue => {}
                }
                continue;
            }
            work.nodes.pop().expect("current draw node");
            if let Some(oid) = self.state.players[index].library.front().copied() {
                resolution::draw_card(&mut self.state, self.registry, request.player)?;
                if self.state.turn_step == TurnStep::Draw
                    && self.state.active_player_id() == request.player
                {
                    if let Some((player, _, count)) = self.state.draw_step_progress.as_mut() {
                        if *player == request.player {
                            *count = count.saturating_add(1);
                        }
                    }
                }
                work.receipts.push(TriggerObjectRef {
                    object_id: oid,
                    zone_change_generation: self.state.zone_change_generation[&oid],
                    controller_at_event: request.player,
                });
                request.drawn += 1;
                self.fire_card_drawn(request.player, events);
            } else {
                self.state.players[index].pending_library_loss = true;
                request.failed = true;
            }
        }
        Ok(DrawProgress::Complete(Box::new(CompletedDraw {
            completion: work.completion,
            receipts: work.receipts,
        })))
    }

    fn apply_draw_replacement(
        &mut self,
        work: &mut PendingDrawTransaction,
        identity: DrawReplacementIdentity,
        action: DrawReplacementAction,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<AppliedDrawProgress, EngineError> {
        let mut node = work.nodes.pop().expect("current draw node");
        let source = identity.source;
        node.applied.push(identity);
        match action {
            DrawReplacementAction::Double => {
                // Children inherit ancestors; a subsequently modified sibling never changes them.
                work.nodes.push(node.clone());
                work.nodes.push(node);
                Ok(AppliedDrawProgress::Continue)
            }
            DrawReplacementAction::WinInstead => {
                // CR 614.6: consume the draw before attempting the replacement's win. Even a
                // future prohibited win cannot restore this node or create a failed draw.
                self.state
                    .outcome
                    .get_or_insert(crate::state::GameOutcome::Winner(
                        work.requests.front().expect("drawer").player,
                    ));
                Ok(AppliedDrawProgress::GameEnded)
            }
            DrawReplacementAction::Library(kind) => {
                let player = work.requests.front().expect("drawer").player;
                let stage = match kind {
                    LibraryDrawReplacement::RevealUntilLandOrNonland => {
                        // Declining leaves the modified node with this identity already applied.
                        work.nodes.push(node);
                        DrawLibraryStage::Optional
                    }
                    LibraryDrawReplacement::LookAtTopThree => {
                        let index = self.state.player_idx(player).expect("drawer");
                        let cards = self.state.players[index]
                            .library
                            .iter()
                            .take(3)
                            .copied()
                            .collect::<Vec<_>>();
                        if cards.is_empty() {
                            return Ok(AppliedDrawProgress::Continue);
                        }
                        DrawLibraryStage::ChooseToHand(super::library_choices::capture(
                            self, &cards,
                        ))
                    }
                };
                work.action = Some(DrawLibraryAction {
                    source,
                    source_label: super::events::object_display_name(
                        &self.state,
                        self.registry,
                        source,
                    ),
                    stage,
                    reveal: None,
                });
                Ok(AppliedDrawProgress::Parked)
            }
            DrawReplacementAction::ZurWeirding => {
                let drawer = work.requests.front().expect("drawer").player;
                let drawer_index = self.state.player_idx(drawer).expect("drawer");
                let revealed_card = self.state.players[drawer_index].library.front().copied();
                let mut reveal = revealed_card.and_then(|oid| {
                    super::reveals::reveal_choice(
                        &self.state,
                        self.registry,
                        &[oid],
                        source,
                        &super::events::object_display_name(&self.state, self.registry, source),
                    )
                });
                if let Some(reveal) = reveal.as_mut() {
                    reveal.reveal_id =
                        format!("draw:{}:{}:{}", self.state.command_index, source, drawer);
                    events.push(rv1::RuledEvent {
                        ev: Some(rv1::ruled_event::Ev::CardsRevealed(reveal.clone())),
                    });
                }
                let mut payers = self
                    .state
                    .players
                    .iter()
                    .filter(|player| player.id != drawer && !player.has_lost)
                    .map(|player| player.id)
                    .collect::<Vec<_>>();
                payers.sort_by_key(|player| self.state.apnap_rank(*player));
                work.nodes.push(node);
                work.action = Some(DrawLibraryAction {
                    source,
                    source_label: super::events::object_display_name(
                        &self.state,
                        self.registry,
                        source,
                    ),
                    stage: DrawLibraryStage::ZurPayment {
                        payers,
                        cursor: 0,
                        pay_intents: Vec::new(),
                    },
                    reveal,
                });
                Ok(AppliedDrawProgress::Parked)
            }
        }
    }

    fn advance_zur_payment_choice(
        &mut self,
        work: &mut PendingDrawTransaction,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<(), EngineError> {
        let Some(action) = work.action.as_mut() else {
            return Ok(());
        };
        let DrawLibraryStage::ZurPayment { payers, cursor, .. } = &mut action.stage else {
            return Ok(());
        };
        while let Some(payer) = payers.get(*cursor).copied() {
            let still_in_game = self
                .state
                .player_idx(payer)
                .is_some_and(|index| !self.state.players[index].has_lost);
            if still_in_game {
                break;
            }
            *cursor += 1;
        }
        let complete = *cursor >= payers.len();
        if complete {
            self.finish_zur_payment(work, events)?;
        }
        Ok(())
    }

    fn finish_zur_payment(
        &mut self,
        work: &mut PendingDrawTransaction,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<(), EngineError> {
        let action = work.action.take().expect("Zur payment action");
        let DrawLibraryStage::ZurPayment { pay_intents, .. } = action.stage else {
            unreachable!("Zur payment stage")
        };
        let drawer = work.requests.front().expect("drawer").player;
        let revealed_card = action
            .reveal
            .as_ref()
            .and_then(|reveal| reveal.cards.first())
            .map(|card| (card.object_id, card.zone_change_generation));
        let revealed_card_is_current = match revealed_card {
            None => self
                .state
                .player_idx(drawer)
                .is_some_and(|index| self.state.players[index].library.is_empty()),
            Some((oid, generation)) => {
                self.state
                    .player_idx(drawer)
                    .is_some_and(|index| self.state.players[index].library.front() == Some(&oid))
                    && self.state.objects.get(&oid).is_some_and(|object| {
                        object.zone == Zone::Library
                            && self
                                .state
                                .zone_change_generation
                                .get(&oid)
                                .copied()
                                .unwrap_or(0)
                                == generation
                    })
            }
        };
        let payer_intents = if revealed_card_is_current {
            pay_intents
        } else {
            Vec::new()
        };
        let payable = payer_intents
            .into_iter()
            .filter_map(|payer| {
                let index = self.state.player_idx(payer)?;
                let player = &self.state.players[index];
                let life_lost = self.state.turn_history.current.player(payer).life_lost;
                (payer != drawer
                    && !player.has_lost
                    && player.life >= 2
                    && player.life.checked_sub(2).is_some()
                    && life_lost.checked_add(2).is_some())
                .then_some((payer, index))
            })
            .collect::<Vec<_>>();
        if !payable.is_empty() {
            for (_, index) in &payable {
                super::history::commit_life_change(&mut self.state, *index, -2);
            }
            for (payer, index) in &payable {
                events.push(rv1::RuledEvent {
                    ev: Some(rv1::ruled_event::Ev::LifeChanged(rv1::LifeChanged {
                        player_id: *payer,
                        new_total: self.state.players[*index].life,
                        delta: -2,
                    })),
                });
                events.push(super::events::ev_log(format!(
                    "P{payer} pays 2 life for Zur's Weirding."
                )));
            }
            if let Some((oid, _)) = revealed_card {
                let card_name = super::events::object_display_name(&self.state, self.registry, oid);
                resolution::move_object_to_zone(
                    &mut self.state,
                    self.registry,
                    oid,
                    Zone::Graveyard,
                    None,
                )?;
                events.push(super::events::ev_log(format!(
                    "P{drawer} puts {card_name} into its owner's graveyard (Zur's Weirding)."
                )));
            }
            work.nodes.pop().expect("consumed Zur draw node");
        }
        work.action = None;
        Ok(())
    }

    pub(super) fn draw_replacement_choice_event(&self) -> Option<rv1::RuledEvent> {
        let pending = self.state.pending_resolution.as_ref()?;
        let Some(PendingReplacementEvent::Draw(work)) = &self.state.pending_replacement_event
        else {
            return None;
        };
        if work.action.is_some() {
            return Some(rv1::RuledEvent {
                ev: Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(
                    self.draw_library_choice(work),
                )),
            });
        }
        Some(rv1::RuledEvent {
            ev: Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(
                rv1::ResolutionChoiceRequired {
                    variable_mana_contribution: false,
                    deciding_player_id: pending.deciding_player,
                    source_object_id: pending.presentation.source_object_id,
                    prompt_text: pending.presentation.prompt.clone(),
                    choice_kind: rv1::ChoiceKind::ReplacementEffect as i32,
                    candidate_object_ids: pending.presentation.candidates.clone(),
                    min: 1,
                    max: 1,
                    candidate_selectable: vec![true; work.applications.len()],
                    replacement_options: work
                        .applications
                        .iter()
                        .map(|value| {
                            value.presentation.option(
                                value.handle,
                                match value.action {
                                    DrawReplacementAction::Double => "Draw two cards instead.",
                                    DrawReplacementAction::WinInstead => "Win the game instead.",
                                    DrawReplacementAction::Library(LibraryDrawReplacement::LookAtTopThree) => "Look at three cards and put one into your hand instead.",
                                    DrawReplacementAction::Library(LibraryDrawReplacement::RevealUntilLandOrNonland) => "Choose whether to replace this draw with a land/nonland reveal.",
                                    DrawReplacementAction::ZurWeirding => "Reveal this card; another player may pay 2 life to put it into its owner's graveyard.",
                                }
                                .to_string(),
                            )
                        })
                        .collect(),
                    ..Default::default()
                },
            )),
        })
    }

    pub(super) fn finish_draw_replacement_choice(
        &mut self,
        pending: PendingResolution,
        answer: &rv1::SubmitResolutionChoice,
        decision: rv1::ResolutionChoiceDecision,
    ) -> Result<RuledEventBatch, EngineError> {
        let Some(PendingReplacementEvent::Draw(work)) = &self.state.pending_replacement_event
        else {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal("draw replacement event missing"));
        };
        if work.action.is_some() {
            return self.finish_draw_library_choice(pending, answer, decision);
        }
        let selected = (decision == rv1::ResolutionChoiceDecision::Unspecified
            && answer.chosen_object_ids.len() == 1
            && answer.chosen_player_ids.is_empty()
            && answer.selected_branch_index == 0
            && answer.cast_spell.is_none()
            && answer.chosen_combat_defender.is_none()
            && answer.payment.is_none()
            && answer.restricted_mana.is_empty()
            && answer.spell_cast_announcement.is_none())
        .then(|| {
            work.applications
                .iter()
                .find(|value| value.handle == answer.chosen_object_ids[0])
        })
        .flatten();
        let Some(selected) = selected else {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal(
                "choose one published draw replacement",
            ));
        };
        let current =
            work.requests
                .front()
                .zip(work.nodes.last())
                .is_some_and(|(request, node)| {
                    self.draw_candidates(request.player, node)
                        .iter()
                        .any(|(identity, _)| identity == &selected.identity)
                });
        if !current {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal("draw replacement source is stale"));
        }
        let identity = selected.identity.clone();
        let action = selected.action;
        let Some(PendingReplacementEvent::Draw(work)) = self.state.pending_replacement_event.take()
        else {
            unreachable!()
        };
        let mut work = *work;
        let ResolutionContinuation::DrawReplacement { stack } = pending.continuation else {
            unreachable!()
        };
        work.completion.transfer_stack(stack);
        let mut events = Vec::new();
        match self.apply_draw_replacement(&mut work, identity, action, &mut events)? {
            AppliedDrawProgress::GameEnded => return Ok(finish_with_events(self, events)),
            AppliedDrawProgress::Parked => {
                self.park_draw_library_action(work, &mut events);
                return Ok(finish_with_events(self, events));
            }
            AppliedDrawProgress::Continue => {}
        }
        match self.advance_draw_transaction(work, &mut events)? {
            DrawProgress::GameEnded => Ok(finish_with_events(self, events)),
            DrawProgress::Parked => Ok(finish_with_events(self, events)),
            DrawProgress::Complete(completed) => self.complete_draw_transaction(completed, events),
        }
    }

    pub(super) fn complete_draw_transaction(
        &mut self,
        completed: Box<CompletedDraw>,
        mut events: Vec<rv1::RuledEvent>,
    ) -> Result<RuledEventBatch, EngineError> {
        match completed.completion {
            DrawCompletion::ResumeEffects { stack, mut result } => {
                result.produced_objects.extend(completed.receipts);
                self.complete_parked_resolution_with_previous(
                    stack.item,
                    stack.resume_effect_index,
                    result,
                    events,
                )
            }
            DrawCompletion::BeginDiscard {
                stack,
                player,
                count,
                mut result,
            } => {
                result.produced_objects.extend(completed.receipts);
                resolution::zones::resume_draw_then_discard(
                    self, stack, player, count, result, events,
                )
            }
            DrawCompletion::FinishDrawStep {
                active_player,
                occurrence,
            } => {
                self.finish_draw_step_action(active_player, occurrence, &mut events)?;
                self.apply_sbas(&mut events)?;
                Ok(finish_with_events(self, events))
            }
            DrawCompletion::BrainstormPutBack {
                stack,
                step,
                scratch,
            } => {
                self.finish_brainstorm_draw(stack.item, step, scratch, &mut events)?;
                if self.state.pending_resolution.is_none() {
                    self.apply_sbas(&mut events)?;
                    events.push(ev_priority_changed(self));
                }
                Ok(finish_with_events(self, events))
            }
        }
    }

    /// An accepted concession can remove the drawer, one replacement source, or the resolving
    /// spell itself. Reevaluate the parked node without applying the vanished choice handle.
    pub(super) fn reconcile_draw_departure(
        &mut self,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<(), EngineError> {
        if !matches!(
            self.state.pending_replacement_event,
            Some(PendingReplacementEvent::Draw(_))
        ) {
            return Ok(());
        }
        let pending = self.state.pending_resolution.take();
        let Some(PendingReplacementEvent::Draw(work)) = self.state.pending_replacement_event.take()
        else {
            unreachable!()
        };
        let Some(pending) = pending.filter(|_| !self.state.is_terminal()) else {
            return Ok(());
        };
        let ResolutionContinuation::DrawReplacement { stack } = pending.continuation else {
            unreachable!()
        };
        let mut work = *work;
        work.completion.transfer_stack(stack);
        work.applications.clear();
        match self.advance_draw_transaction(work, events)? {
            DrawProgress::GameEnded => {}
            DrawProgress::Parked => {}
            DrawProgress::Complete(done) => {
                let batch = self.complete_draw_transaction(done, std::mem::take(events))?;
                *events = batch.events;
            }
        }
        Ok(())
    }

    pub(super) fn finish_draw_step_action(
        &mut self,
        active_player: PlayerId,
        occurrence: u64,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<(), EngineError> {
        // Automatic priority settlement bypasses the ordinary command boundary. The
        // turn-based draw has finished, so perform its deferred loss before publishing
        // draw-step triggers or permitting another automatic pass (CR 504.2/704.5b).
        if self
            .state
            .players
            .iter()
            .any(|player| player.id == active_player && player.pending_library_loss)
        {
            events.push(ev_log(if self.state.players.len() == 2 {
                "Game over: empty library on draw".to_string()
            } else {
                format!("P{active_player} loses: empty library on draw")
            }));
        }
        self.commit_pending_library_losses();
        self.sweep_life();
        self.reconcile_departed_players(events)?;
        if self
            .state
            .draw_step_progress
            .is_some_and(|(player, step, _)| player == active_player && step == occurrence)
            && self
                .state
                .players
                .iter()
                .any(|player| player.id == active_player && !player.has_lost)
        {
            self.state.passes_since_stack_change = 0;
            self.fire_triggers(
                &[GameEvent::PhaseBegan {
                    phase: rv1::PhaseId::Draw,
                    active_player,
                }],
                events,
            );
            self.flush_staged_triggers(events);
            if self.state.blocking_choice().is_none() {
                events.push(ev_priority_changed(self));
            }
        }
        Ok(())
    }
}
