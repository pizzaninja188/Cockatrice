use super::*;

fn ready_engine(seed: u64) -> GameEngine {
    let mut engine = GameEngine::new_with_default_decks(seed, &[0, 1], 20).unwrap();
    engine.state.opening = None;
    engine.state.turn_step = TurnStep::Main1;
    engine.state.priority_idx = 0;
    engine
}

fn permanent(engine: &mut GameEngine, card: &str) -> ObjectId {
    insert_permanent(engine, card, false)
}

fn face_down_permanent(engine: &mut GameEngine, card: &str) -> ObjectId {
    insert_permanent(engine, card, true)
}

fn insert_permanent(engine: &mut GameEngine, card: &str, face_down: bool) -> ObjectId {
    let oid = engine.state.next_object_id;
    engine.state.next_object_id += 1;
    let mut object = engine.state.objects.values().next().unwrap().clone();
    object.id = oid;
    object.card_id = card.into();
    object.owner = 0;
    object.base_controller = 0;
    object.controller = 0;
    object.zone = Zone::Battlefield;
    object.face_up_index = 0;
    object.face_down = face_down;
    object.token_origin = None;
    object.token_faces = None;
    object.copiable_values = None;
    object.tapped = false;
    object.summoning_sick = false;
    object.power = None;
    object.toughness = None;
    engine.state.objects.insert(oid, object);
    engine.state.players[0].battlefield.push(oid);
    engine.reconcile_activated_ability_slots();
    oid
}

fn effect(engine: &mut GameEngine, source: ObjectId, kind: ContinuousEffectKind, timestamp: u64) {
    let effect = ContinuousEffect {
        trigger_grant_origin: None,
        source_id: None,
        affected: AffectedScope::Single(source),
        kind,
        condition: None,
        duration: EffectDuration::UntilEndOfTurn,
        timestamp,
    };
    if matches!(effect.kind, ContinuousEffectKind::GrantActivatedAbility(_)) {
        engine.state.add_activated_ability_grant(effect);
    } else {
        engine.state.continuous_effects.push(effect);
    }
    engine.reconcile_activated_ability_slots();
}

fn forest_setting() -> ContinuousEffectKind {
    ContinuousEffectKind::Layer4SetTypeLine(tricerules_cards::TypeLineReplacement {
        card_types: vec![PermanentTypeFilter::Land],
        creature_types: Vec::new(),
        land_types: vec![BasicLandType::Forest],
    })
}

fn activate(engine: &GameEngine, source: ObjectId, index: u32, option: u32) -> rv1::RuledCommand {
    rv1::RuledCommand {
        cmd: Some(rv1::ruled_command::Cmd::ActivateAbility(
            rv1::ActivateAbility {
                source_object_id: source,
                ability_index: index,
                source_zone: rv1::AbilitySourceZone::Battlefield as i32,
                expected_zone_change_generation: engine
                    .state
                    .zone_change_generation
                    .get(&source)
                    .copied()
                    .unwrap_or(0),
                mana_option_index: option,
                ..Default::default()
            },
        )),
    }
}

#[test]
fn intrinsic_land_mana_derives_unbaked_subtypes_and_preserves_authored_option_order() {
    let mut engine = ready_engine(305_020);
    for (card, expected) in [
        (
            "dryad_arbor",
            vec![ManaAmount {
                g: 1,
                ..Default::default()
            }],
        ),
        (
            "commercial_district",
            vec![
                ManaAmount {
                    r: 1,
                    ..Default::default()
                },
                ManaAmount {
                    g: 1,
                    ..Default::default()
                },
            ],
        ),
        (
            "savannah",
            vec![
                ManaAmount {
                    g: 1,
                    ..Default::default()
                },
                ManaAmount {
                    w: 1,
                    ..Default::default()
                },
            ],
        ),
    ] {
        let source = permanent(&mut engine, card);
        let catalog = engine.effective_activated_abilities(source);
        assert_eq!(
            catalog.len(),
            1,
            "{card} derives exactly one subtype mana bundle"
        );
        assert!(catalog[0].definition.intrinsic_land_mana);
        assert_eq!(
            engine
                .active_mana_options(source, &catalog[0].definition)
                .unwrap(),
            expected
        );
        if card == "savannah" {
            effect(
                &mut engine,
                source,
                ContinuousEffectKind::Layer4AddTypes(tricerules_cards::TypeLineAddition {
                    land_types: vec![BasicLandType::Island],
                    ..Default::default()
                }),
                1,
            );
            let updated = engine.effective_activated_abilities(source);
            assert_eq!(updated[0].slot, catalog[0].slot);
            assert_eq!(
                engine
                    .active_mana_options(source, &updated[0].definition)
                    .unwrap(),
                vec![
                    ManaAmount {
                        g: 1,
                        ..Default::default()
                    },
                    ManaAmount {
                        w: 1,
                        ..Default::default()
                    },
                    ManaAmount {
                        u: 1,
                        ..Default::default()
                    },
                ]
            );
        }
    }
}

