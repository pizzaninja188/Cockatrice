use super::*;

// Source-backed Onyx Magecraft sentence, not an admitted complete Onyx card.
const MAGECRAFT: &str = r#"(
    id: "synthetic_onyx_magecraft", name: "Synthetic Onyx Magecraft",
    face_id: "synthetic_onyx_magecraft", types: ["Artifact"],
    triggered_abilities: [(ability_id: "magecraft", presentation: Fallback,
        trigger: WheneverPlayerCastsOrCopiesSpell(player: Controller, card_type: InstantOrSorcery),
        effect: [LoseLife(amount: Fixed(2), who: EachOpponent), GainLife(amount: 2)],
    )],
)"#;

const COPY_TWICE: &str = r#"(
    id: "synthetic_copy_twice", name: "Synthetic Copy Twice", face_id: "synthetic_copy_twice",
    types: ["Instant"], mana_cost: "{U}",
    spell_effect: [CopyTargetSpell(count: 2, spell_filter: (card_type: Some(InstantOrSorcery)))],
)"#;

const CAST_ONLY: &str = r#"(
    id: "synthetic_cast_only", name: "Synthetic Cast Only", face_id: "synthetic_cast_only",
    types: ["Artifact"], triggered_abilities: [(ability_id: "cast_only", presentation: Fallback,
        trigger: WheneverPlayerCastsSpell(filter: (card_type: Some(InstantOrSorcery))),
        effect: [GainLife(amount: 1)],
    )],
)"#;

const GAIN_ABILITY: &str = r#"(
    id: "synthetic_gain_ability", name: "Synthetic Gain Ability", face_id: "synthetic_gain_ability",
    types: ["Artifact"], activated_abilities: [(ability_id: "gain", presentation: Fallback,
        costs: [Mana("{1}")], effect: [GainLife(amount: 1)],
    )],
)"#;

fn fixture() -> GameEngine {
    fixture_with_magecraft(MAGECRAFT)
}

fn fixture_with_magecraft(magecraft: &str) -> GameEngine {
    let registry = CardRegistry::from_chunks_and_tokens(
        &[
            include_str!("../../../../tricerules-cards/data/forest.ron"),
            include_str!("../../../../tricerules-cards/data/opt.ron"),
            include_str!("../../../../tricerules-cards/data/divination.ron"),
            include_str!("../../../../tricerules-cards/data/generated/l/llanowar_elves.ron"),
            include_str!("../../../../tricerules-cards/data/lightning_bolt.ron"),
            include_str!("../../../../tricerules-cards/data/twincast.ron"),
            include_str!("../../../../tricerules-cards/data/flusterstorm.ron"),
            include_str!("../../../../tricerules-cards/data/rings_of_brighthearth.ron"),
            include_str!("../../../../tricerules-cards/data/voltaic_key.ron"),
            include_str!("../../../../tricerules-cards/data/sol_ring.ron"),
            magecraft,
            COPY_TWICE,
            CAST_ONLY,
            GAIN_ABILITY,
        ],
        &[],
    )
    .unwrap();
    let mut engine = GameEngine::new_with_registry(
        602_452,
        &[17, 29, 41, 53],
        20,
        Some(
            (0..4)
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
    place_ready(&mut engine, 17, "Synthetic Onyx Magecraft");
    engine
}

fn cast(
    engine: &mut GameEngine,
    actor: PlayerId,
    name: &str,
    targets: Vec<rv1::TargetRef>,
) -> ObjectId {
    engine
        .apply_dev_command(
            actor,
            &rv1::DevCommand {
                target_player_id: actor,
                dev: Some(rv1::dev_command::Dev::PutCardInZone(
                    rv1::DevPutCardInZone {
                        card_name: name.into(),
                        zone: rv1::DevZone::Hand as i32,
                        ..Default::default()
                    },
                )),
            },
        )
        .unwrap();
    let object = engine.state.next_object_id - 1;
    let index = engine.state.player_idx(actor).unwrap();
    let hand_index = engine.state.players[index]
        .hand
        .iter()
        .position(|&id| id == object)
        .unwrap() as u32;
    engine.state.priority_idx = index;
    engine.state.players[index].mana_pool.blue = 10;
    engine.state.players[index].mana_pool.green = 10;
    engine.state.players[index].mana_pool.red = 10;
    engine.state.players[index].mana_pool.colorless = 10;
    engine
        .apply_scenario_command(
            actor,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::CastSpell(rv1::CastSpell {
                    cast_method: rv1::CastMethod::Normal as i32,
                    source: Some(rv1::CastSource {
                        expected_zone_change_generation: Some(
                            engine
                                .state
                                .zone_change_generation
                                .get(&object)
                                .copied()
                                .unwrap_or(0),
                        ),
                        location: Some(rv1::cast_source::Location::HandIndex(hand_index)),
                    }),
                    targets,
                    ..Default::default()
                })),
            },
        )
        .unwrap();
    object
}

