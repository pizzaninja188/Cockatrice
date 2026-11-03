use super::*;

const SYNTHETIC_POISON_DIFFERENCE: &str = r#"(
    id: "synthetic_poison_difference", name: "Synthetic Poison Difference",
    face_id: "synthetic_poison_difference", types: ["Artifact"],
    activated_abilities: [(ability_id: "poison", presentation: Fallback,
        costs: [Mana("{1}")],
        effect: [PoisonTargetUpToNine(target: (kind: AnyPlayer)), GainLife(amount: 1)],
        targeting: Some((groups: [(min: 1, max: 1, prompt: "Choose player", effect_indices: [0])])),
    )],
)"#;

fn poison_difference_fixture(count: u32, target: PlayerId) -> (GameEngine, ObjectId) {
    let registry = CardRegistry::from_chunks_and_tokens(
        &[
            include_str!("../../../../tricerules-cards/data/forest.ron"),
            include_str!("../../../../tricerules-cards/data/vorinclex,_monstrous_raider.ron"),
            include_str!("../../../../tricerules-cards/data/laezel,_vlaakiths_champion.ron"),
            SYNTHETIC_POISON_DIFFERENCE,
        ],
        &[],
    )
    .unwrap();
    let mut engine = GameEngine::new_with_registry(
        602_450,
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
    let source = place_ready(&mut engine, 17, "Synthetic Poison Difference");
    let index = engine.state.player_idx(target).unwrap();
    engine.state.players[index]
        .counters
        .insert(CounterKind::Poison, count);
    engine.state.players[0].mana_pool.colorless = 1;
    (engine, source)
}

fn activate_poison_difference(engine: &mut GameEngine, source: ObjectId, target: PlayerId) {
    let source_ref = engine.payment_object_ref(source);
    let command = rv1::ActivateAbility {
        source_object_id: source,
        ability_index: 0,
        expected_zone_change_generation: source_ref.zone_change_generation,
        targets: vec![rv1::TargetRef {
            kind: rv1::TargetRefKind::Player as i32,
            object_id: target as u32,
            ..Default::default()
        }],
        payment: Some(rv1::PaymentSelection {
            expected_state_revision: engine.state.command_index,
            source: Some(source_ref),
            mana: Some(rv1::PaymentMana {
                c: 1,
                ..Default::default()
            }),
            ..Default::default()
        }),
        ..Default::default()
    };
    engine
        .apply_command(
            17,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::ActivateAbility(command)),
            },
        )
        .expect("activate typed poison-difference fixture");
}

fn pass_poison_round(engine: &mut GameEngine) -> rv1::RuledEventBatch {
    let mut last = None;
    for player in [17, 29, 41] {
        last = Some(
            engine
                .apply_command(
                    player,
                    &RuledCommand {
                        cmd: Some(rv1::ruled_command::Cmd::PassPriority(rv1::PassPriority {})),
                    },
                )
                .expect("resolve the poison instruction using ordinary priority"),
        );
    }
    last.unwrap()
}

#[test]
fn poison_difference_adds_only_to_the_target_and_finishes_the_flat_tail() {
    for target in [17, 29, 41] {
        for before in [0, 8, 9] {
            let (mut engine, source) = poison_difference_fixture(before, target);
            activate_poison_difference(&mut engine, source, target);
            pass_poison_round(&mut engine);
            let index = engine.state.player_idx(target).unwrap();
            assert_eq!(
                engine.state.players[index].counters[&CounterKind::Poison],
                9
            );
            for player in &engine.state.players {
                if player.id != target {
                    assert_eq!(
                        player
                            .counters
                            .get(&CounterKind::Poison)
                            .copied()
                            .unwrap_or(0),
                        0
                    );
                }
            }
            assert_eq!(
                engine.state.players[0].life, 21,
                "the following instruction runs once"
            );
            assert!(engine.state.pending_resolution.is_none());
            assert!(engine.state.stack.is_empty());
        }
    }
}
#[test]
fn poison_difference_reads_the_live_count_and_applies_supported_replacements() {
    for (before, replacement_owner, expected) in [
        (0, Some(17), 18),
        (8, Some(17), 10),
        (0, Some(29), 4),
        (8, Some(29), 8),
        (0, None, 9),
    ] {
        let (mut engine, source) = poison_difference_fixture(before, 41);
        if let Some(owner) = replacement_owner {
            place_ready(&mut engine, owner, "Vorinclex, Monstrous Raider");
        }
        activate_poison_difference(&mut engine, source, 41);
        if replacement_owner.is_none() {
            engine.state.players[2]
                .counters
                .insert(CounterKind::Poison, 8);
        }
        pass_poison_round(&mut engine);
        assert_eq!(
            engine.state.players[2].counters[&CounterKind::Poison],
            expected
        );
        assert_eq!(engine.state.players[2].has_lost, expected >= 10);
        assert_eq!(engine.state.players[0].life, 21);
    }
    for (target, expected) in [(17, 10), (41, 9)] {
        let (mut engine, source) = poison_difference_fixture(8, target);
        place_ready(&mut engine, 17, "Lae'zel, Vlaakith's Champion");
        activate_poison_difference(&mut engine, source, target);
        pass_poison_round(&mut engine);
        let index = engine.state.player_idx(target).unwrap();
        assert_eq!(
            engine.state.players[index].counters[&CounterKind::Poison],
            expected
        );
        assert_eq!(engine.state.players[index].has_lost, expected >= 10);
    }
}