#[test]
fn intrinsic_land_mana_catalog_preserves_authored_holes_and_independent_identical_grants() {
    let card = r#"(id: "mana_index_probe", name: "Mana Index Probe", face_id: "mana_index_probe", types: ["Artifact"],
        activated_abilities: [
            (ability_id: "printed_c", presentation: Fallback, costs: [Tap], effect: [ProduceMana(options: [(c: 1)])]),
            (ability_id: "cycling", presentation: Fallback, source_zone: Hand, costs: [Mana("{2}"), DiscardSelf], effect: [Draw(count: 1)]),
            (ability_id: "printed_r", presentation: Fallback, costs: [Tap], effect: [ProduceMana(options: [(r: 1)])])])"#;
    let registry = CardRegistry::from_chunks_and_tokens(&[card], &[]).unwrap();
    let mut engine = ready_engine(305_021);
    engine.registry = Box::leak(Box::new(registry));
    let source = permanent(&mut engine, "mana_index_probe");
    let mut gift = CardRegistry::global()
        .get("forest")
        .unwrap()
        .primary_face()
        .activated_abilities[0]
        .clone();
    gift.intrinsic_land_mana = false;
    gift.ability_id = tricerules_cards::AbilityId::new("gift_green").unwrap();
    gift.presentation = tricerules_cards::AbilityPresentation::Fallback;
    effect(
        &mut engine,
        source,
        ContinuousEffectKind::GrantActivatedAbility(Box::new(gift)),
        5,
    );
    let indices = |engine: &GameEngine| {
        engine
            .effective_activated_abilities(source)
            .iter()
            .map(|entry| entry.slot)
            .collect::<Vec<_>>()
    };
    assert_eq!(
        indices(&engine),
        [0, 2, 4],
        "grants follow the complete authored span and reserved intrinsic slot"
    );
    effect(
        &mut engine,
        source,
        ContinuousEffectKind::Layer4AddTypes(tricerules_cards::TypeLineAddition {
            card_types: vec![PermanentTypeFilter::Land],
            land_types: vec![BasicLandType::Forest],
            ..Default::default()
        }),
        2,
    );
    assert_eq!(indices(&engine), [0, 2, 3, 4]);
    effect(&mut engine, source, forest_setting(), 3);
    let catalog = engine.effective_activated_abilities(source);
    assert_eq!(indices(&engine), [3, 4]);
    assert!(
        catalog[0].definition.intrinsic_land_mana && !catalog[1].definition.intrinsic_land_mana
    );
    assert_eq!(
        engine.active_mana_options(source, &catalog[0].definition),
        engine.active_mana_options(source, &catalog[1].definition)
    );
    for (player, option, stale) in [(1, 0, false), (0, 1, false), (0, 0, true)] {
        let mut command = activate(&engine, source, 3, option);
        if stale {
            if let Some(rv1::ruled_command::Cmd::ActivateAbility(action)) = command.cmd.as_mut() {
                action.expected_zone_change_generation += 1;
            }
        }
        let before = engine.diagnostic_snapshot().unwrap();
        assert!(engine.apply_command(player, &command).is_err());
        assert_eq!(engine.diagnostic_snapshot().unwrap(), before);
    }
    for index in [3, 4] {
        engine.state.objects.get_mut(&source).unwrap().tapped = false;
        engine
            .apply_command(0, &activate(&engine, source, index, 0))
            .unwrap();
    }
    assert_eq!(engine.state.players[0].mana_pool.green, 2);
    engine
        .state
        .continuous_effects
        .retain(|effect| !matches!(effect.kind, ContinuousEffectKind::Layer4SetTypeLine(_)));
    assert_eq!(indices(&engine), [0, 2, 3, 4]);
    engine
        .state
        .continuous_effects
        .retain(|effect| !matches!(effect.kind, ContinuousEffectKind::Layer4AddTypes(_)));
    assert_eq!(indices(&engine), [0, 2, 4]);
}

