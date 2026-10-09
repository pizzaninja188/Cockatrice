use super::*;

const REPEATED_ENTRY: &str = r#"(
    id: "repeated_entry", name: "Repeated Entry", face_id: "repeated_entry",
    mana_cost: "{0}", types: ["Artifact"],
    cast_cost_groups: [(group_id: "multikicker", presentation: Fallback,
        options: [Mana(option_id: "kick", presentation: Fallback,
            kind: Multikicker, cost: "{2}")])],
    static_abilities: [(ability_id: "entry", presentation: Fallback,
        definition: EntersWithCounters(affected: Self_, counter: Charge,
            amount: Count(CastCostPaymentCount(cost: (
                group_id: "multikicker", option_id: "kick")))))],
    activated_abilities: [(ability_id: "mana", presentation: Fallback,
        costs: [Tap], effect: [ProduceManaPerSourceCounter(counter: Charge,
            options: [(c: 1)])])],
)"#;

fn engine() -> GameEngine {
    engine_for("repeated_entry")
}

fn engine_for(card: &str) -> GameEngine {
    let copier = REPEATED_ENTRY.replace("repeated_entry", "repeated_copy_entry")
        .replace("Repeated Entry", "Repeated Copy Entry")
        .replace("static_abilities: [", "static_abilities: [(ability_id: \"copy\", presentation: Fallback, definition: EntersAsCopy(filter: (kind: AnyPermanent))),");
    let probe = r#"(id: "copy_probe", name: "Copy Probe", face_id: "copy_probe",
        mana_cost: "{0}", types: ["Instant"], spell_effect: [CopyTargetSpell(count: 1,
            spell_filter: (card_type: Some(Artifact)))])"#;
    let prohibited = REPEATED_ENTRY.replace("repeated_entry", "prohibited_entry")
        .replace("Repeated Entry", "Prohibited Entry")
        .replace("static_abilities: [", "static_abilities: [(ability_id: \"no_counters\", presentation: Fallback, definition: ProhibitCounters()),");
    let registry =
        CardRegistry::from_chunks_and_tokens(&[REPEATED_ENTRY, &copier, probe, &prohibited], &[])
            .unwrap();
    let decks = (0..2)
        .map(|_| EngineDeck {
            mainboard: vec![card.into(); 40],
            commanders: vec![],
        })
        .collect();
    let mut engine = GameEngine::new_with_registry(
        704_001,
        &[0, 1],
        20,
        Some(decks),
        true,
        Box::leak(Box::new(registry)),
    )
    .unwrap();
    engine.state.turn_step = TurnStep::Main1;
    engine.state.priority_idx = 0;
    engine
}

fn selection(repetitions: Option<u32>) -> rv1::CastCostGroupSelection {
    rv1::CastCostGroupSelection {
        repetitions,
        ..Default::default()
    }
}

fn cast(e: &GameEngine, selections: Vec<rv1::CastCostGroupSelection>) -> RuledCommand {
    cast_object(e, e.state.players[0].hand[0], selections, vec![])
}

fn cast_object(
    e: &GameEngine,
    oid: ObjectId,
    selections: Vec<rv1::CastCostGroupSelection>,
    targets: Vec<rv1::TargetRef>,
) -> RuledCommand {
    RuledCommand {
        cmd: Some(rv1::ruled_command::Cmd::CastSpell(rv1::CastSpell {
            cast_method: rv1::CastMethod::Normal as i32,
            source: Some(rv1::CastSource {
                location: Some(rv1::cast_source::Location::HandIndex(
                    e.state.players[0]
                        .hand
                        .iter()
                        .position(|id| *id == oid)
                        .unwrap() as u32,
                )),
                expected_zone_change_generation: Some(
                    e.state
                        .zone_change_generation
                        .get(&oid)
                        .copied()
                        .unwrap_or(0),
                ),
            }),
            cast_cost_group_selections: selections,
            targets,
            ..Default::default()
        })),
    }
}

