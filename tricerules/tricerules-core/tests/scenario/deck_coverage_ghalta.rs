//! Ghalta sums signed current creature powers before locking the total casting cost.
use super::helpers::*;
use tricerules_cards::{
    CardRegistry, ContinuousEffectKind, ControllerReference, CounterKind, EffectDuration, Keyword,
    PermanentTypeFilter, TypeLineReplacement,
};
use tricerules_core::state::Zone;
use tricerules_core::{AffectedScope, ContinuousEffect};
use tricerules_proto::ruled::v1::{
    BeginSpellCast, CancelSpellCast, CommitSpellCast, PaymentMana, PaymentSelection,
    PreviewPayment, SpellCastAnnouncement,
};

fn setup() -> GameEngine {
    let deck = deck_with("forest", &["ghalta,_primal_hunger"]);
    let mut engine =
        GameEngine::new(26_100_901, &[0, 1, 2], 20, Some(vec![deck; 3]), true).unwrap();
    advance_to_main1_from_game_start(&mut engine);
    ensure_card_in_hand(&mut engine, 0, "ghalta,_primal_hunger");
    engine
}

fn modify(engine: &mut GameEngine, object: u32, kind: ContinuousEffectKind) {
    engine.state.continuous_effects.push(ContinuousEffect {
        source_id: None,
        trigger_grant_origin: None,
        affected: AffectedScope::Single(object),
        kind,
        condition: None,
        duration: EffectDuration::Indefinite,
        timestamp: engine.state.command_index,
    });
}

fn begin(engine: &mut GameEngine) -> u64 {
    let command = begin_command(engine);
    engine.apply_command(0, &command).unwrap();
    engine
        .state
        .pending_spell_cast
        .as_ref()
        .unwrap()
        .transaction_id
}

fn begin_command(engine: &GameEngine) -> RuledCommand {
    let slot = hand_index_for_card(engine, 0, "ghalta,_primal_hunger");
    let Some(Cmd::CastSpell(cast)) = cast_spell(slot, vec![]).cmd else {
        unreachable!()
    };
    RuledCommand {
        cmd: Some(Cmd::BeginSpellCast(BeginSpellCast {
            announcement: Some(SpellCastAnnouncement {
                targets: cast.targets,
                x_value: cast.x_value,
                flex_payments: cast.flex_payments,
                face_index: cast.face_index,
                selected_modes: cast.selected_modes,
                source: cast.source,
                cost_selections: cast.cost_selections,
                cast_cost_group_selections: cast.cast_cost_group_selections,
                cast_method: cast.cast_method,
                casting_permission_id: cast.casting_permission_id,
            }),
        })),
    }
}

#[test]
fn ghalta_cost_sums_signed_current_creature_powers() {
    let mut engine = setup();
    let positive = inject_creature_on_battlefield(&mut engine, 0, "serra_angel");
    let negative = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    modify(
        &mut engine,
        positive,
        ContinuousEffectKind::PtModify {
            delta_power: 3,
            delta_toughness: 0,
        },
    );
    modify(
        &mut engine,
        negative,
        ContinuousEffectKind::PtModify {
            delta_power: -5,
            delta_toughness: 0,
        },
    );
    assert_eq!(engine.characteristics(positive).unwrap().power, Some(5));
    assert_eq!(engine.characteristics(negative).unwrap().power, Some(0));
    begin(&mut engine);
    assert_eq!(
        engine
            .state
            .pending_spell_cast
            .as_ref()
            .unwrap()
            .locked_total_cost,
        "{8}{G}{G}",
        "five plus negative three gives a two-mana reduction"
    );
}

fn expect_cost(engine: &mut GameEngine, cost: &str) {
    let transaction_id = begin(engine);
    assert_eq!(
        engine
            .state
            .pending_spell_cast
            .as_ref()
            .unwrap()
            .locked_total_cost,
        cost
    );
    engine
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::CancelSpellCast(CancelSpellCast { transaction_id })),
            },
        )
        .unwrap();
}

