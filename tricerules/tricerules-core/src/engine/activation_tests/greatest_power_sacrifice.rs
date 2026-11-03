use super::*;

// Onyx's -3 sentence plus a synthetic opponent-life sentinel. No complete card admission.
const SOURCE: &str = r#"(
    id: "synthetic_greatest_sacrifice", name: "Synthetic Greatest Sacrifice",
    face_id: "synthetic_greatest_sacrifice", types: ["Artifact"],
    activated_abilities: [(ability_id: "sacrifice", presentation: Fallback,
        costs: [Mana("{1}")], effect: [ChooseAndSacrifice(players: EachOpponent,
            count: 1, filter: (kind: Creature), restriction: GreatestPower),
            GainLife(amount: 1, who: EachOpponent)],
    )],
)"#;
const SLIVER_TWO: &str = r#"(id: "sliver_two", name: "Sliver Two", face_id: "sliver_two",
    types: ["Creature", "Sliver"], power: 2, toughness: 6)"#;
const SLIVER_ONE: &str = r#"(id: "sliver_one", name: "Sliver One", face_id: "sliver_one",
    types: ["Creature", "Sliver"], power: 1, toughness: 6)"#;
const ELF_THREE: &str = r#"(id: "elf_three", name: "Elf Three", face_id: "elf_three",
    types: ["Creature", "Elf"], power: 3, toughness: 6)"#;
const NEGATIVE_TWO: &str = r#"(id: "negative_two", name: "Negative Two", face_id: "negative_two",
    types: ["Creature", "Zombie"], power: 2, toughness: 6)"#;
const NEGATIVE_FOUR: &str = r#"(id: "negative_four", name: "Negative Four", face_id: "negative_four",
    types: ["Creature", "Zombie"], power: 0, toughness: 6)"#;
const SHIELDED_FIVE: &str = r#"(id: "shielded_five", name: "Shielded Five", face_id: "shielded_five",
    types: ["Creature"], power: 5, toughness: 6, keywords: [Hexproof, Indestructible])"#;
const ANTHEM: &str = r#"(id: "sliver_anthem", name: "Sliver Anthem", face_id: "sliver_anthem",
    types: ["Artifact"], static_abilities: [(ability_id: "anthem", presentation: Fallback,
        definition: AnthemPt(filter: (subtype: Some("Sliver")), delta_power: 3, delta_toughness: 0))])"#;
const DEBUFF: &str = r#"(id: "zombie_debuff", name: "Zombie Debuff", face_id: "zombie_debuff",
    types: ["Artifact"], static_abilities: [(ability_id: "debuff", presentation: Fallback,
        definition: AnthemPt(filter: (subtype: Some("Zombie")), delta_power: -4, delta_toughness: 0))])"#;

fn fixture() -> (GameEngine, ObjectId) {
    let registry = CardRegistry::from_chunks_and_tokens(
        &[
            include_str!("../../../../tricerules-cards/data/forest.ron"),
            SOURCE,
            SLIVER_TWO,
            SLIVER_ONE,
            ELF_THREE,
            NEGATIVE_TWO,
            NEGATIVE_FOUR,
            SHIELDED_FIVE,
            ANTHEM,
            DEBUFF,
        ],
        &[],
    )
    .unwrap();
    let mut engine = GameEngine::new_with_registry(
        602_453,
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
    let source = place_ready(&mut engine, 17, "Synthetic Greatest Sacrifice");
    (engine, source)
}

fn begin(engine: &mut GameEngine, source: ObjectId) -> rv1::RuledEventBatch {
    engine.state.players[0].mana_pool.colorless = 1;
    let source_ref = engine.payment_object_ref(source);
    engine
        .apply_command(
            17,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::ActivateAbility(
                    rv1::ActivateAbility {
                        source_object_id: source,
                        ability_index: 0,
                        expected_zone_change_generation: source_ref.zone_change_generation,
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
                    },
                )),
            },
        )
        .unwrap();
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
                .unwrap(),
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
fn greatest_power_sacrifice_offers_all_tied_maxima_then_commits_simultaneously() {
    let (mut engine, source) = fixture();
    let first = place_ready(&mut engine, 29, "Sliver Two");
    let second = place_ready(&mut engine, 29, "Sliver Two");
    let lower = place_ready(&mut engine, 29, "Sliver One");
    let forced = place_ready(&mut engine, 41, "Elf Three");
    let other = place_ready(&mut engine, 41, "Sliver One");
    begin(&mut engine, source);
    let pending = engine.state.pending_resolution.as_ref().unwrap();
    assert_eq!(pending.deciding_player, 29);
    assert_eq!(
        pending.presentation.candidates,
        [first, second],
        "only tied greatest power is eligible"
    );
    for id in [first, second, lower, forced, other] {
        assert_eq!(engine.state.objects[&id].zone, Zone::Battlefield);
    }
    let batch = answer(&mut engine, 29, vec![second]);
    assert_eq!(engine.state.objects[&second].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&forced].zone, Zone::Graveyard);
    for id in [first, lower, other] {
        assert_eq!(engine.state.objects[&id].zone, Zone::Battlefield);
    }
    assert_eq!(batch.events.iter().filter(|event| matches!(&event.ev,
        Some(rv1::ruled_event::Ev::PermanentMoved(moved)) if [second,forced].contains(&moved.object_id))).count(), 2);
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
    assert_eq!(
        engine
            .state
            .players
            .iter()
            .map(|p| p.life)
            .collect::<Vec<_>>(),
        [20, 21, 21, 21]
    );
}