#[test]
fn poison_difference_completion_rejects_changed_identity_receipts_and_proposals_atomically() {
    use super::super::replacement::CounterPlacementRecipient;
    use crate::state::{CounterPlacementCompletion, ParkedStackResolution};
    let (mut engine, source) = poison_difference_fixture(8, 41);
    activate_poison_difference(&mut engine, source, 41);
    let item = engine.state.stack.last().unwrap().clone();
    let frame = ParkedStackResolution::for_flat_effects(item.clone(), Some(1), Default::default());
    let completion = CounterPlacementCompletion::PoisonTargetUpToNine {
        stack_id: item.id,
        leaf_index: 0,
        controller: 17,
        target_player: 41,
        poison_before: 8,
    };
    let proposal = engine
        .proposed_player_counter_placement(
            17,
            41,
            [(CounterKind::Poison, 1)],
            super::super::continuous::CounterPlacementOrigin::Effect,
        )
        .unwrap();
    let reject = |engine: &mut GameEngine,
                  event: &super::super::replacement::ProposedCounterPlacement,
                  completion: &CounterPlacementCompletion,
                  frame: Option<&ParkedStackResolution>| {
        let before = engine.diagnostic_snapshot().unwrap();
        let mut out = Vec::new();
        assert!(engine
            .commit_counter_placement_batch(
                std::slice::from_ref(event),
                completion,
                frame,
                &mut out
            )
            .is_err());
        assert!(out.is_empty());
        assert_eq!(engine.diagnostic_snapshot().unwrap(), before);
    };
    reject(&mut engine, &proposal, &completion, None);
    for mutation in 0..5 {
        let mut altered = completion.clone();
        let CounterPlacementCompletion::PoisonTargetUpToNine {
            stack_id,
            leaf_index,
            controller,
            target_player,
            poison_before,
        } = &mut altered
        else {
            unreachable!()
        };
        match mutation {
            0 => *stack_id += 1,
            1 => *leaf_index += 1,
            2 => *controller = 29,
            3 => *target_player = 29,
            _ => *poison_before = 7,
        }
        reject(&mut engine, &proposal, &altered, Some(&frame));
    }
    for mutation in 0..5 {
        let mut altered = frame.clone();
        match mutation {
            0 => altered.item.targets.clear(),
            1 => altered.item.targets[0].kind = rv1::TargetRefKind::Permanent as i32,
            2 => altered.item.targets[0].group_index = 1,
            3 => altered.item.targets[0].zone_change_generation = Some(0),
            _ => altered.set_flat_tail(Some(0), Default::default()),
        }
        reject(&mut engine, &proposal, &completion, Some(&altered));
    }
    for mutation in 0..5 {
        let mut altered = proposal.clone();
        match mutation {
            0 => altered.actor = 29,
            1 => {
                altered.original_counters.insert(CounterKind::Poison, 2);
            }
            2 => {
                altered.counters.insert(CounterKind::Charge, 1);
            }
            3 => {
                altered.counters.insert(CounterKind::Poison, 0);
            }
            _ => {
                altered.recipient = CounterPlacementRecipient::Object {
                    object_id: source,
                    zone_change_generation: 1,
                }
            }
        }
        reject(&mut engine, &altered, &completion, Some(&frame));
    }
    let mut out = Vec::new();
    assert!(engine
        .commit_counter_placement_batch(&[proposal], &completion, Some(&frame), &mut out)
        .unwrap()
        .is_empty());
    assert_eq!(engine.state.players[2].counters[&CounterKind::Poison], 9);
}