#[test]
fn intrinsic_land_mana_face_down_offers_and_activation_logs_hide_underlying_identity() {
    let mut offers = Vec::new();
    for card in ["sol_ring", "fetid_pools"] {
        let mut engine = ready_engine(305_022);
        let source = face_down_permanent(&mut engine, card);
        effect(&mut engine, source, forest_setting(), 2);
        let catalog = engine.effective_activated_abilities(source);
        assert_eq!(
            catalog.len(),
            1,
            "a face-down object made Forest receives intrinsic mana"
        );
        assert_eq!(
            catalog[0].slot, 0,
            "concealed authored span never determines the public index"
        );
        let offer =
            super::super::legal_actions::activated_ability_info(&engine, source, &catalog[0]);
        assert!(offer.presentation.is_none());
        offers.push(offer);
        let batch = engine
            .apply_command(0, &activate(&engine, source, 0, 0))
            .unwrap();
        assert_eq!(engine.state.players[0].mana_pool.green, 1);
        let logs = batch
            .events
            .iter()
            .filter_map(|event| match &event.ev {
                Some(rv1::ruled_event::Ev::Log(log)) => Some(log),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert!(!logs.is_empty());
        for log in logs {
            let text = format!("{log:?}");
            assert!(
                !text.contains(card) && !text.contains("Sol Ring") && !text.contains("Fetid Pools")
            );
            assert!(log.ability_presentation.is_none());
        }
    }
    assert_eq!(offers[0], offers[1]);
}

#[test]
fn intrinsic_land_mana_layer_six_removal_overrides_any_type_setting_timestamp() {
    for (setting_time, removal_time) in [(1, 2), (2, 1)] {
        let mut engine = ready_engine(305_023);
        let source = permanent(&mut engine, "sol_ring");
        effect(&mut engine, source, forest_setting(), setting_time);
        effect(
            &mut engine,
            source,
            ContinuousEffectKind::Layer6RemoveAllAbilities,
            removal_time,
        );
        assert!(
            engine.effective_activated_abilities(source).is_empty(),
            "layer 6 follows layer 4 regardless of timestamp"
        );
        engine.state.continuous_effects.retain(|effect| {
            !matches!(effect.kind, ContinuousEffectKind::Layer6RemoveAllAbilities)
        });
        assert_eq!(engine.effective_activated_abilities(source).len(), 1);
    }
}

fn face_down_granted_damage_fixture() -> (GameEngine, ObjectId) {
    let mut engine = ready_engine(305_024);
    let source = face_down_permanent(&mut engine, "sol_ring");
    effect(&mut engine, source, forest_setting(), 2);
    let mut ability = CardRegistry::global()
        .get("forest")
        .unwrap()
        .primary_face()
        .activated_abilities[0]
        .clone();
    ability.intrinsic_land_mana = false;
    ability.effect = vec![SpellEffectKind::DamageTarget {
        amount: tricerules_cards::Amount::Fixed(1),
        target: tricerules_cards::primitives::TargetFilter {
            kind: tricerules_cards::primitives::TargetKind::AnyTarget,
            ..Default::default()
        },
    }];
    ability.conditions = vec![GameCondition::ActivePlayer {
        players: RelativePlayerSet::Controller,
    }];
    effect(
        &mut engine,
        source,
        ContinuousEffectKind::GrantActivatedAbility(Box::new(ability)),
        3,
    );
    (engine, source)
}

#[test]
fn intrinsic_land_mana_face_down_granted_annotation_hides_identity() {
    let (mut engine, source) = face_down_granted_damage_fixture();
    let Some(rv1::ruled_event::Ev::ZoneView(view)) = engine.ev_zone_view_sync().ev else {
        panic!("zone view");
    };
    let objects = view
        .per_player
        .iter()
        .flat_map(|player| &player.battlefield_objects)
        .filter(|object| object.object_id == source)
        .collect::<Vec<_>>();
    assert!(!objects.is_empty());
    for object in objects {
        let text = format!("{:?}", object.rules_annotation_labels);
        assert!(
            !text.contains("Sol Ring") && !text.contains("sol_ring"),
            "{text}"
        );
    }
}

#[test]
fn intrinsic_land_mana_face_down_granted_nonmana_logs_and_resolution_hide_identity() {
    let (mut engine, source) = face_down_granted_damage_fixture();
    let mut command = activate(&engine, source, 1, 0);
    if let Some(rv1::ruled_command::Cmd::ActivateAbility(action)) = command.cmd.as_mut() {
        action.targets = vec![rv1::TargetRef {
            kind: rv1::TargetRefKind::Player as i32,
            object_id: 1,
            ..Default::default()
        }];
    }
    let activated = engine.apply_command(0, &command).unwrap();
    // Resolution must retain the public activation label even after the source is revealed.
    engine.state.objects.get_mut(&source).unwrap().face_down = false;
    let pass = rv1::RuledCommand {
        cmd: Some(rv1::ruled_command::Cmd::PassPriority(rv1::PassPriority {})),
    };
    engine.apply_command(0, &pass).unwrap();
    let resolved = engine.apply_command(1, &pass).unwrap();
    assert_eq!(engine.state.players[1].life, 19);
    for event in activated.events.iter().chain(&resolved.events) {
        match &event.ev {
            Some(rv1::ruled_event::Ev::Log(_)) | Some(rv1::ruled_event::Ev::StackPushed(_)) => {
                let text = format!("{event:?}");
                assert!(
                    !text.contains("Sol Ring") && !text.contains("sol_ring"),
                    "{text}"
                );
            }
            _ => {}
        }
    }
}

#[test]
fn intrinsic_land_mana_countered_face_down_granted_ability_hides_identity() {
    let (mut engine, source) = face_down_granted_damage_fixture();
    let mut command = activate(&engine, source, 1, 0);
    if let Some(rv1::ruled_command::Cmd::ActivateAbility(action)) = command.cmd.as_mut() {
        action.targets = vec![rv1::TargetRef {
            kind: rv1::TargetRefKind::Player as i32,
            object_id: 1,
            ..Default::default()
        }];
    }
    engine.apply_command(0, &command).unwrap();
    let stack_id = engine.state.stack.last().unwrap().id;
    let mut events = Vec::new();
    super::super::resolution::counter_stack_object(
        &mut engine,
        stack_id,
        "Tishana's Tidebinder",
        &mut events,
    )
    .unwrap()
    .unwrap();
    assert!(engine.state.stack.is_empty());
    let text = format!("{events:?}");
    assert!(
        !text.contains("Sol Ring") && !text.contains("sol_ring"),
        "{text}"
    );
    assert!(text.contains("Face-down permanent"), "{text}");
}

#[test]
fn intrinsic_land_mana_targeted_granted_trigger_captures_concealed_label_before_reveal() {
    let mut engine = ready_engine(305_028);
    let source = face_down_permanent(&mut engine, "sol_ring");
    effect(&mut engine, source, forest_setting(), 1);
    let definition = &CardRegistry::global()
        .get("thorin_oakenshield")
        .unwrap()
        .primary_face()
        .static_abilities[1]
        .definition;
    let StaticAbilityDef::GrantTriggeredAbilityToPermanents {
        triggered_abilities,
        ..
    } = definition
    else {
        panic!("Thorin grant");
    };
    let mut ability = triggered_abilities[0].clone();
    ability.effect = vec![SpellEffectKind::DamageTarget {
        amount: tricerules_cards::Amount::Fixed(1),
        target: tricerules_cards::primitives::TargetFilter {
            kind: tricerules_cards::primitives::TargetKind::AnyTarget,
            ..Default::default()
        },
    }];
    effect(
        &mut engine,
        source,
        ContinuousEffectKind::GrantTriggeredAbility(Box::new(ability)),
        2,
    );
    engine
        .state
        .continuous_effects
        .last_mut()
        .unwrap()
        .trigger_grant_origin = Some(TriggerAbilityOrigin::ResolvingGrant(305_028));
    let collected = engine.matching_triggered_abilities("sol_ring", source, 0, 0, |_| true);
    assert_eq!(collected.len(), 1);
    assert_eq!(collected[0].source_label, "Face-down permanent");
    assert!(collected[0].presentation.is_none());
    engine.state.objects.get_mut(&source).unwrap().face_down = false;
    engine.stage_triggers(collected);
    let mut events = Vec::new();
    engine.flush_staged_triggers(&mut events);
    assert_eq!(
        engine.state.pending_triggers.front().unwrap().source_label,
        "Face-down permanent"
    );
    let chosen = engine
        .apply_command(
            0,
            &rv1::RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::ChooseTriggerTarget(
                    rv1::ChooseTriggerTarget {
                        targets: vec![rv1::TargetRef {
                            kind: rv1::TargetRefKind::Player as i32,
                            object_id: 1,
                            ..Default::default()
                        }],
                        ..Default::default()
                    },
                )),
            },
        )
        .unwrap();
    events.extend(chosen.events);
    let pass = rv1::RuledCommand {
        cmd: Some(rv1::ruled_command::Cmd::PassPriority(rv1::PassPriority {})),
    };
    events.extend(engine.apply_command(0, &pass).unwrap().events);
    events.extend(engine.apply_command(1, &pass).unwrap().events);
    assert_eq!(engine.state.players[1].life, 19);
    assert!(engine.state.stack.is_empty());
    for event in events {
        if matches!(
            &event.ev,
            Some(rv1::ruled_event::Ev::Log(_))
                | Some(rv1::ruled_event::Ev::TriggerNeedsTarget(_))
                | Some(rv1::ruled_event::Ev::StackPushed(_))
        ) {
            let text = format!("{event:?}");
            assert!(
                !text.contains("Sol Ring") && !text.contains("sol_ring"),
                "{text}"
            );
        }
    }
}

