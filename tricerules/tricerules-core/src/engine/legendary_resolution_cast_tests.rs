//! Prepared synthetic Kaya-shaped operation; no whole-card or normal-start claim.
use super::*;

const SOURCE: &str = r#"(id:"legendary_offer_source",name:"Legendary Offer Source",face_id:"legendary_offer_source",
    supertypes:["Legendary"],types:["Creature"],power:0,toughness:5,
    activated_abilities:[(ability_id:"offer",presentation:Fallback,costs:[],
        effect:[CastLegendarySpellFromHandGraveyardOrOwnedExile,GainLife(amount:2)])])"#;
const SPELL: &str = r#"(id:"legendary_offer_spell",name:"Legendary Offer Spell",face_id:"legendary_offer_spell",
    supertypes:["Legendary"],types:["Sorcery"],mana_cost:"{10}",spell_effect:[GainLife(amount:3)])"#;

fn fixture() -> (GameEngine, ObjectId) {
    fixture_for(SOURCE, SPELL, &[], &[0, 1])
}
fn fixture_for(
    source: &str,
    spell: &str,
    extra: &[&str],
    players: &[PlayerId],
) -> (GameEngine, ObjectId) {
    let mut chunks = vec![source, spell];
    chunks.extend(extra);
    let registry = CardRegistry::from_chunks_and_tokens(&chunks, &[]).unwrap();
    let mut e = GameEngine::new_with_registry(
        13_070_001,
        players,
        20,
        Some(vec![
            EngineDeck {
                mainboard: vec!["legendary_offer_spell".into(); 40],
                commanders: vec![]
            };
            players.len()
        ]),
        true,
        Box::leak(Box::new(registry)),
    )
    .unwrap();
    e.state.turn_step = TurnStep::Upkeep;
    let source = e.state.next_object_id;
    e.state.next_object_id += 1;
    let object = new_object_from_card(
        source,
        0,
        "legendary_offer_source",
        Zone::Battlefield,
        e.registry
            .get("legendary_offer_source")
            .unwrap()
            .primary_face(),
    );
    e.state.objects.insert(source, object);
    e.state.zone_change_generation.insert(source, 0);
    e.state.players[0].battlefield.push(source);
    e.reconcile_activated_ability_slots();
    (e, source)
}
fn command(e: &mut GameEngine, actor: PlayerId, cmd: rv1::ruled_command::Cmd) -> RuledEventBatch {
    e.apply_command(actor, &RuledCommand { cmd: Some(cmd) })
        .unwrap()
}
fn resolve(e: &mut GameEngine) {
    for _ in 0..e.state.players.iter().filter(|p| !p.has_lost).count() {
        let actor = e.state.priority_player_id();
        command(
            e,
            actor,
            rv1::ruled_command::Cmd::PassPriority(rv1::PassPriority {}),
        );
    }
}
fn offer(e: &mut GameEngine, source: ObjectId) {
    command(
        e,
        0,
        rv1::ruled_command::Cmd::ActivateAbility(rv1::ActivateAbility {
            source_object_id: source,
            expected_zone_change_generation: 0,
            ..Default::default()
        }),
    );
    resolve(e);
    assert!(e.state.pending_resolution.is_some());
}
fn begin_command(e: &GameEngine, oid: ObjectId, zone: Zone) -> rv1::ruled_command::Cmd {
    let location = match zone {
        Zone::Hand => rv1::cast_source::Location::HandIndex(
            e.state.players[0]
                .hand
                .iter()
                .position(|id| *id == oid)
                .unwrap() as u32,
        ),
        Zone::Graveyard => rv1::cast_source::Location::GraveyardObjectId(oid),
        Zone::Exile => rv1::cast_source::Location::ExileObjectId(oid),
        _ => unreachable!(),
    };
    let prompt = e
        .state
        .pending_resolution
        .as_ref()
        .unwrap()
        .resolution_prompt_id
        .get();
    rv1::ruled_command::Cmd::SubmitResolutionChoice(rv1::SubmitResolutionChoice {
        resolution_prompt_id: prompt,
        decision: rv1::ResolutionChoiceDecision::CastSpell as i32,
        spell_cast_announcement: Some(rv1::SpellCastAnnouncement {
            source: Some(rv1::CastSource {
                location: Some(location),
                expected_zone_change_generation: Some(
                    e.state
                        .zone_change_generation
                        .get(&oid)
                        .copied()
                        .unwrap_or(0),
                ),
            }),
            cast_method: rv1::CastMethod::WithoutManaCost as i32,
            ..Default::default()
        }),
        ..Default::default()
    })
}
fn begin(e: &mut GameEngine, oid: ObjectId, zone: Zone) {
    let cmd = begin_command(e, oid, zone);
    command(e, 0, cmd);
}

