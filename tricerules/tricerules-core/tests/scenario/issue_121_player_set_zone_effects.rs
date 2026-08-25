use super::helpers::*;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::ChoiceKind;

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![
        std::iter::repeat_n("forest".to_string(), 20).collect(),
        std::iter::repeat_n("island".to_string(), 20).collect(),
    ]);
    let mut engine = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("new engine");
    advance_to_main1_from_game_start(&mut engine);
    for player in 0..2 {
        let cleared: Vec<_> = engine.state.players[player].hand.drain(..).collect();
        engine.state.players[player].library.extend(cleared);
    }
    engine
}

fn cast_fanatic_and_resolve_creature(engine: &mut GameEngine) {
    inject_card_into_hand(engine, 0, "fanatic_of_the_harrowing");
    give_mana(
        engine,
        0,
        ManaGift {
            b: 1,
            c: 3,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(engine, 0, "fanatic_of_the_harrowing");
    engine
        .apply_command(0, &cast_spell(slot, Vec::new()))
        .expect("cast Fanatic of the Harrowing");
    pass_both_players(engine);
    assert_eq!(engine.state.stack.len(), 1, "ETB trigger is on the stack");
}

fn cast_creature_and_leave_etb_trigger(engine: &mut GameEngine, card_id: &str, mana: ManaGift) {
    inject_card_into_hand(engine, 0, card_id);
    give_mana(engine, 0, mana);
    let slot = hand_index_for_card(engine, 0, card_id);
    engine
        .apply_command(0, &cast_spell(slot, Vec::new()))
        .unwrap_or_else(|error| panic!("cast {card_id}: {error}"));
    pass_both_players(engine);
    assert_eq!(engine.state.stack.len(), 1, "ETB trigger is on the stack");
}

fn resolve_trigger_to_choice(engine: &mut GameEngine) -> RuledEventBatch {
    engine
        .apply_command(0, &pass())
        .expect("active player passes");
    engine
        .apply_command(1, &pass())
        .expect("resolve trigger to a choice")
}

#[test]
fn fanatic_collects_private_apnap_choices_before_committing_the_discard() {
    let mut engine = engine(121_101);
    let controller_card = inject_card_into_hand(&mut engine, 0, "forest");
    let opponent_card = inject_card_into_hand(&mut engine, 1, "island");
    cast_fanatic_and_resolve_creature(&mut engine);

    engine
        .apply_command(0, &pass())
        .expect("active player passes");
    let first = engine
        .apply_command(1, &pass())
        .expect("resolve trigger to first private choice");
    let first_choice = find_resolution_choice(&first).expect("active player's discard choice");
    assert_eq!(first_choice.deciding_player_id, 0);
    assert_eq!(first_choice.choice_kind, ChoiceKind::HandCards as i32);
    assert_eq!((first_choice.min, first_choice.max), (1, 1));
    assert!(first_choice.candidate_object_ids.contains(&controller_card));
    assert!(!first_choice.candidate_object_ids.contains(&opponent_card));

    let second = engine
        .apply_command(0, &submit_resolution_choice(vec![controller_card]))
        .expect("record active player's hidden choice");
    assert_eq!(engine.state.objects[&controller_card].zone, Zone::Hand);
    assert_eq!(engine.state.objects[&opponent_card].zone, Zone::Hand);
    let second_choice = find_resolution_choice(&second).expect("next player's discard choice");
    assert_eq!(second_choice.deciding_player_id, 1);
    assert!(second_choice.candidate_object_ids.contains(&opponent_card));
    assert!(!second_choice
        .candidate_object_ids
        .contains(&controller_card));

    engine
        .apply_command(1, &submit_resolution_choice(vec![opponent_card]))
        .expect("commit the complete discard action");
    assert_eq!(engine.state.objects[&controller_card].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&opponent_card].zone, Zone::Graveyard);
    assert_eq!(
        engine.state.players[0].hand.len(),
        1,
        "Fanatic controller drew"
    );
    assert!(engine.state.pending_resolution.is_none());
}

#[test]
fn fanatic_does_not_draw_when_only_the_opponent_discarded() {
    let mut engine = engine(121_102);
    let opponent_card = inject_card_into_hand(&mut engine, 1, "island");
    cast_fanatic_and_resolve_creature(&mut engine);

    let batch = resolve_trigger_to_choice(&mut engine);
    let choice = find_resolution_choice(&batch).expect("opponent discard choice");
    assert_eq!(choice.deciding_player_id, 1);
    engine
        .apply_command(1, &submit_resolution_choice(vec![opponent_card]))
        .expect("opponent discards");

    assert!(engine.state.players[0].hand.is_empty());
    assert_eq!(engine.state.objects[&opponent_card].zone, Zone::Graveyard);
}

#[test]
fn player_set_discard_rejects_wrong_player_cardinality_and_stale_identity() {
    let mut engine = engine(121_103);
    let controller_card = inject_card_into_hand(&mut engine, 0, "forest");
    inject_card_into_hand(&mut engine, 1, "island");
    cast_fanatic_and_resolve_creature(&mut engine);
    resolve_trigger_to_choice(&mut engine);

    assert!(engine
        .apply_command(1, &submit_resolution_choice(vec![controller_card]))
        .is_err());
    assert!(engine
        .apply_command(0, &submit_resolution_choice(Vec::new()))
        .is_err());
    *engine
        .state
        .zone_change_generation
        .entry(controller_card)
        .or_default() += 1;
    let error = engine
        .apply_command(0, &submit_resolution_choice(vec![controller_card]))
        .expect_err("stale physical identity must fail closed");
    assert!(error.to_string().contains("stale"));
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .deciding_player,
        0
    );
    assert_eq!(engine.state.objects[&controller_card].zone, Zone::Hand);
}

