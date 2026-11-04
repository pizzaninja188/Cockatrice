//! Prepared actual-card cases; normal-session reachability is tested separately.
use super::*;

fn fixture() -> GameEngine {
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        7_313_801,
        &[17, 29, 41],
        20,
        Some(vec![vec!["lightning_bolt".into(); 40]; 3]),
        true,
    )
    .unwrap();
    engine.state.turn_step = TurnStep::Main1;
    engine.state.priority_idx = 0;
    engine
}

fn source(engine: &mut GameEngine, player: PlayerId, counters: u32) -> ObjectId {
    engine
        .apply_dev_command(
            player,
            &rv1::DevCommand {
                target_player_id: player,
                dev: Some(rv1::dev_command::Dev::PutCardInZone(
                    rv1::DevPutCardInZone {
                        card_name: "As Foretold".into(),
                        zone: rv1::DevZone::Battlefield as i32,
                        ready: true,
                    },
                )),
            },
        )
        .unwrap();
    let id = engine.state.next_object_id - 1;
    engine
        .state
        .objects
        .get_mut(&id)
        .unwrap()
        .set_counter(CounterKind::Time, counters);
    id
}

fn bolt(engine: &GameEngine, player: PlayerId, cost_source: ObjectId) -> RuledCommand {
    let idx = engine.state.player_idx(player).unwrap();
    let hand_index = engine.state.players[idx]
        .hand
        .iter()
        .position(|id| engine.state.objects[id].card_id == "lightning_bolt")
        .unwrap();
    RuledCommand {
        cmd: Some(rv1::ruled_command::Cmd::CastSpell(rv1::CastSpell {
            source: Some(rv1::CastSource {
                location: Some(rv1::cast_source::Location::HandIndex(hand_index as u32)),
                expected_zone_change_generation: None,
            }),
            cast_method: rv1::CastMethod::Normal as i32,
            as_foretold_source: Some(engine.payment_object_ref(cost_source)),
            targets: vec![rv1::TargetRef {
                object_id: 41,
                kind: rv1::TargetRefKind::Player as i32,
                ..Default::default()
            }],
            ..Default::default()
        })),
    }
}

#[test]
fn as_foretold_actual_live_source_offers_and_casts_bolt_for_zero() {
    let mut engine = fixture();
    let source = source(&mut engine, 17, 1);
    let offers = engine.initial_response_batch();
    let offer = offers.legal_by_player[&17]
        .hand_actions
        .iter()
        .find(|a| a.card_name == "Lightning Bolt")
        .unwrap();
    assert_eq!(
        offer.as_foretold_costs.len(),
        1,
        "one exact live As Foretold zero-cost offer"
    );
    assert_eq!(
        offer.as_foretold_costs[0].source,
        Some(engine.payment_object_ref(source))
    );
    assert_eq!(offer.as_foretold_costs[0].cost, "{0}");
    engine
        .apply_command(17, &bolt(&engine, 17, source))
        .unwrap();
    assert_eq!(engine.state.stack.len(), 1);
    assert_eq!(
        engine.state.players[0].mana_pool,
        crate::state::ManaPool::default()
    );
    assert!(engine.initial_response_batch().legal_by_player[&17]
        .hand_actions
        .iter()
        .all(|a| a.as_foretold_costs.is_empty()));
}

fn reject(engine: &mut GameEngine, actor: PlayerId, command: &RuledCommand) {
    let before = engine.diagnostic_snapshot().unwrap();
    assert!(engine.apply_command(actor, command).is_err());
    assert_eq!(engine.diagnostic_snapshot().unwrap(), before);
}

fn cast_mut(command: &mut RuledCommand) -> &mut rv1::CastSpell {
    let Some(rv1::ruled_command::Cmd::CastSpell(cast)) = command.cmd.as_mut() else {
        panic!("cast");
    };
    cast
}

fn change_controller(engine: &mut GameEngine, id: ObjectId, player: PlayerId) {
    // Prepared control seam; physical object and its incarnation stay unchanged.
    for state in &mut engine.state.players {
        state.battlefield.retain(|oid| *oid != id);
    }
    let idx = engine.state.player_idx(player).unwrap();
    engine.state.players[idx].battlefield.push(id);
    let object = engine.state.objects.get_mut(&id).unwrap();
    object.controller = player;
    object.base_controller = player;
}

