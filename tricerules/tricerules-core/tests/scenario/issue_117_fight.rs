use crate::helpers::*;
use tricerules_cards::primitives::{ContinuousEffectKind, CounterKind, EffectDuration, Keyword};
use tricerules_core::{AffectedScope, ContinuousEffect, Zone};

fn fight_targets(first: u32, second: u32) -> Vec<TargetRef> {
    vec![
        TargetRef {
            object_id: first,
            group_index: 0,
            kind: TargetRefKind::Permanent as i32,
            ..Default::default()
        },
        TargetRef {
            object_id: second,
            group_index: 1,
            kind: TargetRefKind::Permanent as i32,
            ..Default::default()
        },
    ]
}

#[test]
fn prey_upon_publishes_two_targets_and_fights_with_current_power_simultaneously() {
    let decks = Some(vec![
        deck_with("forest", &["prey_upon", "grizzly_bears"]),
        deck_with("forest", &["colossal_dreadmaw"]),
    ]);
    let mut engine = GameEngine::new(117_100, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut engine);
    let first = relocate_to_battlefield(&mut engine, 0, "grizzly_bears", false);
    let second = relocate_to_battlefield(&mut engine, 1, "colossal_dreadmaw", false);
    ensure_in_hand(&mut engine, 0, "prey_upon");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            g: 1,
            ..Default::default()
        },
    );

    let slot = hand_index_for_card(&engine, 0, "prey_upon");
    let batch = engine.initial_response_batch();
    let targets = &batch.legal_by_player[&0].valid_targets_by_hand_slot[&((slot as u32) << 8)];
    assert_eq!(targets.groups.len(), 2);
    assert_eq!(targets.groups[0].valid_permanent_ids, [first]);
    assert_eq!(targets.groups[1].valid_permanent_ids, [second]);
    assert_eq!(targets.groups[1].distinct_from_group_indices, [0]);

    let hand_before = engine.state.players[0].hand.clone();
    let mana_before = engine.state.players[0].mana_pool.green;
    for invalid in [
        fight_targets(second, first),
        vec![fight_targets(first, second)[0]],
    ] {
        assert!(engine.apply_command(0, &cast_spell(slot, invalid)).is_err());
        assert_eq!(engine.state.players[0].hand, hand_before);
        assert_eq!(engine.state.players[0].mana_pool.green, mana_before);
    }

    engine
        .apply_command(0, &cast_spell(slot, fight_targets(first, second)))
        .expect("cast Prey Upon");
    engine
        .state
        .objects
        .get_mut(&first)
        .expect("first fighter")
        .set_counter(CounterKind::PlusOnePlusOne, 1);
    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(engine.state.objects[&first].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&second].damage, 3);
}

#[test]
fn prey_upon_deals_no_damage_when_either_target_is_illegal_at_resolution() {
    let decks = Some(vec![
        deck_with("forest", &["prey_upon", "grizzly_bears"]),
        deck_with("forest", &["hill_giant"]),
    ]);
    let mut engine = GameEngine::new(117_101, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut engine);
    let first = relocate_to_battlefield(&mut engine, 0, "grizzly_bears", false);
    let second = relocate_to_battlefield(&mut engine, 1, "hill_giant", false);
    ensure_in_hand(&mut engine, 0, "prey_upon");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            g: 1,
            ..Default::default()
        },
    );

    let slot = hand_index_for_card(&engine, 0, "prey_upon");
    engine
        .apply_command(0, &cast_spell(slot, fight_targets(first, second)))
        .expect("cast Prey Upon");
    engine.state.players[1]
        .battlefield
        .retain(|object_id| *object_id != second);
    engine.state.players[1].hand.push(second);
    engine.state.objects.get_mut(&second).expect("second").zone = Zone::Hand;
    *engine
        .state
        .zone_change_generation
        .entry(second)
        .or_default() += 1;
    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(engine.state.objects[&first].damage, 0);
    assert_eq!(engine.state.objects[&first].zone, Zone::Battlefield);
}

