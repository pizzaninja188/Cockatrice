use super::*;

// Source-backed Onyx -8 sentence and cost, with a labeled synthetic opponent-life sentinel.
// This fixture is neither a registered Professor Onyx nor a normal-session pilot.
const SEVEN_ROUNDS: &str = r#"(
    id: "synthetic_onyx_rounds", name: "Synthetic Onyx Rounds", face_id: "synthetic_onyx_rounds",
    types: ["Planeswalker"], loyalty: 8,
    activated_abilities: [(ability_id: "ultimate", presentation: Fallback, costs: [Loyalty(-8)],
        effect: [EachOpponentMayDiscardOrLoseLife(amount: 3),
            EachOpponentMayDiscardOrLoseLife(amount: 3),
            EachOpponentMayDiscardOrLoseLife(amount: 3),
            EachOpponentMayDiscardOrLoseLife(amount: 3),
            EachOpponentMayDiscardOrLoseLife(amount: 3),
            EachOpponentMayDiscardOrLoseLife(amount: 3),
            EachOpponentMayDiscardOrLoseLife(amount: 3),
            GainLife(amount: 1, who: EachOpponent)],
    )],
)"#;

fn fixture() -> (GameEngine, ObjectId) {
    fixture_with(SEVEN_ROUNDS)
}

