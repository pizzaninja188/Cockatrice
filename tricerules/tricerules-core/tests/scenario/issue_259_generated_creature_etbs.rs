//! Issue #259 — generated creature ETBs reuse exact physical exile permissions, normal
//! creature targeting, the canonical Ally token, and the private draw-discard continuation.
//!
//! Oracle and rulings checked 2026-09-11. CR 111 governs token creation, CR 115.1d and
//! 608.2b govern triggered-ability targets and revalidation, CR 121 and 701.9 govern the
//! ordered draw and discard, CR 400.7 and 406 govern exile identity and visibility, CR 603
//! governs trigger staging/APNAP order, and CR 701.18 governs playing cards.

use super::helpers::*;
use tricerules_cards::CounterKind;
use tricerules_core::state::ExilePlayPermissionScope;
use tricerules_core::{TurnStep, Zone};
use tricerules_proto::ruled::v1::{ChoiceKind, ChooseTriggerTarget, TargetRef, TargetRefKind};

fn resolve_top_stack(engine: &mut GameEngine) -> RuledEventBatch {
    let first = engine.state.priority_player_id();
    let second = if first == engine.state.players[0].id {
        engine.state.players[1].id
    } else {
        engine.state.players[0].id
    };
    engine.apply_command(first, &pass()).expect("first pass");
    engine
        .apply_command(second, &pass())
        .expect("second pass resolves stack item")
}

fn seat_on_top(engine: &mut GameEngine, player: usize, card_id: &str) -> u32 {
    let object_id = inject_library_card(engine, player, card_id);
    engine.state.players[player]
        .library
        .retain(|candidate| *candidate != object_id);
    engine.state.players[player].library.push_front(object_id);
    object_id
}

fn choose_trigger_target(target_object_id: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
            decline: false,
            selected_modes: Vec::new(),
            targets: vec![TargetRef {
                object_id: target_object_id,
                group_index: 0,
                kind: TargetRefKind::Permanent as i32,
                ..Default::default()
            }],
        })),
    }
}

fn advance_to_turn_instance(engine: &mut GameEngine, target: u64) {
    let mut commands = 0;
    while engine.state.turn_instance < target {
        resolve_cleanup_discards_if_any(engine);
        let priority = engine.state.priority_player_id();
        engine
            .apply_command(priority, &pass())
            .expect("advance turn");
        commands += 1;
        assert!(commands < 200, "turn advancement stalled");
    }
}

