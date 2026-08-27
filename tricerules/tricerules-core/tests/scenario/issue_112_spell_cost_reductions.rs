use super::helpers::*;
use tricerules_core::GameEngine;

#[test]
fn issue_165_witchstalker_reduction_is_published_and_cannot_pay_colored_mana() {
    let mut engine = GameEngine::new(
        165_201,
        &[0, 1],
        20,
        Some(vec![
            deck_with("island", &["witchstalker_frenzy"]),
            forest_only_deck(),
        ]),
        true,
    )
    .unwrap();
    advance_to_declare_attackers(&mut engine);
    ensure_in_hand(&mut engine, 0, "witchstalker_frenzy");
    let attackers: Vec<_> = (0..4)
        .map(|_| inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears"))
        .collect();
    let target = inject_creature_with_stats(&mut engine, 1, "grizzly_bears", 6, 6);
    engine
        .apply_command(0, &declare_attackers(attackers))
        .unwrap();
    assert_eq!(
        hand_action_reduction(&mut engine, 0, "witchstalker_frenzy", "after attacks"),
        4
    );
    let command = cast_spell(
        hand_index_for_card(&engine, 0, "witchstalker_frenzy"),
        target_object(target),
    );
    let index = engine.state.command_index;
    assert!(engine.apply_command(0, &command).is_err());
    assert_eq!(engine.state.command_index, index);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    engine.apply_command(0, &command).unwrap();
    assert_eq!(engine.state.players[0].mana_pool.red, 0);
    pass_both_players(&mut engine);
    assert_eq!(engine.state.objects[&target].damage, 5);
}

#[track_caller]
fn hand_action_reduction(
    engine: &mut GameEngine,
    player: usize,
    card_id: &str,
    stage: &str,
) -> u32 {
    let hand_index = hand_index_for_card(engine, player, card_id) as u32;
    let batch = engine.initial_response_batch();
    let actions = &batch.legal_by_player[&(player as i32)].hand_actions;
    actions
        .iter()
        .find(|action| action.hand_index == hand_index)
        .unwrap_or_else(|| {
            panic!(
                "missing cast action for {card_id} at {stage}; published hand indices: {:?}",
                actions
                    .iter()
                    .map(|action| (action.hand_index, action.card_name.as_str()))
                    .collect::<Vec<_>>()
            )
        })
        .generic_cost_reduction
}

#[test]
fn tapped_target_reduction_is_authoritative_and_untapped_target_is_not_reduced() {
    let decks = Some(vec![
        deck_with("plains", &["luminous_rebuke", "seized_from_slumber"]),
        deck_with("forest", &["grizzly_bears", "grizzly_bears"]),
    ]);
    let mut engine = GameEngine::new(112_001, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    ensure_in_hand(&mut engine, 0, "luminous_rebuke");
    ensure_in_hand(&mut engine, 0, "seized_from_slumber");
    let tapped_bear = relocate_to_battlefield(&mut engine, 1, "grizzly_bears", false);
    let untapped_bear = relocate_to_battlefield(&mut engine, 1, "grizzly_bears", false);
    engine
        .state
        .objects
        .get_mut(&tapped_bear)
        .expect("tapped bear")
        .tapped = true;

    let luminous = hand_index_for_card(&engine, 0, "luminous_rebuke");
    let batch = engine.initial_response_batch();
    let published =
        &batch.legal_by_player[&0].valid_targets_by_hand_slot[&((luminous as u32) << 8)];
    let application = published
        .targeted_cost_reduction_applications
        .first()
        .expect("target-dependent reduction application");
    assert_eq!(application.generic_mana, 3);
    assert!(application
        .qualifying_targets
        .iter()
        .any(|candidate| candidate.object_id == tapped_bear));
    assert!(!application
        .qualifying_targets
        .iter()
        .any(|candidate| candidate.object_id == untapped_bear));

    engine.state.players[0].mana_pool.white = 1;
    engine.state.players[0].mana_pool.colorless = 1;
    engine
        .apply_command(0, &cast_spell(luminous, target_object(tapped_bear)))
        .expect("Luminous Rebuke costs {1}{W} against a tapped creature");
    resolve_entire_stack_two_player(&mut engine);
    assert!(engine.state.players[1].graveyard.contains(&tapped_bear));

    engine.state.players[0].mana_pool.white = 1;
    engine.state.players[0].mana_pool.colorless = 1;
    let hand_before = engine.state.players[0].hand.clone();
    let command_before = engine.state.command_index;
    let seized = hand_index_for_card(&engine, 0, "seized_from_slumber");
    assert!(engine
        .apply_command(0, &cast_spell(seized, target_object(untapped_bear)))
        .is_err());
    assert_eq!(engine.state.players[0].hand, hand_before);
    assert_eq!(engine.state.command_index, command_before);
    assert_eq!(engine.state.players[0].mana_pool.white, 1);
    assert_eq!(engine.state.players[0].mana_pool.colorless, 1);
}

#[test]
fn battlefield_sources_filter_and_stack_their_reductions() {
    let decks = Some(vec![
        deck_with(
            "island",
            &[
                "mocking_sprite",
                "mocking_sprite",
                "highspire_bell-ringer",
                "unending_whisper",
                "divination",
                "divination",
            ],
        ),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(112_002, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    ensure_in_hand(&mut engine, 0, "unending_whisper");
    ensure_in_hand(&mut engine, 0, "divination");
    relocate_to_battlefield(&mut engine, 0, "mocking_sprite", false);
    relocate_to_battlefield(&mut engine, 0, "mocking_sprite", false);
    relocate_to_battlefield(&mut engine, 0, "highspire_bell-ringer", false);

    engine.state.players[0].mana_pool.blue = 3;
    assert_eq!(
        hand_action_reduction(&mut engine, 0, "divination", "before first spell"),
        2
    );
    assert_eq!(
        hand_action_reduction(&mut engine, 0, "unending_whisper", "before first spell"),
        2
    );

    engine.state.players[0].mana_pool.blue = 1;
    let first_spell = hand_index_for_card(&engine, 0, "unending_whisper");
    engine
        .apply_command(0, &cast_spell(first_spell, vec![]))
        .expect("two Mocking Sprites reduce an instant or sorcery's generic component only");
    resolve_entire_stack_two_player(&mut engine);

    engine.state.players[0].mana_pool.blue = 1;
    assert_eq!(
        hand_action_reduction(&mut engine, 0, "divination", "before second spell"),
        3,
        "the second spell receives both Sprite reductions and Bell-Ringer's reduction"
    );
    engine.state.players[0].mana_pool.blue = 1;
    let divination = hand_index_for_card(&engine, 0, "divination");
    engine
        .apply_command(0, &cast_spell(divination, vec![]))
        .expect("second spell is reduced to {U}");
    resolve_entire_stack_two_player(&mut engine);
    ensure_in_hand(&mut engine, 0, "divination");
    engine.state.players[0].mana_pool.blue = 1;
    assert_eq!(
        hand_action_reduction(&mut engine, 0, "divination", "before third spell"),
        2,
        "Bell-Ringer does not reduce the third spell"
    );
}

#[test]
fn affinity_counts_controlled_creatures_and_packbeast_draws_on_entry() {
    let decks = Some(vec![
        deck_with(
            "plains",
            &["salt_road_packbeast", "grizzly_bears", "grizzly_bears"],
        ),
        deck_with("forest", &["grizzly_bears"]),
    ]);
    let mut engine = GameEngine::new(112_003, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    ensure_in_hand(&mut engine, 0, "salt_road_packbeast");
    relocate_to_battlefield(&mut engine, 0, "grizzly_bears", false);
    relocate_to_battlefield(&mut engine, 0, "grizzly_bears", false);
    relocate_to_battlefield(&mut engine, 1, "grizzly_bears", false);

    assert_eq!(
        hand_action_reduction(&mut engine, 0, "salt_road_packbeast", "affinity"),
        2
    );
    engine.state.players[0].mana_pool.white = 1;
    engine.state.players[0].mana_pool.colorless = 3;
    let hand_before = engine.state.players[0].hand.len();
    let packbeast = hand_index_for_card(&engine, 0, "salt_road_packbeast");
    engine
        .apply_command(0, &cast_spell(packbeast, vec![]))
        .expect("Packbeast costs {3}{W} with two creatures");
    resolve_entire_stack_two_player(&mut engine);

    battlefield_object_for_card(&engine, 0, "salt_road_packbeast");
    assert_eq!(engine.state.players[0].hand.len(), hand_before);
}