fn resolve(engine: &mut GameEngine) {
    for _ in 0..engine.state.players.len() {
        engine
            .apply_command(
                engine.state.priority_player_id(),
                &RuledCommand {
                    cmd: Some(rv1::ruled_command::Cmd::PassPriority(rv1::PassPriority {})),
                },
            )
            .unwrap();
    }
}

#[test]
fn as_foretold_shared_history_survives_both_distinguishing_control_sequences() {
    // A uses, then B gains control: B cannot use the same incarnation this turn.
    let mut engine = fixture();
    let id = source(&mut engine, 17, 1);
    engine.apply_command(17, &bolt(&engine, 17, id)).unwrap();
    resolve(&mut engine);
    change_controller(&mut engine, id, 29);
    engine.state.priority_idx = 1;
    let command = bolt(&engine, 29, id);
    reject(&mut engine, 29, &command);
    // An unused A-to-B transfer allows B. B's use then also consumes A's opportunity on return.
    let mut engine = fixture();
    let id = source(&mut engine, 17, 1);
    let before = bolt(&engine, 17, id);
    change_controller(&mut engine, id, 29);
    reject(&mut engine, 17, &before);
    engine.state.priority_idx = 1;
    engine.apply_command(29, &bolt(&engine, 29, id)).unwrap();
    resolve(&mut engine);
    change_controller(&mut engine, id, 17);
    engine.state.priority_idx = 0;
    let command = bolt(&engine, 17, id);
    reject(&mut engine, 17, &command);
}

#[test]
fn as_foretold_live_threshold_exact_source_and_two_copy_history() {
    let mut engine = fixture();
    let small = source(&mut engine, 17, 0);
    let large = source(&mut engine, 17, 2);
    let command = bolt(&engine, 17, small);
    reject(&mut engine, 17, &command);
    engine
        .state
        .objects
        .get_mut(&small)
        .unwrap()
        .set_counter(CounterKind::Time, 1);
    let offers = engine.initial_response_batch();
    let costs = &offers.legal_by_player[&17].hand_actions[0].as_foretold_costs;
    assert_eq!(
        costs
            .iter()
            .map(|c| c.source.unwrap().object_id)
            .collect::<Vec<_>>(),
        [small, large]
    );
    let mut stale = bolt(&engine, 17, large);
    cast_mut(&mut stale)
        .as_foretold_source
        .as_mut()
        .unwrap()
        .zone_change_generation += 1;
    reject(&mut engine, 17, &stale);
    engine
        .state
        .objects
        .get_mut(&large)
        .unwrap()
        .set_counter(CounterKind::Time, 0);
    let command = bolt(&engine, 17, large);
    reject(&mut engine, 17, &command);
    engine.apply_command(17, &bolt(&engine, 17, small)).unwrap();
    resolve(&mut engine);
    engine
        .state
        .objects
        .get_mut(&large)
        .unwrap()
        .set_counter(CounterKind::Time, 1);
    engine.apply_command(17, &bolt(&engine, 17, large)).unwrap();
    assert_eq!(engine.state.as_foretold_uses.len(), 2);
}

fn fixture_spell(spell: &str) -> GameEngine {
    let registry = CardRegistry::from_chunks_and_tokens(
        &[
            include_str!("../../../tricerules-cards/data/as_foretold.ron"),
            spell,
        ],
        &[],
    )
    .unwrap();
    let mut engine = GameEngine::new_with_registry(
        7_313_802,
        &[17, 29, 41],
        20,
        Some(vec![
            EngineDeck {
                mainboard: vec!["test_spell".into(); 40],
                commanders: vec![]
            };
            3
        ]),
        true,
        Box::leak(Box::new(registry)),
    )
    .unwrap();
    engine.state.turn_step = TurnStep::Main1;
    engine
}

fn plain() -> RuledCommand {
    RuledCommand {
        cmd: Some(rv1::ruled_command::Cmd::CastSpell(rv1::CastSpell {
            cast_method: rv1::CastMethod::Normal as i32,
            source: Some(rv1::CastSource {
                location: Some(rv1::cast_source::Location::HandIndex(0)),
                expected_zone_change_generation: None,
            }),
            ..Default::default()
        })),
    }
}

