use super::*;

const SPELL: &str = r#"(id:"expensive_spell",name:"Expensive Spell",face_id:"expensive_spell",
    mana_cost:"{7}{U}{U}{U}",types:["Sorcery"],spell_effect:[GainLife(amount:3)])"#;

fn fixture() -> GameEngine {
    fixture_for(SPELL, &[])
}

fn fixture_for(spell: &str, extra: &[&str]) -> GameEngine {
    fixture_for_players(spell, extra, &[0, 1])
}

fn fixture_for_players(spell: &str, extra: &[&str], players: &[PlayerId]) -> GameEngine {
    let mut chunks = vec![spell];
    chunks.extend(extra);
    let registry = CardRegistry::from_chunks_and_tokens(&chunks, &[]).unwrap();
    let mut engine = GameEngine::new_with_registry(
        705_001,
        players,
        20,
        Some(vec![
            EngineDeck {
                mainboard: vec!["expensive_spell".into(); 40],
                commanders: vec![]
            };
            players.len()
        ]),
        true,
        Box::leak(Box::new(registry)),
    )
    .unwrap();
    engine.state.turn_step = TurnStep::Main1;
    engine.state.priority_idx = 0;
    engine
}

// Prepared emblem marker seam. Resolution-created markers are tested separately.
fn emblem(engine: &mut GameEngine, controller: PlayerId) {
    let object_id = engine.state.next_object_id;
    engine.state.next_object_id += 1;
    engine
        .state
        .static_emblems
        .push(crate::state::StaticEmblemInstance {
            object_id,
            controller,
            emblem_id: "tamiyo_hand_cast".into(),
            display_name: "Tamiyo Emblem".into(),
            effects: vec![StaticEmblemEffect::CastSpellsFromHandWithoutPayingManaCost],
            triggered_ability: None,
        });
}

fn cast(engine: &GameEngine, method: i32) -> RuledCommand {
    let oid = engine.state.players[0].hand[0];
    RuledCommand {
        cmd: Some(rv1::ruled_command::Cmd::CastSpell(rv1::CastSpell {
            cast_method: method,
            source: Some(rv1::CastSource {
                location: Some(rv1::cast_source::Location::HandIndex(0)),
                expected_zone_change_generation: Some(
                    engine.payment_object_ref(oid).zone_change_generation,
                ),
            }),
            ..Default::default()
        })),
    }
}

#[test]
fn tamiyo_hand_free_cast_offer_and_no_mana_cast_preserve_paid_option() {
    let mut engine = fixture();
    emblem(&mut engine, 0);
    emblem(&mut engine, 0);
    let mut batch = rv1::RuledEventBatch::default();
    legal_actions::fill_legal(&mut batch, &engine);
    let actions = &batch.legal_by_player[&0].hand_actions;
    let first: Vec<_> = actions.iter().filter(|a| a.hand_index == 0).collect();
    assert_eq!(
        first.len(),
        2,
        "paid and free choices, deduplicated across emblems"
    );
    assert!(first
        .iter()
        .any(|a| a.cast_method == rv1::CastMethod::Normal as i32));
    assert!(first.iter().any(|a| a.cast_method == 9));
    assert!(engine
        .apply_command(0, &cast(&engine, rv1::CastMethod::Normal as i32))
        .is_err());
    engine.apply_command(0, &cast(&engine, 9)).unwrap();
    assert_eq!(engine.state.stack.len(), 1);
    assert_eq!(engine.state.stack[0].card_id, "expensive_spell");
    assert_eq!(
        engine
            .characteristics(engine.state.stack[0].id)
            .unwrap()
            .mana_value,
        10
    );
    assert_eq!(engine.state.players[0].hand.len(), 6);
}

fn raw_cast(command: &mut RuledCommand) -> &mut rv1::CastSpell {
    let Some(rv1::ruled_command::Cmd::CastSpell(cast)) = command.cmd.as_mut() else {
        panic!("cast")
    };
    cast
}

fn reject_unchanged(engine: &mut GameEngine, actor: PlayerId, command: &RuledCommand) {
    let before = engine.diagnostic_snapshot().unwrap();
    assert!(engine.apply_command(actor, command).is_err());
    assert_eq!(engine.diagnostic_snapshot().unwrap(), before);
}

#[test]
fn tamiyo_hand_free_cast_requires_owner_authority_and_normal_timing() {
    let mut engine = fixture();
    let command = cast(&engine, 9);
    reject_unchanged(&mut engine, 0, &command);
    emblem(&mut engine, 1);
    reject_unchanged(&mut engine, 0, &command);
    emblem(&mut engine, 0);
    reject_unchanged(&mut engine, 1, &command);
    engine.state.turn_step = TurnStep::Upkeep;
    reject_unchanged(&mut engine, 0, &command);
    assert!(engine.initial_response_batch().legal_by_player[&0]
        .hand_actions
        .is_empty());
    engine.state.turn_step = TurnStep::Main1;
    engine.apply_command(0, &command).unwrap();
}