fn fixture_with(definition: &str) -> (GameEngine, ObjectId) {
    let registry = CardRegistry::from_chunks_and_tokens(
        &[
            include_str!("../../../../tricerules-cards/data/forest.ron"),
            include_str!("../../../../tricerules-cards/data/library_of_leng.ron"),
            include_str!("../../../../tricerules-cards/data/fiery_temper.ron"),
            definition,
        ],
        &[],
    )
    .unwrap();
    let mut engine = GameEngine::new_with_registry(
        602_454,
        &[17, 29, 41, 53],
        20,
        Some(
            (0..4)
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
    let source = place_ready(&mut engine, 17, "Synthetic Onyx Rounds");
    (engine, source)
}

fn keep_hand_size(engine: &mut GameEngine, actor: PlayerId, size: usize) {
    let index = engine.state.player_idx(actor).unwrap();
    let remove = engine.state.players[index]
        .hand
        .iter()
        .skip(size)
        .copied()
        .collect::<Vec<_>>();
    for object in remove {
        super::super::resolution::move_object_to_zone(
            &mut engine.state,
            engine.registry,
            object,
            Zone::Library,
            None,
        )
        .unwrap();
    }
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
    assert_eq!(
        engine.state.objects[&source].zone,
        Zone::Graveyard,
        "paying -8 removes the source before resolution"
    );
    let mut batch = None;
    for _ in 0..4 {
        batch = Some(
            engine
                .apply_command(
                    engine.state.priority_player_id(),
                    &RuledCommand {
                        cmd: Some(rv1::ruled_command::Cmd::PassPriority(rv1::PassPriority {})),
                    },
                )
                .expect("the typed round resolves after the physical source has gone"),
        );
    }
    batch.unwrap()
}

fn answer(engine: &mut GameEngine, actor: PlayerId, chosen: Vec<ObjectId>) -> rv1::RuledEventBatch {
    let prompt = engine
        .state
        .pending_resolution
        .as_ref()
        .unwrap()
        .resolution_prompt_id
        .get();
    engine
        .apply_command(
            actor,
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
fn opponent_discard_round_empty_first_opponent_does_not_skip_others_or_later_rounds() {
    let (mut engine, source) = fixture();
    keep_hand_size(&mut engine, 29, 0);
    keep_hand_size(&mut engine, 41, 1);
    keep_hand_size(&mut engine, 53, 1);
    engine.state.players[1].life = 2;
    let selected = engine.state.players[2].hand[0];
    let batch = begin(&mut engine, source);
    assert!(!batch
        .events
        .iter()
        .any(|event| matches!(event.ev, Some(rv1::ruled_event::Ev::CardsRevealed(_)))));
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .deciding_player,
        41
    );
    let before_all = answer(&mut engine, 41, vec![selected]);
    assert!(!before_all
        .events
        .iter()
        .any(|event| matches!(event.ev, Some(rv1::ruled_event::Ev::CardsRevealed(_)))));
    assert_eq!(engine.state.objects[&selected].zone, Zone::Hand);
    assert_eq!(
        engine.state.players[1].life, 2,
        "penalties wait for every private choice"
    );
    for round in 1..=7 {
        assert_eq!(
            engine
                .state
                .pending_resolution
                .as_ref()
                .unwrap()
                .deciding_player,
            53
        );
        answer(&mut engine, 53, vec![]);
        if round < 7 {
            assert_eq!(engine.state.players[1].life, 2 - 3 * round);
            assert!(
                !engine.state.players[1].has_lost,
                "life0 opponent remains through all seven rounds"
            );
            assert_eq!(engine.state.players[2].life, 20 - 3 * (round - 1));
        }
    }
    assert_eq!(engine.state.objects[&selected].zone, Zone::Graveyard);
    assert_eq!(
        engine
            .state
            .players
            .iter()
            .map(|p| p.life)
            .collect::<Vec<_>>(),
        [20, -18, 3, 0],
        "each outcome settles seven times and the synthetic tail runs once before SBA"
    );
    assert!(engine.state.players[1].has_lost);
    assert!(!engine.state.players[2].has_lost);
    assert!(engine.state.players[3].has_lost);
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
}

const ONE_ROUND: &str = r#"(
    id: "synthetic_onyx_rounds", name: "Synthetic Onyx Rounds", face_id: "synthetic_onyx_rounds",
    types: ["Planeswalker"], loyalty: 8,
    activated_abilities: [(ability_id: "ultimate", presentation: Fallback, costs: [Loyalty(-8)],
        effect: [EachOpponentMayDiscardOrLoseLife(amount: 3), GainLife(amount: 1, who: EachOpponent)],
    )],
)"#;

fn active(engine: &GameEngine) -> Vec<rv1::CardsRevealed> {
    super::super::reveals::active_reveals(engine)
}

fn replacement_fixture() -> (GameEngine, Vec<ObjectId>, rv1::RuledEventBatch) {
    let (mut engine, source) = fixture_with(ONE_ROUND);
    for actor in [29, 41, 53] {
        keep_hand_size(&mut engine, actor, 1);
    }
    place_ready(&mut engine, 29, "Library of Leng");
    place_ready(&mut engine, 41, "Library of Leng");
    let temper = place_ready(&mut engine, 53, "Fiery Temper");
    keep_hand_size(&mut engine, 53, 0);
    super::super::resolution::move_object_to_zone(
        &mut engine.state,
        engine.registry,
        temper,
        Zone::Hand,
        None,
    )
    .unwrap();
    let cards = engine
        .state
        .players
        .iter()
        .skip(1)
        .map(|p| p.hand[0])
        .collect::<Vec<_>>();
    begin(&mut engine, source);
    answer(&mut engine, 29, vec![cards[0]]);
    answer(&mut engine, 41, vec![cards[1]]);
    let batch = answer(&mut engine, 53, vec![cards[2]]);
    (engine, cards, batch)
}

#[test]
fn opponent_discard_round_reveals_every_owner_until_all_destinations_commit() {
    let (mut engine, cards, batch) = replacement_fixture();
    let receipts = batch
        .events
        .iter()
        .filter_map(|event| match &event.ev {
            Some(rv1::ruled_event::Ev::CardsRevealed(receipt)) => Some(receipt.clone()),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(receipts.len(), 3);
    assert_eq!(
        receipts
            .iter()
            .map(|r| r.zone_owner_player_id)
            .collect::<Vec<_>>(),
        [29, 41, 53]
    );
    assert!(receipts.iter().all(|r| !r.reveal_id.is_empty()));
    assert_eq!(
        active(&engine),
        receipts,
        "all owner receipts survive the first replacement prompt"
    );
    assert!(cards
        .iter()
        .all(|id| engine.state.objects[id].zone == Zone::Hand));
    answer(&mut engine, 29, vec![1]); // Leng top of library, not a payment failure.
    assert_eq!(
        active(&engine),
        receipts,
        "stable immutable receipts survive the next owner"
    );
    assert!(cards
        .iter()
        .all(|id| engine.state.objects[id].zone == Zone::Hand));
    let settled = answer(&mut engine, 41, vec![0]);
    assert_eq!(engine.state.objects[&cards[0]].zone, Zone::Library);
    assert_eq!(engine.state.objects[&cards[1]].zone, Zone::Graveyard);
    assert_eq!(
        engine.state.objects[&cards[2]].zone,
        Zone::Exile,
        "supported madness retains effect-discard routing"
    );
    assert_eq!(
        engine
            .state
            .players
            .iter()
            .map(|p| p.life)
            .collect::<Vec<_>>(),
        [20, 21, 21, 21],
        "all chose to pay, independent of final discard destination"
    );
    assert!(active(&engine).is_empty());
    assert!(settled.events.iter().any(|e| matches!(&e.ev,
        Some(rv1::ruled_event::Ev::ActivePublicRevealSnapshot(s)) if s.reveals.is_empty())));
}

#[test]
fn opponent_discard_round_controller_departure_preserves_paid_prefix_and_remaining_opponents() {
    let (mut engine, source) = fixture_with(ONE_ROUND);
    for actor in [29, 41, 53] {
        keep_hand_size(&mut engine, actor, 1);
    }
    let paid = engine.state.players[1].hand[0];
    begin(&mut engine, source);
    answer(&mut engine, 29, vec![paid]);
    engine
        .apply_command(
            17,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::Concede(rv1::Concede {})),
            },
        )
        .unwrap();
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .deciding_player,
        41
    );
    answer(&mut engine, 41, vec![]);
    answer(&mut engine, 53, vec![]);
    assert_eq!(engine.state.objects[&paid].zone, Zone::Graveyard);
    assert_eq!(
        engine
            .state
            .players
            .iter()
            .skip(1)
            .map(|p| p.life)
            .collect::<Vec<_>>(),
        [21, 18, 18]
    );
    assert!(engine.state.pending_resolution.is_none());
}

#[test]
fn opponent_discard_round_replacement_survives_controller_and_selected_owner_departure() {
    for departing in [17, 29, 41, 53] {
        let (mut engine, cards, _) = replacement_fixture();
        engine
            .apply_command(
                departing,
                &RuledCommand {
                    cmd: Some(rv1::ruled_command::Cmd::Concede(rv1::Concede {})),
                },
            )
            .unwrap();
        let expected_owners = [29, 41, 53]
            .into_iter()
            .filter(|p| *p != departing)
            .collect::<Vec<_>>();
        assert_eq!(
            active(&engine)
                .iter()
                .map(|r| r.zone_owner_player_id)
                .collect::<Vec<_>>(),
            expected_owners
        );
        for actor in [29, 41] {
            if actor != departing {
                answer(&mut engine, actor, vec![1]);
            }
        }
        for (index, actor) in [29, 41, 53].into_iter().enumerate() {
            if actor != departing {
                assert_eq!(
                    engine.state.objects[&cards[index]].zone,
                    if actor == 53 {
                        Zone::Exile
                    } else {
                        Zone::Library
                    }
                );
                assert_eq!(engine.state.players[index + 1].life, 21);
            }
        }
        assert!(active(&engine).is_empty());
    }
}

#[test]
fn opponent_discard_round_saved_replacement_validates_full_cohort_before_reoffer() {
    // Synthetic observer interruption of the real Leng transaction, not a card-generated interrupt.
    for mutation in 0..3 {
        let (mut engine, cards, _) = replacement_fixture();
        answer(&mut engine, 29, vec![1]);
        let mut saved = engine.state.pending_resolution.take().unwrap();
        let replacement = engine.state.pending_replacement_event.take();
        let old_prompt = saved.resolution_prompt_id.get();
        let ResolutionContinuation::DiscardReplacement { stack } = &mut saved.continuation else {
            unreachable!();
        };
        *stack = stack
            .clone()
            .with_previous_result(OperationResults::from_value(ProgramValue::Count(37)));
        let previous = serde_json::to_value(stack.previous_result()).unwrap();
        if mutation == 1 {
            *engine
                .state
                .zone_change_generation
                .entry(cards[0])
                .or_default() += 1;
        }
        if mutation == 2 {
            engine.state.players[1]
                .hand
                .retain(|card| *card != cards[0]);
        }
        let before = engine.diagnostic_snapshot().unwrap();
        let mut events = Vec::new();
        let result =
            engine.restore_suspended_observer_resolution(Some(saved), replacement, &mut events);
        if mutation != 0 {
            assert!(
                result.is_err(),
                "accepted earlier destinations cannot rebind a changed incarnation"
            );
            assert_eq!(engine.diagnostic_snapshot().unwrap(), before);
            assert!(events.is_empty());
        } else {
            assert!(result.unwrap());
            engine.publish_resolution_prompt_ids(&mut events).unwrap();
            let pending = engine.state.pending_resolution.as_ref().unwrap();
            assert_ne!(pending.resolution_prompt_id.get(), old_prompt);
            assert_eq!(pending.deciding_player, 41);
            let ResolutionContinuation::DiscardReplacement { stack } = &pending.continuation else {
                unreachable!();
            };
            assert_eq!(stack.resume_effect_index(), Some(1));
            assert_eq!(
                serde_json::to_value(stack.previous_result()).unwrap(),
                previous
            );
            assert_eq!(active(&engine).len(), 3);
            answer(&mut engine, 41, vec![0]);
            assert_eq!(engine.state.players[1].life, 21);
        }
    }
}

#[test]
fn opponent_discard_round_saved_observer_reveals_retire_departed_owner() {
    let (mut engine, _, _) = replacement_fixture();
    let saved = engine.state.pending_resolution.take();
    let replacement = engine.state.pending_replacement_event.take();
    engine.state.pending_observer_return_batch = Some(crate::state::PendingObserverReturnBatch {
        ready: vec![],
        remaining: std::collections::VecDeque::new(),
        resume_stack: None,
        suspended_resolution: saved,
        suspended_replacement_event: replacement,
    });
    assert_eq!(active(&engine).len(), 3);
    // Synthetic extraction seam while an observer owns the paired suspended state. Actual
    // selected-owner concession and continuation are covered by the replacement test above.
    engine.state.players[1].has_lost = true;
    assert_eq!(
        active(&engine)
            .iter()
            .map(|r| r.zone_owner_player_id)
            .collect::<Vec<_>>(),
        [41, 53]
    );
}

#[test]
fn opponent_discard_round_rejects_changed_prefix_before_any_next_private_choice() {
    let (mut engine, source) = fixture_with(ONE_ROUND);
    begin(&mut engine, source);
    let paid = engine.state.players[1].hand[0];
    answer(&mut engine, 29, vec![paid]);
    *engine.state.zone_change_generation.entry(paid).or_default() += 1;
    let before = engine.diagnostic_snapshot().unwrap();
    let prompt = engine
        .state
        .pending_resolution
        .as_ref()
        .unwrap()
        .resolution_prompt_id
        .get();
    assert!(engine
        .apply_command(
            41,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(
                    rv1::SubmitResolutionChoice {
                        resolution_prompt_id: prompt,
                        ..Default::default()
                    }
                ))
            }
        )
        .is_err());
    assert_eq!(engine.diagnostic_snapshot().unwrap(), before);
}