#[test]
fn kaya_resolution_cast_preserves_each_original_zone_on_cancel_and_commits_one_child() {
    for zone in [Zone::Hand, Zone::Graveyard, Zone::Exile] {
        let (mut e, source) = fixture();
        let oid = e.state.players[0].hand[0];
        if zone != Zone::Hand {
            resolution::move_object_to_zone(&mut e.state, e.registry, oid, zone, None).unwrap();
        }
        let generation = e
            .state
            .zone_change_generation
            .get(&oid)
            .copied()
            .unwrap_or(0);
        let order = match zone {
            Zone::Hand => e.state.players[0].hand.clone(),
            Zone::Graveyard => e.state.players[0].graveyard.clone(),
            _ => e.state.players[0].exile.clone(),
        };
        offer(&mut e, source);
        let prompt = e
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .resolution_prompt_id
            .get();
        begin(&mut e, oid, zone);
        assert_eq!(e.state.objects[&oid].zone, Zone::Stack);
        assert!(e.state.stack.is_empty());
        assert!(e.state.turn_history.current.spell_casts.is_empty());
        assert_eq!(e.state.players[0].life, 20);
        let tx = e.state.pending_spell_cast.as_ref().unwrap().transaction_id;
        command(
            &mut e,
            0,
            rv1::ruled_command::Cmd::CancelSpellCast(rv1::CancelSpellCast { transaction_id: tx }),
        );
        assert_eq!(e.state.objects[&oid].zone, zone);
        assert_eq!(
            e.state
                .zone_change_generation
                .get(&oid)
                .copied()
                .unwrap_or(0),
            generation
        );
        assert_eq!(
            match zone {
                Zone::Hand => &e.state.players[0].hand,
                Zone::Graveyard => &e.state.players[0].graveyard,
                _ => &e.state.players[0].exile,
            },
            &order
        );
        assert_ne!(
            e.state
                .pending_resolution
                .as_ref()
                .unwrap()
                .resolution_prompt_id
                .get(),
            prompt
        );
        begin(&mut e, oid, zone);
        let tx = e.state.pending_spell_cast.as_ref().unwrap().transaction_id;
        command(
            &mut e,
            0,
            rv1::ruled_command::Cmd::CommitSpellCast(rv1::CommitSpellCast {
                transaction_id: tx,
                ..Default::default()
            }),
        );
        assert!(e.state.pending_resolution.is_none());
        assert!(e.state.pending_spell_cast.is_none());
        assert_eq!(
            e.state.players[0].life, 22,
            "parent tail finished before priority"
        );
        assert_eq!(e.state.stack.len(), 1);
        assert_eq!(e.state.stack[0].id, oid);
        assert_eq!(e.state.turn_history.current.spell_casts.len(), 1);
        assert_eq!(e.state.turn_history.current.spell_casts[0].origin, zone);
        resolve(&mut e);
        assert_eq!(e.state.players[0].life, 25);
    }
}