fn pass_round(engine: &mut GameEngine) -> rv1::RuledEventBatch {
    let mut batch = None;
    for _ in 0..4 {
        let actor = engine.state.priority_player_id();
        batch = Some(
            engine
                .apply_command(
                    actor,
                    &RuledCommand {
                        cmd: Some(rv1::ruled_command::Cmd::PassPriority(rv1::PassPriority {})),
                    },
                )
                .unwrap(),
        );
    }
    batch.unwrap()
}

#[test]
fn magecraft_cast_condition_matches_own_instants_and_sorceries_with_fixed_life_gain() {
    for name in ["Opt", "Divination"] {
        let mut engine = fixture();
        let original = cast(&mut engine, 17, name, vec![]);
        assert_eq!(
            engine.state.stack.len(),
            2,
            "one Magecraft trigger above the cast {name}"
        );
        assert_eq!(engine.state.stack[0].id, original);
        pass_round(&mut engine);
        assert_eq!(
            engine
                .state
                .players
                .iter()
                .map(|p| p.life)
                .collect::<Vec<_>>(),
            [22, 18, 18, 18]
        );
        assert_eq!(engine.state.stack.len(), 1);
        assert_eq!(engine.state.turn_history.current.spells_cast, 1);
        assert_eq!(engine.state.turn_history.current.player(17).spells_cast, 1);
    }
    for (actor, name) in [(17, "Llanowar Elves"), (29, "Opt")] {
        let mut engine = fixture();
        cast(&mut engine, actor, name, vec![]);
        assert_eq!(
            engine.state.stack.len(),
            1,
            "{name} by {actor} must not match controller's instant-or-sorcery condition"
        );
        assert!(engine.state.pending_trigger_order.is_none());
    }
}

fn stack_target(engine: &GameEngine, id: ObjectId) -> rv1::TargetRef {
    rv1::TargetRef {
        object_id: id,
        kind: rv1::TargetRefKind::Stack as i32,
        expected_zone_change_generation: engine.state.zone_change_generation.get(&id).copied(),
        ..Default::default()
    }
}

fn cast_twincast_and_resolve_its_cast_trigger(engine: &mut GameEngine, original: ObjectId) {
    let target = stack_target(engine, original);
    cast(engine, 17, "Twincast", vec![target]);
    pass_round(engine);
    assert_eq!(engine.state.players[0].life, 24);
}

#[test]
fn magecraft_direct_spell_copy_is_one_occurrence_without_an_extra_cast() {
    let mut engine = fixture();
    let original = cast(&mut engine, 17, "Opt", vec![]);
    pass_round(&mut engine);
    cast_twincast_and_resolve_its_cast_trigger(&mut engine, original);
    let casts = engine.state.turn_history.current.spell_casts.clone();
    place_ready(&mut engine, 17, "Synthetic Cast Only");
    pass_round(&mut engine);
    assert_eq!(
        engine.state.stack.len(),
        3,
        "the committed targetless copy needs its own Magecraft trigger"
    );
    let copy = engine.state.stack.iter().find(|item| item.is_copy).unwrap();
    assert!(copy.cast_occurrence.is_none());
    assert!(!engine.state.objects.contains_key(&copy.id));
    let trigger = engine.state.stack.last().unwrap();
    assert!(trigger.is_triggered);
    assert!(trigger.trigger_context.triggering_spell.is_none());
    assert!(trigger
        .trigger_context
        .triggering_spell_mana_spent
        .is_none());
    pass_round(&mut engine);
    assert_eq!(
        engine
            .state
            .players
            .iter()
            .map(|p| p.life)
            .collect::<Vec<_>>(),
        [26, 14, 14, 14]
    );
    assert_eq!(engine.state.turn_history.current.spell_casts, casts);
    assert_eq!(engine.state.turn_history.current.spells_cast, 2);
    assert_eq!(engine.state.turn_history.current.player(17).spells_cast, 2);
}