#[test]
fn intrinsic_land_mana_mixed_zone_face_reserves_full_span_for_grants() {
    let mut engine = ready_engine(305_025);
    let source = permanent(&mut engine, "fetid_pools");
    let mut ability = CardRegistry::global()
        .get("forest")
        .unwrap()
        .primary_face()
        .activated_abilities[0]
        .clone();
    ability.intrinsic_land_mana = false;
    effect(
        &mut engine,
        source,
        ContinuousEffectKind::GrantActivatedAbility(Box::new(ability)),
        1,
    );
    effect(
        &mut engine,
        source,
        ContinuousEffectKind::Layer4AddTypes(tricerules_cards::TypeLineAddition {
            land_types: vec![BasicLandType::Forest],
            ..Default::default()
        }),
        2,
    );
    let catalog = engine.effective_activated_abilities(source);
    assert_eq!(
        catalog.iter().map(|entry| entry.slot).collect::<Vec<_>>(),
        [0, 2]
    );
    assert_eq!(
        engine
            .active_mana_options(source, &catalog[0].definition)
            .unwrap(),
        vec![
            BasicLandType::Island.mana(),
            BasicLandType::Swamp.mana(),
            BasicLandType::Forest.mana()
        ]
    );
    assert_eq!(
        engine.effective_face(source).unwrap().activated_abilities[1].source_zone,
        AbilitySourceZone::Hand
    );
    let arbor = permanent(&mut engine, "dryad_arbor");
    engine.state.objects.get_mut(&arbor).unwrap().summoning_sick = true;
    assert!(!engine.ability_activatable(
        arbor,
        0,
        &engine.effective_activated_abilities(arbor)[0].definition
    ));
    assert!(engine
        .apply_command(0, &activate(&engine, arbor, 0, 0))
        .is_err());
}

#[test]
fn intrinsic_land_mana_equal_timestamp_removal_orders_independent_grants() {
    for removal_first in [false, true] {
        let mut engine = ready_engine(305_026);
        let source = permanent(&mut engine, "sol_ring");
        effect(&mut engine, source, forest_setting(), 1);
        let mut ability = CardRegistry::global()
            .get("forest")
            .unwrap()
            .primary_face()
            .activated_abilities[0]
            .clone();
        ability.intrinsic_land_mana = false;
        let grant = ContinuousEffectKind::GrantActivatedAbility(Box::new(ability));
        let removal = ContinuousEffectKind::Layer6RemoveAllAbilities;
        let ordered = if removal_first {
            [removal, grant]
        } else {
            [grant, removal]
        };
        for kind in ordered {
            effect(&mut engine, source, kind, 5);
        }
        let catalog = engine.effective_activated_abilities(source);
        assert_eq!(catalog.len(), usize::from(removal_first));
        if removal_first {
            assert_eq!(catalog[0].slot, 2);
            assert!(!catalog[0].definition.intrinsic_land_mana);
        }
    }
}

#[test]
fn intrinsic_land_mana_ignores_inactive_and_stale_static_removal() {
    let mut engine = ready_engine(305_027);
    let source = permanent(&mut engine, "forest");
    effect(
        &mut engine,
        source,
        ContinuousEffectKind::Layer6RemoveAllAbilities,
        1,
    );
    engine
        .state
        .continuous_effects
        .last_mut()
        .unwrap()
        .condition = Some(GameCondition::ActivePlayer {
        players: RelativePlayerSet::Opponents,
    });
    engine
        .state
        .continuous_effects
        .last_mut()
        .unwrap()
        .source_id = Some(source);
    assert_eq!(engine.effective_activated_abilities(source).len(), 1);
    engine.state.continuous_effects.clear();
    let aura = permanent(&mut engine, "kenriths_transformation");
    engine.state.objects.get_mut(&aura).unwrap().attached_to =
        Some(AttachmentRecipient::Object(source));
    engine.emit_static_abilities_on_enter(aura);
    engine
        .state
        .continuous_effects
        .retain(|effect| matches!(effect.kind, ContinuousEffectKind::Layer6RemoveAllAbilities));
    assert!(engine.effective_activated_abilities(source).is_empty());
    *engine.state.zone_change_generation.entry(aura).or_insert(0) += 1;
    assert_eq!(
        engine.effective_activated_abilities(source).len(),
        1,
        "an old static incarnation cannot remove the current land's mana ability"
    );
}