fn reject(e: &mut GameEngine, actor: PlayerId, cmd: rv1::ruled_command::Cmd) {
    let before = e.diagnostic_snapshot().unwrap();
    assert!(e
        .apply_command(actor, &RuledCommand { cmd: Some(cmd) })
        .is_err());
    assert_eq!(
        e.diagnostic_snapshot().unwrap(),
        before,
        "rejection is atomic"
    );
}
fn announcement(cmd: &mut rv1::ruled_command::Cmd) -> &mut rv1::SpellCastAnnouncement {
    let rv1::ruled_command::Cmd::SubmitResolutionChoice(answer) = cmd else {
        panic!("choice")
    };
    answer.spell_cast_announcement.as_mut().unwrap()
}
fn move_to(e: &mut GameEngine, oid: ObjectId, zone: Zone) {
    resolution::move_object_to_zone(&mut e.state, e.registry, oid, zone, None).unwrap();
}
fn decline(e: &mut GameEngine) {
    let prompt = e
        .state
        .pending_resolution
        .as_ref()
        .unwrap()
        .resolution_prompt_id
        .get();
    command(
        e,
        0,
        rv1::ruled_command::Cmd::SubmitResolutionChoice(rv1::SubmitResolutionChoice {
            resolution_prompt_id: prompt,
            decision: rv1::ResolutionChoiceDecision::Decline as i32,
            ..Default::default()
        }),
    );
}
fn commit(e: &mut GameEngine) -> rv1::ruled_command::Cmd {
    rv1::ruled_command::Cmd::CommitSpellCast(rv1::CommitSpellCast {
        transaction_id: e.state.pending_spell_cast.as_ref().unwrap().transaction_id,
        ..Default::default()
    })
}

#[test]
fn kaya_resolution_cast_rejects_foreign_stale_and_alternative_announcements_atomically() {
    for zone in [Zone::Hand, Zone::Graveyard, Zone::Exile] {
        let (mut e, source) = fixture();
        let oid = e.state.players[0].hand[0];
        if zone != Zone::Hand {
            move_to(&mut e, oid, zone);
        }
        offer(&mut e, source);
        let valid = begin_command(&e, oid, zone);
        reject(&mut e, 1, valid.clone());
        let mut stale = valid.clone();
        announcement(&mut stale)
            .source
            .as_mut()
            .unwrap()
            .expected_zone_change_generation = Some(99);
        reject(&mut e, 0, stale);
        for method in [
            rv1::CastMethod::Normal,
            rv1::CastMethod::Flashback,
            rv1::CastMethod::Permission,
        ] {
            let mut bad = valid.clone();
            announcement(&mut bad).cast_method = method as i32;
            reject(&mut e, 0, bad);
        }
        let mut bad = valid.clone();
        announcement(&mut bad).casting_permission_id = Some(42);
        reject(&mut e, 0, bad);
        let mut bad = valid.clone();
        announcement(&mut bad).prototyped = true;
        reject(&mut e, 0, bad);
        let mut bad = valid.clone();
        announcement(&mut bad).x_value = 2;
        reject(&mut e, 0, bad);
        let mut bad = valid.clone();
        let rv1::ruled_command::Cmd::SubmitResolutionChoice(answer) = &mut bad else {
            unreachable!()
        };
        answer.chosen_object_ids = vec![oid];
        reject(&mut e, 0, bad);
        let mut bad = valid.clone();
        let rv1::ruled_command::Cmd::SubmitResolutionChoice(answer) = &mut bad else {
            unreachable!()
        };
        answer.chosen_player_ids = vec![1];
        reject(&mut e, 0, bad);
        decline(&mut e);
        assert_eq!(e.state.objects[&oid].zone, zone);
        assert_eq!(e.state.players[0].life, 22);
        reject(&mut e, 0, valid);
    }
}