fn pass_two(e: &mut GameEngine) {
    for _ in 0..2 {
        e.apply_command(
            e.state.priority_player_id(),
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::PassPriority(rv1::PassPriority {})),
            },
        )
        .unwrap();
    }
}

fn put_hand(e: &mut GameEngine, name: &str) -> ObjectId {
    e.apply_dev_command(&rv1::DevCommand {
        target_player_id: 0,
        dev: Some(rv1::dev_command::Dev::PutCardInZone(
            rv1::DevPutCardInZone {
                card_name: name.into(),
                zone: rv1::DevZone::Hand as i32,
                ready: true,
            },
        )),
    })
    .unwrap();
    e.state.next_object_id - 1
}

fn resolve(e: &mut GameEngine) {
    for _ in 0..8 {
        if e.state.stack.is_empty() {
            return;
        }
        e.apply_command(
            e.state.priority_player_id(),
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::PassPriority(rv1::PassPriority {})),
            },
        )
        .unwrap();
    }
    panic!("private artifact must resolve within two passes");
}

#[test]
fn multikicker_preparation_folds_count_and_records_payment_history() {
    let e = engine();
    let face = e.registry.get("repeated_entry").unwrap().primary_face();
    let prepared = e
        .prepare_spell_costs(
            0,
            0,
            e.state.players[0].hand[0],
            &face.mana_cost,
            0,
            0,
            0,
            &[],
            &[],
            &[],
            &face.cast_cost_groups,
            &[selection(Some(3))],
            &[],
            &[],
            SpellCastMethod::Normal,
            Some(&crate::state::CastCostAbilityOrigin {
                original_card_id: "repeated_entry".into(),
                face_id: face.face_id.clone(),
                copy_revision: 0,
            }),
        )
        .unwrap();
    assert_eq!(prepared.total_cost_label().unwrap(), "{6}");
    assert_eq!(
        prepared.mana.pips.len(),
        2,
        "one folded fee, independent of count"
    );
}

#[test]
fn multikicker_cast_entry_and_counter_mana_use_the_announced_count() {
    for count in [0, 1, 3] {
        let mut e = engine();
        let oid = e.state.players[0].hand[0];
        e.state.players[0].mana_pool.colorless = 2 * count;
        let selections = if count == 0 {
            vec![]
        } else {
            vec![selection(Some(count))]
        };
        e.apply_command(0, &cast(&e, selections)).unwrap();
        assert_eq!(e.state.players[0].mana_pool.colorless, 0);
        let item = e.state.stack.last().unwrap();
        assert_eq!(
            e.registry
                .get(&item.card_id)
                .unwrap()
                .primary_face()
                .mana_cost
                .mana_value_on_stack(item.chosen_x),
            0,
            "additional costs never increase mana value"
        );
        if count > 0 {
            let receipt = item.cast_cost_receipts[0].multikicker.as_ref().unwrap();
            assert_eq!(receipt.repetitions, count);
            assert_eq!(receipt.origin.original_card_id, "repeated_entry");
            assert_eq!(receipt.origin.copy_revision, 0);
        } else {
            assert!(item.cast_cost_receipts.is_empty());
        }
        resolve(&mut e);
        assert_eq!(
            e.state.objects[&oid].counter_count(CounterKind::Charge),
            count
        );
        e.apply_command(
            0,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::ActivateAbility(
                    rv1::ActivateAbility {
                        source_object_id: oid,
                        ability_index: 0,
                        expected_zone_change_generation: e.state.zone_change_generation[&oid],
                        ..Default::default()
                    },
                )),
            },
        )
        .unwrap();
        assert!(e.state.objects[&oid].tapped);
        assert!(
            e.state.stack.is_empty(),
            "counter-based production is a mana ability"
        );
        assert_eq!(e.state.players[0].mana_pool.colorless, count);
    }
}