#[test]
fn magecraft_targeted_copy_waits_for_commit_and_rejects_repeated_answers() {
    for target_player in [29, 41] {
        let mut engine = fixture();
        let original = cast(
            &mut engine,
            17,
            "Lightning Bolt",
            vec![rv1::TargetRef {
                object_id: 29,
                kind: rv1::TargetRefKind::Player as i32,
                ..Default::default()
            }],
        );
        pass_round(&mut engine);
        cast_twincast_and_resolve_its_cast_trigger(&mut engine, original);
        pass_round(&mut engine);
        assert!(engine.state.pending_resolution.is_some());
        assert_eq!(
            engine.state.stack.len(),
            1,
            "no copy or Magecraft occurrence before the target choice commits"
        );
        let prompt = engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .resolution_prompt_id
            .get();
        for (actor, prompt_id, players) in [
            (29, prompt, vec![target_player]),
            (17, prompt + 1, vec![target_player]),
            (17, prompt, vec![target_player, target_player]),
        ] {
            let before = engine.diagnostic_snapshot().unwrap();
            assert!(engine
                .apply_command(
                    actor,
                    &RuledCommand {
                        cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(
                            rv1::SubmitResolutionChoice {
                                resolution_prompt_id: prompt_id,
                                chosen_player_ids: players,
                                ..Default::default()
                            }
                        )),
                    }
                )
                .is_err());
            assert_eq!(engine.diagnostic_snapshot().unwrap(), before);
        }
        // The existing copy chooser refreshes an unavailable player target instead of rejecting
        // the command. That accepted reoffer must still create no copy/Magecraft occurrence.
        let refreshed = engine
            .apply_command(
                17,
                &RuledCommand {
                    cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(
                        rv1::SubmitResolutionChoice {
                            resolution_prompt_id: prompt,
                            chosen_player_ids: vec![999],
                            ..Default::default()
                        },
                    )),
                },
            )
            .unwrap();
        assert!(!refreshed.events.iter().any(|event| matches!(&event.ev,
            Some(rv1::ruled_event::Ev::StackPushed(push)) if push.is_copy || push.is_triggered)));
        assert!(engine.state.staged_trigger_groups.is_empty());
        assert_eq!(engine.state.stack.len(), 1);
        let prompt = engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .resolution_prompt_id
            .get();
        // Synthetic observer seam: republishing the existing pending target choice cannot create
        // a spell-copy occurrence. No actual observer interrupt is asserted for this spell pair.
        let saved = engine.state.pending_resolution.take().unwrap();
        let mut restored_events = Vec::new();
        assert!(engine
            .restore_suspended_observer_resolution(Some(saved), None, &mut restored_events)
            .unwrap());
        engine
            .publish_resolution_prompt_ids(&mut restored_events)
            .unwrap();
        let restored_prompt = engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .resolution_prompt_id
            .get();
        assert_ne!(restored_prompt, prompt);
        assert_eq!(engine.state.stack.len(), 1);
        assert!(engine.state.staged_trigger_groups.is_empty());
        assert_eq!(engine.state.players[0].life, 24);
        let answer = RuledCommand {
            cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(
                rv1::SubmitResolutionChoice {
                    resolution_prompt_id: restored_prompt,
                    chosen_player_ids: vec![target_player],
                    ..Default::default()
                },
            )),
        };
        engine.apply_command(17, &answer).unwrap();
        assert_eq!(engine.state.stack.len(), 3);
        assert_eq!(
            engine
                .state
                .stack
                .iter()
                .filter(|item| item.is_copy)
                .count(),
            1
        );
        assert_eq!(
            engine
                .state
                .stack
                .iter()
                .find(|item| item.is_copy)
                .unwrap()
                .targets[0]
                .object_id,
            target_player as u32
        );
        let before = engine.diagnostic_snapshot().unwrap();
        assert!(engine.apply_command(17, &answer).is_err());
        assert_eq!(engine.diagnostic_snapshot().unwrap(), before);
        pass_round(&mut engine);
        assert_eq!(engine.state.players[0].life, 26);
        assert_eq!(engine.state.turn_history.current.spells_cast, 2);
    }
}