#[test]
fn generated_gundabad_opportunist_grants_exact_public_land_permission_until_next_turn() {
    let decks = Some(vec![
        deck_with("mountain", &["gundabad_opportunist"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(259_001, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    let top = seat_on_top(&mut engine, 0, "forest");

    let opportunist = move_ready_to_battlefield(&mut engine, 0, "gundabad_opportunist");
    assert_eq!(engine.state.stack.len(), 1, "one entry stages one trigger");
    assert_eq!(engine.state.stack[0].source_permanent_id, Some(opportunist));
    resolve_top_stack(&mut engine);

    assert_eq!(engine.state.objects[&top].zone, Zone::Exile);
    let permission = engine
        .state
        .active_exile_play_permissions
        .iter()
        .find(|permission| permission.object_id == top)
        .expect("generated ETB play permission");
    assert_eq!(permission.player_id, 0);
    assert_eq!(permission.source_label, "Gundabad Opportunist");
    assert_eq!(permission.scope, ExilePlayPermissionScope::PlayCard);
    assert_eq!(
        permission.zone_change_generation,
        engine.state.zone_change_generation[&top]
    );
    let expiry = permission
        .expires_at_cleanup_turn_instance
        .expect("end-of-next-turn expiry");
    assert_eq!(expiry, engine.state.turn_instance + 2);

    let legal = &engine.initial_response_batch().legal_by_player[&0];
    assert_eq!(legal.exile_play_permission_groups[0].object_ids, [top]);
    assert_eq!(legal.zone_land_actions.len(), 1);
    assert!(engine.initial_response_batch().legal_by_player[&1]
        .exile_play_permission_groups
        .is_empty());
    engine.state.turn_step = TurnStep::Upkeep;
    assert!(engine.initial_response_batch().legal_by_player[&0]
        .zone_land_actions
        .is_empty());
    engine.state.turn_step = TurnStep::Main1;

    advance_to_turn_instance(&mut engine, expiry + 1);
    assert!(engine.state.active_exile_play_permissions.is_empty());
    assert_eq!(engine.state.objects[&top].zone, Zone::Exile);
}

#[test]
fn generated_counter_etb_publishes_legal_creatures_and_revalidates_generation() {
    let decks = Some(vec![
        deck_with("plains", &["ironpaw_aspirant", "jeong_jeongs_deserters"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(259_002, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    let creature = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let artifact = inject_permanent_on_battlefield(&mut engine, 1, "icy_manipulator");

    move_ready_to_battlefield(&mut engine, 0, "ironpaw_aspirant");
    assert_eq!(engine.state.pending_triggers.len(), 1);
    assert!(engine
        .apply_command(0, &choose_trigger_target(artifact))
        .is_err());
    engine
        .apply_command(0, &choose_trigger_target(creature))
        .expect("choose published creature target");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        engine.state.objects[&creature].counter_count(CounterKind::PlusOnePlusOne),
        1
    );

    let stale = inject_creature_on_battlefield(&mut engine, 1, "storm_crow");
    move_ready_to_battlefield(&mut engine, 0, "jeong_jeongs_deserters");
    engine
        .apply_command(0, &choose_trigger_target(stale))
        .expect("choose initially legal target");
    engine.state.players[1]
        .battlefield
        .retain(|object_id| *object_id != stale);
    engine.state.players[1].hand.push(stale);
    engine.state.objects.get_mut(&stale).expect("target").zone = Zone::Hand;
    *engine
        .state
        .zone_change_generation
        .entry(stale)
        .or_insert(0) += 1;
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        engine.state.objects[&stale].counter_count(CounterKind::PlusOnePlusOne),
        0
    );
}

#[test]
fn generated_invasion_reinforcements_creates_the_canonical_ally_for_its_controller() {
    let decks = Some(vec![
        deck_with("plains", &["invasion_reinforcements"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(259_003, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);

    move_ready_to_battlefield(&mut engine, 0, "invasion_reinforcements");
    resolve_entire_stack_two_player(&mut engine);
    let allies = battlefield_token_oids(&engine, 0, "ally_w_1_1");
    let [ally] = allies.as_slice() else {
        panic!("trigger controller must receive one canonical Ally")
    };
    let ally = &engine.state.objects[ally];
    assert_eq!(ally.owner, 0);
    assert_eq!(ally.controller, 0);
    assert_eq!(ally.card_id, "ally_w_1_1");
    assert!(battlefield_token_oids(&engine, 1, "ally_w_1_1").is_empty());
}

#[test]
fn generated_bellowing_crier_draws_before_its_mandatory_private_discard() {
    let decks = Some(vec![
        deck_with("island", &["bellowing_crier"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(259_004, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    let drawn = seat_on_top(&mut engine, 0, "storm_crow");

    move_ready_to_battlefield(&mut engine, 0, "bellowing_crier");
    let hand_before = engine.state.players[0].hand.len();
    let library_before = engine.state.players[0].library.len();
    let batch = resolve_top_stack(&mut engine);
    let choice = find_resolution_choice(&batch).expect("mandatory discard choice");
    assert_eq!(choice.deciding_player_id, 0);
    assert_eq!(choice.choice_kind(), ChoiceKind::HandCards);
    assert_eq!((choice.min, choice.max), (1, 1));
    assert!(choice.candidate_object_ids.contains(&drawn));
    assert_eq!(engine.state.players[0].library.len(), library_before - 1);
    assert_eq!(engine.state.players[0].hand.len(), hand_before + 1);
    assert_eq!(engine.state.objects[&drawn].zone, Zone::Hand);

    assert!(engine
        .apply_command(1, &submit_resolution_choice(vec![drawn]))
        .is_err());
    engine
        .apply_command(0, &submit_resolution_choice(vec![drawn]))
        .expect("controller makes private discard choice");
    assert_eq!(engine.state.objects[&drawn].zone, Zone::Graveyard);
    assert_eq!(engine.state.players[0].hand.len(), hand_before);
}