#[test]
fn multikicker_bad_announcements_are_atomic_before_source_reservation() {
    let mut bogus_object = selection(Some(2));
    bogus_object.battlefield_objects = Some(rv1::CostObjectRefs::default());
    let mut bogus_generation = selection(Some(2));
    bogus_generation.expected_zone_change_generation = 1;
    for selections in [
        vec![selection(Some(0))],
        vec![selection(Some(u32::MAX))],
        vec![selection(Some(1_073_741_824))],
        vec![selection(Some(1)), selection(Some(1))],
        vec![bogus_object],
        vec![bogus_generation],
        vec![rv1::CastCostGroupSelection {
            option_index: 1,
            ..selection(Some(2))
        }],
        vec![rv1::CastCostGroupSelection {
            group_index: 1,
            ..selection(Some(2))
        }],
    ] {
        let mut e = engine();
        let before = format!("{:?}", e.state);
        assert!(e.apply_command(0, &cast(&e, selections)).is_err());
        assert_eq!(format!("{:?}", e.state), before);
        assert!(e.pending_spell_cast_internal.is_none());
    }
}

#[test]
fn multikicker_arithmetic_bound_includes_base_x_tax_before_reduction() {
    let e = engine();
    let face = e.registry.get("repeated_entry").unwrap().primary_face();
    let origin = crate::state::CastCostAbilityOrigin {
        original_card_id: "repeated_entry".into(),
        face_id: face.face_id.clone(),
        copy_revision: 0,
    };
    let prepare = |base: &str, x, tax, reduction, count| {
        e.prepare_spell_costs(
            0,
            0,
            e.state.players[0].hand[0],
            &ManaCost::parse(base).unwrap(),
            x,
            tax,
            reduction,
            &[],
            &[],
            &[],
            &face.cast_cost_groups,
            &[selection(Some(count))],
            &[],
            &[],
            SpellCastMethod::Normal,
            Some(&origin),
        )
    };
    assert_eq!(
        prepare("{0}", 0, 0, 0, 1_073_741_823)
            .unwrap()
            .total_cost_label()
            .unwrap(),
        "{2147483646}"
    );
    assert_eq!(
        prepare("{0}", 0, 1, 0, 1_073_741_823)
            .unwrap()
            .total_cost_label()
            .unwrap(),
        "{2147483647}"
    );
    assert!(prepare("{0}", 0, 2, 0, 1_073_741_823).is_err());
    assert!(prepare("{R}{R}", 0, 0, 0, 1_073_741_823).is_err());
    assert!(prepare("{X}", 2, 0, 0, 1_073_741_823).is_err());
    assert!(
        prepare("{0}", 0, 2, 2, 1_073_741_823).is_err(),
        "reduction must not conceal an unrepresentable staged aggregate"
    );
    assert_eq!(
        prepare("{0}", 0, 1, 7, 3)
            .unwrap()
            .total_cost_label()
            .unwrap(),
        "{0}"
    );
    assert!(prepare("{0}", 0, 0, 0, 1_073_741_824).is_err());
}

#[test]
fn multikicker_permanent_spell_copy_retains_payment_count_at_a_new_object_id() {
    let mut e = engine();
    e.state.players[0].mana_pool.colorless = 6;
    let original = e.state.players[0].hand[0];
    e.apply_command(0, &cast(&e, vec![selection(Some(3))]))
        .unwrap();
    let original_receipt = e.state.stack.last().unwrap().cast_cost_receipts.clone();
    let probe = put_hand(&mut e, "Copy Probe");
    e.apply_command(
        0,
        &cast_object(
            &e,
            probe,
            vec![],
            vec![rv1::TargetRef {
                kind: rv1::TargetRefKind::Stack as i32,
                object_id: original,
                ..Default::default()
            }],
        ),
    )
    .unwrap();
    pass_two(&mut e);
    let copy = e.state.stack.last().unwrap();
    assert!(copy.is_copy);
    assert_ne!(copy.id, original);
    assert!(copy.cast_occurrence.is_none());
    assert_eq!(copy.cast_cost_receipts, original_receipt);
    let copy_id = copy.id;
    assert!(!e.state.objects.contains_key(&copy_id));
    pass_two(&mut e);
    assert!(e.state.objects[&copy_id].is_token());
    assert_eq!(e.state.objects[&copy_id].copy_revision, 0);
    assert_eq!(
        e.state.objects[&copy_id].counter_count(CounterKind::Charge),
        3
    );
    assert_eq!(e.state.stack.last().unwrap().id, original);
    pass_two(&mut e);
    assert_eq!(
        e.state.objects[&original].counter_count(CounterKind::Charge),
        3
    );
}