#[test]
fn opponent_discard_round_all_empty_all_decline_and_seven_distinct_paid_rounds_complete_once() {
    for hand_size in [0, 7] {
        for pay in [false, true] {
            let (mut engine, source) = fixture();
            for actor in [29, 41, 53] {
                keep_hand_size(&mut engine, actor, hand_size);
            }
            let mut receipts = begin(&mut engine, source).events;
            while let Some(pending) = &engine.state.pending_resolution {
                let actor = pending.deciding_player;
                let chosen = if pay {
                    vec![engine.state.players[engine.state.player_idx(actor).unwrap()].hand[0]]
                } else {
                    vec![]
                };
                receipts.extend(answer(&mut engine, actor, chosen).events);
            }
            let expected = if hand_size > 0 && pay { 21 } else { 0 };
            assert_eq!(
                engine
                    .state
                    .players
                    .iter()
                    .skip(1)
                    .map(|p| p.life)
                    .collect::<Vec<_>>(),
                [expected; 3]
            );
            let ids = receipts
                .iter()
                .filter_map(|e| match &e.ev {
                    Some(rv1::ruled_event::Ev::CardsRevealed(r)) => Some(r.reveal_id.clone()),
                    _ => None,
                })
                .collect::<Vec<_>>();
            if hand_size > 0 && pay {
                assert_eq!(ids.len(), 21);
                assert_eq!(
                    ids.iter().collect::<std::collections::HashSet<_>>().len(),
                    21
                );
            } else {
                assert!(ids.is_empty());
            }
            assert!(engine.state.stack.is_empty());
        }
    }
}

