use super::*;
use crate::state::ManifestDreadLibraryRef;

#[test]
fn prototyped_entry_uses_selected_color_for_battlefield_counter_replacements() {
    let mut engine = engine();
    let counter_source = object(&mut engine, "mind_stone", Zone::Battlefield, 0);
    copy_face(
        &mut engine,
        counter_source,
        r#"(
            id: "red_creature_entry_counter_fixture",
            name: "Red Creature Entry Counter Fixture",
            face_id: "red_creature_entry_counter_fixture",
            types: ["Artifact"],
            static_abilities: [(ability_id: "static_01", presentation: Fallback,
                definition: EntersWithCounters(
                    affected: Creatures((controller: Some(YouControl), color: Some(Red))),
                    counter: PlusOnePlusOne,
                    amount: 1,
                ))],
        )"#,
        "red_creature_entry_counter_fixture",
    );
    engine.emit_static_abilities_on_enter(counter_source);

    let prototyped = object(&mut engine, "skitterbeam_battalion", Zone::Stack, 0);
    engine
        .state
        .objects
        .get_mut(&prototyped)
        .unwrap()
        .prototyped = true;
    let entry = event(&engine, prototyped);
    let preview = engine
        .battlefield_entry_characteristics_through_layer_5(&entry)
        .expect("Prototype entrant characteristics");
    assert_eq!(preview.colors, [Color::Red]);
    assert_eq!(preview.mana_value, 5);
    assert!(
        !engine.battlefield_entry_candidates(&entry).is_empty(),
        "a red Prototype entrant matches a red-creature counter replacement"
    );
}

#[test]
fn devotion_actual_nylea_and_supporter_simultaneous_entry_use_complete_committed_cohort() {
    let mut engine = engine();
    object(&mut engine, "aggressive_mammoth", Zone::Battlefield, 0);
    let observer = object(&mut engine, "soul_warden", Zone::Battlefield, 0);
    let god = object(&mut engine, "nylea,_god_of_the_hunt", Zone::Stack, 0);
    let support = object(&mut engine, "grizzly_bears", Zone::Stack, 0);
    let god_entry = event(&engine, god);
    let support_entry = event(&engine, support);
    assert!(!engine
        .battlefield_entry_characteristics_through_layer_5(&god_entry)
        .unwrap()
        .is_creature());
    let mut triggers = engine
        .commit_battlefield_entry_state(god_entry, None)
        .unwrap();
    triggers.extend(
        engine
            .commit_battlefield_entry_state(support_entry, None)
            .unwrap(),
    );
    triggers.extend([
        GameEvent::EntersBattlefield {
            object_id: god,
            chosen_x: 0,
            entry_cause: None,
        },
        GameEvent::EntersBattlefield {
            object_id: support,
            chosen_x: 0,
            entry_cause: None,
        },
    ]);
    let mut events = vec![];
    engine.fire_triggers(&triggers, &mut events);
    assert!(engine.characteristics(god).unwrap().is_creature());
    let collected: Vec<_> = engine
        .state
        .staged_trigger_groups
        .iter()
        .flat_map(|group| &group.triggers)
        .collect();
    assert_eq!(
        collected.len(),
        2,
        "Soul Warden sees both creatures after the complete cohort commits"
    );
    assert!(collected
        .iter()
        .all(|trigger| trigger.source_permanent_id == observer));
}

#[test]
fn devotion_actual_nylea_excludes_own_pip_for_entry_replacement_and_includes_it_for_etb() {
    let mut engine = engine();
    object(&mut engine, "aggressive_mammoth", Zone::Battlefield, 0);
    object(&mut engine, "llanowar_elves", Zone::Battlefield, 0);
    let observer = object(&mut engine, "soul_warden", Zone::Battlefield, 0);
    engine.emit_static_abilities_on_enter(observer);
    let counters = object(&mut engine, "mind_stone", Zone::Battlefield, 0);
    copy_face(
        &mut engine,
        counters,
        r#"(
        id: "entry_creature_counters", name: "Entry Creature Counters", face_id: "entry_creature_counters",
        types: ["Artifact"], static_abilities: [(ability_id: "static_01", presentation: Fallback,
            definition: EntersWithCounters(affected: Creatures((controller: Some(YouControl))),
                counter: PlusOnePlusOne, amount: 1))])"#,
        "entry_creature_counters",
    );
    engine.emit_static_abilities_on_enter(counters);
    let god = object(&mut engine, "nylea,_god_of_the_hunt", Zone::Stack, 0);
    let entry = event(&engine, god);
    assert!(!engine
        .battlefield_entry_characteristics_through_layer_5(&entry)
        .unwrap()
        .is_creature());
    assert!(
        engine.battlefield_entry_candidates(&entry).is_empty(),
        "four existing green pips do not receive a creature-entry counter replacement"
    );
    let mut events = vec![];
    let item = engine.observer_return_item(god, 0);
    let progress = engine.advance_or_park_battlefield_entry(
        item,
        entry,
        BattlefieldEntryCompletion::PermanentSpell { attached_to: None },
        &mut events,
    );
    let BattlefieldEntryProgress::Ready(entry) = progress else {
        panic!("entry has no replacement choice");
    };
    engine
        .commit_battlefield_entry(*entry, None, &mut events)
        .unwrap();
    assert_eq!(
        engine.state.objects[&god].counter_count(CounterKind::PlusOnePlusOne),
        0
    );
    assert!(
        engine.characteristics(god).unwrap().is_creature(),
        "committed own pip makes five"
    );
    engine.flush_staged_triggers(&mut events);
    assert_eq!(
        engine.state.stack.len(),
        1,
        "Soul Warden sees the committed creature entrant"
    );
    assert_eq!(engine.state.stack[0].source_permanent_id, Some(observer));
}

fn engine() -> GameEngine {
    engine_with_players(&[0, 1])
}

fn engine_with_players(players: &[PlayerId]) -> GameEngine {
    let mut engine = GameEngine::new_with_default_decks(
        tricerules_cards::registry::global(),
        305_030,
        players,
        20,
    )
    .unwrap();
    engine.state.opening = None;
    engine.state.priority_idx = 0;
    engine.state.turn_step = TurnStep::Main1;
    engine.state.command_index = 5;
    engine
}

fn suspended_conductive_machete_manifest_dread() -> (
    GameEngine,
    ParkedStackResolution,
    [ObjectId; 2],
    [ManifestDreadLibraryRef; 2],
) {
    suspended_conductive_machete_manifest_dread_for_players(&[0, 1])
}

fn suspended_conductive_machete_manifest_dread_for_players(
    players: &[PlayerId],
) -> (
    GameEngine,
    ParkedStackResolution,
    [ObjectId; 2],
    [ManifestDreadLibraryRef; 2],
) {
    let mut engine = engine_with_players(players);
    let equipment = engine.state.players[0].hand[0];
    engine.state.objects.get_mut(&equipment).unwrap().card_id = "conductive_machete".into();
    engine.state.players[0].mana_pool.colorless = 10;
    engine
        .cast_spell(
            0,
            &rv1::CastSpell {
                cast_method: rv1::CastMethod::Normal as i32,
                source: Some(rv1::CastSource {
                    location: Some(rv1::cast_source::Location::HandIndex(0)),
                    expected_zone_change_generation: None,
                }),
                ..Default::default()
            },
        )
        .expect("cast Conductive Machete");
    engine
        .resolve_top_of_stack(&mut Vec::new())
        .expect("resolve Equipment spell and trigger its ability");
    engine.flush_staged_triggers(&mut Vec::new());
    engine
        .resolve_top_of_stack(&mut Vec::new())
        .expect("begin the triggered Manifest Dread Program");
    let pending = engine
        .state
        .pending_resolution
        .take()
        .expect("two-card Manifest Dread choice");
    let [first, second] = pending
        .presentation
        .candidates
        .as_slice()
        .try_into()
        .expect("top two cards are offered");
    let (stack, refs) = match &pending.continuation {
        ResolutionContinuation::ManifestDread { stack, looked_at } => {
            (stack.clone(), looked_at.clone())
        }
        _ => panic!("Manifest Dread retains its Program frame"),
    };
    let refs: [ManifestDreadLibraryRef; 2] = refs.try_into().expect("two exact library refs");
    (engine, stack, [first, second], refs)
}

fn skipped_manifest_dread_order_prompt() -> (
    GameEngine,
    [ObjectId; 2],
    [ManifestDreadLibraryRef; 2],
    RuledEventBatch,
) {
    skipped_manifest_dread_order_prompt_for_players(&[0, 1])
}

fn skipped_manifest_dread_order_prompt_for_players(
    players: &[PlayerId],
) -> (
    GameEngine,
    [ObjectId; 2],
    [ManifestDreadLibraryRef; 2],
    RuledEventBatch,
) {
    let (mut engine, stack, looked_at, library_refs) =
        suspended_conductive_machete_manifest_dread_for_players(players);
    let skipped_entry = event(&engine, looked_at[0]);
    engine
        .state
        .objects
        .get_mut(&looked_at[0])
        .unwrap()
        .face_down = true;
    let completion = BattlefieldEntryCompletion::ManifestDread {
        owner: 0,
        library_refs: library_refs.to_vec(),
    };
    let completed = engine
        .finish_entry_copy_without_recipient(stack, skipped_entry, completion, Vec::new())
        .expect("complete skipped Manifest Dread entry");
    (engine, looked_at, library_refs, completed)
}

#[test]
fn manifest_dread_skipped_two_card_completion_offers_private_owner_order() {
    let (engine, looked_at, _library_refs, completed) = skipped_manifest_dread_order_prompt();
    assert!(looked_at
        .iter()
        .all(|object_id| engine.state.objects[object_id].zone == Zone::Library));
    assert!(!looked_at
        .iter()
        .any(|object_id| engine.state.players[0].graveyard.contains(object_id)));
    assert!(!completed.events.iter().any(|event| matches!(
        event.ev.as_ref(),
        Some(rv1::ruled_event::Ev::PermanentMoved(moved))
            if looked_at.contains(&moved.object_id)
    )));
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("two-card skip must ask its owner to order the private graveyard cohort");
    assert_eq!(pending.deciding_player, 0);
    assert_eq!(pending.presentation.candidates, looked_at);
    assert_eq!(pending.presentation.min, 2);
    assert_eq!(pending.presentation.max, 2);
    assert!(pending.presentation.ordered);
    assert_eq!(
        pending.presentation.choice_kind,
        rv1::ChoiceKind::SimultaneousEntryOrder
    );
    assert!(completed.events.iter().any(|event| matches!(
        event.ev.as_ref(),
        Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(choice))
            if choice.deciding_player_id == 0
                && choice.choice_kind == rv1::ChoiceKind::SimultaneousEntryOrder as i32
                && choice.candidate_object_ids == looked_at
                && choice.min == 2
                && choice.max == 2
                && choice.ordered
                && choice.public_reveal.is_none()
    )));
}

#[test]
fn manifest_dread_skipped_two_card_order_commits_both_permutations_with_original_library_positions()
{
    for reverse in [false, true] {
        let (mut engine, looked_at, library_refs, _) = skipped_manifest_dread_order_prompt();
        let next_library_card = engine.state.players[0].library[2];
        let mut order = looked_at.to_vec();
        if reverse {
            order.reverse();
        }
        let batch = engine
            .submit_resolution_choice(
                0,
                &rv1::SubmitResolutionChoice {
                    chosen_object_ids: order.clone(),
                    ..Default::default()
                },
            )
            .expect("owner orders both skipped cards");

        let ordered_graveyard_moves = batch
            .events
            .iter()
            .filter_map(|event| match event.ev.as_ref() {
                Some(rv1::ruled_event::Ev::PermanentMoved(moved))
                    if moved.destination == rv1::permanent_moved::Destination::Graveyard as i32 =>
                {
                    Some((moved.object_id, moved.source_library_position))
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        let expected_positions = order
            .iter()
            .map(|object_id| {
                (
                    *object_id,
                    Some(
                        library_refs
                            .iter()
                            .find(|reference| reference.object_id == *object_id)
                            .unwrap()
                            .library_position,
                    ),
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(ordered_graveyard_moves, expected_positions);
        assert_eq!(engine.state.players[0].graveyard.last(), order.last());
        assert!(looked_at
            .iter()
            .all(|object_id| engine.state.objects[object_id].zone == Zone::Graveyard));
        assert_eq!(engine.state.players[0].library[0], next_library_card);
        assert!(engine.state.pending_resolution.is_none());
        assert!(engine.state.stack.is_empty());
    }
}

#[test]
fn manifest_dread_one_card_skipped_completion_moves_only_its_exact_current_ref() {
    let (mut engine, stack, looked_at, library_refs) =
        suspended_conductive_machete_manifest_dread();
    let skipped_entry = event(&engine, looked_at[0]);
    engine
        .state
        .objects
        .get_mut(&looked_at[0])
        .unwrap()
        .face_down = true;
    let completed = engine
        .finish_entry_copy_without_recipient(
            stack,
            skipped_entry,
            BattlefieldEntryCompletion::ManifestDread {
                owner: 0,
                library_refs: vec![library_refs[0]],
            },
            Vec::new(),
        )
        .expect("complete one-card skipped entry directly");
    assert_eq!(engine.state.objects[&looked_at[0]].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&looked_at[1]].zone, Zone::Library);
    assert!(engine.state.pending_resolution.is_none());
    assert!(completed.events.iter().any(|event| matches!(
        event.ev.as_ref(),
        Some(rv1::ruled_event::Ev::PermanentMoved(moved))
            if moved.object_id == looked_at[0]
                && moved.source_library_position == Some(0)
                && moved.destination == rv1::permanent_moved::Destination::Graveyard as i32
    )));
    assert!(!completed.events.iter().any(|event| matches!(
        event.ev.as_ref(),
        Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(_))
    )));
}

#[test]
fn manifest_dread_skipped_commander_resumes_none_tail_before_903_9a_choice() {
    const TAIL_FIXTURE: &str = r#"(
        id: "manifest_dread_tail_fixture",
        name: "Manifest Dread Tail Fixture",
        face_id: "manifest_dread_tail_fixture",
        mana_cost: "",
        types: ["Artifact", "Equipment"],
        triggered_abilities: [(
            ability_id: "triggered_01",
            presentation: Fallback,
            trigger: WhenSelfEntersBattlefield,
            effect: [],
            program: Some((body: (steps: [
                (id: "manifest_dread", bind: ["manifested"], input: None,
                    kind: Perform(ManifestDread)),
                (id: "attach_equipment", bind: [], input: Some("manifested"),
                    kind: Perform(AttachEquipment(
                        equipment: Source,
                        creature: PreviousEffectObject,
                    ))),
                (id: "tail_choice", bind: [], input: None, kind: Choose(
                    choice: "tail_observer",
                    chooser: Controller,
                    optional: false,
                    branches: [
                        (branch_id: "tail_gain_a", presentation: Fallback, cost: None,
                            requirement: Always, body: (steps: [
                                (id: "tail_gain_a", bind: [], input: None,
                                    kind: Perform(GainLife(amount: 1))),
                            ], yields: [])),
                        (branch_id: "tail_gain_b", presentation: Fallback, cost: None,
                            requirement: Always, body: (steps: [
                                (id: "tail_gain_b", bind: [], input: None,
                                    kind: Perform(GainLife(amount: 1))),
                            ], yields: [])),
                    ],
                    otherwise: (steps: [], yields: []),
                )),
            ], yields: []))),
        )],
    )"#;

    let registry = Box::leak(Box::new(
        tricerules_cards::CardRegistry::from_chunks_and_tokens(
            &[
                TAIL_FIXTURE,
                include_str!("../../../../tricerules-cards/data/forest.ron"),
                include_str!("../../../../tricerules-cards/data/kami_of_the_crescent_moon.ron"),
            ],
            &[],
        )
        .expect("test-only Manifest Dread fixture registry"),
    ));
    let deck = |commanders: Vec<String>| crate::engine::EngineDeck {
        mainboard: vec!["forest".to_owned(); 12],
        commanders,
    };
    let mut engine = GameEngine::new_with_commander_decks(
        registry,
        305_031,
        &[0, 1],
        20,
        Some(vec![
            deck(vec!["kami_of_the_crescent_moon".into()]),
            deck(Vec::new()),
        ]),
        true,
    )
    .expect("Commander engine with test-only Program fixture");
    engine.state.priority_idx = 0;
    engine.state.turn_step = TurnStep::Main1;
    engine.state.command_index = 5;

    let commander = engine.state.players[0].command_zone[0];
    engine.state.players[0]
        .command_zone
        .retain(|object_id| *object_id != commander);
    let commander_object = engine.state.objects.get_mut(&commander).unwrap();
    commander_object.zone = Zone::Library;
    *engine
        .state
        .zone_change_generation
        .entry(commander)
        .or_default() += 1;
    engine.state.players[0].library.push_front(commander);

    let creature = object(
        &mut engine,
        "kami_of_the_crescent_moon",
        Zone::Battlefield,
        0,
    );
    let equipment = engine.state.players[0].hand[0];
    engine.state.objects.get_mut(&equipment).unwrap().card_id =
        "manifest_dread_tail_fixture".into();
    engine.state.players[0].mana_pool.colorless = 10;
    engine
        .cast_spell(
            0,
            &rv1::CastSpell {
                cast_method: rv1::CastMethod::Normal as i32,
                source: Some(rv1::CastSource {
                    location: Some(rv1::cast_source::Location::HandIndex(0)),
                    expected_zone_change_generation: None,
                }),
                ..Default::default()
            },
        )
        .expect("cast test-only Equipment fixture");
    engine
        .resolve_top_of_stack(&mut Vec::new())
        .expect("resolve test Equipment and stage its trigger");
    engine.flush_staged_triggers(&mut Vec::new());
    engine
        .resolve_top_of_stack(&mut Vec::new())
        .expect("park the test Program on its real Manifest Dread choice");
    let pending = engine
        .state
        .pending_resolution
        .take()
        .expect("Manifest Dread offers its top two cards");
    let [commander, other] = pending
        .presentation
        .candidates
        .as_slice()
        .try_into()
        .expect("the Commander and next Library card are offered");
    assert_eq!(commander, engine.state.players[0].library[0]);
    let (stack, library_refs) = match pending.continuation {
        ResolutionContinuation::ManifestDread { stack, looked_at } => (stack, looked_at),
        _ => panic!("Manifest Dread retains the exact parked Program"),
    };
    assert_eq!(library_refs[0].object_id, commander);

    let skipped_entry = event(&engine, commander);
    engine.state.objects.get_mut(&commander).unwrap().face_down = true;
    let completed = engine
        .finish_entry_copy_without_recipient(
            stack,
            skipped_entry,
            BattlefieldEntryCompletion::ManifestDread {
                owner: 0,
                library_refs: library_refs.clone(),
            },
            Vec::new(),
        )
        .expect("synthetic skipped-entry seam retains the private two-card cohort");
    assert!(completed.events.iter().any(|event| matches!(
        event.ev.as_ref(),
        Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(choice))
            if choice.choice_kind == rv1::ChoiceKind::SimultaneousEntryOrder as i32
                && choice.public_reveal.is_none()
    )));
    assert!(engine.state.players[0].library.contains(&commander));
    assert!(engine.state.players[0].library.contains(&other));

    let order = vec![other, commander];
    let ordered = engine
        .submit_resolution_choice(
            0,
            &rv1::SubmitResolutionChoice {
                chosen_object_ids: order,
                ..Default::default()
            },
        )
        .expect("owner privately orders both cards into the graveyard");
    assert_eq!(engine.state.objects[&commander].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&other].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&equipment].attached_to, None);
    assert_eq!(engine.state.objects[&creature].attached_to, None);
    assert_eq!(engine.state.players[0].life, 20);

    let tail_pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("the same Program reaches its observable tail choice");
    assert_eq!(
        tail_pending.presentation.choice_kind,
        rv1::ChoiceKind::ResolutionBranch
    );
    let ResolutionContinuation::ProgramChoose { stack, .. } = &tail_pending.continuation else {
        panic!("the tail choice retains the Program cursor");
    };
    let program = stack.program().expect("parked Program state");
    let compiled = engine
        .registry
        .compiled_program_for_owner(&program.owner)
        .expect("test fixture Program remains compiled");
    let manifested_slot = compiled
        .slots
        .iter()
        .find(|slot| slot.name == "manifested")
        .expect("Manifest Dread output slot");
    assert!(matches!(
        program.slots.get(manifested_slot.id.0 as usize),
        Some(Some(crate::state::ProgramValue::ManifestedPermanent(None)))
    ));
    assert!(engine.state.players[0].graveyard.contains(&commander));
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .deciding_player,
        0
    );
    assert!(!ordered.events.iter().any(|event| matches!(
        event.ev.as_ref(),
        Some(rv1::ruled_event::Ev::StackResolved(_))
    )));

    let tail_prompt_id = tail_pending.resolution_prompt_id.get();
    let tail_batch = engine
        .submit_resolution_choice(
            0,
            &rv1::SubmitResolutionChoice {
                resolution_prompt_id: tail_prompt_id,
                decision: rv1::ResolutionChoiceDecision::SelectBranch as i32,
                selected_branch_index: 0,
                ..Default::default()
            },
        )
        .expect("complete the observable Program tail exactly once");
    let life_deltas = tail_batch
        .events
        .iter()
        .filter_map(|event| match event.ev.as_ref() {
            Some(rv1::ruled_event::Ev::LifeChanged(change)) if change.player_id == 0 => {
                Some(change.delta)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(life_deltas, vec![1]);
    assert_eq!(engine.state.players[0].life, 21);
    let stack_resolved_index = tail_batch
        .events
        .iter()
        .position(|event| {
            matches!(
                event.ev.as_ref(),
                Some(rv1::ruled_event::Ev::StackResolved(_))
            )
        })
        .expect("the triggered Program resolves after its tail");
    let commander_choice_index = tail_batch
        .events
        .iter()
        .position(|event| {
            matches!(
                event.ev.as_ref(),
                Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(choice))
                    if choice.choice_kind == rv1::ChoiceKind::ResolutionBranch as i32
                        && choice.prompt_text.contains("903.9a")
            )
        })
        .expect("Commander state-based choice follows completed resolution");
    assert!(stack_resolved_index < commander_choice_index);
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .expect("Commander choice pending")
            .deciding_player,
        0
    );

    let commander_prompt_id = engine
        .state
        .pending_resolution
        .as_ref()
        .unwrap()
        .resolution_prompt_id
        .get();
    let commander_answer = engine
        .submit_resolution_choice(
            0,
            &rv1::SubmitResolutionChoice {
                resolution_prompt_id: commander_prompt_id,
                decision: rv1::ResolutionChoiceDecision::SelectBranch as i32,
                selected_branch_index: 0,
                ..Default::default()
            },
        )
        .expect("owner chooses the command zone for the declared Commander");
    assert_eq!(engine.state.objects[&commander].zone, Zone::Command);
    assert_eq!(engine.state.players[0].life, 21);
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(engine.state.stack.len(), 0);
    assert_eq!(
        commander_answer
            .events
            .iter()
            .filter(|event| matches!(
                event.ev.as_ref(),
                Some(rv1::ruled_event::Ev::LifeChanged(_))
                    | Some(rv1::ruled_event::Ev::StackResolved(_))
            ))
            .count(),
        0,
        "answering the later Commander choice cannot rerun the completed Program tail"
    );
}

#[test]
fn manifest_dread_skipped_order_rejects_wrong_actor_malformed_and_stale_answers_atomically() {
    let (mut engine, looked_at, _, _) = skipped_manifest_dread_order_prompt();
    let before = engine.state.players[0].graveyard.clone();
    let valid = rv1::SubmitResolutionChoice {
        chosen_object_ids: looked_at.to_vec(),
        ..Default::default()
    };
    assert!(engine.submit_resolution_choice(1, &valid).is_err());
    assert_eq!(engine.state.players[0].graveyard, before);
    assert!(engine.state.pending_resolution.is_some());

    let duplicate = rv1::SubmitResolutionChoice {
        chosen_object_ids: vec![looked_at[0], looked_at[0]],
        ..Default::default()
    };
    assert!(engine.submit_resolution_choice(0, &duplicate).is_err());
    assert_eq!(engine.state.players[0].graveyard, before);
    assert!(engine.state.pending_resolution.is_some());

    let unrelated = rv1::SubmitResolutionChoice {
        chosen_object_ids: looked_at.to_vec(),
        selected_branch_index: 1,
        ..Default::default()
    };
    assert!(engine.submit_resolution_choice(0, &unrelated).is_err());
    assert_eq!(engine.state.players[0].graveyard, before);
    assert!(engine.state.pending_resolution.is_some());

    let (mut stale, stale_ids, _, _) = skipped_manifest_dread_order_prompt();
    *stale
        .state
        .zone_change_generation
        .entry(stale_ids[0])
        .or_insert(0) += 1;
    let stale_before = stale.state.players[0].graveyard.clone();
    assert!(stale
        .submit_resolution_choice(
            0,
            &rv1::SubmitResolutionChoice {
                chosen_object_ids: stale_ids.to_vec(),
                ..Default::default()
            },
        )
        .is_err());
    assert_eq!(stale.state.players[0].graveyard, stale_before);
    assert!(stale.state.pending_resolution.is_some());

    let (mut wrong_zone, wrong_zone_ids, _, _) = skipped_manifest_dread_order_prompt();
    move_object_to_zone(
        &mut wrong_zone.state,
        wrong_zone.registry,
        wrong_zone_ids[0],
        Zone::Hand,
        None,
    )
    .unwrap();
    let wrong_zone_before = wrong_zone.state.players[0].graveyard.clone();
    assert!(wrong_zone
        .submit_resolution_choice(
            0,
            &rv1::SubmitResolutionChoice {
                chosen_object_ids: wrong_zone_ids.to_vec(),
                ..Default::default()
            },
        )
        .is_err());
    assert_eq!(wrong_zone.state.players[0].graveyard, wrong_zone_before);
    assert_eq!(
        wrong_zone.state.objects[&wrong_zone_ids[1]].zone,
        Zone::Library
    );
    assert!(wrong_zone.state.pending_resolution.is_some());
}

#[test]
fn manifest_dread_skipped_order_owner_departure_retires_without_moving_partial_cohort() {
    let (mut engine, looked_at, _, _) = skipped_manifest_dread_order_prompt_for_players(&[0, 1, 2]);
    engine.state.players[0].has_lost = true;
    let mut events = Vec::new();
    engine
        .reconcile_departed_players(&mut events)
        .expect("retire unavailable owner cohort");

    assert!(engine.state.pending_resolution.is_none());
    assert!(looked_at.iter().all(|object_id| {
        !engine.state.players[0].graveyard.contains(object_id)
            && engine
                .state
                .objects
                .get(object_id)
                .is_none_or(|object| object.zone != Zone::Graveyard)
    }));
    assert!(!events.iter().any(|event| matches!(
        event.ev.as_ref(),
        Some(rv1::ruled_event::Ev::PermanentMoved(moved))
            if looked_at.contains(&moved.object_id)
    )));
}

#[test]
fn manifest_dread_skipped_order_preserves_live_cohort_across_source_and_unrelated_departures() {
    let (mut engine, looked_at, _, _) = skipped_manifest_dread_order_prompt_for_players(&[0, 1, 2]);
    let source = engine
        .state
        .objects
        .values()
        .find(|object| object.card_id == "conductive_machete" && object.zone == Zone::Battlefield)
        .expect("Equipment source remains on the battlefield")
        .id;
    move_object_to_zone(
        &mut engine.state,
        engine.registry,
        source,
        Zone::Graveyard,
        None,
    )
    .unwrap();
    let mut source_departure_events = Vec::new();
    engine
        .refresh_manifest_dread_graveyard_order_departure(&mut source_departure_events)
        .expect("source departure preserves the live owner cohort");
    assert!(matches!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .map(|pending| &pending.continuation),
        Some(ResolutionContinuation::ManifestDreadGraveyardOrder { .. })
    ));
    assert!(looked_at
        .iter()
        .all(|object_id| engine.state.objects[object_id].zone == Zone::Library));

    engine.state.players[1].has_lost = true;
    let mut unrelated_departure_events = Vec::new();
    engine
        .reconcile_departed_players(&mut unrelated_departure_events)
        .expect("unrelated player departure preserves the live owner cohort");
    assert!(matches!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .map(|pending| &pending.continuation),
        Some(ResolutionContinuation::ManifestDreadGraveyardOrder { .. })
    ));
    assert!(looked_at
        .iter()
        .all(|object_id| engine.state.objects[object_id].zone == Zone::Library));
}

#[test]
fn reanimated_multiversal_passage_as_enters_choice_belongs_to_its_controller() {
    let mut engine = engine();
    let passage = object(&mut engine, "multiversal_passage", Zone::Graveyard, 0);
    let mut entry = event(&engine, passage);
    entry.deciding_player = 0; // owner
    entry.destination_controller = 1; // reanimation controller
    let mut events = Vec::new();
    let completion = BattlefieldEntryCompletion::ResolutionEffect {
        owner: 0,
        spell_label: "Portal to Phyrexia".to_owned(),
        object_label: "Multiversal Passage".to_owned(),
        from_zone: Zone::Graveyard,
        triggered_return_generation: None,
    };

    let progress = engine.advance_or_park_battlefield_entry(
        engine.observer_return_item(passage, 1),
        entry,
        completion,
        &mut events,
    );
    assert!(matches!(progress, BattlefieldEntryProgress::Parked));
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .deciding_player,
        1
    );
    assert!(events.iter().any(|event| matches!(
        event.ev.as_ref(),
        Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(choice))
            if choice.deciding_player_id == 1
    )));
}

#[test]
fn reanimated_battle_protector_choice_belongs_to_its_controller() {
    let mut engine = engine();
    let battle = object(
        &mut engine,
        "invasion_of_ulgrotha_grandmother_ravi_sengir",
        Zone::Graveyard,
        0,
    );
    let mut entry = event(&engine, battle);
    entry.deciding_player = 0; // owner
    entry.destination_controller = 1; // reanimation controller
    let completion = BattlefieldEntryCompletion::ResolutionEffect {
        owner: 0,
        spell_label: "Portal to Phyrexia".to_owned(),
        object_label: "Invasion of Ulgrotha".to_owned(),
        from_zone: Zone::Graveyard,
        triggered_return_generation: None,
    };
    let mut events = Vec::new();

    let progress = engine.advance_or_park_battlefield_entry(
        engine.observer_return_item(battle, 1),
        entry,
        completion,
        &mut events,
    );
    assert!(matches!(progress, BattlefieldEntryProgress::Parked));
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .deciding_player,
        1
    );
    assert!(events.iter().any(|event| matches!(
        event.ev.as_ref(),
        Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(choice))
            if choice.deciding_player_id == 1
                && choice.choice_kind == rv1::ChoiceKind::BattleProtector as i32
                && choice.candidate_object_ids == [0]
    )));
}

#[test]
fn reanimated_aura_recipient_choice_belongs_to_its_controller() {
    let mut engine = engine();
    let _recipient = object(&mut engine, "grizzly_bears", Zone::Battlefield, 1);
    let aura = object(&mut engine, "pacifism", Zone::Graveyard, 0);
    let mut entry = event(&engine, aura);
    entry.deciding_player = 0; // owner
    entry.destination_controller = 1; // reanimation controller
    let completion = BattlefieldEntryCompletion::ResolutionEffect {
        owner: 0,
        spell_label: "Portal to Phyrexia".to_owned(),
        object_label: "Pacifism".to_owned(),
        from_zone: Zone::Graveyard,
        triggered_return_generation: None,
    };
    let mut events = Vec::new();

    let progress = engine.advance_or_park_battlefield_entry(
        engine.observer_return_item(aura, 1),
        entry,
        completion,
        &mut events,
    );
    assert!(matches!(progress, BattlefieldEntryProgress::Parked));
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .deciding_player,
        1
    );
    assert!(events.iter().any(|event| matches!(
        event.ev.as_ref(),
        Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(choice))
            if choice.deciding_player_id == 1
                && choice.choice_kind == rv1::ChoiceKind::AuraPermanent as i32
    )));
}

#[test]
fn entry_counter_doublers_use_a_shared_precommit_source_snapshot() {
    let mut engine = engine();
    let active_season = object(&mut engine, "doubling_season", Zone::Battlefield, 0);
    engine.emit_static_abilities_on_enter(active_season);

    let entering_season = object(&mut engine, "doubling_season", Zone::Stack, 0);
    let prism = object(&mut engine, "astral_cornucopia", Zone::Stack, 0);
    let season_entry = event(&engine, entering_season);
    let mut prism_entry = event(&engine, prism);
    append_entry_counter_placement(
        &mut prism_entry,
        0,
        [(CounterKind::Charge, 2)],
        EntryCounterSource::ResolutionEffect {
            stack_object_id: entering_season,
            stack_zone_change_generation: engine
                .state
                .zone_change_generation
                .get(&entering_season)
                .copied()
                .unwrap_or(0),
            effect_index: 0,
        },
        false,
    );
    let mut entries = vec![season_entry, prism_entry];

    let references = entries.iter().collect::<Vec<_>>();
    let proposals = engine.proposed_entry_counter_placements(&references, &[]);
    let (counter_batch, choices) = engine.prepare_counter_placement_batch(proposals).unwrap();
    assert!(
        choices.is_none(),
        "the sole counter replacement applies automatically"
    );
    assert_eq!(counter_batch.events.len(), 1);
    assert_eq!(counter_batch.events[0].counters[&CounterKind::Charge], 4);
    for entry in &mut entries {
        GameEngine::apply_entry_counter_placement_results(entry, &counter_batch.events).unwrap();
    }
    assert_eq!(
        entries[1].entry_counters[&CounterKind::Charge],
        4,
        "the source snapshot includes the established season but excludes the simultaneous entrant"
    );

    for entry in entries {
        engine.commit_battlefield_entry_state(entry, None).unwrap();
    }
    assert_eq!(
        engine.state.objects[&prism].counter_count(CounterKind::Charge),
        4,
        "sequential internal commits retain the simultaneous event's shared multiplier"
    );
}

#[test]
fn laezel_entering_with_another_counter_bearing_permanent_does_not_replace_its_entry() {
    let mut engine = engine();
    let incoming_laezel = object(&mut engine, "laezel,_vlaakiths_champion", Zone::Stack, 0);
    let incoming_creature = object(&mut engine, "grizzly_bears", Zone::Stack, 0);
    let mut creature_entry = event(&engine, incoming_creature);
    append_test_entry_counters(
        &engine,
        &mut creature_entry,
        0,
        [(CounterKind::PlusOnePlusOne, 1)],
    );
    let mut entries = vec![event(&engine, incoming_laezel), creature_entry];

    // The shared snapshot is the current battlefield. Lae'zel in the uncommitted entry cohort
    // cannot apply her replacement to the other entrant's pre-commit counter placement.
    let references = entries.iter().collect::<Vec<_>>();
    let proposals = engine.proposed_entry_counter_placements(&references, &[]);
    let (counter_batch, choices) = engine.prepare_counter_placement_batch(proposals).unwrap();
    assert!(choices.is_none());
    assert_eq!(
        counter_batch.events[0].counters[&CounterKind::PlusOnePlusOne],
        1,
        "a simultaneous entrant is not an established replacement source"
    );
    for entry in &mut entries {
        GameEngine::apply_entry_counter_placement_results(entry, &counter_batch.events).unwrap();
    }

    for entry in entries {
        engine.commit_battlefield_entry_state(entry, None).unwrap();
    }
    assert_eq!(
        engine.state.objects[&incoming_creature].counter_count(CounterKind::PlusOnePlusOne),
        1
    );
}