#[test]
fn multikicker_entry_copy_acquires_a_new_link_even_with_the_same_local_ids() {
    for same_definition in [false, true] {
        let mut e = engine_for(if same_definition {
            "repeated_copy_entry"
        } else {
            "repeated_entry"
        });
        e.state.players[0].mana_pool.colorless = 2;
        let model = e.state.players[0].hand[0];
        e.apply_command(0, &cast(&e, vec![selection(Some(1))]))
            .unwrap();
        resolve(&mut e);
        let entrant = put_hand(&mut e, "Repeated Copy Entry");
        e.state.players[0].mana_pool.colorless = 6;
        e.apply_command(
            0,
            &cast_object(&e, entrant, vec![selection(Some(3))], vec![]),
        )
        .unwrap();
        pass_two(&mut e);
        let pending = e.state.pending_resolution.as_ref().unwrap();
        assert_eq!(
            pending.presentation.choice_kind,
            rv1::ChoiceKind::CopySource
        );
        assert!(matches!(
            pending.continuation,
            crate::state::ResolutionContinuation::EntryCopySource { .. }
        ));
        assert_eq!(e.state.objects[&entrant].copy_revision, 0);
        e.apply_command(
            0,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(
                    rv1::SubmitResolutionChoice {
                        chosen_object_ids: vec![model],
                        ..Default::default()
                    },
                )),
            },
        )
        .unwrap();
        assert_eq!(e.state.objects[&entrant].copy_revision, 1);
        if same_definition {
            let pending = e.state.pending_resolution.as_ref().unwrap();
            assert_eq!(
                pending.presentation.choice_kind,
                rv1::ChoiceKind::CopySource
            );
            e.apply_command(
                0,
                &RuledCommand {
                    cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(
                        rv1::SubmitResolutionChoice::default(),
                    )),
                },
            )
            .unwrap();
        }
        assert!(e.state.pending_resolution.is_none());
        assert_eq!(
            e.state.objects[&model].counter_count(CounterKind::Charge),
            1
        );
        assert_eq!(
            e.state.objects[&entrant].counter_count(CounterKind::Charge),
            0,
            "a new intrinsic linked pair cannot read the earlier payment receipt"
        );
    }
}

#[test]
fn multikicker_zero_actual_mana_payment_keeps_its_announced_count() {
    let mut e = engine();
    let face = e.registry.get("repeated_entry").unwrap().primary_face();
    let origin = crate::state::CastCostAbilityOrigin {
        original_card_id: "repeated_entry".into(),
        face_id: face.face_id.clone(),
        copy_revision: 0,
    };
    let prepared = e
        .prepare_spell_costs(
            0,
            0,
            e.state.players[0].hand[0],
            &face.mana_cost,
            0,
            0,
            6,
            &[],
            &[],
            &[],
            &face.cast_cost_groups,
            &[selection(Some(3))],
            &[],
            &[],
            SpellCastMethod::Normal,
            Some(&origin),
        )
        .unwrap();
    assert_eq!(prepared.total_cost_label().unwrap(), "{0}");
    let paid = e
        .commit_cost_transaction(prepared.finish(&e.state).unwrap())
        .unwrap();
    assert_eq!(paid.mana_spent, 0);
    assert_eq!(
        paid.cast_cost_receipts[0]
            .multikicker
            .as_ref()
            .unwrap()
            .repetitions,
        3
    );
}