#[test]
fn opponent_discard_round_seven_leng_payments_reveal_and_replace_each_distinct_card() {
    let (mut engine, source) = fixture();
    place_ready(&mut engine, 29, "Library of Leng");
    let original = engine.state.players[1].hand.clone();
    assert_eq!(original.len(), 7);
    begin(&mut engine, source);
    let mut receipt_ids = std::collections::HashSet::new();
    for selected in &original {
        answer(&mut engine, 29, vec![*selected]);
        for actor in [41, 53] {
            let card = engine.state.players[engine.state.player_idx(actor).unwrap()].hand[0];
            answer(&mut engine, actor, vec![card]);
        }
        assert_eq!(active(&engine).len(), 3);
        assert!(active(&engine)
            .iter()
            .all(|receipt| receipt_ids.insert(receipt.reveal_id.clone())));
        answer(&mut engine, 29, vec![1]);
        assert_eq!(engine.state.players[1].library.front(), Some(selected));
        assert!(active(&engine).is_empty());
    }
    assert_eq!(receipt_ids.len(), 21);
    assert_eq!(
        engine
            .state
            .players
            .iter()
            .skip(1)
            .map(|p| p.life)
            .collect::<Vec<_>>(),
        [21; 3]
    );
    assert!(original
        .iter()
        .all(|card| engine.state.objects[card].zone == Zone::Library));
}

