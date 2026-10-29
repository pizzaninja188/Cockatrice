//! Prepared-state regressions for Kaya's direct legacy multi-opponent Scry1 instruction.
use super::*;

#[test]
fn each_opponent_may_scry_batches_private_looks_and_defers_moves() {
    let mut engine = four_player_engine(711_281);
    engine.state.active_player_idx = 2;
    for player in &mut engine.state.players {
        player.library.clear();
    }
    engine.state.next_object_id = 800_000;
    let p2_top = put_library_card(&mut engine, 2, "forest");
    let p2_next = put_library_card(&mut engine, 2, "mountain");
    let p3_top = put_library_card(&mut engine, 3, "swamp");
    let p1_top = put_library_card(&mut engine, 1, "island");
    let p1_next = put_library_card(&mut engine, 1, "plains");

    push_multiscry_item(
        &mut engine,
        0,
        vec![SpellEffectKind::EachOpponentMayScry { count: 1 }],
    );

    let mut events = Vec::new();
    let progress = engine
        .resolve_top_of_stack(&mut events)
        .expect("EachOpponentMayScry should begin its first player choice");

    assert_eq!(progress, ResolutionProgress::Parked);
    engine
        .publish_resolution_prompt_ids(&mut events)
        .expect("first opt-in prompt gets its engine ID");
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("first opponent's opt-in is pending");
    assert_eq!(
        pending.deciding_player, 2,
        "the active player is first APNAP"
    );

    let first_prompt_id = events
        .iter()
        .find_map(resolution_choice)
        .expect("first opt-in prompt")
        .resolution_prompt_id;
    assert_eq!(
        events
            .iter()
            .find_map(resolution_choice)
            .unwrap()
            .deciding_player_id,
        2
    );
    let before_wrong_actor = serde_json::to_value(&engine.state).unwrap();
    assert!(submit_branch(&mut engine, 1, 0).is_err());
    assert_eq!(
        before_wrong_actor,
        serde_json::to_value(&engine.state).unwrap()
    );

    submit_branch(&mut engine, 2, 0).expect("active opponent accepts Scry 1");
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .deciding_player,
        3
    );
    submit_branch(&mut engine, 3, 1).expect("next APNAP opponent declines");
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .deciding_player,
        1
    );

    let looked_batch = submit_branch(&mut engine, 1, 0)
        .expect("last APNAP opponent accepts and opens the placement stage");
    let look_logs = private_look_logs(&looked_batch.events);
    assert_eq!(
        look_logs.len(),
        2,
        "only accepting present subjects get full looks"
    );
    assert_eq!(look_logs[0].visible_to_player_id, Some(2));
    assert_eq!(
        look_logs[0]
            .private_card_look
            .as_ref()
            .unwrap()
            .viewer_player_id,
        Some(2)
    );
    assert_eq!(
        look_logs[0]
            .private_card_look
            .as_ref()
            .unwrap()
            .external_card_names,
        [engine.registry.get("forest").unwrap().name.to_string()]
    );
    assert_eq!(look_logs[1].visible_to_player_id, Some(1));
    assert_eq!(
        look_logs[1]
            .private_card_look
            .as_ref()
            .unwrap()
            .viewer_player_id,
        Some(1)
    );
    assert_eq!(
        look_logs[1]
            .private_card_look
            .as_ref()
            .unwrap()
            .external_card_names,
        [engine.registry.get("island").unwrap().name.to_string()]
    );
    assert!(private_look_logs(&looked_batch.events)
        .iter()
        .all(|log| log.hidden_from_player_id.is_none()));
    assert!(look_logs.iter().all(|log| {
        !log.text.contains(&p2_top.to_string())
            && !log.text.contains(&p1_top.to_string())
            && !log.text.contains(&p3_top.to_string())
    }));
    assert!(looked_batch
        .events
        .iter()
        .all(|event| match event.ev.as_ref() {
            Some(rv1::ruled_event::Ev::Log(log)) if log.private_card_look.is_none() => {
                !log.text.contains("Forest") && !log.text.contains("Island")
            }
            _ => true,
        }));
    let first_placement_index = looked_batch
        .events
        .iter()
        .position(|event| {
            matches!(
                event.ev,
                Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(_))
            )
        })
        .expect("first placement prompt");
    assert!(looked_batch
        .events
        .iter()
        .enumerate()
        .all(|(index, event)| {
            private_look_logs(std::slice::from_ref(event)).is_empty()
                || index < first_placement_index
        }));
    let placement = looked_batch
        .events
        .iter()
        .find_map(resolution_choice)
        .expect("first scry placement prompt");
    assert_eq!(placement.deciding_player_id, 2);
    assert_eq!(placement.candidate_object_ids, [p2_top]);
    let p2_before = library(&engine, 2);
    let p1_before = library(&engine, 1);

    let stale_prompt_state = serde_json::to_value(&engine.state).unwrap();
    let stale_prompt = rv1::SubmitResolutionChoice {
        resolution_prompt_id: first_prompt_id,
        decision: rv1::ResolutionChoiceDecision::SelectBranch as i32,
        selected_branch_index: 0,
        ..Default::default()
    };
    assert!(submit_answer(&mut engine, 2, stale_prompt).is_err());
    assert_eq!(
        stale_prompt_state,
        serde_json::to_value(&engine.state).unwrap()
    );

    let wrong_stage_state = serde_json::to_value(&engine.state).unwrap();
    assert!(submit_branch(&mut engine, 2, 0).is_err());
    assert_eq!(
        wrong_stage_state,
        serde_json::to_value(&engine.state).unwrap()
    );

    let first_placement =
        submit_objects(&mut engine, 2, vec![p2_top]).expect("first subject chooses bottom");
    assert_eq!(
        library(&engine, 2),
        p2_before,
        "first answer must not move any card"
    );
    assert_eq!(
        library(&engine, 1),
        p1_before,
        "later subject's library remains untouched"
    );
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .deciding_player,
        1
    );
    let decision_log = first_placement
        .events
        .iter()
        .find_map(|event| match event.ev.as_ref() {
            Some(rv1::ruled_event::Ev::Log(log)) if log.visible_to_player_id.is_none() => Some(log),
            _ => None,
        })
        .expect("public identity-free placement decision");
    assert!(decision_log.text.contains("P2"));
    assert!(!decision_log.text.contains("Forest"));
    assert!(!decision_log.text.contains(&p2_top.to_string()));

    let pending_card = match &engine
        .state
        .pending_resolution
        .as_ref()
        .unwrap()
        .continuation
    {
        ResolutionContinuation::EachOpponentMayScry { scry, .. } => scry
            .cards
            .iter()
            .find(|card| card.subject == 1)
            .unwrap()
            .clone(),
        other => panic!("expected multiscry continuation, got {other:?}"),
    };
    engine.state.zone_change_generation.insert(
        pending_card.object_id,
        pending_card.zone_change_generation + 1,
    );
    let stale_state = serde_json::to_value(&engine.state).unwrap();
    assert!(submit_objects(&mut engine, 1, Vec::new()).is_err());
    assert_eq!(stale_state, serde_json::to_value(&engine.state).unwrap());
    engine
        .state
        .zone_change_generation
        .insert(pending_card.object_id, pending_card.zone_change_generation);

    let final_batch = submit_objects(&mut engine, 1, Vec::new())
        .expect("final subject keeps their card on top and commits all answers");
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(library(&engine, 2), [p2_next, p2_top]);
    assert_eq!(library(&engine, 1), [p1_top, p1_next]);
    assert_eq!(library(&engine, 3), [p3_top]);
    let final_public = final_batch
        .events
        .iter()
        .find_map(|event| match event.ev.as_ref() {
            Some(rv1::ruled_event::Ev::Log(log)) if log.visible_to_player_id.is_none() => Some(log),
            _ => None,
        })
        .expect("final identity-free decision");
    assert!(!final_public.text.contains("Island"));
    assert!(!final_public.text.contains(&p1_top.to_string()));
}