#[test]
fn as_foretold_true_absent_cost_requires_an_alternative_cost() {
    let mut engine = fixture_spell(
        r#"(id:"test_spell",name:"Test Spell",face_id:"test_spell",mana_cost:"",types:["Instant"],additional_costs:[DiscardCard],spell_effect:[GainLife(amount:1)])"#,
    );
    assert!(
        engine.initial_response_batch().legal_by_player[&17]
            .hand_actions
            .is_empty(),
        "absent cost cannot be paid as zero"
    );
    reject(&mut engine, 17, &plain());
    let id = source(&mut engine, 17, 0);
    let offers = engine.initial_response_batch();
    assert!(
        offers.legal_by_player[&17].hand_actions[0]
            .as_foretold_costs
            .len()
            == 1
    );
    reject(&mut engine, 17, &plain());
    let mut cast = plain();
    cast_mut(&mut cast).as_foretold_source = Some(engine.payment_object_ref(id));
    reject(&mut engine, 17, &cast);
    cast_mut(&mut cast).cost_selections = vec![rv1::CostSelection {
        cost_index: 0,
        selection: Some(rv1::cost_selection::Selection::HandIndex(1)),
    }];
    engine.apply_command(17, &cast).unwrap();
    assert_eq!(engine.state.stack[0].chosen_x, 0);
}

fn dev_card(engine: &mut GameEngine, player: PlayerId, name: &str, zone: rv1::DevZone) -> ObjectId {
    engine
        .apply_dev_command(
            player,
            &rv1::DevCommand {
                target_player_id: player,
                dev: Some(rv1::dev_command::Dev::PutCardInZone(
                    rv1::DevPutCardInZone {
                        card_name: name.into(),
                        zone: zone as i32,
                        ready: true,
                    },
                )),
            },
        )
        .unwrap();
    engine.state.next_object_id - 1
}

fn hand_cast(engine: &GameEngine, oid: ObjectId, source: ObjectId) -> RuledCommand {
    let mut command = plain();
    let index = engine.state.players[0]
        .hand
        .iter()
        .position(|id| *id == oid)
        .unwrap();
    cast_mut(&mut command).source.as_mut().unwrap().location =
        Some(rv1::cast_source::Location::HandIndex(index as u32));
    cast_mut(&mut command).as_foretold_source = Some(engine.payment_object_ref(source));
    command
}

#[test]
fn as_foretold_pending_receipt_cancel_retry_and_source_loss_are_locked() {
    for change in 0..3 {
        let mut engine = fixture();
        let id = source(&mut engine, 17, 1);
        let selected = engine.payment_object_ref(id);
        let mut command = bolt(&engine, 17, id);
        let announcement = casting::announcement_from_cast(cast_mut(&mut command));
        assert_eq!(announcement.as_foretold_source, Some(selected));
        let begin = RuledCommand {
            cmd: Some(rv1::ruled_command::Cmd::BeginSpellCast(
                rv1::BeginSpellCast {
                    announcement: Some(announcement),
                },
            )),
        };
        engine.apply_command(17, &begin).unwrap();
        let transaction_id = engine
            .state
            .pending_spell_cast
            .as_ref()
            .unwrap()
            .transaction_id;
        assert_eq!(
            engine
                .state
                .pending_spell_cast
                .as_ref()
                .unwrap()
                .announcement
                .as_foretold_source,
            Some(selected)
        );
        assert!(engine.state.as_foretold_uses.is_empty());
        engine
            .apply_command(
                17,
                &RuledCommand {
                    cmd: Some(rv1::ruled_command::Cmd::CancelSpellCast(
                        rv1::CancelSpellCast { transaction_id },
                    )),
                },
            )
            .unwrap();
        assert!(engine.state.as_foretold_uses.is_empty());
        engine.apply_command(17, &begin).unwrap();
        let transaction_id = engine
            .state
            .pending_spell_cast
            .as_ref()
            .unwrap()
            .transaction_id;
        match change {
            0 => engine
                .state
                .objects
                .get_mut(&id)
                .unwrap()
                .set_counter(CounterKind::Time, 0),
            1 => change_controller(&mut engine, id, 29),
            _ => {
                resolution::move_object_to_zone(
                    &mut engine.state,
                    engine.registry,
                    id,
                    Zone::Graveyard,
                    None,
                )
                .unwrap();
            }
        }
        let commit = rv1::CommitSpellCast {
            transaction_id,
            ..Default::default()
        };
        let preview = engine.preview_payment(
            17,
            &rv1::PreviewPayment {
                transaction_id,
                commit_spell_cast: Some(commit.clone()),
                ..Default::default()
            },
        );
        assert!(preview.error.is_empty(), "{}", preview.error);
        assert!(engine.state.as_foretold_uses.is_empty());
        engine
            .apply_command(
                17,
                &RuledCommand {
                    cmd: Some(rv1::ruled_command::Cmd::CommitSpellCast(commit)),
                },
            )
            .unwrap();
        assert!(engine.state.as_foretold_uses.contains(&(
            engine.state.turn_instance,
            id,
            selected.zone_change_generation
        )));
        assert_eq!(engine.state.stack[0].card_id, "lightning_bolt");
    }
}