#[test]
fn opponent_discard_round_payment_choice_is_independent_of_discard_receipts() {
    // Closed settlement seam with no discard action or receipts: CR118.11/118.12 keeps a legal
    // choice to pay distinct from whether a replacement ultimately produces a discard event.
    let (mut engine, _) = fixture_with(ONE_ROUND);
    let round = crate::state::PendingOpponentDiscardRound {
        stack_id: 0,
        leaf_index: 0,
        controller: 17,
        amount: 3,
        occurrence: "synthetic-settlement".into(),
        public_reveals: vec![],
        outcomes: vec![
            (29, crate::state::OpponentDiscardPayment::LegallyChosenToPay),
            (41, crate::state::OpponentDiscardPayment::Declined),
            (53, crate::state::OpponentDiscardPayment::Unpayable),
        ],
    };
    let mut events = Vec::new();
    engine
        .settle_opponent_discard_round(&round, "synthetic settlement", &mut events)
        .unwrap();
    assert_eq!(
        engine
            .state
            .players
            .iter()
            .map(|p| p.life)
            .collect::<Vec<_>>(),
        [20, 20, 17, 17]
    );
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(e.ev, Some(rv1::ruled_event::Ev::LifeChanged(_))))
            .count(),
        2
    );
}

#[test]
fn opponent_discard_round_private_rejections_and_saved_choices_are_atomic() {
    for invalid in 0..7 {
        let (mut engine, source) = fixture_with(ONE_ROUND);
        begin(&mut engine, source);
        let offered = engine.state.players[1].hand[0];
        let prompt = engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .resolution_prompt_id
            .get();
        let (actor, ids, id) = match invalid {
            0 => (41, vec![offered], prompt),
            1 => (29, vec![offered], prompt + 999),
            2 => (29, vec![offered, offered], prompt),
            3 => (29, vec![engine.state.players[2].hand[0]], prompt),
            4 => (29, vec![source], prompt),
            5 => (29, vec![u32::MAX], prompt),
            _ => {
                *engine
                    .state
                    .zone_change_generation
                    .entry(offered)
                    .or_default() += 1;
                (29, vec![offered], prompt)
            }
        };
        let before = engine.diagnostic_snapshot().unwrap();
        assert!(engine
            .apply_command(
                actor,
                &RuledCommand {
                    cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(
                        rv1::SubmitResolutionChoice {
                            resolution_prompt_id: id,
                            chosen_object_ids: ids,
                            ..Default::default()
                        }
                    ))
                }
            )
            .is_err());
        assert_eq!(engine.diagnostic_snapshot().unwrap(), before);
        assert!(active(&engine).is_empty());
    }
    for stale in [false, true] {
        let (mut engine, source) = fixture_with(ONE_ROUND);
        begin(&mut engine, source);
        let paid = engine.state.players[1].hand[0];
        answer(&mut engine, 29, vec![paid]);
        let saved = engine.state.pending_resolution.take().unwrap();
        if stale {
            *engine.state.zone_change_generation.entry(paid).or_default() += 1;
        }
        let before = engine.diagnostic_snapshot().unwrap();
        let mut events = Vec::new();
        let result = engine.restore_suspended_observer_resolution(Some(saved), None, &mut events);
        if stale {
            assert!(result.is_err());
            assert_eq!(engine.diagnostic_snapshot().unwrap(), before);
            assert!(events.is_empty());
        } else {
            assert!(result.unwrap());
            engine.publish_resolution_prompt_ids(&mut events).unwrap();
            assert_eq!(
                engine
                    .state
                    .pending_resolution
                    .as_ref()
                    .unwrap()
                    .deciding_player,
                41
            );
            answer(&mut engine, 41, vec![]);
            answer(&mut engine, 53, vec![]);
            assert_eq!(engine.state.objects[&paid].zone, Zone::Graveyard);
        }
    }
}