#[test]
fn multikicker_locked_count_survives_failed_payment_and_cancellation_retries() {
    let mut e = engine();
    let source = e.state.players[0].hand[0];
    let announcement = rv1::SpellCastAnnouncement {
        source: Some(rv1::CastSource {
            location: Some(rv1::cast_source::Location::HandIndex(0)),
            ..Default::default()
        }),
        cast_method: rv1::CastMethod::Normal as i32,
        cast_cost_group_selections: vec![selection(Some(3))],
        ..Default::default()
    };
    let begin = RuledCommand {
        cmd: Some(rv1::ruled_command::Cmd::BeginSpellCast(
            rv1::BeginSpellCast {
                announcement: Some(announcement.clone()),
            },
        )),
    };
    e.apply_command(0, &begin).unwrap();
    let pending = e.state.pending_spell_cast.as_ref().unwrap();
    assert_eq!(pending.locked_total_cost, "{6}");
    assert_eq!(
        pending.announcement.cast_cost_group_selections[0].repetitions,
        Some(3)
    );
    let transaction_id = pending.transaction_id;
    let commit = RuledCommand {
        cmd: Some(rv1::ruled_command::Cmd::CommitSpellCast(
            rv1::CommitSpellCast {
                transaction_id,
                ..Default::default()
            },
        )),
    };
    let before = format!("{:?}", e.state);
    assert!(e.apply_command(0, &commit).is_err());
    assert_eq!(format!("{:?}", e.state), before);
    e.apply_command(
        0,
        &RuledCommand {
            cmd: Some(rv1::ruled_command::Cmd::CancelSpellCast(
                rv1::CancelSpellCast { transaction_id },
            )),
        },
    )
    .unwrap();
    assert_eq!(e.state.players[0].hand[0], source);
    assert!(e.state.pending_spell_cast.is_none());
    assert!(e.state.stack.is_empty());
    assert_eq!(
        e.state.objects[&source].counter_count(CounterKind::Charge),
        0
    );
    e.apply_command(0, &begin).unwrap();
    let retry_id = e.state.pending_spell_cast.as_ref().unwrap().transaction_id;
    e.state.players[0].mana_pool.colorless = 6;
    e.apply_command(
        0,
        &RuledCommand {
            cmd: Some(rv1::ruled_command::Cmd::CommitSpellCast(
                rv1::CommitSpellCast {
                    transaction_id: retry_id,
                    ..Default::default()
                },
            )),
        },
    )
    .unwrap();
    assert_eq!(
        e.state.stack.last().unwrap().cast_cost_receipts[0]
            .multikicker
            .as_ref()
            .unwrap()
            .repetitions,
        3
    );
    resolve(&mut e);
    assert_eq!(
        e.state.objects[&source].counter_count(CounterKind::Charge),
        3
    );
}

#[test]
fn multikicker_stack_presentation_keeps_the_dynamic_count_visible() {
    let mut e = engine();
    let ron = REPEATED_ENTRY.replace(
        "option_id: \"kick\", presentation: Fallback,",
        "option_id: \"kick\", presentation: OracleLines([1]),",
    );
    e.registry = Box::leak(Box::new(
        CardRegistry::from_chunks_and_tokens(&[&ron], &[]).unwrap(),
    ));
    e.state.players[0].mana_pool.colorless = 6;
    let batch = e
        .apply_command(0, &cast(&e, vec![selection(Some(3))]))
        .unwrap();
    let pushed = batch
        .events
        .iter()
        .find_map(|event| match &event.ev {
            Some(rv1::ruled_event::Ev::StackPushed(pushed)) => Some(pushed),
            _ => None,
        })
        .unwrap();
    let presentation = &pushed.chosen_cast_cost_presentations[0];
    assert_eq!(presentation.fallback_text, "Multikicker {2} × 3");
    assert!(
        presentation.oracle_line_indices.is_empty(),
        "a resolved static Oracle line cannot replace the dynamic paid count"
    );
}

