use super::helpers::*;
use tricerules_cards::CounterKind;
use tricerules_core::{GameEngine, Zone};

fn published_ability_mana_cost(
    engine: &mut GameEngine,
    player: usize,
    object_id: u32,
    ability_index: usize,
) -> String {
    engine
        .initial_response_batch()
        .events
        .iter()
        .find_map(|event| match &event.ev {
            Some(Ev::ZoneView(view)) => Some(view),
            _ => None,
        })
        .and_then(|view| view.per_player.get(player))
        .into_iter()
        .flat_map(|view| view.battlefield_objects.iter())
        .find(|object| object.object_id == object_id)
        .and_then(|object| object.activated_abilities.get(ability_index))
        .map(|ability| ability.mana_cost.clone())
        .unwrap_or_else(|| panic!("missing published ability {ability_index} for {object_id}"))
}

#[test]
fn starport_security_reduces_only_for_a_plus_one_plus_one_counter() {
    let decks = Some(vec![
        deck_with("plains", &["starport_security", "grizzly_bears"]),
        deck_with("forest", &["grizzly_bears"]),
    ]);
    let mut engine = GameEngine::new(158_001, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    let security = relocate_to_battlefield(&mut engine, 0, "starport_security", false);
    let target = relocate_to_battlefield(&mut engine, 1, "grizzly_bears", false);

    engine
        .state
        .objects
        .get_mut(&security)
        .expect("Starport Security")
        .add_counters(CounterKind::Stun, 1, 1);
    assert_eq!(
        published_ability_mana_cost(&mut engine, 0, security, 0),
        "{3}{W}",
        "an unrelated counter must not satisfy the Oracle condition"
    );

    give_mana(
        &mut engine,
        0,
        ManaGift {
            w: 1,
            c: 1,
            ..Default::default()
        },
    );
    let mana_before = (
        engine.state.players[0].mana_pool.white,
        engine.state.players[0].mana_pool.colorless,
    );
    let command_before = engine.state.command_index;
    assert!(apply_ability(&mut engine, 0, security, 0, target_object(target)).is_err());
    assert_eq!(
        (
            engine.state.players[0].mana_pool.white,
            engine.state.players[0].mana_pool.colorless,
        ),
        mana_before
    );
    assert_eq!(engine.state.command_index, command_before);
    assert!(!engine.state.objects[&security].tapped);

    engine
        .state
        .objects
        .get_mut(&security)
        .expect("Starport Security")
        .add_counters(CounterKind::PlusOnePlusOne, 1, 2);
    assert_eq!(
        published_ability_mana_cost(&mut engine, 0, security, 0),
        "{1}{W}"
    );
    apply_ability(&mut engine, 0, security, 0, target_object(target))
        .expect("activate reduced Starport Security ability");
    assert!(engine.state.objects[&security].tapped);
    resolve_entire_stack_two_player(&mut engine);
    assert!(engine.state.objects[&target].tapped);
}

#[test]
fn boneclub_berserker_counts_other_controlled_goblins_in_layer_seven() {
    let decks = Some(vec![
        deck_with(
            "mountain",
            &["boneclub_berserker", "raging_goblin", "goblin_arsonist"],
        ),
        deck_with("mountain", &["crazed_goblin"]),
    ]);
    let mut engine = GameEngine::new(158_002, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    ensure_in_hand(&mut engine, 0, "boneclub_berserker");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            r: 1,
            c: 3,
            ..Default::default()
        },
    );
    let hand_index = hand_index_for_card(&engine, 0, "boneclub_berserker");
    engine
        .apply_command(0, &cast_spell(hand_index, vec![]))
        .expect("cast Boneclub Berserker");
    resolve_entire_stack_two_player(&mut engine);
    let berserker = battlefield_object_for_card(&engine, 0, "boneclub_berserker");
    assert_eq!(engine.characteristics(berserker).unwrap().power, Some(2));

    relocate_to_battlefield(&mut engine, 0, "raging_goblin", false);
    relocate_to_battlefield(&mut engine, 1, "crazed_goblin", false);
    assert_eq!(
        engine.characteristics(berserker).unwrap().power,
        Some(4),
        "the source and an opponent's Goblin are excluded"
    );

    relocate_to_battlefield(&mut engine, 0, "goblin_arsonist", false);
    assert_eq!(engine.characteristics(berserker).unwrap().power, Some(6));
}

#[test]
fn sold_out_uses_the_chosen_targets_pre_exile_damage_identity() {
    let decks = Some(vec![
        deck_with("swamp", &["sold_out", "shock"]),
        deck_with("forest", &["giant_spider"]),
    ]);
    let mut engine = GameEngine::new(158_003, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    let target = relocate_to_battlefield(&mut engine, 1, "giant_spider", false);
    ensure_in_hand(&mut engine, 0, "shock");
    ensure_in_hand(&mut engine, 0, "sold_out");

    give_mana(
        &mut engine,
        0,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    let shock = hand_index_for_card(&engine, 0, "shock");
    engine
        .apply_command(0, &cast_spell(shock, target_object(target)))
        .expect("cast Shock");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&target].damage, 2);

    give_mana(
        &mut engine,
        0,
        ManaGift {
            b: 1,
            c: 3,
            ..Default::default()
        },
    );
    let sold_out = hand_index_for_card(&engine, 0, "sold_out");
    engine
        .apply_command(0, &cast_spell(sold_out, target_object(target)))
        .expect("cast Sold Out");
    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(engine.state.objects[&target].zone, Zone::Exile);
    assert_eq!(battlefield_token_oids(&engine, 0, "clue").len(), 1);
}

#[test]
fn flaring_cinder_cast_trigger_includes_announced_x_in_mana_value() {
    let decks = Some(vec![
        deck_with("mountain", &["flaring_cinder", "blaze", "blaze"]),
        deck_with("island", &[]),
    ]);
    let mut engine = GameEngine::new(158_004, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    let cinder = relocate_to_battlefield(&mut engine, 0, "flaring_cinder", false);
    ensure_in_hand(&mut engine, 0, "blaze");

    give_mana(
        &mut engine,
        0,
        ManaGift {
            r: 1,
            c: 2,
            ..Default::default()
        },
    );
    let first_blaze = hand_index_for_card(&engine, 0, "blaze");
    engine
        .apply_command(0, &cast_spell_x(first_blaze, target_player(1), 2))
        .expect("cast Blaze with mana value three");
    assert_eq!(
        engine.state.stack.len(),
        1,
        "X=2 makes Blaze mana value three and does not trigger Cinder"
    );
    resolve_entire_stack_two_player(&mut engine);

    ensure_in_hand(&mut engine, 0, "blaze");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            r: 1,
            c: 3,
            ..Default::default()
        },
    );
    let second_blaze = hand_index_for_card(&engine, 0, "blaze");
    engine
        .apply_command(0, &cast_spell_x(second_blaze, target_player(1), 3))
        .expect("cast Blaze with mana value four");
    assert_eq!(engine.state.stack.len(), 2);
    let trigger = engine.state.stack.last().expect("Flaring Cinder trigger");
    assert!(trigger.is_triggered);
    assert_eq!(trigger.source_permanent_id, Some(cinder));
}