#[test]
fn kaya_resolution_cast_candidates_preserve_mixed_zones_and_hide_nonlegendary_and_land_faces() {
    let (mut e, source) = fixture();
    let hand = e.state.players[0].hand[0];
    let grave = e.state.players[0].hand[1];
    let exile = e.state.players[0].hand[2];
    move_to(&mut e, grave, Zone::Graveyard);
    move_to(&mut e, exile, Zone::Exile);
    let foreign = e.state.players[1].hand[0];
    move_to(&mut e, foreign, Zone::Exile);
    offer(&mut e, source);
    let event = e.legendary_resolution_choice_event().unwrap();
    let Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(choice)) = event.ev else {
        panic!("choice")
    };
    for (oid, zone) in [
        (hand, rv1::ChoiceCandidateSourceZone::Hand),
        (grave, rv1::ChoiceCandidateSourceZone::Graveyard),
        (exile, rv1::ChoiceCandidateSourceZone::Exile),
    ] {
        let index = choice
            .candidate_object_ids
            .iter()
            .position(|id| *id == oid)
            .unwrap();
        assert_eq!(choice.candidate_source_zones[index], zone as i32);
    }
    assert!(!choice.candidate_object_ids.contains(&foreign));
    let mut unique = choice.candidate_object_ids.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(unique.len(), choice.candidate_object_ids.len());
    assert!(e.initial_response_batch().legal_by_player[&1]
        .hand_actions
        .is_empty());
    assert!(e.initial_response_batch().legal_by_player[&1]
        .zone_cast_actions
        .is_empty());
    assert!(e.initial_response_batch().legal_by_player[&0]
        .hand_actions
        .iter()
        .all(|a| a.cast_method == 9));
    for invalid in [
        SPELL.replace("supertypes:[\"Legendary\"]", "supertypes:[]"),
        SPELL.replace(
            "types:[\"Sorcery\"],mana_cost:\"{10}\",spell_effect:[GainLife(amount:3)]",
            "types:[\"Land\"]",
        ),
    ] {
        let (mut e, source) = fixture_for(SOURCE, &invalid, &[], &[0, 1]);
        command(
            &mut e,
            0,
            rv1::ruled_command::Cmd::ActivateAbility(rv1::ActivateAbility {
                source_object_id: source,
                ..Default::default()
            }),
        );
        resolve(&mut e);
        assert!(e.state.pending_resolution.is_some());
        assert!(legal_actions::legendary_resolution_candidates(&e, 0).is_empty());
        decline(&mut e);
        assert!(e.state.pending_resolution.is_none());
        assert_eq!(e.state.players[0].life, 22);
    }
}

#[test]
fn kaya_resolution_cast_revalidates_legendary_sorcery_and_narset_restrictions_at_commit() {
    for remove_qualifier in [false, true] {
        let (mut e, source) = fixture();
        let oid = e.state.players[0].hand[0];
        offer(&mut e, source);
        begin(&mut e, oid, Zone::Hand);
        if remove_qualifier {
            move_to(&mut e, source, Zone::Graveyard);
        } else {
            e.state.static_emblems.push(StaticEmblemInstance {
                object_id: 9999,
                controller: 1,
                emblem_id: "narset".into(),
                display_name: "Narset Emblem".into(),
                effects: vec![StaticEmblemEffect::OpponentsCantCastNoncreatureSpells],
                triggered_ability: None,
            });
        }
        let cmd = commit(&mut e);
        reject(&mut e, 0, cmd);
        let tx = e.state.pending_spell_cast.as_ref().unwrap().transaction_id;
        command(
            &mut e,
            0,
            rv1::ruled_command::Cmd::CancelSpellCast(rv1::CancelSpellCast { transaction_id: tx }),
        );
        assert_eq!(e.state.objects[&oid].zone, Zone::Hand);
        assert!(e.state.pending_resolution.is_some());
        decline(&mut e);
    }
}

#[test]
fn kaya_resolution_cast_from_another_holders_owned_exile_restores_that_receipt() {
    let (mut e, source) = fixture();
    let oid = e.state.players[0].hand[0];
    move_to(&mut e, oid, Zone::Exile);
    e.state.players[0].exile.retain(|id| *id != oid);
    e.state.players[1].exile.push(oid);
    offer(&mut e, source);
    begin(&mut e, oid, Zone::Exile);
    assert_eq!(
        e.state.pending_spell_cast.as_ref().unwrap().original_holder,
        1
    );
    let tx = e.state.pending_spell_cast.as_ref().unwrap().transaction_id;
    command(
        &mut e,
        0,
        rv1::ruled_command::Cmd::CancelSpellCast(rv1::CancelSpellCast { transaction_id: tx }),
    );
    assert!(e.state.players[1].exile.contains(&oid));
    assert!(!e.state.players[0].exile.contains(&oid));
    begin(&mut e, oid, Zone::Exile);
    let cmd = commit(&mut e);
    command(&mut e, 0, cmd);
    assert_eq!(e.state.stack[0].id, oid);
}