#[test]
fn ghalta_empty_zero_and_negative_totals_leave_full_cost() {
    let mut engine = setup();
    expect_cost(&mut engine, "{10}{G}{G}");
    let own = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    for opponent in [1, 2] {
        inject_creature_with_stats(&mut engine, opponent, "grizzly_bears", 20, 20);
    }
    modify(
        &mut engine,
        own,
        ContinuousEffectKind::PtModify {
            delta_power: -2,
            delta_toughness: 0,
        },
    );
    expect_cost(&mut engine, "{10}{G}{G}");
    modify(
        &mut engine,
        own,
        ContinuousEffectKind::PtModify {
            delta_power: -3,
            delta_toughness: 0,
        },
    );
    expect_cost(&mut engine, "{10}{G}{G}");
}

#[test]
fn ghalta_reads_current_control_types_pumps_and_counters_before_announcement() {
    let mut engine = setup();
    let own = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let stolen = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let other = inject_creature_on_battlefield(&mut engine, 2, "grizzly_bears");
    expect_cost(&mut engine, "{8}{G}{G}");
    modify(
        &mut engine,
        stolen,
        ContinuousEffectKind::Layer2Control {
            controller: ControllerReference::Fixed(0),
        },
    );
    engine
        .state
        .objects
        .get_mut(&own)
        .unwrap()
        .add_counters(CounterKind::PlusOnePlusOne, 1, 1);
    modify(
        &mut engine,
        own,
        ContinuousEffectKind::PtModify {
            delta_power: 2,
            delta_toughness: 0,
        },
    );
    assert_eq!(engine.characteristics(own).unwrap().power, Some(5));
    assert_eq!(engine.characteristics(stolen).unwrap().controller, 0);
    assert_eq!(engine.characteristics(other).unwrap().controller, 2);
    expect_cost(&mut engine, "{3}{G}{G}");
    modify(
        &mut engine,
        stolen,
        ContinuousEffectKind::Layer4SetTypeLine(TypeLineReplacement {
            land_types: Vec::new(),
            card_types: vec![PermanentTypeFilter::Artifact],
            creature_types: vec![],
        }),
    );
    expect_cost(&mut engine, "{5}{G}{G}");
    modify(
        &mut engine,
        own,
        ContinuousEffectKind::Layer2Control {
            controller: ControllerReference::Fixed(1),
        },
    );
    expect_cost(&mut engine, "{10}{G}{G}");
}

#[test]
fn ghalta_discount_preserves_green_and_rejects_bad_actor_stale_or_incomplete_payment_atomically() {
    let mut engine = setup();
    inject_creature_with_stats(&mut engine, 0, "grizzly_bears", 15, 15);
    let transaction_id = begin(&mut engine);
    assert_eq!(
        engine
            .state
            .pending_spell_cast
            .as_ref()
            .unwrap()
            .locked_total_cost,
        "{G}{G}"
    );
    engine.state.players[0].mana_pool.green = 1;
    engine.state.players[0].mana_pool.colorless = 20;
    for (player, id) in [
        (1, transaction_id),
        (0, transaction_id + 1),
        (0, transaction_id),
    ] {
        let before = format!("{:?}", engine.state);
        let command = RuledCommand {
            cmd: Some(Cmd::CommitSpellCast(CommitSpellCast {
                transaction_id: id,
                payment: Some(PaymentSelection {
                    mana: Some(PaymentMana {
                        g: 1,
                        c: 1,
                        ..Default::default()
                    }),
                    ..Default::default()
                }),
                ..Default::default()
            })),
        };
        assert!(engine.apply_command(player, &command).is_err());
        assert_eq!(format!("{:?}", engine.state), before);
    }
}

fn grant_sacrifice_for_green(engine: &mut GameEngine, object: u32) {
    let draft = r#"(id:"mana_probe",name:"Mana Probe",face_id:"mana_probe",types:["Creature"],power:2,toughness:2,
        activated_abilities:[(ability_id:"activated_01",presentation:Fallback,costs:[SacrificeSelf],
        effect:[ProduceMana(options:[(g:1)])])])"#;
    let registry = CardRegistry::from_chunks_and_tokens(&[draft], &[]).unwrap();
    modify(
        engine,
        object,
        ContinuousEffectKind::GrantActivatedAbility(Box::new(
            registry
                .get("mana_probe")
                .unwrap()
                .primary_face()
                .activated_abilities[0]
                .clone(),
        )),
    );
}