#[test]
fn lantern_granted_slots_do_not_rebind_after_an_earlier_occurrence_expires() {
    let mut engine = ready_engine(508_001);
    let recipient = permanent(&mut engine, "sol_ring");
    let mut ability = CardRegistry::global()
        .get("forest")
        .unwrap()
        .primary_face()
        .activated_abilities[0]
        .clone();
    ability.intrinsic_land_mana = false;
    for occurrence in [0, 1] {
        effect(
            &mut engine,
            recipient,
            ContinuousEffectKind::GrantActivatedAbility(Box::new(ability.clone())),
            occurrence,
        );
        assert_eq!(
            engine
                .state
                .continuous_effects
                .last()
                .unwrap()
                .trigger_grant_origin,
            Some(TriggerAbilityOrigin::ResolvingGrant(occurrence))
        );
    }
    let pass = rv1::RuledCommand {
        cmd: Some(rv1::ruled_command::Cmd::PassPriority(rv1::PassPriority {})),
    };
    engine.apply_command(0, &pass).unwrap();
    engine.apply_command(1, &pass).unwrap();
    let first = engine.effective_activated_abilities(recipient);
    let grants: Vec<_> = first
        .iter()
        .filter(|entry| entry.granted)
        .map(|entry| entry.slot)
        .collect();
    assert_eq!(grants.len(), 2);
    let stale = activate(&engine, recipient, grants[0], 0);
    engine.state.continuous_effects.retain(|effect| {
        effect.trigger_grant_origin != Some(TriggerAbilityOrigin::ResolvingGrant(0))
    });
    engine.apply_command(0, &pass).unwrap();
    engine.apply_command(1, &pass).unwrap();
    let remaining = engine.effective_activated_abilities(recipient);
    assert_eq!(
        remaining
            .iter()
            .filter(|entry| entry.granted)
            .map(|entry| entry.slot)
            .collect::<Vec<_>>(),
        vec![grants[1]],
        "removing occurrence A must leave a hole, not rebind A's stale slot to B"
    );
    let before = engine.diagnostic_snapshot().unwrap();
    assert!(engine.apply_command(0, &stale).is_err());
    assert_eq!(engine.diagnostic_snapshot().unwrap(), before);
    let live = activate(&engine, recipient, grants[1], 0);
    engine.apply_command(0, &live).unwrap();
    assert_eq!(engine.state.players[0].mana_pool.green, 1);
    assert!(engine.state.objects[&recipient].tapped);
    assert!(engine.state.stack.is_empty());
}

#[test]
fn lantern_grant_slots_survive_copy_spans_and_returning_native_definitions() {
    let mut engine = ready_engine(508_002);
    let source = permanent(&mut engine, "sol_ring");
    let mut granted = engine
        .registry
        .get("forest")
        .unwrap()
        .primary_face()
        .activated_abilities[0]
        .clone();
    granted.intrinsic_land_mana = false;
    effect(
        &mut engine,
        source,
        ContinuousEffectKind::GrantActivatedAbility(Box::new(granted)),
        1,
    );
    let original = engine.effective_activated_abilities(source);
    assert_eq!(
        original.iter().map(|entry| entry.slot).collect::<Vec<_>>(),
        [0, 2]
    );
    let values = engine.registry.get("fetid_pools").unwrap();
    engine
        .state
        .objects
        .get_mut(&source)
        .unwrap()
        .copiable_values = Some(CopiableValues {
        source_card_id: "fetid_pools".into(),
        source_face_index: 0,
        face: values.primary_face().clone(),
        display_name: "Fetid Pools".into(),
        room_faces: None,
    });
    engine.reconcile_activated_ability_slots();
    let copied = engine.effective_activated_abilities(source);
    assert_eq!(
        copied.iter().map(|entry| entry.slot).collect::<Vec<_>>(),
        [1, 2]
    );
    assert!(copied[0].definition.intrinsic_land_mana);
    assert!(copied[1].granted);
    assert_eq!(
        engine.state.activated_ability_slots[&source].slots.len(),
        4,
        "the copied private-zone ability appends a hole instead of shifting existing grants"
    );
    engine
        .state
        .objects
        .get_mut(&source)
        .unwrap()
        .copiable_values = None;
    engine.reconcile_activated_ability_slots();
    assert_eq!(
        engine
            .effective_activated_abilities(source)
            .iter()
            .map(|entry| entry.slot)
            .collect::<Vec<_>>(),
        [0, 2]
    );
    assert_eq!(engine.state.activated_ability_slots[&source].slots.len(), 4);
}

#[test]
fn lantern_concealed_grant_slots_survive_reveal_and_known_slots_survive_concealment() {
    let mut engine = ready_engine(508_003);
    let hidden = face_down_permanent(&mut engine, "fetid_pools");
    effect(&mut engine, hidden, forest_setting(), 1);
    let mut granted = engine
        .registry
        .get("forest")
        .unwrap()
        .primary_face()
        .activated_abilities[0]
        .clone();
    granted.intrinsic_land_mana = false;
    effect(
        &mut engine,
        hidden,
        ContinuousEffectKind::GrantActivatedAbility(Box::new(granted)),
        2,
    );
    assert_eq!(
        engine
            .effective_activated_abilities(hidden)
            .iter()
            .map(|entry| entry.slot)
            .collect::<Vec<_>>(),
        [0, 1]
    );
    engine.state.objects.get_mut(&hidden).unwrap().face_down = false;
    engine.reconcile_activated_ability_slots();
    assert_eq!(
        engine
            .effective_activated_abilities(hidden)
            .iter()
            .map(|entry| entry.slot)
            .collect::<Vec<_>>(),
        [0, 1]
    );
    assert_eq!(engine.state.activated_ability_slots[&hidden].slots.len(), 3);

    let known = permanent(&mut engine, "sol_ring");
    effect(&mut engine, known, forest_setting(), 3);
    assert_eq!(engine.effective_activated_abilities(known)[0].slot, 1);
    engine.state.objects.get_mut(&known).unwrap().face_down = true;
    engine.reconcile_activated_ability_slots();
    assert_eq!(
        engine.effective_activated_abilities(known)[0].slot,
        1,
        "a previously public incarnation keeps its existing intrinsic slot"
    );
    let before = engine.diagnostic_snapshot().unwrap();
    engine.effective_activated_abilities(hidden);
    engine.effective_activated_abilities(known);
    engine.ev_zone_view_sync();
    assert_eq!(
        engine.diagnostic_snapshot().unwrap(),
        before,
        "immutable catalog and view reads do not allocate slots"
    );
}