#[test]
fn greatest_power_sacrifice_handles_empty_negative_power_counters_and_tokens() {
    let (mut empty, source) = fixture();
    begin(&mut empty, source);
    assert!(empty.state.pending_resolution.is_none());
    assert!(empty.state.stack.is_empty());
    assert_eq!(
        empty
            .state
            .players
            .iter()
            .map(|p| p.life)
            .collect::<Vec<_>>(),
        [20, 21, 21, 21]
    );
    for token in [false, true] {
        let (mut engine, source) = fixture();
        place_ready(&mut engine, 17, "Zombie Debuff");
        let negative_max = place_ready(&mut engine, 29, "Negative Two");
        let negative_lower = place_ready(&mut engine, 29, "Negative Four");
        assert_eq!(
            engine.characteristics(negative_max).unwrap().signed_power,
            Some(-2)
        );
        assert_eq!(
            engine.characteristics(negative_lower).unwrap().signed_power,
            Some(-4)
        );
        let shielded = place_ready(&mut engine, 41, "Shielded Five");
        let lower = place_ready(&mut engine, 41, "Elf Three");
        if token {
            let values = engine.copiable_values_for(shielded).unwrap();
            engine
                .state
                .objects
                .get_mut(&shielded)
                .unwrap()
                .token_origin = Some(values);
        }
        let countered = place_ready(&mut engine, 53, "Sliver One");
        let other = place_ready(&mut engine, 53, "Elf Three");
        engine
            .state
            .objects
            .get_mut(&countered)
            .unwrap()
            .set_counter(CounterKind::PlusOnePlusOne, 3);
        begin(&mut engine, source);
        assert!(engine.state.pending_resolution.is_none());
        for id in [negative_max, countered] {
            assert_eq!(engine.state.objects[&id].zone, Zone::Graveyard);
        }
        for id in [negative_lower, lower, other] {
            assert_eq!(engine.state.objects[&id].zone, Zone::Battlefield);
        }
        if token {
            assert!(!engine.state.objects.contains_key(&shielded));
        } else {
            assert_eq!(engine.state.objects[&shielded].zone, Zone::Graveyard);
        }
        assert_eq!(
            engine
                .state
                .players
                .iter()
                .map(|p| p.life)
                .collect::<Vec<_>>(),
            [20, 21, 21, 21]
        );
    }
}

fn grant_control(engine: &mut GameEngine, permanent: ObjectId, controller: PlayerId) {
    engine.state.continuous_effects.push(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: None,
        affected: AffectedScope::Single(permanent),
        kind: ContinuousEffectKind::Layer2Control {
            controller: ControllerReference::Fixed(controller),
        },
        condition: None,
        duration: EffectDuration::Indefinite,
        timestamp: engine.state.command_index + 1,
    });
    engine.reindex_battlefield_control(&mut Vec::new()).unwrap();
}