#[test]
fn draw_two_replacement_finishes_before_any_multiscry_opt_in() {
    let mut engine = four_player_engine(711_282);
    engine.state.active_player_idx = 0;
    for player in &mut engine.state.players {
        player.library.clear();
    }
    engine.state.next_object_id = 810_000;
    put_library_card(&mut engine, 0, "forest");
    put_library_card(&mut engine, 0, "mountain");
    put_library_card(&mut engine, 0, "island");
    put_battlefield_card(&mut engine, 0, "abundance");
    let initial_hand = engine.state.players[engine.state.player_idx(0).unwrap()]
        .hand
        .len();
    push_multiscry_item(
        &mut engine,
        0,
        vec![
            SpellEffectKind::Draw {
                who: tricerules_card_model::primitives::PlayerRecipient::Controller,
                count: tricerules_card_model::primitives::Amount::Fixed(2),
            },
            SpellEffectKind::EachOpponentMayScry { count: 1 },
        ],
    );

    let mut initial_events = Vec::new();
    assert_eq!(
        engine.resolve_top_of_stack(&mut initial_events).unwrap(),
        ResolutionProgress::Parked
    );
    engine
        .publish_resolution_prompt_ids(&mut initial_events)
        .unwrap();
    assert!(matches!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .map(|pending| &pending.continuation),
        Some(ResolutionContinuation::DrawReplacement { .. })
    ));
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .presentation
            .prompt,
        "Choose whether to replace this draw."
    );
    assert!(private_look_logs(&initial_events).is_empty());
    assert!(initial_events.iter().all(|event| {
        !matches!(
            event.ev,
            Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(_))
        ) || resolution_choice(event)
            .is_some_and(|choice| choice.prompt_text == "Choose whether to replace this draw.")
    }));

    for completed_draw in 1..=2 {
        let pending = engine.state.pending_resolution.as_ref().unwrap();
        assert!(matches!(
            pending.continuation,
            ResolutionContinuation::DrawReplacement { .. }
        ));
        assert!(matches!(
            engine.state.pending_replacement_event,
            Some(crate::engine::replacement::PendingReplacementEvent::Draw(_))
        ));
        let batch = submit_branch(&mut engine, 0, 1)
            .expect("decline Abundance's replacement for this draw");
        if completed_draw == 1 {
            assert!(matches!(
                engine
                    .state
                    .pending_resolution
                    .as_ref()
                    .map(|pending| &pending.continuation),
                Some(ResolutionContinuation::DrawReplacement { .. })
            ));
            assert!(private_look_logs(&batch.events).is_empty());
        }
        initial_events.extend(batch.events);
    }

    assert_eq!(
        engine.state.players[engine.state.player_idx(0).unwrap()]
            .hand
            .len(),
        initial_hand + 2
    );
    assert!(engine.state.pending_replacement_event.is_none());
    assert!(matches!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .map(|pending| &pending.continuation),
        Some(ResolutionContinuation::EachOpponentMayScry { .. })
    ));
    assert!(private_look_logs(&initial_events).is_empty());
    assert!(initial_events.iter().all(|event| {
        !matches!(
            event.ev,
            Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(_))
        ) || resolution_choice(event).is_some_and(|choice| {
            choice.prompt_text == "Choose whether to replace this draw."
                || choice.prompt_text.starts_with("P") && choice.prompt_text.contains("may scry 1")
        })
    }));
}