// Internal incarnation regression: normal commands cannot move an entrant during its prompt.
#[test]
fn metamorph_internal_entry_copy_rejects_actual_entrant_leave_return() {
    for (stale_entrant, decline) in [(true, false), (true, true), (false, false)] {
        let mut engine = engine();
        let source = object(&mut engine, "grizzly_bears", Zone::Battlefield, 1);
        let entrant = object(&mut engine, "phyrexian_metamorph", Zone::Hand, 0);
        engine.state.players[0].hand.push(entrant);
        engine.state.players[0].mana_pool.blue = 1;
        engine.state.players[0].mana_pool.colorless = 3;
        let slot = engine.state.players[0].hand.len() - 1;
        engine
            .apply_scenario_command(
                0,
                &rv1::RuledCommand {
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
        for actor in [0, 1] {
            engine
                .apply_scenario_command(
                    actor,
                    &rv1::RuledCommand {
                        cmd: Some(rv1::ruled_command::Cmd::PassPriority(rv1::PassPriority {})),
                    },
                )
                .unwrap();
        }
        assert_eq!(engine.state.objects[&entrant].zone, Zone::Stack);
        assert!(engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .presentation
            .candidates
            .contains(&source));
        let stale = if stale_entrant { entrant } else { source };
        let original_zone = if stale_entrant {
            Zone::Stack
        } else {
            Zone::Battlefield
        };
        for zone in [Zone::Graveyard, original_zone] {
            super::super::resolution::move_object_to_zone(
                &mut engine.state,
                engine.registry,
                stale,
                zone,
                None,
            )
            .unwrap();
        }
        let before = format!("{:?}", engine.state);
        let answer = rv1::RuledCommand {
            cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(
                rv1::SubmitResolutionChoice {
                    chosen_object_ids: if decline { vec![] } else { vec![source] },
                    ..Default::default()
                },
            )),
        };
        assert!(
            engine.apply_scenario_command(0, &answer).is_err(),
            "the old prompt cannot bind a new entrant incarnation"
        );
        assert_eq!(format!("{:?}", engine.state), before);
        assert!(engine.state.pending_resolution.is_some());
        assert!(engine.state.pending_replacement_event.is_some());
        assert!(engine.state.objects[&entrant].copiable_values.is_none());
        assert_eq!(engine.state.objects[&entrant].copy_revision, 0);
    }
}

fn object(engine: &mut GameEngine, card: &str, zone: Zone, controller: PlayerId) -> ObjectId {
    let oid = engine.state.next_object_id;
    engine.state.next_object_id += 1;
    let mut object = engine.state.objects.values().next().unwrap().clone();
    object.id = oid;
    object.card_id = card.into();
    object.zone = zone;
    object.controller = controller;
    object.base_controller = controller;
    object.owner = controller;
    object.face_up_index = 0;
    object.face_down = false;
    object.copiable_values = None;
    object.token_origin = None;
    object.token_faces = None;
    object.power = None;
    object.toughness = None;
    engine.state.objects.insert(oid, object);
    if zone == Zone::Battlefield {
        engine.state.players[controller as usize]
            .battlefield
            .push(oid);
    }
    oid
}

fn event(engine: &GameEngine, oid: ObjectId) -> BattlefieldEntryEvent {
    BattlefieldEntryEvent {
        phyrexian_symbols_paid_with_life: 0,
        entry_reveal_receipts: Vec::new(),
        mana_colors_spent_to_cast: Default::default(),
        prepared: false,
        object_id: oid,
        deciding_player: 0,
        destination_controller: 0,
        battle_protector: None,
        face_index: 0,
        unlock_room_door: None,
        chosen_x: 0,
        cast_by: None,
        cast_cost_receipts: Vec::new(),
        player_life_snapshot: engine.player_life_snapshot(),
        tapped: false,
        set_types: None,
        chosen_basic_land_type: None,
        chosen_opponents: Vec::new(),
        chosen_creature_types: Vec::new(),
        entry_counters: BTreeMap::new(),
        entry_counter_placements: Vec::new(),
        entry_counter_replacements_resolved: false,
        entry_modifiers: Vec::new(),
        entry_cause: None,
        attached_to: None,
        pending_copy_candidate: None,
        pending_aura_recipient: None,
        accepted_aura_recipient: None,
        applied_effects: Vec::new(),
    }
}

fn append_test_entry_counters(
    engine: &GameEngine,
    event: &mut BattlefieldEntryEvent,
    actor: PlayerId,
    counters: impl IntoIterator<Item = (CounterKind, u32)>,
) {
    let object_id = event.object_id;
    append_entry_counter_placement(
        event,
        actor,
        counters,
        EntryCounterSource::ResolutionEffect {
            stack_object_id: object_id,
            stack_zone_change_generation: engine
                .state
                .zone_change_generation
                .get(&object_id)
                .copied()
                .unwrap_or(0),
            effect_index: 0,
        },
        false,
    );
}

fn finish_entry_counter_replacements(engine: &mut GameEngine, event: &mut BattlefieldEntryEvent) {
    let references: [&BattlefieldEntryEvent; 1] = [&*event];
    let proposals = engine.proposed_entry_counter_placements(&references, &[]);
    let (counter_batch, choices) = engine.prepare_counter_placement_batch(proposals).unwrap();
    assert!(
        choices.is_none(),
        "fixture entry has no replacement-order choice"
    );
    GameEngine::apply_entry_counter_placement_results(event, &counter_batch.events).unwrap();
}

#[test]
fn molten_echoes_and_matching_creature_entering_together_wait_for_choice_then_trigger() {
    let mut engine = engine();
    let bear = object(&mut engine, "grizzly_bears", Zone::Graveyard, 0);
    let molten_echoes = object(&mut engine, "molten_echoes", Zone::Graveyard, 0);
    let mut events = Vec::new();

    // Process the matching creature first so it is already in the batch's ready cohort when
    // Molten Echoes parks its mandatory as-enters choice.
    let resumed = engine
        .begin_zone_entry_batch(
            ParkedStackResolution::dedicated(engine.observer_return_item(molten_echoes, 0)),
            vec![event(&engine, bear), event(&engine, molten_echoes)],
            Zone::Graveyard,
            "simultaneous return",
            None,
            &mut events,
        )
        .expect("the simultaneous entry batch is valid");
    assert!(resumed.is_none(), "the type choice parks the whole batch");
    engine.publish_resolution_prompt_ids(&mut events).unwrap();
    assert_eq!(engine.state.objects[&bear].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&molten_echoes].zone, Zone::Graveyard);
    let choice = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("Molten Echoes needs a creature-type choice");
    assert_eq!(choice.deciding_player, 0);
    assert_eq!(choice.presentation.source_object_id, molten_echoes);

    let bear_branch = tricerules_card_model::CREATURE_TYPES
        .iter()
        .position(|creature_type| *creature_type == "Bear")
        .expect("Bear is a canonical creature type") as u32;
    engine
        .apply_scenario_command(
            0,
            &rv1::RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(
                    rv1::SubmitResolutionChoice {
                        decision: rv1::ResolutionChoiceDecision::SelectBranch as i32,
                        selected_branch_index: bear_branch,
                        ..Default::default()
                    },
                )),
            },
        )
        .expect("choosing Bear resumes and commits the complete entry batch");

    // Both entrants have timestamp-sensitive characteristics, so resolving the type choice
    // correctly proceeds to the engine's timestamp-order choice before the batch commits.
    let timestamp_order = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("the complete entrant cohort requests timestamp order")
        .presentation
        .candidates
        .clone();
    assert_eq!(timestamp_order.len(), 2);
    engine
        .apply_scenario_command(
            0,
            &rv1::RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(
                    rv1::SubmitResolutionChoice {
                        chosen_object_ids: timestamp_order,
                        ..Default::default()
                    },
                )),
            },
        )
        .expect("ordering the simultaneous entrants commits the complete batch");

    assert_eq!(
        engine.state.objects[&bear].zone,
        Zone::Battlefield,
        "the batch should be committed after the type choice; pending resolution: {:?}",
        engine.state.pending_resolution,
    );
    assert_eq!(engine.state.objects[&molten_echoes].zone, Zone::Battlefield);
    let matching_triggers: Vec<_> = engine
        .state
        .stack
        .iter()
        .filter(|item| {
            item.triggered_ability
                .as_ref()
                .is_some_and(|ability| ability.ability_id.as_str() == "copy_matching_creature")
        })
        .collect();
    assert_eq!(matching_triggers.len(), 1);
    assert_eq!(
        matching_triggers[0]
            .trigger_context
            .observed_object
            .as_ref()
            .map(|object| object.object_id),
        Some(bear),
        "the simultaneous matching entrant remains the trigger's exact observed object"
    );

    for _ in 0..2 {
        let player = engine.state.priority_player_id();
        engine
            .apply_scenario_command(
                player,
                &rv1::RuledCommand {
                    cmd: Some(rv1::ruled_command::Cmd::PassPriority(rv1::PassPriority {})),
                },
            )
            .expect("players pass to resolve Molten Echoes' trigger");
    }
    let copies: Vec<_> = engine.state.players[0]
        .battlefield
        .iter()
        .copied()
        .filter(|object_id| {
            engine.state.objects[object_id]
                .token_origin
                .as_ref()
                .is_some_and(|identity| identity.source_card_id == "grizzly_bears")
        })
        .collect();
    assert_eq!(
        copies.len(),
        1,
        "the trigger creates one copy of the entrant"
    );
    assert!(engine.effective_has_keyword(copies[0], tricerules_cards::Keyword::Haste));
}

fn paid_divination_stack(engine: &mut GameEngine) -> ParkedStackResolution {
    let spell = object(engine, "divination", Zone::Hand, 0);
    engine.state.players[0].hand.push(spell);
    engine.state.players[0].mana_pool.blue = 1;
    engine.state.players[0].mana_pool.colorless = 2;
    let slot = engine.state.players[0].hand.len() - 1;
    engine
        .cast_spell(
            0,
            &rv1::CastSpell {
                cast_method: rv1::CastMethod::Normal as i32,
                source: Some(rv1::CastSource {
                    location: Some(rv1::cast_source::Location::HandIndex(slot as u32)),
                    ..Default::default()
                }),
                ..Default::default()
            },
        )
        .unwrap();
    let mut results = crate::state::OperationResults::default();
    results
        .append(crate::state::ProgramValue::PermanentSelection(vec![
            TriggerObjectRef {
                object_id: spell,
                zone_change_generation: engine.state.zone_change_generation[&spell],
                controller_at_event: 0,
            },
        ]))
        .expect("typed retention sentinel");
    ParkedStackResolution::for_flat_effects(engine.state.stack.pop().unwrap(), Some(0), results)
}

// Synthetic Library-entry instruction around a real paid outer spell and registered Pacifism.
#[test]
fn printed_aura_library_entry_parks_before_commit_and_preserves_original_tail() {
    for keyword in [None, Some(Keyword::Shroud), Some(Keyword::Hexproof)] {
        let mut engine = engine();
        let recipient = object(&mut engine, "grizzly_bears", Zone::Battlefield, 1);
        if let Some(keyword) = keyword {
            let mut values = engine.copiable_values_for(recipient).unwrap();
            values.face.keywords.push(keyword);
            engine
                .state
                .objects
                .get_mut(&recipient)
                .unwrap()
                .copiable_values = Some(values);
        }
        let protected = object(&mut engine, "grizzly_bears", Zone::Battlefield, 1);
        let mut values = engine.copiable_values_for(protected).unwrap();
        values
            .face
            .protections
            .push(ProtectionQuality::Color(Color::White));
        engine
            .state
            .objects
            .get_mut(&protected)
            .unwrap()
            .copiable_values = Some(values);
        let aura = object(&mut engine, "pacifism", Zone::Library, 0);
        engine.state.players[0].library.push_front(aura);
        let generation = engine
            .state
            .zone_change_generation
            .get(&aura)
            .copied()
            .unwrap_or(0);
        let stack = paid_divination_stack(&mut engine);
        let expected_previous = format!("{:?}", stack.previous_result());
        let hand_before_tail = engine.state.players[0].hand.len();
        let mut events = Vec::new();
        let entry = event(&engine, aura);
        assert!(
            engine
                .begin_zone_entry_batch(
                    stack.clone(),
                    vec![entry],
                    Zone::Library,
                    "internal printed Aura instruction",
                    None,
                    &mut events
                )
                .unwrap()
                .is_none(),
            "attachment must be chosen before entry commits"
        );
        engine.publish_resolution_prompt_ids(&mut events).unwrap();
        assert_eq!(engine.state.objects[&aura].zone, Zone::Library);
        assert_eq!(engine.state.players[0].library.front(), Some(&aura));
        assert_eq!(
            engine
                .state
                .zone_change_generation
                .get(&aura)
                .copied()
                .unwrap_or(0),
            generation
        );
        let pending = engine.state.pending_resolution.as_ref().unwrap();
        assert_eq!(
            pending.presentation.choice_kind,
            rv1::ChoiceKind::AuraPermanent
        );
        assert_eq!(pending.deciding_player, 0);
        assert_eq!(pending.presentation.candidates, [recipient]);
        assert_eq!(
            pending.continuation.stack().unwrap().resume_effect_index(),
            Some(0)
        );
        assert_eq!(
            format!(
                "{:?}",
                pending.continuation.stack().unwrap().previous_result()
            ),
            expected_previous
        );
        let answer = |recipient| rv1::RuledCommand {
            cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(
                rv1::SubmitResolutionChoice {
                    chosen_object_ids: vec![recipient],
                    ..Default::default()
                },
            )),
        };
        let before = format!("{:?}", engine.state);
        assert!(engine
            .apply_scenario_command(1, &answer(recipient))
            .is_err());
        assert!(engine
            .apply_scenario_command(0, &answer(protected))
            .is_err());
        assert_eq!(format!("{:?}", engine.state), before);
        let batch = engine
            .apply_scenario_command(0, &answer(recipient))
            .unwrap();
        assert_eq!(engine.state.objects[&aura].zone, Zone::Battlefield);
        assert_eq!(
            engine.state.objects[&aura].attached_to,
            Some(AttachmentRecipient::Object(recipient))
        );
        assert_eq!(engine.state.zone_change_generation[&aura], generation + 1);
        assert_eq!(
            engine.state.players[0].hand.len(),
            hand_before_tail + 2,
            "original tail runs once"
        );
        assert!(engine.state.pending_resolution.is_none());
        assert!(engine.state.pending_replacement_event.is_none());
        assert!(engine.state.stack.is_empty());
        assert_eq!(
            batch
                .events
                .iter()
                .filter(|event| matches!(&event.ev,
            Some(rv1::ruled_event::Ev::PermanentMoved(moved)) if moved.object_id == aura))
                .count(),
            1
        );
    }
}

#[test]
fn printed_aura_without_recipient_stays_in_library_and_resumes_tail_once() {
    for protected_recipient in [false, true] {
        let mut engine = engine();
        if protected_recipient {
            let recipient = object(&mut engine, "grizzly_bears", Zone::Battlefield, 1);
            let mut values = engine.copiable_values_for(recipient).unwrap();
            values
                .face
                .protections
                .push(ProtectionQuality::Color(Color::White));
            engine
                .state
                .objects
                .get_mut(&recipient)
                .unwrap()
                .copiable_values = Some(values);
        }
        let aura = object(&mut engine, "pacifism", Zone::Library, 0);
        engine.state.players[0].library.push_front(aura);
        let stack = paid_divination_stack(&mut engine);
        let hand_before_tail = engine.state.players[0].hand.len();
        let mut events = Vec::new();
        let mut entry = event(&engine, aura);
        append_test_entry_counters(&engine, &mut entry, 0, [(CounterKind::Stun, 2)]);
        let resumed = engine
            .begin_zone_entry_batch(
                stack,
                vec![entry],
                Zone::Library,
                "internal printed Aura instruction",
                None,
                &mut events,
            )
            .unwrap()
            .unwrap();
        assert_eq!(engine.state.objects[&aura].zone, Zone::Library);
        assert_eq!(engine.state.players[0].library.front(), Some(&aura));
        assert_eq!(
            engine.state.objects[&aura].counter_count(CounterKind::Stun),
            0
        );
        assert!(!events.iter().any(|event| matches!(&event.ev,
            Some(rv1::ruled_event::Ev::PermanentMoved(moved)) if moved.object_id == aura)));
        assert!(engine.state.pending_resolution.is_none());
        let (item, resume_effect_index, previous_result, _) = resumed.into_parts();
        engine
            .complete_parked_resolution_with_previous(
                item,
                resume_effect_index,
                previous_result,
                events,
            )
            .unwrap();
        assert_eq!(engine.state.players[0].hand.len(), hand_before_tail + 2);
        assert_eq!(
            engine.state.objects[&aura].zone,
            Zone::Hand,
            "only the later draw moves it"
        );
    }
}

#[test]
fn printed_aura_resumed_entry_keeps_deferred_tokens_out_of_library_views_and_rejects_stale_recipient(
) {
    for replacement_pause in [false, true] {
        for stale_recipient in [false, true] {
            let mut engine = engine();
            let recipient = object(&mut engine, "grizzly_bears", Zone::Battlefield, 1);
            if replacement_pause {
                object(&mut engine, "orb_of_dreams", Zone::Battlefield, 0);
                object(&mut engine, "orb_of_dreams", Zone::Battlefield, 1);
            }
            let aura = object(&mut engine, "pacifism", Zone::Library, 0);
            engine.state.players[0].library.push_front(aura);
            let token = object(&mut engine, "grizzly_bears", Zone::Library, 0);
            let values = engine.copiable_values_for(token).unwrap();
            engine.state.objects.get_mut(&token).unwrap().token_origin = Some(values);
            engine.state.players[0].library.push_front(token);
            let mut stack = paid_divination_stack(&mut engine);
            // This composition's outer spell has no remaining effects. Draw-tail coverage is above.
            stack.set_flat_tail(Some(1), stack.previous_result().clone());
            let expected_previous = format!("{:?}", stack.previous_result());
            let mut events = Vec::new();
            let entry = event(&engine, aura);
            assert!(engine
                .begin_zone_entry_batch(
                    stack,
                    vec![entry],
                    Zone::Library,
                    "internal printed Aura instruction",
                    None,
                    &mut events
                )
                .unwrap()
                .is_none());
            engine.publish_resolution_prompt_ids(&mut events).unwrap();
            let answer = |chosen| rv1::RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(
                    rv1::SubmitResolutionChoice {
                        chosen_object_ids: vec![chosen],
                        ..Default::default()
                    },
                )),
            };
            assert_eq!(
                engine
                    .state
                    .pending_resolution
                    .as_ref()
                    .unwrap()
                    .presentation
                    .choice_kind,
                if replacement_pause {
                    rv1::ChoiceKind::ReplacementEffect
                } else {
                    rv1::ChoiceKind::AuraPermanent
                }
            );
            for _ in 0..3 {
                let pending = engine.state.pending_resolution.as_ref().unwrap();
                if pending.presentation.choice_kind == rv1::ChoiceKind::AuraPermanent {
                    break;
                }
                assert_eq!(
                    pending.presentation.choice_kind,
                    rv1::ChoiceKind::ReplacementEffect
                );
                let chosen = pending.presentation.candidates[0];
                engine.apply_scenario_command(0, &answer(chosen)).unwrap();
            }
            let pending = engine.state.pending_resolution.as_ref().unwrap();
            assert_eq!(
                pending.presentation.choice_kind,
                rv1::ChoiceKind::AuraPermanent
            );
            assert_eq!(
                format!(
                    "{:?}",
                    pending.continuation.stack().unwrap().previous_result()
                ),
                expected_previous
            );
            assert_eq!(
                pending.continuation.stack().unwrap().resume_effect_index(),
                Some(1)
            );
            assert_eq!(engine.state.objects[&aura].zone, Zone::Library);
            assert_eq!(engine.state.players[0].library.front(), Some(&token));
            assert!(
                engine.state.objects[&token].is_token(),
                "normal SBA remains deferred"
            );
            let full = engine.ev_zone_view_sync();
            let Some(rv1::ruled_event::Ev::ZoneView(full)) = full.ev else {
                panic!("full view");
            };
            let owner = full
                .per_player
                .iter()
                .find(|player| player.player_id == 0)
                .unwrap();
            assert_eq!(
                owner.library_cards[0].object_id, aura,
                "first physical card skips token"
            );
            assert!(!owner
                .library_cards
                .iter()
                .any(|card| card.object_id == token));
            let tracked = engine.ev_zone_view_sync_tracked();
            let Some(rv1::ruled_event::Ev::ZoneView(tracked)) = tracked.ev else {
                panic!("tracked view");
            };
            assert!(
                tracked
                    .per_player
                    .iter()
                    .find(|player| player.player_id == 0)
                    .unwrap()
                    .private_zones_unchanged
            );
            if stale_recipient {
                for zone in [Zone::Hand, Zone::Battlefield] {
                    super::super::resolution::move_object_to_zone(
                        &mut engine.state,
                        engine.registry,
                        recipient,
                        zone,
                        Some(1),
                    )
                    .unwrap();
                }
                let before = format!("{:?}", engine.state);
                assert!(engine
                    .apply_scenario_command(0, &answer(recipient))
                    .is_err());
                assert_eq!(format!("{:?}", engine.state), before);
                assert_eq!(engine.state.objects[&aura].zone, Zone::Library);
            } else {
                engine
                    .apply_scenario_command(0, &answer(recipient))
                    .unwrap();
                assert_eq!(engine.state.objects[&aura].zone, Zone::Battlefield);
                assert_eq!(
                    engine.state.objects[&aura].attached_to,
                    Some(AttachmentRecipient::Object(recipient))
                );
                assert_eq!(engine.state.objects[&aura].tapped, replacement_pause);
                assert!(
                    !engine.state.objects.contains_key(&token),
                    "SBA runs at completed resolution"
                );
                assert!(engine.state.pending_resolution.is_none());
                assert!(engine.state.pending_replacement_event.is_none());
            }
        }
    }
}

#[test]
fn black_vise_internal_entry_without_eligible_opponents_completes_undefined() {
    let mut engine = engine();
    let source = object(&mut engine, "black_vise", Zone::Hand, 0);
    engine.state.players[0].hand.push(source);
    engine.state.players[0].mana_pool.colorless = 1;
    let slot = engine.state.players[0].hand.len() - 1;
    engine
        .apply_scenario_command(
            0,
            &rv1::RuledCommand {
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
    let item = engine.state.stack.pop().unwrap();
    // Intermediate resolution fixture: the departure is known before final outcome publication.
    engine.state.players[1].has_lost = true;
    let mut events = Vec::new();
    let progress = engine.advance_or_park_battlefield_entry(
        item,
        event(&engine, source),
        BattlefieldEntryCompletion::PermanentSpell { attached_to: None },
        &mut events,
    );
    let BattlefieldEntryProgress::Ready(entry) = progress else {
        panic!("impossible choice must not park");
    };
    engine.commit_battlefield_entry_state(*entry, None).unwrap();
    assert_eq!(engine.state.objects[&source].zone, Zone::Battlefield);
    assert!(engine.state.chosen_opponents.is_empty());
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.chosen_opponent_labels(source).is_empty());
}

// No registered simultaneous hand-entry producer exists. This invokes the production batch
// pipeline with registered cards and labels its synthetic instruction honestly.
#[test]
fn entry_reveal_pair_internal_hand_cohort_retains_paid_reveals_through_timestamp_order() {
    for forest_first in [false, true] {
        for stale_cohort in [false, true] {
            let mut engine = engine();
            let forest = object(&mut engine, "forest", Zone::Hand, 0);
            let first = object(&mut engine, "game_trail", Zone::Hand, 0);
            let second = object(&mut engine, "game_trail", Zone::Hand, 0);
            engine.state.players[0].hand.extend([forest, first, second]);
            let outside = object(&mut engine, "forest", Zone::Hand, 0);
            engine.state.players[0].hand.push(outside);
            let hand_index = engine.state.players[0]
                .hand
                .iter()
                .position(|oid| *oid == first)
                .unwrap();
            engine
                .apply_scenario_command(
                    0,
                    &rv1::RuledCommand {
                        cmd: Some(rv1::ruled_command::Cmd::PlayLand(rv1::PlayLand {
                            resolution_prompt_id: 0,
                            source: Some(rv1::LandSource {
                                location: Some(rv1::land_source::Location::HandIndex(
                                    hand_index as u32,
                                )),
                                ..Default::default()
                            }),
                            face_index: 0,
                        })),
                    },
                )
                .unwrap();
            let item = engine
                .state
                .pending_resolution
                .as_ref()
                .unwrap()
                .continuation
                .stack()
                .unwrap()
                .item
                .clone();
            engine.state.pending_resolution = None;
            engine.state.pending_replacement_event = None;
            let ids = if forest_first {
                [forest, first, second]
            } else {
                [first, second, forest]
            };
            let entries = ids.iter().map(|oid| event(&engine, *oid)).collect();
            let mut events = Vec::new();
            assert!(engine
                .begin_zone_entry_batch(
                    ParkedStackResolution::dedicated(item),
                    entries,
                    Zone::Hand,
                    "internal simultaneous hand instruction",
                    None,
                    &mut events
                )
                .unwrap()
                .is_none());
            engine.publish_resolution_prompt_ids(&mut events).unwrap();
            let choose = |ids| rv1::RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(
                    rv1::SubmitResolutionChoice {
                        chosen_object_ids: ids,
                        ..Default::default()
                    },
                )),
            };
            let first_batch = engine
                .apply_scenario_command(0, &choose(vec![forest]))
                .unwrap();
            assert_eq!(engine.state.objects[&first].zone, Zone::Hand);
            assert_eq!(
                engine.state.objects[&forest].zone,
                Zone::Hand,
                "even an already-ready matching land remains revealable"
            );
            let active = super::super::reveals::active_reveals(&engine);
            assert_eq!(
                active.len(),
                1,
                "the first accepted reveal remains public while the second entry parks"
            );
            assert_eq!(active[0].cards[0].object_id, forest);
            assert_eq!(active[0].source_object_id, first);
            assert_eq!(
                first_batch
                    .events
                    .iter()
                    .filter(|event| matches!(
                        event.ev,
                        Some(rv1::ruled_event::Ev::CardsRevealed(_))
                    ))
                    .count(),
                1
            );
            let first_id = active[0].reveal_id.clone();
            if stale_cohort {
                // Forest is already ready or still remaining, while current entrant and
                // the independently selected outside card both retain their incarnations.
                super::super::resolution::move_object_to_zone(
                    &mut engine.state,
                    engine.registry,
                    forest,
                    Zone::Graveyard,
                    None,
                )
                .unwrap();
                super::super::resolution::move_object_to_zone(
                    &mut engine.state,
                    engine.registry,
                    forest,
                    Zone::Hand,
                    None,
                )
                .unwrap();
                let before = format!("{:?}", engine.state);
                assert!(engine
                    .apply_scenario_command(0, &choose(vec![outside]))
                    .is_err());
                assert_eq!(format!("{:?}", engine.state), before);
                let paid = super::super::reveals::active_reveals(&engine);
                assert_eq!(paid.len(), 1);
                assert_eq!(
                    paid[0], active[0],
                    "accepted receipt remains the frozen original incarnation"
                );
                for oid in ids {
                    assert_eq!(engine.state.objects[&oid].zone, Zone::Hand);
                }
                continue;
            }
            let second_batch = engine
                .apply_scenario_command(0, &choose(vec![forest]))
                .unwrap();
            let active = super::super::reveals::active_reveals(&engine);
            assert_eq!(
                active.len(),
                2,
                "both paid snapshots survive the timestamp-order choice"
            );
            assert!(active.iter().any(|reveal| reveal.reveal_id == first_id));
            assert_ne!(active[0].reveal_id, active[1].reveal_id);
            assert_eq!(
                second_batch
                    .events
                    .iter()
                    .filter(|event| matches!(
                        event.ev,
                        Some(rv1::ruled_event::Ev::CardsRevealed(_))
                    ))
                    .count(),
                1
            );
            for oid in ids {
                assert_eq!(engine.state.objects[&oid].zone, Zone::Hand);
            }
            let pending = engine.state.pending_resolution.as_ref().unwrap();
            assert!(matches!(
                pending.continuation,
                ResolutionContinuation::SimultaneousEntryOrder { .. }
            ));
            let order = pending.presentation.candidates.clone();
            engine.apply_scenario_command(0, &choose(order)).unwrap();
            for oid in ids {
                assert_eq!(engine.state.objects[&oid].zone, Zone::Battlefield);
                assert!(!engine.state.objects[&oid].tapped);
            }
            assert!(super::super::reveals::active_reveals(&engine).is_empty());
        }
    }
}

// Structural continuation fixture: no registered mixed aura/land-token producer is claimed.
#[test]
fn entry_reveal_pair_internal_token_attachment_retains_ready_and_remaining_receipts() {
    let mut engine = engine();
    let first = object(&mut engine, "game_trail", Zone::Stack, 0);
    let second = object(&mut engine, "murmuring_bosk", Zone::Stack, 0);
    let third = object(&mut engine, "game_trail", Zone::Stack, 0);
    let receipt = |oid| rv1::CardsRevealed {
        reveal_id: format!("entry:{oid}:0:fixture"),
        source_object_id: oid,
        source_zone: rv1::ChoiceCandidateSourceZone::Hand as i32,
        source_description: "structural entry fixture".into(),
        cards: vec![rv1::RevealedCard {
            object_id: 999,
            zone_change_generation: 7,
            card_id: "forest".into(),
            card_name: "Forest".into(),
        }],
        ..Default::default()
    };
    let mut first_event = event(&engine, first);
    first_event.entry_reveal_receipts.push(receipt(first));
    let mut second_event = event(&engine, second);
    second_event.entry_reveal_receipts.push(receipt(second));
    let mut third_event = event(&engine, third);
    third_event.entry_reveal_receipts.push(receipt(third));
    engine.state.pending_replacement_event = Some(PendingReplacementEvent::BattlefieldEntry(
        Box::new(PendingBattlefieldEntry {
            event: first_event,
            applications: vec![],
            copy_source_effect: None,
            copy_source_candidates: vec![],
            completion: BattlefieldEntryCompletion::TokenBatch(Box::new(
                crate::state::PendingTokenEntryBatch {
                    current_created: Default::default(),
                    result_object_ids: vec![first, second, third],
                    ready: vec![crate::state::TokenBattlefieldEntry {
                        event: second_event.clone(),
                        created: Default::default(),
                    }],
                    remaining: vec![
                        crate::state::TokenBattlefieldEntry {
                            event: third_event,
                            created: Default::default(),
                        },
                        crate::state::TokenBattlefieldEntry {
                            event: second_event,
                            created: Default::default(),
                        },
                    ],
                    logs: vec![],
                    options: Default::default(),
                },
            )),
        }),
    ));
    let active = super::super::reveals::active_reveals(&engine);
    assert_eq!(
        active,
        vec![receipt(first), receipt(second), receipt(third)]
    );
    let mut batch = RuledEventBatch::default();
    batch.events.push(rv1::RuledEvent {
        ev: Some(rv1::ruled_event::Ev::ActivePublicRevealSnapshot(
            rv1::ActivePublicRevealSnapshot {
                reveals: active.clone(),
            },
        )),
    });
    super::super::legal_actions::fill_legal(&mut batch, &engine);
    assert!(
        !batch
            .events
            .iter()
            .any(|event| matches!(event.ev, Some(rv1::ruled_event::Ev::CardsRevealed(_)))),
        "an unchanged active receipt must not become a second occurrence"
    );

    // The same frozen receipts must survive the observer-return staging and timestamp owners.
    engine.state.pending_replacement_event = None;
    let mut observer_event = event(&engine, first);
    observer_event.entry_reveal_receipts.push(receipt(first));
    let observer = crate::state::PendingObserverReturnBatch {
        ready: vec![crate::state::ObserverReturnEntry {
            event: observer_event,
            owner: 0,
            label: "structural observer fixture".into(),
            attached_to: None,
        }],
        remaining: Default::default(),
        resume_stack: None,
        suspended_resolution: None,
        suspended_replacement_event: None,
    };
    engine.state.pending_observer_return_batch = Some(observer.clone());
    assert_eq!(
        super::super::reveals::active_reveals(&engine),
        vec![receipt(first)]
    );
    engine.state.pending_observer_return_batch = None;
    engine.state.pending_resolution = Some(PendingResolution {
        resolution_prompt_id: crate::state::ResolutionPromptId::UNASSIGNED,
        resolution_prompt_semantics: None,

        deciding_player: 0,
        presentation: PendingResolutionPresentation {
            source_object_id: first,
            candidates: vec![first],
            min: 1,
            max: 1,
            ordered: true,
            prompt: "structural timestamp fixture".into(),
            choice_kind: rv1::ChoiceKind::ReplacementEffect,
            unique_names: false,
        },
        continuation: ResolutionContinuation::SimultaneousEntryOrder {
            stack: None,
            order: Box::new(crate::state::PendingEntryTimestampOrder {
                batch: crate::state::SimultaneousEntryBatch::Observer(Box::new(observer)),
                original_generations: vec![],
                remaining_groups: Default::default(),
                chosen_order: vec![],
            }),
        },
    });
    assert_eq!(
        super::super::reveals::active_reveals(&engine),
        vec![receipt(first)]
    );
}

#[test]
fn entry_reveal_pair_internal_actual_leave_return_rejects_candidate_and_entrant_incarnations() {
    for stale_entrant in [false, true] {
        let mut engine = engine();
        let forest = object(&mut engine, "forest", Zone::Hand, 0);
        let source = object(&mut engine, "game_trail", Zone::Hand, 0);
        engine.state.players[0].hand.extend([forest, source]);
        let slot = engine.state.players[0]
            .hand
            .iter()
            .position(|oid| *oid == source)
            .unwrap();
        engine
            .apply_scenario_command(
                0,
                &rv1::RuledCommand {
                    cmd: Some(rv1::ruled_command::Cmd::PlayLand(rv1::PlayLand {
                        resolution_prompt_id: 0,
                        source: Some(rv1::LandSource {
                            location: Some(rv1::land_source::Location::HandIndex(slot as u32)),
                            ..Default::default()
                        }),
                        face_index: 0,
                    })),
                },
            )
            .unwrap();
        let stale = if stale_entrant { source } else { forest };
        super::super::resolution::move_object_to_zone(
            &mut engine.state,
            engine.registry,
            stale,
            Zone::Graveyard,
            None,
        )
        .unwrap();
        super::super::resolution::move_object_to_zone(
            &mut engine.state,
            engine.registry,
            stale,
            Zone::Hand,
            None,
        )
        .unwrap();
        let before = format!("{:?}", engine.state);
        let answer = rv1::RuledCommand {
            cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(
                rv1::SubmitResolutionChoice {
                    chosen_object_ids: vec![forest],
                    ..Default::default()
                },
            )),
        };
        assert!(engine.apply_scenario_command(0, &answer).is_err());
        assert_eq!(format!("{:?}", engine.state), before);
        assert!(
            engine.state.pending_resolution.is_some()
                && engine.state.pending_replacement_event.is_some()
        );
        assert!(super::super::reveals::active_reveals(&engine).is_empty());
    }
}

fn effect(engine: &mut GameEngine, affected: AffectedScope, kind: ContinuousEffectKind) {
    engine.state.continuous_effects.push(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: None,
        affected,
        kind,
        condition: None,
        duration: EffectDuration::Indefinite,
        timestamp: 1,
    });
}