fn parked_poison_difference() -> (GameEngine, u32, u32, rv1::RuledEventBatch) {
    let (mut engine, source) = poison_difference_fixture(8, 41);
    let double = place_ready(&mut engine, 17, "Vorinclex, Monstrous Raider");
    let halve = place_ready(&mut engine, 29, "Vorinclex, Monstrous Raider");
    activate_poison_difference(&mut engine, source, 41);
    let batch = pass_poison_round(&mut engine);
    assert!(engine.state.pending_resolution.is_some());
    assert_eq!(engine.state.players[2].counters[&CounterKind::Poison], 8);
    (engine, double, halve, batch)
}

fn poison_replacement_answer(batch: &rv1::RuledEventBatch, source: u32) -> RuledCommand {
    let choice = batch
        .events
        .iter()
        .find_map(|event| match &event.ev {
            Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(choice)) => Some(choice),
            _ => None,
        })
        .unwrap();
    assert_eq!(choice.deciding_player_id, 41);
    let application = choice
        .replacement_options
        .iter()
        .find(|option| option.source_object_id == source)
        .unwrap()
        .application_id;
    RuledCommand {
        cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(
            rv1::SubmitResolutionChoice {
                chosen_object_ids: vec![application],
                resolution_prompt_id: choice.resolution_prompt_id,
                ..Default::default()
            },
        )),
    }
}

#[test]
fn poison_difference_parked_order_and_rejected_answers_preserve_exactly_once_tail() {
    for halve_first in [false, true] {
        let (mut engine, double, halve, batch) = parked_poison_difference();
        let command = poison_replacement_answer(&batch, if halve_first { halve } else { double });
        let before = engine.diagnostic_snapshot().unwrap();
        assert!(engine.apply_command(17, &command).is_err());
        assert_eq!(engine.diagnostic_snapshot().unwrap(), before);
        let mut stale = command.clone();
        let Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(answer)) = stale.cmd.as_mut()
        else {
            unreachable!()
        };
        answer.resolution_prompt_id += 1;
        assert!(engine.apply_command(41, &stale).is_err());
        assert_eq!(engine.diagnostic_snapshot().unwrap(), before);
        engine.apply_command(41, &command).unwrap();
        assert_eq!(
            engine.state.players[2].counters[&CounterKind::Poison],
            if halve_first { 8 } else { 9 }
        );
        assert_eq!(engine.state.players[0].life, 21);
        assert!(engine.state.pending_resolution.is_none());
        let after = engine.diagnostic_snapshot().unwrap();
        assert!(engine.apply_command(41, &command).is_err());
        assert_eq!(engine.diagnostic_snapshot().unwrap(), after);
    }
}

#[test]
fn poison_difference_departure_prunes_only_the_lost_target_and_keeps_the_tail() {
    let (mut engine, _, _, _) = parked_poison_difference();
    engine
        .apply_command(
            41,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::Concede(rv1::Concede {})),
            },
        )
        .unwrap();
    assert!(engine.state.players[2].has_lost);
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.pending_replacement_event.is_none());
    assert_eq!(engine.state.players[0].life, 21);
    assert_eq!(
        engine.state.players[1]
            .counters
            .get(&CounterKind::Poison)
            .copied()
            .unwrap_or(0),
        0
    );
}

#[test]
fn poison_difference_retains_an_applied_replacement_after_its_source_and_actor_leave() {
    let (mut engine, source) = poison_difference_fixture(6, 41);
    let double = place_ready(&mut engine, 17, "Vorinclex, Monstrous Raider");
    let halve = place_ready(&mut engine, 29, "Vorinclex, Monstrous Raider");
    place_ready(&mut engine, 41, "Vorinclex, Monstrous Raider");
    activate_poison_difference(&mut engine, source, 41);
    let batch = pass_poison_round(&mut engine);
    let answer = poison_replacement_answer(&batch, double);
    let second_offer = engine.apply_command(41, &answer).unwrap();
    assert!(
        engine.state.pending_resolution.is_some(),
        "two remaining halvers require another order choice"
    );
    let answer = poison_replacement_answer(&second_offer, halve);
    let prompt_id = engine
        .state
        .pending_resolution
        .as_ref()
        .unwrap()
        .resolution_prompt_id;
    let departure = engine
        .apply_command(
            17,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::Concede(rv1::Concede {})),
            },
        )
        .unwrap();
    assert!(engine.state.players[0].has_lost);
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .resolution_prompt_id,
        prompt_id,
        "unchanged replacement answer semantics retain the already-published prompt"
    );
    assert!(!departure.events.iter().any(|event| matches!(
        event.ev,
        Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(_))
    )));
    engine.apply_command(41, &answer).unwrap();
    assert_eq!(
        engine.state.players[2].counters[&CounterKind::Poison],
        7,
        "the saved three-counter proposal was doubled, then halved twice: six plus one"
    );
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.pending_replacement_event.is_none());
    assert!(engine.state.stack.is_empty());
}
