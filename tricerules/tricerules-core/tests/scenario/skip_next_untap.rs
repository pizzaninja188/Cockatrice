//! CR 502.3 / Issue #93: one-shot effects that keep a permanent from untapping during its
//! controller's next untap step.

use crate::helpers::*;

fn target(oid: u32) -> Vec<TargetRef> {
    vec![TargetRef {
        object_id: oid,
        damage_amount: 0,
    }]
}

fn cast_crippling_chill(engine: &mut GameEngine, caster: i32, target_id: u32) {
    ensure_in_hand(engine, caster as usize, "crippling_chill");
    give_mana(
        engine,
        caster,
        ManaGift {
            u: 3,
            ..Default::default()
        },
    );
    let chill = hand_index_for_card(engine, caster as usize, "crippling_chill");
    engine
        .apply_command(caster, &cast_spell(chill, target(target_id)))
        .expect("cast Crippling Chill");
    resolve_entire_stack_two_player(engine);
}

fn advance_upkeep_to_main1(engine: &mut GameEngine) {
    pass_both_players(engine);
    pass_both_players(engine);
    assert_eq!(engine.state.turn_step, tricerules_core::TurnStep::Main1);
}

#[test]
fn crippling_chill_skips_exactly_the_next_controller_untap() {
    let decks = Some(vec![
        deck_with("island", &["crippling_chill"]),
        deck_with("forest", &["grizzly_bears"]),
    ]);
    let mut engine = GameEngine::new(9301, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut engine);

    let bear = relocate_to_battlefield(&mut engine, 1, "grizzly_bears", false);
    let hand_before = engine.state.players[0].hand.len();
    cast_crippling_chill(&mut engine, 0, bear);

    assert!(engine.state.objects[&bear].tapped, "the target was tapped");
    assert_eq!(
        engine.state.players[0].hand.len(),
        hand_before,
        "Crippling Chill draws after leaving its caster's hand"
    );
    assert_eq!(
        zone_view_rules_annotation_labels(&mut engine, 1, bear),
        vec!["Doesn't untap during its controller's next untap step"]
    );

    end_active_turn(&mut engine, 0);
    assert!(
        engine.state.objects[&bear].tapped,
        "the bear skipped its controller's next untap"
    );
    assert!(zone_view_rules_annotation_labels(&mut engine, 1, bear).is_empty());

    advance_upkeep_to_main1(&mut engine);
    end_active_turn(&mut engine, 1);
    advance_upkeep_to_main1(&mut engine);
    end_active_turn(&mut engine, 0);
    assert!(
        !engine.state.objects[&bear].tapped,
        "the restriction expired after one applicable untap step"
    );
}

#[test]
fn repeated_restrictions_on_an_already_tapped_target_expire_together() {
    let decks = Some(vec![
        deck_with("island", &["crippling_chill", "crippling_chill"]),
        deck_with("forest", &["grizzly_bears"]),
    ]);
    let mut engine = GameEngine::new(9302, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut engine);
    let bear = relocate_to_battlefield(&mut engine, 1, "grizzly_bears", true);

    cast_crippling_chill(&mut engine, 0, bear);
    cast_crippling_chill(&mut engine, 0, bear);
    assert_eq!(
        engine.state.skip_next_untap.len(),
        1,
        "identical next-step restrictions coalesce"
    );

    end_active_turn(&mut engine, 0);
    assert!(engine.state.objects[&bear].tapped);
    assert!(
        engine.state.skip_next_untap.is_empty(),
        "all restrictions expire at the same applicable untap step"
    );

    advance_upkeep_to_main1(&mut engine);
    end_active_turn(&mut engine, 1);
    advance_upkeep_to_main1(&mut engine);
    end_active_turn(&mut engine, 0);
    assert!(!engine.state.objects[&bear].tapped);
}

#[test]
fn restriction_follows_the_permanents_current_controller() {
    let decks = Some(vec![
        deck_with("island", &["crippling_chill"]),
        deck_with("forest", &["grizzly_bears"]),
    ]);
    let mut engine = GameEngine::new(9303, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut engine);
    let bear = relocate_to_battlefield(&mut engine, 1, "grizzly_bears", false);
    cast_crippling_chill(&mut engine, 0, bear);

    engine.state.players[1]
        .battlefield
        .retain(|&object_id| object_id != bear);
    engine.state.players[0].battlefield.push(bear);
    let object = engine.state.objects.get_mut(&bear).expect("bear");
    object.base_controller = 0;
    object.controller = 0;

    end_active_turn(&mut engine, 0);
    assert!(engine.state.objects[&bear].tapped);
    assert_eq!(
        engine.state.skip_next_untap.len(),
        1,
        "P1's untap does not consume a restriction on a P0-controlled permanent"
    );

    advance_upkeep_to_main1(&mut engine);
    end_active_turn(&mut engine, 1);
    assert!(engine.state.objects[&bear].tapped);
    assert!(
        engine.state.skip_next_untap.is_empty(),
        "the restriction is consumed during the new controller's untap"
    );
}

#[test]
fn leaving_and_returning_clears_the_old_objects_restriction() {
    let decks = Some(vec![
        deck_with("island", &["crippling_chill", "unsummon"]),
        deck_with("forest", &["grizzly_bears"]),
    ]);
    let mut engine = GameEngine::new(9304, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut engine);
    let bear = relocate_to_battlefield(&mut engine, 1, "grizzly_bears", false);
    cast_crippling_chill(&mut engine, 0, bear);

    ensure_in_hand(&mut engine, 0, "unsummon");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    let unsummon = hand_index_for_card(&engine, 0, "unsummon");
    engine
        .apply_command(0, &cast_spell(unsummon, target(bear)))
        .expect("cast Unsummon");
    resolve_entire_stack_two_player(&mut engine);
    assert!(engine.state.skip_next_untap.is_empty());

    let returned = relocate_to_battlefield(&mut engine, 1, "grizzly_bears", true);
    assert_eq!(returned, bear, "the relay-compatible ObjectId is reused");
    assert!(zone_view_rules_annotation_labels(&mut engine, 1, bear).is_empty());
    end_active_turn(&mut engine, 0);
    assert!(
        !engine.state.objects[&bear].tapped,
        "the new object is not restricted by its previous existence"
    );
}

#[test]
fn crippling_chill_fizzles_without_drawing_when_its_target_leaves() {
    let decks = Some(vec![
        deck_with("island", &["crippling_chill"]),
        deck_with("island", &["grizzly_bears", "unsummon"]),
    ]);
    let mut engine = GameEngine::new(9305, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut engine);
    let bear = relocate_to_battlefield(&mut engine, 1, "grizzly_bears", false);
    ensure_in_hand(&mut engine, 0, "crippling_chill");
    ensure_in_hand(&mut engine, 1, "unsummon");
    let library_before = engine.state.players[0].library.len();

    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 3,
            ..Default::default()
        },
    );
    let chill = hand_index_for_card(&engine, 0, "crippling_chill");
    engine
        .apply_command(0, &cast_spell(chill, target(bear)))
        .expect("cast Crippling Chill");
    engine.apply_command(0, &pass()).expect("pass priority");

    give_mana(
        &mut engine,
        1,
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    let unsummon = hand_index_for_card(&engine, 1, "unsummon");
    engine
        .apply_command(1, &cast_spell(unsummon, target(bear)))
        .expect("cast Unsummon in response");
    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(
        engine.state.objects[&bear].zone,
        tricerules_core::Zone::Hand
    );
    assert_eq!(engine.state.players[0].library.len(), library_before);
    assert!(engine.state.skip_next_untap.is_empty());
}
