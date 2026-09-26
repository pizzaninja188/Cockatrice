use super::helpers::*;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::{ChoiceKind, ResolutionChoiceDecision};

fn engine(seed: u64) -> GameEngine {
    let mut engine = GameEngine::new(seed, &[0, 1], 20, None, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn cast_solemn_simulacrum(engine: &mut GameEngine) -> u32 {
    let solemn = inject_card_into_hand(engine, 0, "solemn_simulacrum");
    grant_pool(engine, 0);
    let hand_index = hand_index_for_card(engine, 0, "solemn_simulacrum");
    engine
        .apply_command(0, &cast_spell(hand_index, vec![]))
        .expect("cast Solemn Simulacrum");
    pass_both_players(engine);
    assert_eq!(engine.state.objects[&solemn].zone, Zone::Battlefield);
    solemn
}

fn pass_both_and_capture(engine: &mut GameEngine) -> tricerules_proto::ruled::v1::RuledEventBatch {
    let first = engine.state.priority_player_id();
    let second = if first == engine.state.players[0].id {
        engine.state.players[1].id
    } else {
        engine.state.players[0].id
    };
    engine
        .apply_command(first, &pass())
        .expect("first player passes");
    engine
        .apply_command(second, &pass())
        .expect("second player passes")
}

fn pass_priority_to(engine: &mut GameEngine, player: i32) {
    while engine.state.priority_player_id() != player {
        let current = engine.state.priority_player_id();
        engine
            .apply_command(current, &pass())
            .expect("pass priority to the requested player");
    }
}

#[test]
fn solemn_simulacrum_searches_for_a_tapped_basic_and_its_controller_may_draw_on_death() {
    let mut engine = engine(906_201);
    let solemn = cast_solemn_simulacrum(&mut engine);
    let forest = inject_library_card(&mut engine, 0, "forest");
    let taiga = inject_library_card(&mut engine, 0, "taiga");

    let entry_batch = pass_both_and_capture(&mut engine);
    let branch = find_resolution_choice(&entry_batch).expect("optional ETB search branch");
    assert_eq!(branch.deciding_player_id, 0);
    assert_eq!(branch.choice_kind(), ChoiceKind::ResolutionBranch);
    assert_eq!((branch.min, branch.max), (0, 1));

    let search_batch = engine
        .apply_command(
            0,
            &submit_resolution_decision(ResolutionChoiceDecision::SelectBranch),
        )
        .expect("choose to search");
    let search = find_resolution_choice(&search_batch).expect("basic-land selection");
    assert_eq!(search.deciding_player_id, 0);
    assert_eq!(search.choice_kind(), ChoiceKind::LibrarySearch);
    assert_eq!((search.min, search.max), (0, 1));
    assert!(search.candidate_object_ids.contains(&forest));
    assert!(!search.candidate_object_ids.contains(&taiga));

    let searched = engine
        .apply_command(0, &submit_resolution_choice(vec![forest]))
        .expect("choose Forest");
    assert_eq!(engine.state.objects[&forest].zone, Zone::Battlefield);
    assert!(engine.state.objects[&forest].tapped);
    assert_eq!(engine.state.objects[&forest].controller, 0);
    assert!(engine.state.players[0].library.contains(&taiga));
    assert!(!engine.state.players[0].library.contains(&forest));
    assert!(searched.events.iter().any(|event| {
        matches!(
            &event.ev,
            Some(Ev::Log(log)) if log.text == "P0 shuffles their library."
        )
    }));

    let opponent_draw = inject_library_card(&mut engine, 1, "plains");
    let expected_draw = *engine.state.players[0]
        .library
        .front()
        .expect("remaining library card");
    let hand_before = engine.state.players[0].hand.len();

    pass_priority_to(&mut engine, 1);
    inject_card_into_hand(&mut engine, 1, "lightning_bolt");
    grant_pool(&mut engine, 1);
    let bolt_index = hand_index_for_card(&engine, 1, "lightning_bolt");
    engine
        .apply_command(1, &cast_spell(bolt_index, target_object(solemn)))
        .expect("cast Lightning Bolt at Solemn Simulacrum");
    pass_both_players(&mut engine);
    assert_eq!(engine.state.objects[&solemn].zone, Zone::Graveyard);

    let death_batch = pass_both_and_capture(&mut engine);
    let death_branch = find_resolution_choice(&death_batch).expect("optional dies draw branch");
    assert_eq!(death_branch.deciding_player_id, 0);
    assert_eq!(death_branch.choice_kind(), ChoiceKind::ResolutionBranch);
    assert_eq!((death_branch.min, death_branch.max), (0, 1));

    engine
        .apply_command(
            0,
            &submit_resolution_decision(ResolutionChoiceDecision::SelectBranch),
        )
        .expect("controller chooses to draw");
    assert_eq!(engine.state.players[0].hand.len(), hand_before + 1);
    assert!(engine.state.players[0].hand.contains(&expected_draw));
    assert!(!engine.state.players[1].hand.contains(&expected_draw));
    assert!(engine.state.players[1].library.contains(&opponent_draw));
    assert!(engine.state.pending_resolution.is_none());
}

#[test]
fn solemn_simulacrum_can_decline_both_optional_abilities() {
    let mut engine = engine(906_202);
    let solemn = cast_solemn_simulacrum(&mut engine);
    let forest = inject_library_card(&mut engine, 0, "forest");
    let taiga = inject_library_card(&mut engine, 0, "taiga");
    let library_before = engine.state.players[0].library.clone();

    let entry_batch = pass_both_and_capture(&mut engine);
    let branch = find_resolution_choice(&entry_batch).expect("optional ETB search branch");
    assert_eq!(branch.deciding_player_id, 0);
    assert_eq!(branch.choice_kind(), ChoiceKind::ResolutionBranch);
    assert!(engine
        .apply_command(
            0,
            &submit_resolution_decision(ResolutionChoiceDecision::Decline),
        )
        .expect("decline the optional search")
        .events
        .iter()
        .all(|event| !matches!(
            &event.ev,
            Some(Ev::Log(log)) if log.text == "P0 shuffles their library."
        )));
    assert_eq!(engine.state.players[0].library, library_before);
    assert_eq!(engine.state.objects[&forest].zone, Zone::Library);
    assert_eq!(engine.state.objects[&taiga].zone, Zone::Library);

    let drawn_if_incorrectly_accepted = inject_library_card(&mut engine, 0, "island");
    let opponent_draw = inject_library_card(&mut engine, 1, "plains");
    let hand_before = engine.state.players[0].hand.len();
    pass_priority_to(&mut engine, 1);
    inject_card_into_hand(&mut engine, 1, "lightning_bolt");
    grant_pool(&mut engine, 1);
    let bolt_index = hand_index_for_card(&engine, 1, "lightning_bolt");
    engine
        .apply_command(1, &cast_spell(bolt_index, target_object(solemn)))
        .expect("cast Lightning Bolt at Solemn Simulacrum");
    pass_both_players(&mut engine);
    assert_eq!(engine.state.objects[&solemn].zone, Zone::Graveyard);

    let death_batch = pass_both_and_capture(&mut engine);
    let death_branch = find_resolution_choice(&death_batch).expect("optional dies draw branch");
    assert_eq!(death_branch.deciding_player_id, 0);
    assert_eq!(death_branch.choice_kind(), ChoiceKind::ResolutionBranch);
    let completion = engine
        .apply_command(
            0,
            &submit_resolution_decision(ResolutionChoiceDecision::Decline),
        )
        .expect("decline the optional draw");

    assert_eq!(engine.state.players[0].hand.len(), hand_before);
    assert!(!engine.state.players[0]
        .hand
        .contains(&drawn_if_incorrectly_accepted));
    assert!(engine.state.players[0]
        .library
        .contains(&drawn_if_incorrectly_accepted));
    assert!(engine.state.players[1].library.contains(&opponent_draw));
    assert!(completion.events.iter().all(|event| !matches!(
        &event.ev,
        Some(Ev::Log(log)) if log.text == "P0 shuffles their library."
    )));
    assert!(engine.state.pending_resolution.is_none());
}