fn land_scope(controller: tricerules_card_model::primitives::TargetController) -> AffectedScope {
    AffectedScope::PermanentsMatching {
        reference_player: 1,
        exclude: None,
        filter: Box::new(tricerules_card_model::primitives::TargetFilter {
            kind: tricerules_card_model::primitives::TargetKind::AnyPermanent,
            controller,
            permanent_types: vec![PermanentTypeFilter::Land],
            ..Default::default()
        }),
    }
}

fn land() -> tricerules_card_model::TypeLineReplacement {
    tricerules_card_model::TypeLineReplacement {
        card_types: vec![PermanentTypeFilter::Land],
        creature_types: Vec::new(),
        land_types: Vec::new(),
    }
}

#[test]
fn entry_early_layers_land_making_precedes_existing_land_scope_in_both_forms() {
    for replace in [false, true] {
        let mut engine = engine();
        let oid = object(&mut engine, "hill_giant", Zone::Stack, 0);
        effect(
            &mut engine,
            land_scope(tricerules_card_model::primitives::TargetController::Any),
            ContinuousEffectKind::Layer4AddTypes(tricerules_card_model::TypeLineAddition {
                land_types: vec![BasicLandType::Forest],
                ..Default::default()
            }),
        );
        let mut entry = event(&engine, oid);
        entry.entry_modifiers = vec![if replace {
            ResolvingPermanentModifier::SetTypeLine(land())
        } else {
            ResolvingPermanentModifier::AddTypes(tricerules_card_model::TypeLineAddition {
                card_types: vec![PermanentTypeFilter::Land],
                ..Default::default()
            })
        }];
        let preview = engine
            .battlefield_entry_characteristics_through_layer_5(&entry)
            .unwrap();
        assert!(
            preview.has_type("Forest"),
            "new land scope must see the projected entrant"
        );
        engine
            .commit_battlefield_entry(entry, None, &mut Vec::new())
            .unwrap();
        assert_eq!(
            engine.characteristics_through_layer_5(oid).unwrap(),
            preview
        );
    }
}

#[test]
fn entry_early_layers_destination_controller_is_visible_to_land_scope() {
    let mut engine = engine();
    let oid = object(&mut engine, "forest", Zone::Stack, 0);
    effect(
        &mut engine,
        land_scope(tricerules_card_model::primitives::TargetController::You),
        ContinuousEffectKind::Layer4AddTypes(tricerules_card_model::TypeLineAddition {
            card_types: vec![PermanentTypeFilter::Artifact],
            ..Default::default()
        }),
    );
    let mut entry = event(&engine, oid);
    entry.destination_controller = 1;
    let preview = engine
        .battlefield_entry_characteristics_through_layer_5(&entry)
        .unwrap();
    assert!(preview.has_type("Artifact"));
    engine
        .commit_battlefield_entry(entry, None, &mut Vec::new())
        .unwrap();
    assert_eq!(
        engine.characteristics_through_layer_5(oid).unwrap(),
        preview
    );
}

#[test]
fn entry_early_layers_face_down_values_precede_type_and_color_effects() {
    let mut engine = engine();
    let oid = object(&mut engine, "hill_giant", Zone::Stack, 0);
    engine.state.objects.get_mut(&oid).unwrap().face_down = true;
    effect(
        &mut engine,
        land_scope(tricerules_card_model::primitives::TargetController::Any),
        ContinuousEffectKind::Layer5SetColors(vec![Color::Blue]),
    );
    let mut entry = event(&engine, oid);
    entry.set_types = Some(land());
    let preview = engine
        .battlefield_entry_characteristics_through_layer_5(&entry)
        .unwrap();
    assert!(preview.names.is_empty());
    assert_eq!(preview.colors, [Color::Blue]);
    assert_eq!(preview.power, Some(2));
    engine
        .commit_battlefield_entry(entry, None, &mut Vec::new())
        .unwrap();
    assert_eq!(
        engine.characteristics_through_layer_5(oid).unwrap(),
        preview
    );
}

#[test]
fn entry_early_layers_old_single_effect_is_not_carried_to_new_incarnation() {
    let mut engine = engine();
    let oid = object(&mut engine, "forest", Zone::Graveyard, 0);
    effect(
        &mut engine,
        AffectedScope::Single(oid),
        ContinuousEffectKind::Layer5SetColors(vec![Color::Red]),
    );
    let entry = event(&engine, oid);
    let preview = engine
        .battlefield_entry_characteristics_through_layer_5(&entry)
        .unwrap();
    assert!(preview.colors.is_empty());
    engine
        .commit_battlefield_entry(entry, None, &mut Vec::new())
        .unwrap();
    assert_eq!(
        engine.characteristics_through_layer_5(oid).unwrap(),
        preview
    );
}

#[test]
fn entry_early_layers_selected_face_and_copied_own_static_are_used() {
    let mut engine = engine();
    let oid = object(
        &mut engine,
        "cragcrown_pathway_timbercrown_pathway",
        Zone::Hand,
        0,
    );
    let mut entry = event(&engine, oid);
    entry.face_index = 1;
    assert_eq!(
        engine
            .battlefield_entry_characteristics_through_layer_5(&entry)
            .unwrap()
            .names,
        ["Timbercrown Pathway"]
    );
    let source = r#"(id: "own_entry_land", name: "Own Entry Land", face_id: "own_entry_land", types: ["Land"],
        static_abilities: [(ability_id: "static_01", presentation: Fallback,
            definition: ConditionalSelfModifier(condition: ActivePlayer(players: Controller),
                add_types: (card_types: [Artifact])))])"#;
    let face = CardRegistry::from_chunks_and_tokens(&[source], &[])
        .unwrap()
        .get("own_entry_land")
        .unwrap()
        .primary_face()
        .clone();
    engine.state.objects.get_mut(&oid).unwrap().copiable_values = Some(CopiableValues {
        prototyped: false,
        source_card_id: "own_entry_land".into(),
        source_face_index: 0,
        display_name: face.name.clone(),
        face,
        room_faces: None,
    });
    let mut entry = event(&engine, oid);
    assert!(engine
        .battlefield_entry_characteristics_through_layer_5(&entry)
        .unwrap()
        .has_type("Artifact"));
    // CR 613.7n: entrant-owned statics precede same-timestamp resolving entry effects.
    entry.set_types = Some(land());
    let preview = engine
        .battlefield_entry_characteristics_through_layer_5(&entry)
        .unwrap();
    assert!(!preview.has_type("Artifact"));
    engine
        .commit_battlefield_entry(entry, None, &mut Vec::new())
        .unwrap();
    assert_eq!(
        engine.characteristics_through_layer_5(oid).unwrap(),
        preview
    );
}

#[test]
fn entry_early_layers_chosen_basic_and_type_override_have_commit_order() {
    let mut engine = engine();
    let oid = object(&mut engine, "forest", Zone::Stack, 0);
    let mut entry = event(&engine, oid);
    entry.chosen_basic_land_type = Some(BasicLandType::Island);
    entry.set_types = Some(land());
    let preview = engine
        .battlefield_entry_characteristics_through_layer_5(&entry)
        .unwrap();
    engine
        .commit_battlefield_entry(entry, None, &mut Vec::new())
        .unwrap();
    assert_eq!(
        engine.characteristics_through_layer_5(oid).unwrap(),
        preview
    );
}

#[test]
fn entry_early_layers_provisional_entrant_does_not_satisfy_battlefield_count() {
    let mut engine = engine();
    let oid = object(&mut engine, "forest", Zone::Stack, 0);
    let source = r#"(id: "count_entry_land", name: "Count Entry Land", face_id: "count_entry_land", types: ["Land"],
        static_abilities: [(ability_id: "static_01", presentation: Fallback,
            definition: ConditionalSelfModifier(condition: BattlefieldAggregate(
                filter: (controllers: All, card_type: Some(Land)), aggregate: Count, min: Some(1)),
                add_types: (card_types: [Artifact])))])"#;
    let face = CardRegistry::from_chunks_and_tokens(&[source], &[])
        .unwrap()
        .get("count_entry_land")
        .unwrap()
        .primary_face()
        .clone();
    engine.state.objects.get_mut(&oid).unwrap().copiable_values = Some(CopiableValues {
        prototyped: false,
        source_card_id: "count_entry_land".into(),
        source_face_index: 0,
        display_name: face.name.clone(),
        face,
        room_faces: None,
    });
    let entry = event(&engine, oid);
    assert!(!engine
        .battlefield_entry_characteristics_through_layer_5(&entry)
        .unwrap()
        .has_type("Artifact"));
    let existing = object(&mut engine, "forest", Zone::Battlefield, 1);
    assert!(engine
        .battlefield_entry_characteristics_through_layer_5(&entry)
        .unwrap()
        .has_type("Artifact"));
    engine.state.players[1]
        .battlefield
        .retain(|&candidate| candidate != existing);
    engine.state.objects.get_mut(&existing).unwrap().zone = Zone::Graveyard;
    assert!(!engine
        .battlefield_entry_characteristics_through_layer_5(&entry)
        .unwrap()
        .has_type("Artifact"));
}

#[test]
fn entry_early_layers_type_setting_suppresses_intrinsic_entry_replacements() {
    let mut engine = engine();
    let oid = object(&mut engine, "fetid_pools", Zone::Stack, 0);
    let mut entry = event(&engine, oid);
    assert!(!engine.battlefield_entry_candidates(&entry).is_empty());
    let mut types = land();
    types.land_types = vec![BasicLandType::Forest];
    entry.set_types = Some(types);
    assert!(
        engine.battlefield_entry_candidates(&entry).is_empty(),
        "the resulting Forest has no printed enters-tapped replacement"
    );
    entry.set_types = None;
    assert!(!engine.battlefield_entry_candidates(&entry).is_empty());
}

fn copy_face(engine: &mut GameEngine, oid: ObjectId, source: &str, card: &str) {
    let face = CardRegistry::from_chunks_and_tokens(&[source], &[])
        .unwrap()
        .get(card)
        .unwrap()
        .primary_face()
        .clone();
    engine.state.objects.get_mut(&oid).unwrap().copiable_values = Some(CopiableValues {
        prototyped: false,
        source_card_id: card.into(),
        source_face_index: 0,
        display_name: face.name.clone(),
        face,
        room_faces: None,
    });
}

#[test]
fn entry_early_layers_own_global_static_is_restricted_to_entrant_during_preview() {
    let mut engine = engine();
    let existing = object(&mut engine, "forest", Zone::Battlefield, 1);
    let oid = object(&mut engine, "forest", Zone::Stack, 0);
    let source = r#"(id: "global_entry_land", name: "Global Entry Land", face_id: "global_entry_land", types: ["Land"],
        static_abilities: [
            (ability_id: "static_01", presentation: Fallback, definition: AddTypesToPermanents(
                filter: (kind: AnyPermanent, permanent_types: [Land]), addition: (card_types: [Artifact]))),
            (ability_id: "static_02", presentation: Fallback, definition: ConditionalSelfModifier(
                condition: BattlefieldAggregate(filter: (controllers: All, card_type: Some(Artifact)),
                    aggregate: Count, min: Some(1)), add_types: (card_types: [Enchantment])))])"#;
    copy_face(&mut engine, oid, source, "global_entry_land");
    let entry = event(&engine, oid);
    let preview = engine
        .battlefield_entry_characteristics_through_layer_5(&entry)
        .unwrap();
    assert!(preview.has_type("Artifact"));
    assert!(
        !preview.has_type("Enchantment"),
        "own global statics cannot alter existing permanents during entry preview"
    );
    assert!(!engine
        .characteristics(existing)
        .unwrap()
        .has_type("Artifact"));
    engine
        .commit_battlefield_entry(entry, None, &mut Vec::new())
        .unwrap();
    assert!(engine
        .characteristics(existing)
        .unwrap()
        .has_type("Artifact"));
    assert!(engine.characteristics(oid).unwrap().has_type("Enchantment"));
}

#[test]
fn entry_early_layers_own_conditions_use_projected_counters_and_tapped_status() {
    let mut engine = engine();
    let oid = object(&mut engine, "forest", Zone::Stack, 0);
    let source = r#"(id: "condition_entry_land", name: "Condition Entry Land", face_id: "condition_entry_land", types: ["Land"],
        static_abilities: [(ability_id: "static_01", presentation: Fallback,
            definition: ConditionalSelfModifier(condition: AllOf([
                SourceCounterCount(counter: Charge, min: Some(2)), ObjectTapped(object: Source, tapped: true)]),
                add_types: (card_types: [Artifact])))])"#;
    copy_face(&mut engine, oid, source, "condition_entry_land");
    let mut entry = event(&engine, oid);
    entry.tapped = true;
    append_test_entry_counters(&engine, &mut entry, 0, [(CounterKind::Charge, 2)]);
    let preview = engine
        .battlefield_entry_characteristics_through_layer_5(&entry)
        .unwrap();
    assert!(preview.has_type("Artifact"));
    finish_entry_counter_replacements(&mut engine, &mut entry);
    engine
        .commit_battlefield_entry(entry, None, &mut Vec::new())
        .unwrap();
    assert_eq!(
        engine.characteristics_through_layer_5(oid).unwrap(),
        preview
    );
}

#[test]
fn entry_early_layers_room_door_statics_and_locked_copy_match_commitment() {
    let mut engine = engine();
    let source = r#"(id: "first_room_second_room", name: "First Room // Second Room", layout: Room, faces: [
        (name: "First Room", face_id: "first_room", mana_cost: "{1}{U}", types: ["Enchantment", "Room"],
            static_abilities: [(ability_id: "static_01", presentation: Fallback, definition: ConditionalSelfModifier(
                condition: ActivePlayer(players: Controller), add_types: (card_types: [Artifact])))]),
        (name: "Second Room", face_id: "second_room", mana_cost: "{2}{R}", types: ["Enchantment", "Room"])])"#;
    engine.registry = Box::leak(Box::new(
        CardRegistry::from_chunks_and_tokens(&[source], &[]).unwrap(),
    ));
    let oid = object(&mut engine, "first_room_second_room", Zone::Stack, 0);
    let mut entry = event(&engine, oid);
    entry.unlock_room_door = Some(0);
    let raw = engine
        .battlefield_entry_characteristics_through_layer_5(&entry)
        .unwrap();
    assert_eq!(raw.names, ["First Room"]);
    assert_eq!(raw.colors, [Color::Blue]);
    assert_eq!(raw.mana_value, 2);
    assert!(raw.has_type("Artifact"));
    entry.set_types = Some(land());
    let preview = engine
        .battlefield_entry_characteristics_through_layer_5(&entry)
        .unwrap();
    assert!(!preview.has_type("Artifact"));
    engine
        .commit_battlefield_entry(entry, None, &mut Vec::new())
        .unwrap();
    assert_eq!(
        engine.characteristics_through_layer_5(oid).unwrap(),
        preview
    );
    let own = engine
        .state
        .continuous_effects
        .iter()
        .filter(|effect| effect.source_id == Some(oid) && effect.trigger_grant_origin.is_some())
        .collect::<Vec<_>>();
    assert_eq!(own.len(), 1);
    let Some(TriggerAbilityOrigin::StaticGrant {
        source_zone_change,
        definition,
        ..
    }) = &own[0].trigger_grant_origin
    else {
        panic!("door provenance");
    };
    assert_eq!(
        *source_zone_change,
        engine.state.zone_change_generation[&oid]
    );
    assert_eq!(definition.face_id.as_str(), "first_room");
    let copy = object(&mut engine, "first_room_second_room", Zone::Stack, 0);
    let card = engine.registry.get("first_room_second_room").unwrap();
    engine.state.objects.get_mut(&copy).unwrap().copiable_values = Some(CopiableValues {
        prototyped: false,
        source_card_id: card.id.clone(),
        source_face_index: 0,
        display_name: card.name.clone(),
        face: card.primary_face().clone(),
        room_faces: Some(card.faces.clone()),
    });
    let mut entry = event(&engine, copy);
    entry.unlock_room_door = Some(0);
    let preview = engine
        .battlefield_entry_characteristics_through_layer_5(&entry)
        .unwrap();
    assert!(preview.names.is_empty() && preview.colors.is_empty());
    assert_eq!(preview.mana_value, 0);
    assert!(!preview.has_type("Artifact"));
    engine
        .commit_battlefield_entry(entry, None, &mut Vec::new())
        .unwrap();
    assert_eq!(
        engine.characteristics_through_layer_5(copy).unwrap(),
        preview
    );
    assert!(!engine
        .state
        .continuous_effects
        .iter()
        .any(|effect| effect.source_id == Some(copy) && effect.trigger_grant_origin.is_some()));
}

#[test]
fn entry_early_layers_kaito_uses_entry_loyalty_before_commitment() {
    let mut engine = engine();
    let oid = object(&mut engine, "kaito,_bane_of_nightmares", Zone::Stack, 0);
    assert_eq!(
        engine.state.objects[&oid].counter_count(CounterKind::Loyalty),
        0
    );
    let mut entry = event(&engine, oid);
    append_test_entry_counters(&engine, &mut entry, 0, [(CounterKind::Loyalty, 4)]);
    let preview = engine
        .battlefield_entry_characteristics_through_layer_5(&entry)
        .unwrap();
    assert!(preview.is_creature() && preview.has_type("Ninja"));
    assert!(!preview.has_type("Planeswalker"));
    finish_entry_counter_replacements(&mut engine, &mut entry);
    engine
        .commit_battlefield_entry(entry, None, &mut Vec::new())
        .unwrap();
    assert_eq!(
        engine.characteristics_through_layer_5(oid).unwrap(),
        preview
    );
}

#[test]
fn compleated_history_paid_cast_survives_entry_copy_pause_and_rejects_stale_answers() {
    let (mut engine, black_vise, metamorph) = compleated_history_metamorph_copy_pending(true);
    let first_pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("Metamorph parks for its As Enters copy choice");
    assert_eq!(
        first_pending.presentation.choice_kind,
        rv1::ChoiceKind::CopySource
    );
    assert!(first_pending.presentation.candidates.contains(&black_vise));
    let first_prompt_id = first_pending.resolution_prompt_id.get();
    let copy_answer = rv1::SubmitResolutionChoice {
        resolution_prompt_id: first_prompt_id,
        chosen_object_ids: vec![black_vise],
        ..Default::default()
    };
    let first_snapshot = engine
        .diagnostic_snapshot()
        .expect("complete first-pause diagnostic snapshot");
    assert!(engine.submit_resolution_choice(1, &copy_answer).is_err());
    assert_eq!(
        engine
            .diagnostic_snapshot()
            .expect("snapshot after wrong-actor copy answer"),
        first_snapshot
    );
    let invalid_copy_answer = rv1::SubmitResolutionChoice {
        resolution_prompt_id: first_prompt_id,
        chosen_object_ids: vec![u32::MAX],
        ..Default::default()
    };
    assert!(engine
        .submit_resolution_choice(0, &invalid_copy_answer)
        .is_err());
    assert_eq!(
        engine
            .diagnostic_snapshot()
            .expect("snapshot after unoffered copy answer"),
        first_snapshot
    );
    let first_resume = engine
        .submit_resolution_choice(0, &copy_answer)
        .expect("choose the engine-offered Black Vise copy source");
    assert!(first_resume.events.iter().any(|event| matches!(
        event.ev.as_ref(),
        Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(_))
    )));

    let second_pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("copied Black Vise parks for its As Enters opponent choice");
    assert_ne!(second_pending.resolution_prompt_id.get(), first_prompt_id);
    assert_eq!(
        second_pending.presentation.choice_kind,
        rv1::ChoiceKind::ResolutionBranch
    );
    assert_eq!(second_pending.deciding_player, 0);
    let second_prompt_id = second_pending.resolution_prompt_id.get();
    let second_parked_stack_value = crate::diagnostic_json::to_value(
        &second_pending
            .continuation
            .stack()
            .expect("copied opponent choice retains the resolving Metamorph stack item")
            .item,
    )
    .expect("serialize copied-opponent parked stack item");
    assert_eq!(
        second_parked_stack_value["phyrexian_symbols_paid_with_life"].as_u64(),
        Some(1)
    );
    let pending_entry_value = match engine.state.pending_replacement_event.as_ref() {
        Some(PendingReplacementEvent::BattlefieldEntry(entry)) => {
            crate::diagnostic_json::to_value(&entry.event)
                .expect("serialize copied paused battlefield entry")
        }
        _ => panic!("expected the copied Black Vise battlefield entry to remain parked"),
    };
    assert_eq!(
        pending_entry_value["phyrexian_symbols_paid_with_life"].as_u64(),
        Some(1)
    );

    let stale_first_epoch = engine
        .diagnostic_snapshot()
        .expect("complete second-pause diagnostic snapshot");
    assert!(engine.submit_resolution_choice(0, &copy_answer).is_err());
    assert_eq!(
        engine
            .diagnostic_snapshot()
            .expect("snapshot after first-epoch answer at second choice"),
        stale_first_epoch
    );
    let wrong_actor_opponent_answer = rv1::SubmitResolutionChoice {
        resolution_prompt_id: second_prompt_id,
        decision: rv1::ResolutionChoiceDecision::SelectBranch as i32,
        selected_branch_index: 0,
        ..Default::default()
    };
    assert!(engine
        .submit_resolution_choice(1, &wrong_actor_opponent_answer)
        .is_err());
    assert_eq!(
        engine
            .diagnostic_snapshot()
            .expect("snapshot after wrong-actor opponent answer"),
        stale_first_epoch
    );
    let invalid_opponent_answer = rv1::SubmitResolutionChoice {
        resolution_prompt_id: second_prompt_id,
        decision: rv1::ResolutionChoiceDecision::SelectBranch as i32,
        selected_branch_index: 1,
        ..Default::default()
    };
    assert!(engine
        .submit_resolution_choice(0, &invalid_opponent_answer)
        .is_err());
    assert_eq!(
        engine
            .diagnostic_snapshot()
            .expect("snapshot after unoffered opponent answer"),
        stale_first_epoch
    );
    let completion = engine
        .submit_resolution_choice(
            0,
            &rv1::SubmitResolutionChoice {
                resolution_prompt_id: second_prompt_id,
                decision: rv1::ResolutionChoiceDecision::SelectBranch as i32,
                selected_branch_index: 0,
                ..Default::default()
            },
        )
        .expect("choose the offered opponent and complete entry");
    assert_eq!(engine.state.objects[&metamorph].zone, Zone::Battlefield);
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.pending_replacement_event.is_none());
    assert_eq!(engine.state.players[0].life, 18);
    assert!(!completion.events.iter().any(|event| matches!(
        event.ev.as_ref(),
        Some(rv1::ruled_event::Ev::LifeChanged(change)) if change.player_id == 0
    )));

    let (mut mana_paid_engine, mana_paid_vise, mana_paid_metamorph) =
        compleated_history_metamorph_copy_pending(false);
    assert_eq!(mana_paid_engine.state.players[0].life, 20);
    assert_eq!(
        mana_paid_engine.state.players[0].mana_pool.blue, 0,
        "the U/P symbol used the one available blue mana"
    );
    let mana_copy_prompt_id = mana_paid_engine
        .state
        .pending_resolution
        .as_ref()
        .expect("mana-paid Metamorph copy-source prompt")
        .resolution_prompt_id
        .get();
    let mana_copy_answer = rv1::SubmitResolutionChoice {
        resolution_prompt_id: mana_copy_prompt_id,
        chosen_object_ids: vec![mana_paid_vise],
        ..Default::default()
    };
    mana_paid_engine
        .submit_resolution_choice(0, &mana_copy_answer)
        .expect("choose Black Vise for mana-paid Metamorph");
    let mana_opponent_pending = mana_paid_engine
        .state
        .pending_resolution
        .as_ref()
        .expect("mana-paid copied Black Vise opponent choice");
    let mana_opponent_prompt_id = mana_opponent_pending.resolution_prompt_id.get();
    let mana_parked_stack_value = crate::diagnostic_json::to_value(
        &mana_opponent_pending
            .continuation
            .stack()
            .expect("mana-paid opponent pause retains its resolving stack item")
            .item,
    )
    .expect("serialize mana-paid copied-opponent stack item");
    assert_eq!(
        mana_parked_stack_value["phyrexian_symbols_paid_with_life"].as_u64(),
        Some(0)
    );
    let mana_parked_entry_value = match mana_paid_engine.state.pending_replacement_event.as_ref() {
        Some(PendingReplacementEvent::BattlefieldEntry(entry)) => {
            crate::diagnostic_json::to_value(&entry.event)
                .expect("serialize mana-paid copied paused entry")
        }
        _ => panic!("mana-paid copied entry should remain parked"),
    };
    assert_eq!(
        mana_parked_entry_value["phyrexian_symbols_paid_with_life"].as_u64(),
        Some(0)
    );
    let mana_completion = mana_paid_engine
        .submit_resolution_choice(
            0,
            &rv1::SubmitResolutionChoice {
                resolution_prompt_id: mana_opponent_prompt_id,
                decision: rv1::ResolutionChoiceDecision::SelectBranch as i32,
                selected_branch_index: 0,
                ..Default::default()
            },
        )
        .expect("complete mana-paid Metamorph entry");
    assert_eq!(
        mana_paid_engine.state.objects[&mana_paid_metamorph].zone,
        Zone::Battlefield
    );
    assert_eq!(mana_paid_engine.state.players[0].life, 20);
    assert!(!mana_completion.events.iter().any(|event| matches!(
        event.ev.as_ref(),
        Some(rv1::ruled_event::Ev::LifeChanged(change)) if change.player_id == 0
    )));

    // Keep entrant-generation invalidation separate. The snapshot deliberately starts after
    // Stack -> Graveyard -> Stack, so only binding the stale answer is under test.
    let (mut stale_entrant_engine, stale_entrant_vise, stale_entrant) =
        compleated_history_metamorph_copy_pending(true);
    let stale_entrant_pending = stale_entrant_engine
        .state
        .pending_resolution
        .as_ref()
        .expect("stale-entrant fixture parks for copy source");
    let stale_entrant_prompt_id = stale_entrant_pending.resolution_prompt_id.get();
    let stale_entrant_answer = rv1::SubmitResolutionChoice {
        resolution_prompt_id: stale_entrant_prompt_id,
        chosen_object_ids: vec![stale_entrant_vise],
        ..Default::default()
    };
    let old_entrant_generation = stale_entrant_engine.state.zone_change_generation[&stale_entrant];
    super::super::resolution::move_object_to_zone(
        &mut stale_entrant_engine.state,
        stale_entrant_engine.registry,
        stale_entrant,
        Zone::Graveyard,
        None,
    )
    .unwrap();
    super::super::resolution::move_object_to_zone(
        &mut stale_entrant_engine.state,
        stale_entrant_engine.registry,
        stale_entrant,
        Zone::Stack,
        None,
    )
    .unwrap();
    assert_ne!(
        stale_entrant_engine.state.zone_change_generation[&stale_entrant],
        old_entrant_generation
    );
    let after_entrant_invalidation = stale_entrant_engine
        .diagnostic_snapshot()
        .expect("snapshot after intentional entrant-generation change");
    assert!(stale_entrant_engine
        .submit_resolution_choice(0, &stale_entrant_answer)
        .is_err());
    assert_eq!(
        stale_entrant_engine
            .diagnostic_snapshot()
            .expect("snapshot after stale entrant-generation answer"),
        after_entrant_invalidation
    );
    let stale_entrant_entry_value = match stale_entrant_engine
        .state
        .pending_replacement_event
        .as_ref()
    {
        Some(PendingReplacementEvent::BattlefieldEntry(entry)) => {
            crate::diagnostic_json::to_value(&entry.event)
                .expect("serialize stale-entrant paused entry")
        }
        _ => panic!("expected the stale-entrant entry to remain parked"),
    };
    assert_eq!(
        stale_entrant_entry_value["phyrexian_symbols_paid_with_life"].as_u64(),
        Some(1)
    );

    // Keep the pre-existing copy-source-generation invalidation as its own fixture phase. Its
    // snapshot likewise starts after the intentional Black Vise leave/return mutation.
    let (mut stale_engine, stale_vise, _) = compleated_history_metamorph_copy_pending(true);
    let stale_pending = stale_engine
        .state
        .pending_resolution
        .as_ref()
        .expect("stale-source fixture parks for copy source");
    let stale_prompt_id = stale_pending.resolution_prompt_id.get();
    let stale_answer = rv1::SubmitResolutionChoice {
        resolution_prompt_id: stale_prompt_id,
        chosen_object_ids: vec![stale_vise],
        ..Default::default()
    };
    let old_vise_generation = stale_engine
        .state
        .zone_change_generation
        .get(&stale_vise)
        .copied()
        .unwrap_or(0);
    super::super::resolution::move_object_to_zone(
        &mut stale_engine.state,
        stale_engine.registry,
        stale_vise,
        Zone::Graveyard,
        None,
    )
    .unwrap();
    super::super::resolution::move_object_to_zone(
        &mut stale_engine.state,
        stale_engine.registry,
        stale_vise,
        Zone::Battlefield,
        None,
    )
    .unwrap();
    assert_ne!(
        stale_engine.state.zone_change_generation[&stale_vise],
        old_vise_generation
    );
    let after_invalidation = stale_engine
        .diagnostic_snapshot()
        .expect("snapshot after intentional candidate-generation change");
    assert!(stale_engine
        .submit_resolution_choice(0, &stale_answer)
        .is_err());
    assert_eq!(
        stale_engine
            .diagnostic_snapshot()
            .expect("snapshot after stale candidate-generation answer"),
        after_invalidation
    );
    let stale_entry_value = match stale_engine.state.pending_replacement_event.as_ref() {
        Some(PendingReplacementEvent::BattlefieldEntry(entry)) => {
            crate::diagnostic_json::to_value(&entry.event)
                .expect("serialize stale-source paused entry")
        }
        _ => panic!("expected the stale-source entry to remain parked"),
    };
    assert_eq!(
        stale_entry_value["phyrexian_symbols_paid_with_life"].as_u64(),
        Some(1)
    );
}

#[test]
fn compleated_history_noncast_exile_return_and_reanimate_start_zero() {
    let (mut return_engine, black_vise, metamorph) =
        compleated_history_metamorph_copy_pending(true);
    let first_prompt_id = return_engine
        .state
        .pending_resolution
        .as_ref()
        .expect("cast entrant copy prompt")
        .resolution_prompt_id
        .get();
    return_engine
        .submit_resolution_choice(
            0,
            &rv1::SubmitResolutionChoice {
                resolution_prompt_id: first_prompt_id,
                chosen_object_ids: vec![black_vise],
                ..Default::default()
            },
        )
        .expect("choose Black Vise as the cast entrant's copy source");
    let second_pending = return_engine
        .state
        .pending_resolution
        .as_ref()
        .expect("copied Black Vise opponent prompt");
    let second_prompt_id = second_pending.resolution_prompt_id.get();
    let copied_parked_stack_value = crate::diagnostic_json::to_value(
        &second_pending
            .continuation
            .stack()
            .expect("copied opponent choice retains the resolving Metamorph stack item")
            .item,
    )
    .expect("serialize copied-opponent stack item");
    assert_eq!(
        copied_parked_stack_value["phyrexian_symbols_paid_with_life"].as_u64(),
        Some(1)
    );
    let copied_entry_value = match return_engine.state.pending_replacement_event.as_ref() {
        Some(PendingReplacementEvent::BattlefieldEntry(entry)) => {
            crate::diagnostic_json::to_value(&entry.event)
                .expect("serialize copied entry before opponent choice")
        }
        _ => panic!("expected copied entry to remain parked"),
    };
    assert_eq!(
        copied_entry_value["phyrexian_symbols_paid_with_life"].as_u64(),
        Some(1)
    );
    let completion = return_engine
        .submit_resolution_choice(
            0,
            &rv1::SubmitResolutionChoice {
                resolution_prompt_id: second_prompt_id,
                decision: rv1::ResolutionChoiceDecision::SelectBranch as i32,
                selected_branch_index: 0,
                ..Default::default()
            },
        )
        .expect("finish the actual cast entry");
    assert_eq!(
        return_engine.state.objects[&metamorph].zone,
        Zone::Battlefield
    );
    assert_eq!(return_engine.state.players[0].life, 18);
    assert!(!completion.events.iter().any(|event| matches!(
        event.ev.as_ref(),
        Some(rv1::ruled_event::Ev::LifeChanged(change)) if change.player_id == 0
    )));

    let battlefield_generation = return_engine.state.zone_change_generation[&metamorph];
    super::super::resolution::move_object_to_zone(
        &mut return_engine.state,
        return_engine.registry,
        metamorph,
        Zone::Exile,
        None,
    )
    .unwrap();
    let exile_generation = return_engine.state.zone_change_generation[&metamorph];
    assert!(exile_generation > battlefield_generation);
    assert!(return_engine.state.objects[&metamorph]
        .copiable_values
        .is_none());
    let return_entry =
        super::super::resolution::zones::plain_return_entry(&return_engine, metamorph, 0);
    let mut return_events = Vec::new();
    let return_progress = return_engine.advance_or_park_battlefield_entry(
        return_engine.observer_return_item(metamorph, 0),
        return_entry,
        BattlefieldEntryCompletion::ResolutionEffect {
            owner: 0,
            spell_label: "prepared blink return fixture".to_owned(),
            object_label: "Phyrexian Metamorph".to_owned(),
            from_zone: Zone::Exile,
            triggered_return_generation: None,
        },
        &mut return_events,
    );
    assert!(matches!(return_progress, BattlefieldEntryProgress::Parked));
    assert_eq!(
        return_engine.state.zone_change_generation[&metamorph],
        exile_generation
    );
    let returned_entry_value = match return_engine.state.pending_replacement_event.as_ref() {
        Some(PendingReplacementEvent::BattlefieldEntry(entry)) => {
            crate::diagnostic_json::to_value(&entry.event)
                .expect("serialize plain-return paused entry")
        }
        _ => panic!("plain-return entry should pause for its copy choice"),
    };
    assert_eq!(
        returned_entry_value["phyrexian_symbols_paid_with_life"].as_u64(),
        Some(0)
    );

    let (mut reanimate_engine, black_vise, metamorph) =
        compleated_history_metamorph_copy_pending(true);
    let copy_prompt_id = reanimate_engine
        .state
        .pending_resolution
        .as_ref()
        .expect("cast entrant copy prompt")
        .resolution_prompt_id
        .get();
    reanimate_engine
        .submit_resolution_choice(
            0,
            &rv1::SubmitResolutionChoice {
                resolution_prompt_id: copy_prompt_id,
                chosen_object_ids: vec![black_vise],
                ..Default::default()
            },
        )
        .expect("choose Black Vise as the cast entrant's copy source");
    let opponent_prompt_id = reanimate_engine
        .state
        .pending_resolution
        .as_ref()
        .expect("copied Black Vise opponent prompt")
        .resolution_prompt_id
        .get();
    reanimate_engine
        .submit_resolution_choice(
            0,
            &rv1::SubmitResolutionChoice {
                resolution_prompt_id: opponent_prompt_id,
                decision: rv1::ResolutionChoiceDecision::SelectBranch as i32,
                selected_branch_index: 0,
                ..Default::default()
            },
        )
        .expect("finish the actual cast entry before reanimation");
    super::super::resolution::move_object_to_zone(
        &mut reanimate_engine.state,
        reanimate_engine.registry,
        metamorph,
        Zone::Graveyard,
        None,
    )
    .unwrap();
    assert!(reanimate_engine.state.objects[&metamorph]
        .copiable_values
        .is_none());
    assert!(reanimate_engine
        .characteristics(metamorph)
        .unwrap()
        .is_creature());

    let reanimate = object(&mut reanimate_engine, "reanimate", Zone::Hand, 0);
    reanimate_engine.state.players[0].hand.push(reanimate);
    reanimate_engine.state.players[0].mana_pool.black = 1;
    let reanimate_slot = reanimate_engine.state.players[0]
        .hand
        .iter()
        .position(|object_id| *object_id == reanimate)
        .expect("actual registered Reanimate is in hand") as u32;
    reanimate_engine
        .apply_scenario_command(
            0,
            &rv1::RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::CastSpell(rv1::CastSpell {
                    cast_method: rv1::CastMethod::Normal as i32,
                    source: Some(rv1::CastSource {
                        location: Some(rv1::cast_source::Location::HandIndex(reanimate_slot)),
                        ..Default::default()
                    }),
                    targets: vec![rv1::TargetRef {
                        object_id: metamorph,
                        group_index: 0,
                        kind: 0,
                        ..Default::default()
                    }],
                    ..Default::default()
                })),
            },
        )
        .expect("cast registered Reanimate targeting the printed Metamorph creature");
    for _ in 0..2 {
        let actor = reanimate_engine.state.priority_player_id();
        reanimate_engine
            .apply_scenario_command(
                actor,
                &rv1::RuledCommand {
                    cmd: Some(rv1::ruled_command::Cmd::PassPriority(rv1::PassPriority {})),
                },
            )
            .expect("resolve Reanimate until the returned entry pauses");
    }
    let reanimated_entry = match reanimate_engine.state.pending_replacement_event.as_ref() {
        Some(PendingReplacementEvent::BattlefieldEntry(entry)) => {
            crate::diagnostic_json::to_value(&entry.event)
                .expect("serialize Reanimate-created entry")
        }
        _ => panic!("Reanimate should park its returned Metamorph at the copy prompt"),
    };
    assert_eq!(
        reanimated_entry["phyrexian_symbols_paid_with_life"].as_u64(),
        Some(0)
    );
    assert_eq!(
        reanimate_engine
            .state
            .pending_resolution
            .as_ref()
            .expect("Reanimate copy-source choice")
            .presentation
            .choice_kind,
        rv1::ChoiceKind::CopySource
    );
}