#[test]
fn accepted_empty_library_scries_complete_without_a_look_or_placement_prompt() {
    let mut engine = four_player_engine(711_283);
    for player in &mut engine.state.players {
        player.library.clear();
    }
    push_multiscry_item(
        &mut engine,
        0,
        vec![SpellEffectKind::EachOpponentMayScry { count: 1 }],
    );

    let mut events = Vec::new();
    assert_eq!(
        engine.resolve_top_of_stack(&mut events).unwrap(),
        ResolutionProgress::Parked
    );
    engine.publish_resolution_prompt_ids(&mut events).unwrap();
    for subject in [1, 2, 3] {
        assert_eq!(
            engine
                .state
                .pending_resolution
                .as_ref()
                .unwrap()
                .deciding_player,
            subject
        );
        let batch = submit_branch(&mut engine, subject, 0).expect("accept empty-library Scry 1");
        events.extend(batch.events);
    }

    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
    assert!(private_look_logs(&events).is_empty());
    assert!(events
        .iter()
        .filter_map(resolution_choice)
        .all(|choice| { choice.prompt_text.contains("may scry 1") }));
    for subject in [1, 2, 3] {
        assert!(events.iter().any(|event| matches!(
            event.ev.as_ref(),
            Some(rv1::ruled_event::Ev::Log(log))
                if log.text == format!("P{subject} scries 1 with an empty library (Brambleguard Captain).")
        )));
    }
}