#[test]
fn as_foretold_actual_bargain_can_sacrifice_its_own_cost_source() {
    let mut engine = fixture();
    let id = source(&mut engine, 17, 1);
    let exact = engine.payment_object_ref(id);
    let creature = dev_card(&mut engine, 17, "Grizzly Bears", rv1::DevZone::Battlefield);
    let spell = dev_card(&mut engine, 17, "Archon's Glory", rv1::DevZone::Hand);
    let mut command = hand_cast(&engine, spell, id);
    cast_mut(&mut command).targets = vec![rv1::TargetRef {
        object_id: creature,
        kind: rv1::TargetRefKind::Permanent as i32,
        expected_zone_change_generation: Some(
            engine.payment_object_ref(creature).zone_change_generation,
        ),
        ..Default::default()
    }];
    cast_mut(&mut command).cast_cost_group_selections = vec![rv1::CastCostGroupSelection {
        battlefield_objects: Some(rv1::CostObjectRefs {
            objects: vec![exact],
        }),
        ..Default::default()
    }];
    engine.apply_command(17, &command).unwrap();
    assert_eq!(engine.state.objects[&id].zone, Zone::Graveyard);
    assert!(engine.state.as_foretold_uses.contains(&(
        engine.state.turn_instance,
        id,
        exact.zone_change_generation
    )));
    assert_eq!(engine.state.stack[0].cast_cost_receipts.len(), 1);
    resolve(&mut engine);
    assert!(engine
        .characteristics(creature)
        .unwrap()
        .keywords
        .contains(&Keyword::Lifelink));
}

#[test]
fn as_foretold_prototype_uses_selected_characteristics_and_retains_them() {
    for counters in [5, 9] {
        let mut engine = fixture();
        let id = source(&mut engine, 17, counters);
        let spell = dev_card(&mut engine, 17, "Skitterbeam Battalion", rv1::DevZone::Hand);
        let mut command = hand_cast(&engine, spell, id);
        let hand = match cast_mut(&mut command)
            .source
            .as_ref()
            .unwrap()
            .location
            .unwrap()
        {
            rv1::cast_source::Location::HandIndex(i) => i,
            _ => unreachable!(),
        };
        let offers = engine.initial_response_batch();
        let actions = &offers.legal_by_player[&17].hand_actions;
        let normal = actions
            .iter()
            .find(|a| a.hand_index == hand && !a.prototyped)
            .unwrap();
        let prototype = actions
            .iter()
            .find(|a| a.hand_index == hand && a.prototyped)
            .unwrap();
        assert_eq!(normal.as_foretold_costs.len(), usize::from(counters == 9));
        assert_eq!(prototype.as_foretold_costs.len(), 1);
        if counters == 5 {
            reject(&mut engine, 17, &command);
        }
        cast_mut(&mut command).prototyped = true;
        engine.apply_command(17, &command).unwrap();
        let characteristics = engine.characteristics(spell).unwrap();
        assert_eq!(
            (
                characteristics.mana_value,
                characteristics.power,
                characteristics.toughness
            ),
            (5, Some(2), Some(2))
        );
        assert!(characteristics.colors.contains(&Color::Red));
        resolve(&mut engine);
        let characteristics = engine.characteristics(spell).unwrap();
        assert_eq!(
            (
                characteristics.mana_value,
                characteristics.power,
                characteristics.toughness
            ),
            (5, Some(2), Some(2))
        );
    }
}