#[test]
fn tamiyo_hand_free_cast_x_is_zero_and_printed_mana_value_is_retained() {
    let spell = SPELL.replace("{7}{U}{U}{U}", "{X}{U}{U}");
    let mut engine = fixture_for(&spell, &[]);
    emblem(&mut engine, 0);
    let mut command = cast(&engine, 9);
    raw_cast(&mut command).x_value = 3;
    reject_unchanged(&mut engine, 0, &command);
    raw_cast(&mut command).x_value = 0;
    engine.apply_command(0, &command).unwrap();
    assert_eq!(
        engine
            .characteristics(engine.state.stack[0].id)
            .unwrap()
            .mana_value,
        2
    );
}

#[test]
fn tamiyo_hand_free_cast_rejects_other_zones_permissions_lands_and_prototype() {
    let prototype = r#"(id:"expensive_spell",name:"Expensive Spell",face_id:"expensive_spell",
        mana_cost:"{9}",types:["Artifact","Creature"],power:4,toughness:4,
        prototype:Some((mana_cost:"{3}{R}{R}",power:2,toughness:2)))"#;
    let mut engine = fixture_for(prototype, &[]);
    emblem(&mut engine, 0);
    let offers = &engine.initial_response_batch().legal_by_player[&0].hand_actions;
    assert_eq!(
        offers
            .iter()
            .filter(|a| a.hand_index == 0 && a.cast_method == 9)
            .count(),
        1
    );
    assert!(!offers.iter().any(|a| a.cast_method == 9 && a.prototyped));
    let mut command = cast(&engine, 9);
    raw_cast(&mut command).prototyped = true;
    reject_unchanged(&mut engine, 0, &command);
    raw_cast(&mut command).prototyped = false;
    raw_cast(&mut command).casting_permission_id = Some(123);
    reject_unchanged(&mut engine, 0, &command);
    raw_cast(&mut command).casting_permission_id = None;
    let oid = engine.state.players[0].hand[0];
    super::resolution::move_object_to_zone(
        &mut engine.state,
        engine.registry,
        oid,
        Zone::Graveyard,
        None,
    )
    .unwrap();
    raw_cast(&mut command).source = Some(rv1::CastSource {
        location: Some(rv1::cast_source::Location::GraveyardObjectId(oid)),
        expected_zone_change_generation: Some(
            engine.payment_object_ref(oid).zone_change_generation,
        ),
    });
    reject_unchanged(&mut engine, 0, &command);
    let land =
        r#"(id:"expensive_spell",name:"Expensive Spell",face_id:"expensive_spell",types:["Land"])"#;
    let mut engine = fixture_for(land, &[]);
    emblem(&mut engine, 0);
    let command = cast(&engine, 9);
    reject_unchanged(&mut engine, 0, &command);
    assert!(!engine.initial_response_batch().legal_by_player[&0]
        .hand_actions
        .iter()
        .any(|a| a.cast_method == 9));
}

#[test]
fn tamiyo_hand_free_cast_pending_authority_and_cancel_retry_are_atomic() {
    let mut engine = fixture();
    emblem(&mut engine, 0);
    let mut command = cast(&engine, 9);
    let announcement = super::casting::announcement_from_cast(raw_cast(&mut command));
    let begin = RuledCommand {
        cmd: Some(rv1::ruled_command::Cmd::BeginSpellCast(
            rv1::BeginSpellCast {
                announcement: Some(announcement),
            },
        )),
    };
    engine.apply_command(0, &begin).unwrap();
    let transaction_id = engine
        .state
        .pending_spell_cast
        .as_ref()
        .unwrap()
        .transaction_id;
    let commit = RuledCommand {
        cmd: Some(rv1::ruled_command::Cmd::CommitSpellCast(
            rv1::CommitSpellCast {
                transaction_id,
                ..Default::default()
            },
        )),
    };
    // Persistent emblems do not ordinarily disappear. This prepared authority-loss seam
    // proves cached offers/locked costs cannot bypass the authority check.
    let markers = std::mem::take(&mut engine.state.static_emblems);
    reject_unchanged(&mut engine, 0, &commit);
    let Some(rv1::ruled_command::Cmd::CommitSpellCast(commit_data)) = &commit.cmd else {
        unreachable!()
    };
    let preview = engine.preview_payment(
        0,
        &rv1::PreviewPayment {
            transaction_id,
            commit_spell_cast: Some(commit_data.clone()),
            ..Default::default()
        },
    );
    assert!(preview.error.contains("requires your emblem"));
    engine.state.static_emblems = markers;
    engine
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::CancelSpellCast(
                    rv1::CancelSpellCast { transaction_id },
                )),
            },
        )
        .unwrap();
    assert!(engine.state.pending_spell_cast.is_none());
    assert_eq!(engine.state.players[0].hand.len(), 7);
    engine.apply_command(0, &begin).unwrap();
    let transaction_id = engine
        .state
        .pending_spell_cast
        .as_ref()
        .unwrap()
        .transaction_id;
    engine
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::CommitSpellCast(
                    rv1::CommitSpellCast {
                        transaction_id,
                        ..Default::default()
                    },
                )),
            },
        )
        .unwrap();
    assert_eq!(engine.state.stack.len(), 1);
    assert_eq!(
        engine.state.stack[0].cast_method,
        SpellCastMethod::WithoutManaCost
    );
}

