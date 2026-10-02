use super::*;

fn ready_engine(seed: u64) -> GameEngine {
    let mut engine = GameEngine::new_with_default_decks(seed, &[0, 1], 20).unwrap();
    engine.state.opening = None;
    engine.state.turn_step = TurnStep::Main1;
    engine.state.priority_idx = 0;
    engine
}

fn permanent(engine: &mut GameEngine, card: &str) -> ObjectId {
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
    object.face_down = false;
    object.token_origin = None;
    object.token_faces = None;
    object.copiable_values = None;
    object.tapped = false;
    object.summoning_sick = false;
    object.power = None;
    object.toughness = None;
    engine.state.objects.insert(oid, object);
    engine.state.players[0].battlefield.push(oid);
    oid
}

fn effect(engine: &mut GameEngine, source: ObjectId, kind: ContinuousEffectKind, timestamp: u64) {
    engine.state.continuous_effects.push(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: None,
        affected: AffectedScope::Single(source),
        kind,
        condition: None,
        duration: EffectDuration::UntilEndOfTurn,
        timestamp,
    });
}

fn forest_setting() -> ContinuousEffectKind {
    ContinuousEffectKind::Layer4SetTypeLine(tricerules_cards::TypeLineReplacement {
        card_types: vec![PermanentTypeFilter::Land],
        creature_types: Vec::new(),
        land_types: vec![BasicLandType::Forest],
    })
}

fn activate(engine: &GameEngine, source: ObjectId, index: usize, option: u32) -> rv1::RuledCommand {
    rv1::RuledCommand {
        cmd: Some(rv1::ruled_command::Cmd::ActivateAbility(
            rv1::ActivateAbility {
                source_object_id: source,
                ability_index: index as u32,
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
        assert!(catalog[0].1.intrinsic_land_mana);
        assert_eq!(
            engine.active_mana_options(source, &catalog[0].1).unwrap(),
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
            assert_eq!(updated[0].0, catalog[0].0);
            assert_eq!(
                engine.active_mana_options(source, &updated[0].1).unwrap(),
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
            .map(|entry| entry.0)
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
    assert!(catalog[0].1.intrinsic_land_mana && !catalog[1].1.intrinsic_land_mana);
    assert_eq!(
        engine.active_mana_options(source, &catalog[0].1),
        engine.active_mana_options(source, &catalog[1].1)
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
        let source = permanent(&mut engine, card);
        engine.state.objects.get_mut(&source).unwrap().face_down = true;
        effect(&mut engine, source, forest_setting(), 2);
        let catalog = engine.effective_activated_abilities(source);
        assert_eq!(
            catalog.len(),
            1,
            "a face-down object made Forest receives intrinsic mana"
        );
        assert_eq!(
            catalog[0].0, 0,
            "concealed authored span never determines the public index"
        );
        let offer = super::super::legal_actions::activated_ability_info(
            &engine,
            source,
            0,
            0,
            &catalog[0].3,
            &catalog[0].1,
        );
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
    let source = permanent(&mut engine, "sol_ring");
    engine.state.objects.get_mut(&source).unwrap().face_down = true;
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
    let source = permanent(&mut engine, "sol_ring");
    engine.state.objects.get_mut(&source).unwrap().face_down = true;
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
        catalog.iter().map(|entry| entry.0).collect::<Vec<_>>(),
        [0, 3]
    );
    assert_eq!(
        engine.active_mana_options(source, &catalog[0].1).unwrap(),
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
        &engine.effective_activated_abilities(arbor)[0].1
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
            assert_eq!(catalog[0].0, 2);
            assert!(!catalog[0].1.intrinsic_land_mana);
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