#[test]
fn lantern_recipient_slots_retire_even_on_same_zone_generation_changes() {
    let mut engine = ready_engine(508_004);
    let source = permanent(&mut engine, "sol_ring");
    let stale = activate(&engine, source, 0, 0);
    super::super::resolution::move_object_to_zone(
        &mut engine.state,
        engine.registry,
        source,
        Zone::Battlefield,
        None,
    )
    .unwrap();
    assert!(!engine.state.activated_ability_slots.contains_key(&source));
    engine.initial_response_batch();
    assert_eq!(
        engine.state.activated_ability_slots[&source].zone_change_generation,
        1
    );
    let before = engine.diagnostic_snapshot().unwrap();
    assert!(engine.apply_command(0, &stale).is_err());
    assert_eq!(engine.diagnostic_snapshot().unwrap(), before);
    engine
        .state
        .objects
        .get_mut(&source)
        .unwrap()
        .summoning_sick = false;
    engine
        .apply_command(0, &activate(&engine, source, 0, 0))
        .unwrap();
    assert_eq!(engine.state.players[0].mana_pool.colorless, 2);
    assert!(engine.state.objects[&source].tapped);
}

#[test]
fn lantern_identical_resolving_grants_have_independent_activation_limits() {
    use tricerules_cards::primitives::ActivationLimit;
    for limit in [
        ActivationLimit::PerTurn { max_activations: 1 },
        ActivationLimit::PerObject { max_activations: 1 },
    ] {
        let mut engine = ready_engine(508_005);
        let source = permanent(&mut engine, "sol_ring");
        let mut granted = engine
            .registry
            .get("forest")
            .unwrap()
            .primary_face()
            .activated_abilities[0]
            .clone();
        granted.intrinsic_land_mana = false;
        granted.costs = vec![AbilityCost::PayLife { amount: 1 }];
        granted.activation_limit = Some(limit);
        for _ in 0..2 {
            effect(
                &mut engine,
                source,
                ContinuousEffectKind::GrantActivatedAbility(Box::new(granted.clone())),
                1,
            );
        }
        engine
            .apply_command(0, &activate(&engine, source, 2, 0))
            .unwrap();
        engine
            .apply_command(0, &activate(&engine, source, 3, 0))
            .expect(
                "identical definition from a second resolving occurrence has its own allowance",
            );
        assert_eq!(engine.state.players[0].mana_pool.green, 2);
        assert_eq!(engine.state.players[0].life, 18);
        for slot in [2, 3] {
            let before = engine.diagnostic_snapshot().unwrap();
            assert!(engine
                .apply_command(0, &activate(&engine, source, slot, 0))
                .is_err());
            assert_eq!(engine.diagnostic_snapshot().unwrap(), before);
        }
        effect(
            &mut engine,
            source,
            ContinuousEffectKind::Layer6RemoveAllAbilities,
            2,
        );
        assert!(engine.effective_activated_abilities(source).is_empty());
        engine.state.continuous_effects.retain(|effect| {
            !matches!(effect.kind, ContinuousEffectKind::Layer6RemoveAllAbilities)
        });
        engine.reconcile_activated_ability_slots();
        let before = engine.diagnostic_snapshot().unwrap();
        assert!(engine
            .apply_command(0, &activate(&engine, source, 3, 0))
            .is_err());
        assert_eq!(
            engine.diagnostic_snapshot().unwrap(),
            before,
            "suppression and restoration do not reset the occurrence allowance"
        );
        effect(
            &mut engine,
            source,
            ContinuousEffectKind::GrantActivatedAbility(Box::new(granted)),
            3,
        );
        engine
            .apply_command(0, &activate(&engine, source, 4, 0))
            .unwrap();
        assert_eq!(engine.state.players[0].mana_pool.green, 3);
        assert_eq!(engine.state.players[0].life, 17);
    }
}

#[test]
fn lantern_existing_static_grant_presentation_names_the_granting_definition() {
    let mut engine = ready_engine(508_006);
    let land = permanent(&mut engine, "forest");
    let aura = permanent(&mut engine, "gift_of_paradise");
    engine.state.objects.get_mut(&aura).unwrap().attached_to =
        Some(AttachmentRecipient::Object(land));
    engine.emit_static_abilities_on_enter(aura);
    engine.reconcile_activated_ability_slots();
    let granted = engine
        .effective_activated_abilities(land)
        .into_iter()
        .find(|entry| entry.granted)
        .unwrap();
    let info = super::super::legal_actions::activated_ability_info(&engine, land, &granted);
    let presentation = info
        .presentation
        .expect("public static grant has definition provenance");
    assert_eq!(presentation.card_id, "gift_of_paradise");
    assert_eq!(presentation.path.len(), 2);
    assert_eq!(presentation.path[0].id, "static_01");
    assert_eq!(presentation.path[1].id, "activated_01");
    assert_eq!(info.ability_index, 1);
    assert_eq!(
        engine.state.objects[&land].card_id, "forest",
        "the activating physical recipient keeps its own identity"
    );
}

