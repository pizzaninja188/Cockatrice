use super::helpers::*;
use tricerules_cards::CounterKind;

fn ensure_copies_in_hand(engine: &mut GameEngine, player: usize, card_id: &str, count: usize) {
    while engine.state.players[player]
        .hand
        .iter()
        .filter(|object_id| engine.state.objects[object_id].card_id == card_id)
        .count()
        < count
    {
        take_card_from_library_to_hand(engine, player, card_id);
    }
}

#[test]
fn second_committed_cast_triggers_flurry_exactly_once_for_that_player() {
    let decks = Some(vec![
        deck_with(
            "forest",
            &[
                "poised_practitioner",
                "life_goes_on",
                "life_goes_on",
                "life_goes_on",
            ],
        ),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(11101, &[0, 1], 20, decks, true).expect("new");
    let practitioner = relocate_to_battlefield(&mut engine, 0, "poised_practitioner", false);
    ensure_copies_in_hand(&mut engine, 0, "life_goes_on", 3);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            g: 3,
            ..Default::default()
        },
    );

    let first = hand_index_for_card(&engine, 0, "life_goes_on");
    engine
        .apply_command(0, &cast_spell(first, vec![]))
        .expect("cast first spell");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        engine.state.objects[&practitioner].counter_count(CounterKind::PlusOnePlusOne),
        0
    );

    let second = hand_index_for_card(&engine, 0, "life_goes_on");
    engine
        .apply_command(0, &cast_spell(second, vec![]))
        .expect("cast second spell");
    // The cast trigger is above Life Goes On, so the first pass cycle resolves flurry through
    // the counter instruction and parks at its trailing scry 1 choice.
    pass_both_players(&mut engine);

    assert_eq!(
        engine.state.objects[&practitioner].counter_count(CounterKind::PlusOnePlusOne),
        1
    );
    assert!(
        engine.state.pending_resolution.is_some(),
        "scry 1 must still be chosen"
    );
    assert_eq!(engine.state.turn_history.current.player(0).spells_cast, 2);
    assert_eq!(engine.state.turn_history.current.player(1).spells_cast, 0);

    engine
        .apply_command(0, &submit_resolution_choice(vec![]))
        .expect("keep the scried card on top");
    resolve_entire_stack_two_player(&mut engine);
    let third = hand_index_for_card(&engine, 0, "life_goes_on");
    engine
        .apply_command(0, &cast_spell(third, vec![]))
        .expect("cast third spell");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        engine.state.objects[&practitioner].counter_count(CounterKind::PlusOnePlusOne),
        1,
        "the third cast must not retrigger the second-cast ordinal"
    );
    assert_eq!(engine.state.turn_history.current.player(0).spells_cast, 3);
}