#[test]
fn greatest_power_sacrifice_rejects_wrong_stale_and_no_longer_maximal_choices_atomically() {
    for mutation in 0..8 {
        let (mut engine, source) = fixture();
        let first = place_ready(&mut engine, 29, "Sliver Two");
        place_ready(&mut engine, 29, "Sliver Two");
        let lower = place_ready(&mut engine, 29, "Sliver One");
        begin(&mut engine, source);
        let mut actor = 29;
        let mut selection = rv1::SubmitResolutionChoice {
            resolution_prompt_id: engine
                .state
                .pending_resolution
                .as_ref()
                .unwrap()
                .resolution_prompt_id
                .get(),
            chosen_object_ids: vec![first],
            ..Default::default()
        };
        match mutation {
            0 => actor = 41,
            1 => selection.resolution_prompt_id += 1,
            2 => selection.chosen_object_ids.clear(),
            3 => selection.chosen_object_ids = vec![first, first],
            4 => selection.chosen_object_ids = vec![lower],
            5 => {
                *engine
                    .state
                    .zone_change_generation
                    .entry(first)
                    .or_default() += 1
            }
            6 => grant_control(&mut engine, first, 41),
            _ => engine
                .state
                .objects
                .get_mut(&lower)
                .unwrap()
                .set_counter(CounterKind::PlusOnePlusOne, 4),
        }
        let before = engine.diagnostic_snapshot().unwrap();
        assert!(
            engine
                .apply_command(
                    actor,
                    &RuledCommand {
                        cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(selection)),
                    }
                )
                .is_err(),
            "mutation={mutation}"
        );
        assert_eq!(
            engine.diagnostic_snapshot().unwrap(),
            before,
            "mutation={mutation}"
        );
    }
}

#[test]
fn greatest_power_sacrifice_keeps_accepted_rank_drift_and_refreshes_unanswered_choices() {
    let (mut engine, source) = fixture();
    let anthem = place_ready(&mut engine, 53, "Sliver Anthem");
    let accepted = place_ready(&mut engine, 29, "Sliver Two");
    place_ready(&mut engine, 29, "Sliver Two");
    let now_greater = place_ready(&mut engine, 29, "Elf Three");
    let old_first = place_ready(&mut engine, 41, "Sliver One");
    place_ready(&mut engine, 41, "Sliver One");
    let fresh_first = place_ready(&mut engine, 41, "Elf Three");
    let fresh_second = place_ready(&mut engine, 41, "Elf Three");
    begin(&mut engine, source);
    answer(&mut engine, 29, vec![accepted]);
    let old_prompt = engine
        .state
        .pending_resolution
        .as_ref()
        .unwrap()
        .resolution_prompt_id
        .get();
    engine
        .apply_command(
            53,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::Concede(rv1::Concede {})),
            },
        )
        .unwrap();
    assert!(!engine.state.objects.contains_key(&anthem));
    assert_eq!(
        engine.characteristics(accepted).unwrap().signed_power,
        Some(2)
    );
    assert_eq!(
        engine.characteristics(now_greater).unwrap().signed_power,
        Some(3)
    );
    let pending = engine.state.pending_resolution.as_ref().unwrap();
    assert_eq!(pending.deciding_player, 41);
    assert_eq!(pending.presentation.candidates, [fresh_first, fresh_second]);
    assert_ne!(pending.resolution_prompt_id.get(), old_prompt);
    let ResolutionContinuation::PlayerSetSacrifice { sacrifice, .. } = &pending.continuation else {
        unreachable!();
    };
    assert_eq!(sacrifice.accepted.len(), 1);
    assert_eq!(sacrifice.accepted[0].object_id, accepted);
    let before = engine.diagnostic_snapshot().unwrap();
    assert!(engine
        .apply_command(
            41,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(
                    rv1::SubmitResolutionChoice {
                        resolution_prompt_id: old_prompt,
                        chosen_object_ids: vec![old_first],
                        ..Default::default()
                    }
                )),
            }
        )
        .is_err());
    assert_eq!(engine.diagnostic_snapshot().unwrap(), before);
    answer(&mut engine, 41, vec![fresh_second]);
    assert_eq!(
        engine.state.objects[&accepted].zone,
        Zone::Graveyard,
        "accepted identity remains owed after its power rank changes"
    );
    assert_eq!(engine.state.objects[&now_greater].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&fresh_second].zone, Zone::Graveyard);
    assert_eq!(
        (engine.state.players[1].life, engine.state.players[2].life),
        (21, 21)
    );
}