#[test]
fn multikicker_entry_counter_prohibition_keeps_the_paid_count_separate() {
    let mut e = engine_for("prohibited_entry");
    e.state.players[0].mana_pool.colorless = 6;
    let oid = e.state.players[0].hand[0];
    e.apply_command(0, &cast(&e, vec![selection(Some(3))]))
        .unwrap();
    assert_eq!(
        e.state.stack.last().unwrap().cast_cost_receipts[0]
            .multikicker
            .as_ref()
            .unwrap()
            .repetitions,
        3
    );
    resolve(&mut e);
    assert_eq!(e.state.objects[&oid].counter_count(CounterKind::Charge), 0);
}

#[test]
fn multikicker_absent_count_is_one_and_ordinary_kicker_rejects_repeat_counts() {
    let mut e = engine();
    e.state.players[0].mana_pool.colorless = 2;
    let source = e.state.players[0].hand[0];
    e.apply_command(0, &cast(&e, vec![selection(None)]))
        .unwrap();
    assert_eq!(
        e.state.stack.last().unwrap().cast_cost_receipts[0]
            .multikicker
            .as_ref()
            .unwrap()
            .repetitions,
        1
    );
    resolve(&mut e);
    assert_eq!(
        e.state.objects[&source].counter_count(CounterKind::Charge),
        1
    );

    let e = engine();
    let face = e.registry.get("repeated_entry").unwrap().primary_face();
    let mut groups = face.cast_cost_groups.clone();
    let CastCostOptionDef::Mana { kind, .. } = &mut groups[0].options[0] else {
        unreachable!()
    };
    *kind = tricerules_card_model::primitives::ManaCostChoiceKind::Kicker;
    for repetitions in [None, Some(1), Some(0), Some(2)] {
        let result = e.prepare_spell_costs(
            0,
            0,
            e.state.players[0].hand[0],
            &face.mana_cost,
            0,
            0,
            0,
            &[],
            &[],
            &[],
            &groups,
            &[selection(repetitions)],
            &[],
            &[],
            SpellCastMethod::Normal,
            None,
        );
        assert_eq!(result.is_ok(), repetitions.is_none_or(|count| count == 1));
    }
}

#[test]
fn multikicker_private_comet_storm_generic_fee_witness_composes_with_x_and_red_cost() {
    let e = engine();
    let face = e.registry.get("repeated_entry").unwrap().primary_face();
    let mut groups = face.cast_cost_groups.clone();
    let CastCostOptionDef::Mana { cost, .. } = &mut groups[0].options[0] else {
        unreachable!()
    };
    *cost = ManaCost::parse("{1}").unwrap();
    let origin = crate::state::CastCostAbilityOrigin {
        original_card_id: "private_generic_fee_witness".into(),
        face_id: face.face_id.clone(),
        copy_revision: 0,
    };
    let prepared = e
        .prepare_spell_costs(
            0,
            0,
            e.state.players[0].hand[0],
            &ManaCost::parse("{X}{R}{R}").unwrap(),
            4,
            0,
            0,
            &[],
            &[],
            &[],
            &groups,
            &[selection(Some(3))],
            &[],
            &[],
            SpellCastMethod::Normal,
            Some(&origin),
        )
        .unwrap();
    assert_eq!(prepared.total_cost_label().unwrap(), "{7}{R}{R}");
    assert!(
        tricerules_cards::registry::global()
            .get("comet_storm")
            .is_none(),
        "the payment witness never admits an incomplete whole card"
    );
}
