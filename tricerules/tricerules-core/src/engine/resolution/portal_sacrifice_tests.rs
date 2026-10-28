//! Focused Portal test fixtures for the ordered, simultaneous sacrifice choice.
use super::*;

const PORTAL: &str = include_str!("../../../../tricerules-cards/data/portal_to_phyrexia.ron");
const TYPE_CHOICE_FIXTURE: &str = r#"(
    id: "type_choice_sacrifice_fixture",
    name: "Type Choice Sacrifice Fixture",
    face_id: "type_choice_sacrifice_fixture",
    mana_cost: "{0}",
    types: ["Instant"],
    spell_effect: [ChoosePermanentsByTypeAndSacrificeRest(
        players: EachOpponent,
        types: [Artifact],
        scope: AllPermanents,
    )],
)"#;
const MULTITYPE_PERMANENT_FIXTURE: &str = r#"(
    id: "multitype_permanent_fixture",
    name: "Multitype Permanent Fixture",
    face_id: "multitype_permanent_fixture",
    mana_cost: "{0}",
    types: ["Artifact", "Creature"],
    power: 1,
    toughness: 1,
)"#;
const TYPE_CHOICE_MULTITYPE_FIXTURE: &str = r#"(
    id: "type_choice_multitype_fixture",
    name: "Type Choice Multitype Fixture",
    face_id: "type_choice_multitype_fixture",
    mana_cost: "{0}",
    types: ["Instant"],
    spell_effect: [ChoosePermanentsByTypeAndSacrificeRest(
        players: EachOpponent,
        types: [Artifact, Creature],
        scope: AllPermanents,
    )],
)"#;
const TYPE_CHOICE_NONLAND_FIXTURE: &str = r#"(
    id: "type_choice_nonland_fixture",
    name: "Type Choice Nonland Fixture",
    face_id: "type_choice_nonland_fixture",
    mana_cost: "{0}",
    types: ["Instant"],
    spell_effect: [ChoosePermanentsByTypeAndSacrificeRest(
        players: EachPlayer,
        types: [Artifact],
        scope: NonlandPermanents,
    )],
)"#;
const TYPE_CHOICE_TAIL_FIXTURE: &str = r#"(
    id: "type_choice_tail_fixture",
    name: "Type Choice Tail Fixture",
    face_id: "type_choice_tail_fixture",
    mana_cost: "{0}",
    types: ["Instant"],
    spell_effect: [
        ChoosePermanentsByTypeAndSacrificeRest(
            players: EachOpponent,
            types: [Artifact],
            scope: AllPermanents,
        ),
        Draw(count: 1),
    ],
)"#;

fn fresh() -> GameEngine {
    let registry = Box::leak(Box::new(
        CardRegistry::from_chunks_and_tokens(
            &[
                include_str!("../../../../tricerules-cards/data/mountain.ron"),
                include_str!("../../../../tricerules-cards/data/grizzly_bears.ron"),
                include_str!("../../../../tricerules-cards/data/sol_ring.ron"),
                include_str!("../../../../tricerules-cards/data/generated/d/darksteel_citadel.ron"),
                include_str!("../../../../tricerules-cards/data/storm_crow.ron"),
                include_str!("../../../../tricerules-cards/data/beloved_chaplain.ron"),
                include_str!("../../../../tricerules-cards/data/gladecover_scout.ron"),
                include_str!("../../../../tricerules-cards/data/argothian_enchantress.ron"),
                include_str!("../../../../tricerules-cards/data/cackling_prowler.ron"),
                MULTITYPE_PERMANENT_FIXTURE,
                TYPE_CHOICE_FIXTURE,
                TYPE_CHOICE_MULTITYPE_FIXTURE,
                TYPE_CHOICE_NONLAND_FIXTURE,
                TYPE_CHOICE_TAIL_FIXTURE,
                PORTAL,
            ],
            &[],
        )
        .unwrap(),
    ));
    let deck = EngineDeck {
        mainboard: vec!["mountain".into(); 20],
        ..Default::default()
    };
    let mut engine = GameEngine::new_with_registry(
        670_121,
        &[4, 9, 27, 51],
        20,
        Some(vec![deck; 4]),
        true,
        registry,
    )
    .unwrap();
    engine.state.turn_step = TurnStep::Main1;
    engine
}

fn cast_type_choice_spell(engine: &mut GameEngine, spell: &str) -> ObjectId {
    let source = place(engine, 0, spell, Zone::Hand);
    let slot = engine.state.players[0]
        .hand
        .iter()
        .position(|id| *id == source)
        .unwrap();
    engine
        .apply_scenario_command(
            4,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::CastSpell(rv1::CastSpell {
                    cast_method: rv1::CastMethod::Normal as i32,
                    source: Some(rv1::CastSource {
                        location: Some(rv1::cast_source::Location::HandIndex(slot as u32)),
                        ..Default::default()
                    }),
                    ..Default::default()
                })),
            },
        )
        .unwrap();
    source
}

/// Resolve the stack until its first choice or until its stack object finishes.
fn resolve_type_choice_stack(engine: &mut GameEngine) -> bool {
    resolve_type_choice_stack_with_prompt(engine).is_some()
}