#[test]
fn greatest_power_sacrifice_uses_owner_graveyard_order_and_resumes_tail_once() {
    let (mut engine, source) = fixture();
    let borrowed_a = place_ready(&mut engine, 41, "Sliver Two");
    let borrowed_b = place_ready(&mut engine, 41, "Sliver Two");
    let native_a = place_ready(&mut engine, 41, "Sliver Two");
    let native_b = place_ready(&mut engine, 41, "Sliver Two");
    grant_control(&mut engine, borrowed_a, 29);
    grant_control(&mut engine, borrowed_b, 29);
    begin(&mut engine, source);
    let public_progress = answer(&mut engine, 29, vec![borrowed_a]);
    assert!(public_progress.events.iter().any(|event| matches!(&event.ev,
        Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(choice))
            if choice.accepted_permanent_selections.iter().any(|selection| selection.object_id == borrowed_a && selection.selecting_player_id == 29))));
    assert_eq!(engine.state.objects[&borrowed_a].zone, Zone::Battlefield);
    answer(&mut engine, 41, vec![native_a]);
    let pending = engine.state.pending_resolution.as_ref().unwrap();
    assert_eq!(pending.deciding_player, 41);
    assert!(pending.presentation.ordered);
    assert_eq!(engine.state.objects[&borrowed_a].zone, Zone::Graveyard);
    assert_eq!(
        engine.state.players[1].life, 20,
        "the tail waits for owner graveyard ordering"
    );
    answer(&mut engine, 41, vec![native_a, borrowed_a]);
    assert_eq!(engine.state.players[2].graveyard, [native_a, borrowed_a]);
    for id in [borrowed_b, native_b] {
        assert_eq!(engine.state.objects[&id].zone, Zone::Battlefield);
    }
    assert_eq!(
        engine
            .state
            .players
            .iter()
            .map(|p| p.life)
            .collect::<Vec<_>>(),
        [20, 21, 21, 21]
    );
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
}

#[test]
fn greatest_power_sacrifice_restores_observer_cursor_result_and_accepted_prefix() {
    let (mut engine, source) = fixture();
    let accepted = place_ready(&mut engine, 29, "Sliver Two");
    place_ready(&mut engine, 29, "Sliver Two");
    let current = place_ready(&mut engine, 41, "Sliver Two");
    place_ready(&mut engine, 41, "Sliver Two");
    begin(&mut engine, source);
    answer(&mut engine, 29, vec![accepted]);
    // Synthetic observer seam. Preserve input facts while parked; the sacrifice operation's
    // actual card-result receipt replaces its input only when the operation completes.
    let mut saved = engine.state.pending_resolution.take().unwrap();
    let old_prompt = saved.resolution_prompt_id.get();
    let ResolutionContinuation::PlayerSetSacrifice { stack, .. } = &mut saved.continuation else {
        unreachable!();
    };
    *stack = stack
        .clone()
        .with_previous_result(OperationResults::from_value(ProgramValue::Count(37)));
    let previous = serde_json::to_value(stack.previous_result()).unwrap();
    let mut events = Vec::new();
    assert!(engine
        .restore_suspended_observer_resolution(Some(saved), None, &mut events)
        .unwrap());
    engine.publish_resolution_prompt_ids(&mut events).unwrap();
    let pending = engine.state.pending_resolution.as_ref().unwrap();
    assert_ne!(pending.resolution_prompt_id.get(), old_prompt);
    let ResolutionContinuation::PlayerSetSacrifice { stack, sacrifice } = &pending.continuation
    else {
        unreachable!();
    };
    assert_eq!(stack.resume_effect_index(), Some(1));
    assert_eq!(
        serde_json::to_value(stack.previous_result()).unwrap(),
        previous
    );
    assert_eq!(sacrifice.accepted[0].object_id, accepted);
    answer(&mut engine, 41, vec![current]);
    assert_eq!(
        engine
            .state
            .players
            .iter()
            .map(|p| p.life)
            .collect::<Vec<_>>(),
        [20, 21, 21, 21]
    );
    assert!(engine.state.stack.is_empty());
}