#[test]
fn prey_upon_deals_no_damage_when_a_target_is_no_longer_a_creature() {
    let decks = Some(vec![
        deck_with("forest", &["prey_upon", "grizzly_bears"]),
        deck_with("forest", &["hill_giant"]),
    ]);
    let mut engine = GameEngine::new(117_105, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut engine);
    let first = relocate_to_battlefield(&mut engine, 0, "grizzly_bears", false);
    let second = relocate_to_battlefield(&mut engine, 1, "hill_giant", false);
    ensure_in_hand(&mut engine, 0, "prey_upon");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            g: 1,
            ..Default::default()
        },
    );

    let slot = hand_index_for_card(&engine, 0, "prey_upon");
    engine
        .apply_command(0, &cast_spell(slot, fight_targets(first, second)))
        .expect("cast Prey Upon");
    // Model a copy/type-changing effect making the same physical target a land before resolution.
    engine
        .state
        .objects
        .get_mut(&second)
        .expect("second target")
        .card_id = "forest".into();
    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(engine.state.objects[&first].damage, 0);
    assert_eq!(engine.state.objects[&second].damage, 0);
}

#[test]
fn fight_reuses_prevention_deathtouch_lifelink_and_state_based_actions() {
    let decks = Some(vec![
        deck_with("forest", &["prey_upon", "grizzly_bears"]),
        deck_with("forest", &["colossal_dreadmaw"]),
    ]);
    let mut engine = GameEngine::new(117_102, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut engine);
    let first = relocate_to_battlefield(&mut engine, 0, "grizzly_bears", false);
    let second = relocate_to_battlefield(&mut engine, 1, "colossal_dreadmaw", false);
    engine.state.players[0].life = 10;
    for keyword in [Keyword::Deathtouch, Keyword::Lifelink] {
        engine.state.continuous_effects.push(ContinuousEffect {
            trigger_grant_origin: None,
            source_id: None,
            affected: AffectedScope::Single(first),
            kind: ContinuousEffectKind::Layer6AddKeyword(keyword),
            condition: None,
            duration: EffectDuration::UntilEndOfTurn,
            timestamp: engine.state.command_index,
        });
    }
    engine.state.add_damage_prevention_shield(second, 1);
    ensure_in_hand(&mut engine, 0, "prey_upon");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            g: 1,
            ..Default::default()
        },
    );

    let slot = hand_index_for_card(&engine, 0, "prey_upon");
    engine
        .apply_command(0, &cast_spell(slot, fight_targets(first, second)))
        .expect("cast Prey Upon");
    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(engine.state.players[0].life, 11);
    assert_eq!(engine.state.objects[&first].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&second].zone, Zone::Graveyard);
}

#[test]
fn bushwhack_search_mode_reveals_a_basic_land_to_hand() {
    let decks = Some(vec![
        deck_with("forest", &["bushwhack"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(117_103, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut engine);
    ensure_in_hand(&mut engine, 0, "bushwhack");
    let land = inject_library_card(&mut engine, 0, "plains");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            g: 1,
            ..Default::default()
        },
    );

    let slot = hand_index_for_card(&engine, 0, "bushwhack");
    engine
        .apply_command(0, &cast_modal_spell(slot, vec![(0, vec![])]))
        .expect("cast Bushwhack search mode");
    engine.apply_command(0, &pass()).expect("caster pass");
    let batch = engine.apply_command(1, &pass()).expect("opponent pass");
    let choice = find_resolution_choice(&batch).expect("library search choice");
    assert_eq!(choice.choice_kind(), ChoiceKind::LibrarySearch);
    assert!(choice.candidate_object_ids.contains(&land));
    engine
        .apply_command(0, &submit_resolution_choice(vec![land]))
        .expect("choose basic land");

    assert!(engine.state.players[0].hand.contains(&land));
    assert_eq!(engine.state.objects[&land].zone, Zone::Hand);
}

#[test]
fn bushwhack_fight_mode_uses_both_target_groups() {
    let decks = Some(vec![
        deck_with("forest", &["bushwhack", "grizzly_bears"]),
        deck_with("forest", &["hill_giant"]),
    ]);
    let mut engine = GameEngine::new(117_104, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut engine);
    let first = relocate_to_battlefield(&mut engine, 0, "grizzly_bears", false);
    let second = relocate_to_battlefield(&mut engine, 1, "hill_giant", false);
    ensure_in_hand(&mut engine, 0, "bushwhack");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            g: 1,
            ..Default::default()
        },
    );

    let slot = hand_index_for_card(&engine, 0, "bushwhack");
    engine
        .apply_command(
            0,
            &cast_modal_spell(slot, vec![(1, fight_targets(first, second))]),
        )
        .expect("cast Bushwhack fight mode");
    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(engine.state.objects[&first].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&second].damage, 2);
}