#[test]
fn kaya_resolution_created_emblems_offer_separate_controller_upkeep_casts_after_source_leaves() {
    let source_definition=SOURCE.replace("CastLegendarySpellFromHandGraveyardOrOwnedExile,GainLife(amount:2)",r#"CreateTriggeredEmblem(
        emblem_id:"kaya_test",display_name:"Kaya Test Emblem",ability:(ability_id:"upkeep",presentation:Fallback,
        trigger:AtBeginningOfUpkeep(player:Controller),effect:[CastLegendarySpellFromHandGraveyardOrOwnedExile]))"#);
    let creature = SPELL.replace(
        "types:[\"Sorcery\"]",
        "types:[\"Creature\"],power:1,toughness:1",
    );
    let (mut e, source) = fixture_for(&source_definition, &creature, &[], &[0, 1]);
    for _ in 0..2 {
        command(
            &mut e,
            0,
            rv1::ruled_command::Cmd::ActivateAbility(rv1::ActivateAbility {
                source_object_id: source,
                ..Default::default()
            }),
        );
        resolve(&mut e);
    }
    assert_eq!(e.state.static_emblems.len(), 2);
    move_to(&mut e, source, Zone::Graveyard);
    let mut events = vec![];
    e.fire_triggers(
        &[GameEvent::PhaseBegan {
            phase: rv1::PhaseId::Upkeep,
            active_player: 1,
        }],
        &mut events,
    );
    e.flush_staged_triggers(&mut events);
    assert!(e.state.stack.is_empty());
    e.fire_triggers(
        &[GameEvent::PhaseBegan {
            phase: rv1::PhaseId::Upkeep,
            active_player: 0,
        }],
        &mut events,
    );
    e.flush_staged_triggers(&mut events);
    for _ in 0..2 {
        if let Some(order) = e.state.pending_trigger_order.as_ref() {
            let id = order.candidates[0].object_id;
            command(
                &mut e,
                0,
                rv1::ruled_command::Cmd::SubmitTriggerOrder(rv1::SubmitTriggerOrder {
                    trigger_object_id: id,
                }),
            );
        }
    }
    assert_eq!(e.state.stack.len(), 2);
    resolve(&mut e);
    let prompt = e
        .state
        .pending_resolution
        .as_ref()
        .unwrap()
        .resolution_prompt_id
        .get();
    let oid = e.state.players[0].hand[0];
    begin(&mut e, oid, Zone::Hand);
    let cmd = commit(&mut e);
    command(&mut e, 0, cmd);
    assert_eq!(
        e.state.stack.len(),
        2,
        "child sits above the other independent emblem trigger"
    );
    resolve(&mut e);
    assert_eq!(e.state.objects[&oid].zone, Zone::Battlefield);
    resolve(&mut e);
    assert_ne!(
        e.state
            .pending_resolution
            .as_ref()
            .unwrap()
            .resolution_prompt_id
            .get(),
        prompt
    );
    decline(&mut e);
    assert!(e.state.pending_resolution.is_none());
    assert!(e.state.stack.is_empty());
    assert_eq!(e.state.turn_history.current.spell_casts.len(), 1);
}

#[test]
fn kaya_resolution_cast_x_zero_keeps_required_additional_cost_and_target() {
    let spell=SPELL.replace("{10}","{X}{U}").replace("spell_effect:[GainLife(amount:3)]",
        "additional_costs:[DiscardCard],spell_effect:[DamageTarget(amount:1,target:(kind:Creature))]");
    let (mut e, source) = fixture_for(SOURCE, &spell, &[], &[0, 1]);
    let oid = e.state.players[0].hand[0];
    offer(&mut e, source);
    let mut cmd = begin_command(&e, oid, Zone::Hand);
    reject(&mut e, 0, cmd.clone());
    announcement(&mut cmd).targets = vec![rv1::TargetRef {
        object_id: source,
        expected_zone_change_generation: Some(0),
        kind: rv1::TargetRefKind::Permanent as i32,
        ..Default::default()
    }];
    reject(&mut e, 0, cmd.clone());
    let discard = e.state.players[0].hand[1];
    announcement(&mut cmd).cost_selections = vec![rv1::CostSelection {
        cost_index: 0,
        selection: Some(rv1::cost_selection::Selection::HandIndex(1)),
    }];
    command(&mut e, 0, cmd);
    let cmd = commit(&mut e);
    command(&mut e, 0, cmd);
    assert_eq!(e.state.objects[&discard].zone, Zone::Graveyard);
    assert_eq!(e.characteristics(oid).unwrap().mana_value, 1);
    assert_eq!(
        e.state.players[0].mana_pool,
        crate::state::ManaPool::default()
    );
}