#[test]
fn actual_concede_during_opt_in_delegates_the_owed_choice_and_excludes_departed_subject() {
    let mut engine = four_player_engine(711_284);
    engine.state.active_player_idx = 2;
    for player in &mut engine.state.players {
        player.library.clear();
    }
    engine.state.next_object_id = 820_000;
    let departed_top = put_library_card(&mut engine, 2, "forest");
    let p3_top = put_library_card(&mut engine, 3, "mountain");
    let p1_top = put_library_card(&mut engine, 1, "island");
    push_multiscry_item(
        &mut engine,
        0,
        vec![SpellEffectKind::EachOpponentMayScry { count: 1 }],
    );

    let mut events = Vec::new();
    assert_eq!(
        engine.resolve_top_of_stack(&mut events).unwrap(),
        ResolutionProgress::Parked
    );
    engine.publish_resolution_prompt_ids(&mut events).unwrap();
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .deciding_player,
        2
    );
    let conceded = concede_player(&mut engine, 2);
    assert_eq!(
        engine
            .state
            .players
            .iter()
            .filter(|player| !player.has_lost)
            .count(),
        3,
        "this uses a real Concede command in a four-player game"
    );
    let pending = engine.state.pending_resolution.as_ref().unwrap();
    assert_eq!(
        pending.deciding_player, 0,
        "the ability controller selects the delegate"
    );
    let (controller, subjects, stage) = match &pending.continuation {
        ResolutionContinuation::EachOpponentMayScry { scry, .. } => {
            (scry.controller, scry.subjects.clone(), scry.stage.clone())
        }
        other => panic!("expected Kaya Scry1 continuation, got {other:?}"),
    };
    assert_eq!(controller, 0);
    assert_eq!(
        subjects,
        [2, 3, 1],
        "the original APNAP cohort remains frozen"
    );
    assert!(matches!(
        stage,
        crate::state::PendingEachOpponentScryStage::ChoosingDelegate {
            choice: crate::state::PendingEachOpponentScryChoice::OptIn { subject_index: 0 },
            ..
        }
    ));

    let delegate_menu = conceded
        .events
        .iter()
        .find_map(resolution_choice)
        .expect("the owed opt-in is delegated");
    assert_eq!(delegate_menu.deciding_player_id, 0);
    assert!(delegate_menu.resolution_branches.len() >= 2);
    let delegation = submit_branch(&mut engine, 0, 0).expect("controller chooses a live delegate");
    let delegate = engine
        .state
        .pending_resolution
        .as_ref()
        .unwrap()
        .deciding_player;
    assert_ne!(delegate, 2);
    assert!(!engine.state.players[engine.state.player_idx(delegate).unwrap()].has_lost);
    let delegated_opt_in = submit_branch(&mut engine, delegate, 0)
        .expect("delegate answers the departed subject's already-owed choice");
    events.extend(conceded.events);
    events.extend(delegation.events);
    events.extend(delegated_opt_in.events);
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .deciding_player,
        3
    );
    events.extend(submit_branch(&mut engine, 3, 0).unwrap().events);
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .deciding_player,
        1
    );
    let cohort = submit_branch(&mut engine, 1, 0).expect("capture the surviving accepted subjects");
    events.extend(cohort.events);

    let accepted_subjects = match &engine
        .state
        .pending_resolution
        .as_ref()
        .unwrap()
        .continuation
    {
        ResolutionContinuation::EachOpponentMayScry { scry, .. } => scry.accepted_subjects.clone(),
        other => panic!("expected Kaya placement continuation, got {other:?}"),
    };
    assert_eq!(
        accepted_subjects,
        [3, 1],
        "the departed subject cannot scry through its delegate"
    );
    assert_eq!(
        private_look_logs(&events)
            .iter()
            .filter_map(|log| log.private_card_look.as_ref()?.viewer_player_id)
            .collect::<Vec<_>>(),
        [3, 1]
    );
    assert!(!private_look_logs(&events)
        .iter()
        .any(|log| log.text.contains("Forest")));
    assert!(!library(&engine, 3).is_empty() && library(&engine, 3)[0] == p3_top);
    assert!(!library(&engine, 1).is_empty() && library(&engine, 1)[0] == p1_top);
    assert!(!library(&engine, 2).contains(&departed_top));
    events.extend(submit_objects(&mut engine, 3, Vec::new()).unwrap().events);
    events.extend(submit_objects(&mut engine, 1, Vec::new()).unwrap().events);
    assert!(engine.state.pending_resolution.is_none());
}