fn permanent(engine: &mut GameEngine, player: PlayerId, name: &str) -> ObjectId {
    engine
        .apply_dev_command(
            player,
            &rv1::DevCommand {
                target_player_id: player,
                dev: Some(rv1::dev_command::Dev::PutCardInZone(
                    rv1::DevPutCardInZone {
                        card_name: name.into(),
                        zone: rv1::DevZone::Battlefield as i32,
                        ready: true,
                    },
                )),
            },
        )
        .unwrap();
    engine.reconcile_activated_ability_slots();
    engine.state.next_object_id - 1
}

#[test]
fn tamiyo_hand_free_cast_preserves_targets_tax_mandatory_and_optional_costs() {
    let spell = r#"(id:"expensive_spell",name:"Expensive Spell",face_id:"expensive_spell",
        mana_cost:"{7}{U}{U}{U}",types:["Sorcery"],
        additional_costs:[SacrificePermanent(filter:(kind:Creature,controller:You))],
        cast_cost_groups:[(group_id:"multikicker",presentation:Fallback,
            options:[Mana(option_id:"kick",presentation:Fallback,kind:Multikicker,cost:"{3}")])],
        spell_effect:[DamageTarget(amount:1,target:(kind:Creature))])"#;
    let creature = r#"(id:"cost_creature",name:"Cost Creature",face_id:"cost_creature",types:["Creature"],power:1,toughness:1)"#;
    let mut engine = fixture_for(
        spell,
        &[
            creature,
            include_str!("../../../tricerules-cards/data/boreal_elemental.ron"),
        ],
    );
    let sacrifice = permanent(&mut engine, 0, "Cost Creature");
    let target = permanent(&mut engine, 1, "Boreal Elemental");
    emblem(&mut engine, 0);
    let mut command = cast(&engine, 9);
    reject_unchanged(&mut engine, 0, &command);
    raw_cast(&mut command).targets = vec![rv1::TargetRef {
        object_id: target,
        kind: rv1::TargetRefKind::Permanent as i32,
        expected_zone_change_generation: Some(
            engine.payment_object_ref(target).zone_change_generation,
        ),
        ..Default::default()
    }];
    reject_unchanged(&mut engine, 0, &command);
    raw_cast(&mut command).cost_selections = vec![rv1::CostSelection {
        cost_index: 0,
        selection: Some(rv1::cost_selection::Selection::PermanentId(sacrifice)),
    }];
    raw_cast(&mut command).cast_cost_group_selections = vec![rv1::CastCostGroupSelection {
        repetitions: Some(1),
        ..Default::default()
    }];
    // The printed ten mana is waived; the targeting tax (2) and optional kicker (3) survive.
    engine.state.players[0].mana_pool.colorless = 4;
    reject_unchanged(&mut engine, 0, &command);
    engine.state.players[0].mana_pool.colorless = 5;
    engine.apply_command(0, &command).unwrap();
    assert_eq!(engine.state.players[0].mana_pool.colorless, 0);
    assert_eq!(engine.state.objects[&sacrifice].zone, Zone::Graveyard);
    assert_eq!(
        engine
            .characteristics(engine.state.stack[0].id)
            .unwrap()
            .mana_value,
        10
    );
    assert_eq!(
        engine.state.stack[0].cast_cost_receipts[0]
            .multikicker
            .as_ref()
            .unwrap()
            .repetitions,
        1
    );
}