fn compleated_history_metamorph_copy_pending(pay_life: bool) -> (GameEngine, ObjectId, ObjectId) {
    let mut engine = engine();
    let black_vise = object(&mut engine, "black_vise", Zone::Battlefield, 0);
    let metamorph = object(&mut engine, "phyrexian_metamorph", Zone::Hand, 0);
    engine.state.players[0].hand.push(metamorph);
    engine.state.players[0].mana_pool.colorless = 3;
    if !pay_life {
        engine.state.players[0].mana_pool.blue = 1;
    }
    let hand_slot = engine.state.players[0]
        .hand
        .iter()
        .position(|object_id| *object_id == metamorph)
        .expect("actual Metamorph is in hand") as u32;
    let cast_batch = engine
        .apply_scenario_command(
            0,
            &rv1::RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::CastSpell(rv1::CastSpell {
                    cast_method: rv1::CastMethod::Normal as i32,
                    source: Some(rv1::CastSource {
                        location: Some(rv1::cast_source::Location::HandIndex(hand_slot)),
                        ..Default::default()
                    }),
                    flex_payments: vec![rv1::FlexPipPayment {
                        pip_index: 1,
                        pay_life,
                    }],
                    ..Default::default()
                })),
            },
        )
        .expect("cast actual Metamorph with an explicit Phyrexian payment");
    let paid_cast = engine
        .state
        .stack
        .last()
        .expect("accepted Metamorph stack item");
    let paid_cast_value = crate::diagnostic_json::to_value(paid_cast)
        .expect("serialize actual paid Metamorph stack item");
    assert_eq!(
        paid_cast_value["phyrexian_symbols_paid_with_life"].as_u64(),
        if pay_life { Some(1) } else { Some(0) }
    );
    assert_eq!(
        paid_cast.mana_colors_spent_to_cast.count(),
        if pay_life { 0 } else { 1 }
    );
    assert!(paid_cast.cast_occurrence.is_some());
    assert_eq!(engine.state.players[0].mana_pool.colorless, 0);
    assert_eq!(engine.state.players[0].mana_pool.blue, 0);
    assert_eq!(engine.state.players[0].life, if pay_life { 18 } else { 20 });
    assert_eq!(
        cast_batch
            .events
            .iter()
            .filter(|event| matches!(
                event.ev.as_ref(),
                Some(rv1::ruled_event::Ev::LifeChanged(change)) if change.player_id == 0
            ))
            .count(),
        if pay_life { 1 } else { 0 },
        "the control cast pays no life"
    );

    for _ in 0..2 {
        let actor = engine.state.priority_player_id();
        engine
            .apply_scenario_command(
                actor,
                &rv1::RuledCommand {
                    cmd: Some(rv1::ruled_command::Cmd::PassPriority(rv1::PassPriority {})),
                },
            )
            .expect("resolve Metamorph to its real battlefield entry");
    }
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("Metamorph copy-source choice");
    assert_eq!(
        pending.presentation.choice_kind,
        rv1::ChoiceKind::CopySource
    );
    assert!(pending.presentation.candidates.contains(&black_vise));
    let parked_stack_value = crate::diagnostic_json::to_value(
        &pending
            .continuation
            .stack()
            .expect("copy-source prompt retains the resolving Metamorph stack item")
            .item,
    )
    .expect("serialize copy-source parked stack item");
    assert_eq!(
        parked_stack_value["phyrexian_symbols_paid_with_life"].as_u64(),
        if pay_life { Some(1) } else { Some(0) }
    );
    let pending_entry_value = match engine.state.pending_replacement_event.as_ref() {
        Some(PendingReplacementEvent::BattlefieldEntry(entry)) => {
            crate::diagnostic_json::to_value(&entry.event)
                .expect("serialize actual cast's paused entry")
        }
        _ => panic!("Metamorph's actual entry should be parked"),
    };
    assert_eq!(
        pending_entry_value["phyrexian_symbols_paid_with_life"].as_u64(),
        if pay_life { Some(1) } else { Some(0) }
    );
    (engine, black_vise, metamorph)
}

#[test]
fn compleated_entry_groups_two_loyalty_rows_before_reducing_once() {
    let mut engine = engine();
    let entrant = object(&mut engine, "jace_beleren", Zone::Stack, 0);
    let source_one = object(&mut engine, "mind_stone", Zone::Stack, 0);
    let source_two = object(&mut engine, "mind_stone", Zone::Stack, 1);

    let mut face = engine
        .registry
        .get("jace_beleren")
        .expect("registered ordinary planeswalker")
        .primary_face()
        .clone();
    assert!(face.static_abilities.iter().any(|ability| matches!(
        &ability.definition,
        StaticAbilityDef::EntersWithCounters {
            counter: CounterKind::Loyalty,
            ..
        }
    )));
    face.static_abilities.retain(|ability| {
        !matches!(
            &ability.definition,
            StaticAbilityDef::EntersWithCounters { .. }
        )
    });
    face.loyalty = None;

    let compleated_ability = CardRegistry::from_chunks_and_tokens(
        &[r#"(
            id: "compleated_entry_probe",
            name: "Compleated Entry Probe",
            face_id: "compleated_entry_probe",
            mana_cost: "{4}{B}{B/P}",
            supertypes: ["Legendary"],
            types: ["Planeswalker", "Vraska"],
            loyalty: Some(6),
            static_abilities: [(
                ability_id: "compleated",
                presentation: Fallback,
                definition: Compleated,
            )],
        )"#],
        &[],
    )
    .expect("valid test-only Compleated face")
    .get("compleated_entry_probe")
    .expect("test-only Compleated face is registered")
    .primary_face()
    .static_abilities
    .iter()
    .find(|ability| matches!(&ability.definition, StaticAbilityDef::Compleated))
    .expect("test-only face has Compleated")
    .clone();
    face.static_abilities.push(compleated_ability);

    let display_name = face.name.clone();
    engine
        .state
        .objects
        .get_mut(&entrant)
        .unwrap()
        .copiable_values = Some(CopiableValues {
        prototyped: false,
        source_card_id: "jace_beleren".into(),
        source_face_index: 0,
        display_name,
        face,
        room_faces: None,
    });

    let mut entry = event(&engine, entrant);
    entry.phyrexian_symbols_paid_with_life = 1;
    let source_one_generation = engine
        .state
        .zone_change_generation
        .get(&source_one)
        .copied()
        .unwrap_or(0);
    let source_two_generation = engine
        .state
        .zone_change_generation
        .get(&source_two)
        .copied()
        .unwrap_or(0);
    append_entry_counter_placement(
        &mut entry,
        0,
        [(CounterKind::Loyalty, 1)],
        EntryCounterSource::ResolutionEffect {
            stack_object_id: source_one,
            stack_zone_change_generation: source_one_generation,
            effect_index: 0,
        },
        false,
    );
    append_entry_counter_placement(
        &mut entry,
        1,
        [(CounterKind::Loyalty, 5)],
        EntryCounterSource::ResolutionEffect {
            stack_object_id: source_two,
            stack_zone_change_generation: source_two_generation,
            effect_index: 0,
        },
        false,
    );

    assert_eq!(entry.phyrexian_symbols_paid_with_life, 1);
    assert_eq!(entry.entry_counter_placements.len(), 2);
    assert_eq!(entry.entry_counter_placements[0].actor, 0);
    assert_eq!(entry.entry_counter_placements[1].actor, 1);
    assert_eq!(
        entry.entry_counter_placements[0]
            .counters
            .get(&CounterKind::Loyalty),
        Some(&1)
    );
    assert_eq!(
        entry.entry_counter_placements[1]
            .counters
            .get(&CounterKind::Loyalty),
        Some(&5)
    );
    assert_ne!(
        source_one, source_two,
        "the two effect identities are distinct"
    );
    assert!(matches!(
        &entry.entry_counter_placements[0].source,
        EntryCounterSource::ResolutionEffect { stack_object_id, effect_index: 0, .. }
            if *stack_object_id == source_one
    ));
    assert!(matches!(
        &entry.entry_counter_placements[1].source,
        EntryCounterSource::ResolutionEffect { stack_object_id, effect_index: 0, .. }
            if *stack_object_id == source_two
    ));
    assert_eq!(entry.entry_counters[&CounterKind::Loyalty], 6);
    assert!(engine.battlefield_entry_face(&entry).is_some_and(|face| {
        face.static_abilities
            .iter()
            .any(|ability| matches!(&ability.definition, StaticAbilityDef::Compleated))
    }));

    let references: [&BattlefieldEntryEvent; 1] = [&entry];
    let proposals = engine.proposed_entry_counter_placements(&references, &[]);
    assert_eq!(proposals.len(), 2);
    let (counter_batch, choices) = engine.prepare_counter_placement_batch(proposals).unwrap();
    assert!(
        choices.is_none(),
        "the fixture has no competing replacement choice"
    );
    GameEngine::apply_entry_counter_placement_results(&mut entry, &counter_batch.events).unwrap();

    assert_eq!(
        entry.entry_counters.get(&CounterKind::Loyalty).copied(),
        Some(4),
        "Compleated reduces the positive loyalty group once by two per life-paid symbol"
    );
}

fn compleated_doubling_season_entry_fixture(
    phyrexian_symbols_paid_with_life: u32,
) -> (
    GameEngine,
    ObjectId,
    ParkedStackResolution,
    BattlefieldEntryEvent,
) {
    let mut engine = engine();
    let season = object(&mut engine, "doubling_season", Zone::Battlefield, 0);
    engine.emit_static_abilities_on_enter(season);

    let entrant = object(&mut engine, "jace_beleren", Zone::Stack, 0);
    let mut face = engine
        .registry
        .get("jace_beleren")
        .expect("registered ordinary planeswalker")
        .primary_face()
        .clone();
    assert!(face.static_abilities.iter().any(|ability| matches!(
        &ability.definition,
        StaticAbilityDef::EntersWithCounters {
            counter: CounterKind::Loyalty,
            ..
        }
    )));
    face.static_abilities.retain(|ability| {
        !matches!(
            &ability.definition,
            StaticAbilityDef::EntersWithCounters { .. }
        )
    });
    face.loyalty = None;

    let compleated_ability = CardRegistry::from_chunks_and_tokens(
        &[r#"(
            id: "compleated_season_entry_probe",
            name: "Compleated Season Entry Probe",
            face_id: "compleated_season_entry_probe",
            mana_cost: "{4}{B}{B/P}",
            supertypes: ["Legendary"],
            types: ["Planeswalker", "Vraska"],
            loyalty: Some(6),
            static_abilities: [(
                ability_id: "compleated",
                presentation: Fallback,
                definition: Compleated,
            )],
        )"#],
        &[],
    )
    .expect("valid test-only Compleated face")
    .get("compleated_season_entry_probe")
    .expect("test-only Compleated face is registered")
    .primary_face()
    .static_abilities
    .iter()
    .find(|ability| matches!(&ability.definition, StaticAbilityDef::Compleated))
    .expect("test-only face has Compleated")
    .clone();
    face.static_abilities.push(compleated_ability);

    let display_name = face.name.clone();
    engine
        .state
        .objects
        .get_mut(&entrant)
        .expect("planeswalker entrant exists")
        .copiable_values = Some(CopiableValues {
        prototyped: false,
        source_card_id: "jace_beleren".into(),
        source_face_index: 0,
        display_name,
        face,
        room_faces: None,
    });

    let stack = paid_divination_stack(&mut engine);
    let mut entry = event(&engine, entrant);
    entry.phyrexian_symbols_paid_with_life = phyrexian_symbols_paid_with_life;
    let stack_zone_change_generation = engine
        .state
        .zone_change_generation
        .get(&stack.item.id)
        .copied()
        .unwrap_or(0);
    append_entry_counter_placement(
        &mut entry,
        0,
        [(CounterKind::Loyalty, 6)],
        EntryCounterSource::ResolutionEffect {
            stack_object_id: stack.item.id,
            stack_zone_change_generation,
            effect_index: 0,
        },
        false,
    );
    assert_eq!(entry.entry_counter_placements.len(), 1);
    assert_eq!(
        entry.entry_counter_placements[0]
            .counters
            .get(&CounterKind::Loyalty),
        Some(&6)
    );
    assert!(engine.battlefield_entry_face(&entry).is_some_and(|face| {
        face.static_abilities
            .iter()
            .any(|ability| matches!(&ability.definition, StaticAbilityDef::Compleated))
    }));

    (engine, entrant, stack, entry)
}

#[test]
fn compleated_entry_doubling_season_offers_both_orders_and_mana_control() {
    for (compleated_first, expected_loyalty) in [(true, 8), (false, 10)] {
        let (mut engine, entrant, stack, entry) = compleated_doubling_season_entry_fixture(1);
        let completion = crate::state::BattlefieldEntryCompletion::ResolutionEffect {
            owner: 0,
            spell_label: "Divination".into(),
            object_label: "Jace Beleren".into(),
            from_zone: Zone::Stack,
            triggered_return_generation: None,
        };
        let mut events = Vec::new();
        assert!(
            engine
                .prepare_single_entry_counter_placements(stack, entry, completion, &mut events)
                .expect("prepare grouped Compleated and Doubling Season choices")
                .is_none(),
            "both live replacements require the real first player choice"
        );
        assert_eq!(
            events
                .iter()
                .filter(|event| matches!(
                    event.ev.as_ref(),
                    Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(_))
                ))
                .count(),
            1
        );
        engine
            .publish_resolution_prompt_ids(&mut events)
            .expect("publish the replacement choice prompt ID");

        let (compleated_application_id, season_application_id, prompt_id) = {
            let offer = events
                .iter()
                .find_map(|event| match event.ev.as_ref() {
                    Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(choice)) => Some(choice),
                    _ => None,
                })
                .expect("first replacement offer is present");
            assert_eq!(offer.choice_kind, rv1::ChoiceKind::ReplacementEffect as i32);
            assert_eq!(
                offer.replacement_options.len(),
                2,
                "the first offer contains Compleated and Doubling Season"
            );
            let compleated = offer
                .replacement_options
                .iter()
                .find(|option| {
                    option
                        .effect_summary
                        .to_ascii_lowercase()
                        .contains("loyalty")
                })
                .expect("Compleated loyalty option is engine-offered");
            let season = offer
                .replacement_options
                .iter()
                .find(|option| option.effect_summary.contains("doubles effect counters"))
                .expect("Doubling Season option is engine-offered");
            assert_ne!(
                compleated.application_id, season.application_id,
                "each public option has its own application identity"
            );
            assert!(offer
                .candidate_object_ids
                .contains(&compleated.application_id));
            assert!(offer.candidate_object_ids.contains(&season.application_id));
            (
                compleated.application_id,
                season.application_id,
                offer.resolution_prompt_id,
            )
        };
        assert_ne!(prompt_id, 0, "the published prompt ID is nonzero");

        let chosen_application_id = if compleated_first {
            compleated_application_id
        } else {
            season_application_id
        };
        let resumed = engine
            .apply_command(
                0,
                &rv1::RuledCommand {
                    cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(
                        rv1::SubmitResolutionChoice {
                            resolution_prompt_id: prompt_id,
                            chosen_object_ids: vec![chosen_application_id],
                            ..Default::default()
                        },
                    )),
                },
            )
            .expect("apply the engine-published replacement choice through normal command flow");
        assert!(
            !resumed.events.iter().any(|event| matches!(
                event.ev.as_ref(),
                Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(_))
            )),
            "the sole remaining replacement applies automatically without a second prompt"
        );
        assert_eq!(engine.state.objects[&entrant].zone, Zone::Battlefield);
        assert_eq!(
            engine.state.objects[&entrant].counter_count(CounterKind::Loyalty),
            expected_loyalty,
            "the selected order determines the final grouped Loyalty total"
        );
        assert!(engine.state.pending_resolution.is_none());
        assert!(engine.state.pending_replacement_event.is_none());
    }

    let (mut engine, entrant, stack, entry) = compleated_doubling_season_entry_fixture(0);
    let completion = crate::state::BattlefieldEntryCompletion::ResolutionEffect {
        owner: 0,
        spell_label: "Divination".into(),
        object_label: "Jace Beleren".into(),
        from_zone: Zone::Stack,
        triggered_return_generation: None,
    };
    let mut events = Vec::new();
    let entry = engine
        .prepare_single_entry_counter_placements(stack, entry, completion, &mut events)
        .expect("prepare the zero-life-payment counter entry")
        .expect("the sole Doubling Season replacement applies automatically");
    assert!(
        events.is_empty(),
        "no replacement choice is needed for the control"
    );
    assert_eq!(
        entry.entry_counters.get(&CounterKind::Loyalty).copied(),
        Some(12)
    );
    engine
        .commit_battlefield_entry_state(entry, None)
        .expect("commit the automatically doubled entry");
    assert_eq!(engine.state.objects[&entrant].zone, Zone::Battlefield);
    assert_eq!(
        engine.state.objects[&entrant].counter_count(CounterKind::Loyalty),
        12,
        "without a life-paid Phyrexian symbol, Compleated does not reduce the entry"
    );
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.pending_replacement_event.is_none());
}

#[test]
fn compleated_entry_group_preserves_contributors_and_isolates_other_counters_and_entrants() {
    let (donor_engine, donor_entrant, _donor_stack, _donor_entry) =
        compleated_doubling_season_entry_fixture(1);
    let compleated_values = donor_engine.state.objects[&donor_entrant]
        .copiable_values
        .clone()
        .expect("the donor entrant has its effective Compleated face");
    assert!(compleated_values
        .face
        .static_abilities
        .iter()
        .any(|ability| { matches!(&ability.definition, StaticAbilityDef::Compleated) }));

    let mut engine = engine_with_players(&[0, 1, 2]);
    let season = object(&mut engine, "doubling_season", Zone::Battlefield, 2);
    engine.emit_static_abilities_on_enter(season);
    let season_generation = engine
        .state
        .zone_change_generation
        .get(&season)
        .copied()
        .unwrap_or(0);

    let paid_stack = paid_divination_stack(&mut engine);
    let paid_stack_id = paid_stack.item.id;
    let paid_stack_generation = engine
        .state
        .zone_change_generation
        .get(&paid_stack_id)
        .copied()
        .unwrap_or(0);

    let entrant_a = object(&mut engine, "jace_beleren", Zone::Stack, 0);
    let entrant_b = object(&mut engine, "jace_beleren", Zone::Stack, 1);
    let entrant_c = object(&mut engine, "forest", Zone::Stack, 2);
    engine
        .state
        .objects
        .get_mut(&entrant_a)
        .unwrap()
        .copiable_values = Some(compleated_values.clone());
    engine
        .state
        .objects
        .get_mut(&entrant_b)
        .unwrap()
        .copiable_values = Some(compleated_values);

    let entrant_generations = [entrant_a, entrant_b, entrant_c].map(|object_id| {
        engine
            .state
            .zone_change_generation
            .get(&object_id)
            .copied()
            .unwrap_or(0)
    });

    let mut entry_a = event(&engine, entrant_a);
    entry_a.deciding_player = 0;
    entry_a.destination_controller = 0;
    entry_a.phyrexian_symbols_paid_with_life = 1;
    append_entry_counter_placement(
        &mut entry_a,
        0,
        [(CounterKind::Loyalty, 1)],
        EntryCounterSource::ResolutionEffect {
            stack_object_id: paid_stack_id,
            stack_zone_change_generation: paid_stack_generation,
            effect_index: 0,
        },
        false,
    );
    append_entry_counter_placement(
        &mut entry_a,
        1,
        [(CounterKind::Loyalty, 5)],
        EntryCounterSource::ResolutionEffect {
            stack_object_id: paid_stack_id,
            stack_zone_change_generation: paid_stack_generation,
            effect_index: 1,
        },
        true,
    );
    append_entry_counter_placement(
        &mut entry_a,
        1,
        [(CounterKind::Charge, 3)],
        EntryCounterSource::ResolutionEffect {
            stack_object_id: paid_stack_id,
            stack_zone_change_generation: paid_stack_generation,
            effect_index: 2,
        },
        false,
    );

    let mut entry_b = event(&engine, entrant_b);
    entry_b.deciding_player = 1;
    entry_b.destination_controller = 1;
    entry_b.phyrexian_symbols_paid_with_life = 2;
    append_entry_counter_placement(
        &mut entry_b,
        1,
        [(CounterKind::Loyalty, 9)],
        EntryCounterSource::ResolutionEffect {
            stack_object_id: paid_stack_id,
            stack_zone_change_generation: paid_stack_generation,
            effect_index: 3,
        },
        false,
    );

    // Forest is intentionally a prepared-state counter-planner fixture; this does not claim
    // that a land can legally be cast as a spell.
    let mut entry_c = event(&engine, entrant_c);
    entry_c.deciding_player = 2;
    entry_c.destination_controller = 2;
    append_entry_counter_placement(
        &mut entry_c,
        0,
        [(CounterKind::Charge, 5)],
        EntryCounterSource::ResolutionEffect {
            stack_object_id: paid_stack_id,
            stack_zone_change_generation: paid_stack_generation,
            effect_index: 4,
        },
        false,
    );

    type ExpectedRow = (usize, usize, PlayerId, CounterKind, u32, u32, u32, bool);
    let expected_rows: [ExpectedRow; 5] = [
        (0, 0, 0, CounterKind::Loyalty, 1, 0, 0, false),
        (0, 1, 1, CounterKind::Loyalty, 5, 4, 1, true),
        (0, 2, 1, CounterKind::Charge, 3, 3, 2, false),
        (1, 0, 1, CounterKind::Loyalty, 9, 5, 3, false),
        (2, 0, 0, CounterKind::Charge, 5, 10, 4, false),
    ];
    let expected_objects = [entrant_a, entrant_b, entrant_c];
    let expected_controllers: [PlayerId; 3] = [0, 1, 2];
    assert_eq!(entry_a.entry_counter_placements.len(), 3);
    assert_eq!(entry_b.entry_counter_placements.len(), 1);
    assert_eq!(entry_c.entry_counter_placements.len(), 1);
    assert_eq!(
        (entry_a.deciding_player, entry_a.destination_controller),
        (0, 0)
    );
    assert_eq!(
        (entry_b.deciding_player, entry_b.destination_controller),
        (1, 1)
    );
    assert_eq!(
        (entry_c.deciding_player, entry_c.destination_controller),
        (2, 2)
    );
    assert_eq!(entry_a.phyrexian_symbols_paid_with_life, 1);
    assert_eq!(entry_b.phyrexian_symbols_paid_with_life, 2);
    assert_eq!(entry_c.phyrexian_symbols_paid_with_life, 0);
    assert_eq!(
        entry_a.entry_counters,
        BTreeMap::from([(CounterKind::Loyalty, 6), (CounterKind::Charge, 3)])
    );
    assert_eq!(
        entry_b.entry_counters,
        BTreeMap::from([(CounterKind::Loyalty, 9)])
    );
    assert_eq!(
        entry_c.entry_counters,
        BTreeMap::from([(CounterKind::Charge, 5)])
    );
    assert!(engine.battlefield_entry_face(&entry_a).is_some_and(|face| {
        face.static_abilities
            .iter()
            .any(|ability| matches!(&ability.definition, StaticAbilityDef::Compleated))
    }));
    assert!(engine.battlefield_entry_face(&entry_b).is_some_and(|face| {
        face.static_abilities
            .iter()
            .any(|ability| matches!(&ability.definition, StaticAbilityDef::Compleated))
    }));

    for &(event_index, placement_index, actor, counter, baseline, _, effect_index, read_ahead) in
        &expected_rows
    {
        let entry = match event_index {
            0 => &entry_a,
            1 => &entry_b,
            2 => &entry_c,
            _ => unreachable!("expected event index is in range"),
        };
        let placement = &entry.entry_counter_placements[placement_index];
        assert_eq!(placement.actor, actor);
        assert_eq!(placement.counters, BTreeMap::from([(counter, baseline)]));
        assert_eq!(placement.read_ahead_entry, read_ahead);
        assert!(placement.applied_replacements.is_empty());
        assert!(placement.history.is_none());
        assert!(matches!(
            &placement.source,
            EntryCounterSource::ResolutionEffect {
                stack_object_id,
                stack_zone_change_generation,
                effect_index: actual_effect_index,
            } if *stack_object_id == paid_stack_id
                && *stack_zone_change_generation == paid_stack_generation
                && *actual_effect_index == effect_index
        ));
    }

    let expected_original_maps = expected_rows
        .iter()
        .map(|row| BTreeMap::from([(row.3, row.4)]))
        .collect::<Vec<_>>();
    let entries = [&entry_a, &entry_b, &entry_c];
    let proposals = engine.proposed_entry_counter_placements(&entries, &[]);
    assert_eq!(proposals.len(), 5);
    assert_eq!(
        proposals
            .iter()
            .map(|proposal| proposal.counters.clone())
            .collect::<Vec<_>>(),
        expected_original_maps
    );
    let saved_original_maps = proposals
        .iter()
        .map(|proposal| proposal.original_counters.clone())
        .collect::<Vec<_>>();
    assert_eq!(saved_original_maps, expected_original_maps);

    let (counter_batch, choices) = engine
        .prepare_counter_placement_batch(proposals)
        .expect("prepare the simultaneous grouped counter proposals");
    assert!(
        choices.is_none(),
        "there are no competing replacement choices"
    );
    assert_eq!(counter_batch.events.len(), 5);
    assert_eq!(
        counter_batch
            .events
            .iter()
            .map(|proposal| proposal.original_counters.clone())
            .collect::<Vec<_>>(),
        saved_original_maps,
        "replacement preparation preserves every proposal's original row map"
    );
    assert_eq!(
        counter_batch
            .events
            .iter()
            .zip(expected_rows.iter())
            .map(|(proposal, row)| proposal.counters[&row.3])
            .collect::<Vec<_>>(),
        vec![0, 4, 3, 5, 10]
    );

    for (
        proposal,
        &(event_index, placement_index, actor, counter, baseline, result, effect_index, _),
    ) in counter_batch.events.iter().zip(expected_rows.iter())
    {
        let expected_object = expected_objects[event_index];
        let expected_generation = entrant_generations[event_index];
        let context = proposal
            .entry_context
            .as_ref()
            .expect("entry proposal retains its row context");
        assert_eq!(proposal.actor, actor);
        assert_eq!(
            proposal.original_counters,
            BTreeMap::from([(counter, baseline)])
        );
        assert_eq!(proposal.counters, BTreeMap::from([(counter, result)]));
        assert_eq!(context.placement_index, placement_index);
        assert_eq!(
            context.destination_controller,
            expected_controllers[event_index]
        );
        assert_eq!(
            context.history.baseline_counters,
            BTreeMap::from([(counter, baseline)])
        );
        assert!(context.history.baseline_battlefield_receipts.is_empty());
        assert!(matches!(
            &proposal.recipient,
            CounterPlacementRecipient::EnteringObject {
                object_id,
                zone_change_generation,
            } if *object_id == expected_object && *zone_change_generation == expected_generation
        ));
        assert!(matches!(
            &proposal.entry_source,
            Some(EntryCounterSource::ResolutionEffect {
                stack_object_id,
                stack_zone_change_generation,
                effect_index: actual_effect_index,
            }) if *stack_object_id == paid_stack_id
                && *stack_zone_change_generation == paid_stack_generation
                && *actual_effect_index == effect_index
        ));
        if event_index < 2 {
            assert!(proposal.applied_replacements.is_empty());
        } else {
            assert_eq!(proposal.applied_replacements.len(), 1);
        }
    }

    let history_a_first = &counter_batch.events[0]
        .entry_context
        .as_ref()
        .unwrap()
        .history;
    let history_a_second = &counter_batch.events[1]
        .entry_context
        .as_ref()
        .unwrap()
        .history;
    let history_a_charge = &counter_batch.events[2]
        .entry_context
        .as_ref()
        .unwrap()
        .history;
    let history_b = &counter_batch.events[3]
        .entry_context
        .as_ref()
        .unwrap()
        .history;
    let history_c = &counter_batch.events[4]
        .entry_context
        .as_ref()
        .unwrap()
        .history;
    assert_eq!(
        history_a_first.baseline_counters,
        BTreeMap::from([(CounterKind::Loyalty, 1)])
    );
    assert_eq!(
        history_a_second.baseline_counters,
        BTreeMap::from([(CounterKind::Loyalty, 5)])
    );
    assert_eq!(
        history_a_charge.baseline_counters,
        BTreeMap::from([(CounterKind::Charge, 3)])
    );
    assert_eq!(
        history_b.baseline_counters,
        BTreeMap::from([(CounterKind::Loyalty, 9)])
    );
    assert_eq!(
        history_c.baseline_counters,
        BTreeMap::from([(CounterKind::Charge, 5)])
    );
    assert!(history_a_first.baseline_battlefield_receipts.is_empty());
    assert!(history_a_second.baseline_battlefield_receipts.is_empty());
    assert!(history_a_charge.baseline_battlefield_receipts.is_empty());
    assert!(history_b.baseline_battlefield_receipts.is_empty());
    assert!(history_c.baseline_battlefield_receipts.is_empty());

    assert_eq!(history_a_first.receipts.len(), 1);
    let (a_first_identity, a_first_removed) = match &history_a_first.receipts[0] {
        EntryCounterReplacementReceipt::Compleated {
            identity,
            removed_loyalty,
        } => (identity, *removed_loyalty),
        receipt => panic!("A's first loyalty row has the Compleated receipt, got {receipt:?}"),
    };
    assert_eq!(a_first_removed, 1);
    assert_eq!(a_first_identity.recipient_object_id, entrant_a);
    assert_eq!(
        a_first_identity.recipient_zone_change_generation,
        entrant_generations[0]
    );
    assert_eq!(
        a_first_identity.copy_revision,
        engine.state.objects[&entrant_a].copy_revision
    );
    assert_eq!(a_first_identity.ability_id, "compleated");

    assert_eq!(history_a_second.receipts.len(), 1);
    let (a_second_identity, a_second_removed) = match &history_a_second.receipts[0] {
        EntryCounterReplacementReceipt::Compleated {
            identity,
            removed_loyalty,
        } => (identity, *removed_loyalty),
        receipt => panic!("A's second loyalty row has the Compleated receipt, got {receipt:?}"),
    };
    assert_eq!(a_second_removed, 1);
    assert_eq!(a_second_identity, a_first_identity);
    assert!(history_a_charge.receipts.is_empty());

    assert_eq!(history_b.receipts.len(), 1);
    let (b_identity, b_removed) = match &history_b.receipts[0] {
        EntryCounterReplacementReceipt::Compleated {
            identity,
            removed_loyalty,
        } => (identity, *removed_loyalty),
        receipt => panic!("B's loyalty row has its own Compleated receipt, got {receipt:?}"),
    };
    assert_eq!(b_removed, 4);
    assert_eq!(b_identity.recipient_object_id, entrant_b);
    assert_eq!(
        b_identity.recipient_zone_change_generation,
        entrant_generations[1]
    );
    assert_eq!(
        b_identity.copy_revision,
        engine.state.objects[&entrant_b].copy_revision
    );
    assert_eq!(b_identity.ability_id, "compleated");
    assert_ne!(b_identity, a_first_identity);

    assert_eq!(history_c.receipts.len(), 1);
    let season_receipt = match &history_c.receipts[0] {
        EntryCounterReplacementReceipt::Battlefield { instance } => instance,
        receipt => panic!("C's history records the actual battlefield receipt, got {receipt:?}"),
    };
    assert_eq!(season_receipt.source_object_id, season);
    assert_eq!(
        season_receipt.source_zone_change_generation,
        season_generation
    );
    assert!(!season_receipt.ability_id.trim().is_empty());
    assert_eq!(season_receipt.kind, CounterReplacementKind::DoublingSeason);
    assert_eq!(
        counter_batch.events[4].applied_replacements,
        vec![season_receipt.clone()]
    );
    assert!(history_c
        .receipts
        .iter()
        .all(|receipt| !matches!(receipt, EntryCounterReplacementReceipt::Compleated { .. })));
    for proposal in &counter_batch.events[..4] {
        assert!(proposal.applied_replacements.is_empty());
        assert!(proposal
            .entry_context
            .as_ref()
            .unwrap()
            .history
            .receipts
            .iter()
            .all(|receipt| !matches!(receipt, EntryCounterReplacementReceipt::Battlefield { .. })));
    }

    GameEngine::apply_entry_counter_placement_results(&mut entry_a, &counter_batch.events)
        .expect("write finalized proposals back to A");
    GameEngine::apply_entry_counter_placement_results(&mut entry_b, &counter_batch.events)
        .expect("write finalized proposals back to B");
    GameEngine::apply_entry_counter_placement_results(&mut entry_c, &counter_batch.events)
        .expect("write finalized proposals back to C");

    assert_eq!(
        entry_a.entry_counters,
        BTreeMap::from([(CounterKind::Loyalty, 4), (CounterKind::Charge, 3)])
    );
    assert_eq!(
        entry_b.entry_counters,
        BTreeMap::from([(CounterKind::Loyalty, 5)])
    );
    assert_eq!(
        entry_c.entry_counters,
        BTreeMap::from([(CounterKind::Charge, 10)])
    );
    assert!(entry_a.entry_counter_replacements_resolved);
    assert!(entry_b.entry_counter_replacements_resolved);
    assert!(entry_c.entry_counter_replacements_resolved);
    assert_eq!(entry_a.phyrexian_symbols_paid_with_life, 1);
    assert_eq!(entry_b.phyrexian_symbols_paid_with_life, 2);
    assert_eq!(entry_c.phyrexian_symbols_paid_with_life, 0);

    let written_entries = [&entry_a, &entry_b, &entry_c];
    for (
        proposal_index,
        &(event_index, placement_index, actor, counter, _, result, effect_index, read_ahead),
    ) in expected_rows.iter().enumerate()
    {
        let entry = written_entries[event_index];
        let placement = &entry.entry_counter_placements[placement_index];
        assert_eq!(placement.actor, actor);
        assert_eq!(placement.counters, BTreeMap::from([(counter, result)]));
        assert_eq!(placement.read_ahead_entry, read_ahead);
        assert!(matches!(
            &placement.source,
            EntryCounterSource::ResolutionEffect {
                stack_object_id,
                stack_zone_change_generation,
                effect_index: actual_effect_index,
            } if *stack_object_id == paid_stack_id
                && *stack_zone_change_generation == paid_stack_generation
                && *actual_effect_index == effect_index
        ));
        assert_eq!(
            placement.history.as_ref().unwrap(),
            &counter_batch.events[proposal_index]
                .entry_context
                .as_ref()
                .unwrap()
                .history
        );
        assert_eq!(
            placement.applied_replacements,
            counter_batch.events[proposal_index].applied_replacements
        );
    }
    assert_eq!(
        entry_a.entry_counter_placements[0].counters,
        BTreeMap::from([(CounterKind::Loyalty, 0)]),
        "the zero result remains on its original contribution row"
    );
    assert!(entry_a.entry_counter_placements[0]
        .history
        .as_ref()
        .unwrap()
        .receipts
        .iter()
        .any(|receipt| matches!(receipt, EntryCounterReplacementReceipt::Compleated { .. })));
    assert!(entry_a.entry_counter_placements[1]
        .history
        .as_ref()
        .unwrap()
        .receipts
        .iter()
        .any(|receipt| matches!(receipt, EntryCounterReplacementReceipt::Compleated { .. })));
    assert!(entry_a.entry_counter_placements[2]
        .history
        .as_ref()
        .unwrap()
        .receipts
        .is_empty());
    assert!(entry_a.entry_counter_placements[2]
        .applied_replacements
        .is_empty());
    assert!(entry_b.entry_counter_placements[0]
        .history
        .as_ref()
        .unwrap()
        .receipts
        .iter()
        .any(|receipt| matches!(receipt, EntryCounterReplacementReceipt::Compleated { .. })));
    assert_eq!(
        entry_c.entry_counter_placements[0]
            .history
            .as_ref()
            .unwrap()
            .receipts,
        vec![EntryCounterReplacementReceipt::Battlefield {
            instance: season_receipt.clone(),
        }]
    );
    assert_eq!(
        entry_c.entry_counter_placements[0].applied_replacements,
        vec![season_receipt.clone()]
    );

    assert_eq!(engine.state.objects[&entrant_a].zone, Zone::Stack);
    assert_eq!(engine.state.objects[&entrant_b].zone, Zone::Stack);
    assert_eq!(engine.state.objects[&entrant_c].zone, Zone::Stack);
    assert_eq!(
        [entrant_a, entrant_b, entrant_c].map(|object_id| {
            engine
                .state
                .zone_change_generation
                .get(&object_id)
                .copied()
                .unwrap_or(0)
        }),
        entrant_generations
    );
}