#[test]
fn actual_concede_during_placement_never_uses_the_delegate_library() {
    let mut engine = four_player_engine(711_285);
    engine.state.active_player_idx = 2;
    for player in &mut engine.state.players {
        player.library.clear();
    }
    engine.state.next_object_id = 830_000;
    let _p2_top = put_library_card(&mut engine, 2, "forest");
    let _p2_next = put_library_card(&mut engine, 2, "mountain");
    let p3_top = put_library_card(&mut engine, 3, "island");
    let p1_top = put_library_card(&mut engine, 1, "plains");
    push_multiscry_item(
        &mut engine,
        0,
        vec![SpellEffectKind::EachOpponentMayScry { count: 1 }],
    );
    let mut events = Vec::new();
    engine.resolve_top_of_stack(&mut events).unwrap();
    engine.publish_resolution_prompt_ids(&mut events).unwrap();
    for subject in [2, 3, 1] {
        events.extend(submit_branch(&mut engine, subject, 0).unwrap().events);
    }
    assert_eq!(
        private_look_logs(&events)
            .iter()
            .filter_map(|log| log.private_card_look.as_ref()?.viewer_player_id)
            .collect::<Vec<_>>(),
        [2, 3, 1]
    );
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .deciding_player,
        2
    );
    let p3_before = library(&engine, 3);
    let p1_before = library(&engine, 1);

    let conceded = concede_player(&mut engine, 2);
    assert_eq!(
        engine
            .state
            .players
            .iter()
            .filter(|player| !player.has_lost)
            .count(),
        3,
        "the subject concedes through the command path while placement is owed"
    );
    let delegate_menu = conceded
        .events
        .iter()
        .find_map(resolution_choice)
        .expect("placement for the departed subject is delegated");
    assert_eq!(delegate_menu.deciding_player_id, 0);
    let delegation = submit_branch(&mut engine, 0, 0).unwrap();
    let delegate = engine
        .state
        .pending_resolution
        .as_ref()
        .unwrap()
        .deciding_player;
    assert!(
        delegate != 2 && !engine.state.players[engine.state.player_idx(delegate).unwrap()].has_lost
    );
    let delegated_prompt = submit_branch(&mut engine, delegate, 0).unwrap();
    assert_eq!(
        library(&engine, 3),
        p3_before,
        "P3's library is not substituted for P2's"
    );
    assert_eq!(
        library(&engine, 1),
        p1_before,
        "P1's library is not substituted for P2's"
    );
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .deciding_player,
        3
    );
    events.extend(conceded.events);
    events.extend(delegation.events);
    events.extend(delegated_prompt.events);
    events.extend(submit_objects(&mut engine, 3, Vec::new()).unwrap().events);
    events.extend(submit_objects(&mut engine, 1, Vec::new()).unwrap().events);

    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(library(&engine, 3), p3_before);
    assert_eq!(library(&engine, 1), p1_before);
    assert!(library(&engine, 3)
        .iter()
        .all(|object_id| engine.state.objects[object_id].owner != 2));
    assert!(library(&engine, 1)
        .iter()
        .all(|object_id| engine.state.objects[object_id].owner != 2));
    assert_eq!(library(&engine, 3)[0], p3_top);
    assert_eq!(library(&engine, 1)[0], p1_top);
}