fn resolve_one(engine: &mut GameEngine) {
    for _ in 0..engine.state.players.iter().filter(|p| !p.has_lost).count() {
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
fn tamiyo_hand_free_cast_retains_narset_rebound_hand_origin_receipt() {
    let mut engine = fixture();
    emblem(&mut engine, 0);
    // Prepared observer registration seam; the subsequent spell is an actual hand cast.
    let source = StackItem::entry_context(
        900_000,
        0,
        "expensive_spell".into(),
        "prepared rebound source",
    );
    engine.register_next_hand_spell_rebound(&source, 0, "Prepared Rebound");
    let oid = engine.state.players[0].hand[0];
    engine.apply_command(0, &cast(&engine, 9)).unwrap();
    let occurrence = engine.state.stack[0].cast_occurrence.unwrap();
    let key = (oid, occurrence.zone_change_generation);
    assert_eq!(engine.state.hand_cast_holders.get(&key), Some(&0));
    assert_eq!(
        engine.state.stack.len(),
        2,
        "ordinary cast stages the rebound grant"
    );
    resolve_one(&mut engine);
    assert!(engine.state.rebound_spell_grants.contains(&key));
    resolve_one(&mut engine);
    assert_eq!(engine.state.objects[&oid].zone, Zone::Exile);
}

#[test]
fn tamiyo_hand_free_cast_draw_replacements_finish_before_one_persistent_emblem() {
    let source = r#"(id:"synthetic_tamiyo_emblem",name:"Synthetic Tamiyo Emblem",face_id:"synthetic_tamiyo_emblem",
        types:["Planeswalker"],loyalty:7,
        activated_abilities:[(ability_id:"ultimate",presentation:Fallback,costs:[Loyalty(-7)],
            effect:[Draw(who:Controller,count:3),CreateStaticEmblem(emblem_id:"tamiyo_hand_cast",display_name:"Tamiyo Emblem",
                effects:[CastSpellsFromHandWithoutPayingManaCost])])])"#;
    let mut engine = fixture_for_players(
        SPELL,
        &[
            source,
            include_str!("../../../tricerules-cards/data/abundance.ron"),
        ],
        &[0, 1, 2],
    );
    let source = permanent(&mut engine, 0, "Synthetic Tamiyo Emblem");
    permanent(&mut engine, 0, "Abundance");
    let initial_hand = engine.state.players[0].hand.len();
    engine
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::ActivateAbility(
                    rv1::ActivateAbility {
                        source_object_id: source,
                        ability_index: 0,
                        expected_zone_change_generation: engine
                            .payment_object_ref(source)
                            .zone_change_generation,
                        ..Default::default()
                    },
                )),
            },
        )
        .unwrap();
    assert_eq!(engine.state.objects[&source].zone, Zone::Graveyard);
    resolve_one(&mut engine);
    for completed in 1..=3 {
        assert!(engine.state.static_emblems.is_empty());
        let pending = engine.state.pending_resolution.as_ref().unwrap();
        let answer = RuledCommand {
            cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(
                rv1::SubmitResolutionChoice {
                    resolution_prompt_id: pending.resolution_prompt_id.get(),
                    decision: rv1::ResolutionChoiceDecision::SelectBranch as i32,
                    selected_branch_index: 1,
                    ..Default::default()
                },
            )),
        };
        engine.apply_command(0, &answer).unwrap();
        assert_eq!(engine.state.players[0].hand.len(), initial_hand + completed);
    }
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(engine.state.static_emblems.len(), 1);
    assert!(engine.hand_without_mana_cost_permitted(0));
    assert!(!engine.hand_without_mana_cost_permitted(1));
    // Source is already gone; controller departure retires the persistent permission.
    engine
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::Concede(rv1::Concede {})),
            },
        )
        .unwrap();
    assert!(!engine.hand_without_mana_cost_permitted(0));
    assert!(engine.state.static_emblems.is_empty());
}

#[test]
fn tamiyo_hand_free_cast_keeps_mode_selection_and_instant_timing() {
    let spell = r#"(id:"expensive_spell",name:"Expensive Spell",face_id:"expensive_spell",
        mana_cost:"{7}{U}{U}{U}",types:["Instant"],
        modal_spell:(min_modes:1,max_modes:1,modes:[
            (mode_id:"gain",presentation:Fallback,effects:[GainLife(amount:3)]),
            (mode_id:"draw",presentation:Fallback,effects:[Draw(count:1)])]))"#;
    let mut engine = fixture_for(spell, &[]);
    emblem(&mut engine, 0);
    engine.state.turn_step = TurnStep::Upkeep;
    let offers = engine.initial_response_batch();
    let free = offers.legal_by_player[&0]
        .hand_actions
        .iter()
        .find(|a| a.hand_index == 0 && a.cast_method == 9)
        .unwrap();
    assert_eq!(
        (free.min_modes, free.max_modes, free.modes.len()),
        (1, 1, 2)
    );
    let mut command = cast(&engine, 9);
    reject_unchanged(&mut engine, 0, &command);
    raw_cast(&mut command).selected_modes = vec![rv1::SelectedSpellMode {
        mode_index: 0,
        ..Default::default()
    }];
    engine.apply_command(0, &command).unwrap();
    assert_eq!(engine.state.stack[0].chosen_modes.len(), 1);
    resolve_one(&mut engine);
    assert_eq!(engine.state.players[0].life, 23);
}