#[test]
fn kaya_resolution_cast_needs_current_inspection_right_for_owned_face_down_exile() {
    let (mut e, source) = fixture();
    let oid = e.state.players[0].hand[0];
    move_to(&mut e, oid, Zone::Exile);
    e.state.objects.get_mut(&oid).unwrap().face_down = true;
    let generation = e.state.zone_change_generation[&oid];
    offer(&mut e, source);
    assert!(!e.legendary_cast_face_allowed(0, oid, 0));
    let bad = begin_command(&e, oid, Zone::Exile);
    reject(&mut e, 0, bad);
    let key = LinkedExileKey {
        source_object_id: source,
        source_zone_change: 0,
        producer: AbilityDefinitionId {
            card_id: "legendary_offer_source".into(),
            face_id: e
                .registry
                .get("legendary_offer_source")
                .unwrap()
                .primary_face()
                .face_id
                .clone(),
            ability_path: vec![tricerules_card_model::AbilityId::new("offer").unwrap()],
        },
        ability_link_id: tricerules_card_model::AbilityLinkId::new("look").unwrap(),
        occurrence: LinkedExileOccurrence::NativeOrTokenBase,
    };
    e.state
        .hideaway_look_permissions
        .insert((key.clone(), oid, generation + 1), BTreeSet::from([0]));
    e.state
        .hideaway_look_permissions
        .insert((key.clone(), oid, generation), BTreeSet::from([1]));
    assert!(!e.legendary_cast_face_allowed(0, oid, 0));
    e.state
        .hideaway_look_permissions
        .insert((key, oid, generation), BTreeSet::from([0]));
    assert!(e.legendary_cast_face_allowed(0, oid, 0));
    e.refresh_legendary_cast_cohort();
    let mut events = vec![e.legendary_resolution_choice_event().unwrap()];
    e.publish_resolution_prompt_ids(&mut events).unwrap();
    begin(&mut e, oid, Zone::Exile);
    assert!(!e.state.objects[&oid].face_down);
    let tx = e.state.pending_spell_cast.as_ref().unwrap().transaction_id;
    command(
        &mut e,
        0,
        rv1::ruled_command::Cmd::CancelSpellCast(rv1::CancelSpellCast { transaction_id: tx }),
    );
    assert!(e.state.objects[&oid].face_down);
    assert_eq!(e.state.zone_change_generation[&oid], generation);
}

#[test]
fn kaya_resolution_cast_begun_parent_survives_its_physical_source_owner_departure() {
    let creature = SPELL.replace(
        "types:[\"Sorcery\"]",
        "types:[\"Creature\"],power:1,toughness:1",
    );
    let (mut e, source) = fixture_for(SOURCE, &creature, &[], &[0, 1, 2]);
    e.state.objects.get_mut(&source).unwrap().owner = 2;
    let oid = e.state.players[0].hand[0];
    offer(&mut e, source);
    let parent = e
        .state
        .pending_resolution
        .as_ref()
        .unwrap()
        .continuation
        .stack()
        .unwrap()
        .item
        .id;
    command(&mut e, 2, rv1::ruled_command::Cmd::Concede(rv1::Concede {}));
    assert!(!e.state.objects.contains_key(&source));
    assert_eq!(
        e.state
            .pending_resolution
            .as_ref()
            .expect("begun parent survives")
            .continuation
            .stack()
            .unwrap()
            .item
            .id,
        parent
    );
    begin(&mut e, oid, Zone::Hand);
    let cmd = commit(&mut e);
    command(&mut e, 0, cmd);
    assert!(e.state.pending_resolution.is_none());
    assert_eq!(e.state.players[0].life, 22);
    assert_eq!(e.state.stack[0].id, oid);
}

