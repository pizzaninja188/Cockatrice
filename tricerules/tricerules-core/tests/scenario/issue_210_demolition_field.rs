use super::helpers::*;
use tricerules_core::{GameEngine, Zone};
use tricerules_proto::ruled::v1::{
    ruled_command::Cmd, ruled_event::Ev, ChoiceKind, ResolutionChoiceDecision, RuledCommand,
    SubmitResolutionChoice,
};

fn select_branch(index: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
            decision: ResolutionChoiceDecision::SelectBranch as i32,
            selected_branch_index: index,
            ..Default::default()
        })),
    }
}

fn setup(seed: u64) -> (GameEngine, u32, u32, u32, u32) {
    let decks = Some(vec![
        deck_with("forest", &["demolition_field", "taiga"]),
        deck_with("island", &["taiga"]),
    ]);
    let mut engine = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    let field = relocate_to_battlefield(&mut engine, 0, "demolition_field", false);
    let target = relocate_to_battlefield(&mut engine, 0, "taiga", false);
    engine.state.players[0]
        .battlefield
        .retain(|object_id| *object_id != target);
    engine.state.players[1].battlefield.push(target);
    let opponent = engine.state.players[1].id;
    let object = engine.state.objects.get_mut(&target).expect("target land");
    object.base_controller = opponent;
    object.controller = opponent;
    let forest = inject_library_card(&mut engine, 0, "forest");
    let island = inject_library_card(&mut engine, 1, "island");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );
    (engine, field, target, forest, island)
}

fn activate_and_resolve_to_first_prompt(
    engine: &mut GameEngine,
    field: u32,
    target: u32,
) -> RuledEventBatch {
    apply_ability(engine, 0, field, 1, target_object(target)).expect("activate Demolition Field");
    assert_eq!(engine.state.objects[&field].zone, Zone::Graveyard);
    engine.apply_command(0, &pass()).expect("controller passes");
    engine
        .apply_command(1, &pass())
        .expect("opponent passes and ability begins resolving")
}

fn shuffle_logs(batch: &RuledEventBatch, player: i32) -> usize {
    let expected = format!("P{player} shuffles their library.");
    batch
        .events
        .iter()
        .filter(|event| matches!(&event.ev, Some(Ev::Log(log)) if log.text == expected))
        .count()
}

#[test]
fn issue_210_uses_destroyed_targets_last_controller_then_resumes_for_activator() {
    let (mut engine, field, target, forest, island) = setup(210_001);
    let first = activate_and_resolve_to_first_prompt(&mut engine, field, target);

    assert_eq!(engine.state.objects[&target].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&target].owner, 0);
    assert!(engine.state.players[0].graveyard.contains(&target));
    let choice = find_resolution_choice(&first).expect("target controller optional search");
    assert_eq!(choice.deciding_player_id, 1);
    assert_eq!(choice.choice_kind(), ChoiceKind::ResolutionBranch);
    assert_eq!((choice.min, choice.max), (0, 1));
    assert_eq!(choice.resolution_branches.len(), 1);

    assert!(
        engine
            .apply_command(
                0,
                &submit_resolution_decision(ResolutionChoiceDecision::SelectBranch),
            )
            .is_err(),
        "the activator cannot answer the target controller's search"
    );
    assert!(engine.apply_command(1, &select_branch(1)).is_err());
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .expect("malformed choice preserves prompt")
            .deciding_player,
        1
    );
    let search = engine
        .apply_command(
            1,
            &submit_resolution_decision(ResolutionChoiceDecision::SelectBranch),
        )
        .expect("target controller elects to search");
    let choice = find_resolution_choice(&search).expect("private library search");
    assert_eq!(choice.deciding_player_id, 1);
    assert_eq!(choice.choice_kind(), ChoiceKind::LibrarySearch);
    assert!(choice.candidate_object_ids.contains(&island));
    assert!(!choice.candidate_object_ids.contains(&forest));

    let second = engine
        .apply_command(1, &submit_resolution_choice(vec![island]))
        .expect("target controller finds Island");
    assert_eq!(engine.state.objects[&island].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&island].controller, 1);
    assert!(!engine.state.objects[&island].tapped);
    let choice = find_resolution_choice(&second).expect("activator optional search");
    assert_eq!(choice.deciding_player_id, 0);
    assert_eq!(choice.choice_kind(), ChoiceKind::ResolutionBranch);

    let completion = engine
        .apply_command(
            0,
            &submit_resolution_decision(ResolutionChoiceDecision::Decline),
        )
        .expect("activator declines independently");
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.players[0].library.contains(&forest));
    assert_eq!(shuffle_logs(&completion, 0), 0);
}

