use super::helpers::*;
use tricerules_cards::CounterKind;
use tricerules_core::Zone;

fn resolve_top_stack(engine: &mut GameEngine) -> RuledEventBatch {
    let first = engine.state.priority_player_id();
    let second = if first == engine.state.players[0].id {
        engine.state.players[1].id
    } else {
        engine.state.players[0].id
    };
    engine
        .apply_command(first, &pass())
        .expect("first priority pass");
    engine
        .apply_command(second, &pass())
        .expect("second priority pass resolves stack item")
}

fn cast_and_resolve_mutagen_source(
    seed: u64,
    player: usize,
    card_id: &str,
) -> (GameEngine, u32, u32, RuledEventBatch) {
    let decks = Some(if player == 0 {
        vec![
            deck_with("island", &[card_id, "grizzly_bears"]),
            deck_with("forest", &["grizzly_bears"]),
        ]
    } else {
        vec![
            deck_with("island", &["grizzly_bears"]),
            deck_with("forest", &[card_id, "grizzly_bears"]),
        ]
    });
    let mut engine = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    move_ready_to_battlefield(&mut engine, player, card_id);
    assert_eq!(
        engine.state.stack.len(),
        1,
        "entry stages exactly one trigger"
    );
    let resolved = resolve_top_stack(&mut engine);
    let source = battlefield_object_for_card(&engine, player, card_id);
    let mutagens = battlefield_token_oids(&engine, player, "mutagen");
    let [mutagen] = mutagens.as_slice() else {
        panic!("{card_id} should create one Mutagen");
    };
    (engine, source, *mutagen, resolved)
}

#[test]
fn issue_291_player_one_enters_and_owns_the_mutagen_token() {
    let (engine, source, mutagen, resolved) =
        cast_and_resolve_mutagen_source(291_006, 1, "slithering_cryptid");
    let source_object = &engine.state.objects[&source];
    let token = &engine.state.objects[&mutagen];
    assert_eq!(source_object.zone, Zone::Battlefield);
    assert_eq!(source_object.owner, 1);
    assert_eq!(source_object.controller, 1);
    assert_eq!(token.zone, Zone::Battlefield);
    assert_eq!(token.owner, 1, "token owner is the creating controller");
    assert_eq!(token.controller, 1);
    assert_eq!(token.base_controller, 1);
    assert!(battlefield_token_oids(&engine, 0, "mutagen").is_empty());
    let created = token_created_events(&resolved);
    assert_eq!(created.len(), 1, "one public token event");
    assert_eq!(created[0].object_id, mutagen);
    assert_eq!(created[0].controller_player_id, 1);
    assert_eq!(created[0].card_id, "mutagen");
    assert!(!created[0].enters_tapped);
}

#[test]
fn issue_291_each_allowlisted_creature_enters_and_creates_controller_owned_mutagen() {
    for (seed, card_id) in [
        (291_001, "crustacean_commando"),
        (291_002, "slithering_cryptid"),
    ] {
        let (engine, source, mutagen, resolved) = cast_and_resolve_mutagen_source(seed, 0, card_id);
        let source_object = &engine.state.objects[&source];
        let token = &engine.state.objects[&mutagen];
        assert_eq!(source_object.zone, Zone::Battlefield);
        assert_eq!(token.zone, Zone::Battlefield);
        assert_eq!(token.owner, 0, "token owner is the creating controller");
        assert_eq!(token.controller, 0);
        assert_eq!(token.base_controller, 0);
        assert!(!token.tapped, "the exact recipe enters the token untapped");
        assert!(
            token.summoning_sick,
            "the artifact token is a new permanent"
        );
        assert!(battlefield_token_oids(&engine, 1, "mutagen").is_empty());
        let created = token_created_events(&resolved);
        assert_eq!(created.len(), 1, "one public token event");
        assert_eq!(created[0].object_id, mutagen);
        assert_eq!(created[0].controller_player_id, 0);
        assert_eq!(created[0].card_id, "mutagen");
        assert!(!created[0].enters_tapped);
    }
}