fn resolve_type_choice_stack_with_prompt(engine: &mut GameEngine) -> Option<rv1::RuledEventBatch> {
    let pass = RuledCommand {
        cmd: Some(rv1::ruled_command::Cmd::PassPriority(Default::default())),
    };
    for _ in 0..20 {
        if engine.state.pending_resolution.is_some() {
            return None;
        }
        if engine.state.stack.is_empty() {
            return None;
        }
        let batch = engine
            .apply_scenario_command(engine.state.priority_player_id(), &pass)
            .unwrap();
        if engine.state.pending_resolution.is_some() {
            return Some(batch);
        }
    }
    panic!("type-choice spell did not finish or open a choice");
}

/// Cast a zero-cost fixture and resolve until its first choice or completion.
fn resolve_type_choice_spell(engine: &mut GameEngine, spell: &str) -> bool {
    cast_type_choice_spell(engine, spell);
    resolve_type_choice_stack(engine)
}

fn choose_delegate(branch_index: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(
            rv1::SubmitResolutionChoice {
                decision: rv1::ResolutionChoiceDecision::SelectBranch as i32,
                selected_branch_index: branch_index,
                ..Default::default()
            },
        )),
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
    engine.reindex_battlefield_control(&mut Vec::new());
}

fn remove_permanent_types(engine: &mut GameEngine, permanent: ObjectId) {
    engine.state.continuous_effects.push(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: None,
        affected: AffectedScope::Single(permanent),
        kind: ContinuousEffectKind::Layer4SetTypeLine(
            tricerules_card_model::primitives::TypeLineReplacement {
                card_types: Vec::new(),
                creature_types: Vec::new(),
                land_types: Vec::new(),
            },
        ),
        condition: None,
        duration: EffectDuration::Indefinite,
        timestamp: engine.state.command_index + 1,
    });
}

fn type_choice_departure_fixture() -> (GameEngine, [ObjectId; 2], [ObjectId; 2]) {
    let mut engine = fresh();
    let native = [
        place(
            &mut engine,
            2,
            "multitype_permanent_fixture",
            Zone::Battlefield,
        ),
        place(
            &mut engine,
            2,
            "multitype_permanent_fixture",
            Zone::Battlefield,
        ),
    ];
    let borrowed = [
        place(
            &mut engine,
            2,
            "multitype_permanent_fixture",
            Zone::Battlefield,
        ),
        place(
            &mut engine,
            2,
            "multitype_permanent_fixture",
            Zone::Battlefield,
        ),
    ];
    for object_id in borrowed {
        grant_control(&mut engine, object_id, 51);
    }
    cast_type_choice_spell(&mut engine, "type_choice_multitype_fixture");
    engine.state.active_player_idx = 2;
    engine.state.priority_idx = 2;
    assert!(resolve_type_choice_stack(&mut engine));
    let first = engine.state.pending_resolution.as_ref().unwrap();
    assert_eq!(first.deciding_player, 27);
    assert_eq!(first.presentation.candidates, native);
    (engine, native, borrowed)
}

fn concede(engine: &mut GameEngine, player: PlayerId) {
    engine
        .apply_scenario_command(
            player,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::Concede(rv1::Concede {})),
            },
        )
        .unwrap();
}

fn delegate_branch_for(engine: &GameEngine, delegate: PlayerId) -> u32 {
    let pending = engine.state.pending_resolution.as_ref().unwrap();
    let ResolutionContinuation::TypeChoiceSacrifice { sacrifice, .. } = &pending.continuation
    else {
        panic!("expected type-choice delegate continuation");
    };
    let crate::state::PendingTypeChoiceSacrificeStage::ChoosingDelegate { candidate_slots } =
        &sacrifice.stage
    else {
        panic!("expected delegate-selection stage");
    };
    candidate_slots
        .iter()
        .position(|candidate| *candidate == Some(delegate))
        .expect("requested live player is an offered delegate") as u32
}

fn finish_pending_graveyard_order(engine: &mut GameEngine) {
    let pending = engine.state.pending_resolution.as_ref().unwrap();
    assert!(matches!(
        pending.continuation,
        ResolutionContinuation::MassSacrificeGraveyardOrder { .. }
    ));
    let decider = pending.deciding_player;
    let order = pending.presentation.candidates.clone();
    engine
        .apply_scenario_command(decider, &answer(order))
        .unwrap();
}

#[test]
fn type_choice_sacrifice_starts_with_the_active_player_in_apnap_order() {
    let mut engine = fresh();
    let active_player_artifacts = [
        place(&mut engine, 2, "sol_ring", Zone::Battlefield),
        place(&mut engine, 2, "sol_ring", Zone::Battlefield),
    ];
    let source = place(&mut engine, 0, "type_choice_sacrifice_fixture", Zone::Hand);
    let slot = engine.state.players[0]
        .hand
        .iter()
        .position(|id| *id == source)
        .unwrap();
    engine
        .apply_scenario_command(
            4,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::CastSpell(rv1::CastSpell {
                    cast_method: rv1::CastMethod::Normal as i32,
                    source: Some(rv1::CastSource {
                        location: Some(rv1::cast_source::Location::HandIndex(slot as u32)),
                        ..Default::default()
                    }),
                    ..Default::default()
                })),
            },
        )
        .unwrap();
    engine.state.active_player_idx = 2;
    engine.state.priority_idx = 2;

    let mut first_choice = None;
    for _ in 0..12 {
        if engine.state.pending_resolution.is_some() {
            break;
        }
        let batch = engine
            .apply_scenario_command(
                engine.state.priority_player_id(),
                &RuledCommand {
                    cmd: Some(rv1::ruled_command::Cmd::PassPriority(Default::default())),
                },
            )
            .unwrap();
        first_choice = batch
            .events
            .iter()
            .find_map(|event| match event.ev.as_ref()? {
                rv1::ruled_event::Ev::ResolutionChoiceRequired(choice) => Some(choice.clone()),
                _ => None,
            });
        if first_choice.is_some() {
            break;
        }
    }

    let choice = first_choice.expect("the operation opens its first type choice");
    assert_eq!(choice.deciding_player_id, 27);
    assert_eq!(choice.candidate_object_ids, active_player_artifacts);
    assert_eq!(choice.min, 1);
    assert_eq!(choice.max, 1);
}