#[test]
fn magecraft_archmage_draw_sentence_reuses_each_cast_and_copy_occurrence() {
    let source = MAGECRAFT.replace(
        "LoseLife(amount: Fixed(2), who: EachOpponent), GainLife(amount: 2)",
        "Draw(count: 1)",
    );
    let mut engine = fixture_with_magecraft(&source);
    let hand = engine.state.players[0].hand.len();
    let draws = engine.state.turn_history.current.player(17).cards_drawn;
    let original = cast(&mut engine, 17, "Opt", vec![]);
    pass_round(&mut engine);
    let target = stack_target(&engine, original);
    cast(&mut engine, 17, "Twincast", vec![target]);
    pass_round(&mut engine);
    pass_round(&mut engine);
    pass_round(&mut engine);
    assert_eq!(engine.state.players[0].hand.len(), hand + 3);
    assert_eq!(
        engine.state.turn_history.current.player(17).cards_drawn,
        draws + 3
    );
    assert_eq!(engine.state.turn_history.current.player(17).spells_cast, 2);
    assert_eq!(
        engine.state.stack.len(),
        2,
        "the original and copied Opt have not drawn yet"
    );
}

#[test]
fn magecraft_real_storm_counts_casts_and_captured_copies_survive_original_departure() {
    let mut engine = fixture();
    let original = cast(&mut engine, 17, "Opt", vec![]);
    pass_round(&mut engine);
    cast_twincast_and_resolve_its_cast_trigger(&mut engine, original);
    pass_round(&mut engine);
    pass_round(&mut engine);
    assert_eq!(engine.state.turn_history.current.spells_cast, 2);
    let target = stack_target(&engine, original);
    let storm_spell = cast(&mut engine, 17, "Flusterstorm", vec![target]);
    let pending = engine.state.pending_trigger_order.as_ref().unwrap();
    assert_eq!(pending.candidates.len(), 2);
    let storm = pending
        .candidates
        .iter()
        .find(|trigger| trigger.card_id == "flusterstorm")
        .unwrap();
    assert!(
        matches!(
            storm.ability.effect.as_slice(),
            [SpellEffectKind::CopyCapturedSpell { count: 2 }]
        ),
        "the earlier Opt copy is not a third cast for storm"
    );
    let magecraft = pending
        .candidates
        .iter()
        .find(|trigger| trigger.card_id == "synthetic_onyx_magecraft")
        .unwrap()
        .object_id;
    engine
        .apply_command(
            17,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::SubmitTriggerOrder(
                    rv1::SubmitTriggerOrder {
                        trigger_object_id: magecraft,
                    },
                )),
            },
        )
        .unwrap();
    // Prepared departure seam: remove the original through the shared zone funnel while its
    // real Storm trigger still owns its captured copy receipt. This is not a counterspell pilot.
    engine.state.stack.retain(|item| item.id != storm_spell);
    super::super::resolution::move_object_to_zone(
        &mut engine.state,
        engine.registry,
        storm_spell,
        Zone::Graveyard,
        None,
    )
    .unwrap();
    pass_round(&mut engine);
    for remaining in [1, 0] {
        let prompt = engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .resolution_prompt_id
            .get();
        engine
            .apply_command(
                17,
                &RuledCommand {
                    cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(
                        rv1::SubmitResolutionChoice {
                            resolution_prompt_id: prompt,
                            ..Default::default()
                        },
                    )),
                },
            )
            .unwrap();
        if remaining == 1 {
            assert!(engine.state.pending_resolution.is_some());
            assert!(
                engine.state.pending_trigger_order.is_none(),
                "copy triggers wait while the second copy target choice is parked"
            );
        }
    }
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(
        engine
            .state
            .pending_trigger_order
            .as_ref()
            .unwrap()
            .candidates
            .len(),
        2
    );
    let copies: Vec<_> = engine
        .state
        .stack
        .iter()
        .filter(|item| item.is_copy && item.card_id == "flusterstorm")
        .collect();
    assert_eq!(copies.len(), 2);
    assert!(
        copies
            .iter()
            .all(|copy| copy.cast_occurrence.is_none()
                && !engine.state.objects.contains_key(&copy.id))
    );
    assert_eq!(engine.state.turn_history.current.spells_cast, 3);
    assert_eq!(engine.state.turn_history.current.spell_casts.len(), 3);
    assert_eq!(engine.state.objects[&storm_spell].zone, Zone::Graveyard);
}

