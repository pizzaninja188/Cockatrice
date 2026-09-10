use super::helpers::*;
use tricerules_cards::{ContinuousEffectKind, EffectDuration, Keyword};
use tricerules_core::{GameEngine, Zone};

fn setup(seed: u64) -> GameEngine {
    let mut engine = GameEngine::new(
        seed,
        &[0, 1],
        20,
        Some(vec![
            deck_with("mountain", &["evils_thrall"]),
            forest_only_deck(),
        ]),
        true,
    )
    .expect("Evil's Thrall is registered");
    advance_to_main1_from_game_start(&mut engine);
    relocate_to_hand(&mut engine, 0, "evils_thrall");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            r: 1,
            c: 2,
            ..Default::default()
        },
    );
    engine
}

fn cast_thrall(engine: &mut GameEngine, target: u32) {
    let slot = hand_index_for_card(engine, 0, "evils_thrall");
    engine
        .apply_command(0, &cast_spell(slot, target_object(target)))
        .expect("cast Evil's Thrall");
    resolve_entire_stack_two_player(engine);
}

fn copy_characteristics(engine: &mut GameEngine, object_id: u32, card_id: &str) {
    let definition = tricerules_cards::CardRegistry::global()
        .get(card_id)
        .expect("copy source is registered");
    engine
        .state
        .objects
        .get_mut(&object_id)
        .unwrap()
        .copiable_values = Some(tricerules_core::state::CopiableValues {
        source_card_id: card_id.into(),
        source_face_index: 0,
        face: definition.primary_face().clone(),
        room_faces: None,
        display_name: definition.name.clone(),
    });
}

#[test]
fn issue_243_evils_thrall_is_registered() {
    GameEngine::new(
        243_001,
        &[0, 1],
        20,
        Some(vec![
            deck_with("mountain", &["evils_thrall"]),
            forest_only_deck(),
        ]),
        true,
    )
    .expect("Evil's Thrall is registered");
}

#[test]
fn issue_243_no_villain_and_equal_derived_mana_value_end_this_turn() {
    for (seed, add_equal_villain) in [(243_002, false), (243_003, true)] {
        let mut engine = setup(seed);
        let target = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
        if add_equal_villain {
            let derived_villain = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
            copy_characteristics(&mut engine, derived_villain, "glamorous_grapplers");
            copy_characteristics(&mut engine, target, "glamorous_grapplers");
        }
        engine.state.objects.get_mut(&target).unwrap().tapped = true;

        cast_thrall(&mut engine, target);
        assert_eq!(engine.state.objects[&target].controller, 0);
        assert!(!engine.state.objects[&target].tapped);
        assert!(engine.effective_has_keyword(target, Keyword::Haste));
        assert!(engine.state.continuous_effects.iter().any(|effect| {
            matches!(effect.kind, ContinuousEffectKind::Layer2Control { .. })
                && effect.duration == EffectDuration::UntilEndOfTurn
        }));

        end_active_turn(&mut engine, 0);
        assert_eq!(engine.state.objects[&target].controller, 1);
        assert!(!engine.effective_has_keyword(target, Keyword::Haste));
    }
}

#[test]
fn issue_243_greater_villain_lasts_through_other_turn_to_controller_next_cleanup() {
    let mut engine = setup(243_004);
    let villain = inject_creature_on_battlefield(&mut engine, 0, "grendel,_spawn_of_knull");
    let target = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    cast_thrall(&mut engine, target);

    assert!(engine
        .state
        .continuous_effects
        .iter()
        .any(|effect| matches!(
            effect.duration,
            EffectDuration::UntilEndOfNextTurn {
                player: 0,
                created_turn_instance: _,
            }
        )));
    assert_eq!(
        engine
            .state
            .continuous_effects
            .iter()
            .filter(|effect| matches!(effect.kind, ContinuousEffectKind::Layer2Control { .. }))
            .count(),
        1
    );
    engine.state.players[0]
        .battlefield
        .retain(|id| *id != villain);
    engine.state.objects.get_mut(&villain).unwrap().zone = Zone::Graveyard;
    end_active_turn(&mut engine, 0);
    assert_eq!(engine.state.objects[&target].controller, 0);
    assert!(!engine.effective_has_keyword(target, Keyword::Haste));

    pass_both_players(&mut engine);
    pass_both_players(&mut engine);
    end_active_turn(&mut engine, 1);
    assert_eq!(engine.state.objects[&target].controller, 0);

    pass_both_players(&mut engine);
    pass_both_players(&mut engine);
    end_active_turn(&mut engine, 0);
    assert_eq!(engine.state.objects[&target].controller, 1);
}

