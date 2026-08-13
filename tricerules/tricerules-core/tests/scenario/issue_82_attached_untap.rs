//! Issue #82: effects and continuous restrictions that refer to an attached object.

use crate::helpers::*;

fn capture_sphere_engine(seed: u64, creature_controller: usize) -> (GameEngine, u32, u32) {
    let decks = Some(vec![
        deck_with("island", &["capture_sphere", "vitalize"]),
        forest_only_deck(),
    ]);
    let mut engine = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("new engine");
    advance_to_main1_from_game_start(&mut engine);
    ensure_in_hand(&mut engine, 0, "capture_sphere");
    let creature =
        inject_creature_on_battlefield(&mut engine, creature_controller, "grizzly_bears");

    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 4,
            ..Default::default()
        },
    );
    let hand_index = hand_index_for_card(&engine, 0, "capture_sphere");
    engine
        .apply_command(0, &cast_spell(hand_index, target_object(creature)))
        .expect("cast Capture Sphere");
    resolve_entire_stack_two_player(&mut engine);

    let aura = battlefield_object_for_card(&engine, 0, "capture_sphere");
    (engine, aura, creature)
}

fn advance_to_active_player(engine: &mut GameEngine, player: i32) {
    for _ in 0..50 {
        let (actor, command) = match engine.state.cleanup_discard_player {
            Some(cleanup_player) => {
                let player_index = engine
                    .state
                    .player_idx(cleanup_player)
                    .expect("cleanup player");
                let excess = engine.state.players[player_index].hand.len() - 7;
                (
                    cleanup_player,
                    discard_cleanup_batch((0..excess as u32).collect()),
                )
            }
            None => (engine.state.priority_player_id(), pass()),
        };
        engine
            .apply_command(actor, &command)
            .expect("pass through turn");
        if engine.state.active_player_id() == player
            && engine.state.turn_step == tricerules_core::TurnStep::Upkeep
        {
            return;
        }
    }
    panic!("game did not reach player {player}'s upkeep");
}

#[test]
fn capture_sphere_taps_and_restricts_the_enchanted_creature() {
    let (mut engine, aura, creature) = capture_sphere_engine(8201, 1);

    assert_eq!(
        engine.state.objects[&aura].attached_to,
        Some(AttachmentRecipient::Object(creature))
    );
    assert!(
        engine.state.objects[&creature].tapped,
        "the targetless ETB trigger taps the enchanted creature"
    );
    assert_eq!(
        zone_view_rules_annotation_labels(&mut engine, 1, creature),
        ["Doesn't untap during its controller's untap step"],
        "the attached static restriction is engine-published"
    );
}

#[test]
fn restriction_uses_the_enchanted_creatures_current_controller() {
    let (mut engine, _aura, creature) = capture_sphere_engine(8202, 1);

    advance_to_active_player(&mut engine, 1);

    assert!(
        engine.state.objects[&creature].tapped,
        "the creature stays tapped during its own controller's untap step"
    );
}

#[test]
fn explicit_untap_effect_works_while_capture_sphere_is_attached() {
    let (mut engine, _aura, creature) = capture_sphere_engine(8203, 0);
    ensure_in_hand(&mut engine, 0, "vitalize");

    cast_instant_and_resolve(
        &mut engine,
        0,
        "vitalize",
        ManaGift {
            g: 1,
            ..Default::default()
        },
    );

    assert!(
        !engine.state.objects[&creature].tapped,
        "the restriction changes only the untap-step action"
    );
}

#[test]
fn skip_marker_is_consumed_under_static_restriction_and_detachment_restores_untap() {
    let (mut engine, aura, creature) = capture_sphere_engine(8204, 1);
    let generation = engine
        .state
        .zone_change_generation
        .get(&creature)
        .copied()
        .unwrap_or(0);
    engine.state.skip_next_untap.insert((creature, generation));

    advance_to_active_player(&mut engine, 1);
    assert!(engine.state.objects[&creature].tapped);
    assert!(
        !engine
            .state
            .skip_next_untap
            .contains(&(creature, generation)),
        "the one-shot skip is consumed even when a static effect also prevents untapping"
    );

    engine.state.objects.get_mut(&aura).unwrap().attached_to = None;
    advance_to_active_player(&mut engine, 0);
    advance_to_active_player(&mut engine, 1);
    assert!(
        !engine.state.objects[&creature].tapped,
        "the dynamic attachment scope stops applying after detachment"
    );
}