#[test]
fn magecraft_nonstack_cards_are_not_copy_events_but_virtual_cast_facts_match() {
    let mut engine = fixture();
    engine
        .apply_dev_command(
            17,
            &rv1::DevCommand {
                target_player_id: 17,
                dev: Some(rv1::dev_command::Dev::PutCardInZone(
                    rv1::DevPutCardInZone {
                        card_name: "Opt".into(),
                        zone: rv1::DevZone::Hand as i32,
                        ..Default::default()
                    },
                )),
            },
        )
        .unwrap();
    let off_stack = engine.state.next_object_id - 1;
    super::super::resolution::move_object_to_zone(
        &mut engine.state,
        engine.registry,
        off_stack,
        Zone::Exile,
        None,
    )
    .unwrap();
    let before = engine.diagnostic_snapshot().unwrap();
    assert!(engine.committed_spell_copy_event(off_stack).is_err());
    assert_eq!(engine.diagnostic_snapshot().unwrap(), before);
    assert!(engine.state.stack.is_empty());

    cast(&mut engine, 17, "Opt", vec![]);
    let mut fact = engine.state.turn_history.current.spell_casts[0].clone();
    fact.origin = Zone::Exile;
    fact.occurrence.zone_change_generation = None;
    // Consumer-only virtual card-copy cast shape. The engine has no admitted off-stack card-copy
    // producer; this does not claim to execute one or authorize a new copying/casting mechanic.
    let before = engine.diagnostic_snapshot().unwrap();
    let matches = engine.collect_event_triggers(&[GameEvent::SpellCast { fact }]);
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].controller, 17);
    assert_eq!(engine.diagnostic_snapshot().unwrap(), before);
}

#[test]
fn magecraft_two_copies_are_distinct_occurrences_in_one_trigger_order_boundary() {
    let mut engine = fixture();
    let original = cast(&mut engine, 17, "Opt", vec![]);
    pass_round(&mut engine);
    let target = stack_target(&engine, original);
    cast(&mut engine, 17, "Synthetic Copy Twice", vec![target]);
    pass_round(&mut engine);
    assert_eq!(engine.state.players[0].life, 24);
    let casts = engine.state.turn_history.current.spell_casts.clone();
    pass_round(&mut engine);
    let copies: Vec<_> = engine
        .state
        .stack
        .iter()
        .filter(|item| item.is_copy)
        .map(|item| item.id)
        .collect();
    assert_eq!(copies.len(), 2);
    assert_ne!(copies[0], copies[1]);
    for id in copies {
        let GameEvent::SpellCopied { fact } = engine.committed_spell_copy_event(id).unwrap() else {
            unreachable!();
        };
        assert_eq!(fact.occurrence.object_id, id);
        assert_eq!(fact.occurrence.zone_change_generation, None);
        assert_eq!(fact.controller, 17);
        assert!(fact
            .matched_card_types
            .contains(&CardTypeFilter::InstantOrSorcery));
    }
    let order = engine.state.pending_trigger_order.as_ref().unwrap();
    assert_eq!(order.candidates.len(), 2);
    let next = order.candidates[0].object_id;
    engine
        .apply_command(
            17,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::SubmitTriggerOrder(
                    rv1::SubmitTriggerOrder {
                        trigger_object_id: next,
                    },
                )),
            },
        )
        .unwrap();
    assert!(engine.state.pending_trigger_order.is_none());
    pass_round(&mut engine);
    pass_round(&mut engine);
    assert_eq!(
        engine
            .state
            .players
            .iter()
            .map(|p| p.life)
            .collect::<Vec<_>>(),
        [28, 12, 12, 12]
    );
    assert_eq!(engine.state.turn_history.current.spell_casts, casts);
}

#[test]
fn magecraft_copy_fact_keeps_event_time_types_and_actor_after_the_copy_leaves() {
    let mut engine = fixture();
    let original = cast(&mut engine, 17, "Opt", vec![]);
    let before = engine.diagnostic_snapshot().unwrap();
    assert!(
        engine.committed_spell_copy_event(original).is_err(),
        "a cast is not a copy"
    );
    assert_eq!(engine.diagnostic_snapshot().unwrap(), before);
    pass_round(&mut engine);
    cast_twincast_and_resolve_its_cast_trigger(&mut engine, original);
    pass_round(&mut engine);
    let copy = engine
        .state
        .stack
        .iter()
        .find(|item| item.is_copy)
        .unwrap()
        .id;
    let event = engine.committed_spell_copy_event(copy).unwrap();
    // Internal event-snapshot seam, not a command that modifies a resolving copy's type.
    engine.state.stack.retain(|item| item.id != copy);
    engine
        .state
        .stack
        .iter_mut()
        .find(|item| item.id == original)
        .unwrap()
        .card_id = "llanowar_elves".into();
    let triggers = engine.collect_event_triggers(&[event]);
    assert_eq!(triggers.len(), 1);
    assert_eq!(triggers[0].controller, 17);
    assert_eq!(triggers[0].trigger_context.affected_player, Some(17));
    assert!(triggers[0].trigger_context.triggering_spell.is_none());
    assert!(triggers[0]
        .trigger_context
        .triggering_spell_mana_spent
        .is_none());
}

