use crate::helpers::*;
use tricerules_cards::primitives::{ContinuousEffectKind, CounterKind, EffectDuration};
use tricerules_core::{AffectedScope, ContinuousEffect};

fn advance_to_end_step(engine: &mut GameEngine, player: i32) {
    for _ in 0..8 {
        if engine.state.turn_step == tricerules_core::TurnStep::EndStep {
            return;
        }
        engine
            .apply_command(player, &primitive_yield())
            .expect("advance toward end step");
    }
    panic!("did not reach end step");
}

#[test]
fn kav_lander_waits_for_its_controllers_next_turn_end_step() {
    let decks = Some(vec![
        deck_with("mountain", &["kav_landseeker"]),
        deck_with("island", &[]),
    ]);
    let mut engine = GameEngine::new(161_001, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut engine);
    ensure_in_hand(&mut engine, 0, "kav_landseeker");
    grant_pool(&mut engine, 0);
    let kav = hand_index_for_card(&engine, 0, "kav_landseeker");
    engine
        .apply_command(0, &cast_spell(kav, vec![]))
        .expect("cast Kav Landseeker");
    resolve_entire_stack_two_player(&mut engine);

    let landers = battlefield_token_oids(&engine, 0, "lander");
    assert_eq!(landers.len(), 1);
    let lander = landers[0];

    advance_to_end_step(&mut engine, 0);
    assert!(
        engine.state.objects.contains_key(&lander),
        "the creation turn's end step is too early"
    );
    assert!(engine.state.stack.is_empty(), "no early delayed trigger");

    engine
        .apply_command(0, &primitive_yield())
        .expect("finish the creation turn");
    advance_to_main1_from_game_start(&mut engine);
    end_active_turn(&mut engine, 1);
    advance_to_main1_from_game_start(&mut engine);
    advance_to_end_step(&mut engine, 0);
    assert_eq!(engine.state.stack.len(), 1, "one delayed cohort trigger");
    resolve_entire_stack_two_player(&mut engine);
    assert!(
        !engine.state.objects.contains_key(&lander),
        "the exact controlled Lander is sacrificed"
    );
}

#[test]
fn waterskin_untaps_at_another_players_untap_boundary_unless_it_lost_the_ability() {
    let decks = Some(vec![
        deck_with(
            "mountain",
            &[
                "benders_waterskin",
                "benders_waterskin",
                "benders_waterskin",
            ],
        ),
        deck_with("island", &[]),
    ]);
    let mut engine = GameEngine::new(161_002, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut engine);
    let waterskin = relocate_to_battlefield(&mut engine, 0, "benders_waterskin", true);
    let silenced = relocate_to_battlefield(&mut engine, 0, "benders_waterskin", true);
    let stunned = relocate_to_battlefield(&mut engine, 0, "benders_waterskin", true);
    engine
        .state
        .objects
        .get_mut(&stunned)
        .expect("stunned Waterskin")
        .set_counter(CounterKind::Stun, 1);
    engine.state.continuous_effects.push(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: None,
        affected: AffectedScope::Single(silenced),
        kind: ContinuousEffectKind::Layer6RemoveAllAbilities,
        condition: None,
        duration: EffectDuration::WhileSourceOnBattlefield,
        timestamp: engine.state.command_index,
    });
    engine.state.turn_step = tricerules_core::TurnStep::EndStep;
    engine.state.active_player_idx = 0;
    engine.state.priority_idx = 0;
    engine.state.passes_since_stack_change = 0;
    let _ = engine.initial_response_batch();

    let batch = engine
        .apply_command(0, &primitive_yield())
        .expect("roll into the opponent's untap step");

    assert!(!engine.state.objects[&waterskin].tapped);
    assert!(engine.state.objects[&silenced].tapped);
    assert!(engine.state.objects[&stunned].tapped);
    assert_eq!(
        engine.state.objects[&stunned].counter_count(CounterKind::Stun),
        0
    );
    assert!(batch.events.iter().any(|event| matches!(
        event.ev.as_ref(),
        Some(Ev::PermanentsUntapped(untapped))
            if untapped.object_ids.contains(&waterskin)
                && !untapped.object_ids.contains(&silenced)
                && !untapped.object_ids.contains(&stunned)
    )));
}

#[test]
fn kav_delayed_sacrifice_leaves_a_lander_no_longer_controlled_by_its_creator() {
    let decks = Some(vec![
        deck_with("mountain", &["kav_landseeker"]),
        deck_with("island", &[]),
    ]);
    let mut engine = GameEngine::new(161_003, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut engine);
    ensure_in_hand(&mut engine, 0, "kav_landseeker");
    grant_pool(&mut engine, 0);
    let kav = hand_index_for_card(&engine, 0, "kav_landseeker");
    engine
        .apply_command(0, &cast_spell(kav, vec![]))
        .expect("cast Kav Landseeker");
    resolve_entire_stack_two_player(&mut engine);
    let lander = battlefield_token_oids(&engine, 0, "lander")[0];

    engine.state.players[0]
        .battlefield
        .retain(|object_id| *object_id != lander);
    engine.state.players[1].battlefield.push(lander);
    let lander_object = engine.state.objects.get_mut(&lander).expect("Lander");
    lander_object.base_controller = 1;
    lander_object.controller = 1;

    advance_to_end_step(&mut engine, 0);
    engine
        .apply_command(0, &primitive_yield())
        .expect("finish creation turn");
    advance_to_main1_from_game_start(&mut engine);
    end_active_turn(&mut engine, 1);
    advance_to_main1_from_game_start(&mut engine);
    advance_to_end_step(&mut engine, 0);
    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(engine.state.objects[&lander].controller, 1);
    assert_eq!(
        engine.state.objects[&lander].zone,
        tricerules_core::Zone::Battlefield
    );
}