#[test]
fn opponent_discard_round_saved_accepted_prefix_ignores_unchosen_hand_drift() {
    for paid in [false, true] {
        let (mut engine, source) = fixture_with(ONE_ROUND);
        begin(&mut engine, source);
        let selected = engine.state.players[1].hand[0];
        let unchosen = engine.state.players[1].hand[1];
        answer(&mut engine, 29, if paid { vec![selected] } else { vec![] });
        let saved = engine.state.pending_resolution.take().unwrap();
        *engine
            .state
            .zone_change_generation
            .entry(unchosen)
            .or_default() += 1;
        let mut events = Vec::new();
        assert!(engine
            .restore_suspended_observer_resolution(Some(saved), None, &mut events)
            .unwrap());
        engine.publish_resolution_prompt_ids(&mut events).unwrap();
        answer(&mut engine, 41, vec![]);
        answer(&mut engine, 53, vec![]);
        assert_eq!(engine.state.players[1].life, if paid { 21 } else { 18 });
        assert_eq!(
            engine.state.objects[&selected].zone,
            if paid { Zone::Graveyard } else { Zone::Hand }
        );
    }
}

#[test]
fn opponent_discard_round_saved_halves_and_captured_leaf_must_match() {
    for mutation in 0..5 {
        let (mut engine, source) = fixture_with(ONE_ROUND);
        begin(&mut engine, source);
        let mut saved = engine.state.pending_resolution.take().unwrap();
        let replacement = if mutation == 0 {
            let (mut other, _, _) = replacement_fixture();
            other.state.pending_replacement_event.take()
        } else {
            None
        };
        let ResolutionContinuation::PlayerSetDiscard { stack, discard } = &mut saved.continuation
        else {
            unreachable!();
        };
        match mutation {
            1 => stack.item.id += 900,
            2 => {
                stack.item.activated_ability.as_mut().unwrap().effect[0] = SpellEffectKind::Draw {
                    count: 1.into(),
                    who: PlayerRecipient::Controller,
                }
            }
            3 => {
                let crate::state::DiscardCompletion::OpponentRound(round) = &mut discard.completion
                else {
                    unreachable!();
                };
                round.amount += 1;
            }
            4 => {
                *stack = ParkedStackResolution::for_flat_effects(
                    stack.item.clone(),
                    Some(2),
                    stack.previous_result().clone(),
                )
            }
            _ => {}
        }
        let before = engine.diagnostic_snapshot().unwrap();
        let mut events = Vec::new();
        assert!(engine
            .restore_suspended_observer_resolution(Some(saved), replacement, &mut events)
            .is_err());
        assert_eq!(engine.diagnostic_snapshot().unwrap(), before);
        assert!(events.is_empty());
    }
}

#[test]
fn opponent_discard_round_saved_replacement_cannot_lose_its_completion() {
    for missing_batch in [false, true] {
        let (mut engine, _, _) = replacement_fixture();
        let saved = engine.state.pending_resolution.take();
        let mut replacement = engine.state.pending_replacement_event.take();
        if missing_batch {
            replacement = None;
        } else {
            let Some(super::super::replacement::PendingReplacementEvent::Discard(batch)) =
                &mut replacement
            else {
                unreachable!();
            };
            batch.completion = crate::state::DiscardCompletion::Ordinary;
        }
        let before = engine.diagnostic_snapshot().unwrap();
        let mut events = Vec::new();
        assert!(engine
            .restore_suspended_observer_resolution(saved, replacement, &mut events)
            .is_err());
        assert_eq!(engine.diagnostic_snapshot().unwrap(), before);
        assert!(events.is_empty());
    }
}
