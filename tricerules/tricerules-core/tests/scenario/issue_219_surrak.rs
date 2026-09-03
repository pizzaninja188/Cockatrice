use super::helpers::*;
use tricerules_core::Zone;

fn setup_surrak_and_counterspell(seed: u64) -> GameEngine {
    let decks = Some(vec![
        deck_with("forest", &["surrak,_elusive_hunter", "grizzly_bears"]),
        deck_with("island", &["counterspell"]),
    ]);
    let mut engine = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    give_mana(
        &mut engine,
        1,
        ManaGift {
            u: 2,
            ..Default::default()
        },
    );
    engine
}

#[test]
fn opponent_counterspell_targeting_a_controlled_creature_spell_triggers_surrak() {
    let mut engine = setup_surrak_and_counterspell(219_001);
    relocate_to_battlefield(&mut engine, 0, "surrak,_elusive_hunter", false);
    ensure_in_hand(&mut engine, 0, "grizzly_bears");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            g: 1,
            c: 1,
            ..Default::default()
        },
    );
    engine
        .apply_command(
            0,
            &cast_spell(hand_index_for_card(&engine, 0, "grizzly_bears"), vec![]),
        )
        .expect("cast creature spell");
    let creature_spell = engine.state.stack.last().expect("creature spell").id;
    engine.apply_command(0, &pass()).expect("pass to opponent");
    engine
        .apply_command(
            1,
            &cast_spell(
                hand_index_for_card(&engine, 1, "counterspell"),
                target_object(creature_spell),
            ),
        )
        .expect("cast Counterspell");

    assert_eq!(engine.state.stack.len(), 3);
    let trigger = engine.state.stack.last().expect("Surrak trigger");
    assert!(trigger.is_triggered);
    assert_eq!(
        trigger.ability_text.as_deref(),
        Some("Surrak, Elusive Hunter — triggered ability (triggered_02)")
    );

    let hand_before = engine.state.players[0].hand.len();
    pass_both_players(&mut engine);
    assert_eq!(engine.state.players[0].hand.len(), hand_before + 1);
    assert!(engine
        .state
        .stack
        .iter()
        .any(|item| item.id == creature_spell));
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&creature_spell].zone, Zone::Graveyard);
}

#[test]
fn counterspell_targets_but_cannot_counter_surrak() {
    let mut engine = setup_surrak_and_counterspell(219_002);
    ensure_in_hand(&mut engine, 0, "surrak,_elusive_hunter");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            g: 1,
            c: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "surrak,_elusive_hunter");
    let surrak = engine.state.players[0].hand[slot];
    engine
        .apply_command(0, &cast_spell(slot, vec![]))
        .expect("cast Surrak");
    engine.apply_command(0, &pass()).expect("pass to opponent");
    engine
        .apply_command(
            1,
            &cast_spell(
                hand_index_for_card(&engine, 1, "counterspell"),
                target_object(surrak),
            ),
        )
        .expect("Counterspell can target Surrak");
    assert_eq!(
        engine.state.stack.len(),
        2,
        "Surrak's trigger is not active while Surrak is only a spell"
    );
    engine
        .apply_command(1, &pass())
        .expect("counter caster passes");
    let batch = engine
        .apply_command(0, &pass())
        .expect("Counterspell resolves without countering Surrak");

    assert!(!batch.events.iter().any(|event| matches!(
        &event.ev,
        Some(Ev::StackObjectCountered(countered)) if countered.object_id == surrak
    )));
    assert!(!batch.events.iter().any(|event| matches!(
        &event.ev,
        Some(Ev::PermanentMoved(moved)) if moved.object_id == surrak
    )));
    assert!(batch.events.iter().any(|event| matches!(
        &event.ev,
        Some(Ev::Log(log)) if log.text == "Counterspell cannot counter Surrak, Elusive Hunter"
    )));
    assert!(engine.state.stack.iter().any(|item| item.id == surrak));

    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&surrak].zone, Zone::Battlefield);
    assert!(engine.state.players[0].battlefield.contains(&surrak));
}

#[test]
fn soft_counter_still_offers_payment_before_failing_to_counter_surrak() {
    let decks = Some(vec![
        deck_with("forest", &["surrak,_elusive_hunter"]),
        deck_with("island", &["convolute"]),
    ]);
    let mut engine = GameEngine::new(219_003, &[0, 1], 20, decks, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    ensure_in_hand(&mut engine, 0, "surrak,_elusive_hunter");
    ensure_in_hand(&mut engine, 1, "convolute");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            g: 1,
            c: 2,
            ..Default::default()
        },
    );
    give_mana(
        &mut engine,
        1,
        ManaGift {
            u: 1,
            c: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "surrak,_elusive_hunter");
    let surrak = engine.state.players[0].hand[slot];
    engine
        .apply_command(0, &cast_spell(slot, vec![]))
        .expect("cast Surrak");
    engine.apply_command(0, &pass()).expect("pass to opponent");
    engine
        .apply_command(
            1,
            &cast_spell(
                hand_index_for_card(&engine, 1, "convolute"),
                target_object(surrak),
            ),
        )
        .expect("cast Convolute");
    engine
        .apply_command(1, &pass())
        .expect("Convolute caster passes");
    let parked = engine
        .apply_command(0, &pass())
        .expect("offer soft-counter payment");
    let choice = find_resolution_choice(&parked).expect("Convolute payment choice");
    assert_eq!(choice.deciding_player_id, 0);
    assert_eq!(choice.generic_mana_cost, 4);

    let batch = engine
        .apply_command(
            0,
            &submit_resolution_decision(
                tricerules_proto::ruled::v1::ResolutionChoiceDecision::Decline,
            ),
        )
        .expect("decline Convolute payment");
    assert!(!batch.events.iter().any(|event| matches!(
        &event.ev,
        Some(Ev::StackObjectCountered(countered)) if countered.object_id == surrak
    )));
    assert!(batch.events.iter().any(|event| matches!(
        &event.ev,
        Some(Ev::Log(log)) if log.text == "Convolute cannot counter Surrak, Elusive Hunter"
    )));
    assert!(engine.state.stack.iter().any(|item| item.id == surrak));

    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&surrak].zone, Zone::Battlefield);
}