#[test]
fn as_foretold_nonzero_x_and_competing_methods_reject_without_consuming() {
    let mut engine = fixture_spell(
        r#"(id:"test_spell",name:"Test Spell",face_id:"test_spell",mana_cost:"{X}{U}",types:["Instant"],flashback_cost:Some("{1}{U}"),spell_effect:[GainLife(amount:1)])"#,
    );
    let id = source(&mut engine, 17, 1);
    let mut command = plain();
    cast_mut(&mut command).as_foretold_source = Some(engine.payment_object_ref(id));
    cast_mut(&mut command).x_value = 1;
    reject(&mut engine, 17, &command);
    cast_mut(&mut command).x_value = 0;
    for method in [
        rv1::CastMethod::Warp,
        rv1::CastMethod::WithoutManaCost,
        rv1::CastMethod::Permission,
        rv1::CastMethod::Sneak,
    ] {
        cast_mut(&mut command).cast_method = method as i32;
        reject(&mut engine, 17, &command);
    }
    cast_mut(&mut command).cast_method = rv1::CastMethod::Normal as i32;
    engine.apply_command(17, &command).unwrap();
    assert_eq!(
        engine
            .characteristics(engine.state.stack[0].id)
            .unwrap()
            .mana_value,
        1
    );
}

#[test]
fn as_foretold_real_extra_turn_and_new_incarnation_restore_separate_uses() {
    let mut engine = fixture();
    let id = source(&mut engine, 17, 1);
    let stale = bolt(&engine, 17, id);
    engine.apply_command(17, &stale).unwrap();
    resolve(&mut engine);
    resolution::move_object_to_zone(
        &mut engine.state,
        engine.registry,
        id,
        Zone::Graveyard,
        None,
    )
    .unwrap();
    resolution::move_object_to_zone(
        &mut engine.state,
        engine.registry,
        id,
        Zone::Battlefield,
        None,
    )
    .unwrap();
    engine.emit_static_abilities_on_enter(id);
    engine
        .state
        .objects
        .get_mut(&id)
        .unwrap()
        .set_counter(CounterKind::Time, 1);
    reject(&mut engine, 17, &stale);
    engine.apply_command(17, &bolt(&engine, 17, id)).unwrap();
    resolve(&mut engine);
    let turn = engine.state.turn;
    let instance = engine.state.turn_instance;
    engine.schedule_extra_turns(17, 1);
    engine.finish_cleanup_roll_new_turn(Vec::new()).unwrap();
    assert_eq!(engine.state.active_player_id(), 17);
    assert_eq!(
        engine.state.turn, turn,
        "same player/cycle, different actual turn"
    );
    assert_eq!(engine.state.turn_instance, instance + 1);
    assert!(engine.state.as_foretold_uses.is_empty());
    engine.apply_command(17, &bolt(&engine, 17, id)).unwrap();
}

#[test]
fn as_foretold_source_must_keep_its_effective_static_ability_and_timing() {
    let mut engine = fixture();
    let id = source(&mut engine, 17, 1);
    let command = bolt(&engine, 17, id);
    engine.state.objects.get_mut(&id).unwrap().face_down = true;
    reject(&mut engine, 17, &command);
    engine.state.objects.get_mut(&id).unwrap().face_down = false;
    engine.state.objects.get_mut(&id).unwrap().phase_status = Some(crate::state::PhasedOutStatus {
        phase_out_under: 17,
        phase_in_due_after_departed_turn: false,
        phase_in_with: None,
        direct_attachment_snapshot: None,
    });
    reject(&mut engine, 17, &command);
    engine.state.objects.get_mut(&id).unwrap().phase_status = None;
    engine.state.continuous_effects.push(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: None,
        affected: AffectedScope::Single(id),
        kind: ContinuousEffectKind::Layer6RemoveAllAbilities,
        condition: None,
        duration: EffectDuration::UntilEndOfTurn,
        timestamp: engine.state.command_index,
    });
    reject(&mut engine, 17, &command);
    engine.state.continuous_effects.clear();
    engine.apply_command(17, &command).unwrap();
    let mut engine = fixture();
    let id = source(&mut engine, 17, 3);
    let spell = dev_card(&mut engine, 17, "Tormenting Voice", rv1::DevZone::Hand);
    let mut command = hand_cast(&engine, spell, id);
    cast_mut(&mut command).cost_selections = vec![rv1::CostSelection {
        cost_index: 0,
        selection: Some(rv1::cost_selection::Selection::HandIndex(0)),
    }];
    engine.state.active_player_idx = 1;
    reject(&mut engine, 17, &command);
}