#[test]
fn kaya_resolution_cast_controller_departure_retires_choice_and_finishes_begun_parent_once() {
    let source_definition = SOURCE.replace(
        "GainLife(amount:2)",
        "EachOpponentLosesLifeYouGainEqual(amount:2)",
    );
    for paying in [false, true] {
        let (mut e, source) = fixture_for(&source_definition, SPELL, &[], &[0, 1, 2]);
        let oid = e.state.players[0].hand[0];
        offer(&mut e, source);
        if paying {
            begin(&mut e, oid, Zone::Hand);
        }
        command(&mut e, 0, rv1::ruled_command::Cmd::Concede(rv1::Concede {}));
        assert!(e.state.pending_spell_cast.is_none());
        assert!(e.state.pending_resolution.is_none());
        assert!(!e.state.objects.contains_key(&oid));
        assert_eq!(
            e.state.players[1].life, 18,
            "begun parent tail survives departed controller"
        );
        assert_eq!(e.state.players[2].life, 18);
        assert!(e.state.stack.is_empty());
    }
}

#[test]
fn kaya_resolution_cast_hand_slot_is_bound_to_private_generation_and_reissued_after_change() {
    let (mut e, source) = fixture();
    let oid = e.state.players[0].hand[0];
    offer(&mut e, source);
    let mut old = begin_command(&e, oid, Zone::Hand);
    announcement(&mut old)
        .source
        .as_mut()
        .unwrap()
        .expected_zone_change_generation = None;
    move_to(&mut e, oid, Zone::Graveyard);
    move_to(&mut e, oid, Zone::Hand);
    reject(&mut e, 0, old.clone());
    let old_prompt = e
        .state
        .pending_resolution
        .as_ref()
        .unwrap()
        .resolution_prompt_id
        .get();
    e.refresh_legendary_cast_cohort();
    let mut events = vec![e.legendary_resolution_choice_event().unwrap()];
    e.publish_resolution_prompt_ids(&mut events).unwrap();
    assert_ne!(
        e.state
            .pending_resolution
            .as_ref()
            .unwrap()
            .resolution_prompt_id
            .get(),
        old_prompt
    );
    reject(&mut e, 0, old);
    let mut current = begin_command(&e, oid, Zone::Hand);
    announcement(&mut current)
        .source
        .as_mut()
        .unwrap()
        .expected_zone_change_generation = None;
    command(&mut e, 0, current);
    assert_eq!(
        e.state
            .pending_spell_cast
            .as_ref()
            .unwrap()
            .reserved_object_id,
        oid
    );
    let cmd = commit(&mut e);
    command(&mut e, 0, cmd);
    assert_eq!(e.state.stack[0].id, oid);
}

#[test]
fn kaya_prepared_copied_emblem_trigger_has_its_own_optional_parent_without_another_emblem() {
    // Prepared copy result only: this does not admit a trigger-copying card or claim its costs.
    let definition=SOURCE.replace("CastLegendarySpellFromHandGraveyardOrOwnedExile,GainLife(amount:2)",r#"CreateTriggeredEmblem(
        emblem_id:"kaya_test",display_name:"Kaya Test Emblem",ability:(ability_id:"upkeep",presentation:Fallback,
        trigger:AtBeginningOfUpkeep(player:Controller),effect:[CastLegendarySpellFromHandGraveyardOrOwnedExile]))"#);
    let creature = SPELL.replace(
        "types:[\"Sorcery\"]",
        "types:[\"Creature\"],power:1,toughness:1",
    );
    let (mut e, source) = fixture_for(&definition, &creature, &[], &[0, 1]);
    command(
        &mut e,
        0,
        rv1::ruled_command::Cmd::ActivateAbility(rv1::ActivateAbility {
            source_object_id: source,
            ..Default::default()
        }),
    );
    resolve(&mut e);
    let mut events = vec![];
    e.fire_triggers(
        &[GameEvent::PhaseBegan {
            phase: rv1::PhaseId::Upkeep,
            active_player: 0,
        }],
        &mut events,
    );
    e.flush_staged_triggers(&mut events);
    assert_eq!(e.state.stack.len(), 1);
    let original = e.state.stack[0].id;
    let mut copy = e.state.stack[0].clone();
    copy.id = e.state.next_object_id;
    e.state.next_object_id += 1;
    copy.is_copy = true;
    let copied_id = copy.id;
    e.state.stack.push(copy);
    resolve(&mut e);
    assert_eq!(
        e.state
            .pending_resolution
            .as_ref()
            .unwrap()
            .continuation
            .stack()
            .unwrap()
            .item
            .id,
        copied_id
    );
    let copied_prompt = e
        .state
        .pending_resolution
        .as_ref()
        .unwrap()
        .resolution_prompt_id
        .get();
    decline(&mut e);
    resolve(&mut e);
    assert_eq!(
        e.state
            .pending_resolution
            .as_ref()
            .unwrap()
            .continuation
            .stack()
            .unwrap()
            .item
            .id,
        original
    );
    assert_ne!(
        e.state
            .pending_resolution
            .as_ref()
            .unwrap()
            .resolution_prompt_id
            .get(),
        copied_prompt
    );
    let oid = e.state.players[0].hand[0];
    begin(&mut e, oid, Zone::Hand);
    let cmd = commit(&mut e);
    command(&mut e, 0, cmd);
    assert_eq!(e.state.static_emblems.len(), 1);
    assert_eq!(e.state.turn_history.current.spell_casts.len(), 1);
    assert_eq!(e.state.stack[0].id, oid);
}