#[test]
fn type_choice_reuses_a_multitype_keep_and_publishes_one_exact_reference() {
    let mut engine = fresh();
    let kept = place(
        &mut engine,
        2,
        "multitype_permanent_fixture",
        Zone::Battlefield,
    );
    let sacrificed = place(
        &mut engine,
        2,
        "multitype_permanent_fixture",
        Zone::Battlefield,
    );
    cast_type_choice_spell(&mut engine, "type_choice_multitype_fixture");
    engine.state.active_player_idx = 2;
    engine.state.priority_idx = 2;
    assert!(resolve_type_choice_stack(&mut engine));

    let first = engine.state.pending_resolution.as_ref().unwrap();
    assert_eq!(first.deciding_player, 27);
    assert_eq!(first.presentation.candidates, [kept, sacrificed]);
    assert!(engine
        .apply_scenario_command(27, &answer(vec![kept]))
        .is_ok());
    let second = engine.state.pending_resolution.as_ref().unwrap();
    assert_eq!(second.deciding_player, 27);
    assert_eq!(second.presentation.candidates, [kept, sacrificed]);
    let progress = engine
        .state
        .pending_resolution
        .as_ref()
        .and_then(|pending| match &pending.continuation {
            ResolutionContinuation::TypeChoiceSacrifice { sacrifice, .. } => {
                Some(sacrifice.accepted.clone())
            }
            _ => None,
        })
        .expect("the next type prompt keeps the same continuation");
    assert_eq!(progress.len(), 1);
    assert_eq!(progress[0].object_id, kept);
    assert_eq!(progress[0].zone_change_generation, 0);

    let second_answer = engine
        .apply_scenario_command(27, &answer(vec![kept]))
        .unwrap();
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(engine.state.objects[&kept].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&sacrificed].zone, Zone::Graveyard);
    let public_prompt = second_answer
        .events
        .iter()
        .find_map(|event| match event.ev.as_ref()? {
            rv1::ruled_event::Ev::ResolutionChoiceRequired(choice) => Some(choice),
            _ => None,
        });
    assert!(
        public_prompt.is_none(),
        "the final keep clears its public marker"
    );
    assert!(second_answer.events.iter().any(|event| matches!(
        event.ev.as_ref(),
        Some(rv1::ruled_event::Ev::ResolutionChoiceProgress(progress))
            if progress.accepted_permanent_selections.is_empty()
    )));
}

#[test]
fn type_choice_kept_object_survives_type_and_control_changes_in_the_same_generation() {
    let mut engine = fresh();
    let kept = place(
        &mut engine,
        1,
        "multitype_permanent_fixture",
        Zone::Battlefield,
    );
    let creature_a = place(
        &mut engine,
        1,
        "multitype_permanent_fixture",
        Zone::Battlefield,
    );
    let creature_b = place(
        &mut engine,
        1,
        "multitype_permanent_fixture",
        Zone::Battlefield,
    );
    cast_type_choice_spell(&mut engine, "type_choice_multitype_fixture");
    engine.state.active_player_idx = 1;
    engine.state.priority_idx = 1;
    assert!(resolve_type_choice_stack(&mut engine));
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .presentation
            .candidates,
        [kept, creature_a, creature_b]
    );

    engine
        .apply_scenario_command(9, &answer(vec![kept]))
        .unwrap();
    let second_type = engine.state.pending_resolution.as_ref().unwrap();
    assert_eq!(second_type.deciding_player, 9);
    assert_eq!(
        second_type.presentation.candidates,
        [kept, creature_a, creature_b]
    );
    let accepted = match &second_type.continuation {
        ResolutionContinuation::TypeChoiceSacrifice { sacrifice, .. } => &sacrifice.accepted,
        _ => panic!("the second type prompt retains the first exact keep"),
    };
    assert_eq!(accepted.len(), 1);
    assert_eq!(accepted[0].object_id, kept);
    assert_eq!(accepted[0].zone_change_generation, 0);

    remove_permanent_types(&mut engine, kept);
    engine.state.command_index += 1;
    grant_control(&mut engine, kept, 4);
    assert_eq!(engine.characteristics(kept).unwrap().controller, 4);
    engine.state.command_index += 1;
    grant_control(&mut engine, kept, 9);
    assert_eq!(engine.characteristics(kept).unwrap().controller, 9);
    assert!(engine.characteristics(kept).unwrap().types.is_empty());
    assert_eq!(engine.state.zone_change_generation[&kept], 0);

    engine
        .apply_scenario_command(9, &answer(vec![creature_a]))
        .expect("the still-legal creature candidate can finish the subject's choices");
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(engine.state.objects[&kept].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&creature_a].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&creature_b].zone, Zone::Graveyard);
}