#[test]
fn issue_291_mutagen_activation_pays_cost_targets_creature_and_places_one_counter() {
    let (mut engine, _source, mutagen, _resolved) =
        cast_and_resolve_mutagen_source(291_003, 0, "crustacean_commando");
    let target = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    assert_eq!(
        engine.state.objects[&target].counter_count(CounterKind::PlusOnePlusOne),
        0
    );
    grant_pool(&mut engine, 0);
    let before_mana = engine.state.players[0].mana_pool;
    apply_ability(&mut engine, 0, mutagen, 0, target_object(target))
        .expect("activate Mutagen at sorcery speed");
    let after_mana = engine.state.players[0].mana_pool;
    let total = |pool: tricerules_core::state::ManaPool| {
        pool.white + pool.blue + pool.black + pool.red + pool.green + pool.colorless
    };
    assert_eq!(total(after_mana), total(before_mana) - 1, "pay {{1}}");
    assert!(
        battlefield_token_oids(&engine, 0, "mutagen").is_empty(),
        "SacrificeSelf is paid when the activation is announced"
    );
    pass_both_players(&mut engine);
    assert_eq!(
        engine.state.objects[&target].counter_count(CounterKind::PlusOnePlusOne),
        1
    );
    assert!(battlefield_token_oids(&engine, 0, "mutagen").is_empty());
}

#[test]
fn issue_291_mutagen_requires_one_mana_and_sorcery_timing() {
    let (mut engine, _source, mutagen, _resolved) =
        cast_and_resolve_mutagen_source(291_004, 0, "slithering_cryptid");
    let target = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    engine.state.players[0].mana_pool = Default::default();
    let before_command = engine.state.command_index;
    let err = apply_ability(&mut engine, 0, mutagen, 0, target_object(target))
        .expect_err("Mutagen cannot activate without its {1} cost");
    assert!(matches!(err, tricerules_core::EngineError::Illegal(_)));
    assert_eq!(engine.state.command_index, before_command);
    assert!(engine.state.objects.contains_key(&mutagen));
    assert_eq!(
        engine.state.objects[&target].counter_count(CounterKind::PlusOnePlusOne),
        0
    );

    grant_pool(&mut engine, 0);
    end_active_turn(&mut engine, 0);
    assert!(!zone_view_ability_flags(&mut engine, 0, mutagen)[0]);
    let err = apply_ability(&mut engine, 0, mutagen, 0, target_object(target))
        .expect_err("Mutagen is sorcery-speed and P0 has no priority on P1's turn");
    assert!(matches!(err, tricerules_core::EngineError::Illegal(_)));
    assert!(engine.state.objects.contains_key(&mutagen));
    assert_eq!(
        engine.state.objects[&target].counter_count(CounterKind::PlusOnePlusOne),
        0
    );
}

#[test]
fn issue_291_mutagen_rejects_noncreature_targets_and_stale_source_generation() {
    let (mut engine, _source, mutagen, _resolved) =
        cast_and_resolve_mutagen_source(291_005, 0, "crustacean_commando");
    let noncreature = inject_permanent_on_battlefield(&mut engine, 1, "forest");
    grant_pool(&mut engine, 0);
    let before_pool = engine.state.players[0].mana_pool;
    let err = apply_ability(&mut engine, 0, mutagen, 0, target_object(noncreature))
        .expect_err("Mutagen requires a creature target");
    assert!(matches!(err, tricerules_core::EngineError::Illegal(_)));
    assert_eq!(engine.state.players[0].mana_pool, before_pool);
    assert!(engine.state.objects.contains_key(&mutagen));

    let target = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let generation = engine.state.zone_change_generation[&mutagen];
    let mut stale = activate_ability_for(&engine, mutagen, 0, target_object(target));
    let Some(Cmd::ActivateAbility(ability)) = stale.cmd.as_mut() else {
        unreachable!("activate_ability_for emits ActivateAbility");
    };
    ability.expected_zone_change_generation = generation + 1;
    let err = engine
        .apply_command(0, &stale)
        .expect_err("stale Mutagen physical identity must be rejected");
    assert!(
        format!("{err:?}").contains("stale"),
        "unexpected error: {err}"
    );
    assert_eq!(engine.state.players[0].mana_pool, before_pool);
    assert!(engine.state.objects.contains_key(&mutagen));
    assert_eq!(
        engine.state.objects[&target].counter_count(CounterKind::PlusOnePlusOne),
        0
    );
}