#[test]
fn greatest_power_sacrifice_survives_begun_controller_or_source_owner_departure() {
    for departing in [17, 53] {
        let (mut engine, source) = fixture();
        engine.state.objects.get_mut(&source).unwrap().owner = 53;
        let accepted = place_ready(&mut engine, 29, "Sliver Two");
        place_ready(&mut engine, 29, "Sliver Two");
        let current = place_ready(&mut engine, 41, "Sliver Two");
        place_ready(&mut engine, 41, "Sliver Two");
        begin(&mut engine, source);
        answer(&mut engine, 29, vec![accepted]);
        engine
            .apply_command(
                departing,
                &RuledCommand {
                    cmd: Some(rv1::ruled_command::Cmd::Concede(rv1::Concede {})),
                },
            )
            .unwrap();
        answer(&mut engine, 41, vec![current]);
        assert_eq!(engine.state.objects[&accepted].zone, Zone::Graveyard);
        assert_eq!(engine.state.objects[&current].zone, Zone::Graveyard);
        assert_eq!(
            (engine.state.players[1].life, engine.state.players[2].life),
            (21, 21)
        );
        assert!(engine.state.pending_resolution.is_none());
        assert!(engine.state.stack.is_empty());
    }
}

#[test]
fn greatest_power_sacrifice_retires_changed_accepted_identity_without_rebinding_it() {
    for mutation in 0..4 {
        let (mut engine, source) = fixture();
        let accepted = place_ready(&mut engine, 29, "Sliver Two");
        let unchosen = place_ready(&mut engine, 29, "Sliver Two");
        let current = place_ready(&mut engine, 41, "Sliver Two");
        place_ready(&mut engine, 41, "Sliver Two");
        begin(&mut engine, source);
        answer(&mut engine, 29, vec![accepted]);
        // Synthetic observer seam: a saved public prefix can lose action possibility while a
        // nested operation runs. Its replacement incarnation is never silently rebound.
        let saved = engine.state.pending_resolution.take().unwrap();
        match mutation {
            0 => {
                *engine
                    .state
                    .zone_change_generation
                    .entry(accepted)
                    .or_default() += 1
            }
            1 => grant_control(&mut engine, accepted, 17),
            2 => engine.state.continuous_effects.push(ContinuousEffect {
                trigger_grant_origin: None,
                source_id: None,
                affected: AffectedScope::Single(accepted),
                kind: ContinuousEffectKind::Layer4SetTypeLine(
                    tricerules_card_model::primitives::TypeLineReplacement {
                        card_types: vec![
                            tricerules_card_model::primitives::PermanentTypeFilter::Artifact,
                        ],
                        artifact_types: vec![],
                        creature_types: vec![],
                        land_types: vec![],
                    },
                ),
                condition: None,
                duration: EffectDuration::Indefinite,
                timestamp: engine.state.command_index + 1,
            }),
            _ => {
                engine
                    .apply_command(
                        29,
                        &RuledCommand {
                            cmd: Some(rv1::ruled_command::Cmd::Concede(rv1::Concede {})),
                        },
                    )
                    .unwrap();
            }
        }
        let mut events = Vec::new();
        assert!(engine
            .restore_suspended_observer_resolution(Some(saved), None, &mut events)
            .unwrap());
        engine.publish_resolution_prompt_ids(&mut events).unwrap();
        let pending = engine.state.pending_resolution.as_ref().unwrap();
        assert_eq!(pending.deciding_player, 41);
        let ResolutionContinuation::PlayerSetSacrifice { sacrifice, .. } = &pending.continuation
        else {
            unreachable!();
        };
        assert!(sacrifice.accepted.is_empty());
        answer(&mut engine, 41, vec![current]);
        assert_eq!(engine.state.objects[&current].zone, Zone::Graveyard);
        if mutation < 3 {
            assert_eq!(engine.state.objects[&accepted].zone, Zone::Battlefield);
            assert_eq!(engine.state.objects[&unchosen].zone, Zone::Battlefield);
        } else {
            assert!(!engine.state.objects.contains_key(&accepted));
        }
        assert_eq!(engine.state.players[2].life, 21);
        assert!(engine.state.pending_resolution.is_none());
        assert!(engine.state.stack.is_empty());
    }
}