#[test]
fn type_choice_skips_missing_types_and_sacrifices_typeless_permanents() {
    let mut engine = fresh();
    let creature = place(&mut engine, 1, "grizzly_bears", Zone::Battlefield);
    let typeless = place(&mut engine, 2, "grizzly_bears", Zone::Battlefield);
    remove_permanent_types(&mut engine, typeless);
    assert!(engine.characteristics(typeless).unwrap().types.is_empty());

    assert!(!resolve_type_choice_spell(
        &mut engine,
        "type_choice_sacrifice_fixture"
    ));
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(engine.state.objects[&creature].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&typeless].zone, Zone::Graveyard);
}

#[test]
fn type_choice_auto_keeps_the_sole_candidate_without_opening_a_prompt() {
    let mut engine = fresh();
    let ring = place(&mut engine, 1, "sol_ring", Zone::Battlefield);
    let creature = place(&mut engine, 1, "grizzly_bears", Zone::Battlefield);

    assert!(!resolve_type_choice_spell(
        &mut engine,
        "type_choice_sacrifice_fixture"
    ));
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(engine.state.objects[&ring].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&creature].zone, Zone::Graveyard);
}

#[test]
fn type_choice_nonland_scope_hides_and_preserves_artifact_lands() {
    let mut engine = fresh();
    let ring_one = place(&mut engine, 1, "sol_ring", Zone::Battlefield);
    let ring_two = place(&mut engine, 1, "sol_ring", Zone::Battlefield);
    let artifact_land = place(&mut engine, 1, "darksteel_citadel", Zone::Battlefield);

    assert!(resolve_type_choice_spell(
        &mut engine,
        "type_choice_nonland_fixture"
    ));
    let prompt = engine.state.pending_resolution.as_ref().unwrap();
    assert_eq!(prompt.deciding_player, 9);
    assert_eq!(prompt.presentation.candidates, [ring_one, ring_two]);
    engine
        .apply_scenario_command(9, &answer(vec![ring_one]))
        .unwrap();
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(engine.state.objects[&ring_one].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&ring_two].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&artifact_land].zone, Zone::Battlefield);
}

#[test]
fn type_choice_rejects_wrong_actor_duplicate_and_reentered_object_atomically() {
    let mut engine = fresh();
    let candidates = [
        place(&mut engine, 1, "sol_ring", Zone::Battlefield),
        place(&mut engine, 1, "sol_ring", Zone::Battlefield),
    ];
    assert!(resolve_type_choice_spell(
        &mut engine,
        "type_choice_sacrifice_fixture"
    ));

    let before_wrong_actor = format!("{:?}", engine.state);
    assert!(engine
        .apply_scenario_command(4, &answer(vec![candidates[0]]))
        .is_err());
    assert_eq!(format!("{:?}", engine.state), before_wrong_actor);

    let before_duplicate = format!("{:?}", engine.state);
    assert!(engine
        .apply_scenario_command(9, &answer(vec![candidates[0], candidates[0]]))
        .is_err());
    assert_eq!(format!("{:?}", engine.state), before_duplicate);

    super::move_object_to_zone(
        &mut engine.state,
        engine.registry,
        candidates[0],
        Zone::Graveyard,
        None,
    )
    .unwrap();
    super::move_object_to_zone(
        &mut engine.state,
        engine.registry,
        candidates[0],
        Zone::Battlefield,
        None,
    )
    .unwrap();
    assert!(engine.state.zone_change_generation[&candidates[0]] > 0);
    let before_stale = format!("{:?}", engine.state);
    assert!(engine
        .apply_scenario_command(9, &answer(vec![candidates[0]]))
        .is_err());
    assert_eq!(format!("{:?}", engine.state), before_stale);

    engine
        .apply_scenario_command(9, &answer(vec![candidates[1]]))
        .unwrap();
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(engine.state.objects[&candidates[0]].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&candidates[1]].zone, Zone::Battlefield);
}

#[test]
fn type_choice_face_down_candidate_hides_card_identity() {
    let mut engine = fresh();
    let hidden = place(&mut engine, 1, "grizzly_bears", Zone::Battlefield);
    let visible = place(&mut engine, 1, "storm_crow", Zone::Battlefield);
    engine.state.objects.get_mut(&hidden).unwrap().face_down = true;

    cast_type_choice_spell(&mut engine, "type_choice_multitype_fixture");
    engine.state.active_player_idx = 2;
    engine.state.priority_idx = 2;
    let batch = resolve_type_choice_stack_with_prompt(&mut engine)
        .expect("the opponent receives a creature keep choice");
    let choice = batch
        .events
        .iter()
        .find_map(|event| match event.ev.as_ref()? {
            rv1::ruled_event::Ev::ResolutionChoiceRequired(choice) => Some(choice),
            _ => None,
        })
        .expect("the prompt is published with candidate labels");
    let hidden_index = choice
        .candidate_object_ids
        .iter()
        .position(|object_id| *object_id == hidden)
        .expect("the face-down permanent remains an eligible creature");
    assert!(choice.candidate_card_ids[hidden_index].is_empty());
    assert_eq!(choice.candidate_names[hidden_index], "Face-down permanent");
    assert!(choice.candidate_object_ids.contains(&visible));

    engine
        .apply_scenario_command(9, &answer(vec![hidden]))
        .expect("a face-down permanent can be kept without revealing its identity");
    assert_eq!(engine.state.objects[&hidden].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&visible].zone, Zone::Graveyard);
}

