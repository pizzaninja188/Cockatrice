use super::helpers::{semantic::*, *};

#[test]
fn semantic_fixtures_registered_draw_pilot() {
    for case in [divination(), visionary(), pawpatch()] {
        exercise_draw(case).require_exercised();
    }
}

#[test]
#[should_panic(expected = "expected accepted command")]
fn semantic_fixtures_rejected_cast_fails() {
    let mut case = divination();
    case.mana = ManaGift::default();
    exercise_draw(case).require_exercised();
}

#[test]
#[should_panic(expected = "exact hand")]
fn semantic_fixtures_wrong_count_fails() {
    let mut case = divination();
    case.count = 1;
    exercise_draw(case).require_exercised();
}

#[test]
#[should_panic(expected = "exact hand")]
fn semantic_fixtures_wrong_recipient_fails() {
    let mut case = visionary();
    case.recipient = 1;
    exercise_draw(case).require_exercised();
}

#[test]
#[should_panic(expected = "selected mode identity")]
fn semantic_fixtures_wrong_mode_fails() {
    let mut case = pawpatch();
    case.surface = DrawSurface::Mode {
        index: 2,
        id: "mode_02",
    };
    exercise_draw(case).require_exercised();
}

fn cast_opt() -> GameEngine {
    let mut engine = main_phase(448_002);
    inject_card_into_hand(&mut engine, 0, "opt");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "opt");
    accepted(&mut engine, 0, &cast_spell(slot, vec![]));
    engine
}

#[test]
fn semantic_fixtures_exhausted_bound_is_blocked() {
    let mut engine = cast_opt();
    assert_eq!(
        complete(&mut engine, 1, |_| None),
        Evidence::FixtureBlocked("exhausted command bound 1".into())
    );
    assert_eq!(engine.state.stack.len(), 1);
}

#[test]
#[should_panic(expected = "semantic evidence was not exercised")]
fn semantic_fixtures_blocked_cannot_count_as_exercised() {
    complete(&mut cast_opt(), 0, |_| None).require_exercised();
}

#[test]
#[should_panic(expected = "semantic evidence was not exercised")]
fn semantic_fixtures_na_cannot_count_as_exercised() {
    Evidence::NotApplicable("untargeted draw has no target-selection contract").require_exercised();
}

#[test]
fn semantic_fixtures_explicit_choice_completes_draw() {
    let mut engine = cast_opt();
    let drawn = *engine.state.players[0].library.front().unwrap();
    let hand = engine.state.players[0].hand.len();
    let mut choices = 0;
    complete(&mut engine, 3, |e| {
        assert!(e.state.pending_resolution.is_some());
        choices += 1;
        Some((0, submit_resolution_choice(vec![])))
    })
    .require_exercised();
    assert_eq!(choices, 1);
    assert_eq!(engine.state.players[0].hand.len(), hand + 1);
    assert!(engine.state.players[0].hand.contains(&drawn));
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
}

#[test]
#[should_panic(expected = "expected accepted command")]
fn semantic_fixtures_unauthorized_choice_fails() {
    complete(&mut cast_opt(), 4, |_| {
        Some((1, submit_resolution_choice(vec![])))
    });
}

#[test]
fn semantic_fixtures_accepted_activation_completes() {
    let mut engine = main_phase(448_003);
    let food = inject_permanent_on_battlefield(&mut engine, 0, "food");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );
    let command = activate_ability_for(&engine, food, 0, vec![]);
    accepted(&mut engine, 0, &command);
    complete(&mut engine, 2, |_| None).require_exercised();
    assert_eq!(engine.state.players[0].life, 23);
    assert_eq!(engine.state.players[1].life, 20);
    assert!(!engine.state.players[0].battlefield.contains(&food));
}

#[test]
#[should_panic(expected = "expected accepted command")]
fn semantic_fixtures_rejected_activation_fails() {
    let mut engine = main_phase(448_004);
    let food = inject_permanent_on_battlefield(&mut engine, 0, "food");
    let command = activate_ability_for(&engine, food, 0, vec![]);
    accepted(&mut engine, 0, &command);
}

#[test]
fn semantic_fixtures_modal_illegal_targets_preserve_state() {
    let mut engine = main_phase(448_005);
    let source = inject_card_into_hand(&mut engine, 0, "pawpatch_formation");
    let bear = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            g: 1,
            c: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "pawpatch_formation");
    let revision = engine.state.command_index;
    // The other modes require a flying creature or an enchantment. The draw mode has no targets.
    for command in [
        cast_modal_spell(slot, vec![(0, target_object(bear))]),
        cast_modal_spell(slot, vec![(1, target_object(bear))]),
        cast_modal_spell(slot, vec![(3, vec![])]),
    ] {
        assert!(engine.apply_command(0, &command).is_err());
        assert_eq!(engine.state.command_index, revision);
        assert_object(
            &engine,
            source,
            "pawpatch_formation",
            0,
            0,
            tricerules_core::Zone::Hand,
            0,
        );
        assert!(engine.state.stack.is_empty());
        assert_eq!(engine.state.players[0].mana_pool.green, 1);
        assert_eq!(engine.state.players[0].mana_pool.colorless, 1);
    }
    accepted(&mut engine, 0, &cast_modal_spell(slot, vec![(2, vec![])]));
    complete(&mut engine, 2, |_| None).require_exercised();
    assert_eq!(
        engine.state.objects[&bear].zone,
        tricerules_core::Zone::Battlefield
    );
    assert_eq!(battlefield_token_oids(&engine, 0, "food").len(), 1);
}

#[test]
fn semantic_fixtures_drain_uses_all_priority_holders() {
    let mut engine = GameEngine::new(
        448_006,
        &[7, 19],
        20,
        Some(vec![deck_with("island", &[]); 2]),
        true,
    )
    .unwrap();
    // Instant cast in upkeep avoids the existing two-player main-phase setup helper.
    inject_card_into_hand(&mut engine, 0, "reach_through_mists");
    give_mana(
        &mut engine,
        7,
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "reach_through_mists");
    accepted(&mut engine, 7, &cast_spell(slot, vec![]));
    let hand = engine.state.players[0].hand.len();
    complete(&mut engine, 2, |_| None).require_exercised();
    assert_eq!(engine.state.players[0].hand.len(), hand + 1);
    assert_eq!(engine.state.priority_player_id(), 7);
}

#[test]
fn semantic_fixtures_parked_resolution_is_not_exercised() {
    let mut engine = GameEngine::new(
        448,
        &[0, 1],
        20,
        Some(vec![deck_with("island", &[]), deck_with("forest", &[])]),
        true,
    )
    .unwrap();
    advance_to_main1_from_game_start(&mut engine);
    inject_card_into_hand(&mut engine, 0, "opt");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "opt");
    engine.apply_command(0, &cast_spell(slot, vec![])).unwrap();
    pass_both_players(&mut engine);
    assert!(engine.state.stack.is_empty());
    assert!(engine.state.pending_resolution.is_some());
    assert!(matches!(
        complete(&mut engine, 8, |_| None),
        Evidence::FixtureBlocked(_)
    ));
}