fn gift_fixture_with_child(
    engine: &mut GameEngine,
    land: ObjectId,
    child: ActivatedAbilityDef,
) -> ObjectId {
    let aura = permanent(engine, "gift_of_paradise");
    let mut values = engine.copiable_values_for(aura).unwrap();
    let StaticAbilityDef::AttachedModifier {
        activated_abilities,
        ..
    } = &mut values.face.static_abilities[0].definition
    else {
        panic!("Gift modifier");
    };
    *activated_abilities = vec![child];
    let object = engine.state.objects.get_mut(&aura).unwrap();
    object.attached_to = Some(AttachmentRecipient::Object(land));
    object.copiable_values = Some(values);
    engine.emit_static_abilities_on_enter(aura);
    engine.reconcile_activated_ability_slots();
    aura
}

#[test]
fn lantern_scoped_producer_uses_live_controller_and_source_exclusions() {
    let mut engine = ready_engine(508_009);
    let land = permanent(&mut engine, "forest");
    let opponent_land = permanent(&mut engine, "island");
    engine
        .state
        .objects
        .get_mut(&opponent_land)
        .unwrap()
        .base_controller = 1;
    engine
        .state
        .objects
        .get_mut(&opponent_land)
        .unwrap()
        .controller = 1;
    let source = permanent(&mut engine, "sol_ring");
    let mut values = engine.copiable_values_for(source).unwrap();
    let mut static_ability = engine
        .registry
        .get("gift_of_paradise")
        .unwrap()
        .primary_face()
        .static_abilities[0]
        .clone();
    let mut child = engine
        .registry
        .get("forest")
        .unwrap()
        .primary_face()
        .activated_abilities[0]
        .clone();
    child.intrinsic_land_mana = false;
    static_ability.definition = StaticAbilityDef::GrantActivatedAbilityToPermanents {
        filter: TargetFilter {
            kind: TargetKind::AnyPermanent,
            controller: tricerules_cards::primitives::TargetController::You,
            permanent_types: vec![PermanentTypeFilter::Land],
            ..Default::default()
        },
        activated_abilities: vec![child],
    };
    values.face.static_abilities = vec![static_ability];
    engine
        .state
        .objects
        .get_mut(&source)
        .unwrap()
        .copiable_values = Some(values);
    engine.emit_static_abilities_on_enter(source);
    engine.reconcile_activated_ability_slots();
    assert_eq!(engine.effective_activated_abilities(land).len(), 2);
    assert_eq!(engine.effective_activated_abilities(opponent_land).len(), 1);
    assert_eq!(engine.effective_activated_abilities(source).len(), 1);
    let later_land = permanent(&mut engine, "swamp");
    assert_eq!(engine.effective_activated_abilities(later_land).len(), 2);
    // The source can itself become a land; scope source references are not automatic exclusions.
    effect(
        &mut engine,
        source,
        ContinuousEffectKind::Layer4AddTypes(tricerules_cards::TypeLineAddition {
            card_types: vec![PermanentTypeFilter::Land],
            ..Default::default()
        }),
        1,
    );
    assert!(engine
        .effective_activated_abilities(source)
        .iter()
        .any(|entry| entry.granted));
    engine
        .state
        .objects
        .get_mut(&source)
        .unwrap()
        .base_controller = 1;
    engine.state.objects.get_mut(&source).unwrap().controller = 1;
    engine.reconcile_activated_ability_slots();
    assert_eq!(engine.effective_activated_abilities(land).len(), 1);
    assert_eq!(engine.effective_activated_abilities(opponent_land).len(), 2);
    let granted = engine
        .effective_activated_abilities(opponent_land)
        .into_iter()
        .find(|entry| entry.granted)
        .unwrap();
    assert!(matches!(granted.occurrence,
        crate::state::ActivatedAbilityOccurrence::Granted(TriggerAbilityOrigin::StaticGrant { source_id, .. }) if source_id == source));
}

#[test]
fn lantern_rite_shaped_creature_grant_obeys_sickness_and_explicit_source_exclusion() {
    let mut engine = ready_engine(508_010);
    let source = permanent(&mut engine, "grizzly_bears");
    let recipient = permanent(&mut engine, "grizzly_bears");
    let land = permanent(&mut engine, "forest");
    let mut values = engine.copiable_values_for(source).unwrap();
    let mut parent = engine
        .registry
        .get("gift_of_paradise")
        .unwrap()
        .primary_face()
        .static_abilities[0]
        .clone();
    let mut child = engine
        .registry
        .get("forest")
        .unwrap()
        .primary_face()
        .activated_abilities[0]
        .clone();
    child.intrinsic_land_mana = false;
    parent.definition = StaticAbilityDef::GrantActivatedAbilityToPermanents {
        filter: TargetFilter {
            kind: TargetKind::Creature,
            controller: tricerules_cards::primitives::TargetController::You,
            excluded_objects: vec![tricerules_cards::TargetObjectExclusion::Source],
            ..Default::default()
        },
        activated_abilities: vec![child],
    };
    values.face.static_abilities = vec![parent];
    engine
        .state
        .objects
        .get_mut(&source)
        .unwrap()
        .copiable_values = Some(values);
    engine
        .state
        .objects
        .get_mut(&recipient)
        .unwrap()
        .summoning_sick = true;
    engine.emit_static_abilities_on_enter(source);
    engine.reconcile_activated_ability_slots();
    assert!(engine.effective_activated_abilities(source).is_empty());
    assert_eq!(engine.effective_activated_abilities(land).len(), 1);
    let granted = engine
        .effective_activated_abilities(recipient)
        .pop()
        .unwrap();
    assert_eq!(granted.slot, 1);
    let command = activate(&engine, recipient, granted.slot, 0);
    let before = engine.diagnostic_snapshot().unwrap();
    assert!(engine.apply_command(0, &command).is_err());
    assert_eq!(engine.diagnostic_snapshot().unwrap(), before);
    effect(
        &mut engine,
        recipient,
        ContinuousEffectKind::Layer6AddKeyword(Keyword::Haste),
        1,
    );
    engine.apply_command(0, &command).unwrap();
    assert_eq!(engine.state.players[0].mana_pool.green, 1);
    assert!(engine.state.objects[&recipient].tapped);
    assert!(!engine.state.objects[&source].tapped);
}