fn compleated_vorinclex_entry_batch_fixture(
    phyrexian_symbols_paid_with_life: u32,
    loyalty_rows: &[(u32, PlayerId)],
) -> (
    GameEngine,
    ObjectId,
    ParkedStackResolution,
    ObjectId,
    ObjectId,
    Vec<rv1::RuledEvent>,
) {
    let (mut engine, entrant, stack, mut loyalty_entry) =
        compleated_doubling_season_entry_fixture(phyrexian_symbols_paid_with_life);
    let vorinclex = object(
        &mut engine,
        "vorinclex,_monstrous_raider",
        Zone::Battlefield,
        0,
    );
    engine.emit_static_abilities_on_enter(vorinclex);

    loyalty_entry.deciding_player = 1;
    loyalty_entry.destination_controller = 1;
    loyalty_entry.entry_counters.clear();
    loyalty_entry.entry_counter_placements.clear();
    loyalty_entry.entry_counter_replacements_resolved = false;
    let stack_zone_change_generation = engine
        .state
        .zone_change_generation
        .get(&stack.item.id)
        .copied()
        .unwrap_or(0);
    for (effect_index, &(count, actor)) in loyalty_rows.iter().enumerate() {
        append_entry_counter_placement(
            &mut loyalty_entry,
            actor,
            [(CounterKind::Loyalty, count)],
            EntryCounterSource::ResolutionEffect {
                stack_object_id: stack.item.id,
                stack_zone_change_generation,
                effect_index: effect_index as u32,
            },
            false,
        );
    }

    let charge_object = object(&mut engine, "forest", Zone::Stack, 0);
    let mut charge_entry = event(&engine, charge_object);
    charge_entry.deciding_player = 0;
    charge_entry.destination_controller = 0;
    append_entry_counter_placement(
        &mut charge_entry,
        0,
        [(CounterKind::Charge, 5)],
        EntryCounterSource::ResolutionEffect {
            stack_object_id: stack.item.id,
            stack_zone_change_generation,
            effect_index: loyalty_rows.len() as u32,
        },
        false,
    );

    let ready = vec![loyalty_entry, charge_entry];
    let generations = ready
        .iter()
        .map(|entry| {
            (
                entry.object_id,
                engine
                    .state
                    .zone_change_generation
                    .get(&entry.object_id)
                    .copied()
                    .unwrap_or(0),
            )
        })
        .collect::<Vec<_>>();
    let origin_mana_values = generations
        .iter()
        .filter_map(|(object_id, generation)| {
            engine
                .characteristics(*object_id)
                .map(|characteristics| (*object_id, *generation, characteristics.mana_value))
        })
        .collect();
    let batch = crate::state::SimultaneousEntryBatch::Zone(crate::state::PendingZoneEntryBatch {
        ready,
        remaining: Vec::new(),
        generations,
        origin_mana_values,
        origin: Zone::Stack,
        spell_label: "Divination".into(),
        completion: None,
    });
    let mut events = Vec::new();
    assert!(engine
        .begin_entry_counter_placements_for_batch(batch, Some(stack.clone()), &mut events)
        .expect("prepare the mixed-controller entry-counter batch")
        .is_none());
    engine
        .publish_resolution_prompt_ids(&mut events)
        .expect("publish the initial entry-counter replacement choice");

    (engine, entrant, stack, vorinclex, charge_object, events)
}

#[test]
fn compleated_entry_vorinclex_preserves_mixed_actors_and_clamps_after_halving() {
    let (mut engine, entrant, stack, vorinclex, charge_object, initial_events) =
        compleated_vorinclex_entry_batch_fixture(1, &[(4, 0), (4, 1)]);
    let stack_generation = engine
        .state
        .zone_change_generation
        .get(&stack.item.id)
        .copied()
        .unwrap_or(0);
    let entrant_generation = engine
        .state
        .zone_change_generation
        .get(&entrant)
        .copied()
        .unwrap_or(0);
    let charge_generation = engine
        .state
        .zone_change_generation
        .get(&charge_object)
        .copied()
        .unwrap_or(0);
    let vorinclex_generation = engine
        .state
        .zone_change_generation
        .get(&vorinclex)
        .copied()
        .unwrap_or(0);
    let season = engine.state.players[0]
        .battlefield
        .iter()
        .copied()
        .find(|object_id| {
            engine
                .state
                .objects
                .get(object_id)
                .is_some_and(|object| object.card_id == "doubling_season")
        })
        .expect("the existing fixture has an actual Doubling Season controlled by player 0");
    let season_generation = engine
        .state
        .zone_change_generation
        .get(&season)
        .copied()
        .unwrap_or(0);
    assert_eq!(
        engine.state.objects[&vorinclex].card_id,
        "vorinclex,_monstrous_raider"
    );
    assert_eq!(engine.state.objects[&vorinclex].controller, 0);
    assert_eq!(engine.state.objects[&season].controller, 0);
    assert_eq!(engine.state.objects[&entrant].zone, Zone::Stack);
    assert_eq!(engine.state.objects[&charge_object].zone, Zone::Stack);

    let initial_offers = initial_events
        .iter()
        .filter_map(|event| match event.ev.as_ref() {
            Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(choice)) => Some(choice),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(initial_offers.len(), 1);
    let initial_offer = initial_offers[0];
    assert_eq!(
        initial_offer.choice_kind,
        rv1::ChoiceKind::ReplacementEffect as i32
    );
    assert_eq!(initial_offer.deciding_player_id, 1);
    assert_ne!(initial_offer.resolution_prompt_id, 0);
    assert_eq!(initial_offer.replacement_options.len(), 3);

    let (
        compleated_application_id,
        initial_double_application_id,
        initial_half_application_id,
        double_instance,
        half_instance,
    ) = {
        let Some(PendingReplacementEvent::Counters(counter_batch)) =
            engine.state.pending_replacement_event.as_ref()
        else {
            panic!("the initial grouped replacement offer retains counter proposals");
        };
        assert_eq!(counter_batch.event_index, 0);
        assert_eq!(counter_batch.events.len(), 3);
        assert_eq!(counter_batch.applications.len(), 3);
        for (row_index, actor, count, effect_index) in [(0, 0, 4, 0), (1, 1, 4, 1), (2, 0, 5, 2)] {
            let proposal = &counter_batch.events[row_index];
            assert_eq!(proposal.actor, actor);
            let expected_counter = if row_index == 2 {
                CounterKind::Charge
            } else {
                CounterKind::Loyalty
            };
            assert_eq!(
                proposal.original_counters.get(&expected_counter),
                Some(&count)
            );
            assert_eq!(proposal.counters.get(&expected_counter), Some(&count));
            assert!(matches!(
                proposal.entry_source.as_ref(),
                Some(EntryCounterSource::ResolutionEffect {
                    stack_object_id,
                    stack_zone_change_generation,
                    effect_index: actual_effect_index,
                }) if *stack_object_id == stack.item.id
                    && *stack_zone_change_generation == stack_generation
                    && *actual_effect_index == effect_index
            ));
        }

        let compleated = counter_batch
            .applications
            .iter()
            .find(|application| {
                matches!(
                    &application.replacement,
                    CounterPlacementReplacement::Compleated {
                        member_event_indices,
                        ..
                    } if member_event_indices.as_slice() == [0, 1]
                )
            })
            .expect("Compleated is offered for both loyalty placement rows");
        let double = counter_batch
            .applications
            .iter()
            .find(|application| {
                matches!(
                    &application.replacement,
                    CounterPlacementReplacement::Battlefield {
                        instance,
                        member_event_indices,
                        source_controller: Some(0),
                    } if instance.source_object_id == vorinclex
                        && instance.source_zone_change_generation == vorinclex_generation
                        && instance.kind == CounterReplacementKind::DoubleCountersYouWouldPut
                        && member_event_indices.as_slice() == [0]
                )
            })
            .expect("Vorinclex double targets only actor 0's loyalty row");
        let half = counter_batch
            .applications
            .iter()
            .find(|application| {
                matches!(
                    &application.replacement,
                    CounterPlacementReplacement::Battlefield {
                        instance,
                        member_event_indices,
                        source_controller: Some(0),
                    } if instance.source_object_id == vorinclex
                        && instance.source_zone_change_generation == vorinclex_generation
                        && instance.kind == CounterReplacementKind::HalveCountersOpponentsWouldPut
                        && member_event_indices.as_slice() == [1]
                )
            })
            .expect("Vorinclex half targets only actor 1's loyalty row");
        assert!(!counter_batch.applications.iter().any(|application| {
            matches!(
                &application.replacement,
                CounterPlacementReplacement::Battlefield { instance, .. }
                    if instance.kind == CounterReplacementKind::DoublingSeason
            )
        }));
        assert_eq!(initial_offer.candidate_object_ids.len(), 3);
        for application in &counter_batch.applications {
            assert!(initial_offer
                .candidate_object_ids
                .contains(&application.choice_id));
            assert!(initial_offer
                .replacement_options
                .iter()
                .any(|option| option.application_id == application.choice_id));
        }
        assert_ne!(double.choice_id, half.choice_id);
        let double_instance = match &double.replacement {
            CounterPlacementReplacement::Battlefield { instance, .. } => instance.clone(),
            CounterPlacementReplacement::Compleated { .. } => unreachable!(),
        };
        let half_instance = match &half.replacement {
            CounterPlacementReplacement::Battlefield { instance, .. } => instance.clone(),
            CounterPlacementReplacement::Compleated { .. } => unreachable!(),
        };
        (
            compleated.choice_id,
            double.choice_id,
            half.choice_id,
            double_instance,
            half_instance,
        )
    };
    assert_ne!(
        double_instance.ability_id, half_instance.ability_id,
        "Vorinclex double and half are separate stable abilities"
    );
    assert_eq!(double_instance.source_object_id, vorinclex);
    assert_eq!(half_instance.source_object_id, vorinclex);
    assert_eq!(
        double_instance.source_zone_change_generation,
        vorinclex_generation
    );
    assert_eq!(
        half_instance.source_zone_change_generation,
        vorinclex_generation
    );

    let after_compleated = engine
        .apply_command(
            1,
            &rv1::RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(
                    rv1::SubmitResolutionChoice {
                        resolution_prompt_id: initial_offer.resolution_prompt_id,
                        chosen_object_ids: vec![compleated_application_id],
                        ..Default::default()
                    },
                )),
            },
        )
        .expect("submit the current Compleated application through normal command handling");
    let compleated_followup_offers = after_compleated
        .events
        .iter()
        .filter_map(|event| match event.ev.as_ref() {
            Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(choice)) => Some(choice),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(compleated_followup_offers.len(), 1);
    let compleated_followup_offer = compleated_followup_offers[0];
    assert_eq!(compleated_followup_offer.deciding_player_id, 1);
    assert_eq!(compleated_followup_offer.replacement_options.len(), 2);
    assert_ne!(
        compleated_followup_offer.resolution_prompt_id,
        initial_offer.resolution_prompt_id
    );

    let (current_double_application_id, _current_half_application_id) = {
        let Some(PendingReplacementEvent::Counters(counter_batch)) =
            engine.state.pending_replacement_event.as_ref()
        else {
            panic!("the current loyalty choice retains the updated proposals");
        };
        assert_eq!(counter_batch.event_index, 0);
        assert_eq!(counter_batch.applications.len(), 2);
        assert_eq!(counter_batch.events[0].counters[&CounterKind::Loyalty], 2);
        assert_eq!(counter_batch.events[1].counters[&CounterKind::Loyalty], 4);
        for (row_index, expected_removed) in [(0, 2), (1, 0)] {
            let proposal = &counter_batch.events[row_index];
            let context = proposal
                .entry_context
                .as_ref()
                .expect("the retained loyalty proposal has entry context");
            assert_eq!(
                proposal.original_counters,
                BTreeMap::from([(CounterKind::Loyalty, 4)])
            );
            assert_eq!(
                context.history.baseline_counters,
                BTreeMap::from([(CounterKind::Loyalty, 4)])
            );
            assert!(context.history.baseline_battlefield_receipts.is_empty());
            assert!(proposal.applied_replacements.is_empty());
            assert_eq!(
                context
                    .history
                    .receipts
                    .iter()
                    .filter(|receipt| matches!(
                        receipt,
                        EntryCounterReplacementReceipt::Compleated { .. }
                    ))
                    .count(),
                1
            );
            assert!(!context.history.receipts.iter().any(|receipt| matches!(
                receipt,
                EntryCounterReplacementReceipt::Battlefield { .. }
            )));
            let (identity, removed_loyalty) = context
                .history
                .receipts
                .iter()
                .find_map(|receipt| match receipt {
                    EntryCounterReplacementReceipt::Compleated {
                        identity,
                        removed_loyalty,
                    } => Some((identity, *removed_loyalty)),
                    EntryCounterReplacementReceipt::Battlefield { .. } => None,
                })
                .expect("the grouped Compleated receipt is retained");
            assert_eq!(removed_loyalty, expected_removed);
            assert_eq!(identity.recipient_object_id, entrant);
            assert_eq!(
                identity.recipient_zone_change_generation,
                entrant_generation
            );
            assert_eq!(
                identity.copy_revision,
                engine.state.objects[&entrant].copy_revision
            );
            assert_eq!(identity.ability_id, "compleated");
        }

        let double = counter_batch
            .applications
            .iter()
            .find(|application| {
                matches!(
                    &application.replacement,
                    CounterPlacementReplacement::Battlefield {
                        instance,
                        member_event_indices,
                        ..
                    } if instance == &double_instance && member_event_indices.as_slice() == [0]
                )
            })
            .expect("the current double application still targets only row 0");
        let half = counter_batch
            .applications
            .iter()
            .find(|application| {
                matches!(
                    &application.replacement,
                    CounterPlacementReplacement::Battlefield {
                        instance,
                        member_event_indices,
                        ..
                    } if instance == &half_instance && member_event_indices.as_slice() == [1]
                )
            })
            .expect("the current half application still targets only row 1");
        assert_ne!(double.choice_id, initial_double_application_id);
        assert_ne!(half.choice_id, initial_half_application_id);
        assert_ne!(double.choice_id, half.choice_id);
        for application in [double, half] {
            assert!(compleated_followup_offer
                .candidate_object_ids
                .contains(&application.choice_id));
            assert!(compleated_followup_offer
                .replacement_options
                .iter()
                .any(|option| option.application_id == application.choice_id));
        }
        (double.choice_id, half.choice_id)
    };

    let player_zero_next_prompt_id_before_double = engine
        .state
        .next_resolution_prompt_id
        .get(&0)
        .copied()
        .unwrap_or(0);
    let after_double = engine
        .apply_command(
            1,
            &rv1::RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(
                    rv1::SubmitResolutionChoice {
                        resolution_prompt_id: compleated_followup_offer.resolution_prompt_id,
                        chosen_object_ids: vec![current_double_application_id],
                        ..Default::default()
                    },
                )),
            },
        )
        .expect("submit the current Vorinclex double through normal command handling");
    let charge_offers = after_double
        .events
        .iter()
        .filter_map(|event| match event.ev.as_ref() {
            Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(choice)) => Some(choice),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        charge_offers.len(),
        1,
        "the accepted double emits the independent Charge offer without another loyalty choice"
    );
    let charge_offer = charge_offers[0];
    assert_eq!(charge_offer.deciding_player_id, 0);
    assert_eq!(charge_offer.replacement_options.len(), 2);
    assert_ne!(charge_offer.resolution_prompt_id, 0);
    assert_eq!(
        charge_offer.resolution_prompt_id,
        player_zero_next_prompt_id_before_double
            .checked_add(1)
            .expect("player 0 has room for a Charge prompt ID"),
    );
    assert_eq!(
        engine.state.next_resolution_prompt_id.get(&0).copied(),
        Some(charge_offer.resolution_prompt_id)
    );

    let (mut written_back_loyalty_entry, retained_counter_events) = {
        let Some(PendingReplacementEvent::Counters(counter_batch)) =
            engine.state.pending_replacement_event.as_ref()
        else {
            panic!("the independent Charge replacement remains parked");
        };
        assert_eq!(counter_batch.event_index, 2);
        assert_eq!(counter_batch.events.len(), 3);
        assert_eq!(
            counter_batch.events[2].counters[&CounterKind::Charge],
            5,
            "Charge remains five while its ordering choice is unanswered"
        );
        assert_eq!(
            counter_batch.events[2].original_counters,
            BTreeMap::from([(CounterKind::Charge, 5)])
        );
        assert!(matches!(
            &counter_batch.events[2].recipient,
            CounterPlacementRecipient::EnteringObject {
                object_id,
                zone_change_generation,
            } if *object_id == charge_object && *zone_change_generation == charge_generation
        ));
        let season_application = counter_batch
            .applications
            .iter()
            .find(|application| {
                matches!(
                    &application.replacement,
                    CounterPlacementReplacement::Battlefield {
                        instance,
                        member_event_indices,
                        ..
                    } if instance.kind == CounterReplacementKind::DoublingSeason
                        && instance.source_object_id == season
                        && instance.source_zone_change_generation == season_generation
                        && member_event_indices.as_slice() == [2]
                )
            })
            .expect("the parked Charge row has a current Doubling Season application");
        let charge_double_application = counter_batch
            .applications
            .iter()
            .find(|application| {
                matches!(
                    &application.replacement,
                    CounterPlacementReplacement::Battlefield {
                        instance,
                        member_event_indices,
                        ..
                    } if instance == &double_instance && member_event_indices.as_slice() == [2]
                )
            })
            .expect("the parked Charge row has the same current Vorinclex double ability");
        assert_ne!(
            season_application.choice_id,
            charge_double_application.choice_id
        );
        for application in [season_application, charge_double_application] {
            assert!(charge_offer
                .candidate_object_ids
                .contains(&application.choice_id));
            assert!(charge_offer
                .replacement_options
                .iter()
                .any(|option| option.application_id == application.choice_id));
        }
        let pending = engine
            .state
            .pending_resolution
            .as_ref()
            .expect("the Charge replacement decision remains pending");
        assert_eq!(pending.deciding_player, 0);
        let crate::state::ResolutionContinuation::EntryCounterReplacement { resume } =
            &pending.continuation
        else {
            panic!("the parked Charge choice retains its simultaneous entry continuation");
        };
        let crate::state::EntryCounterReplacementResume::Batch {
            batch: crate::state::SimultaneousEntryBatch::Zone(zone_batch),
            ..
        } = resume.as_ref()
        else {
            panic!("the authoritative resume retains the Zone batch");
        };
        assert_eq!(zone_batch.origin, Zone::Stack);
        assert!(zone_batch.completion.is_none());
        let loyalty_entry = zone_batch
            .ready
            .iter()
            .find(|entry| entry.object_id == entrant)
            .expect("the loyalty entrant remains in the ready batch")
            .clone();
        (loyalty_entry, counter_batch.events.clone())
    };

    assert_eq!(
        retained_counter_events[0].counters[&CounterKind::Loyalty],
        4
    );
    assert_eq!(
        retained_counter_events[1].counters[&CounterKind::Loyalty],
        2
    );
    assert_eq!(
        retained_counter_events[0].original_counters[&CounterKind::Loyalty],
        4
    );
    assert_eq!(
        retained_counter_events[1].original_counters[&CounterKind::Loyalty],
        4
    );
    assert_eq!(
        retained_counter_events[0].counters[&CounterKind::Loyalty]
            + retained_counter_events[1].counters[&CounterKind::Loyalty],
        6
    );
    for (row_index, expected_instance, expected_removed) in
        [(0, &double_instance, 2), (1, &half_instance, 0)]
    {
        let proposal = &retained_counter_events[row_index];
        let context = proposal
            .entry_context
            .as_ref()
            .expect("the parked loyalty proposal retains its entry context");
        assert_eq!(proposal.actor, row_index as PlayerId);
        assert_eq!(
            context.history.baseline_counters,
            BTreeMap::from([(CounterKind::Loyalty, 4)])
        );
        assert_eq!(
            proposal.original_counters,
            BTreeMap::from([(CounterKind::Loyalty, 4)])
        );
        assert_eq!(
            proposal.applied_replacements,
            vec![(*expected_instance).clone()]
        );
        assert_eq!(
            context
                .history
                .receipts
                .iter()
                .filter(|receipt| matches!(
                    receipt,
                    EntryCounterReplacementReceipt::Compleated { .. }
                ))
                .count(),
            1,
            "the intrinsic Compleated receipt is retained once per row"
        );
        let (identity, removed_loyalty) = context
            .history
            .receipts
            .iter()
            .find_map(|receipt| match receipt {
                EntryCounterReplacementReceipt::Compleated {
                    identity,
                    removed_loyalty,
                } => Some((identity, *removed_loyalty)),
                EntryCounterReplacementReceipt::Battlefield { .. } => None,
            })
            .expect("the intrinsic receipt remains attached to the row");
        assert_eq!(removed_loyalty, expected_removed);
        assert_eq!(identity.recipient_object_id, entrant);
        assert_eq!(
            identity.recipient_zone_change_generation,
            entrant_generation
        );
        let battlefield_receipts = context
            .history
            .receipts
            .iter()
            .filter_map(|receipt| match receipt {
                EntryCounterReplacementReceipt::Battlefield { instance } => Some(instance),
                EntryCounterReplacementReceipt::Compleated { .. } => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(battlefield_receipts, vec![expected_instance]);
        assert_eq!(battlefield_receipts[0].source_object_id, vorinclex);
        assert_eq!(
            battlefield_receipts[0].source_zone_change_generation,
            vorinclex_generation
        );
        assert_ne!(battlefield_receipts[0].ability_id, String::new());
        assert!(!context.history.receipts.iter().any(|receipt| matches!(
            receipt,
            EntryCounterReplacementReceipt::Battlefield { instance }
                if instance.kind == CounterReplacementKind::DoublingSeason
        )));
    }
    assert_eq!(
        retained_counter_events[0]
            .entry_context
            .as_ref()
            .unwrap()
            .history
            .receipts
            .last(),
        Some(&EntryCounterReplacementReceipt::Battlefield {
            instance: double_instance.clone()
        })
    );
    assert_eq!(
        retained_counter_events[1]
            .entry_context
            .as_ref()
            .unwrap()
            .history
            .receipts
            .last(),
        Some(&EntryCounterReplacementReceipt::Battlefield {
            instance: half_instance.clone()
        })
    );
    assert_ne!(double_instance.ability_id, half_instance.ability_id);

    GameEngine::apply_entry_counter_placement_results(
        &mut written_back_loyalty_entry,
        &retained_counter_events,
    )
    .expect("write back the retained grouped proposals to a cloned authoritative event");
    assert_eq!(
        written_back_loyalty_entry
            .entry_counter_placements
            .iter()
            .map(|placement| placement.counters[&CounterKind::Loyalty])
            .collect::<Vec<_>>(),
        [4, 2]
    );
    assert_eq!(
        written_back_loyalty_entry.entry_counters[&CounterKind::Loyalty],
        6
    );
    for (row_index, retained_event) in retained_counter_events.iter().enumerate().take(2) {
        assert_eq!(
            written_back_loyalty_entry.entry_counter_placements[row_index]
                .history
                .as_ref(),
            retained_event
                .entry_context
                .as_ref()
                .map(|context| &context.history)
        );
    }
    assert!(written_back_loyalty_entry.entry_counter_replacements_resolved);
    assert_eq!(engine.state.objects[&entrant].zone, Zone::Stack);
    assert_eq!(
        engine
            .state
            .zone_change_generation
            .get(&entrant)
            .copied()
            .unwrap_or(0),
        entrant_generation
    );
    assert_eq!(
        engine
            .state
            .zone_change_generation
            .get(&charge_object)
            .copied()
            .unwrap_or(0),
        charge_generation
    );
    assert!(engine
        .state
        .players
        .iter()
        .all(|player| !player.battlefield.contains(&entrant)));
    assert_eq!(
        engine.state.objects[&stack.item.id].zone,
        Zone::Stack,
        "the prepared-state test makes no cast-completion claim"
    );
    let (mut engine, entrant, stack, vorinclex, charge_object, initial_events) =
        compleated_vorinclex_entry_batch_fixture(2, &[(7, 1)]);
    let stack_generation = engine
        .state
        .zone_change_generation
        .get(&stack.item.id)
        .copied()
        .unwrap_or(0);
    let entrant_generation = engine
        .state
        .zone_change_generation
        .get(&entrant)
        .copied()
        .unwrap_or(0);
    let charge_generation = engine
        .state
        .zone_change_generation
        .get(&charge_object)
        .copied()
        .unwrap_or(0);
    let vorinclex_generation = engine
        .state
        .zone_change_generation
        .get(&vorinclex)
        .copied()
        .unwrap_or(0);
    let season = engine.state.players[0]
        .battlefield
        .iter()
        .copied()
        .find(|object_id| {
            engine
                .state
                .objects
                .get(object_id)
                .is_some_and(|object| object.card_id == "doubling_season")
        })
        .expect("the fresh fixture has an actual Doubling Season controlled by player 0");
    let season_generation = engine
        .state
        .zone_change_generation
        .get(&season)
        .copied()
        .unwrap_or(0);

    let initial_offers = initial_events
        .iter()
        .filter_map(|event| match event.ev.as_ref() {
            Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(choice)) => Some(choice),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(initial_offers.len(), 1);
    let initial_offer = initial_offers[0];
    assert_eq!(initial_offer.deciding_player_id, 1);
    assert_ne!(initial_offer.resolution_prompt_id, 0);
    assert_eq!(initial_offer.replacement_options.len(), 2);

    let (half_application_id, half_instance) = {
        let Some(PendingReplacementEvent::Counters(counter_batch)) =
            engine.state.pending_replacement_event.as_ref()
        else {
            panic!("the clamp case retains its initial replacement proposals");
        };
        assert_eq!(counter_batch.event_index, 0);
        assert_eq!(counter_batch.events.len(), 2);
        assert_eq!(counter_batch.applications.len(), 2);
        assert_eq!(
            counter_batch.events[0].original_counters,
            BTreeMap::from([(CounterKind::Loyalty, 7)])
        );
        assert!(matches!(
            counter_batch.events[0].entry_source.as_ref(),
            Some(EntryCounterSource::ResolutionEffect {
                stack_object_id,
                stack_zone_change_generation,
                effect_index: 0,
            }) if *stack_object_id == stack.item.id
                && *stack_zone_change_generation == stack_generation
        ));
        assert_eq!(
            counter_batch.events[1].original_counters,
            BTreeMap::from([(CounterKind::Charge, 5)])
        );
        let compleated = counter_batch
            .applications
            .iter()
            .find(|application| {
                matches!(
                    &application.replacement,
                    CounterPlacementReplacement::Compleated {
                        member_event_indices,
                        ..
                    } if member_event_indices.as_slice() == [0]
                )
            })
            .expect("the initial offer contains Compleated for the single loyalty row");
        let half = counter_batch
            .applications
            .iter()
            .find(|application| {
                matches!(
                    &application.replacement,
                    CounterPlacementReplacement::Battlefield {
                        instance,
                        member_event_indices,
                        source_controller: Some(0),
                    } if instance.source_object_id == vorinclex
                        && instance.source_zone_change_generation == vorinclex_generation
                        && instance.kind == CounterReplacementKind::HalveCountersOpponentsWouldPut
                        && member_event_indices.as_slice() == [0]
                )
            })
            .expect("Vorinclex half applies to the actor-1 loyalty row");
        assert!(!counter_batch.applications.iter().any(|application| {
            matches!(
                &application.replacement,
                CounterPlacementReplacement::Battlefield { instance, .. }
                    if instance.kind == CounterReplacementKind::DoublingSeason
                        || instance.kind == CounterReplacementKind::DoubleCountersYouWouldPut
            )
        }));
        for application in &counter_batch.applications {
            assert!(initial_offer
                .candidate_object_ids
                .contains(&application.choice_id));
            assert!(initial_offer
                .replacement_options
                .iter()
                .any(|option| option.application_id == application.choice_id));
        }
        assert_ne!(half.choice_id, compleated.choice_id);
        let half_instance = match &half.replacement {
            CounterPlacementReplacement::Battlefield { instance, .. } => instance.clone(),
            CounterPlacementReplacement::Compleated { .. } => unreachable!(),
        };
        (half.choice_id, half_instance)
    };

    let player_zero_next_prompt_id_before_half = engine
        .state
        .next_resolution_prompt_id
        .get(&0)
        .copied()
        .unwrap_or(0);
    let after_half = engine
        .apply_command(
            1,
            &rv1::RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(
                    rv1::SubmitResolutionChoice {
                        resolution_prompt_id: initial_offer.resolution_prompt_id,
                        chosen_object_ids: vec![half_application_id],
                        ..Default::default()
                    },
                )),
            },
        )
        .expect("submit the current Vorinclex half through normal command handling");
    let charge_offers = after_half
        .events
        .iter()
        .filter_map(|event| match event.ev.as_ref() {
            Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(choice)) => Some(choice),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        charge_offers.len(),
        1,
        "the half then automatic Compleated clamp leaves only the Charge choice"
    );
    let charge_offer = charge_offers[0];
    assert_eq!(charge_offer.deciding_player_id, 0);
    assert_ne!(charge_offer.resolution_prompt_id, 0);
    assert_eq!(
        charge_offer.resolution_prompt_id,
        player_zero_next_prompt_id_before_half
            .checked_add(1)
            .expect("player 0 has room for a Charge prompt ID"),
    );
    assert_eq!(
        engine.state.next_resolution_prompt_id.get(&0).copied(),
        Some(charge_offer.resolution_prompt_id)
    );
    assert_eq!(charge_offer.replacement_options.len(), 2);

    let (mut written_back_loyalty_entry, retained_counter_events) = {
        let Some(PendingReplacementEvent::Counters(counter_batch)) =
            engine.state.pending_replacement_event.as_ref()
        else {
            panic!("the clamp case remains parked on independent Charge");
        };
        assert_eq!(counter_batch.event_index, 1);
        assert_eq!(counter_batch.events.len(), 2);
        assert_eq!(counter_batch.applications.len(), 2);
        let loyalty = &counter_batch.events[0];
        assert_eq!(
            loyalty.counters,
            BTreeMap::from([(CounterKind::Loyalty, 0)])
        );
        assert_eq!(
            loyalty.original_counters,
            BTreeMap::from([(CounterKind::Loyalty, 7)])
        );
        let context = loyalty
            .entry_context
            .as_ref()
            .expect("the zero proposal retains entry history");
        assert_eq!(
            context.history.baseline_counters,
            BTreeMap::from([(CounterKind::Loyalty, 7)])
        );
        assert!(context.history.baseline_battlefield_receipts.is_empty());
        assert_eq!(loyalty.applied_replacements, vec![half_instance.clone()]);
        assert_eq!(context.history.receipts.len(), 2);
        assert_eq!(
            context.history.receipts[0],
            EntryCounterReplacementReceipt::Battlefield {
                instance: half_instance.clone()
            }
        );
        let intrinsic = context
            .history
            .receipts
            .iter()
            .filter_map(|receipt| match receipt {
                EntryCounterReplacementReceipt::Compleated {
                    identity,
                    removed_loyalty,
                } => Some((identity, *removed_loyalty)),
                EntryCounterReplacementReceipt::Battlefield { .. } => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(intrinsic.len(), 1);
        assert_eq!(intrinsic[0].0.recipient_object_id, entrant);
        assert_eq!(
            intrinsic[0].0.recipient_zone_change_generation,
            entrant_generation
        );
        assert_eq!(
            intrinsic[0].0.copy_revision,
            engine.state.objects[&entrant].copy_revision
        );
        assert_eq!(intrinsic[0].0.ability_id, "compleated");
        assert_eq!(intrinsic[0].1, 3);
        assert_eq!(
            context.history.receipts[1],
            EntryCounterReplacementReceipt::Compleated {
                identity: intrinsic[0].0.clone(),
                removed_loyalty: 3,
            }
        );
        assert_eq!(
            context
                .history
                .receipts
                .iter()
                .filter_map(|receipt| match receipt {
                    EntryCounterReplacementReceipt::Battlefield { instance } => Some(instance),
                    EntryCounterReplacementReceipt::Compleated { .. } => None,
                })
                .collect::<Vec<_>>(),
            vec![&half_instance]
        );
        assert!(!context.history.receipts.iter().any(|receipt| matches!(
            receipt,
            EntryCounterReplacementReceipt::Battlefield { instance }
                if instance.kind == CounterReplacementKind::DoublingSeason
                    || instance.kind == CounterReplacementKind::DoubleCountersYouWouldPut
        )));
        assert_eq!(
            counter_batch.events[1].counters[&CounterKind::Charge],
            5,
            "the independent Charge ordering choice remains unanswered"
        );
        assert_eq!(
            counter_batch.events[1].original_counters,
            BTreeMap::from([(CounterKind::Charge, 5)])
        );
        assert!(matches!(
            &counter_batch.events[1].recipient,
            CounterPlacementRecipient::EnteringObject {
                object_id,
                zone_change_generation,
            } if *object_id == charge_object && *zone_change_generation == charge_generation
        ));

        let season_application = counter_batch
            .applications
            .iter()
            .find(|application| {
                matches!(
                    &application.replacement,
                    CounterPlacementReplacement::Battlefield {
                        instance,
                        member_event_indices,
                        ..
                    } if instance.kind == CounterReplacementKind::DoublingSeason
                        && instance.source_object_id == season
                        && instance.source_zone_change_generation == season_generation
                        && member_event_indices.as_slice() == [1]
                )
            })
            .expect("the parked Charge row has a current Doubling Season application");
        let charge_double_application = counter_batch
            .applications
            .iter()
            .find(|application| {
                matches!(
                    &application.replacement,
                    CounterPlacementReplacement::Battlefield {
                        instance,
                        member_event_indices,
                        ..
                    } if instance.source_object_id == vorinclex
                        && instance.source_zone_change_generation == vorinclex_generation
                        && instance.kind == CounterReplacementKind::DoubleCountersYouWouldPut
                        && member_event_indices.as_slice() == [1]
                )
            })
            .expect("the parked Charge row has the current Vorinclex double application");
        assert_ne!(
            season_application.choice_id,
            charge_double_application.choice_id
        );
        for application in [season_application, charge_double_application] {
            assert!(charge_offer
                .candidate_object_ids
                .contains(&application.choice_id));
            assert!(charge_offer
                .replacement_options
                .iter()
                .any(|option| option.application_id == application.choice_id));
        }

        let pending = engine
            .state
            .pending_resolution
            .as_ref()
            .expect("the Charge choice remains pending after the automatic clamp");
        assert_eq!(pending.deciding_player, 0);
        let crate::state::ResolutionContinuation::EntryCounterReplacement { resume } =
            &pending.continuation
        else {
            panic!("the clamp case retains its simultaneous batch continuation");
        };
        let crate::state::EntryCounterReplacementResume::Batch {
            batch: crate::state::SimultaneousEntryBatch::Zone(zone_batch),
            ..
        } = resume.as_ref()
        else {
            panic!("the authoritative resume retains the Zone batch");
        };
        assert_eq!(zone_batch.origin, Zone::Stack);
        assert!(zone_batch.completion.is_none());
        let loyalty_entry = zone_batch
            .ready
            .iter()
            .find(|entry| entry.object_id == entrant)
            .expect("the clamped entrant remains in the ready batch")
            .clone();
        (loyalty_entry, counter_batch.events.clone())
    };

    GameEngine::apply_entry_counter_placement_results(
        &mut written_back_loyalty_entry,
        &retained_counter_events,
    )
    .expect("write back the clamped proposal to a cloned authoritative event");
    assert_eq!(
        written_back_loyalty_entry.entry_counter_placements.len(),
        1,
        "the zero placement row is retained"
    );
    assert_eq!(
        written_back_loyalty_entry.entry_counter_placements[0]
            .counters
            .get(&CounterKind::Loyalty),
        Some(&0),
        "the zero placement keeps its explicit Loyalty key"
    );
    assert_eq!(
        written_back_loyalty_entry.entry_counter_placements[0]
            .history
            .as_ref()
            .unwrap()
            .baseline_counters,
        BTreeMap::from([(CounterKind::Loyalty, 7)])
    );
    assert_eq!(
        written_back_loyalty_entry.entry_counter_placements[0]
            .history
            .as_ref()
            .unwrap()
            .receipts,
        retained_counter_events[0]
            .entry_context
            .as_ref()
            .unwrap()
            .history
            .receipts
    );
    assert_eq!(
        written_back_loyalty_entry
            .entry_counters
            .get(&CounterKind::Loyalty)
            .copied()
            .unwrap_or(0),
        0,
        "the aggregate projection reads zero even if it omits the key"
    );
    assert!(written_back_loyalty_entry.entry_counter_replacements_resolved);
    assert_eq!(engine.state.objects[&entrant].zone, Zone::Stack);
    assert_eq!(
        engine
            .state
            .zone_change_generation
            .get(&entrant)
            .copied()
            .unwrap_or(0),
        entrant_generation
    );
    assert_eq!(
        engine
            .state
            .zone_change_generation
            .get(&charge_object)
            .copied()
            .unwrap_or(0),
        charge_generation
    );
    assert!(engine
        .state
        .players
        .iter()
        .all(|player| !player.battlefield.contains(&entrant)));
    assert_eq!(
        engine.state.objects[&stack.item.id].zone,
        Zone::Stack,
        "the prepared-state test makes no cast-completion claim"
    );
}

fn compleated_grouped_entry_fixture(
    engine: &mut GameEngine,
    stack: &ParkedStackResolution,
) -> (ObjectId, BattlefieldEntryEvent, Vec<ObjectId>, u64) {
    let seasons = (0..3)
        .map(|_| {
            let season = object(engine, "doubling_season", Zone::Battlefield, 0);
            assert_eq!(
                engine
                    .state
                    .objects
                    .get(&season)
                    .expect("registered Season object")
                    .zone,
                Zone::Battlefield
            );
            engine.emit_static_abilities_on_enter(season);
            season
        })
        .collect::<Vec<_>>();
    assert_eq!(seasons.len(), 3);
    assert_ne!(seasons[0], seasons[1]);
    assert_ne!(seasons[0], seasons[2]);
    assert_ne!(seasons[1], seasons[2]);

    let entrant = object(engine, "jace_beleren", Zone::Stack, 0);
    let entrant_generation = engine
        .state
        .zone_change_generation
        .get(&entrant)
        .copied()
        .unwrap_or(0);
    assert_eq!(
        engine
            .state
            .objects
            .get(&entrant)
            .expect("entrant object exists")
            .zone,
        Zone::Stack
    );
    let mut face = engine
        .registry
        .get("jace_beleren")
        .expect("registered ordinary planeswalker")
        .primary_face()
        .clone();
    assert!(face.static_abilities.iter().any(|ability| matches!(
        &ability.definition,
        StaticAbilityDef::EntersWithCounters {
            counter: CounterKind::Loyalty,
            ..
        }
    )));
    face.static_abilities.retain(|ability| {
        !matches!(
            &ability.definition,
            StaticAbilityDef::EntersWithCounters { .. }
        )
    });
    face.loyalty = None;

    let compleated_ability = CardRegistry::from_chunks_and_tokens(
        &[r#"(
            id: "compleated_epoch_entry_probe",
            name: "Compleated Epoch Entry Probe",
            face_id: "compleated_epoch_entry_probe",
            mana_cost: "{4}{B}{B/P}",
            supertypes: ["Legendary"],
            types: ["Planeswalker", "Vraska"],
            loyalty: Some(6),
            static_abilities: [(
                ability_id: "compleated",
                presentation: Fallback,
                definition: Compleated,
            )],
        )"#],
        &[],
    )
    .expect("valid test-only Compleated face")
    .get("compleated_epoch_entry_probe")
    .expect("test-only Compleated face is registered")
    .primary_face()
    .static_abilities
    .iter()
    .find(|ability| matches!(&ability.definition, StaticAbilityDef::Compleated))
    .expect("test-only face has Compleated")
    .clone();
    face.static_abilities.push(compleated_ability);
    let display_name = face.name.clone();
    engine
        .state
        .objects
        .get_mut(&entrant)
        .expect("entrant object exists")
        .copiable_values = Some(CopiableValues {
        prototyped: false,
        source_card_id: "jace_beleren".into(),
        source_face_index: 0,
        display_name,
        face,
        room_faces: None,
    });

    let mut entry = event(engine, entrant);
    entry.phyrexian_symbols_paid_with_life = 1;
    let stack_zone_change_generation = engine
        .state
        .zone_change_generation
        .get(&stack.item.id)
        .copied()
        .unwrap_or(0);
    for (effect_index, count) in [1_u32, 5].into_iter().enumerate() {
        append_entry_counter_placement(
            &mut entry,
            0,
            [(CounterKind::Loyalty, count)],
            EntryCounterSource::ResolutionEffect {
                stack_object_id: stack.item.id,
                stack_zone_change_generation,
                effect_index: effect_index as u32,
            },
            false,
        );
    }
    assert_eq!(entry.entry_counter_placements.len(), 2);
    assert_eq!(
        entry.entry_counter_placements[0]
            .counters
            .get(&CounterKind::Loyalty),
        Some(&1)
    );
    assert_eq!(
        entry.entry_counter_placements[1]
            .counters
            .get(&CounterKind::Loyalty),
        Some(&5)
    );
    assert!(engine.battlefield_entry_face(&entry).is_some_and(|face| {
        face.static_abilities
            .iter()
            .any(|ability| matches!(&ability.definition, StaticAbilityDef::Compleated))
    }));
    (entrant, entry, seasons, entrant_generation)
}

// Keep each parked-prompt expectation explicit so fixtures expose the exact offer and current counters.
#[allow(clippy::too_many_arguments)]
fn assert_compleated_grouped_entry_prompt(
    engine: &GameEngine,
    entrant: ObjectId,
    entrant_generation: u64,
    all_seasons: &[ObjectId],
    current_seasons: &[ObjectId],
    expected_stack_debug: &str,
    offer: &rv1::ResolutionChoiceRequired,
    expect_compleated: bool,
    expected_current_counters: [u32; 2],
) -> (u64, Option<u32>, Vec<(u32, ObjectId)>) {
    assert_eq!(offer.choice_kind, rv1::ChoiceKind::ReplacementEffect as i32);
    assert_ne!(offer.resolution_prompt_id, 0);
    assert_eq!(
        offer.candidate_object_ids.len(),
        offer.replacement_options.len()
    );

    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("the entry-counter choice remains parked");
    assert_eq!(pending.deciding_player, 0);
    assert_eq!(
        pending.resolution_prompt_id.get(),
        offer.resolution_prompt_id
    );
    let parked_stack = pending
        .continuation
        .stack()
        .expect("the entry-counter continuation retains its parked stack");
    assert_eq!(format!("{parked_stack:?}"), expected_stack_debug);
    assert_eq!(
        engine
            .state
            .objects
            .get(&entrant)
            .expect("entrant remains present")
            .zone,
        Zone::Stack
    );
    assert_eq!(
        engine
            .state
            .zone_change_generation
            .get(&entrant)
            .copied()
            .unwrap_or(0),
        entrant_generation
    );

    let counter_batch = match engine.state.pending_replacement_event.as_ref() {
        Some(PendingReplacementEvent::Counters(batch)) => batch,
        _ => panic!("the entry-counter offer retains its current counter batch"),
    };
    let entry_rows = counter_batch
        .events
        .iter()
        .filter(|proposal| {
            matches!(
                &proposal.recipient,
                CounterPlacementRecipient::EnteringObject { object_id, .. }
                    if *object_id == entrant
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(entry_rows.len(), 2);
    assert_eq!(
        entry_rows
            .iter()
            .map(|proposal| {
                proposal
                    .original_counters
                    .get(&CounterKind::Loyalty)
                    .copied()
                    .unwrap_or(0)
            })
            .collect::<Vec<_>>(),
        vec![1_u32, 5_u32]
    );
    assert_eq!(
        entry_rows
            .iter()
            .map(|proposal| {
                proposal
                    .counters
                    .get(&CounterKind::Loyalty)
                    .copied()
                    .unwrap_or(0)
            })
            .collect::<Vec<_>>(),
        expected_current_counters.to_vec()
    );
    for proposal in entry_rows {
        assert!(matches!(
            &proposal.origin,
            super::continuous::CounterPlacementOrigin::Entry
        ));
        let context = proposal
            .entry_context
            .as_ref()
            .expect("each loyalty row retains its actual entry history");
        if expect_compleated {
            assert!(
                context.history.receipts.is_empty(),
                "the initial offer has not recorded replacement receipts"
            );
        } else {
            assert!(
                !context.history.receipts.is_empty(),
                "the applied Compleated replacement retains real intrinsic receipts"
            );
        }
        assert!(context.compleated.is_some());
    }

    let compleated_options = offer
        .replacement_options
        .iter()
        .filter(|option| {
            option
                .effect_summary
                .to_ascii_lowercase()
                .contains("loyalty")
        })
        .collect::<Vec<_>>();
    let expected_compleated_count = if expect_compleated { 1 } else { 0 };
    assert_eq!(compleated_options.len(), expected_compleated_count);
    let compleated_handle = compleated_options
        .first()
        .map(|option| option.application_id);
    if let Some(handle) = compleated_handle {
        assert!(offer.candidate_object_ids.contains(&handle));
    }

    let mut season_handles = Vec::new();
    for &season in all_seasons {
        let source = engine
            .state
            .objects
            .get(&season)
            .expect("registered Doubling Season source exists");
        assert_eq!(source.zone, Zone::Battlefield);
        let generation = engine
            .state
            .zone_change_generation
            .get(&season)
            .copied()
            .unwrap_or(0);
        let application = counter_batch.applications.iter().find(|application| {
            matches!(
                &application.replacement,
                CounterPlacementReplacement::Battlefield { instance, .. }
                    if instance.source_object_id == season
                        && instance.source_zone_change_generation == generation
            )
        });
        if current_seasons.contains(&season) {
            let application = application.expect("current Season source has a fresh application");
            assert!(offer.candidate_object_ids.contains(&application.choice_id));
            let option = offer
                .replacement_options
                .iter()
                .find(|option| option.application_id == application.choice_id)
                .expect("current Season application is engine-offered");
            assert!(option.effect_summary.contains("doubles effect counters"));
            season_handles.push((application.choice_id, season));
        } else {
            assert!(
                application.is_none(),
                "a consumed Season application is not reused in the new epoch"
            );
        }
    }
    assert_eq!(season_handles.len(), current_seasons.len());
    assert_eq!(
        offer.replacement_options.len(),
        current_seasons.len() + expected_compleated_count
    );
    assert_eq!(
        offer.candidate_object_ids.len(),
        current_seasons.len() + expected_compleated_count
    );
    let prompt_id = pending.resolution_prompt_id.get();
    (prompt_id, compleated_handle, season_handles)
}

// Keep rejection inputs and expected parked-state fields explicit to show the fixture's atomicity contract.
#[allow(clippy::too_many_arguments)]
fn assert_rejected_compleated_grouped_choice_atomically(
    engine: &mut GameEngine,
    actor: PlayerId,
    answer: rv1::SubmitResolutionChoice,
    entrant: ObjectId,
    entrant_generation: u64,
    all_seasons: &[ObjectId],
    current_seasons: &[ObjectId],
    expected_stack_debug: &str,
    current_offer: &rv1::ResolutionChoiceRequired,
    expect_compleated: bool,
    expected_current_counters: [u32; 2],
) {
    let before = engine
        .diagnostic_snapshot()
        .expect("capture the complete parked-state snapshot");
    let command_index = engine.state.command_index;
    let command = rv1::RuledCommand {
        cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(answer)),
    };
    assert!(
        engine.apply_command(actor, &command).is_err(),
        "invalid resolution-choice command is rejected"
    );
    assert_eq!(engine.state.command_index, command_index);
    assert_eq!(
        engine
            .diagnostic_snapshot()
            .expect("snapshot after rejected resolution choice"),
        before
    );
    let _ = assert_compleated_grouped_entry_prompt(
        engine,
        entrant,
        entrant_generation,
        all_seasons,
        current_seasons,
        expected_stack_debug,
        current_offer,
        expect_compleated,
        expected_current_counters,
    );
}

#[test]
fn compleated_grouped_entry_rejects_wrong_actor_stale_epoch_and_old_or_foreign_handles_atomically()
{
    for (batch_continuation, program_stack) in
        [(false, false), (false, true), (true, false), (true, true)]
    {
        // The registered Program donor supplies continuation state only; this prepared test stops before resume.
        let (mut engine, stack) = if program_stack {
            let (engine, mut stack, _, refs) = suspended_conductive_machete_manifest_dread();
            assert_eq!(
                format!("{:?}", stack.previous_result()),
                format!("{:?}", crate::state::OperationResults::default()),
                "the real Manifest Dread pause has no previous Program result"
            );
            let manifest_ref = refs[0];
            let object = engine
                .state
                .objects
                .get(&manifest_ref.object_id)
                .expect("the exact looked-at Manifest Dread object remains present");
            assert_eq!(object.zone, Zone::Library);
            let current_generation = engine
                .state
                .zone_change_generation
                .get(&manifest_ref.object_id)
                .copied()
                .unwrap_or(0);
            assert_eq!(current_generation, manifest_ref.zone_change_generation);
            // Prepared-retention sentinel only; this is not an actual Program output.
            let mut results = stack.previous_result().clone();
            results
                .append(crate::state::ProgramValue::PermanentSelection(vec![
                    TriggerObjectRef {
                        object_id: manifest_ref.object_id,
                        zone_change_generation: manifest_ref.zone_change_generation,
                        controller_at_event: object.controller,
                    },
                ]))
                .expect("typed retention sentinel");
            stack.replace_completed_results(results);
            assert_eq!(stack.previous_result().object_refs().len(), 1);
            (engine, stack)
        } else {
            let mut engine = engine();
            let stack = paid_divination_stack(&mut engine);
            (engine, stack)
        };
        let (entrant, entry, seasons, entrant_generation) =
            compleated_grouped_entry_fixture(&mut engine, &stack);
        let expected_stack_debug = format!("{stack:?}");
        assert_ne!(
            format!("{:?}", stack.previous_result()),
            format!("{:?}", crate::state::OperationResults::default()),
            "both parked-stack donors carry a nonempty previous result"
        );
        if program_stack {
            assert!(
                stack
                    .program()
                    .is_some_and(|program| !program.frames.is_empty()),
                "the registered Program donor has a real suspended frame"
            );
        }
        let command_index_before_prepare = engine.state.command_index;
        let mut events = Vec::new();
        if batch_continuation {
            let non_counter_cohort_member = object(&mut engine, "grizzly_bears", Zone::Stack, 0);
            let ready = vec![entry, event(&engine, non_counter_cohort_member)];
            let generations = ready
                .iter()
                .map(|entry| {
                    let entrant_object = engine
                        .state
                        .objects
                        .get(&entry.object_id)
                        .expect("prepared entrant object exists");
                    assert_eq!(entrant_object.zone, Zone::Stack);
                    (
                        entry.object_id,
                        engine
                            .state
                            .zone_change_generation
                            .get(&entry.object_id)
                            .copied()
                            .unwrap_or(0),
                    )
                })
                .collect::<Vec<_>>();
            assert_eq!(generations[0].1, entrant_generation);
            let origin_mana_values = generations
                .iter()
                .filter_map(|(object_id, generation)| {
                    engine.characteristics(*object_id).map(|characteristics| {
                        (*object_id, *generation, characteristics.mana_value)
                    })
                })
                .collect();
            let batch =
                crate::state::SimultaneousEntryBatch::Zone(crate::state::PendingZoneEntryBatch {
                    ready,
                    remaining: Vec::new(),
                    generations,
                    origin_mana_values,
                    origin: Zone::Stack,
                    spell_label: "Prepared Compleated grouped entry".into(),
                    completion: None,
                });
            assert!(engine
                .begin_entry_counter_placements_for_batch(batch, Some(stack.clone()), &mut events,)
                .expect("prepare the grouped Compleated entry batch")
                .is_none());
        } else {
            let completion = crate::state::BattlefieldEntryCompletion::ResolutionEffect {
                owner: 0,
                spell_label: "Prepared Compleated grouped entry".into(),
                object_label: "Compleated Entry Probe".into(),
                from_zone: Zone::Stack,
                triggered_return_generation: None,
            };
            assert!(engine
                .prepare_single_entry_counter_placements(
                    stack.clone(),
                    entry,
                    completion,
                    &mut events,
                )
                .expect("prepare the grouped Compleated single entry")
                .is_none());
        }
        engine
            .publish_resolution_prompt_ids(&mut events)
            .expect("publish the initial replacement prompt ID");
        let first_offer = events
            .iter()
            .find_map(|event| match event.ev.as_ref() {
                Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(offer)) => Some(offer.clone()),
                _ => None,
            })
            .expect("the initial replacement order is engine-offered");
        let (first_prompt_id, first_compleated_handle, first_season_handles) =
            assert_compleated_grouped_entry_prompt(
                &engine,
                entrant,
                entrant_generation,
                &seasons,
                &seasons,
                &expected_stack_debug,
                &first_offer,
                true,
                [1, 5],
            );
        let first_compleated_handle =
            first_compleated_handle.expect("the initial offer includes Compleated");
        assert_eq!(first_season_handles.len(), 3);
        assert_eq!(engine.state.command_index, command_index_before_prepare);

        assert_rejected_compleated_grouped_choice_atomically(
            &mut engine,
            1,
            rv1::SubmitResolutionChoice {
                resolution_prompt_id: first_prompt_id,
                chosen_object_ids: vec![first_compleated_handle],
                ..Default::default()
            },
            entrant,
            entrant_generation,
            &seasons,
            &seasons,
            &expected_stack_debug,
            &first_offer,
            true,
            [1, 5],
        );

        let command_index_before_compleated = engine.state.command_index;
        let compleated_resume = engine
            .apply_command(
                0,
                &rv1::RuledCommand {
                    cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(
                        rv1::SubmitResolutionChoice {
                            resolution_prompt_id: first_prompt_id,
                            chosen_object_ids: vec![first_compleated_handle],
                            ..Default::default()
                        },
                    )),
                },
            )
            .expect("accept the current engine-offered Compleated application");
        assert_eq!(
            engine.state.command_index,
            command_index_before_compleated + 1,
            "one accepted command advances command_index exactly once"
        );
        let second_offer = compleated_resume
            .events
            .iter()
            .find_map(|event| match event.ev.as_ref() {
                Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(offer)) => Some(offer.clone()),
                _ => None,
            })
            .expect("three fresh Season applications repark the entry");
        let (second_prompt_id, second_compleated_handle, second_season_handles) =
            assert_compleated_grouped_entry_prompt(
                &engine,
                entrant,
                entrant_generation,
                &seasons,
                &seasons,
                &expected_stack_debug,
                &second_offer,
                false,
                [0, 4],
            );
        assert_ne!(second_prompt_id, first_prompt_id);
        assert!(second_compleated_handle.is_none());
        assert_eq!(second_season_handles.len(), 3);
        assert!(second_offer
            .candidate_object_ids
            .iter()
            .all(|handle| !first_offer.candidate_object_ids.contains(handle)));
        assert!(!second_offer
            .candidate_object_ids
            .contains(&first_compleated_handle));
        assert!(
            !second_offer
                .candidate_object_ids
                .contains(&first_season_handles[0].0),
            "the prior Season handle is foreign to the fresh offer"
        );

        assert_rejected_compleated_grouped_choice_atomically(
            &mut engine,
            0,
            rv1::SubmitResolutionChoice {
                resolution_prompt_id: first_prompt_id,
                chosen_object_ids: vec![second_season_handles[0].0],
                ..Default::default()
            },
            entrant,
            entrant_generation,
            &seasons,
            &seasons,
            &expected_stack_debug,
            &second_offer,
            false,
            [0, 4],
        );
        assert_rejected_compleated_grouped_choice_atomically(
            &mut engine,
            0,
            rv1::SubmitResolutionChoice {
                resolution_prompt_id: second_prompt_id,
                chosen_object_ids: vec![first_season_handles[0].0],
                ..Default::default()
            },
            entrant,
            entrant_generation,
            &seasons,
            &seasons,
            &expected_stack_debug,
            &second_offer,
            false,
            [0, 4],
        );
        assert_rejected_compleated_grouped_choice_atomically(
            &mut engine,
            0,
            rv1::SubmitResolutionChoice {
                resolution_prompt_id: second_prompt_id,
                chosen_object_ids: vec![first_compleated_handle],
                ..Default::default()
            },
            entrant,
            entrant_generation,
            &seasons,
            &seasons,
            &expected_stack_debug,
            &second_offer,
            false,
            [0, 4],
        );

        let accepted_season = second_season_handles[0];
        let remaining_seasons = second_season_handles
            .iter()
            .filter(|(_, season)| *season != accepted_season.1)
            .map(|(_, season)| *season)
            .collect::<Vec<_>>();
        assert_eq!(remaining_seasons.len(), 2);
        let command_index_before_season = engine.state.command_index;
        let season_resume = engine
            .apply_command(
                0,
                &rv1::RuledCommand {
                    cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(
                        rv1::SubmitResolutionChoice {
                            resolution_prompt_id: second_prompt_id,
                            chosen_object_ids: vec![accepted_season.0],
                            ..Default::default()
                        },
                    )),
                },
            )
            .expect("accept one current Doubling Season application");
        assert_eq!(
            engine.state.command_index,
            command_index_before_season + 1,
            "one accepted Season command advances command_index exactly once"
        );
        let third_offer = season_resume
            .events
            .iter()
            .find_map(|event| match event.ev.as_ref() {
                Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(offer)) => Some(offer.clone()),
                _ => None,
            })
            .expect("two fresh Season choices remain parked");
        let (third_prompt_id, third_compleated_handle, third_season_handles) =
            assert_compleated_grouped_entry_prompt(
                &engine,
                entrant,
                entrant_generation,
                &seasons,
                &remaining_seasons,
                &expected_stack_debug,
                &third_offer,
                false,
                [0, 8],
            );
        assert_ne!(third_prompt_id, second_prompt_id);
        assert_ne!(third_prompt_id, first_prompt_id);
        assert!(third_compleated_handle.is_none());
        assert_eq!(third_season_handles.len(), 2);
        assert!(third_offer
            .candidate_object_ids
            .iter()
            .all(|handle| !second_offer.candidate_object_ids.contains(handle)));
        assert_eq!(
            engine.state.command_index,
            command_index_before_prepare + 2,
            "the two accepted commands are the only command-index advances"
        );
        assert!(engine.state.pending_resolution.is_some());
        assert!(engine.state.pending_replacement_event.is_some());
        // Stop at the still-parked prepared state; this case does not finish the synthetic entry.
        assert_eq!(
            engine
                .state
                .objects
                .get(&entrant)
                .expect("entrant remains present")
                .zone,
            Zone::Stack,
            "stop at the prepared-state boundary with two Season choices parked"
        );
    }
}

