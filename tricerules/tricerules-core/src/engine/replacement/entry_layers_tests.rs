use super::*;

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
        },
        GameEvent::EntersBattlefield {
            object_id: support,
            chosen_x: 0,
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
    let mut engine = GameEngine::new_with_default_decks(
        tricerules_cards::registry::global(),
        305_030,
        &[0, 1],
        20,
    )
    .unwrap();
    engine.state.opening = None;
    engine.state.priority_idx = 0;
    engine.state.turn_step = TurnStep::Main1;
    engine.state.command_index = 5;
    engine
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
    prism_entry.entry_counters.insert(CounterKind::Charge, 2);
    let mut entries = vec![season_entry, prism_entry];

    engine.replace_entry_counter_events(entries.iter_mut());
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
            .apply_command(
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
                .apply_command(
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
            engine.apply_command(0, &answer).is_err(),
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
        entry_modifiers: Vec::new(),
        attached_to: None,
        pending_copy_candidate: None,
        pending_aura_recipient: None,
        accepted_aura_recipient: None,
        applied_effects: Vec::new(),
    }
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
            ParkedStackResolution::new(engine.observer_return_item(molten_echoes, 0)),
            vec![event(&engine, bear), event(&engine, molten_echoes)],
            Zone::Graveyard,
            "simultaneous return",
            None,
            &mut events,
        )
        .expect("the simultaneous entry batch is valid");
    assert!(resumed.is_none(), "the type choice parks the whole batch");
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
        .apply_command(
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
        .apply_command(
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
            .apply_command(
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
    let mut stack = ParkedStackResolution::new(engine.state.stack.pop().unwrap());
    stack.resume_effect_index = Some(0);
    stack
        .previous_result
        .produced_objects
        .push(TriggerObjectRef {
            object_id: spell,
            zone_change_generation: engine.state.zone_change_generation[&spell],
            controller_at_event: 0,
        });
    stack
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
        let expected_previous = format!("{:?}", stack.previous_result);
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
            pending.continuation.stack().unwrap().resume_effect_index,
            Some(0)
        );
        assert_eq!(
            format!(
                "{:?}",
                pending.continuation.stack().unwrap().previous_result
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
        assert!(engine.apply_command(1, &answer(recipient)).is_err());
        assert!(engine.apply_command(0, &answer(protected)).is_err());
        assert_eq!(format!("{:?}", engine.state), before);
        let batch = engine.apply_command(0, &answer(recipient)).unwrap();
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
        entry.entry_counters.insert(CounterKind::Stun, 2);
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
        engine
            .complete_parked_resolution_with_previous(
                resumed.item,
                resumed.resume_effect_index,
                resumed.previous_result,
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
            stack.resume_effect_index = Some(1);
            let expected_previous = format!("{:?}", stack.previous_result);
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
                engine.apply_command(0, &answer(chosen)).unwrap();
            }
            let pending = engine.state.pending_resolution.as_ref().unwrap();
            assert_eq!(
                pending.presentation.choice_kind,
                rv1::ChoiceKind::AuraPermanent
            );
            assert_eq!(
                format!(
                    "{:?}",
                    pending.continuation.stack().unwrap().previous_result
                ),
                expected_previous
            );
            assert_eq!(
                pending.continuation.stack().unwrap().resume_effect_index,
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
                assert!(engine.apply_command(0, &answer(recipient)).is_err());
                assert_eq!(format!("{:?}", engine.state), before);
                assert_eq!(engine.state.objects[&aura].zone, Zone::Library);
            } else {
                engine.apply_command(0, &answer(recipient)).unwrap();
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
        .apply_command(
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
                .apply_command(
                    0,
                    &rv1::RuledCommand {
                        cmd: Some(rv1::ruled_command::Cmd::PlayLand(rv1::PlayLand {
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
                    ParkedStackResolution::new(item),
                    entries,
                    Zone::Hand,
                    "internal simultaneous hand instruction",
                    None,
                    &mut events
                )
                .unwrap()
                .is_none());
            let choose = |ids| rv1::RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(
                    rv1::SubmitResolutionChoice {
                        chosen_object_ids: ids,
                        ..Default::default()
                    },
                )),
            };
            let first_batch = engine.apply_command(0, &choose(vec![forest])).unwrap();
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
                assert!(engine.apply_command(0, &choose(vec![outside])).is_err());
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
            let second_batch = engine.apply_command(0, &choose(vec![forest])).unwrap();
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
            engine.apply_command(0, &choose(order)).unwrap();
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
    };
    engine.state.pending_observer_return_batch = Some(observer.clone());
    assert_eq!(
        super::super::reveals::active_reveals(&engine),
        vec![receipt(first)]
    );
    engine.state.pending_observer_return_batch = None;
    engine.state.pending_resolution = Some(PendingResolution {
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
            .apply_command(
                0,
                &rv1::RuledCommand {
                    cmd: Some(rv1::ruled_command::Cmd::PlayLand(rv1::PlayLand {
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
        assert!(engine.apply_command(0, &answer).is_err());
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
    entry.entry_counters.insert(CounterKind::Charge, 2);
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
    entry.entry_counters.insert(CounterKind::Loyalty, 4);
    let preview = engine
        .battlefield_entry_characteristics_through_layer_5(&entry)
        .unwrap();
    assert!(preview.is_creature() && preview.has_type("Ninja"));
    assert!(!preview.has_type("Planeswalker"));
    engine
        .commit_battlefield_entry(entry, None, &mut Vec::new())
        .unwrap();
    assert_eq!(
        engine.characteristics_through_layer_5(oid).unwrap(),
        preview
    );
}