#[test]
fn kaya_optional_cast_does_not_publish_hidden_hand_eligibility_by_skipping_an_empty_offer() {
    let mut public_shapes = vec![];
    for legendary in [false, true] {
        let spell = if legendary {
            SPELL.to_string()
        } else {
            SPELL.replace("supertypes:[\"Legendary\"]", "supertypes:[]")
        };
        let (mut e, source) = fixture_for(SOURCE, &spell, &[], &[0, 1]);
        command(
            &mut e,
            0,
            rv1::ruled_command::Cmd::ActivateAbility(rv1::ActivateAbility {
                source_object_id: source,
                ..Default::default()
            }),
        );
        resolve(&mut e);
        assert!(
            e.state.pending_resolution.is_some(),
            "a private empty offer must not auto-skip and reveal hidden eligibility"
        );
        assert_eq!(
            e.state.players[0].life, 20,
            "parent remains paused until decline"
        );
        let event = e.legendary_resolution_choice_event().unwrap();
        let Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(choice)) = event.ev else {
            panic!("choice")
        };
        assert_eq!(choice.candidate_object_ids.is_empty(), !legendary);
        public_shapes.push((
            choice.deciding_player_id,
            choice.source_object_id,
            choice.choice_kind,
            choice.min,
            choice.max,
            choice.ordered,
        ));
        decline(&mut e);
        assert_eq!(e.state.players[0].life, 22);
        assert!(e.state.pending_resolution.is_none());
    }
    assert_eq!(public_shapes[0], public_shapes[1]);
}

#[test]
fn kaya_public_zone_offers_have_stable_object_order_for_replay_and_private_cohorts() {
    let (mut e, source) = fixture();
    let public = e.state.players[0].hand[1..].to_vec();
    assert!(public.len() >= 6);
    for (index, id) in public.iter().enumerate() {
        move_to(
            &mut e,
            *id,
            if index % 2 == 0 {
                Zone::Exile
            } else {
                Zone::Graveyard
            },
        );
    }
    offer(&mut e, source);
    let offered: Vec<_> = e.initial_response_batch().legal_by_player[&0]
        .zone_cast_actions
        .iter()
        .map(|a| a.object_id)
        .collect();
    let mut expected = public;
    expected.sort_unstable();
    assert_eq!(
        offered, expected,
        "Public zone offer order must not depend on randomized object-map iteration"
    );
    let pending = e.state.pending_resolution.as_ref().unwrap();
    let ResolutionContinuation::LegendarySpellCast { candidates, .. } = &pending.continuation
    else {
        panic!("legendary parent")
    };
    let stored: Vec<_> = candidates
        .iter()
        .filter(|c| c.reference.source_zone != Zone::Hand)
        .map(|c| c.reference.object_id)
        .collect();
    assert_eq!(stored, expected);
}