#[test]
fn successful_primitive_draws_are_ordinal_events_but_opening_cards_are_not() {
    let decks = Some(vec![
        deck_with("island", &["erudite_wizard", "divination"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(11102, &[0, 1], 20, decks, true).expect("new");
    assert_eq!(engine.state.turn_history.current.player(0).cards_drawn, 0);
    advance_to_main1_from_game_start(&mut engine);
    let wizard = relocate_to_battlefield(&mut engine, 0, "erudite_wizard", false);
    ensure_in_hand(&mut engine, 0, "divination");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            c: 2,
            ..Default::default()
        },
    );

    let divination = hand_index_for_card(&engine, 0, "divination");
    engine
        .apply_command(0, &cast_spell(divination, vec![]))
        .expect("cast Divination");
    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(engine.state.turn_history.current.player(0).cards_drawn, 2);
    assert_eq!(
        engine.state.objects[&wizard].counter_count(CounterKind::PlusOnePlusOne),
        1,
        "one trigger is created for the second successful draw"
    );
}

#[test]
fn erudite_wizard_only_needs_to_observe_the_second_draw() {
    let decks = Some(vec![
        deck_with(
            "forest",
            &["erudite_wizard", "elvish_visionary", "elvish_visionary"],
        ),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(11108, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut engine);
    ensure_copies_in_hand(&mut engine, 0, "elvish_visionary", 2);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            g: 2,
            c: 2,
            ..Default::default()
        },
    );

    let first = hand_index_for_card(&engine, 0, "elvish_visionary");
    engine
        .apply_command(0, &cast_spell(first, vec![]))
        .expect("cast first Visionary");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.turn_history.current.player(0).cards_drawn, 1);

    let wizard = relocate_to_battlefield(&mut engine, 0, "erudite_wizard", false);
    let second = hand_index_for_card(&engine, 0, "elvish_visionary");
    engine
        .apply_command(0, &cast_spell(second, vec![]))
        .expect("cast second Visionary");
    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(engine.state.turn_history.current.player(0).cards_drawn, 2);
    assert_eq!(
        engine.state.objects[&wizard].counter_count(CounterKind::PlusOnePlusOne),
        1,
        "the first draw may happen before Erudite Wizard enters"
    );
}

#[test]
fn normal_turn_draws_are_recorded_for_only_the_drawing_player() {
    let decks = Some(vec![deck_with("forest", &[]), deck_with("island", &[])]);
    let mut engine = GameEngine::new(11103, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut engine);
    assert_eq!(engine.state.turn_history.current.player(0).cards_drawn, 0);

    end_active_turn(&mut engine, 0);
    pass_both_players(&mut engine); // P1 upkeep ends; P1 takes the turn-based draw.

    assert_eq!(engine.state.turn_history.current.player(0).cards_drawn, 0);
    assert_eq!(engine.state.turn_history.current.player(1).cards_drawn, 1);
}

#[test]
fn failed_empty_library_draw_attempts_do_not_increment_draw_history() {
    let decks = Some(vec![
        deck_with("island", &["divination"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(11107, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut engine);
    ensure_in_hand(&mut engine, 0, "divination");
    engine.state.players[0].library.clear();
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            c: 2,
            ..Default::default()
        },
    );

    let divination = hand_index_for_card(&engine, 0, "divination");
    engine
        .apply_command(0, &cast_spell(divination, vec![]))
        .expect("cast Divination");
    pass_both_players(&mut engine);

    assert_eq!(engine.state.turn_history.current.player(0).cards_drawn, 0);
    assert!(engine.state.players[0].has_lost);
}

#[test]
fn only_a_committed_nonempty_declaration_sets_the_attack_fact() {
    let mut attacking = GameEngine::new(
        11104,
        &[0, 1],
        20,
        Some(vec![deck_with("forest", &[]), deck_with("forest", &[])]),
        true,
    )
    .expect("new");
    advance_to_declare_attackers(&mut attacking);
    let attacker = battlefield_object_for_card(&attacking, 0, "grizzly_bears");
    attacking
        .apply_command(0, &declare_attackers(vec![attacker]))
        .expect("declare attacker");
    assert!(attacking.state.turn_history.current.player(0).attacked);
    assert!(!attacking.state.turn_history.current.player(1).attacked);

    let mut empty = GameEngine::new(
        11105,
        &[0, 1],
        20,
        Some(vec![deck_with("forest", &[]), deck_with("forest", &[])]),
        true,
    )
    .expect("new");
    advance_to_declare_attackers(&mut empty);
    empty
        .apply_command(0, &declare_attackers(vec![]))
        .expect("declare no attackers");
    assert!(!empty.state.turn_history.current.player(0).attacked);
}

#[test]
fn focus_the_mind_reduction_reads_prior_casts_not_the_current_spell() {
    let decks = Some(vec![
        deck_with("island", &["focus_the_mind", "life_goes_on"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(11106, &[0, 1], 20, decks, true).expect("new");
    ensure_in_hand(&mut engine, 0, "focus_the_mind");
    ensure_in_hand(&mut engine, 0, "life_goes_on");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            g: 1,
            u: 1,
            c: 2,
            ..Default::default()
        },
    );

    let focus = hand_index_for_card(&engine, 0, "focus_the_mind");
    engine
        .apply_command(0, &cast_spell(focus, vec![]))
        .expect_err("Focus cannot discount itself as the first spell");
    assert_eq!(engine.state.turn_history.current.player(0).spells_cast, 0);

    let life = hand_index_for_card(&engine, 0, "life_goes_on");
    engine
        .apply_command(0, &cast_spell(life, vec![]))
        .expect("cast prior spell");
    resolve_entire_stack_two_player(&mut engine);

    let focus = hand_index_for_card(&engine, 0, "focus_the_mind");
    engine
        .apply_command(0, &cast_spell(focus, vec![]))
        .expect("cast discounted Focus");
    assert_eq!(engine.state.turn_history.current.player(0).spells_cast, 2);
}
