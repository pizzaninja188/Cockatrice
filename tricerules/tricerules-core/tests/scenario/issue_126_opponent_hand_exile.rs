use crate::helpers::*;
use tricerules_cards::primitives::CounterKind;
use tricerules_core::Zone;

fn aggressive_negotiations_targets(opponent: i32, creature: Option<u32>) -> Vec<TargetRef> {
    let mut targets = vec![TargetRef {
        object_id: opponent as u32,
        group_index: 0,
        kind: TargetRefKind::Player as i32,
        ..Default::default()
    }];
    if let Some(creature) = creature {
        targets.push(TargetRef {
            object_id: creature,
            group_index: 1,
            kind: TargetRefKind::Permanent as i32,
            ..Default::default()
        });
    }
    targets
}

#[test]
fn issue_143_aggressive_negotiations_publicly_reveals_the_hand() {
    let decks = Some(vec![
        deck_with("swamp", &["aggressive_negotiations", "grizzly_bears"]),
        deck_with("forest", &["grizzly_bears"]),
    ]);
    let mut engine = GameEngine::new(126_001, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut engine);

    let counter_target = relocate_to_battlefield(&mut engine, 0, "grizzly_bears", false);
    let chosen_nonland = relocate_to_hand(&mut engine, 1, "grizzly_bears");
    let ineligible_land = relocate_to_hand(&mut engine, 1, "forest");
    relocate_to_hand(&mut engine, 0, "aggressive_negotiations");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            b: 1,
            c: 2,
            ..Default::default()
        },
    );

    let spell = hand_index_for_card(&engine, 0, "aggressive_negotiations");
    engine
        .apply_command(
            0,
            &cast_spell(
                spell,
                aggressive_negotiations_targets(1, Some(counter_target)),
            ),
        )
        .expect("cast Aggressive Negotiations");
    engine.apply_command(0, &pass()).expect("caster passes");
    let parked = engine
        .apply_command(1, &pass())
        .expect("opponent passes and resolution parks");

    let choice = find_resolution_choice(&parked).expect("opponent-hand choice");
    assert_eq!(choice.choice_kind(), ChoiceKind::OpponentHand);
    assert_eq!(
        choice.reveal_audience(),
        ResolutionRevealAudience::AllParticipants
    );
    assert_eq!(choice.revealed_zone_owner_player_id, Some(1));
    assert_eq!(choice.deciding_player_id, 0);
    assert_eq!(choice.min, 1);
    assert_eq!(choice.max, 1);
    let chosen_index = choice
        .candidate_object_ids
        .iter()
        .position(|object_id| *object_id == chosen_nonland)
        .expect("nonland candidate");
    let land_index = choice
        .candidate_object_ids
        .iter()
        .position(|object_id| *object_id == ineligible_land)
        .expect("land remains visible");
    assert!(choice.candidate_selectable[chosen_index]);
    assert!(!choice.candidate_selectable[land_index]);
    assert!(matches!(
        &engine
            .state
            .pending_resolution
            .as_ref()
            .expect("pending hand choice")
            .continuation,
        ResolutionContinuation::HandChoice {
            hand_choice,
            ..
        } if hand_choice.action == HandCardAction::Exile
    ));

    let generation_before = engine
        .state
        .zone_change_generation
        .get(&chosen_nonland)
        .copied()
        .unwrap_or(0);
    engine
        .apply_command(0, &submit_resolution_choice(vec![chosen_nonland]))
        .expect("choose the nonland card");

    assert_eq!(engine.state.objects[&chosen_nonland].zone, Zone::Exile);
    assert_eq!(
        engine.state.zone_change_generation[&chosen_nonland],
        generation_before + 1
    );
    assert_eq!(
        engine.state.objects[&counter_target].counter_count(CounterKind::PlusOnePlusOne),
        1,
        "the effect tail resumes after the hand choice"
    );
}

#[test]
fn aggressive_negotiations_allows_omitting_the_creature_target() {
    let decks = Some(vec![
        deck_with("swamp", &["aggressive_negotiations"]),
        deck_with("forest", &["grizzly_bears"]),
    ]);
    let mut engine = GameEngine::new(126_002, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut engine);
    let chosen = relocate_to_hand(&mut engine, 1, "grizzly_bears");
    relocate_to_hand(&mut engine, 0, "aggressive_negotiations");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            b: 1,
            c: 2,
            ..Default::default()
        },
    );

    let spell = hand_index_for_card(&engine, 0, "aggressive_negotiations");
    engine
        .apply_command(
            0,
            &cast_spell(spell, aggressive_negotiations_targets(1, None)),
        )
        .expect("cast without the optional target");
    engine.apply_command(0, &pass()).expect("caster passes");
    engine.apply_command(1, &pass()).expect("resolution parks");
    engine
        .apply_command(0, &submit_resolution_choice(vec![chosen]))
        .expect("choose the nonland card");

    assert_eq!(engine.state.objects[&chosen].zone, Zone::Exile);
}

