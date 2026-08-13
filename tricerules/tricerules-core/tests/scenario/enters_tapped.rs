use crate::helpers::*;

#[test]
fn tranquil_cove_enters_tapped_as_initial_entry_state() {
    let decks = Some(vec![
        vec!["tranquil_cove".into(); 7],
        vec!["forest".into(); 7],
    ]);
    let mut engine = GameEngine::new(50_001, &[0, 1], 20, decks, true).expect("new engine");
    advance_to_main1_from_game_start(&mut engine);

    let cove = hand_index_for_card(&engine, 0, "tranquil_cove");
    let played = engine
        .apply_command(0, &play_land(cove))
        .expect("play Tranquil Cove");

    let oid = battlefield_object_for_card(&engine, 0, "tranquil_cove");
    assert!(engine.state.objects[&oid].tapped);
    assert!(played.events.iter().any(|event| matches!(
        &event.ev,
        Some(Ev::StackPushed(stack)) if stack.is_triggered && stack.description == "Tranquil Cove"
    )));
    assert_eq!(engine.state.players[0].life, 20);
    pass_both_players(&mut engine);
    assert_eq!(engine.state.players[0].life, 21);
}

#[test]
fn orb_and_intrinsic_entry_replacement_ask_for_the_next_cr_616_effect() {
    let decks = Some(vec![
        vec![
            "orb_of_dreams".into(),
            "diregraf_ghoul".into(),
            "swamp".into(),
            "swamp".into(),
            "swamp".into(),
            "swamp".into(),
            "swamp".into(),
        ],
        vec!["forest".into(); 7],
    ]);
    let mut engine = GameEngine::new(50_002, &[0, 1], 20, decks, true).expect("new engine");
    advance_to_main1_from_game_start(&mut engine);

    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 3,
            ..Default::default()
        },
    );
    let orb = hand_index_for_card(&engine, 0, "orb_of_dreams");
    engine
        .apply_command(0, &cast_spell(orb, vec![]))
        .expect("cast Orb of Dreams");
    pass_both_players(&mut engine);
    let orb_oid = battlefield_object_for_card(&engine, 0, "orb_of_dreams");
    assert!(
        !engine.state.objects[&orb_oid].tapped,
        "Orb does not affect itself"
    );

    give_mana(
        &mut engine,
        0,
        ManaGift {
            b: 1,
            ..Default::default()
        },
    );
    let ghoul = hand_index_for_card(&engine, 0, "diregraf_ghoul");
    engine
        .apply_command(0, &cast_spell(ghoul, vec![]))
        .expect("cast Diregraf Ghoul");
    pass_both_players(&mut engine);

    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("two applicable entry replacements require a CR 616 choice");
    assert_eq!(pending.deciding_player, 0);
    assert_eq!(pending.choice_kind, ChoiceKind::ReplacementEffect);
    assert_eq!(pending.candidates.len(), 2);
    let application = pending.candidates[0];
    assert!(engine
        .apply_command(1, &submit_resolution_choice(vec![application]))
        .is_err());
    assert!(engine.state.pending_resolution.is_some());
    assert!(engine
        .apply_command(0, &submit_resolution_choice(vec![u32::MAX]))
        .is_err());
    assert!(engine.state.pending_resolution.is_some());
    engine
        .apply_command(0, &submit_resolution_choice(vec![application]))
        .expect("choose the next entry replacement");

    let ghoul_oid = battlefield_object_for_card(&engine, 0, "diregraf_ghoul");
    assert!(engine.state.objects[&ghoul_oid].tapped);
    assert!(engine.state.pending_resolution.is_none());
}

#[test]
fn reanimation_applies_intrinsic_entry_replacement_before_etb_triggers() {
    let decks = Some(vec![
        vec![
            "zombify".into(),
            "swamp".into(),
            "swamp".into(),
            "swamp".into(),
            "swamp".into(),
            "swamp".into(),
            "swamp".into(),
        ],
        vec!["forest".into(); 7],
    ]);
    let mut engine = GameEngine::new(50_003, &[0, 1], 20, decks, true).expect("new engine");
    advance_to_main1_from_game_start(&mut engine);
    let ghoul = inject_graveyard_card(&mut engine, 0, "diregraf_ghoul");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            b: 1,
            c: 3,
            ..Default::default()
        },
    );
    let zombify = hand_index_for_card(&engine, 0, "zombify");
    engine
        .apply_command(
            0,
            &cast_spell(
                zombify,
                vec![TargetRef {
                    object_id: ghoul,
                    damage_amount: 0,
                    group_index: 0,
                    kind: 0,
                }],
            ),
        )
        .expect("cast Zombify");
    pass_both_players(&mut engine);

    assert_eq!(
        engine.state.objects[&ghoul].zone,
        tricerules_core::Zone::Battlefield
    );
    assert!(engine.state.objects[&ghoul].tapped);
}

