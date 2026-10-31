//! Prepared-state coverage for attachment links when their recipient changes zones.
use super::*;
use std::collections::BTreeMap;

fn fixture_engine() -> GameEngine {
    let deck = ["grizzly_bears", "skullclamp", "forest"]
        .into_iter()
        .cycle()
        .take(32)
        .map(str::to_owned)
        .collect::<Vec<_>>();
    GameEngine::new(
        tricerules_cards::registry::global(),
        20_261_031,
        &[0, 1],
        20,
        Some(vec![deck; 2]),
        true,
    )
    .expect("attachment departure fixture")
}

fn add_battlefield_card(engine: &mut GameEngine, seat: usize, card_id: &str) -> ObjectId {
    let object_id = engine.state.next_object_id;
    engine.state.next_object_id += 1;
    let owner = engine.state.players[seat].id;
    let object = new_object_from_card(
        object_id,
        owner,
        card_id,
        Zone::Battlefield,
        engine
            .registry
            .get(card_id)
            .expect("fixture card definition")
            .primary_face(),
    );
    engine.state.objects.insert(object_id, object);
    engine.state.zone_change_generation.insert(object_id, 0);
    let player_index = engine.state.player_idx(owner).expect("fixture player");
    engine.state.players[player_index]
        .battlefield
        .push(object_id);
    engine.emit_static_abilities_on_enter(object_id);
    object_id
}

fn clear_projection_caches(engine: &mut GameEngine) {
    // This prepared-state fixture mutates GameState directly. Match the cold publication caches
    // normal command callers rebuild after state changes so stale views cannot mask the result.
    engine.private_zone_cache.clear();
    engine.battlefield_view_cache = None;
    engine.first_strike_step_pending_cache = false;
}

#[test]
fn prepared_state_clears_inbound_attachment_before_same_resolution_return() {
    let mut engine = fixture_engine();
    let skullclamp = add_battlefield_card(&mut engine, 0, "skullclamp");
    let creature = add_battlefield_card(&mut engine, 0, "grizzly_bears");
    let mut events = Vec::new();
    assert!(attach_equipment_ids(
        &mut engine,
        skullclamp,
        creature,
        "prepared Skullclamp fixture",
        &mut events,
    ));

    let original_generation = engine.state.zone_change_generation[&creature];
    let source_generation = engine.state.zone_change_generation[&skullclamp];
    let owner = engine.state.objects[&creature].owner;
    engine
        .state
        .objects
        .get_mut(&creature)
        .unwrap()
        .counters
        .insert(CounterKind::MinusOneMinusOne, 1);
    engine
        .state
        .objects
        .get_mut(&creature)
        .unwrap()
        .counter_timestamps
        .insert(
            CounterKind::MinusOneMinusOne,
            engine.state.next_game_rule_timestamp,
        );
    clear_projection_caches(&mut engine);

    let pre_departure = engine
        .characteristics(creature)
        .expect("attached Grizzly Bears characteristics");
    assert_eq!(pre_departure.power, Some(2));
    assert_eq!(pre_departure.toughness, Some(0));

    let departure = prepare_zone_move(
        &engine.state,
        engine.registry,
        creature,
        Zone::Graveyard,
        None,
    )
    .expect("prepare recipient departure")
    .expect("Grizzly Bears is movable");
    assert_eq!(
        departure.counters,
        BTreeMap::from([(CounterKind::MinusOneMinusOne, 1)])
    );
    let prepared_lki = departure
        .last_known_characteristics
        .as_ref()
        .expect("prepared departure characteristics");
    assert_eq!(prepared_lki.power, Some(2));
    assert_eq!(prepared_lki.toughness, Some(0));

    commit_zone_move(&mut engine.state, engine.registry, departure)
        .expect("commit recipient departure");

    // This is the boundary regression: clear before any SBA settlement or same-resolution return.
    assert_eq!(
        engine.state.objects[&skullclamp].attached_to, None,
        "an attachment must stop naming the departed ObjectId at the central commit boundary"
    );
    assert_eq!(engine.state.objects[&creature].zone, Zone::Graveyard);
    assert_eq!(
        engine.state.zone_change_generation[&creature],
        original_generation + 1
    );
    assert_eq!(
        engine
            .state
            .last_known_counters_by_generation
            .get(&(creature, original_generation)),
        Some(&BTreeMap::from([(CounterKind::MinusOneMinusOne, 1)]))
    );
    assert_eq!(engine.state.objects[&skullclamp].zone, Zone::Battlefield);
    assert_eq!(
        engine.state.zone_change_generation[&skullclamp], source_generation,
        "recipient departure does not move or reincarnate the attachment source"
    );

    let return_move = prepare_zone_move(
        &engine.state,
        engine.registry,
        creature,
        Zone::Battlefield,
        Some(owner),
    )
    .expect("prepare same-ObjectId return")
    .expect("Grizzly Bears returns to the battlefield");
    commit_zone_move(&mut engine.state, engine.registry, return_move)
        .expect("commit same-ObjectId return");
    clear_projection_caches(&mut engine);

    assert_eq!(
        engine.state.zone_change_generation[&creature],
        original_generation + 2
    );
    assert!(engine.state.objects[&creature].counters.is_empty());
    assert_eq!(engine.state.objects[&skullclamp].attached_to, None);
    let returned = engine
        .characteristics(creature)
        .expect("returned Grizzly Bears characteristics");
    assert_eq!(returned.power, Some(2));
    assert_eq!(returned.toughness, Some(2));
}