#[test]
fn aggressive_negotiations_all_land_hand_skips_the_choice_and_resumes_the_tail() {
    let decks = Some(vec![
        deck_with("swamp", &["aggressive_negotiations", "grizzly_bears"]),
        vec!["forest".into(); 20],
    ]);
    let mut engine = GameEngine::new(126_003, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut engine);
    let counter_target = relocate_to_battlefield(&mut engine, 0, "grizzly_bears", false);
    relocate_to_hand(&mut engine, 0, "aggressive_negotiations");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            b: 1,
            c: 2,
            ..Default::default()
        },
    );

    let spell = hand_index_for_card(&engine, 0, "aggressive_negotiations");
    engine
        .apply_command(
            0,
            &cast_spell(
                spell,
                aggressive_negotiations_targets(1, Some(counter_target)),
            ),
        )
        .expect("cast Aggressive Negotiations");
    engine.apply_command(0, &pass()).expect("caster passes");
    let resolved = engine.apply_command(1, &pass()).expect("spell resolves");

    assert!(find_resolution_choice(&resolved).is_none());
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(
        engine.state.objects[&counter_target].counter_count(CounterKind::PlusOnePlusOne),
        1
    );
}

#[test]
fn aggressive_negotiations_keeps_the_hand_effect_when_the_optional_target_is_illegal() {
    let decks = Some(vec![
        deck_with("swamp", &["aggressive_negotiations", "grizzly_bears"]),
        deck_with("forest", &["grizzly_bears"]),
    ]);
    let mut engine = GameEngine::new(126_004, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut engine);
    let departed = relocate_to_battlefield(&mut engine, 0, "grizzly_bears", false);
    let chosen = relocate_to_hand(&mut engine, 1, "grizzly_bears");
    relocate_to_hand(&mut engine, 0, "aggressive_negotiations");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            b: 1,
            c: 2,
            ..Default::default()
        },
    );

    let spell = hand_index_for_card(&engine, 0, "aggressive_negotiations");
    engine
        .apply_command(
            0,
            &cast_spell(spell, aggressive_negotiations_targets(1, Some(departed))),
        )
        .expect("cast Aggressive Negotiations");
    engine.state.players[0]
        .battlefield
        .retain(|object_id| *object_id != departed);
    engine.state.players[0].hand.push(departed);
    engine
        .state
        .objects
        .get_mut(&departed)
        .expect("departed")
        .zone = Zone::Hand;
    *engine
        .state
        .zone_change_generation
        .entry(departed)
        .or_default() += 1;

    engine.apply_command(0, &pass()).expect("caster passes");
    engine.apply_command(1, &pass()).expect("resolution parks");
    engine
        .apply_command(0, &submit_resolution_choice(vec![chosen]))
        .expect("hand effect still resolves");

    assert_eq!(engine.state.objects[&chosen].zone, Zone::Exile);
    assert_eq!(
        engine.state.objects[&departed].counter_count(CounterKind::PlusOnePlusOne),
        0
    );
}

#[test]
fn aggressive_negotiations_rejects_ineligible_and_stale_choices_atomically() {
    let decks = Some(vec![
        deck_with("swamp", &["aggressive_negotiations"]),
        deck_with("forest", &["grizzly_bears"]),
    ]);
    let mut engine = GameEngine::new(126_005, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut engine);
    let chosen = relocate_to_hand(&mut engine, 1, "grizzly_bears");
    let land = relocate_to_hand(&mut engine, 1, "forest");
    relocate_to_hand(&mut engine, 0, "aggressive_negotiations");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            b: 1,
            c: 2,
            ..Default::default()
        },
    );
    let spell = hand_index_for_card(&engine, 0, "aggressive_negotiations");
    engine
        .apply_command(
            0,
            &cast_spell(spell, aggressive_negotiations_targets(1, None)),
        )
        .expect("cast Aggressive Negotiations");
    engine.apply_command(0, &pass()).expect("caster passes");
    engine.apply_command(1, &pass()).expect("resolution parks");

    let hand_before = engine.state.players[1].hand.clone();
    let wrong_owner = *engine.state.players[0]
        .hand
        .first()
        .expect("caster still has a hand card");
    assert!(engine
        .apply_command(0, &submit_resolution_choice(vec![wrong_owner]))
        .is_err());
    assert_eq!(engine.state.players[1].hand, hand_before);
    assert!(engine.state.pending_resolution.is_some());

    {
        let pending = engine
            .state
            .pending_resolution
            .as_mut()
            .expect("hand choice remains pending");
        pending.presentation.min = 2;
        pending.presentation.max = 2;
    }
    assert!(engine
        .apply_command(0, &submit_resolution_choice(vec![chosen, chosen]))
        .is_err());
    assert_eq!(engine.state.players[1].hand, hand_before);
    assert_eq!(engine.state.objects[&chosen].zone, Zone::Hand);
    {
        let pending = engine
            .state
            .pending_resolution
            .as_mut()
            .expect("duplicate rejection preserves the hand choice");
        pending.presentation.min = 1;
        pending.presentation.max = 1;
    }

    assert!(engine
        .apply_command(0, &submit_resolution_choice(vec![land]))
        .is_err());
    assert_eq!(engine.state.players[1].hand, hand_before);
    assert_eq!(engine.state.objects[&chosen].zone, Zone::Hand);
    assert!(engine.state.pending_resolution.is_some());

    *engine
        .state
        .zone_change_generation
        .entry(chosen)
        .or_default() += 1;
    assert!(engine
        .apply_command(0, &submit_resolution_choice(vec![chosen]))
        .is_err());
    assert_eq!(engine.state.players[1].hand, hand_before);
    assert_eq!(engine.state.objects[&chosen].zone, Zone::Hand);
    assert!(engine.state.pending_resolution.is_some());
}