#[derive(Clone, Copy, Debug)]
enum CompleatedIdentityInvalidation {
    CopyRevision,
    ZoneChangeGeneration,
    RemoveCompleated,
    ChangeCompleatedAbilityId,
    VorinclexController,
    ResumedEventDestination,
}

impl CompleatedIdentityInvalidation {
    fn label(self) -> &'static str {
        match self {
            Self::CopyRevision => "entrant copy revision",
            Self::ZoneChangeGeneration => "entrant zone-change generation",
            Self::RemoveCompleated => "effective Compleated removal",
            Self::ChangeCompleatedAbilityId => "stable Compleated ability ID",
            Self::VorinclexController => "Vorinclex controller and member rows",
            Self::ResumedEventDestination => "authoritative resumed-event destination",
        }
    }
}

fn compleated_identity_probe_object(engine: &mut GameEngine, controller: PlayerId) -> ObjectId {
    let entrant = object(engine, "jace_beleren", Zone::Stack, controller);
    let mut face = engine
        .registry
        .get("jace_beleren")
        .expect("registered Jace fixture face")
        .primary_face()
        .clone();
    face.static_abilities.retain(|ability| {
        !matches!(
            &ability.definition,
            StaticAbilityDef::EntersWithCounters { .. }
        )
    });
    face.loyalty = None;
    let compleated = CardRegistry::from_chunks_and_tokens(
        &[r#"(
            id: "compleated_grouped_identity_probe",
            name: "Compleated Grouped Identity Probe",
            face_id: "compleated_grouped_identity_probe",
            mana_cost: "{4}{B}{B/P}",
            supertypes: ["Legendary"],
            types: ["Planeswalker", "Vraska"],
            loyalty: Some(6),
            static_abilities: [(
                ability_id: "compleated",
                presentation: Fallback,
                definition: Compleated,
            )],
        )"#],
        &[],
    )
    .expect("valid test-only Compleated identity face")
    .get("compleated_grouped_identity_probe")
    .expect("test-only Compleated identity face is registered")
    .primary_face()
    .static_abilities
    .iter()
    .find(|ability| matches!(&ability.definition, StaticAbilityDef::Compleated))
    .expect("test-only face has Compleated")
    .clone();
    face.static_abilities.push(compleated);
    let display_name = face.name.clone();
    engine
        .state
        .objects
        .get_mut(&entrant)
        .expect("entrant object exists")
        .copiable_values = Some(CopiableValues {
        prototyped: false,
        source_card_id: "jace_beleren".into(),
        source_face_index: 0,
        display_name,
        face,
        room_faces: None,
    });
    entrant
}

fn compleated_identity_entry(
    engine: &GameEngine,
    stack: &ParkedStackResolution,
    entrant: ObjectId,
    deciding_player: PlayerId,
    destination_controller: PlayerId,
    phyrexian_symbols_paid_with_life: u32,
    rows: &[(u32, PlayerId)],
) -> BattlefieldEntryEvent {
    let mut entry = event(engine, entrant);
    entry.deciding_player = deciding_player;
    entry.destination_controller = destination_controller;
    entry.phyrexian_symbols_paid_with_life = phyrexian_symbols_paid_with_life;
    let stack_generation = engine
        .state
        .zone_change_generation
        .get(&stack.item.id)
        .copied()
        .unwrap_or(0);
    for (effect_index, &(count, actor)) in rows.iter().enumerate() {
        append_entry_counter_placement(
            &mut entry,
            actor,
            [(CounterKind::Loyalty, count)],
            EntryCounterSource::ResolutionEffect {
                stack_object_id: stack.item.id,
                stack_zone_change_generation: stack_generation,
                effect_index: effect_index as u32,
            },
            false,
        );
    }
    entry
}

fn compleated_identity_zone_batch(
    engine: &GameEngine,
    ready: Vec<BattlefieldEntryEvent>,
) -> crate::state::SimultaneousEntryBatch {
    let generations = ready
        .iter()
        .map(|entry| {
            (
                entry.object_id,
                engine
                    .state
                    .zone_change_generation
                    .get(&entry.object_id)
                    .copied()
                    .unwrap_or(0),
            )
        })
        .collect::<Vec<_>>();
    let origin_mana_values = generations
        .iter()
        .filter_map(|(object_id, generation)| {
            engine
                .characteristics(*object_id)
                .map(|characteristics| (*object_id, *generation, characteristics.mana_value))
        })
        .collect();
    crate::state::SimultaneousEntryBatch::Zone(crate::state::PendingZoneEntryBatch {
        ready,
        remaining: Vec::new(),
        generations,
        origin_mana_values,
        origin: Zone::Stack,
        spell_label: "Prepared Compleated identity entry".into(),
        completion: None,
    })
}

fn identity_choice_offer(events: &[rv1::RuledEvent]) -> rv1::ResolutionChoiceRequired {
    events
        .iter()
        .find_map(|event| match event.ev.as_ref() {
            Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(offer)) => Some(offer.clone()),
            _ => None,
        })
        .expect("engine publishes the current replacement choice")
}

fn identity_compleated_choice(engine: &GameEngine) -> (u64, u32) {
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("Compleated parks its competing replacement choice");
    let Some(PendingReplacementEvent::Counters(batch)) =
        engine.state.pending_replacement_event.as_ref()
    else {
        panic!("the Compleated offer retains its counter batch");
    };
    let application = batch
        .applications
        .iter()
        .find(|application| {
            matches!(
                &application.replacement,
                CounterPlacementReplacement::Compleated { .. }
            )
        })
        .expect("Compleated is a current application");
    assert_eq!(application.event_index, batch.event_index);
    assert!(pending
        .presentation
        .candidates
        .contains(&application.choice_id));
    (pending.resolution_prompt_id.get(), application.choice_id)
}

fn identity_vorin_double_choice(engine: &GameEngine, vorinclex: ObjectId) -> (u64, u32) {
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("Vorinclex leaves a competing replacement choice");
    let Some(PendingReplacementEvent::Counters(batch)) =
        engine.state.pending_replacement_event.as_ref()
    else {
        panic!("the Vorinclex offer retains its current counter batch");
    };
    let application = batch
        .applications
        .iter()
        .find(|application| {
            matches!(
                &application.replacement,
                CounterPlacementReplacement::Battlefield { instance, .. }
                    if instance.source_object_id == vorinclex
                        && instance.kind == CounterReplacementKind::DoubleCountersYouWouldPut
            )
        })
        .expect("current offer includes Vorinclex double for actor 0");
    assert_eq!(application.event_index, batch.event_index);
    assert!(pending
        .presentation
        .candidates
        .contains(&application.choice_id));
    (pending.resolution_prompt_id.get(), application.choice_id)
}

fn submit_identity_choice(prompt_id: u64, application_id: u32) -> rv1::RuledCommand {
    rv1::RuledCommand {
        cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(
            rv1::SubmitResolutionChoice {
                resolution_prompt_id: prompt_id,
                chosen_object_ids: vec![application_id],
                ..Default::default()
            },
        )),
    }
}