#[test]
fn type_choice_is_not_targeting_and_ignores_protection_hexproof_shroud_and_ward() {
    let mut engine = fresh();
    let protected = place(&mut engine, 1, "beloved_chaplain", Zone::Battlefield);
    let hexproof = place(&mut engine, 1, "gladecover_scout", Zone::Battlefield);
    let shroud = place(&mut engine, 1, "argothian_enchantress", Zone::Battlefield);
    let warded = place(&mut engine, 1, "cackling_prowler", Zone::Battlefield);

    assert!(!engine
        .characteristics(protected)
        .unwrap()
        .protections
        .is_empty());
    assert!(engine
        .characteristics(hexproof)
        .unwrap()
        .has_keyword(tricerules_card_model::primitives::Keyword::Hexproof));
    assert!(engine
        .characteristics(shroud)
        .unwrap()
        .has_keyword(tricerules_card_model::primitives::Keyword::Shroud));

    cast_type_choice_spell(&mut engine, "type_choice_multitype_fixture");
    engine.state.active_player_idx = 2;
    engine.state.priority_idx = 2;
    assert!(resolve_type_choice_stack(&mut engine));
    let choice = engine.state.pending_resolution.as_ref().unwrap();
    assert_eq!(choice.deciding_player, 9);
    assert_eq!(
        choice.presentation.candidates,
        [protected, hexproof, shroud, warded]
    );
    engine
        .apply_scenario_command(9, &answer(vec![protected]))
        .expect("the untargeted effect permits a protected choice");
    assert_eq!(engine.state.objects[&protected].zone, Zone::Battlefield);
    for object_id in [hexproof, shroud, warded] {
        assert_eq!(engine.state.objects[&object_id].zone, Zone::Graveyard);
    }
}

#[test]
fn type_choice_resumes_the_effect_list_tail_once_after_graveyard_order() {
    let mut engine = fresh();
    let artifacts = (0..3)
        .map(|_| place(&mut engine, 1, "sol_ring", Zone::Battlefield))
        .collect::<Vec<_>>();
    let library_before = engine.state.players[0].library.len();
    assert!(resolve_type_choice_spell(
        &mut engine,
        "type_choice_tail_fixture"
    ));

    engine
        .apply_scenario_command(9, &answer(vec![artifacts[0]]))
        .unwrap();
    assert!(matches!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .continuation,
        ResolutionContinuation::MassSacrificeGraveyardOrder { .. }
    ));
    finish_pending_graveyard_order(&mut engine);
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(engine.state.players[0].library.len(), library_before - 1);
    assert_eq!(engine.state.objects[&artifacts[0]].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&artifacts[1]].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&artifacts[2]].zone, Zone::Graveyard);
}

#[test]
fn type_choice_retains_a_subject_who_departed_before_their_first_type_choice() {
    let (mut engine, native, borrowed) = type_choice_departure_fixture();
    concede(&mut engine, 51);

    let refreshed = engine.state.pending_resolution.as_ref().unwrap();
    assert_eq!(refreshed.deciding_player, 27);
    assert_eq!(
        refreshed.presentation.candidates,
        [native[0], native[1], borrowed[0], borrowed[1]]
    );
    assert_eq!(engine.state.objects[&borrowed[0]].controller, 27);
    assert_eq!(engine.state.objects[&borrowed[1]].controller, 27);

    engine
        .apply_scenario_command(27, &answer(vec![native[0]]))
        .unwrap();
    engine
        .apply_scenario_command(27, &answer(vec![native[0]]))
        .unwrap();
    let delegate = engine.state.pending_resolution.as_ref().unwrap();
    assert_eq!(delegate.deciding_player, 4);
    assert_eq!(
        delegate.presentation.choice_kind,
        rv1::ChoiceKind::ResolutionBranch
    );
    let ResolutionContinuation::TypeChoiceSacrifice { sacrifice, .. } = &delegate.continuation
    else {
        panic!("departed subject slot remains in the type-choice continuation");
    };
    assert_eq!(sacrifice.current_subject, 1);
    assert_eq!(sacrifice.current_type, 0);
    let mut captured = sacrifice.subjects[1]
        .departure_cohort
        .as_ref()
        .unwrap()
        .clone();
    let mut expected_captured = borrowed.map(|object_id| (object_id, 0)).to_vec();
    captured.sort_unstable();
    expected_captured.sort_unstable();
    assert_eq!(captured, expected_captured);
    let branch = delegate_branch_for(&engine, 27);
    engine
        .apply_scenario_command(4, &choose_delegate(branch))
        .unwrap();
    let delegated = engine.state.pending_resolution.as_ref().unwrap();
    assert_eq!(delegated.deciding_player, 27);
    assert_eq!(delegated.presentation.candidates, borrowed);

    engine
        .apply_scenario_command(27, &answer(vec![borrowed[0]]))
        .unwrap();
    let repeated_type = engine.state.pending_resolution.as_ref().unwrap();
    assert_eq!(repeated_type.deciding_player, 27);
    assert_eq!(repeated_type.presentation.candidates, borrowed);
    let progress = match &repeated_type.continuation {
        ResolutionContinuation::TypeChoiceSacrifice { sacrifice, .. } => sacrifice.accepted.clone(),
        _ => panic!("same subject's second type prompt"),
    };
    assert_eq!(progress.len(), 2);
    assert!(progress.iter().any(|selection| {
        selection.player == 51
            && selection.object_id == borrowed[0]
            && selection.zone_change_generation == 0
    }));

    engine
        .apply_scenario_command(27, &answer(vec![borrowed[0]]))
        .unwrap();
    finish_pending_graveyard_order(&mut engine);
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(engine.state.objects[&native[0]].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&native[1]].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&borrowed[0]].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&borrowed[1]].zone, Zone::Graveyard);
}