#[test]
fn magecraft_opponent_copy_uses_copy_actor_and_source_controller_separately() {
    let source = MAGECRAFT.replace("player: Controller", "player: Opponent");
    let mut engine = fixture_with_magecraft(&source);
    let original = cast(&mut engine, 29, "Opt", vec![]);
    pass_round(&mut engine);
    assert_eq!(engine.state.players[0].life, 22);
    let target = stack_target(&engine, original);
    cast(&mut engine, 29, "Twincast", vec![target]);
    pass_round(&mut engine);
    pass_round(&mut engine);
    let trigger = engine.state.stack.last().unwrap();
    assert_eq!(trigger.controller, 17);
    assert_eq!(trigger.trigger_context.affected_player, Some(29));
    assert!(trigger.trigger_context.triggering_spell.is_none());
    assert_eq!(
        engine
            .state
            .stack
            .iter()
            .find(|item| item.is_copy)
            .unwrap()
            .controller,
        29
    );
    pass_round(&mut engine);
    assert_eq!(
        engine
            .state
            .players
            .iter()
            .map(|p| p.life)
            .collect::<Vec<_>>(),
        [26, 14, 14, 14]
    );
    assert_eq!(engine.state.turn_history.current.player(17).spells_cast, 0);
    assert_eq!(engine.state.turn_history.current.player(29).spells_cast, 2);
}

#[test]
fn magecraft_actual_rings_ability_copies_are_not_spell_copy_occurrences() {
    for targeted in [false, true] {
        let mut engine = fixture();
        place_ready(&mut engine, 17, "Rings of Brighthearth");
        let source = place_ready(
            &mut engine,
            17,
            if targeted {
                "Voltaic Key"
            } else {
                "Synthetic Gain Ability"
            },
        );
        let artifact = place_ready(&mut engine, 17, "Sol Ring");
        engine.state.objects.get_mut(&artifact).unwrap().tapped = true;
        engine.state.players[0].mana_pool.colorless = 3;
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
                            targets: if targeted {
                                vec![rv1::TargetRef {
                                    object_id: artifact,
                                    kind: rv1::TargetRefKind::Permanent as i32,
                                    expected_zone_change_generation: engine
                                        .state
                                        .zone_change_generation
                                        .get(&artifact)
                                        .copied(),
                                    ..Default::default()
                                }]
                            } else {
                                vec![]
                            },
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
        assert_eq!(
            engine.state.stack.len(),
            2,
            "only the original ability and actual Rings trigger"
        );
        pass_round(&mut engine);
        engine
            .apply_scenario_command(
                17,
                &RuledCommand {
                    cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(
                        rv1::SubmitResolutionChoice {
                            decision: rv1::ResolutionChoiceDecision::SelectBranch as i32,
                            ..Default::default()
                        },
                    )),
                },
            )
            .unwrap();
        let pending = engine.state.pending_resolution.as_ref().unwrap();
        let payment = rv1::PaymentSelection {
            expected_state_revision: engine.state.command_index,
            source: Some(engine.payment_object_ref(pending.presentation.source_object_id)),
            mana: Some(rv1::PaymentMana {
                c: 2,
                ..Default::default()
            }),
            ..Default::default()
        };
        engine
            .apply_scenario_command(
                17,
                &RuledCommand {
                    cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(
                        rv1::SubmitResolutionChoice {
                            decision: rv1::ResolutionChoiceDecision::PayMana as i32,
                            payment: Some(payment),
                            ..Default::default()
                        },
                    )),
                },
            )
            .unwrap();
        if targeted {
            engine
                .apply_scenario_command(
                    17,
                    &RuledCommand {
                        cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(
                            rv1::SubmitResolutionChoice::default(),
                        )),
                    },
                )
                .unwrap();
        }
        assert_eq!(engine.state.stack.len(), 2);
        let copy = engine.state.stack.iter().find(|item| item.is_copy).unwrap();
        assert!(copy.activated_ability.is_some());
        assert!(engine.committed_spell_copy_event(copy.id).is_err());
        assert_eq!(
            engine
                .state
                .players
                .iter()
                .map(|p| p.life)
                .collect::<Vec<_>>(),
            [20, 20, 20, 20]
        );
        assert_eq!(engine.state.turn_history.current.spells_cast, 0);
        assert!(engine.state.staged_trigger_groups.is_empty());
    }
}