fn prepare_compleated_identity_vorin_choice(
    batch_continuation: bool,
) -> (
    GameEngine,
    ObjectId,
    ObjectId,
    rv1::ResolutionChoiceRequired,
    u32,
) {
    let mut engine = engine();
    let stack = paid_divination_stack(&mut engine);
    let entrant = compleated_identity_probe_object(&mut engine, 0);
    let vorinclex = object(
        &mut engine,
        "vorinclex,_monstrous_raider",
        Zone::Battlefield,
        0,
    );
    engine.emit_static_abilities_on_enter(vorinclex);
    let entry = compleated_identity_entry(&engine, &stack, entrant, 0, 0, 1, &[(4, 0), (4, 1)]);
    let mut events = Vec::new();
    if batch_continuation {
        let batch = compleated_identity_zone_batch(&engine, vec![entry]);
        assert!(engine
            .begin_entry_counter_placements_for_batch(batch, Some(stack.clone()), &mut events,)
            .expect("prepare the one-entrant identity Batch")
            .is_none());
    } else {
        let completion = crate::state::BattlefieldEntryCompletion::ResolutionEffect {
            owner: 0,
            spell_label: "Prepared Compleated identity entry".into(),
            object_label: "Compleated Grouped Identity Probe".into(),
            from_zone: Zone::Stack,
            triggered_return_generation: None,
        };
        assert!(engine
            .prepare_single_entry_counter_placements(stack, entry, completion, &mut events,)
            .expect("prepare the Single identity entry")
            .is_none());
    }
    engine
        .publish_resolution_prompt_ids(&mut events)
        .expect("publish the initial Compleated choice");
    let initial_offer = identity_choice_offer(&events);
    let (initial_prompt_id, compleated_handle) = identity_compleated_choice(&engine);
    assert_eq!(initial_offer.resolution_prompt_id, initial_prompt_id);
    assert_eq!(initial_offer.deciding_player_id, 0);
    assert!(initial_offer
        .candidate_object_ids
        .contains(&compleated_handle));

    let command_index = engine.state.command_index;
    let after_compleated = engine
        .apply_command(
            0,
            &submit_identity_choice(initial_prompt_id, compleated_handle),
        )
        .expect("accept Compleated through the normal command route");
    assert_eq!(engine.state.command_index, command_index + 1);
    let current_offer = identity_choice_offer(&after_compleated.events);
    let (prompt_id, double_handle) = identity_vorin_double_choice(&engine, vorinclex);
    assert_eq!(current_offer.resolution_prompt_id, prompt_id);
    assert_eq!(current_offer.deciding_player_id, 0);

    let Some(PendingReplacementEvent::Counters(batch)) =
        engine.state.pending_replacement_event.as_ref()
    else {
        panic!("the current Vorinclex choice retains the counter batch");
    };
    let rows = batch
        .events
        .iter()
        .filter(|proposal| {
            matches!(
                &proposal.recipient,
                CounterPlacementRecipient::EnteringObject { object_id, .. }
                    if *object_id == entrant
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(rows.len(), 2);
    assert_eq!(
        rows.iter()
            .map(|proposal| proposal.counters[&CounterKind::Loyalty])
            .collect::<Vec<_>>(),
        [2, 4]
    );
    for row in rows {
        let context = row
            .entry_context
            .as_ref()
            .expect("each row keeps its captured entry context");
        assert!(
            context.history.receipts.iter().any(|receipt| matches!(
                receipt,
                EntryCounterReplacementReceipt::Compleated { .. }
            )),
            "accepted Compleated leaves an actual intrinsic receipt on every row"
        );
    }
    (engine, entrant, vorinclex, current_offer, double_handle)
}

fn invalidate_compleated_identity_fixture(
    engine: &mut GameEngine,
    entrant: ObjectId,
    vorinclex: ObjectId,
    batch_continuation: bool,
    invalidation: CompleatedIdentityInvalidation,
) {
    match invalidation {
        CompleatedIdentityInvalidation::CopyRevision => {
            let object = engine
                .state
                .objects
                .get_mut(&entrant)
                .expect("current entrant exists");
            object.copy_revision = object
                .copy_revision
                .checked_add(1)
                .expect("copy revision has room");
        }
        CompleatedIdentityInvalidation::ZoneChangeGeneration => {
            let generation = engine
                .state
                .zone_change_generation
                .get(&entrant)
                .copied()
                .unwrap_or(0);
            engine.state.zone_change_generation.insert(
                entrant,
                generation
                    .checked_add(1)
                    .expect("zone-change generation has room"),
            );
        }
        CompleatedIdentityInvalidation::RemoveCompleated => {
            let object = engine
                .state
                .objects
                .get_mut(&entrant)
                .expect("current entrant exists");
            let copy_revision = object.copy_revision;
            let face = &mut object
                .copiable_values
                .as_mut()
                .expect("prepared entrant has a copiable face")
                .face;
            face.static_abilities
                .retain(|ability| !matches!(&ability.definition, StaticAbilityDef::Compleated));
            assert_eq!(object.copy_revision, copy_revision);
        }
        CompleatedIdentityInvalidation::ChangeCompleatedAbilityId => {
            let object = engine
                .state
                .objects
                .get_mut(&entrant)
                .expect("current entrant exists");
            let copy_revision = object.copy_revision;
            let face = &mut object
                .copiable_values
                .as_mut()
                .expect("prepared entrant has a copiable face")
                .face;
            let ability = face
                .static_abilities
                .iter_mut()
                .find(|ability| matches!(&ability.definition, StaticAbilityDef::Compleated))
                .expect("the prepared ability remains present");
            ability.ability_id = tricerules_card_model::AbilityId::new("compleated_changed")
                .expect("valid replacement ability ID");
            assert_eq!(object.copy_revision, copy_revision);
        }
        CompleatedIdentityInvalidation::VorinclexController => {
            let old_controller = engine.state.objects[&vorinclex].controller;
            assert_eq!(old_controller, 0);
            let old_index = engine
                .state
                .player_idx(old_controller)
                .expect("old Vorinclex controller exists");
            let new_index = engine.state.player_idx(1).expect("new controller exists");
            engine.state.players[old_index]
                .battlefield
                .retain(|candidate| *candidate != vorinclex);
            engine.state.players[new_index].battlefield.push(vorinclex);
            engine
                .state
                .objects
                .get_mut(&vorinclex)
                .expect("Vorinclex remains present")
                .base_controller = 1;
            engine
                .reindex_battlefield_control(&mut Vec::new())
                .expect("reindex the prepared Vorinclex controller");
            assert_eq!(engine.state.objects[&vorinclex].controller, 1);
        }
        CompleatedIdentityInvalidation::ResumedEventDestination => {
            let pending = engine
                .state
                .pending_resolution
                .as_mut()
                .expect("the current entry choice remains parked");
            let ResolutionContinuation::EntryCounterReplacement { resume } =
                &mut pending.continuation
            else {
                panic!("the current choice retains its entry-counter continuation");
            };
            match resume.as_mut() {
                crate::state::EntryCounterReplacementResume::Single { event, .. } => {
                    event.destination_controller = 1;
                }
                crate::state::EntryCounterReplacementResume::Batch { batch, .. } => {
                    let crate::state::SimultaneousEntryBatch::Zone(batch) = batch else {
                        panic!("the prepared Batch continuation uses the Zone variant");
                    };
                    let event = batch
                        .ready
                        .iter_mut()
                        .find(|event| event.object_id == entrant)
                        .expect("authoritative Batch event exists");
                    event.destination_controller = 1;
                }
            }
            assert_eq!(pending.deciding_player, 0);
        }
    }
    if batch_continuation
        && matches!(
            invalidation,
            CompleatedIdentityInvalidation::ResumedEventDestination
        )
    {
        let Some(PendingReplacementEvent::Counters(_)) =
            engine.state.pending_replacement_event.as_ref()
        else {
            panic!("the invalidated Batch retains its saved counter proposals");
        };
    }
}

fn identity_rows_after_vorin_double(
    engine: &mut GameEngine,
    entrant: ObjectId,
    double_handle: u32,
) -> Vec<u32> {
    let mut batch = {
        let Some(PendingReplacementEvent::Counters(saved_batch)) =
            engine.state.pending_replacement_event.as_ref()
        else {
            panic!("control retains its current counter batch");
        };
        (**saved_batch).clone()
    };
    let mut entry = {
        let pending = engine
            .state
            .pending_resolution
            .as_ref()
            .expect("control remains parked at the current Vorinclex offer");
        match &pending.continuation {
            ResolutionContinuation::EntryCounterReplacement { resume } => match resume.as_ref() {
                crate::state::EntryCounterReplacementResume::Single { event, .. } => {
                    event.as_ref().clone()
                }
                crate::state::EntryCounterReplacementResume::Batch { batch, .. } => {
                    let crate::state::SimultaneousEntryBatch::Zone(batch) = batch else {
                        panic!("the prepared Batch continuation uses the Zone variant");
                    };
                    batch
                        .ready
                        .iter()
                        .find(|event| event.object_id == entrant)
                        .expect("control entrant remains in the authoritative Batch")
                        .clone()
                }
            },
            _ => panic!("control retains its entry-counter continuation"),
        }
    };
    let application = batch
        .applications
        .iter()
        .find(|application| application.choice_id == double_handle)
        .cloned()
        .expect("control double handle is current");
    GameEngine::apply_counter_placement_replacement(&mut batch, &application.replacement)
        .expect("apply the captured double operation to a cloned control batch");
    batch.applications.clear();
    assert!(
        engine
            .advance_counter_placement_batch(&mut batch)
            .expect("advance the cloned control batch")
            .is_none(),
        "the only remaining half operation finishes automatically"
    );
    GameEngine::apply_entry_counter_placement_results(&mut entry, &batch.events)
        .expect("write the cloned control rows back to the cloned event");
    entry
        .entry_counter_placements
        .iter()
        .map(|placement| placement.counters[&CounterKind::Loyalty])
        .collect()
}

fn prepare_compleated_identity_sibling_batch() -> (
    GameEngine,
    ObjectId,
    ObjectId,
    rv1::ResolutionChoiceRequired,
    u32,
) {
    let mut engine = engine();
    let stack = paid_divination_stack(&mut engine);
    let season = object(&mut engine, "doubling_season", Zone::Battlefield, 0);
    engine.emit_static_abilities_on_enter(season);
    let entrant_a = compleated_identity_probe_object(&mut engine, 0);
    let entrant_b = compleated_identity_probe_object(&mut engine, 1);
    let entry_a = compleated_identity_entry(&engine, &stack, entrant_a, 0, 0, 1, &[(6, 0)]);
    let entry_b = compleated_identity_entry(&engine, &stack, entrant_b, 1, 1, 2, &[(9, 1)]);
    let batch = compleated_identity_zone_batch(&engine, vec![entry_a, entry_b]);
    let mut events = Vec::new();
    assert!(engine
        .begin_entry_counter_placements_for_batch(batch, Some(stack), &mut events)
        .expect("prepare distinct-controller sibling entries")
        .is_none());
    engine
        .publish_resolution_prompt_ids(&mut events)
        .expect("publish A's initial Compleated and Season choice");
    let offer = identity_choice_offer(&events);
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("A's replacement choice remains parked");
    assert_eq!(pending.deciding_player, 0);
    let Some(PendingReplacementEvent::Counters(batch)) =
        engine.state.pending_replacement_event.as_ref()
    else {
        panic!("the sibling choice retains the current counter batch");
    };
    let application = batch
        .applications
        .iter()
        .find(|application| {
            matches!(
                &application.replacement,
                CounterPlacementReplacement::Compleated { replacement, .. }
                    if replacement.identity.recipient_object_id == entrant_a
            )
        })
        .expect("A's Compleated choice is a current application");
    assert!(offer.candidate_object_ids.contains(&application.choice_id));
    let b_rows = batch
        .events
        .iter()
        .filter(|proposal| {
            matches!(
                &proposal.recipient,
                CounterPlacementRecipient::EnteringObject { object_id, .. }
                    if *object_id == entrant_b
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(b_rows.len(), 1);
    assert_eq!(b_rows[0].original_counters[&CounterKind::Loyalty], 9);
    assert_eq!(b_rows[0].counters[&CounterKind::Loyalty], 9);
    let b_context = b_rows[0]
        .entry_context
        .as_ref()
        .expect("B has captured entry context before its rows advance");
    assert_eq!(b_context.destination_controller, 1);
    assert!(b_context.compleated.is_some());
    assert!(b_context.history.receipts.is_empty());
    let compleated_handle = application.choice_id;
    (engine, entrant_a, entrant_b, offer, compleated_handle)
}

fn compleated_grouped_entry_identity_invalidations_reject_atomically_for_single_and_batch_cases() {
    let invalidations = [
        CompleatedIdentityInvalidation::CopyRevision,
        CompleatedIdentityInvalidation::ZoneChangeGeneration,
        CompleatedIdentityInvalidation::RemoveCompleated,
        CompleatedIdentityInvalidation::ChangeCompleatedAbilityId,
        CompleatedIdentityInvalidation::VorinclexController,
        CompleatedIdentityInvalidation::ResumedEventDestination,
    ];
    for batch_continuation in [false, true] {
        for invalidation in invalidations {
            let (mut engine, entrant, vorinclex, offer, handle) =
                prepare_compleated_identity_vorin_choice(batch_continuation);
            invalidate_compleated_identity_fixture(
                &mut engine,
                entrant,
                vorinclex,
                batch_continuation,
                invalidation,
            );
            let before = engine
                .diagnostic_snapshot()
                .expect("snapshot after the intentional identity invalidation");
            let command_index = engine.state.command_index;
            assert!(
                matches!(
                    engine.apply_command(
                        0,
                        &submit_identity_choice(offer.resolution_prompt_id, handle)
                    ),
                    Err(EngineError::Illegal(_))
                ),
                "current Single/Batch choice rejects {} atomically",
                invalidation.label()
            );
            assert_eq!(engine.state.command_index, command_index);
            assert_eq!(
                engine
                    .diagnostic_snapshot()
                    .expect("snapshot after rejected current identity choice"),
                before,
                "rejected {} preserves the complete invalidated snapshot",
                invalidation.label()
            );
        }

        let (mut engine, entrant, _vorinclex, offer, double_handle) =
            prepare_compleated_identity_vorin_choice(batch_continuation);
        assert_eq!(
            identity_rows_after_vorin_double(&mut engine, entrant, double_handle),
            [4, 2],
            "Vorinclex double and the remaining half produce the expected row attribution"
        );
        let command_index = engine.state.command_index;
        let completed = engine
            .apply_command(
                0,
                &submit_identity_choice(offer.resolution_prompt_id, double_handle),
            )
            .expect("accept the unchanged current Vorinclex double handle");
        assert_eq!(engine.state.command_index, command_index + 1);
        assert!(
            !completed.events.iter().any(|event| matches!(
                &event.ev,
                Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(_))
            )),
            "the remaining half operation completes automatically"
        );
        assert!(engine.state.pending_resolution.is_none());
        assert!(engine.state.pending_replacement_event.is_none());
        assert_eq!(engine.state.objects[&entrant].zone, Zone::Battlefield);
        assert_eq!(
            engine.state.objects[&entrant].counter_count(CounterKind::Loyalty),
            6,
            "the committed grouped loyalty rows total six"
        );
    }

    for invalidation in [
        CompleatedIdentityInvalidation::CopyRevision,
        CompleatedIdentityInvalidation::RemoveCompleated,
    ] {
        let (mut engine, entrant_a, entrant_b, offer, compleated_handle) =
            prepare_compleated_identity_sibling_batch();
        match invalidation {
            CompleatedIdentityInvalidation::CopyRevision => {
                let object = engine
                    .state
                    .objects
                    .get_mut(&entrant_b)
                    .expect("sibling B exists");
                object.copy_revision = object
                    .copy_revision
                    .checked_add(1)
                    .expect("B copy revision has room");
            }
            CompleatedIdentityInvalidation::RemoveCompleated => {
                let object = engine
                    .state
                    .objects
                    .get_mut(&entrant_b)
                    .expect("sibling B exists");
                let copy_revision = object.copy_revision;
                object
                    .copiable_values
                    .as_mut()
                    .expect("B has a prepared copiable face")
                    .face
                    .static_abilities
                    .retain(|ability| !matches!(&ability.definition, StaticAbilityDef::Compleated));
                assert_eq!(object.copy_revision, copy_revision);
            }
            _ => unreachable!("only the two Batch sibling invalidations are tested here"),
        }
        let before = engine
            .diagnostic_snapshot()
            .expect("snapshot after the intentional B invalidation");
        let command_index = engine.state.command_index;
        assert!(
            matches!(
                engine.apply_command(
                    0,
                    &submit_identity_choice(offer.resolution_prompt_id, compleated_handle)
                ),
                Err(EngineError::Illegal(_))
            ),
            "A's current offer rejects B's {} invalidation before advancing either entrant",
            invalidation.label()
        );
        assert_eq!(engine.state.command_index, command_index);
        assert_eq!(
            engine
                .diagnostic_snapshot()
                .expect("snapshot after rejected sibling invalidation"),
            before,
            "A's rejection leaves both sibling rows and prompt allocation untouched"
        );
        assert_eq!(
            engine.state.objects[&entrant_a].zone,
            Zone::Stack,
            "A stays uncommitted after sibling rejection"
        );
        assert_eq!(
            engine.state.objects[&entrant_b].zone,
            Zone::Stack,
            "B stays uncommitted after sibling rejection"
        );
    }

    let (mut engine, entrant_a, entrant_b, offer, compleated_handle) =
        prepare_compleated_identity_sibling_batch();
    let command_index = engine.state.command_index;
    let completed = engine
        .apply_command(
            0,
            &submit_identity_choice(offer.resolution_prompt_id, compleated_handle),
        )
        .expect("accept A's current Compleated choice through the normal command route");
    assert_eq!(engine.state.command_index, command_index + 1);
    assert!(
        !completed.events.iter().any(|event| matches!(
            &event.ev,
            Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(_))
        )),
        "A's Season and B's sole Compleated replacement finish automatically"
    );
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.pending_replacement_event.is_none());
    assert_eq!(engine.state.objects[&entrant_a].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&entrant_b].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&entrant_a].controller, 0);
    assert_eq!(engine.state.objects[&entrant_b].controller, 1);
    assert_eq!(
        engine.state.objects[&entrant_a].counter_count(CounterKind::Loyalty),
        8,
        "A's accepted Compleated replacement and Season finish at eight"
    );
    assert_eq!(
        engine.state.objects[&entrant_b].counter_count(CounterKind::Loyalty),
        5,
        "B's paid-count-two Compleated replacement finishes at five"
    );

    // These are prepared-state exact-return kernel cases. The positive paid-count value below is
    // fixture data for Compleated; it does not establish an actual noncast payment or card reachability.
    let (mut unchanged_engine, unchanged_entrant, unchanged_observed) =
        prepare_compleated_exact_owner_exile_return();
    let (unchanged_pending, unchanged_replacement) = exact_return_saved_state(&unchanged_engine);
    assert_eq!(
        unchanged_engine
            .begun_exact_owner_exile_return(&unchanged_pending, Some(&unchanged_replacement),),
        Some(unchanged_observed),
        "the prepared Single exact Exile-to-owner return is recognized before refresh"
    );
    assert_eq!(
        unchanged_engine.state.objects[&unchanged_entrant].zone,
        Zone::Exile
    );
    let (unchanged_prompt, unchanged_application_ids, unchanged_histories) =
        exact_return_prompt_identity(&unchanged_engine);
    assert_eq!(unchanged_application_ids.len(), 3);
    assert!(unchanged_histories.iter().any(|history| {
        history.receipts.iter().any(|receipt| {
            matches!(
                receipt,
                crate::state::EntryCounterReplacementReceipt::Compleated { .. }
            )
        })
    }));
    let unchanged_snapshot = unchanged_engine
        .diagnostic_snapshot()
        .expect("snapshot before unchanged exact-return refresh");
    let unchanged_prompt_epochs = unchanged_engine.state.next_resolution_prompt_id.clone();
    let unchanged_application_counter = unchanged_engine.state.next_replacement_application_id;
    let unchanged_command_index = unchanged_engine.state.command_index;
    let mut unchanged_events = Vec::new();
    assert!(
        !unchanged_engine
            .refresh_exact_return_entry_counters(&mut unchanged_events)
            .expect("unchanged exact-return entry refresh is valid"),
        "unchanged grouped identities retain their current offer"
    );
    assert!(unchanged_events.is_empty());
    assert_eq!(
        unchanged_engine
            .diagnostic_snapshot()
            .expect("snapshot after unchanged exact-return refresh"),
        unchanged_snapshot
    );
    assert_eq!(
        unchanged_engine.state.command_index,
        unchanged_command_index
    );
    assert_eq!(
        unchanged_engine.state.next_resolution_prompt_id,
        unchanged_prompt_epochs
    );
    assert_eq!(
        unchanged_engine.state.next_replacement_application_id,
        unchanged_application_counter
    );
    assert_eq!(
        exact_return_prompt_identity(&unchanged_engine),
        (
            unchanged_prompt,
            unchanged_application_ids,
            unchanged_histories
        ),
        "unchanged refresh preserves prompt, handles, and intrinsic history"
    );

    for invalidation in [
        CompleatedIdentityInvalidation::CopyRevision,
        CompleatedIdentityInvalidation::RemoveCompleated,
        CompleatedIdentityInvalidation::ResumedEventDestination,
    ] {
        let (mut engine, entrant, observed) = prepare_compleated_exact_owner_exile_return();
        match invalidation {
            CompleatedIdentityInvalidation::CopyRevision => {
                let object = engine
                    .state
                    .objects
                    .get_mut(&entrant)
                    .expect("exact-return entrant exists");
                object.copy_revision = object
                    .copy_revision
                    .checked_add(1)
                    .expect("exact-return copy revision has room");
            }
            CompleatedIdentityInvalidation::RemoveCompleated => {
                let object = engine
                    .state
                    .objects
                    .get_mut(&entrant)
                    .expect("exact-return entrant exists");
                let copy_revision = object.copy_revision;
                object
                    .copiable_values
                    .as_mut()
                    .expect("exact-return entrant retains its prepared Compleated face")
                    .face
                    .static_abilities
                    .retain(|ability| !matches!(&ability.definition, StaticAbilityDef::Compleated));
                assert_eq!(object.copy_revision, copy_revision);
            }
            CompleatedIdentityInvalidation::ResumedEventDestination => {
                exact_return_set_resumed_event_destination(
                    engine
                        .state
                        .pending_resolution
                        .as_mut()
                        .expect("exact-return counter choice remains parked"),
                    1,
                );
            }
            _ => unreachable!("only the three approved exact-return invalidations are used"),
        }
        let (pending, replacement) = exact_return_saved_state(&engine);
        let recognition = engine.begun_exact_owner_exile_return(&pending, Some(&replacement));
        if matches!(
            invalidation,
            CompleatedIdentityInvalidation::ResumedEventDestination
        ) {
            assert_eq!(
                recognition, None,
                "destination mismatch leaves the strict begun-return recognition contract"
            );
        } else {
            assert_eq!(
                recognition,
                Some(observed),
                "{} does not weaken the captured exact-return recognition contract",
                invalidation.label()
            );
        }
        let before = engine
            .diagnostic_snapshot()
            .expect("snapshot after the intentional exact-return invalidation");
        let prompt_epochs_before = engine.state.next_resolution_prompt_id.clone();
        let application_counter_before = engine.state.next_replacement_application_id;
        let command_index_before = engine.state.command_index;
        let identity_before = exact_return_prompt_identity(&engine);
        let mut events = Vec::new();
        assert!(
            matches!(
                engine.refresh_exact_return_entry_counters(&mut events),
                Err(EngineError::Illegal(_))
            ),
            "direct refresh rejects {} before changing the parked operation",
            invalidation.label()
        );
        assert!(events.is_empty());
        assert_eq!(engine.state.command_index, command_index_before);
        assert_eq!(
            engine
                .diagnostic_snapshot()
                .expect("snapshot after rejected exact-return refresh"),
            before,
            "rejected {} preserves the complete post-invalidation snapshot",
            invalidation.label()
        );
        assert_eq!(engine.state.next_resolution_prompt_id, prompt_epochs_before);
        assert_eq!(
            engine.state.next_replacement_application_id,
            application_counter_before
        );
        assert_eq!(exact_return_prompt_identity(&engine), identity_before);
    }

    let (mut restore_copy_engine, restore_copy_entrant, restore_copy_observed) =
        prepare_compleated_exact_owner_exile_return();
    let (restore_copy_pending, restore_copy_replacement) =
        exact_return_saved_state(&restore_copy_engine);
    assert_eq!(
        restore_copy_engine.begun_exact_owner_exile_return(
            &restore_copy_pending,
            Some(&restore_copy_replacement),
        ),
        Some(restore_copy_observed)
    );
    let object = restore_copy_engine
        .state
        .objects
        .get_mut(&restore_copy_entrant)
        .expect("restore copy-revision entrant exists");
    object.copy_revision = object
        .copy_revision
        .checked_add(1)
        .expect("restore copy revision has room");
    let restore_copy_before = restore_copy_engine
        .diagnostic_snapshot()
        .expect("snapshot after live copy-revision invalidation");
    let restore_copy_epochs = restore_copy_engine.state.next_resolution_prompt_id.clone();
    let restore_copy_application_counter =
        restore_copy_engine.state.next_replacement_application_id;
    let restore_copy_command_index = restore_copy_engine.state.command_index;
    let restore_copy_identity = exact_return_prompt_identity(&restore_copy_engine);
    let mut restore_copy_events = Vec::new();
    assert!(
        matches!(
            restore_copy_engine.restore_suspended_observer_resolution(
                Some(restore_copy_pending),
                Some(restore_copy_replacement),
                &mut restore_copy_events,
            ),
            Err(EngineError::Illegal(_))
        ),
        "invalid live copy revision rejects before installing the saved observer state"
    );
    assert!(restore_copy_events.is_empty());
    assert_eq!(
        restore_copy_engine.state.command_index,
        restore_copy_command_index
    );
    assert_eq!(
        restore_copy_engine
            .diagnostic_snapshot()
            .expect("snapshot after rejected copy-revision restore"),
        restore_copy_before
    );
    assert_eq!(
        restore_copy_engine.state.next_resolution_prompt_id,
        restore_copy_epochs
    );
    assert_eq!(
        restore_copy_engine.state.next_replacement_application_id,
        restore_copy_application_counter
    );
    assert_eq!(
        exact_return_prompt_identity(&restore_copy_engine),
        restore_copy_identity
    );

    let (mut restore_destination_engine, _, _) = prepare_compleated_exact_owner_exile_return();
    let (mut invalid_pending, invalid_replacement) =
        exact_return_saved_state(&restore_destination_engine);
    exact_return_set_resumed_event_destination(&mut invalid_pending, 1);
    let incoming_snapshot = (invalid_pending.clone(), invalid_replacement.clone());
    let incoming_debug_before = format!("{incoming_snapshot:?}");
    assert_eq!(
        restore_destination_engine
            .begun_exact_owner_exile_return(&invalid_pending, Some(&invalid_replacement)),
        None,
        "incoming-only destination mismatch bypasses begun-return recognition"
    );
    let restore_destination_before = restore_destination_engine
        .diagnostic_snapshot()
        .expect("active state snapshot before invalid incoming restore");
    let restore_destination_epochs = restore_destination_engine
        .state
        .next_resolution_prompt_id
        .clone();
    let restore_destination_application_counter = restore_destination_engine
        .state
        .next_replacement_application_id;
    let restore_destination_command_index = restore_destination_engine.state.command_index;
    let restore_destination_identity = exact_return_prompt_identity(&restore_destination_engine);
    let mut restore_destination_events = Vec::new();
    assert!(
        matches!(
            restore_destination_engine.restore_suspended_observer_resolution(
                Some(invalid_pending),
                Some(invalid_replacement),
                &mut restore_destination_events,
            ),
            Err(EngineError::Illegal(_))
        ),
        "incoming-only destination mismatch rejects before epoch reset or state installation"
    );
    assert!(restore_destination_events.is_empty());
    assert_eq!(
        restore_destination_engine.state.command_index,
        restore_destination_command_index
    );
    assert_eq!(
        restore_destination_engine
            .diagnostic_snapshot()
            .expect("active snapshot after rejected incoming restore"),
        restore_destination_before
    );
    assert_eq!(
        restore_destination_engine.state.next_resolution_prompt_id,
        restore_destination_epochs
    );
    assert_eq!(
        restore_destination_engine
            .state
            .next_replacement_application_id,
        restore_destination_application_counter
    );
    assert_eq!(
        exact_return_prompt_identity(&restore_destination_engine),
        restore_destination_identity
    );
    assert_eq!(
        format!("{incoming_snapshot:?}"),
        incoming_debug_before,
        "the typed invalid incoming payload snapshot remains available and unchanged"
    );

    let (mut restored_engine, _, _) = prepare_compleated_exact_owner_exile_return();
    let (saved_pending, saved_replacement) = exact_return_saved_state(&restored_engine);
    let old_prompt = saved_pending.resolution_prompt_id.get();
    let (_, old_application_ids, old_histories) = exact_return_prompt_identity(&restored_engine);
    assert_eq!(old_application_ids.len(), 3);
    let old_prompt_epochs = restored_engine.state.next_resolution_prompt_id.clone();
    let old_application_counter = restored_engine.state.next_replacement_application_id;
    let mut restored_events = Vec::new();
    assert!(
        restored_engine
            .restore_suspended_observer_resolution(
                Some(saved_pending),
                Some(saved_replacement),
                &mut restored_events,
            )
            .expect("unchanged exact-return observer restore is valid"),
        "unchanged current entrant restores and republishes its parked offer"
    );
    restored_engine
        .publish_resolution_prompt_ids(&mut restored_events)
        .expect("publish the fresh observer-restored prompt epoch");
    let restored_offer = identity_choice_offer(&restored_events);
    let restored_prompt = restored_offer.resolution_prompt_id;
    assert_ne!(restored_prompt, 0);
    assert_ne!(restored_prompt, old_prompt);
    assert_eq!(restored_offer.deciding_player_id, 0);
    let (stored_restored_prompt, restored_application_ids, restored_histories) =
        exact_return_prompt_identity(&restored_engine);
    assert_eq!(stored_restored_prompt, restored_prompt);
    assert_eq!(restored_application_ids, old_application_ids);
    assert_eq!(restored_histories, old_histories);
    assert_eq!(
        restored_engine.state.next_replacement_application_id, old_application_counter,
        "unchanged restoration reuses application IDs"
    );
    assert!(
        restored_engine
            .state
            .next_resolution_prompt_id
            .get(&0)
            .copied()
            .unwrap_or(0)
            > old_prompt_epochs.get(&0).copied().unwrap_or(0),
        "the correct player's prompt epoch advances"
    );
    assert_eq!(
        restored_engine
            .state
            .pending_resolution
            .as_ref()
            .expect("fresh observer prompt remains active")
            .deciding_player,
        0
    );
    let current_handle = restored_offer
        .candidate_object_ids
        .iter()
        .copied()
        .find(|candidate| restored_application_ids.contains(candidate))
        .expect("the fresh prompt offers a current retained application handle");
    let before_old_epoch = restored_engine
        .diagnostic_snapshot()
        .expect("snapshot before stale old-epoch command");
    let before_old_epoch_counters = restored_engine.state.next_resolution_prompt_id.clone();
    let before_old_epoch_application_counter =
        restored_engine.state.next_replacement_application_id;
    let before_old_epoch_command_index = restored_engine.state.command_index;
    assert!(
        matches!(
            restored_engine.apply_command(0, &submit_identity_choice(old_prompt, current_handle)),
            Err(EngineError::Illegal(_))
        ),
        "ordinary submitted choices still reject the old prompt epoch"
    );
    assert_eq!(
        restored_engine.state.command_index,
        before_old_epoch_command_index
    );
    assert_eq!(
        restored_engine
            .diagnostic_snapshot()
            .expect("snapshot after rejected old-epoch command"),
        before_old_epoch
    );
    assert_eq!(
        restored_engine.state.next_resolution_prompt_id,
        before_old_epoch_counters
    );
    assert_eq!(
        restored_engine.state.next_replacement_application_id,
        before_old_epoch_application_counter
    );
    let command_index_before_current = restored_engine.state.command_index;
    restored_engine
        .apply_command(0, &submit_identity_choice(restored_prompt, current_handle))
        .expect("accept a current retained handle using the fresh owner epoch");
    assert_eq!(
        restored_engine.state.command_index,
        command_index_before_current + 1
    );

    let (mut departure_engine, departure_entrant, departure_observed) =
        prepare_compleated_exact_owner_exile_return();
    let (departure_pending, departure_replacement) = exact_return_saved_state(&departure_engine);
    assert_eq!(
        departure_engine
            .begun_exact_owner_exile_return(&departure_pending, Some(&departure_replacement),),
        Some(departure_observed)
    );
    let (_, departure_application_ids, departure_histories) =
        exact_return_prompt_identity(&departure_engine);
    assert_eq!(departure_application_ids.len(), 3);
    assert!(departure_histories
        .iter()
        .any(|history| !history.receipts.is_empty()));
    let life_before_departure = departure_engine.state.players[0].life;
    super::super::resolution::move_object_to_zone(
        &mut departure_engine.state,
        departure_engine.registry,
        departure_entrant,
        Zone::Graveyard,
        None,
    )
    .expect("move the exact-return entrant through an actual zone change");
    let departure_generation = departure_engine
        .state
        .zone_change_generation
        .get(&departure_entrant)
        .copied()
        .unwrap_or(0);
    assert_ne!(
        departure_generation,
        departure_observed.zone_change_generation
    );
    let departed_object = departure_engine
        .state
        .objects
        .get(&departure_entrant)
        .expect("new departed incarnation remains present");
    assert_eq!(departed_object.zone, Zone::Graveyard);
    assert_eq!(departed_object.owner, 0);
    let departure_copy_revision = departed_object.copy_revision;
    let departure_counter_count = departed_object.counter_count(CounterKind::Loyalty);
    assert_eq!(
        departure_counter_count, 0,
        "no proposed old entry counters were committed"
    );
    let departure_copy_values = format!("{:?}", departed_object.copiable_values);
    assert_eq!(
        departure_engine
            .begun_exact_owner_exile_return(&departure_pending, Some(&departure_replacement),),
        Some(departure_observed),
        "saved exact-return recognition survives the actual zone change"
    );
    let mut departure_events = Vec::new();
    assert!(
        departure_engine
            .restore_suspended_observer_resolution(
                Some(departure_pending),
                Some(departure_replacement),
                &mut departure_events,
            )
            .expect("recognized departed return finishes through observer restoration"),
        "recognized departure takes the existing finish/skip path"
    );
    assert!(departure_engine.state.pending_resolution.is_none());
    assert!(departure_engine.state.pending_replacement_event.is_none());
    assert!(
        departure_events.iter().all(|event| !matches!(
            event.ev.as_ref(),
            Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(_))
        )),
        "the old Season offer is not republished"
    );
    assert!(
        departure_events.iter().any(|event| matches!(
            event.ev.as_ref(),
            Some(rv1::ruled_event::Ev::LifeChanged(_))
        )),
        "the consumed-return stack tail resolves"
    );
    assert_eq!(
        departure_engine.state.players[0].life,
        life_before_departure + 1
    );
    assert!(
        departure_events.iter().all(|event| !matches!(
            event.ev.as_ref(),
            Some(rv1::ruled_event::Ev::PermanentMoved(moved))
                if moved.object_id == departure_entrant
        )),
        "finish/skip emits no old entry movement for the departed incarnation"
    );
    let preserved_departed_object = departure_engine
        .state
        .objects
        .get(&departure_entrant)
        .expect("new departed incarnation remains present after cleanup");
    assert_eq!(preserved_departed_object.owner, 0);
    assert_eq!(preserved_departed_object.zone, Zone::Graveyard);
    assert_eq!(
        preserved_departed_object.copy_revision,
        departure_copy_revision
    );
    assert_eq!(
        preserved_departed_object.counter_count(CounterKind::Loyalty),
        departure_counter_count
    );
    assert_eq!(
        format!("{:?}", preserved_departed_object.copiable_values),
        departure_copy_values,
    );
    assert_eq!(
        departure_engine
            .state
            .zone_change_generation
            .get(&departure_entrant)
            .copied()
            .unwrap_or(0),
        departure_generation,
        "cleanup preserves the new zone-change generation"
    );
}

fn exact_return_saved_state(
    engine: &GameEngine,
) -> (crate::state::PendingResolution, PendingReplacementEvent) {
    (
        engine
            .state
            .pending_resolution
            .as_ref()
            .expect("exact-return resolution remains parked")
            .clone(),
        engine
            .state
            .pending_replacement_event
            .as_ref()
            .expect("exact-return replacement remains parked")
            .clone(),
    )
}

fn exact_return_prompt_identity(
    engine: &GameEngine,
) -> (
    u64,
    Vec<u32>,
    Vec<crate::state::EntryCounterPlacementHistory>,
) {
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("exact-return prompt remains active");
    let Some(PendingReplacementEvent::Counters(batch)) =
        engine.state.pending_replacement_event.as_ref()
    else {
        panic!("exact-return prompt retains its counter application batch");
    };
    let application_ids = batch
        .applications
        .iter()
        .map(|application| application.choice_id)
        .collect();
    let histories = batch
        .events
        .iter()
        .filter_map(|proposal| {
            proposal
                .entry_context
                .as_ref()
                .map(|context| context.history.clone())
        })
        .collect();
    (
        pending.resolution_prompt_id.get(),
        application_ids,
        histories,
    )
}

