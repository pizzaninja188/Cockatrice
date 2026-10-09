//! Triplicate Titan: exact Golem token identities and death controller.
use super::helpers::*;
use tricerules_cards::Keyword;
use tricerules_core::Zone;

fn game(seed: u64) -> GameEngine {
    let deck = deck_with("forest", &[]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1],
        20,
        Some(vec![deck; 2]),
        true,
    )
    .unwrap();
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn cast_creature(engine: &mut GameEngine, card: &str, cost: u32) -> u32 {
    assert!(
        tricerules_cards::registry::global().get(card).is_some(),
        "missing exact {card}"
    );
    let source = inject_card_into_hand(engine, 0, card);
    give_mana(
        engine,
        0,
        ManaGift {
            c: cost,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(engine, 0, card);
    engine.apply_command(0, &cast_spell(slot, vec![])).unwrap();
    pass_priority_round(engine);
    assert_eq!(engine.state.objects[&source].zone, Zone::Battlefield);
    assert_eq!(engine.state.players[0].mana_pool.colorless, 0);
    source
}

#[test]
fn titan_actual_cast_has_exact_nine_nine_and_three_keywords() {
    let mut engine = game(2026100110);
    let source = cast_creature(&mut engine, "triplicate_titan", 9);
    let stats = engine.characteristics(source).unwrap();
    assert_eq!((stats.power, stats.toughness), (Some(9), Some(9)));
    assert!(stats.is_artifact() && stats.is_creature());
    for keyword in [Keyword::Flying, Keyword::Vigilance, Keyword::Trample] {
        assert!(stats.keywords.contains(&keyword));
    }
}

fn resolve_one(engine: &mut GameEngine) -> RuledEventBatch {
    let mut result = RuledEventBatch::default();
    let count = engine.state.players.iter().filter(|p| !p.has_lost).count()
        - engine.state.passes_since_stack_change as usize;
    for _ in 0..count {
        let actor = engine.state.priority_player_id();
        result = engine.apply_command(actor, &pass()).unwrap();
    }
    result
}

#[test]
fn titan_real_death_creates_three_distinct_exact_tokens_for_death_controller() {
    let deck = deck_with("forest", &[]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        2026100112,
        &[10, 20, 30],
        20,
        Some(vec![deck; 3]),
        true,
    )
    .unwrap();
    advance_to_main1_from_game_start(&mut engine);
    // Fixture: a Titan owned by 10 is currently controlled by 20. Death is a real spell.
    let source = inject_permanent_on_battlefield(&mut engine, 1, "triplicate_titan");
    engine.state.objects.get_mut(&source).unwrap().owner = 10;
    let spell = inject_card_into_hand(&mut engine, 0, "murder");
    give_mana(
        &mut engine,
        10,
        ManaGift {
            b: 2,
            c: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "murder");
    engine
        .apply_command(10, &cast_spell(slot, target_object(source)))
        .unwrap();
    resolve_one(&mut engine);
    assert_eq!(engine.state.objects[&source].zone, Zone::Graveyard);
    assert!(engine.state.players[0].graveyard.contains(&source));
    assert_eq!(
        engine.state.stack.len(),
        1,
        "the mandatory death trigger survived its source"
    );
    let mut batch = resolve_one(&mut engine);
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("one token timestamp order");
    assert_eq!(
        pending.presentation.choice_kind,
        ChoiceKind::SimultaneousEntryOrder
    );
    let order = pending.presentation.candidates.clone();
    batch.events.extend(
        engine
            .apply_command(20, &submit_resolution_choice(order))
            .unwrap()
            .events,
    );
    let tokens = token_created_events(&batch);
    assert_eq!(tokens.len(), 3);
    let mut ids = std::collections::BTreeSet::new();
    for (card, keyword, display) in [
        ("golem_c_3_3_flying", Keyword::Flying, "Flying"),
        ("golem_c_3_3_vigilance", Keyword::Vigilance, "Vigilance"),
        ("golem_c_3_3_trample", Keyword::Trample, "Trample"),
    ] {
        let created = tokens
            .iter()
            .find(|t| t.card_id == card)
            .expect("distinct token definition");
        assert!(ids.insert(created.object_id));
        assert_eq!(created.controller_player_id, 20);
        assert!(!created.enters_tapped);
        let identity = created.identity.as_ref().unwrap();
        assert_eq!(identity.name, "Golem");
        assert_eq!(identity.pt, "3/3");
        assert_eq!(identity.color, "");
        assert_eq!(identity.types, ["Artifact", "Creature", "Golem"]);
        assert!(identity.is_creature);
        assert_eq!(identity.keywords, [display]);
        assert!(identity.ability_texts.is_empty());
        let object = &engine.state.objects[&created.object_id];
        assert!(object.is_token());
        assert_eq!(
            (object.owner, object.controller, object.zone),
            (20, 20, Zone::Battlefield)
        );
        let stats = engine.characteristics(created.object_id).unwrap();
        assert_eq!((stats.power, stats.toughness), (Some(3), Some(3)));
        assert!(stats.colors.is_empty() && stats.is_artifact() && stats.is_creature());
        assert!(stats.types.iter().any(|t| t == "Golem"));
        assert_eq!(stats.keywords.len(), 1);
        assert!(stats.keywords.contains(&keyword));
    }
    assert_eq!(engine.state.objects[&spell].zone, Zone::Graveyard);
    assert_eq!(engine.state.players[1].battlefield.len(), 3);
    assert!(engine.state.players[0].battlefield.is_empty());
    assert!(engine.state.players[2].battlefield.is_empty());
    assert!(engine.state.stack.is_empty());
}

#[test]
fn titan_bounce_exile_and_suppressed_death_do_not_create_tokens() {
    for (spell, destination, suppress) in [
        ("unsummon", Zone::Hand, false),
        ("swords_to_plowshares", Zone::Exile, false),
        ("murder", Zone::Graveyard, true),
    ] {
        let mut engine = game(2026100113);
        let source = inject_permanent_on_battlefield(&mut engine, 0, "triplicate_titan");
        if suppress {
            engine
                .state
                .continuous_effects
                .push(tricerules_core::ContinuousEffect {
                    trigger_grant_origin: None,
                    source_id: None,
                    affected: tricerules_core::AffectedScope::Single(source),
                    kind: tricerules_cards::ContinuousEffectKind::Layer6RemoveAllAbilities,
                    condition: None,
                    duration: tricerules_cards::EffectDuration::Indefinite,
                    timestamp: 1,
                });
        }
        inject_card_into_hand(&mut engine, 0, spell);
        grant_pool(&mut engine, 0);
        let slot = hand_index_for_card(&engine, 0, spell);
        engine
            .apply_command(0, &cast_spell(slot, target_object(source)))
            .unwrap();
        let batch = resolve_one(&mut engine);
        assert_eq!(engine.state.objects[&source].zone, destination);
        assert!(token_created_events(&batch).is_empty());
        assert!(engine.state.stack.is_empty());
        assert!(!engine
            .state
            .objects
            .values()
            .any(|object| object.is_token()));
    }
}

#[test]
fn titan_heterogeneous_tokens_enter_as_one_simultaneous_cohort() {
    let mut engine = game(2026100117);
    let source = inject_permanent_on_battlefield(&mut engine, 0, "triplicate_titan");
    // Fixture: every creature has Soul Warden's printed entry observer. Each of the three
    // simultaneous entrants must observe the other two entries: six triggers, not 0+1+2.
    let observer = tricerules_cards::registry::global()
        .get("soul_warden")
        .unwrap()
        .primary_face()
        .triggered_abilities[0]
        .clone();
    engine
        .state
        .add_triggered_ability_grant(tricerules_core::ContinuousEffect {
            trigger_grant_origin: None,
            source_id: None,
            affected: tricerules_core::AffectedScope::AllCreatures,
            kind: tricerules_cards::ContinuousEffectKind::GrantTriggeredAbility(Box::new(observer)),
            condition: None,
            duration: tricerules_cards::EffectDuration::Indefinite,
            timestamp: 1,
        });
    inject_card_into_hand(&mut engine, 0, "murder");
    grant_pool(&mut engine, 0);
    let slot = hand_index_for_card(&engine, 0, "murder");
    engine
        .apply_command(0, &cast_spell(slot, target_object(source)))
        .unwrap();
    resolve_one(&mut engine);
    resolve_one(&mut engine);
    answer_simultaneous_entry_order_in_engine_order(&mut engine);
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        engine.state.players[0].life, 26,
        "all three token observers see the other two simultaneous entries and gain six life"
    );
}

#[test]
fn titan_replacements_park_the_complete_cohort_and_order_once() {
    let mut engine = game(2026100118);
    inject_permanent_on_battlefield(&mut engine, 0, "orb_of_dreams");
    inject_permanent_on_battlefield(&mut engine, 0, "orb_of_dreams");
    let source = inject_permanent_on_battlefield(&mut engine, 0, "triplicate_titan");
    inject_card_into_hand(&mut engine, 0, "murder");
    grant_pool(&mut engine, 0);
    let slot = hand_index_for_card(&engine, 0, "murder");
    engine
        .apply_command(0, &cast_spell(slot, target_object(source)))
        .unwrap();
    resolve_one(&mut engine);
    let mut batch = resolve_one(&mut engine);
    for _ in 0..3 {
        assert_eq!(engine.state.objects[&source].zone, Zone::Graveyard);
        assert!(token_created_events(&batch).is_empty());
        assert!(!engine
            .state
            .objects
            .values()
            .any(|o| o.is_token() && o.zone == Zone::Battlefield));
        let pending = engine.state.pending_resolution.as_ref().unwrap();
        assert_eq!(
            pending.presentation.choice_kind,
            ChoiceKind::ReplacementEffect
        );
        assert_eq!(pending.presentation.candidates.len(), 2);
        let application = pending.presentation.candidates[0];
        let before = format!("{:?}", engine.state);
        assert!(engine
            .apply_command(1, &submit_resolution_choice(vec![application]))
            .is_err());
        assert_eq!(format!("{:?}", engine.state), before);
        batch = engine
            .apply_command(0, &submit_resolution_choice(vec![application]))
            .unwrap();
    }
    assert!(token_created_events(&batch).is_empty());
    assert!(!engine
        .state
        .objects
        .values()
        .any(|o| o.is_token() && o.zone == Zone::Battlefield));
    let pending = engine.state.pending_resolution.as_ref().unwrap();
    assert_eq!(
        pending.presentation.choice_kind,
        ChoiceKind::SimultaneousEntryOrder
    );
    assert_eq!((pending.presentation.min, pending.presentation.max), (3, 3));
    let mut order = pending.presentation.candidates.clone();
    assert_eq!(order.len(), 3);
    let choice = batch
        .events
        .iter()
        .find_map(|event| match &event.ev {
            Some(tricerules_proto::ruled::v1::ruled_event::Ev::ResolutionChoiceRequired(
                choice,
            )) if choice.choice_kind == ChoiceKind::SimultaneousEntryOrder as i32 => Some(choice),
            _ => None,
        })
        .expect("private proposed token identity");
    assert_eq!(choice.candidate_object_ids, order);
    assert_eq!(choice.candidate_names, ["Golem", "Golem", "Golem"]);
    assert_eq!(choice.candidate_token_identities.len(), 3);
    for (identity, keyword) in
        choice
            .candidate_token_identities
            .iter()
            .zip(["Flying", "Vigilance", "Trample"])
    {
        assert_eq!(identity.name, "Golem");
        assert_eq!(identity.pt, "3/3");
        assert_eq!(identity.color, "");
        assert_eq!(identity.types, ["Artifact", "Creature", "Golem"]);
        assert!(identity.is_creature);
        assert_eq!(identity.keywords, [keyword]);
    }
    let original_generation = engine
        .state
        .zone_change_generation
        .get(&order[0])
        .copied()
        .unwrap_or(0);
    engine
        .state
        .zone_change_generation
        .insert(order[0], original_generation + 1);
    let before = format!("{:?}", engine.state);
    assert!(engine
        .apply_command(0, &submit_resolution_choice(order.clone()))
        .is_err());
    assert_eq!(format!("{:?}", engine.state), before);
    assert!(!engine
        .state
        .objects
        .values()
        .any(|o| o.is_token() && o.zone == Zone::Battlefield));
    engine
        .state
        .zone_change_generation
        .insert(order[0], original_generation);
    order.reverse();
    for invalid in [
        vec![order[0]; 3],
        vec![order[0], order[1]],
        vec![order[0], order[1], u32::MAX],
    ] {
        let before = format!("{:?}", engine.state);
        assert!(engine
            .apply_command(0, &submit_resolution_choice(invalid))
            .is_err());
        assert_eq!(format!("{:?}", engine.state), before);
    }
    let before = format!("{:?}", engine.state);
    assert!(engine
        .apply_command(1, &submit_resolution_choice(order.clone()))
        .is_err());
    assert_eq!(format!("{:?}", engine.state), before);
    let committed = engine
        .apply_command(0, &submit_resolution_choice(order.clone()))
        .unwrap();
    let created = token_created_events(&committed);
    assert_eq!(
        created.iter().map(|t| t.object_id).collect::<Vec<_>>(),
        order
    );
    assert_eq!(created.len(), 3);
    assert!(created
        .iter()
        .all(|t| t.enters_tapped && engine.state.objects[&t.object_id].tapped));
    for token in created {
        let index = choice
            .candidate_object_ids
            .iter()
            .position(|oid| *oid == token.object_id)
            .unwrap();
        assert_eq!(
            token.identity.as_ref(),
            Some(&choice.candidate_token_identities[index])
        );
    }
    assert!(engine.state.pending_resolution.is_none());
    let before = format!("{:?}", engine.state);
    assert!(engine
        .apply_command(0, &submit_resolution_choice(order))
        .is_err());
    assert_eq!(format!("{:?}", engine.state), before);
}
