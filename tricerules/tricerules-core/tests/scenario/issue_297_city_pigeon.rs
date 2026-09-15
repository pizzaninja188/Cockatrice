use super::helpers::*;
use tricerules_core::Zone;

fn three_player_main1(seed: u64) -> GameEngine {
    let mut engine = GameEngine::new(seed, &[0, 1], 20, None, true).expect("engine");
    engine
        .state
        .players
        .push(tricerules_core::state::PlayerState::new(2, 20));
    while engine.state.turn_step != tricerules_core::TurnStep::Main1 {
        let player = engine.state.priority_player_id();
        engine
            .apply_command(player, &pass())
            .expect("advance three-player turn");
    }
    engine
}

fn resolve_entire_stack_three_player(engine: &mut GameEngine) {
    loop {
        answer_trigger_order_in_engine_order(engine);
        if engine.state.stack.is_empty() {
            break;
        }
        for _ in 0..engine.state.players.len() {
            if engine.state.stack.is_empty() {
                break;
            }
            let player = engine.state.priority_player_id();
            engine
                .apply_command(player, &pass())
                .expect("three-player priority pass");
        }
    }
}

#[test]
fn issue_297_city_pigeon_leaves_to_hand_graveyard_and_exile_once() {
    let decks = Some(vec![
        deck_with(
            "plains",
            &[
                "city_pigeon",
                "city_pigeon",
                "city_pigeon",
                "unsummon",
                "lightning_bolt",
                "wander_off",
            ],
        ),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(297_001, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);

    for card_id in ["unsummon", "lightning_bolt", "wander_off"] {
        relocate_to_hand(&mut engine, 0, card_id);
    }
    let cities = (0..3)
        .map(|_| relocate_to_battlefield(&mut engine, 0, "city_pigeon", false))
        .collect::<Vec<_>>();
    assert!(
        engine.state.stack.is_empty(),
        "City Pigeon has no ETB trigger"
    );

    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            b: 1,
            r: 1,
            c: 3,
            ..Default::default()
        },
    );
    let unsummon = hand_index_for_card(&engine, 0, "unsummon");
    engine
        .apply_command(0, &cast_spell(unsummon, target_object(cities[0])))
        .expect("bounce City Pigeon to hand");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&cities[0]].zone, Zone::Hand);
    assert_eq!(battlefield_token_oids(&engine, 0, "food").len(), 1);

    let bolt = hand_index_for_card(&engine, 0, "lightning_bolt");
    engine
        .apply_command(0, &cast_spell(bolt, target_object(cities[1])))
        .expect("damage City Pigeon into the graveyard");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&cities[1]].zone, Zone::Graveyard);
    assert_eq!(battlefield_token_oids(&engine, 0, "food").len(), 2);

    let wander_off = hand_index_for_card(&engine, 0, "wander_off");
    engine
        .apply_command(0, &cast_spell(wander_off, target_object(cities[2])))
        .expect("exile City Pigeon");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&cities[2]].zone, Zone::Exile);
    assert_eq!(battlefield_token_oids(&engine, 0, "food").len(), 3);
    assert!(battlefield_token_oids(&engine, 1, "food").is_empty());
}

#[test]
fn issue_297_city_pigeon_uses_last_known_controller_but_owner_zone() {
    let decks = Some(vec![
        deck_with("island", &["unsummon"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(297_002, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);

    let city = inject_creature_under_foreign_control(&mut engine, 1, 0, "city_pigeon");
    assert_eq!(engine.state.objects[&city].owner, 1);
    assert_eq!(engine.state.objects[&city].controller, 0);
    relocate_to_hand(&mut engine, 0, "unsummon");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    let generation_before = engine
        .state
        .zone_change_generation
        .get(&city)
        .copied()
        .unwrap_or(0);
    let slot = hand_index_for_card(&engine, 0, "unsummon");
    engine
        .apply_command(0, &cast_spell(slot, target_object(city)))
        .expect("bounce foreign-controlled City Pigeon");
    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(engine.state.objects[&city].zone, Zone::Hand);
    assert_eq!(engine.state.objects[&city].controller, 1);
    assert!(engine.state.players[1].hand.contains(&city));
    assert!(!engine.state.players[0].hand.contains(&city));
    assert!(
        engine.state.zone_change_generation[&city] > generation_before,
        "the leave creates a new object generation"
    );
    assert_eq!(battlefield_token_oids(&engine, 0, "food").len(), 1);
    assert!(battlefield_token_oids(&engine, 1, "food").is_empty());
}

#[test]
fn issue_297_city_pigeon_multiplayer_food_stays_with_event_controller() {
    let mut engine = three_player_main1(297_004);
    let city = inject_creature_under_foreign_control(&mut engine, 1, 0, "city_pigeon");
    inject_card_into_hand(&mut engine, 0, "unsummon");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "unsummon");
    engine
        .apply_command(0, &cast_spell(slot, target_object(city)))
        .expect("bounce foreign-controlled City Pigeon in multiplayer");
    resolve_entire_stack_three_player(&mut engine);

    assert_eq!(engine.state.objects[&city].zone, Zone::Hand);
    assert!(engine.state.players[1].hand.contains(&city));
    assert_eq!(battlefield_token_oids(&engine, 0, "food").len(), 1);
    assert!(battlefield_token_oids(&engine, 1, "food").is_empty());
    assert!(battlefield_token_oids(&engine, 2, "food").is_empty());
}

#[test]
fn issue_297_city_pigeon_blink_has_one_leave_food_and_no_return_duplicate() {
    let decks = Some(vec![
        deck_with("plains", &["city_pigeon", "wander_off"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(297_003, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);

    relocate_to_hand(&mut engine, 0, "city_pigeon");
    assert!(
        engine.state.stack.is_empty(),
        "a card in hand cannot trigger"
    );
    let city = move_ready_to_battlefield(&mut engine, 0, "city_pigeon");
    assert!(
        engine.state.stack.is_empty(),
        "City Pigeon has no ETB trigger"
    );

    relocate_to_hand(&mut engine, 0, "wander_off");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            b: 1,
            c: 3,
            ..Default::default()
        },
    );
    let wander_off = hand_index_for_card(&engine, 0, "wander_off");
    engine
        .apply_command(0, &cast_spell(wander_off, target_object(city)))
        .expect("blink City Pigeon to exile");
    assert_eq!(engine.state.objects[&city].zone, Zone::Battlefield);
    assert_eq!(
        engine.state.stack.len(),
        1,
        "one trigger for one leave event"
    );
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(battlefield_token_oids(&engine, 0, "food").len(), 1);

    let before_return = engine.state.objects[&city].zone;
    assert_eq!(before_return, Zone::Exile);
    let returned = move_ready_to_battlefield(&mut engine, 0, "city_pigeon");
    assert_eq!(returned, city);
    assert_eq!(engine.state.objects[&city].zone, Zone::Battlefield);
    assert!(
        engine.state.stack.is_empty(),
        "return is not a second leave"
    );
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(battlefield_token_oids(&engine, 0, "food").len(), 1);
}