#[test]
fn conceding_during_the_second_hidden_choice_commits_no_staged_discard() {
    let mut engine = engine(121_104);
    let controller_card = inject_card_into_hand(&mut engine, 0, "forest");
    let opponent_card = inject_card_into_hand(&mut engine, 1, "island");
    cast_fanatic_and_resolve_creature(&mut engine);
    resolve_trigger_to_choice(&mut engine);
    engine
        .apply_command(0, &submit_resolution_choice(vec![controller_card]))
        .expect("stage the first choice");

    engine
        .apply_command(1, &concede())
        .expect("concede at any time");
    assert_eq!(engine.state.winner, Some(0));
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(engine.state.objects[&controller_card].zone, Zone::Hand);
    assert_eq!(engine.state.objects[&opponent_card].zone, Zone::Hand);
}

#[test]
fn burglar_rat_discards_only_from_each_opponent() {
    let mut engine = engine(121_110);
    let controller_card = inject_card_into_hand(&mut engine, 0, "forest");
    let opponent_card = inject_card_into_hand(&mut engine, 1, "island");
    cast_creature_and_leave_etb_trigger(
        &mut engine,
        "burglar_rat",
        ManaGift {
            b: 1,
            c: 1,
            ..Default::default()
        },
    );

    let batch = resolve_trigger_to_choice(&mut engine);
    let choice = find_resolution_choice(&batch).expect("opponent discard choice");
    assert_eq!(choice.deciding_player_id, 1);
    assert!(choice.candidate_object_ids.contains(&opponent_card));
    assert!(!choice.candidate_object_ids.contains(&controller_card));
    engine
        .apply_command(1, &submit_resolution_choice(vec![opponent_card]))
        .expect("opponent discards");
    assert_eq!(engine.state.objects[&opponent_card].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&controller_card].zone, Zone::Hand);
}

#[test]
fn friendly_teddy_death_trigger_draws_for_each_player() {
    let mut engine = engine(121_120);
    inject_card_into_hand(&mut engine, 0, "friendly_teddy");
    let teddy = relocate_to_battlefield(&mut engine, 0, "friendly_teddy", false);
    let hands_before = [
        engine.state.players[0].hand.len(),
        engine.state.players[1].hand.len(),
    ];
    engine.state.objects.get_mut(&teddy).unwrap().damage = 99;
    engine
        .apply_command(0, &pass())
        .expect("SBA destroys Teddy");
    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(engine.state.objects[&teddy].zone, Zone::Graveyard);
    assert_eq!(engine.state.players[0].hand.len(), hands_before[0] + 1);
    assert_eq!(engine.state.players[1].hand.len(), hands_before[1] + 1);
}

#[test]
fn macabre_waltz_discards_with_zero_targets() {
    let mut engine = engine(121_130);
    let discard = inject_card_into_hand(&mut engine, 0, "forest");
    inject_card_into_hand(&mut engine, 0, "macabre_waltz");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            b: 1,
            c: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "macabre_waltz");
    engine
        .apply_command(0, &cast_spell(slot, Vec::new()))
        .expect("cast with zero targets");
    let batch = resolve_trigger_to_choice(&mut engine);
    let choice = find_resolution_choice(&batch).expect("mandatory self-discard");
    assert_eq!(choice.candidate_object_ids, vec![discard]);
    engine
        .apply_command(0, &submit_resolution_choice(vec![discard]))
        .expect("discard despite choosing zero targets");
    assert_eq!(engine.state.objects[&discard].zone, Zone::Graveyard);
}

#[test]
fn macabre_waltz_can_discard_the_creature_it_returned() {
    let mut engine = engine(121_131);
    let returned = inject_graveyard_card(&mut engine, 0, "grizzly_bears");
    inject_card_into_hand(&mut engine, 0, "macabre_waltz");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            b: 1,
            c: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "macabre_waltz");
    engine
        .apply_command(0, &cast_spell(slot, target_object(returned)))
        .expect("cast targeting one creature card");
    let batch = resolve_trigger_to_choice(&mut engine);
    let choice = find_resolution_choice(&batch).expect("discard after return");
    assert!(choice.candidate_object_ids.contains(&returned));
    engine
        .apply_command(0, &submit_resolution_choice(vec![returned]))
        .expect("discard the returned creature");
    assert_eq!(engine.state.objects[&returned].zone, Zone::Graveyard);
}