#[test]
fn surrak_spell_watcher_rejects_own_targeting_and_noncreature_spells() {
    let decks = Some(vec![
        deck_with(
            "forest",
            &["surrak,_elusive_hunter", "grizzly_bears", "counterspell"],
        ),
        deck_with("island", &[]),
    ]);
    let mut own = GameEngine::new(219_004, &[0, 1], 20, decks, true).expect("new game");
    advance_to_main1_from_game_start(&mut own);
    relocate_to_battlefield(&mut own, 0, "surrak,_elusive_hunter", false);
    ensure_in_hand(&mut own, 0, "grizzly_bears");
    ensure_in_hand(&mut own, 0, "counterspell");
    give_mana(
        &mut own,
        0,
        ManaGift {
            g: 1,
            u: 2,
            c: 1,
            ..Default::default()
        },
    );
    own.apply_command(
        0,
        &cast_spell(hand_index_for_card(&own, 0, "grizzly_bears"), vec![]),
    )
    .expect("cast Bear");
    let bear = own.state.stack.last().expect("Bear spell").id;
    own.apply_command(
        0,
        &cast_spell(
            hand_index_for_card(&own, 0, "counterspell"),
            target_object(bear),
        ),
    )
    .expect("target own creature spell");
    assert_eq!(
        own.state.stack.len(),
        2,
        "own targeting must not trigger Surrak"
    );

    let decks = Some(vec![
        deck_with("forest", &["surrak,_elusive_hunter", "lightning_bolt"]),
        deck_with("island", &["counterspell"]),
    ]);
    let mut noncreature = GameEngine::new(219_005, &[0, 1], 20, decks, true).expect("new game");
    advance_to_main1_from_game_start(&mut noncreature);
    relocate_to_battlefield(&mut noncreature, 0, "surrak,_elusive_hunter", false);
    ensure_in_hand(&mut noncreature, 0, "lightning_bolt");
    ensure_in_hand(&mut noncreature, 1, "counterspell");
    give_mana(
        &mut noncreature,
        0,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    give_mana(
        &mut noncreature,
        1,
        ManaGift {
            u: 2,
            ..Default::default()
        },
    );
    noncreature
        .apply_command(
            0,
            &cast_spell(
                hand_index_for_card(&noncreature, 0, "lightning_bolt"),
                target_player(1),
            ),
        )
        .expect("cast Bolt");
    let bolt = noncreature.state.stack.last().expect("Bolt spell").id;
    noncreature
        .apply_command(0, &pass())
        .expect("pass to opponent");
    noncreature
        .apply_command(
            1,
            &cast_spell(
                hand_index_for_card(&noncreature, 1, "counterspell"),
                target_object(bolt),
            ),
        )
        .expect("target noncreature spell");
    assert_eq!(
        noncreature.state.stack.len(),
        2,
        "the creature spell filter must reject Bolt"
    );
}

#[test]
fn surrak_spell_watcher_rejects_an_opponents_creature_spell() {
    let decks = Some(vec![
        deck_with("forest", &["surrak,_elusive_hunter"]),
        deck_with("island", &["ambush_viper", "counterspell"]),
    ]);
    let mut engine = GameEngine::new(219_006, &[0, 1], 20, decks, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    relocate_to_battlefield(&mut engine, 0, "surrak,_elusive_hunter", false);
    ensure_in_hand(&mut engine, 1, "ambush_viper");
    ensure_in_hand(&mut engine, 1, "counterspell");
    give_mana(
        &mut engine,
        1,
        ManaGift {
            g: 1,
            u: 2,
            c: 1,
            ..Default::default()
        },
    );
    engine.apply_command(0, &pass()).expect("offer priority");
    engine
        .apply_command(
            1,
            &cast_spell(hand_index_for_card(&engine, 1, "ambush_viper"), vec![]),
        )
        .expect("cast flash creature");
    let viper = engine.state.stack.last().expect("Viper spell").id;
    engine
        .apply_command(
            1,
            &cast_spell(
                hand_index_for_card(&engine, 1, "counterspell"),
                target_object(viper),
            ),
        )
        .expect("opponent targets their own creature spell");
    assert_eq!(
        engine.state.stack.len(),
        2,
        "Surrak only watches creature spells its controller controls"
    );
}