#[test]
fn type_choice_departure_after_a_type_choice_keeps_the_cursor_and_exact_keep() {
    let (mut engine, native, borrowed) = type_choice_departure_fixture();
    engine
        .apply_scenario_command(27, &answer(vec![native[0]]))
        .unwrap();
    engine
        .apply_scenario_command(27, &answer(vec![native[0]]))
        .unwrap();
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .deciding_player,
        51
    );
    engine
        .apply_scenario_command(51, &answer(vec![borrowed[0]]))
        .unwrap();
    let before_departure = engine.state.pending_resolution.as_ref().unwrap();
    assert_eq!(before_departure.deciding_player, 51);
    assert_eq!(before_departure.presentation.candidates, borrowed);
    let ResolutionContinuation::TypeChoiceSacrifice { sacrifice, .. } =
        &before_departure.continuation
    else {
        panic!("type-choice continuation remains active after the first type");
    };
    assert_eq!(sacrifice.current_subject, 1);
    assert_eq!(sacrifice.current_type, 1);
    assert!(sacrifice.accepted.iter().any(|selection| {
        selection.player == 51
            && selection.object_id == borrowed[0]
            && selection.zone_change_generation == 0
    }));

    concede(&mut engine, 51);
    let delegate = engine.state.pending_resolution.as_ref().unwrap();
    assert_eq!(delegate.deciding_player, 4);
    assert_eq!(
        delegate.presentation.choice_kind,
        rv1::ChoiceKind::ResolutionBranch
    );
    let ResolutionContinuation::TypeChoiceSacrifice { sacrifice, .. } = &delegate.continuation
    else {
        panic!("delegate selection retains the original type cursor");
    };
    assert_eq!(sacrifice.current_subject, 1);
    assert_eq!(sacrifice.current_type, 1);
    assert_eq!(sacrifice.accepted.len(), 2);
    let branch = delegate_branch_for(&engine, 27);
    engine
        .apply_scenario_command(4, &choose_delegate(branch))
        .unwrap();
    let delegated = engine.state.pending_resolution.as_ref().unwrap();
    assert_eq!(delegated.deciding_player, 27);
    assert_eq!(delegated.presentation.candidates, borrowed);
    let accepted = match &delegated.continuation {
        ResolutionContinuation::TypeChoiceSacrifice { sacrifice, .. } => &sacrifice.accepted,
        _ => unreachable!(),
    };
    assert_eq!(accepted.len(), 2);
    assert!(accepted.iter().any(|selection| {
        selection.player == 51
            && selection.object_id == borrowed[0]
            && selection.zone_change_generation == 0
    }));

    engine
        .apply_scenario_command(27, &answer(vec![borrowed[0]]))
        .unwrap();
    finish_pending_graveyard_order(&mut engine);
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(engine.state.objects[&native[0]].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&native[1]].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&borrowed[0]].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&borrowed[1]].zone, Zone::Graveyard);
}

#[test]
fn type_choice_survives_controller_departure_and_uses_next_live_selector() {
    let mut engine = fresh();
    let borrowed = [
        place(&mut engine, 1, "sol_ring", Zone::Battlefield),
        place(&mut engine, 1, "sol_ring", Zone::Battlefield),
    ];
    for object_id in borrowed {
        grant_control(&mut engine, object_id, 27);
    }
    let source = cast_type_choice_spell(&mut engine, "type_choice_sacrifice_fixture");
    engine.state.active_player_idx = 2;
    engine.state.priority_idx = 2;
    assert!(resolve_type_choice_stack(&mut engine));
    let first = engine.state.pending_resolution.as_ref().unwrap();
    assert_eq!(first.deciding_player, 27);
    assert_eq!(first.presentation.candidates, borrowed);
    assert!(matches!(
        first.continuation,
        ResolutionContinuation::TypeChoiceSacrifice { .. }
    ));

    concede(&mut engine, 4);
    assert!(engine.state.players[0].has_lost);
    assert!(engine.state.pending_resolution.is_some());
    assert!(matches!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .continuation,
        ResolutionContinuation::TypeChoiceSacrifice { .. }
    ));
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .deciding_player,
        27
    );

    concede(&mut engine, 27);
    assert!(engine.state.players[2].has_lost);
    let selector = engine.state.pending_resolution.as_ref().unwrap();
    assert_eq!(selector.deciding_player, 9);
    assert_eq!(
        selector.presentation.choice_kind,
        rv1::ChoiceKind::ResolutionBranch
    );
    let ResolutionContinuation::TypeChoiceSacrifice { stack, sacrifice } = &selector.continuation
    else {
        panic!("started resolution survives its controller and subject departures");
    };
    assert_eq!(stack.item.controller, 4);
    assert_eq!(sacrifice.current_subject, 0);
    assert_eq!(sacrifice.subjects[0].player, 27);
    assert!(sacrifice.subjects[0].departure_cohort.is_some());
    assert_eq!(engine.state.objects[&borrowed[0]].controller, 9);

    let branch = delegate_branch_for(&engine, 51);
    engine
        .apply_scenario_command(9, &choose_delegate(branch))
        .unwrap();
    let delegated = engine.state.pending_resolution.as_ref().unwrap();
    assert_eq!(delegated.deciding_player, 51);
    assert_eq!(delegated.presentation.candidates, borrowed);
    engine
        .apply_scenario_command(51, &answer(vec![borrowed[0]]))
        .unwrap();
    let later_subject = engine.state.pending_resolution.as_ref().unwrap();
    assert_eq!(later_subject.deciding_player, 9);
    assert_eq!(later_subject.presentation.candidates, borrowed);
    let accepted = match &later_subject.continuation {
        ResolutionContinuation::TypeChoiceSacrifice { sacrifice, .. } => &sacrifice.accepted,
        _ => unreachable!(),
    };
    assert_eq!(accepted.len(), 1);
    assert_eq!(accepted[0].object_id, borrowed[0]);
    engine
        .apply_scenario_command(9, &answer(vec![borrowed[0]]))
        .unwrap();
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(engine.state.objects[&borrowed[0]].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&borrowed[1]].zone, Zone::Graveyard);
    assert!(
        !engine.state.objects.contains_key(&source),
        "the departed spell owner still removes the physical spell object"
    );
}