#[test]
fn as_foretold_mandatory_discard_optional_cost_and_failed_payment_preserve_use() {
    let mut engine = fixture();
    let id = source(&mut engine, 17, 2);
    let spell = dev_card(&mut engine, 17, "Tormenting Voice", rv1::DevZone::Hand);
    let mut command = hand_cast(&engine, spell, id);
    reject(&mut engine, 17, &command);
    let discarded = engine.state.players[0].hand[0];
    cast_mut(&mut command).cost_selections = vec![rv1::CostSelection {
        cost_index: 0,
        selection: Some(rv1::cost_selection::Selection::HandIndex(0)),
    }];
    engine.apply_command(17, &command).unwrap();
    assert_eq!(engine.state.objects[&discarded].zone, Zone::Graveyard);
    assert_eq!(
        engine.state.players[0].mana_pool,
        crate::state::ManaPool::default()
    );
    let mut engine = fixture_spell(
        r#"(id:"test_spell",name:"Test Spell",face_id:"test_spell",mana_cost:"{U}",types:["Instant"],cast_cost_groups:[(group_id:"kicker",presentation:Fallback,options:[Mana(option_id:"kick",presentation:Fallback,kind:Kicker,cost:"{2}")])],spell_effect:[GainLife(amount:1)])"#,
    );
    let id = source(&mut engine, 17, 1);
    let mut command = plain();
    cast_mut(&mut command).as_foretold_source = Some(engine.payment_object_ref(id));
    cast_mut(&mut command).cast_cost_group_selections =
        vec![rv1::CastCostGroupSelection::default()];
    reject(&mut engine, 17, &command);
    assert!(engine.state.as_foretold_uses.is_empty());
    engine.state.players[0].mana_pool.colorless = 2;
    engine.apply_command(17, &command).unwrap();
    assert_eq!(engine.state.players[0].mana_pool.colorless, 0);
}

#[test]
fn as_foretold_printed_zero_is_payable_without_a_source() {
    let mut engine = fixture_spell(
        r#"(id:"test_spell",name:"Test Spell",face_id:"test_spell",mana_cost:"{0}",types:["Instant"],spell_effect:[GainLife(amount:1)])"#,
    );
    let offers = engine.initial_response_batch();
    assert!(!offers.legal_by_player[&17].hand_actions[0].base_cost_unpayable);
    engine.apply_command(17, &plain()).unwrap();
    assert!(engine.state.as_foretold_uses.is_empty());
}

#[test]
fn as_foretold_preserves_target_tax_reduction_and_printed_mana_value_threshold() {
    let mut engine = fixture();
    let id = source(&mut engine, 17, 1);
    let target = dev_card(
        &mut engine,
        29,
        "Boreal Elemental",
        rv1::DevZone::Battlefield,
    );
    let mut command = bolt(&engine, 17, id);
    cast_mut(&mut command).targets = vec![rv1::TargetRef {
        object_id: target,
        kind: rv1::TargetRefKind::Permanent as i32,
        expected_zone_change_generation: Some(
            engine.payment_object_ref(target).zone_change_generation,
        ),
        ..Default::default()
    }];
    engine.state.players[0].mana_pool.colorless = 1;
    reject(&mut engine, 17, &command); // The source waives the base; the targeting tax still costs two.
    dev_card(&mut engine, 17, "Mocking Sprite", rv1::DevZone::Battlefield);
    engine
        .state
        .objects
        .get_mut(&id)
        .unwrap()
        .set_counter(CounterKind::Time, 0);
    reject(&mut engine, 17, &command); // A reduction never changes Lightning Bolt's mana value.
    engine
        .state
        .objects
        .get_mut(&id)
        .unwrap()
        .set_counter(CounterKind::Time, 1);
    engine.apply_command(17, &command).unwrap();
    assert_eq!(engine.state.players[0].mana_pool.colorless, 0);
    assert_eq!(
        engine
            .characteristics(engine.state.stack[0].id)
            .unwrap()
            .mana_value,
        1
    );
}

