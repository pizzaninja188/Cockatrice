//! Generated Unscrupulous Agent / Skullcap Snail ETB exile scenarios.
//!
//! Oracle and rulings checked 2026-09-15. CR 115.1d and 608.2b govern target selection and
//! resolution revalidation; CR 402.3 and 406.1 govern private hand information and exile; CR
//! 603.2 governs the ETB trigger; CR 400.7 governs zone-change identity; and CR 102.1-102.3 with
//! CR 800.1 define opponents and multiplayer context.

use super::helpers::*;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::{ChooseTriggerTarget, TargetRef, TargetRefKind};

fn choose_target_player(player_id: i32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
            decline: false,
            selected_modes: Vec::new(),
            targets: vec![TargetRef {
                object_id: player_id as u32,
                group_index: 0,
                kind: TargetRefKind::Player as i32,
                ..Default::default()
            }],
        })),
    }
}

fn resolve_top_stack_two_player(engine: &mut GameEngine) -> RuledEventBatch {
    let first = engine.state.priority_player_id();
    let second = engine
        .state
        .players
        .iter()
        .map(|player| player.id)
        .find(|player_id| *player_id != first)
        .expect("second player");
    engine.apply_command(first, &pass()).expect("first pass");
    engine
        .apply_command(second, &pass())
        .expect("second pass resolves stack item")
}

fn resolve_top_stack_three_player(engine: &mut GameEngine) -> RuledEventBatch {
    let mut resolved = None;
    for _ in 0..engine.state.players.len() {
        let player = engine.state.priority_player_id();
        resolved = Some(
            engine
                .apply_command(player, &pass())
                .expect("player passes priority"),
        );
    }
    resolved.expect("three-player resolution batch")
}