#[test]
fn lantern_identical_static_grants_keep_limits_when_a_source_is_suppressed() {
    use tricerules_cards::primitives::ActivationLimit;
    for limit in [
        ActivationLimit::PerTurn { max_activations: 1 },
        ActivationLimit::PerObject { max_activations: 1 },
    ] {
        let mut engine = ready_engine(508_007);
        let land = permanent(&mut engine, "forest");
        let mut child = engine
            .registry
            .get("forest")
            .unwrap()
            .primary_face()
            .activated_abilities[0]
            .clone();
        child.intrinsic_land_mana = false;
        child.costs = vec![AbilityCost::PayLife { amount: 1 }];
        child.activation_limit = Some(limit);
        let first = gift_fixture_with_child(&mut engine, land, child.clone());
        gift_fixture_with_child(&mut engine, land, child.clone());
        engine
            .apply_command(0, &activate(&engine, land, 1, 0))
            .unwrap();
        engine
            .apply_command(0, &activate(&engine, land, 2, 0))
            .unwrap();
        assert_eq!(engine.state.players[0].mana_pool.green, 2);
        assert_eq!(engine.state.players[0].life, 18);
        effect(
            &mut engine,
            first,
            ContinuousEffectKind::Layer6RemoveAllAbilities,
            3,
        );
        assert_eq!(
            engine
                .effective_activated_abilities(land)
                .iter()
                .map(|ability| ability.slot)
                .collect::<Vec<_>>(),
            [0, 2]
        );
        engine.state.continuous_effects.retain(|effect| {
            !matches!(effect.kind, ContinuousEffectKind::Layer6RemoveAllAbilities)
        });
        engine.reconcile_activated_ability_slots();
        for slot in [1, 2] {
            let before = engine.diagnostic_snapshot().unwrap();
            assert!(engine
                .apply_command(0, &activate(&engine, land, slot, 0))
                .is_err());
            assert_eq!(engine.diagnostic_snapshot().unwrap(), before);
        }
        gift_fixture_with_child(&mut engine, land, child);
        engine
            .apply_command(0, &activate(&engine, land, 3, 0))
            .unwrap();
        assert_eq!(engine.state.players[0].mana_pool.green, 3);
        assert_eq!(engine.state.players[0].life, 17);
    }
}

#[test]
fn lantern_nonmana_grants_capture_occurrence_and_presentation_before_source_cost_departure() {
    use tricerules_cards::primitives::ActivationLimit;
    let mut engine = ready_engine(508_008);
    let land = permanent(&mut engine, "forest");
    let mut child = engine
        .registry
        .get("forest")
        .unwrap()
        .primary_face()
        .activated_abilities[0]
        .clone();
    child.intrinsic_land_mana = false;
    child.costs = vec![AbilityCost::SacrificePermanent {
        count: 1,
        filter: TargetFilter {
            kind: TargetKind::AnyPermanent,
            controller: TargetController::You,
            permanent_types: vec![PermanentTypeFilter::Enchantment],
            ..Default::default()
        },
    }];
    child.effect = vec![SpellEffectKind::Draw {
        who: PlayerRecipient::Controller,
        count: Amount::Fixed(1),
    }];
    child.activation_limit = Some(ActivationLimit::PerTurn { max_activations: 1 });
    let first = gift_fixture_with_child(&mut engine, land, child.clone());
    let second = gift_fixture_with_child(&mut engine, land, child);
    let hand_before = engine.state.players[0].hand.len();
    for (slot, aura) in [(1, first), (2, second)] {
        let mut command = activate(&engine, land, slot, 0);
        let Some(rv1::ruled_command::Cmd::ActivateAbility(action)) = command.cmd.as_mut() else {
            unreachable!();
        };
        action.cost_selections = vec![rv1::CostSelection {
            cost_index: 0,
            selection: Some(rv1::cost_selection::Selection::PermanentId(aura)),
        }];
        let batch = engine.apply_command(0, &command).unwrap();
        assert_eq!(engine.state.objects[&aura].zone, Zone::Graveyard);
        let item = engine.state.stack.last().unwrap();
        assert_eq!(item.source_permanent_id, Some(land));
        let primary = engine.state.stack_presentations[&item.id]
            .primary
            .as_ref()
            .unwrap();
        assert_eq!(primary.card_id, "gift_of_paradise");
        assert_eq!(primary.path.len(), 2);
        assert!(batch.events.iter().any(|event| matches!(&event.ev,
            Some(rv1::ruled_event::Ev::Log(log)) if log.ability_presentation.as_ref()
                .is_some_and(|reference| reference.ability.as_ref()
                    .is_some_and(|ability| ability.card_id == "gift_of_paradise")))));
    }
    assert_eq!(engine.state.activation_uses_this_turn.len(), 2);
    for key in engine.state.activation_uses_this_turn.keys() {
        assert_eq!(key.object_id, land);
        assert!(matches!(&key.identity, ActivationUseIdentity::Ability(
            ActivatedAbilityOccurrence::Granted(TriggerAbilityOrigin::StaticGrant { source_id, .. })
        ) if [first, second].contains(source_id)));
    }
    let pass = rv1::RuledCommand {
        cmd: Some(rv1::ruled_command::Cmd::PassPriority(rv1::PassPriority {})),
    };
    for _ in 0..4 {
        engine
            .apply_command(engine.state.priority_player_id(), &pass)
            .unwrap();
    }
    assert!(engine.state.stack.is_empty());
    assert_eq!(engine.state.players[0].hand.len(), hand_before + 2);
}