#[test]
fn issue_210_decline_does_not_shuffle_or_skip_the_second_search() {
    let (mut engine, field, target, forest, _) = setup(210_002);
    let library_before = engine.state.players[1].library.clone();
    activate_and_resolve_to_first_prompt(&mut engine, field, target);

    let second = engine
        .apply_command(
            1,
            &submit_resolution_decision(ResolutionChoiceDecision::Decline),
        )
        .expect("target controller declines");
    assert_eq!(engine.state.players[1].library, library_before);
    assert_eq!(shuffle_logs(&second, 1), 0);
    let choice = find_resolution_choice(&second).expect("second optional search still occurs");
    assert_eq!(choice.deciding_player_id, 0);

    engine
        .apply_command(
            0,
            &submit_resolution_decision(ResolutionChoiceDecision::SelectBranch),
        )
        .expect("activator elects to search");
    let completion = engine
        .apply_command(0, &submit_resolution_choice(Vec::new()))
        .expect("qualified hidden search may fail to find");
    assert!(engine.state.players[0].library.contains(&forest));
    assert_eq!(shuffle_logs(&completion, 0), 1);
    assert!(engine.state.pending_resolution.is_none());
}

#[test]
fn issue_210_basic_and_own_lands_are_not_legal_targets() {
    let (mut engine, field, _, _, _) = setup(210_003);
    inject_library_card(&mut engine, 0, "taiga");
    let own_nonbasic = relocate_to_battlefield(&mut engine, 0, "taiga", false);
    let opponent_basic = relocate_to_battlefield(&mut engine, 1, "island", false);
    assert!(apply_ability(&mut engine, 0, field, 1, target_object(own_nonbasic)).is_err());
    assert!(apply_ability(&mut engine, 0, field, 1, target_object(opponent_basic)).is_err());
}

#[test]
fn issue_210_illegal_target_fizzles_before_either_search() {
    let (mut engine, field, target, _, _) = setup(210_004);
    apply_ability(&mut engine, 0, field, 1, target_object(target)).expect("activate");
    engine.state.players[1]
        .battlefield
        .retain(|object_id| *object_id != target);
    engine.state.players[0].battlefield.push(target);
    let controller = engine.state.players[0].id;
    let object = engine.state.objects.get_mut(&target).unwrap();
    object.base_controller = controller;
    object.controller = controller;

    engine.apply_command(0, &pass()).expect("controller passes");
    let completion = engine.apply_command(1, &pass()).expect("ability fizzles");
    assert!(find_resolution_choice(&completion).is_none());
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(engine.state.objects[&target].zone, Zone::Battlefield);
}

#[test]
fn issue_210_indestructible_legal_target_still_allows_its_controller_to_search() {
    let decks = Some(vec![
        deck_with("forest", &["demolition_field"]),
        deck_with("island", &["darksteel_citadel"]),
    ]);
    let mut engine = GameEngine::new(210_005, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    let field = relocate_to_battlefield(&mut engine, 0, "demolition_field", false);
    let citadel = relocate_to_battlefield(&mut engine, 1, "darksteel_citadel", false);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );
    let first = activate_and_resolve_to_first_prompt(&mut engine, field, citadel);
    assert_eq!(engine.state.objects[&citadel].zone, Zone::Battlefield);
    assert_eq!(
        find_resolution_choice(&first)
            .expect("search after failed destruction")
            .deciding_player_id,
        1
    );
}

#[test]
fn issue_210_rejects_a_stale_private_candidate_without_losing_the_search() {
    let (mut engine, field, target, _, island) = setup(210_006);
    activate_and_resolve_to_first_prompt(&mut engine, field, target);
    engine
        .apply_command(1, &select_branch(0))
        .expect("elect to search");
    *engine
        .state
        .zone_change_generation
        .entry(island)
        .or_insert(0) += 1;

    assert!(
        engine
            .apply_command(1, &submit_resolution_choice(vec![island]))
            .is_err(),
        "the published incarnation is stale"
    );
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .expect("rejected choice remains pending")
            .deciding_player,
        1
    );
    engine
        .apply_command(1, &submit_resolution_choice(Vec::new()))
        .expect("search can recover by failing to find");
}