#[test]
fn issue_291_mutagen_fizzles_against_a_stale_target_generation() {
    let (mut engine, _source, mutagen, _resolved) =
        cast_and_resolve_mutagen_source(291_007, 0, "crustacean_commando");
    let target = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let published_generation = engine
        .state
        .zone_change_generation
        .get(&target)
        .copied()
        .unwrap_or(0);
    let ability_key = u64::from(mutagen) << 32;
    let published = engine.initial_response_batch();
    assert!(
        published.legal_by_player[&0].valid_targets_by_ability[&ability_key].groups[0]
            .valid_permanent_ids
            .contains(&target)
    );

    grant_pool(&mut engine, 0);
    let before_pool = engine.state.players[0].mana_pool;
    apply_ability(&mut engine, 0, mutagen, 0, target_object(target))
        .expect("activate Mutagen while the published target is legal");
    let after_pool = engine.state.players[0].mana_pool;
    let total = |pool: tricerules_core::state::ManaPool| {
        pool.white + pool.blue + pool.black + pool.red + pool.green + pool.colorless
    };
    assert_eq!(total(after_pool), total(before_pool) - 1, "pay {{1}}");
    assert!(
        battlefield_token_oids(&engine, 0, "mutagen").is_empty(),
        "SacrificeSelf is paid before the target can become stale"
    );
    assert_eq!(engine.state.stack.len(), 1);
    assert_eq!(
        engine.state.stack[0].targets[0].zone_change_generation,
        Some(published_generation),
        "the activated ability captures the target's physical generation"
    );

    // Model a real leave-and-return: the same ObjectId is a new CR 400.7 object after each zone
    // change, so the StackTarget captured above must not apply to the returned generation.
    engine.state.players[1]
        .battlefield
        .retain(|object_id| *object_id != target);
    engine.state.players[1].graveyard.push(target);
    engine.state.objects.get_mut(&target).unwrap().zone = Zone::Graveyard;
    *engine
        .state
        .zone_change_generation
        .entry(target)
        .or_default() += 1;
    let departed_generation = engine.state.zone_change_generation[&target];
    engine.state.players[1]
        .graveyard
        .retain(|object_id| *object_id != target);
    engine.state.players[1].battlefield.push(target);
    let object = engine.state.objects.get_mut(&target).unwrap();
    object.zone = Zone::Battlefield;
    object.base_controller = 1;
    object.controller = 1;
    object.tapped = false;
    object.summoning_sick = false;
    *engine
        .state
        .zone_change_generation
        .entry(target)
        .or_default() += 1;
    let returned_generation = engine.state.zone_change_generation[&target];
    assert_ne!(
        returned_generation, published_generation,
        "the returned battlefield object has a new physical generation"
    );
    assert_eq!(departed_generation + 1, returned_generation);
    assert!(engine.state.players[1].battlefield.contains(&target));

    let resolved = resolve_top_stack(&mut engine);
    assert!(resolved.events.iter().any(|event| {
        matches!(
            event.ev,
            Some(Ev::StackResolved(ref stack_resolved)) if stack_resolved.object_id != target
        )
    }));
    assert_eq!(engine.state.stack.len(), 0);
    assert_eq!(engine.state.objects[&target].zone, Zone::Battlefield);
    assert_eq!(
        engine.state.zone_change_generation[&target],
        returned_generation
    );
    assert_eq!(
        engine.state.objects[&target].counter_count(CounterKind::PlusOnePlusOne),
        0,
        "the old StackTarget cannot put a counter on the returned generation"
    );
    assert!(battlefield_token_oids(&engine, 0, "mutagen").is_empty());
}
