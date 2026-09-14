use super::helpers::*;
use tricerules_core::state::{PendingLibraryPartitionKind, ResolutionContinuation};
use tricerules_core::GameEngine;

#[test]
fn issue_277_affinity_for_artifacts_counts_only_controlled_artifacts_and_preserves_etb() {
    let decks = Some(vec![deck_with("island", &[]), deck_with("forest", &[])]);
    let mut engine = GameEngine::new(277_001, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);

    inject_card_into_hand(&mut engine, 0, "memory_guardian");
    assert_eq!(hand_action_reduction(&mut engine, 0, "memory_guardian"), 0);

    inject_permanent_on_battlefield(&mut engine, 0, "short_sword");
    assert_eq!(
        hand_action_reduction(&mut engine, 0, "memory_guardian"),
        1,
        "one controller-owned artifact reduces generic mana by one"
    );

    inject_permanent_on_battlefield(&mut engine, 1, "short_sword");
    assert_eq!(
        hand_action_reduction(&mut engine, 0, "memory_guardian"),
        1,
        "an opponent's artifact does not count"
    );

    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 3,
            ..Default::default()
        },
    );
    let memory = hand_index_for_card(&engine, 0, "memory_guardian");
    let hand_before = engine.state.players[0].hand.clone();
    let command_index = engine.state.command_index;
    assert!(
        engine
            .apply_command(0, &cast_spell(memory, vec![]))
            .is_err(),
        "three generic mana cannot pay Memory Guardian's required blue mana"
    );
    assert_eq!(engine.state.players[0].hand, hand_before);
    assert_eq!(engine.state.command_index, command_index);

    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            c: 3,
            ..Default::default()
        },
    );
    engine
        .apply_command(0, &cast_spell(memory, vec![]))
        .expect("affinity should leave Memory Guardian's {{3}}{{U}} payable");
    engine.apply_command(0, &pass()).expect("caster pass");
    engine.apply_command(1, &pass()).expect("opponent pass");

    for _ in 0..5 {
        inject_permanent_on_battlefield(&mut engine, 0, "short_sword");
    }

    inject_card_into_hand(&mut engine, 0, "valkyrie_aerial_unit");
    inject_library_card(&mut engine, 0, "island");
    inject_library_card(&mut engine, 0, "island");
    assert_eq!(
        hand_action_reduction(&mut engine, 0, "valkyrie_aerial_unit"),
        7,
        "the published reduction counts all controller artifacts, including Memory Guardian"
    );
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 2,
            ..Default::default()
        },
    );
    let valkyrie = hand_index_for_card(&engine, 0, "valkyrie_aerial_unit");
    engine
        .apply_command(0, &cast_spell(valkyrie, vec![]))
        .expect("affinity should leave Valkyrie's {U}{U} payable");
    engine.apply_command(0, &pass()).expect("caster pass");
    engine.apply_command(1, &pass()).expect("opponent pass");
    resolve_entire_stack_two_player(&mut engine);
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("Valkyrie ETB surveil choice");
    let ResolutionContinuation::LibraryPartition {
        looked_at, kind, ..
    } = &pending.continuation
    else {
        panic!("Valkyrie ETB should park a library partition");
    };
    assert_eq!(pending.deciding_player, 0);
    assert_eq!(*kind, PendingLibraryPartitionKind::Surveil);
    assert_eq!(looked_at.len(), 2);
    engine
        .apply_command(0, &submit_resolution_choice(Vec::new()))
        .expect("decline to surveil either card");
    let top_order = match &engine
        .state
        .pending_resolution
        .as_ref()
        .expect("surveil ordering choice")
        .continuation
    {
        ResolutionContinuation::LibraryPartition { looked_at, .. } => looked_at.clone(),
        _ => panic!("surveil should retain a library-partition continuation"),
    };
    engine
        .apply_command(0, &submit_resolution_choice(top_order))
        .expect("keep both surveilled cards on top in order");
    assert!(engine.state.pending_resolution.is_none());
}

#[track_caller]
fn hand_action_reduction(engine: &mut GameEngine, player: usize, card_id: &str) -> u32 {
    let hand_index = hand_index_for_card(engine, player, card_id) as u32;
    engine.initial_response_batch().legal_by_player[&(player as i32)]
        .hand_actions
        .iter()
        .find(|action| action.hand_index == hand_index)
        .unwrap_or_else(|| panic!("missing cast action for {card_id}"))
        .generic_cost_reduction
}