fn stage_agent(seed: u64) -> (GameEngine, u32, u32) {
    let mut engine = GameEngine::new(seed, &[0, 1], 20, None, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    inject_card_into_hand(&mut engine, 0, "unscrupulous_agent");
    engine.state.players[1].hand.clear();
    let candidate = inject_card_into_hand(&mut engine, 1, "storm_crow");
    let agent = move_ready_to_battlefield(&mut engine, 0, "unscrupulous_agent");
    assert_eq!(engine.state.pending_triggers.len(), 1);
    (engine, agent, candidate)
}

fn append_third_player(engine: &mut GameEngine) {
    // GameEngine::new intentionally remains a two-seat constructor. Extending the state here
    // verifies that the target filter is player-set-generic at resolution time.
    let mut third = engine.state.players[1].clone();
    third.id = 2;
    third.battlefield.clear();
    third.hand.clear();
    third.library.clear();
    third.graveyard.clear();
    third.exile.clear();
    engine.state.players.push(third);
}

fn move_hand_card_through_library(engine: &mut GameEngine, player: usize, object_id: u32) {
    engine.state.players[player]
        .hand
        .retain(|candidate| *candidate != object_id);
    engine.state.players[player].library.push_back(object_id);
    engine
        .state
        .objects
        .get_mut(&object_id)
        .expect("hand card")
        .zone = Zone::Library;
    *engine
        .state
        .zone_change_generation
        .entry(object_id)
        .or_default() += 1;

    engine.state.players[player].hand.push(object_id);
    engine
        .state
        .objects
        .get_mut(&object_id)
        .expect("library card")
        .zone = Zone::Hand;
    *engine
        .state
        .zone_change_generation
        .entry(object_id)
        .or_default() += 1;
}

#[test]
fn generated_opponent_hand_exile_succeeds_with_private_affected_player_choice() {
    let (mut engine, _agent, candidate) = stage_agent(288_001);
    assert!(
        engine.apply_command(0, &choose_target_player(0)).is_err(),
        "the source controller cannot be the opponent target"
    );
    engine
        .apply_command(0, &choose_target_player(1))
        .expect("choose target opponent");

    let parked = resolve_top_stack_two_player(&mut engine);
    let choice = find_resolution_choice(&parked).expect("private hand choice");
    assert_eq!(choice.deciding_player_id, 1);
    assert_eq!(choice.choice_kind(), ChoiceKind::HandCards);
    assert_eq!((choice.min, choice.max), (1, 1));
    assert_eq!(choice.candidate_object_ids, [candidate]);
    assert!(matches!(
        &engine
            .state
            .pending_resolution
            .as_ref()
            .expect("pending hand choice")
            .continuation,
        ResolutionContinuation::HandChoice { hand_choice, .. }
            if hand_choice.action == HandCardAction::Exile
    ));

    let generation_before = engine
        .state
        .zone_change_generation
        .get(&candidate)
        .copied()
        .unwrap_or(0);
    assert!(
        engine
            .apply_command(0, &submit_resolution_choice(vec![candidate]))
            .is_err(),
        "only the affected opponent may submit the private choice"
    );
    engine
        .apply_command(1, &submit_resolution_choice(vec![candidate]))
        .expect("affected opponent chooses the card");
    assert_eq!(engine.state.objects[&candidate].zone, Zone::Exile);
    assert_eq!(
        engine.state.zone_change_generation[&candidate],
        generation_before + 1
    );
    assert!(engine.state.pending_resolution.is_none());
}

#[test]
fn generated_opponent_hand_exile_is_empty_hand_noop() {
    let (mut engine, _agent, candidate) = stage_agent(288_002);
    engine.state.players[1].hand.clear();
    engine
        .apply_command(0, &choose_target_player(1))
        .expect("choose target opponent");
    let resolved = resolve_top_stack_two_player(&mut engine);

    assert!(find_resolution_choice(&resolved).is_none());
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(engine.state.objects[&candidate].zone, Zone::Hand);
}

#[test]
fn generated_opponent_hand_exile_rejects_unauthorized_and_noncandidate_objects() {
    let (mut engine, agent, candidate) = stage_agent(288_003);
    let wrong_owner = inject_card_into_hand(&mut engine, 0, "forest");
    let wrong_zone = inject_library_card(&mut engine, 1, "forest");
    engine
        .apply_command(0, &choose_target_player(1))
        .expect("choose target opponent");
    let parked = resolve_top_stack_two_player(&mut engine);
    let choice = find_resolution_choice(&parked).expect("private hand choice");
    assert_eq!(choice.candidate_object_ids, [candidate]);

    for (player, object_id, description) in [
        (0, candidate, "unauthorized chooser"),
        (1, wrong_owner, "wrong-owner object"),
        (1, wrong_zone, "noncandidate library object"),
        (1, agent, "noncandidate battlefield object"),
    ] {
        assert!(
            engine
                .apply_command(player, &submit_resolution_choice(vec![object_id]))
                .is_err(),
            "{description} must be rejected"
        );
        assert!(engine.state.pending_resolution.is_some());
    }
    engine
        .apply_command(1, &submit_resolution_choice(vec![candidate]))
        .expect("valid affected-player choice");
    assert_eq!(engine.state.objects[&candidate].zone, Zone::Exile);
}

#[test]
fn generated_opponent_hand_exile_rejects_stale_zone_change_generation() {
    let (mut engine, _agent, candidate) = stage_agent(288_004);
    engine
        .apply_command(0, &choose_target_player(1))
        .expect("choose target opponent");
    let parked = resolve_top_stack_two_player(&mut engine);
    assert!(find_resolution_choice(&parked).is_some());

    let generation_before = engine
        .state
        .zone_change_generation
        .get(&candidate)
        .copied()
        .unwrap_or(0);
    move_hand_card_through_library(&mut engine, 1, candidate);
    assert_eq!(
        engine.state.zone_change_generation[&candidate],
        generation_before + 2
    );
    assert!(
        engine
            .apply_command(1, &submit_resolution_choice(vec![candidate]))
            .is_err(),
        "the choice captured the previous physical card generation"
    );
    assert!(engine.state.pending_resolution.is_some());
    assert_eq!(engine.state.objects[&candidate].zone, Zone::Hand);
}

#[test]
fn generated_opponent_hand_exile_targets_a_nondefault_multiplayer_opponent() {
    let (mut engine, _agent, player_one_card) = stage_agent(288_005);
    append_third_player(&mut engine);
    let player_two_card = inject_card_into_hand(&mut engine, 2, "storm_crow");
    engine
        .apply_command(0, &choose_target_player(2))
        .expect("choose the non-default opponent target");

    let parked = resolve_top_stack_three_player(&mut engine);
    let choice = find_resolution_choice(&parked).expect("private hand choice for P2");
    assert_eq!(choice.deciding_player_id, 2);
    assert_eq!(choice.choice_kind(), ChoiceKind::HandCards);
    assert_eq!(choice.candidate_object_ids, [player_two_card]);
    assert!(
        engine
            .apply_command(1, &submit_resolution_choice(vec![player_two_card]))
            .is_err(),
        "the non-target opponent cannot submit P2's private choice"
    );
    engine
        .apply_command(2, &submit_resolution_choice(vec![player_two_card]))
        .expect("targeted opponent chooses the card");
    assert_eq!(engine.state.objects[&player_two_card].zone, Zone::Exile);
    assert_eq!(engine.state.objects[&player_one_card].zone, Zone::Hand);
}