#[test]
fn controller_concede_after_multiscry_begins_keeps_frozen_cohort_and_resumes_tail_once() {
    fn run() -> Vec<rv1::RuledEvent> {
        let mut engine = four_player_engine(711_286);
        engine.state.active_player_idx = 2;
        for player in &mut engine.state.players {
            player.library.clear();
        }
        engine.state.next_object_id = 840_000;
        for (player, card) in [(2, "forest"), (3, "mountain"), (1, "island")] {
            put_library_card(&mut engine, player, card);
        }
        push_multiscry_item(
            &mut engine,
            0,
            vec![
                SpellEffectKind::EachOpponentMayScry { count: 1 },
                SpellEffectKind::GainLife {
                    amount: tricerules_card_model::primitives::Amount::Fixed(2),
                    who: tricerules_card_model::primitives::PlayerRecipient::EachPlayer,
                },
            ],
        );

        let mut events = Vec::new();
        assert_eq!(
            engine.resolve_top_of_stack(&mut events).unwrap(),
            ResolutionProgress::Parked
        );
        engine.publish_resolution_prompt_ids(&mut events).unwrap();
        let before = match &engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .continuation
        {
            ResolutionContinuation::EachOpponentMayScry { scry, .. } => scry.subjects.clone(),
            other => panic!("expected a parked Kaya resolution, got {other:?}"),
        };
        assert_eq!(before, [2, 3, 1]);
        events.extend(concede_player(&mut engine, 0).events);
        let pending = engine.state.pending_resolution.as_ref().unwrap();
        assert_eq!(
            pending.deciding_player, 2,
            "the current subject still owns the opt-in"
        );
        match &pending.continuation {
            ResolutionContinuation::EachOpponentMayScry { stack, scry } => {
                assert_eq!(
                    scry.controller, 0,
                    "the original controller identity is retained"
                );
                assert_eq!(
                    scry.subjects, before,
                    "the opponent cohort is not recomputed"
                );
                assert_eq!(
                    stack.resume_effect_index,
                    Some(1),
                    "the tail cursor remains after the Scry effect"
                );
            }
            other => panic!("expected a surviving Kaya resolution, got {other:?}"),
        }
        for subject in [2, 3, 1] {
            assert_eq!(
                engine
                    .state
                    .pending_resolution
                    .as_ref()
                    .unwrap()
                    .deciding_player,
                subject
            );
            events.extend(submit_branch(&mut engine, subject, 0).unwrap().events);
        }
        for subject in [2, 3, 1] {
            assert_eq!(
                engine
                    .state
                    .pending_resolution
                    .as_ref()
                    .unwrap()
                    .deciding_player,
                subject
            );
            events.extend(
                submit_objects(&mut engine, subject, Vec::new())
                    .unwrap()
                    .events,
            );
        }

        assert!(engine.state.pending_resolution.is_none());
        assert!(engine.state.stack.is_empty());
        assert!(engine.state.players[engine.state.player_idx(0).unwrap()].has_lost);
        for subject in [1, 2, 3] {
            assert_eq!(
                engine.state.players[engine.state.player_idx(subject).unwrap()].life,
                22
            );
        }
        events
    }

    assert_eq!(
        run(),
        run(),
        "the resumed cohort, choices, and tail are deterministic"
    );
}

#[test]
fn controller_concede_before_resolution_removes_the_unbegun_ability() {
    let mut engine = four_player_engine(711_287);
    push_multiscry_item(
        &mut engine,
        0,
        vec![SpellEffectKind::EachOpponentMayScry { count: 1 }],
    );
    let stack_id = engine.state.stack.last().unwrap().id;
    let conceded = concede_player(&mut engine, 0);

    assert_eq!(
        engine
            .state
            .players
            .iter()
            .filter(|player| !player.has_lost)
            .count(),
        3
    );
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.iter().all(|item| item.id != stack_id));
    assert!(conceded.events.iter().all(|event| {
        !matches!(
            event.ev,
            Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(_))
        ) && private_look_logs(std::slice::from_ref(event)).is_empty()
    }));
}