#[test]
fn token_batch_waits_for_every_replacement_choice_then_enters_simultaneously() {
    let decks = Some(vec![
        vec![
            "orb_of_dreams".into(),
            "orb_of_dreams".into(),
            "raise_the_alarm".into(),
            "plains".into(),
            "plains".into(),
            "plains".into(),
            "plains".into(),
        ],
        vec!["forest".into(); 7],
    ]);
    let mut engine = GameEngine::new(50_004, &[0, 1], 20, decks, true).expect("new engine");
    advance_to_main1_from_game_start(&mut engine);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 3,
            ..Default::default()
        },
    );
    let orb = hand_index_for_card(&engine, 0, "orb_of_dreams");
    engine
        .apply_command(0, &cast_spell(orb, vec![]))
        .expect("cast Orb");
    pass_both_players(&mut engine);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 3,
            ..Default::default()
        },
    );
    let second_orb = hand_index_for_card(&engine, 0, "orb_of_dreams");
    engine
        .apply_command(0, &cast_spell(second_orb, vec![]))
        .expect("cast second Orb");
    pass_both_players(&mut engine);

    give_mana(
        &mut engine,
        0,
        ManaGift {
            w: 1,
            c: 1,
            ..Default::default()
        },
    );
    let alarm = hand_index_for_card(&engine, 0, "raise_the_alarm");
    engine
        .apply_command(0, &cast_spell(alarm, vec![]))
        .expect("cast Raise the Alarm");
    pass_both_players(&mut engine);

    let soldiers_on_battlefield = |engine: &GameEngine| {
        engine
            .state
            .objects
            .values()
            .filter(|object| {
                object.card_id == "soldier_w_1_1"
                    && object.zone == tricerules_core::Zone::Battlefield
            })
            .count()
    };
    assert_eq!(soldiers_on_battlefield(&engine), 0);
    for _ in 0..2 {
        let pending = engine
            .state
            .pending_resolution
            .as_ref()
            .expect("each token has two Orb replacements to order");
        assert_eq!(pending.choice_kind, ChoiceKind::ReplacementEffect);
        assert_eq!(pending.candidates.len(), 2);
        let application = pending.candidates[0];
        engine
            .apply_command(0, &submit_resolution_choice(vec![application]))
            .expect("choose an Orb replacement");
    }
    assert_eq!(soldiers_on_battlefield(&engine), 2);

    let soldiers: Vec<_> = engine
        .state
        .objects
        .values()
        .filter(|object| {
            object.card_id == "soldier_w_1_1" && object.zone == tricerules_core::Zone::Battlefield
        })
        .collect();
    assert_eq!(soldiers.len(), 2);
    assert!(soldiers.iter().all(|soldier| soldier.tapped));
}

#[test]
fn graveyard_owner_orders_replacements_even_when_the_permanent_enters_under_opponent_control() {
    let decks = Some(vec![
        vec![
            "orb_of_dreams".into(),
            "reanimate".into(),
            "swamp".into(),
            "swamp".into(),
            "swamp".into(),
            "swamp".into(),
            "swamp".into(),
        ],
        vec!["forest".into(); 7],
    ]);
    let mut engine = GameEngine::new(50_005, &[0, 1], 20, decks, true).expect("new engine");
    advance_to_main1_from_game_start(&mut engine);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 3,
            ..Default::default()
        },
    );
    let orb = hand_index_for_card(&engine, 0, "orb_of_dreams");
    engine
        .apply_command(0, &cast_spell(orb, vec![]))
        .expect("cast Orb");
    pass_both_players(&mut engine);

    let ghoul = inject_graveyard_card(&mut engine, 1, "diregraf_ghoul");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            b: 1,
            ..Default::default()
        },
    );
    let reanimate = hand_index_for_card(&engine, 0, "reanimate");
    engine
        .apply_command(
            0,
            &cast_spell(
                reanimate,
                vec![TargetRef {
                    object_id: ghoul,
                    damage_amount: 0,
                    group_index: 0,
                    kind: 0,
                }],
            ),
        )
        .expect("cast Reanimate");
    pass_both_players(&mut engine);

    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("CR 616 choice");
    assert_eq!(
        pending.deciding_player, 1,
        "the graveyard card's owner decides"
    );
    let application = pending.candidates[0];
    assert!(engine
        .apply_command(0, &submit_resolution_choice(vec![application]))
        .is_err());
    engine
        .apply_command(1, &submit_resolution_choice(vec![application]))
        .expect("owner orders the replacement effects");

    assert_eq!(engine.state.objects[&ghoul].owner, 1);
    assert_eq!(engine.state.objects[&ghoul].controller, 0);
    assert!(engine.state.players[0].battlefield.contains(&ghoul));
    assert!(engine.state.objects[&ghoul].tapped);
}