fn place(engine: &mut GameEngine, seat: usize, card: &str, zone: Zone) -> ObjectId {
    let oid = engine.state.next_object_id;
    engine.state.next_object_id += 1;
    let player = engine.state.players[seat].id;
    let object = new_object_from_card(
        oid,
        player,
        card,
        zone,
        engine.registry.get(card).unwrap().primary_face(),
    );
    engine.state.objects.insert(oid, object);
    engine.state.zone_change_generation.insert(oid, 0);
    match zone {
        Zone::Hand => engine.state.players[seat].hand.push(oid),
        Zone::Battlefield => {
            engine.state.players[seat].battlefield.push(oid);
            engine.emit_static_abilities_on_enter(oid);
        }
        Zone::Graveyard => engine.state.players[seat].graveyard.push(oid),
        _ => unreachable!(),
    }
    oid
}

fn answer(ids: Vec<ObjectId>) -> RuledCommand {
    RuledCommand {
        cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(
            rv1::SubmitResolutionChoice {
                chosen_object_ids: ids,
                decision: rv1::ResolutionChoiceDecision::Unspecified as i32,
                ..Default::default()
            },
        )),
    }
}

fn resolve_portal_to_first_choice(engine: &mut GameEngine) -> Vec<rv1::RuledEvent> {
    let portal = place(engine, 0, "portal_to_phyrexia", Zone::Hand);
    let slot = engine.state.players[0]
        .hand
        .iter()
        .position(|id| *id == portal)
        .unwrap();
    engine.state.players[0].mana_pool.colorless = 9;
    engine
        .apply_scenario_command(
            4,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::CastSpell(rv1::CastSpell {
                    cast_method: rv1::CastMethod::Normal as i32,
                    source: Some(rv1::CastSource {
                        location: Some(rv1::cast_source::Location::HandIndex(slot as u32)),
                        ..Default::default()
                    }),
                    ..Default::default()
                })),
            },
        )
        .unwrap();
    let mut events = Vec::new();
    for _ in 0..12 {
        if engine.state.pending_resolution.is_some() {
            return events;
        }
        let batch = engine
            .apply_scenario_command(
                engine.state.priority_player_id(),
                &RuledCommand {
                    cmd: Some(rv1::ruled_command::Cmd::PassPriority(Default::default())),
                },
            )
            .unwrap();
        events.extend(batch.events);
    }
    panic!("Portal did not reach its ETB sacrifice choice");
}

fn target_portal_upkeep_trigger(engine: &mut GameEngine, target: ObjectId) {
    let active_player = engine.state.active_player_id();
    engine.state.turn_step = TurnStep::Upkeep;
    let mut events = Vec::new();
    engine.fire_triggers(
        &[GameEvent::PhaseBegan {
            phase: rv1::PhaseId::Upkeep,
            active_player,
        }],
        &mut events,
    );
    engine.flush_staged_triggers(&mut events);
    assert_eq!(
        engine.state.pending_triggers.front().unwrap().controller,
        active_player
    );
    let generation = engine.state.zone_change_generation[&target];
    engine
        .apply_scenario_command(
            active_player,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::ChooseTriggerTarget(
                    rv1::ChooseTriggerTarget {
                        targets: vec![rv1::TargetRef {
                            object_id: target,
                            kind: rv1::TargetRefKind::Graveyard as i32,
                            expected_zone_change_generation: Some(generation),
                            ..Default::default()
                        }],
                        ..Default::default()
                    },
                )),
            },
        )
        .unwrap();
}

#[test]
fn portal_upkeep_returns_an_opponents_creature_as_phyrexian_until_it_changes_zones() {
    let mut engine = fresh();
    let _portal = place(&mut engine, 0, "portal_to_phyrexia", Zone::Battlefield);
    let target = place(&mut engine, 1, "grizzly_bears", Zone::Graveyard);

    target_portal_upkeep_trigger(&mut engine, target);
    let pass = RuledCommand {
        cmd: Some(rv1::ruled_command::Cmd::PassPriority(Default::default())),
    };
    for _ in 0..12 {
        if engine.state.objects[&target].zone == Zone::Battlefield {
            break;
        }
        let player = engine.state.priority_player_id();
        engine.apply_scenario_command(player, &pass).unwrap();
    }

    let object = &engine.state.objects[&target];
    assert_eq!(object.zone, Zone::Battlefield);
    assert_eq!(object.owner, 9);
    assert_eq!(object.controller, 4);
    assert_eq!(engine.state.zone_change_generation[&target], 1);
    assert!(engine.characteristics(target).unwrap().is_creature());
    assert!(engine
        .characteristics(target)
        .unwrap()
        .has_type("Phyrexian"));

    super::move_object_to_zone(
        &mut engine.state,
        engine.registry,
        target,
        Zone::Graveyard,
        None,
    )
    .unwrap();
    assert!(!engine
        .characteristics(target)
        .unwrap()
        .has_type("Phyrexian"));
}