#[test]
fn as_foretold_library_needs_its_independent_permission_and_required_exile_cost_cannot_combine() {
    let mut engine = fixture();
    let id = source(&mut engine, 17, 1);
    let library = engine.state.players[0].library[0];
    let mut command = bolt(&engine, 17, id);
    cast_mut(&mut command).source = Some(rv1::CastSource {
        location: Some(rv1::cast_source::Location::LibraryObjectId(library)),
        expected_zone_change_generation: Some(
            engine.payment_object_ref(library).zone_change_generation,
        ),
    });
    reject(&mut engine, 17, &command);
    // Prepared independent permission seam; no new source zone or permission is supplied by As Foretold.
    engine.state.continuous_effects.push(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: None,
        affected: AffectedScope::Player(17),
        condition: None,
        kind: ContinuousEffectKind::LibraryTopPermission {
            look_at_top: true,
            cast_filter: Some(ZoneCardFilter {
                card_type: Some(CardTypeFilter::InstantOrSorcery),
                ..Default::default()
            }),
            play_lands: false,
        },
        duration: EffectDuration::Indefinite,
        timestamp: engine.state.command_index,
    });
    let offers = engine.initial_response_batch();
    let offer = offers.legal_by_player[&17]
        .zone_cast_actions
        .iter()
        .find(|a| a.object_id == library)
        .unwrap();
    assert_eq!(
        offer.as_foretold_costs[0].source,
        Some(engine.payment_object_ref(id))
    );
    engine.apply_command(17, &command).unwrap();
    assert_eq!(engine.state.stack[0].id, library);
    let mut engine = fixture();
    let id = source(&mut engine, 17, 1);
    let exiled = engine.state.players[0].hand[0];
    resolution::move_object_to_zone(
        &mut engine.state,
        engine.registry,
        exiled,
        Zone::Exile,
        None,
    )
    .unwrap();
    let mut command = bolt(&engine, 17, id);
    let generation = engine.payment_object_ref(exiled).zone_change_generation;
    cast_mut(&mut command).source = Some(rv1::CastSource {
        location: Some(rv1::cast_source::Location::ExileObjectId(exiled)),
        expected_zone_change_generation: Some(generation),
    });
    reject(&mut engine, 17, &command);
    engine
        .state
        .active_exile_play_permissions
        .push(ActiveExilePlayPermission {
            group_id: 700,
            player_id: 17,
            source_label: "Prepared alternative permission".into(),
            object_id: exiled,
            zone_change_generation: generation,
            scope: ExilePlayPermissionScope::CastCard,
            max_face_mana_value: None,
            cast_cost: crate::state::ExilePermissionCastCost::AlternativeManaCost(
                ManaCost::parse("{0}").unwrap(),
            ),
            origin: crate::state::ExilePlayPermissionOrigin::Effect,
            available_after_turn_instance: None,
            expires_at_cleanup_turn_instance: None,
        });
    cast_mut(&mut command).casting_permission_id = Some(700);
    cast_mut(&mut command).cast_method = rv1::CastMethod::Permission as i32;
    reject(&mut engine, 17, &command);
    cast_mut(&mut command).as_foretold_source = None;
    engine.apply_command(17, &command).unwrap();
    assert!(engine.state.as_foretold_uses.is_empty());
}

#[test]
fn as_foretold_does_not_grant_graveyard_origin_or_combine_with_flashback_or_harmonize() {
    for method in [rv1::CastMethod::Flashback, rv1::CastMethod::Harmonize] {
        let mut engine = fixture_spell(
            r#"(id:"test_spell",name:"Test Spell",face_id:"test_spell",mana_cost:"{U}",types:["Sorcery"],flashback_cost:Some("{U}"),harmonize_cost:Some("{U}"),spell_effect:[GainLife(amount:1)])"#,
        );
        let id = source(&mut engine, 17, 1);
        let spell = engine.state.players[0].hand[0];
        resolution::move_object_to_zone(
            &mut engine.state,
            engine.registry,
            spell,
            Zone::Graveyard,
            None,
        )
        .unwrap();
        let mut command = plain();
        cast_mut(&mut command).source = Some(rv1::CastSource {
            location: Some(rv1::cast_source::Location::GraveyardObjectId(spell)),
            expected_zone_change_generation: Some(
                engine.payment_object_ref(spell).zone_change_generation,
            ),
        });
        cast_mut(&mut command).as_foretold_source = Some(engine.payment_object_ref(id));
        reject(&mut engine, 17, &command);
        cast_mut(&mut command).cast_method = method as i32;
        reject(&mut engine, 17, &command);
        cast_mut(&mut command).as_foretold_source = None;
        engine.state.players[0].mana_pool.blue = 1;
        engine.apply_command(17, &command).unwrap();
        assert!(engine.state.as_foretold_uses.is_empty());
    }
}