#[test]
fn ghalta_locked_sacrifice_mana_payment_paid_cast_and_serialized_replay() {
    use prost::Message;
    fn fresh() -> (GameEngine, u32) {
        let mut engine = setup();
        let contributor = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
        inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
        inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
        grant_sacrifice_for_green(&mut engine, contributor);
        engine.state.players[0].mana_pool.green = 5;
        (engine, contributor)
    }
    let (mut engine, contributor) = fresh();
    let source =
        engine.state.players[0].hand[hand_index_for_card(&engine, 0, "ghalta,_primal_hunger")];
    let mut commands = Vec::new();
    let mut batches = Vec::new();
    let mut apply = |engine: &mut GameEngine, actor: i32, command: RuledCommand| {
        let encoded = command.encode_to_vec();
        let decoded = RuledCommand::decode(encoded.as_slice()).unwrap();
        batches.push(engine.apply_command(actor, &decoded).unwrap());
        commands.push((actor, decoded));
    };
    let begin = begin_command(&engine);
    apply(&mut engine, 0, begin);
    let pending = engine.state.pending_spell_cast.as_ref().unwrap();
    assert_eq!(pending.locked_total_cost, "{4}{G}{G}");
    let transaction_id = pending.transaction_id;
    apply(&mut engine, 0, activate_ability(contributor, 1, vec![]));
    assert_eq!(engine.state.objects[&contributor].zone, Zone::Graveyard);
    assert_eq!(engine.state.players[0].mana_pool.green, 6);
    assert_eq!(
        engine
            .state
            .pending_spell_cast
            .as_ref()
            .unwrap()
            .locked_total_cost,
        "{4}{G}{G}"
    );
    let preview = engine.preview_payment(
        0,
        &PreviewPayment {
            transaction_id,
            revision: 1,
            commit_spell_cast: Some(CommitSpellCast {
                transaction_id,
                payment: Some(PaymentSelection {
                    mana: Some(PaymentMana {
                        g: 6,
                        ..Default::default()
                    }),
                    ..Default::default()
                }),
                ..Default::default()
            }),
            ..Default::default()
        },
    );
    assert!(preview.valid && preview.complete, "{preview:?}");
    apply(
        &mut engine,
        0,
        RuledCommand {
            cmd: Some(Cmd::CommitSpellCast(CommitSpellCast {
                transaction_id,
                payment: preview.selection,
                restricted_mana: preview.restricted_mana,
            })),
        },
    );
    assert_eq!(engine.state.players[0].mana_pool.green, 0);
    assert_eq!(engine.state.objects[&source].zone, Zone::Stack);
    for _ in 0..3 {
        let actor = engine.state.priority_player_id();
        apply(&mut engine, actor, pass());
    }
    assert_eq!(engine.state.objects[&source].zone, Zone::Battlefield);
    let face = engine.characteristics(source).unwrap();
    assert_eq!((face.power, face.toughness), (Some(12), Some(12)));
    assert!(face.keywords.contains(&Keyword::Trample));
    assert_eq!(face.mana_value, 12);
    assert!(face.supertypes.contains(&"Legendary".to_string()));
    let (mut replay, replay_contributor) = fresh();
    assert_eq!(replay_contributor, contributor);
    let replayed = commands
        .iter()
        .map(|(actor, command)| replay.apply_command(*actor, command).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(batches, replayed);
    assert_eq!(engine.state.command_index, replay.state.command_index);
    assert_eq!(
        engine.state.zone_change_generation,
        replay.state.zone_change_generation
    );
    assert_eq!(
        serde_json::to_value(&engine.state.players).unwrap(),
        serde_json::to_value(&replay.state.players).unwrap()
    );
    for (id, object) in &engine.state.objects {
        assert_eq!(
            serde_json::to_value(object).unwrap(),
            serde_json::to_value(&replay.state.objects[id]).unwrap()
        );
    }
}

#[test]
fn ghalta_accepts_generator_servant_mana_produced_after_cost_lock() {
    let mut engine = setup();
    let servant = inject_permanent_on_battlefield(&mut engine, 0, "generator_servant");
    inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    engine.state.players[0].mana_pool.colorless = 4;
    engine.state.players[0].mana_pool.green = 2;
    assert!(engine.state.players[0].restricted_mana.is_empty());
    let transaction_id = begin(&mut engine);
    let source = engine
        .state
        .pending_spell_cast
        .as_ref()
        .unwrap()
        .reserved_object_id;
    assert_eq!(
        engine
            .state
            .pending_spell_cast
            .as_ref()
            .unwrap()
            .locked_total_cost,
        "{6}{G}{G}"
    );
    let activation = activate_ability_for(&engine, servant, 0, vec![]);
    engine.apply_command(0, &activation).unwrap();
    assert_eq!(engine.state.objects[&servant].zone, Zone::Graveyard);
    let group = engine.state.players[0].restricted_mana[0].restriction_group_id;
    assert_eq!(engine.state.players[0].restricted_mana[0].amount.c, 2);
    assert_eq!(
        engine
            .state
            .pending_spell_cast
            .as_ref()
            .unwrap()
            .locked_total_cost,
        "{6}{G}{G}"
    );

    let partial = engine.preview_payment(
        0,
        &PreviewPayment {
            transaction_id,
            revision: 1,
            commit_spell_cast: Some(CommitSpellCast {
                transaction_id,
                restricted_mana: vec![ManaSpendSelection {
                    restriction_group_id: group,
                    c: 1,
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        },
    );
    assert!(
        partial.valid && !partial.complete && !partial.selection_changed,
        "{partial:?}"
    );
    assert_eq!(partial.total_cost, "{6}{G}{G}");
    assert_eq!(partial.remaining_cost, "{5}{G}{G}");
    assert_eq!(partial.restricted_mana[0].c, 1);

    let commit = CommitSpellCast {
        transaction_id,
        payment: Some(PaymentSelection {
            mana: Some(PaymentMana {
                c: 4,
                g: 2,
                ..Default::default()
            }),
            ..Default::default()
        }),
        restricted_mana: vec![ManaSpendSelection {
            restriction_group_id: group,
            c: 2,
            ..Default::default()
        }],
    };
    let complete = engine.preview_payment(
        0,
        &PreviewPayment {
            transaction_id,
            revision: 2,
            commit_spell_cast: Some(commit.clone()),
            ..Default::default()
        },
    );
    assert!(
        complete.valid && complete.complete && !complete.selection_changed,
        "{complete:?}"
    );
    assert_eq!(complete.total_cost, "{6}{G}{G}");
    assert_eq!(complete.restricted_mana[0].c, 2);
    let mut invalid = commit.clone();
    invalid.restricted_mana[0].restriction_group_id = group + 100;
    let before = format!("{:?}", engine.state);
    engine
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::CommitSpellCast(invalid)),
            },
        )
        .unwrap_err();
    assert_eq!(format!("{:?}", engine.state), before);
    engine
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::CommitSpellCast(CommitSpellCast {
                    payment: complete.selection,
                    restricted_mana: complete.restricted_mana,
                    ..commit
                })),
            },
        )
        .unwrap();
    assert_eq!(engine.state.players[0].mana_pool.colorless, 0);
    assert_eq!(engine.state.players[0].mana_pool.green, 0);
    assert!(engine.state.players[0].restricted_mana.is_empty());
    for _ in 0..3 {
        let actor = engine.state.priority_player_id();
        engine.apply_command(actor, &pass()).unwrap();
    }
    assert_eq!(engine.state.objects[&source].zone, Zone::Battlefield);
    let characteristics = engine.characteristics(source).unwrap();
    assert_eq!(
        (characteristics.power, characteristics.toughness),
        (Some(12), Some(12))
    );
    assert!(characteristics.keywords.contains(&Keyword::Trample));
    assert!(characteristics.keywords.contains(&Keyword::Haste));
}