#[test]
fn portal_collects_exact_apnap_choices_before_one_simultaneous_sacrifice() {
    let mut engine = fresh();
    let opponent_one = (0..4)
        .map(|_| place(&mut engine, 1, "grizzly_bears", Zone::Battlefield))
        .collect::<Vec<_>>();
    let opponent_two = (0..4)
        .map(|_| place(&mut engine, 2, "grizzly_bears", Zone::Battlefield))
        .collect::<Vec<_>>();
    let opponent_three = (0..4)
        .map(|_| place(&mut engine, 3, "grizzly_bears", Zone::Battlefield))
        .collect::<Vec<_>>();
    resolve_portal_to_first_choice(&mut engine);

    let first = engine.state.pending_resolution.as_ref().unwrap();
    assert_eq!(first.deciding_player, 9);
    assert_eq!(first.presentation.candidates.len(), 4);
    let first_picks = opponent_one[..3].to_vec();
    let accepted = engine
        .apply_scenario_command(9, &answer(first_picks.clone()))
        .unwrap();
    let second = engine.state.pending_resolution.as_ref().unwrap();
    assert_eq!(second.deciding_player, 27);
    assert!(matches!(
        second.continuation,
        ResolutionContinuation::PlayerSetSacrifice { .. }
    ));
    assert!(opponent_one
        .iter()
        .chain(&opponent_two)
        .chain(&opponent_three)
        .all(|oid| { engine.state.objects[oid].zone == Zone::Battlefield }));
    let prompt = accepted
        .events
        .iter()
        .find_map(|event| match event.ev.as_ref()? {
            rv1::ruled_event::Ev::ResolutionChoiceRequired(choice) => Some(choice),
            _ => None,
        })
        .expect("next APNAP prompt is public");
    assert_eq!(prompt.accepted_permanent_selections.len(), 3);
    for pick in &first_picks {
        assert!(prompt
            .accepted_permanent_selections
            .iter()
            .any(|selection| {
                selection.selecting_player_id == 9
                    && selection.object_id == *pick
                    && selection.zone_change_generation == 0
            }));
    }

    let second_picks = opponent_two[..3].to_vec();
    let third_prompt = engine
        .apply_scenario_command(27, &answer(second_picks.clone()))
        .unwrap();
    let third = engine.state.pending_resolution.as_ref().unwrap();
    assert_eq!(third.deciding_player, 51);
    let prompt = third_prompt
        .events
        .iter()
        .find_map(|event| match event.ev.as_ref()? {
            rv1::ruled_event::Ev::ResolutionChoiceRequired(choice) => Some(choice),
            _ => None,
        })
        .expect("third APNAP prompt is public");
    assert_eq!(prompt.accepted_permanent_selections.len(), 6);
    assert!(first_picks.iter().chain(&second_picks).all(|pick| {
        prompt
            .accepted_permanent_selections
            .iter()
            .any(|selection| selection.object_id == *pick && selection.zone_change_generation == 0)
    }));

    let third_picks = opponent_three[..3].to_vec();
    engine
        .apply_scenario_command(51, &answer(third_picks.clone()))
        .unwrap();
    for pick in first_picks.iter().chain(&second_picks).chain(&third_picks) {
        assert_eq!(engine.state.objects[pick].zone, Zone::Graveyard);
    }
    assert_eq!(
        engine.state.objects[&opponent_one[3]].zone,
        Zone::Battlefield
    );
    assert_eq!(
        engine.state.objects[&opponent_two[3]].zone,
        Zone::Battlefield
    );
    assert_eq!(
        engine.state.objects[&opponent_three[3]].zone,
        Zone::Battlefield
    );
    assert!(matches!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .continuation,
        ResolutionContinuation::MassSacrificeGraveyardOrder { .. }
    ));
}

#[test]
fn portal_rejects_stale_exact_object_choice_without_consuming_the_prompt() {
    let mut engine = fresh();
    let candidates = (0..4)
        .map(|_| place(&mut engine, 1, "grizzly_bears", Zone::Battlefield))
        .collect::<Vec<_>>();
    place(&mut engine, 2, "grizzly_bears", Zone::Battlefield);
    place(&mut engine, 2, "grizzly_bears", Zone::Battlefield);
    place(&mut engine, 3, "grizzly_bears", Zone::Battlefield);
    place(&mut engine, 3, "grizzly_bears", Zone::Battlefield);
    resolve_portal_to_first_choice(&mut engine);
    engine.state.zone_change_generation.insert(candidates[0], 1);
    let before = format!("{:?}", engine.state);
    assert!(engine
        .apply_scenario_command(9, &answer(candidates[..3].to_vec()))
        .is_err());
    assert_eq!(format!("{:?}", engine.state), before);
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .deciding_player,
        9
    );
}
