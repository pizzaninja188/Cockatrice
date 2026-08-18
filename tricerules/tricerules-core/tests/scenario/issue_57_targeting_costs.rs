use super::helpers::*;

#[test]
fn boreal_elemental_increases_an_opponents_targeting_spell_cost_atomically() {
    let decks = vec![
        deck_with("island", &["unsummon"]),
        deck_with("island", &["boreal_elemental"]),
    ];
    let mut engine = GameEngine::new(57_001, &[0, 1], 20, Some(decks), true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);

    let boreal = relocate_to_battlefield(&mut engine, 1, "boreal_elemental", false);
    ensure_in_hand(&mut engine, 0, "unsummon");
    let unsummon_index = hand_index_for_card(&engine, 0, "unsummon");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );

    let original_hand = engine.state.players[0].hand.clone();
    let error = engine
        .apply_command(0, &cast_spell(unsummon_index, target_object(boreal)))
        .expect_err("Boreal Elemental should require two additional generic mana");
    assert!(
        error.to_string().contains("mana"),
        "unexpected error: {error}"
    );
    assert_eq!(engine.state.players[0].hand, original_hand);
    assert!(engine.state.stack.is_empty());
    assert_eq!(engine.state.players[0].mana_pool.blue, 1);

    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );
    engine
        .apply_command(0, &cast_spell(unsummon_index, target_object(boreal)))
        .expect("paying {2}{U} should cast Unsummon");
    assert_eq!(engine.state.stack.len(), 1);
    assert_eq!(engine.state.players[0].mana_pool.blue, 0);
    assert_eq!(engine.state.players[0].mana_pool.colorless, 0);
}

#[test]
fn kopala_applies_once_when_a_spell_targets_multiple_merfolk() {
    let decks = vec![
        deck_with("mountain", &["fireball"]),
        deck_with("island", &["kopala,_warden_of_waves", "coral_merfolk"]),
    ];
    let mut engine = GameEngine::new(57_002, &[0, 1], 20, Some(decks), true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);

    let kopala = relocate_to_battlefield(&mut engine, 1, "kopala,_warden_of_waves", false);
    let merfolk = relocate_to_battlefield(&mut engine, 1, "coral_merfolk", false);
    ensure_in_hand(&mut engine, 0, "fireball");
    let fireball_index = hand_index_for_card(&engine, 0, "fireball");
    let legal = engine.initial_response_batch();
    let published =
        &legal.legal_by_player[&0].valid_targets_by_hand_slot[&((fireball_index as u32) << 8)];
    assert_eq!(published.targeting_cost_applications.len(), 1);
    assert_eq!(published.targeting_cost_applications[0].generic_mana, 2);
    let affected = &published.targeting_cost_applications[0].affected_targets;
    assert!(affected.iter().any(|target| target.object_id == kopala));
    assert!(affected.iter().any(|target| target.object_id == merfolk));
    give_mana(
        &mut engine,
        0,
        ManaGift {
            r: 1,
            c: 4,
            ..Default::default()
        },
    );

    engine
        .apply_command(
            0,
            &cast_spell_x(
                fireball_index,
                vec![
                    tricerules_proto::ruled::v1::TargetRef {
                        object_id: kopala,
                        ..Default::default()
                    },
                    tricerules_proto::ruled::v1::TargetRef {
                        object_id: merfolk,
                        ..Default::default()
                    },
                ],
                1,
            ),
        )
        .expect("one Kopala applies one {2} increase, not once per target");
    assert_eq!(engine.state.stack.len(), 1);
    assert_eq!(engine.state.players[0].mana_pool.colorless, 0);
}

#[test]
fn kopala_adds_mana_to_an_opponents_manaless_activated_ability() {
    let decks = vec![
        deck_with("island", &["prodigal_sorcerer"]),
        deck_with("island", &["kopala,_warden_of_waves"]),
    ];
    let mut engine = GameEngine::new(57_003, &[0, 1], 20, Some(decks), true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);

    let prodigal = relocate_to_battlefield(&mut engine, 0, "prodigal_sorcerer", false);
    let kopala = relocate_to_battlefield(&mut engine, 1, "kopala,_warden_of_waves", false);
    let ability_key = u64::from(prodigal) << 32;
    let legal = engine.initial_response_batch();
    let published = &legal.legal_by_player[&0].valid_targets_by_ability[&ability_key];
    assert_eq!(published.targeting_cost_applications.len(), 1);
    assert_eq!(published.targeting_cost_applications[0].generic_mana, 2);
    assert!(published.targeting_cost_applications[0]
        .affected_targets
        .iter()
        .any(|target| target.object_id == kopala));
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );

    let error = engine
        .apply_command(0, &activate_ability(prodigal, 0, target_object(kopala)))
        .expect_err("Kopala should add {2} even when the printed ability has no mana cost");
    assert!(
        error.to_string().contains("mana"),
        "unexpected error: {error}"
    );
    assert!(!engine.state.objects[&prodigal].tapped);
    assert!(engine.state.stack.is_empty());

    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );
    engine
        .apply_command(0, &activate_ability(prodigal, 0, target_object(kopala)))
        .expect("paying {2} should activate the ability");
    assert!(engine.state.objects[&prodigal].tapped);
    assert_eq!(engine.state.stack.len(), 1);
    assert_eq!(engine.state.players[0].mana_pool.colorless, 0);
}

#[test]
fn distinct_kopala_sources_stack_but_do_not_tax_their_controller() {
    let decks = vec![
        deck_with("island", &["unsummon"]),
        deck_with(
            "island",
            &[
                "unsummon",
                "kopala,_warden_of_waves",
                "kopala,_warden_of_waves",
            ],
        ),
    ];
    let mut engine = GameEngine::new(57_004, &[0, 1], 20, Some(decks), true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);

    let first = relocate_to_battlefield(&mut engine, 1, "kopala,_warden_of_waves", false);
    relocate_to_battlefield(&mut engine, 1, "kopala,_warden_of_waves", false);
    ensure_in_hand(&mut engine, 0, "unsummon");
    let opponent_unsummon = hand_index_for_card(&engine, 0, "unsummon");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            c: 3,
            ..Default::default()
        },
    );
    engine
        .apply_command(0, &cast_spell(opponent_unsummon, target_object(first)))
        .expect_err("two Kopala sources should increase the opposing spell by {4}");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );
    engine
        .apply_command(0, &cast_spell(opponent_unsummon, target_object(first)))
        .expect("paying both Kopala applications should cast the spell");

    // Direct setup leaves the duplicate legendary permanents in place intentionally so this
    // test isolates static-source stacking. A fresh engine verifies the controller exemption.
    let own_decks = vec![
        deck_with("island", &[]),
        deck_with("island", &["unsummon", "kopala,_warden_of_waves"]),
    ];
    let mut own_engine =
        GameEngine::new(57_005, &[0, 1], 20, Some(own_decks), true).expect("new game");
    advance_to_main1_from_game_start(&mut own_engine);
    let own_kopala = relocate_to_battlefield(&mut own_engine, 1, "kopala,_warden_of_waves", false);
    ensure_in_hand(&mut own_engine, 1, "unsummon");
    give_mana(
        &mut own_engine,
        1,
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    let own_unsummon = hand_index_for_card(&own_engine, 1, "unsummon");
    own_engine
        .apply_command(1, &cast_spell(own_unsummon, target_object(own_kopala)))
        .expect("Kopala does not tax its controller");
}