fn four_player_engine(game_id: u64) -> GameEngine {
    GameEngine::new(
        tricerules_cards::registry::global(),
        game_id,
        &[0, 1, 2, 3],
        20,
        None,
        true,
    )
    .expect("four-player engine")
}

fn push_multiscry_item(
    engine: &mut GameEngine,
    controller: PlayerId,
    effects: Vec<SpellEffectKind>,
) {
    let mut ability = tricerules_cards::registry::global()
        .get("brambleguard_captain")
        .expect("trigger fixture")
        .primary_face()
        .triggered_abilities[0]
        .clone();
    ability.effect = effects;
    let mut item = engine.observer_return_item(90_281, controller);
    item.card_id = "brambleguard_captain".to_string();
    item.is_copy = false;
    item.is_triggered = true;
    item.triggered_ability = Some(ability);
    engine.state.stack.push(item);
}

fn put_library_card(engine: &mut GameEngine, player: PlayerId, card_id: &str) -> ObjectId {
    let object_id = engine.state.next_object_id;
    engine.state.next_object_id += 1;
    let face = engine.registry.get(card_id).unwrap().primary_face();
    let object = new_object_from_card(object_id, player, card_id, Zone::Library, face);
    engine.state.objects.insert(object_id, object);
    engine.state.zone_change_generation.insert(object_id, 0);
    let index = engine.state.player_idx(player).unwrap();
    engine.state.players[index].library.push_back(object_id);
    object_id
}

fn put_battlefield_card(engine: &mut GameEngine, player: PlayerId, card_id: &str) -> ObjectId {
    let object_id = engine.state.next_object_id;
    engine.state.next_object_id += 1;
    let face = engine.registry.get(card_id).unwrap().primary_face();
    let object = new_object_from_card(object_id, player, card_id, Zone::Battlefield, face);
    engine.state.objects.insert(object_id, object);
    engine.state.zone_change_generation.insert(object_id, 0);
    let index = engine.state.player_idx(player).unwrap();
    engine.state.players[index].battlefield.push(object_id);
    object_id
}

fn library(engine: &GameEngine, player: PlayerId) -> Vec<ObjectId> {
    let index = engine.state.player_idx(player).unwrap();
    engine.state.players[index]
        .library
        .iter()
        .copied()
        .collect()
}

fn submit_branch(
    engine: &mut GameEngine,
    player: PlayerId,
    index: u32,
) -> Result<RuledEventBatch, EngineError> {
    submit_answer(
        engine,
        player,
        rv1::SubmitResolutionChoice {
            decision: rv1::ResolutionChoiceDecision::SelectBranch as i32,
            selected_branch_index: index,
            ..Default::default()
        },
    )
}

fn submit_objects(
    engine: &mut GameEngine,
    player: PlayerId,
    object_ids: Vec<ObjectId>,
) -> Result<RuledEventBatch, EngineError> {
    submit_answer(
        engine,
        player,
        rv1::SubmitResolutionChoice {
            chosen_object_ids: object_ids,
            ..Default::default()
        },
    )
}

fn submit_answer(
    engine: &mut GameEngine,
    player: PlayerId,
    answer: rv1::SubmitResolutionChoice,
) -> Result<RuledEventBatch, EngineError> {
    engine.apply_scenario_command(
        player,
        &rv1::RuledCommand {
            cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(answer)),
        },
    )
}

fn concede_player(engine: &mut GameEngine, player: PlayerId) -> RuledEventBatch {
    engine
        .apply_scenario_command(
            player,
            &rv1::RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::Concede(rv1::Concede {})),
            },
        )
        .expect("Concede is accepted through the command path")
}

fn resolution_choice(event: &rv1::RuledEvent) -> Option<&rv1::ResolutionChoiceRequired> {
    match event.ev.as_ref() {
        Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(choice)) => Some(choice),
        _ => None,
    }
}

fn private_look_logs(events: &[rv1::RuledEvent]) -> Vec<&rv1::LogMessage> {
    events
        .iter()
        .filter_map(|event| match event.ev.as_ref() {
            Some(rv1::ruled_event::Ev::Log(log)) if log.private_card_look.is_some() => Some(log),
            _ => None,
        })
        .collect()
}