fn exact_return_set_resumed_event_destination(
    pending: &mut crate::state::PendingResolution,
    destination: PlayerId,
) {
    let ResolutionContinuation::EntryCounterReplacement { resume } = &mut pending.continuation
    else {
        panic!("exact-return fixture retains its counter continuation");
    };
    let crate::state::EntryCounterReplacementResume::Single { event, .. } = resume.as_mut() else {
        panic!("exact-return fixture uses the Single counter continuation");
    };
    event.destination_controller = destination;
}

fn prepare_compleated_exact_owner_exile_return(
) -> (GameEngine, ObjectId, crate::state::TriggerObjectRef) {
    let mut engine = engine();
    let trigger_source = object(&mut engine, "oath_of_teferi", Zone::Stack, 0);
    let mut ability = engine
        .registry
        .get("oath_of_teferi")
        .expect("registered typed exact-return analogue")
        .primary_face()
        .triggered_abilities[0]
        .clone();
    ability.ability_id = tricerules_card_model::AbilityId::new("exact_owner_return_fixture")
        .expect("valid fixture ability ID");
    ability.program = None;
    ability.targeting = None;
    ability.trigger = tricerules_card_model::primitives::TriggerCondition::AtBeginningOfNextEndStep;
    ability.effect = vec![
        tricerules_card_model::primitives::SpellEffectKind::ReturnTriggeredCard {
            reference:
                tricerules_card_model::primitives::TriggeredCardReference::ExactTriggerObject,
            from: vec![tricerules_card_model::primitives::EventZone::Exile],
            destination: tricerules_card_model::primitives::TriggeredCardDestination::Battlefield,
            tapped: false,
            controller: tricerules_card_model::primitives::ReturnController::Owner,
            entry_counters: Vec::new(),
            set_types: None,
        },
        tricerules_card_model::primitives::SpellEffectKind::GainLife {
            amount: tricerules_card_model::primitives::Amount::Fixed(1),
            who: tricerules_card_model::primitives::PlayerRecipient::Controller,
        },
    ];
    let mut item = engine.observer_return_item(trigger_source, 0);
    item.is_triggered = true;
    item.triggered_ability = Some(ability);
    let mut stack =
        crate::state::ParkedStackResolution::for_flat_effects(item, Some(1), Default::default());

    let (entrant, entry, seasons, _) = compleated_grouped_entry_fixture(&mut engine, &stack);
    assert_eq!(seasons.len(), 3);
    let prepared_compleated_face = engine
        .state
        .objects
        .get(&entrant)
        .expect("prepared Compleated entrant exists")
        .copiable_values
        .clone()
        .expect("prepared entrant has its Compleated face");
    super::super::resolution::move_object_to_zone(
        &mut engine.state,
        engine.registry,
        entrant,
        Zone::Exile,
        None,
    )
    .expect("place the exact-return entrant in Exile through a zone change");
    engine
        .state
        .objects
        .get_mut(&entrant)
        .expect("entrant remains present in Exile")
        .copiable_values = Some(prepared_compleated_face);
    let generation = engine
        .state
        .zone_change_generation
        .get(&entrant)
        .copied()
        .unwrap_or(0);
    let observed = crate::state::TriggerObjectRef {
        object_id: entrant,
        zone_change_generation: generation,
        controller_at_event: 0,
    };
    stack.item.trigger_context.observed_object = Some(observed);
    assert_eq!(engine.state.objects[&entrant].owner, 0);
    assert_eq!(engine.state.objects[&entrant].zone, Zone::Exile);
    assert_eq!(
        entry.phyrexian_symbols_paid_with_life, 1,
        "positive paid count is prepared kernel input, not a payment claim"
    );
    assert_eq!(entry.object_id, entrant);
    assert_eq!(entry.deciding_player, 0);
    assert_eq!(entry.destination_controller, 0);
    let completion = crate::state::BattlefieldEntryCompletion::ResolutionEffect {
        owner: 0,
        spell_label: "Prepared exact owner return".into(),
        object_label: "Compleated exact-return probe".into(),
        from_zone: Zone::Exile,
        triggered_return_generation: Some(generation),
    };
    let mut events = Vec::new();
    assert!(engine
        .prepare_single_entry_counter_placements(stack, entry, completion, &mut events)
        .expect("prepare the exact-return Single counter continuation")
        .is_none());
    engine
        .publish_resolution_prompt_ids(&mut events)
        .expect("publish the initial Compleated replacement offer");
    let initial_offer = identity_choice_offer(&events);
    let (initial_prompt, compleated_handle) = identity_compleated_choice(&engine);
    assert_eq!(initial_offer.resolution_prompt_id, initial_prompt);
    assert_eq!(initial_offer.deciding_player_id, 0);
    assert!(initial_offer
        .candidate_object_ids
        .contains(&compleated_handle));
    let command_index = engine.state.command_index;
    let accepted = engine
        .apply_command(
            0,
            &submit_identity_choice(initial_prompt, compleated_handle),
        )
        .expect("accept the exact-return entrant's Compleated application normally");
    assert_eq!(engine.state.command_index, command_index + 1);
    let current_offer = identity_choice_offer(&accepted.events);
    assert_eq!(current_offer.deciding_player_id, 0);
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("the three Seasons leave a counter offer parked");
    assert_eq!(pending.deciding_player, 0);
    assert!(matches!(
        &pending.continuation,
        ResolutionContinuation::EntryCounterReplacement { resume }
            if matches!(
                resume.as_ref(),
                crate::state::EntryCounterReplacementResume::Single { .. }
            )
    ));
    let Some(PendingReplacementEvent::Counters(batch)) =
        engine.state.pending_replacement_event.as_ref()
    else {
        panic!("the exact-return offer retains its counter batch");
    };
    for season in seasons {
        let application = batch
            .applications
            .iter()
            .find(|application| {
                matches!(
                    &application.replacement,
                    CounterPlacementReplacement::Battlefield { instance, .. }
                        if instance.source_object_id == season
                )
            })
            .expect("each real Season remains a current replacement application");
        assert!(current_offer
            .candidate_object_ids
            .contains(&application.choice_id));
    }
    assert!(batch
        .events
        .iter()
        .filter_map(|proposal| proposal.entry_context.as_ref())
        .any(|context| {
            context.history.receipts.iter().any(|receipt| {
                matches!(
                    receipt,
                    crate::state::EntryCounterReplacementReceipt::Compleated { .. }
                )
            })
        }));
    let (pending, replacement) = exact_return_saved_state(&engine);
    let observed = pending
        .continuation
        .stack()
        .and_then(|stack| stack.item.trigger_context.observed_object)
        .expect("parked triggered item retains the observed incarnation");
    assert_eq!(
        engine.begun_exact_owner_exile_return(&pending, Some(&replacement)),
        Some(observed),
        "prepared Single event, cursor, completion, and actors satisfy recognition"
    );
    (engine, entrant, observed)
}

#[test]
fn compleated_grouped_entry_identity_invalidations_reject_atomically_for_single_and_batch() {
    let worker = std::thread::Builder::new()
        .name("compleated-entry-identity-invalidation-cases".to_owned())
        .stack_size(16 * 1024 * 1024)
        .spawn(compleated_grouped_entry_identity_invalidations_reject_atomically_for_single_and_batch_cases)
        .expect("failed to spawn compleated entry identity invalidation worker");
    if let Err(payload) = worker.join() {
        std::panic::resume_unwind(payload);
    }
}

fn compleated_grouped_numeric_history_after_one_season(
    baseline: [u32; 2],
) -> (GameEngine, ObjectId, Vec<ObjectId>, ObjectId, String) {
    assert!(baseline == [1, 5] || baseline == [4, 4]);
    let mut engine = engine();
    let stack = paid_divination_stack(&mut engine);
    let stack_debug = format!("{stack:?}");
    let (entrant, mut entry, seasons, _) = compleated_grouped_entry_fixture(&mut engine, &stack);
    if baseline == [4, 4] {
        assert!(entry
            .entry_counter_placements
            .iter()
            .all(|placement| placement.history.is_none()
                && placement.applied_replacements.is_empty()));
        for (placement, count) in entry.entry_counter_placements.iter_mut().zip(baseline) {
            placement.counters.insert(CounterKind::Loyalty, count);
        }
        entry
            .entry_counters
            .insert(CounterKind::Loyalty, baseline[0] + baseline[1]);
    }
    assert_eq!(
        entry
            .entry_counter_placements
            .iter()
            .map(|placement| placement.counters[&CounterKind::Loyalty])
            .collect::<Vec<_>>(),
        baseline
    );
    let completion = crate::state::BattlefieldEntryCompletion::ResolutionEffect {
        owner: 0,
        spell_label: "Prepared Compleated numeric history".into(),
        object_label: "Compleated Entry Probe".into(),
        from_zone: Zone::Stack,
        triggered_return_generation: None,
    };
    let mut events = Vec::new();
    assert!(engine
        .prepare_single_entry_counter_placements(stack, entry, completion, &mut events)
        .expect("prepare the numeric-history Single entry")
        .is_none());
    engine
        .publish_resolution_prompt_ids(&mut events)
        .expect("publish the initial Compleated offer");
    let initial_offer = identity_choice_offer(&events);
    let (initial_prompt, compleated_handle) = identity_compleated_choice(&engine);
    assert_eq!(initial_offer.resolution_prompt_id, initial_prompt);
    let compleated_result = engine
        .apply_command(
            0,
            &submit_identity_choice(initial_prompt, compleated_handle),
        )
        .expect("accept authentic Compleated through normal command handling");
    let compleated_offer = identity_choice_offer(&compleated_result.events);
    let (after_compleated, removed) = if baseline == [1, 5] {
        ([0, 4], [1, 1])
    } else {
        ([2, 4], [2, 0])
    };
    let batch = match engine.state.pending_replacement_event.as_ref() {
        Some(PendingReplacementEvent::Counters(batch)) => batch,
        _ => panic!("the accepted Compleated result leaves Season choices parked"),
    };
    let rows = batch
        .events
        .iter()
        .filter(|proposal| {
            matches!(
                &proposal.recipient,
                CounterPlacementRecipient::EnteringObject { object_id, .. }
                    if *object_id == entrant
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(rows.len(), 2);
    for row in 0..2 {
        assert_eq!(
            rows[row].original_counters[&CounterKind::Loyalty],
            baseline[row]
        );
        assert_eq!(
            rows[row].counters[&CounterKind::Loyalty],
            after_compleated[row]
        );
        assert_eq!(
            rows[row]
                .entry_context
                .as_ref()
                .unwrap()
                .history
                .receipts
                .iter()
                .filter_map(|receipt| match receipt {
                    EntryCounterReplacementReceipt::Compleated {
                        removed_loyalty, ..
                    } => {
                        Some(*removed_loyalty)
                    }
                    _ => None,
                })
                .collect::<Vec<_>>(),
            vec![removed[row]]
        );
    }
    let first_season = seasons[0];
    let season_choice = batch
        .applications
        .iter()
        .find(|application| {
            matches!(
                &application.replacement,
                CounterPlacementReplacement::Battlefield { instance, .. }
                    if instance.source_object_id == first_season
            )
        })
        .expect("the first registered Season has a current application")
        .choice_id;
    assert!(compleated_offer
        .candidate_object_ids
        .contains(&season_choice));
    let season_result = engine
        .apply_command(
            0,
            &submit_identity_choice(compleated_offer.resolution_prompt_id, season_choice),
        )
        .expect("accept one authentic Season through normal command handling");
    let current_offer = identity_choice_offer(&season_result.events);
    assert_ne!(
        current_offer.resolution_prompt_id,
        compleated_offer.resolution_prompt_id
    );
    let current = if baseline == [1, 5] { [0, 8] } else { [4, 8] };
    let batch = match engine.state.pending_replacement_event.as_ref() {
        Some(PendingReplacementEvent::Counters(batch)) => batch,
        _ => panic!("two registered Seasons leave the history fixture parked"),
    };
    let rows = batch
        .events
        .iter()
        .filter(|proposal| {
            matches!(
                &proposal.recipient,
                CounterPlacementRecipient::EnteringObject { object_id, .. }
                    if *object_id == entrant
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(rows.len(), 2);
    for row in 0..2 {
        assert_eq!(
            rows[row].original_counters[&CounterKind::Loyalty],
            baseline[row]
        );
        assert_eq!(rows[row].counters[&CounterKind::Loyalty], current[row]);
        let receipts = &rows[row].entry_context.as_ref().unwrap().history.receipts;
        assert_eq!(
            receipts
                .iter()
                .filter_map(|receipt| match receipt {
                    EntryCounterReplacementReceipt::Compleated {
                        removed_loyalty, ..
                    } => {
                        Some(*removed_loyalty)
                    }
                    _ => None,
                })
                .collect::<Vec<_>>(),
            vec![removed[row]]
        );
        let season_receipts = receipts
            .iter()
            .filter_map(|receipt| match receipt {
                EntryCounterReplacementReceipt::Battlefield { instance } => {
                    Some(instance.source_object_id)
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        if current[row] > 0 {
            assert_eq!(season_receipts, vec![first_season]);
        } else {
            assert!(season_receipts.is_empty());
        }
    }
    let pending = engine.state.pending_resolution.as_ref().unwrap();
    assert_eq!(
        pending.resolution_prompt_id.get(),
        current_offer.resolution_prompt_id
    );
    assert!(matches!(
        &pending.continuation,
        crate::state::ResolutionContinuation::EntryCounterReplacement { resume }
            if matches!(resume.as_ref(), crate::state::EntryCounterReplacementResume::Single { .. })
    ));
    let crate::state::ResolutionContinuation::EntryCounterReplacement { resume } =
        &pending.continuation
    else {
        panic!("the numeric history fixture retains its entry continuation");
    };
    let crate::state::EntryCounterReplacementResume::Single { event, .. } = resume.as_ref() else {
        panic!("the numeric history fixture retains its authoritative Single event");
    };
    assert_eq!(event.object_id, entrant);
    assert!(!event.entry_counter_replacements_resolved);
    assert!(event
        .entry_counter_placements
        .iter()
        .all(|placement| placement.history.is_none() && placement.applied_replacements.is_empty()));
    (engine, entrant, seasons, first_season, stack_debug)
}

fn assert_compleated_numeric_history_forgery_rejected(
    engine: &mut GameEngine,
    _entrant: ObjectId,
    seasons: &[ObjectId],
) {
    let (prompt_id, application_id) = {
        let pending = engine.state.pending_resolution.as_ref().unwrap();
        let Some(PendingReplacementEvent::Counters(batch)) =
            engine.state.pending_replacement_event.as_ref()
        else {
            panic!("the forged proposal retains its counter batch");
        };
        let application = batch
            .applications
            .iter()
            .find(|application| {
                application.event_index == batch.event_index
                    && matches!(
                        &application.replacement,
                        CounterPlacementReplacement::Battlefield { instance, .. }
                            if seasons.contains(&instance.source_object_id)
                    )
            })
            .expect("a current remaining Season handle is still offered");
        assert!(pending
            .presentation
            .candidates
            .contains(&application.choice_id));
        (pending.resolution_prompt_id.get(), application.choice_id)
    };
    let after_mutation = engine.diagnostic_snapshot().unwrap();
    let command_index = engine.state.command_index;
    let rejected = engine.apply_command(0, &submit_identity_choice(prompt_id, application_id));
    assert!(
        matches!(rejected, Err(EngineError::Illegal(_))),
        "the current Season command must reject the forged history"
    );
    assert_eq!(engine.state.command_index, command_index);
    assert_eq!(engine.diagnostic_snapshot().unwrap(), after_mutation);

    let proposals = match engine.state.pending_replacement_event.as_ref() {
        Some(PendingReplacementEvent::Counters(batch)) => batch.events.clone(),
        _ => panic!("rejection preserves the forged proposal batch"),
    };
    let mut event = {
        let pending = engine.state.pending_resolution.as_ref().unwrap();
        let crate::state::ResolutionContinuation::EntryCounterReplacement { resume } =
            &pending.continuation
        else {
            panic!("rejection restores the entry continuation");
        };
        let crate::state::EntryCounterReplacementResume::Single { event, .. } = resume.as_ref()
        else {
            panic!("writeback uses the authoritative Single event clone");
        };
        assert!(event
            .entry_counter_placements
            .iter()
            .all(|placement| placement.history.is_none()));
        (**event).clone()
    };
    let unchanged = event.clone();
    let writeback = GameEngine::apply_entry_counter_placement_results(&mut event, &proposals);
    assert!(
        matches!(writeback, Err(EngineError::Illegal(_))),
        "direct entry writeback must reject the same forged history"
    );
    assert_eq!(format!("{event:?}"), format!("{unchanged:?}"));
}

#[test]
fn compleated_grouped_entry_replays_nonempty_history_once_after_regrouping_and_appending_contributor(
) {
    let worker = std::thread::Builder::new()
        .name("compleated-grouped-numeric-history-test3".to_owned())
        .stack_size(16 * 1024 * 1024)
        .spawn(compleated_grouped_numeric_history_test3_cases)
        .expect("failed to spawn Compleated numeric-history Test 3 worker");
    if let Err(payload) = worker.join() {
        std::panic::resume_unwind(payload);
    }
}

fn compleated_grouped_numeric_history_test3_cases() {
    // Positive control: real [1,5] receipts survive writeback and regrouping before a new
    // actor/source row is appended to the same parked Single continuation.
    let (mut engine, entrant, seasons, first_season, stack_debug) =
        compleated_grouped_numeric_history_after_one_season([1, 5]);
    let old_prompt = engine
        .state
        .pending_resolution
        .as_ref()
        .unwrap()
        .resolution_prompt_id
        .get();
    let batch = match engine.state.pending_replacement_event.as_ref() {
        Some(PendingReplacementEvent::Counters(batch)) => (**batch).clone(),
        _ => panic!("the authentic history has a parked counter batch"),
    };
    let mut event = {
        let pending = engine.state.pending_resolution.as_ref().unwrap();
        assert_eq!(
            format!("{:?}", pending.continuation.stack().unwrap()),
            stack_debug
        );
        let crate::state::ResolutionContinuation::EntryCounterReplacement { resume } =
            &pending.continuation
        else {
            panic!("the positive control retains an entry continuation");
        };
        let crate::state::EntryCounterReplacementResume::Single { event, .. } = resume.as_ref()
        else {
            panic!("the positive control retains a Single resume payload");
        };
        assert!(event
            .entry_counter_placements
            .iter()
            .all(|placement| placement.history.is_none()));
        (**event).clone()
    };
    GameEngine::apply_entry_counter_placement_results(&mut event, &batch.events)
        .expect("write back the authentic Compleated and Season histories");
    assert_eq!(
        event
            .entry_counter_placements
            .iter()
            .map(
                |placement| placement.history.as_ref().unwrap().baseline_counters
                    [&CounterKind::Loyalty]
            )
            .collect::<Vec<_>>(),
        vec![1, 5]
    );
    assert_eq!(
        event
            .entry_counter_placements
            .iter()
            .map(|placement| placement.counters[&CounterKind::Loyalty])
            .collect::<Vec<_>>(),
        vec![0, 8]
    );
    assert!(event.entry_counter_placements[0]
        .history
        .as_ref()
        .unwrap()
        .receipts
        .iter()
        .any(|receipt| matches!(receipt, EntryCounterReplacementReceipt::Compleated { .. })));
    assert!(event.entry_counter_placements[0]
        .history
        .as_ref()
        .unwrap()
        .receipts
        .iter()
        .all(|receipt| !matches!(receipt, EntryCounterReplacementReceipt::Battlefield { .. })));
    assert!(event.entry_counter_placements[1]
        .history
        .as_ref()
        .unwrap()
        .receipts
        .iter()
        .any(|receipt| matches!(
            receipt,
            EntryCounterReplacementReceipt::Battlefield { instance }
                if instance.source_object_id == first_season
        )));

    let new_source = object(&mut engine, "divination", Zone::Stack, 1);
    let source_generation = engine
        .state
        .zone_change_generation
        .get(&new_source)
        .copied()
        .unwrap_or(0);
    append_entry_counter_placement(
        &mut event,
        1,
        [(CounterKind::Loyalty, 3)],
        EntryCounterSource::ResolutionEffect {
            stack_object_id: new_source,
            stack_zone_change_generation: source_generation,
            effect_index: 0,
        },
        false,
    );
    assert_eq!(event.entry_counter_placements[2].actor, 1);
    assert!(matches!(
        &event.entry_counter_placements[2].source,
        EntryCounterSource::ResolutionEffect { stack_object_id, .. }
            if *stack_object_id == new_source
    ));
    assert!(event.entry_counter_placements[..2]
        .iter()
        .all(|placement| matches!(
            &placement.source,
            EntryCounterSource::ResolutionEffect { stack_object_id, .. }
                if *stack_object_id != new_source
        )));
    assert!(!event.entry_counter_replacements_resolved);
    assert_eq!(
        event
            .entry_counter_placements
            .iter()
            .map(|placement| placement
                .history
                .as_ref()
                .map(|history| history.baseline_counters[&CounterKind::Loyalty])
                .unwrap_or_else(|| placement.counters[&CounterKind::Loyalty]))
            .collect::<Vec<_>>(),
        vec![1, 5, 3]
    );
    assert_eq!(
        event
            .entry_counter_placements
            .iter()
            .map(|placement| placement.counters[&CounterKind::Loyalty])
            .collect::<Vec<_>>(),
        vec![0, 8, 3]
    );
    assert!(event.entry_counter_placements[2]
        .history
        .as_ref()
        .is_none_or(|history| history
            .receipts
            .iter()
            .all(|receipt| !matches!(receipt, EntryCounterReplacementReceipt::Compleated { .. }))));

    let references: [&BattlefieldEntryEvent; 1] = [&event];
    let proposals = engine.proposed_entry_counter_placements(&references, &[]);
    assert_eq!(proposals.len(), 3);
    let (counter_batch, candidates) = engine
        .prepare_counter_placement_batch(proposals)
        .expect("replay the accepted history and propose replacements for the new row");
    let candidates = candidates.expect("current Seasons repark the three-row entry");
    assert!(candidates.iter().all(|(replacement, _)| !matches!(
        replacement,
        CounterPlacementReplacement::Compleated { .. }
    )));
    let rows = counter_batch
        .events
        .iter()
        .filter(|proposal| {
            matches!(
                &proposal.recipient,
                CounterPlacementRecipient::EnteringObject { object_id, .. }
                    if *object_id == entrant
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(rows.len(), 3);
    assert_eq!(
        rows.iter()
            .map(|proposal| proposal.original_counters[&CounterKind::Loyalty])
            .collect::<Vec<_>>(),
        vec![1, 5, 3]
    );
    assert_eq!(
        rows.iter()
            .map(|proposal| proposal.counters[&CounterKind::Loyalty])
            .collect::<Vec<_>>(),
        vec![0, 8, 3]
    );
    assert!(rows[2]
        .entry_context
        .as_ref()
        .unwrap()
        .history
        .receipts
        .iter()
        .all(|receipt| !matches!(receipt, EntryCounterReplacementReceipt::Compleated { .. })));

    let mut resume = {
        let pending = engine.state.pending_resolution.as_ref().unwrap();
        let crate::state::ResolutionContinuation::EntryCounterReplacement { resume } =
            &pending.continuation
        else {
            panic!("the positive control retains its original entry continuation");
        };
        (**resume).clone()
    };
    match &mut resume {
        crate::state::EntryCounterReplacementResume::Single {
            stack,
            event: saved,
            ..
        } => {
            assert_eq!(format!("{stack:?}"), stack_debug);
            assert_eq!(saved.object_id, entrant);
            **saved = event.clone();
        }
        _ => panic!("the positive control reparks through its original Single resume"),
    }
    let mut events = Vec::new();
    engine.park_entry_counter_placement_choice(counter_batch, candidates, resume, &mut events);
    engine
        .publish_resolution_prompt_ids(&mut events)
        .expect("publish a fresh epoch for the amended three-row history");
    let mut offer = identity_choice_offer(&events);
    assert_ne!(offer.resolution_prompt_id, old_prompt);
    let mut first_season_reappeared_for_new_row = false;
    let mut final_probe_reached = false;
    let mut accepted_commands = 0;
    loop {
        let (prompt_id, application_id, members, mut probe_batch, operation, mut probe_event) = {
            let Some(pending) = engine.state.pending_resolution.as_ref() else {
                break;
            };
            if !matches!(
                &pending.continuation,
                crate::state::ResolutionContinuation::EntryCounterReplacement { .. }
            ) {
                break;
            }
            assert_eq!(
                pending.resolution_prompt_id.get(),
                offer.resolution_prompt_id
            );
            let Some(PendingReplacementEvent::Counters(batch)) =
                engine.state.pending_replacement_event.as_ref()
            else {
                panic!("each repark retains its current counter batch");
            };
            let application = batch
                .applications
                .iter()
                .find(|application| {
                    application.event_index == batch.event_index
                        && matches!(
                            &application.replacement,
                            CounterPlacementReplacement::Battlefield { instance, .. }
                                if instance.source_object_id == first_season
                        )
                })
                .or_else(|| {
                    batch.applications.iter().find(|application| {
                        application.event_index == batch.event_index
                            && matches!(
                                &application.replacement,
                                CounterPlacementReplacement::Battlefield { instance, .. }
                                    if seasons.contains(&instance.source_object_id)
                            )
                    })
                })
                .expect("a current eligible Season remains engine-offered");
            let (source, members) = match &application.replacement {
                CounterPlacementReplacement::Battlefield {
                    instance,
                    member_event_indices,
                    ..
                } => (instance.source_object_id, member_event_indices.clone()),
                CounterPlacementReplacement::Compleated { .. } => {
                    panic!("accepted Compleated is not offered again after regrouping")
                }
            };
            assert!(pending
                .presentation
                .candidates
                .contains(&application.choice_id));
            if source == first_season {
                assert_eq!(members, vec![2]);
                first_season_reappeared_for_new_row = true;
            }
            let resume_event = match &pending.continuation {
                crate::state::ResolutionContinuation::EntryCounterReplacement { resume } => {
                    match resume.as_ref() {
                        crate::state::EntryCounterReplacementResume::Single { event, .. } => {
                            (**event).clone()
                        }
                        _ => panic!("the positive control retains its Single event"),
                    }
                }
                _ => unreachable!("the pending continuation was checked above"),
            };
            (
                pending.resolution_prompt_id.get(),
                application.choice_id,
                members,
                (**batch).clone(),
                application.replacement.clone(),
                resume_event,
            )
        };
        assert!(!members.is_empty());

        // Probe the exact selected operation and any unique automatic remainder on clones.
        // The probe never replaces the authoritative pending batch or resume event.
        GameEngine::apply_counter_placement_replacement(&mut probe_batch, &operation)
            .expect("apply the offered replacement to a cloned probe batch");
        if engine
            .advance_counter_placement_batch(&mut probe_batch)
            .expect("advance the cloned probe batch")
            .is_none()
        {
            GameEngine::apply_entry_counter_placement_results(
                &mut probe_event,
                &probe_batch.events,
            )
            .expect("write the completed cloned probe back to its cloned event");
            assert_eq!(
                probe_batch
                    .events
                    .iter()
                    .map(|proposal| proposal.original_counters[&CounterKind::Loyalty])
                    .collect::<Vec<_>>(),
                vec![1, 5, 3]
            );
            assert_eq!(
                probe_event
                    .entry_counter_placements
                    .iter()
                    .map(
                        |placement| placement.history.as_ref().unwrap().baseline_counters
                            [&CounterKind::Loyalty]
                    )
                    .collect::<Vec<_>>(),
                vec![1, 5, 3]
            );
            assert_eq!(
                probe_event
                    .entry_counter_placements
                    .iter()
                    .map(|placement| placement.counters[&CounterKind::Loyalty])
                    .collect::<Vec<_>>(),
                vec![0, 32, 24]
            );
            for (row, placement) in probe_event.entry_counter_placements.iter().enumerate() {
                let history = placement.history.as_ref().unwrap();
                let compleated_removals = history
                    .receipts
                    .iter()
                    .filter_map(|receipt| match receipt {
                        EntryCounterReplacementReceipt::Compleated {
                            removed_loyalty, ..
                        } => Some(*removed_loyalty),
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                assert_eq!(
                    compleated_removals,
                    match row {
                        0 => vec![1],
                        1 => vec![1],
                        2 => Vec::new(),
                        _ => unreachable!(),
                    }
                );
                let battlefield_suffix = history
                    .receipts
                    .iter()
                    .filter_map(|receipt| match receipt {
                        EntryCounterReplacementReceipt::Battlefield { instance } => {
                            Some(instance.clone())
                        }
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                let mut expected_applied = history.baseline_battlefield_receipts.clone();
                expected_applied.extend(battlefield_suffix.iter().cloned());
                assert_eq!(placement.applied_replacements, expected_applied);
                let mut battlefield_sources = battlefield_suffix
                    .iter()
                    .map(|receipt| receipt.source_object_id)
                    .collect::<Vec<_>>();
                battlefield_sources.sort_unstable();
                if row == 0 {
                    assert!(battlefield_sources.is_empty());
                } else {
                    let mut expected_sources = seasons.clone();
                    expected_sources.sort_unstable();
                    assert_eq!(battlefield_sources, expected_sources);
                }
            }
            final_probe_reached = true;
        }

        let command_index = engine.state.command_index;
        let accepted = engine
            .apply_command(0, &submit_identity_choice(prompt_id, application_id))
            .expect("finish the amended history through normal offered choices");
        assert_eq!(engine.state.command_index, command_index + 1);
        accepted_commands += 1;
        assert!(accepted_commands <= 9);
        if let Some(pending) = engine.state.pending_resolution.as_ref() {
            if matches!(
                &pending.continuation,
                crate::state::ResolutionContinuation::EntryCounterReplacement { .. }
            ) {
                offer = identity_choice_offer(&accepted.events);
                assert_eq!(
                    offer.resolution_prompt_id,
                    pending.resolution_prompt_id.get()
                );
                assert_ne!(offer.resolution_prompt_id, prompt_id);
            } else {
                break;
            }
        } else {
            break;
        }
    }
    assert!(accepted_commands > 0);
    assert!(first_season_reappeared_for_new_row);
    assert!(
        final_probe_reached,
        "the completed-history probe reached no remaining choices"
    );
    assert_eq!(engine.state.objects[&entrant].zone, Zone::Battlefield);
    let expected_final_rows = [0_u32, 32, 24];
    assert_eq!(expected_final_rows.iter().sum::<u32>(), 56);
    assert_eq!(
        engine.state.objects[&entrant].counter_count(CounterKind::Loyalty),
        expected_final_rows.iter().sum::<u32>()
    );

    let (mut amount_engine, amount_entrant, amount_seasons, _, _) =
        compleated_grouped_numeric_history_after_one_season([1, 5]);
    {
        let Some(PendingReplacementEvent::Counters(batch)) =
            amount_engine.state.pending_replacement_event.as_mut()
        else {
            panic!("the amount forgery starts from a parked authentic batch");
        };
        let mut rows = batch
            .events
            .iter_mut()
            .filter(|proposal| {
                matches!(
                    &proposal.recipient,
                    CounterPlacementRecipient::EnteringObject { object_id, .. }
                        if *object_id == amount_entrant
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(rows.len(), 2);
        assert_eq!(
            rows.iter()
                .map(|proposal| proposal.counters[&CounterKind::Loyalty])
                .collect::<Vec<_>>(),
            vec![0, 8]
        );
        assert!(rows.iter().all(|proposal| !proposal
            .entry_context
            .as_ref()
            .unwrap()
            .history
            .receipts
            .is_empty()));
        rows[1].counters.insert(CounterKind::Loyalty, 9);
        assert_eq!(
            rows[1]
                .entry_context
                .as_ref()
                .unwrap()
                .history
                .receipts
                .len(),
            2
        );
    }
    assert_compleated_numeric_history_forgery_rejected(
        &mut amount_engine,
        amount_entrant,
        &amount_seasons,
    );

    let (mut attribution_engine, attribution_entrant, attribution_seasons, _, _) =
        compleated_grouped_numeric_history_after_one_season([4, 4]);
    {
        let Some(PendingReplacementEvent::Counters(batch)) =
            attribution_engine.state.pending_replacement_event.as_mut()
        else {
            panic!("the attribution forgery starts from a parked authentic batch");
        };
        let indices = batch
            .events
            .iter()
            .enumerate()
            .filter_map(|(index, proposal)| {
                matches!(
                    &proposal.recipient,
                    CounterPlacementRecipient::EnteringObject { object_id, .. }
                        if *object_id == attribution_entrant
                )
                .then_some(index)
            })
            .collect::<Vec<_>>();
        assert_eq!(indices.len(), 2);
        assert_eq!(
            indices
                .iter()
                .map(|index| batch.events[*index].counters[&CounterKind::Loyalty])
                .collect::<Vec<_>>(),
            vec![4, 8]
        );
        for (row, removal, amount) in [(0, 0, 8), (1, 2, 4)] {
            let proposal = &mut batch.events[indices[row]];
            proposal.counters.insert(CounterKind::Loyalty, amount);
            let history = &mut proposal.entry_context.as_mut().unwrap().history;
            let mut found = false;
            for receipt in &mut history.receipts {
                if let EntryCounterReplacementReceipt::Compleated {
                    removed_loyalty, ..
                } = receipt
                {
                    *removed_loyalty = removal;
                    found = true;
                    break;
                }
            }
            assert!(found);
            assert!(history.receipts.iter().any(|receipt| matches!(
                receipt,
                EntryCounterReplacementReceipt::Battlefield { .. }
            )));
        }
    }
    assert_compleated_numeric_history_forgery_rejected(
        &mut attribution_engine,
        attribution_entrant,
        &attribution_seasons,
    );

    let (mut duplicate_engine, duplicate_entrant, duplicate_seasons, _, _) =
        compleated_grouped_numeric_history_after_one_season([1, 5]);
    {
        let Some(PendingReplacementEvent::Counters(batch)) =
            duplicate_engine.state.pending_replacement_event.as_mut()
        else {
            panic!("the duplicate forgery starts from a parked authentic batch");
        };
        let index = batch
            .events
            .iter()
            .position(|proposal| {
                matches!(
                    &proposal.recipient,
                    CounterPlacementRecipient::EnteringObject { object_id, .. }
                        if *object_id == duplicate_entrant
                ) && proposal.original_counters.get(&CounterKind::Loyalty) == Some(&1)
            })
            .expect("the original one-loyalty row is parked at zero");
        let proposal = &mut batch.events[index];
        assert_eq!(proposal.counters[&CounterKind::Loyalty], 0);
        let history = &mut proposal.entry_context.as_mut().unwrap().history;
        let duplicate = history
            .receipts
            .iter()
            .find_map(|receipt| match receipt {
                EntryCounterReplacementReceipt::Compleated { identity, .. } => {
                    Some(EntryCounterReplacementReceipt::Compleated {
                        identity: identity.clone(),
                        removed_loyalty: 0,
                    })
                }
                _ => None,
            })
            .expect("the zero row retains its real Compleated receipt");
        history.receipts.push(duplicate);
        assert_eq!(
            history
                .receipts
                .iter()
                .filter(|receipt| matches!(
                    receipt,
                    EntryCounterReplacementReceipt::Compleated { .. }
                ))
                .count(),
            2
        );
    }
    assert_compleated_numeric_history_forgery_rejected(
        &mut duplicate_engine,
        duplicate_entrant,
        &duplicate_seasons,
    );
}