#[test]
fn issue_243_an_inserted_controller_turn_is_the_next_actual_turn() {
    let mut engine = setup(243_008);
    inject_creature_on_battlefield(&mut engine, 0, "grendel,_spawn_of_knull");
    let target = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    cast_thrall(&mut engine, target);

    // Model the turn scheduler inserting P0's extra turn: the active player is unchanged, while
    // the monotonic instance advances. The duration must expire at this actual turn's cleanup.
    engine.state.turn_instance += 1;
    end_active_turn(&mut engine, 0);
    assert_eq!(engine.state.objects[&target].controller, 1);
}

#[test]
fn issue_243_later_control_effect_wins_then_expiring_it_reveals_the_extended_effect() {
    let mut engine = GameEngine::new(
        243_007,
        &[0, 1],
        20,
        Some(vec![
            deck_with("mountain", &["evils_thrall"]),
            deck_with("mountain", &["act_of_treason"]),
        ]),
        true,
    )
    .unwrap();
    advance_to_main1_from_game_start(&mut engine);
    relocate_to_hand(&mut engine, 0, "evils_thrall");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            r: 1,
            c: 2,
            ..Default::default()
        },
    );
    inject_creature_on_battlefield(&mut engine, 0, "grendel,_spawn_of_knull");
    let target = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    engine.state.objects.get_mut(&target).unwrap().tapped = true;
    cast_thrall(&mut engine, target);
    end_active_turn(&mut engine, 0);

    pass_both_players(&mut engine);
    pass_both_players(&mut engine);
    relocate_to_hand(&mut engine, 1, "act_of_treason");
    give_mana(
        &mut engine,
        1,
        ManaGift {
            r: 1,
            c: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 1, "act_of_treason");
    engine
        .apply_command(1, &cast_spell(slot, target_object(target)))
        .unwrap();
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&target].controller, 1);

    end_active_turn(&mut engine, 1);
    assert_eq!(engine.state.objects[&target].controller, 0);
    pass_both_players(&mut engine);
    pass_both_players(&mut engine);
    end_active_turn(&mut engine, 0);
    assert_eq!(engine.state.objects[&target].controller, 1);
}

#[test]
fn issue_243_reentered_target_is_not_controlled() {
    let mut engine = setup(243_005);
    inject_creature_on_battlefield(&mut engine, 0, "grendel,_spawn_of_knull");
    let target = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    engine.state.objects.get_mut(&target).unwrap().tapped = true;
    let slot = hand_index_for_card(&engine, 0, "evils_thrall");
    engine
        .apply_command(0, &cast_spell(slot, target_object(target)))
        .expect("cast Evil's Thrall");

    engine.state.players[1]
        .battlefield
        .retain(|id| *id != target);
    engine.state.objects.get_mut(&target).unwrap().zone = Zone::Graveyard;
    *engine
        .state
        .zone_change_generation
        .entry(target)
        .or_default() += 1;
    engine.state.objects.get_mut(&target).unwrap().zone = Zone::Battlefield;
    engine.state.players[1].battlefield.push(target);
    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(engine.state.objects[&target].controller, 1);
    assert!(engine.state.objects[&target].tapped);
    assert!(!engine.effective_has_keyword(target, Keyword::Haste));
    assert!(!engine
        .state
        .continuous_effects
        .iter()
        .any(|effect| { matches!(effect.kind, ContinuousEffectKind::Layer2Control { .. }) }));
}

#[test]
fn issue_243_extended_control_ends_if_its_controller_loses() {
    let mut engine = setup(243_006);
    inject_creature_on_battlefield(&mut engine, 0, "grendel,_spawn_of_knull");
    let target = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    cast_thrall(&mut engine, target);
    engine.apply_command(0, &concede()).expect("concede");

    assert_eq!(engine.state.objects[&target].controller, 1);
    assert!(!engine
        .state
        .continuous_effects
        .iter()
        .any(|effect| matches!(
            effect.duration,
            EffectDuration::UntilEndOfNextTurn { player: 0, .. }
        )));
}
