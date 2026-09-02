use crate::helpers::*;
use tricerules_cards::primitives::{CounterKind, Keyword, TriggerCondition};
use tricerules_core::state::CopiableValues;
use tricerules_core::{GameEngine, Zone};
use tricerules_proto::ruled::v1::{
    dev_command, ruled_command::Cmd, ruled_event::Ev, ChooseTriggerTarget, DevCommand, DevMoveCard,
    DevZone, ResolutionChoiceDecision, RuledCommand, SubmitResolutionChoice, TargetRef,
};

fn choose_trigger_targets(targets: Vec<TargetRef>) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
            decline: false,
            selected_modes: Vec::new(),
            targets,
        })),
    }
}

fn choose_read_ahead(branch_index: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
            decision: ResolutionChoiceDecision::SelectBranch as i32,
            selected_branch_index: branch_index,
            ..Default::default()
        })),
    }
}

#[test]
fn saga_entry_places_lore_and_stages_the_crossed_chapter() {
    let decks = Some(vec![
        deck_with("mountain", &["burn,_burn,_tree_and_fern"]),
        deck_with("forest", &["grizzly_bears"]),
    ]);
    let mut engine = GameEngine::new(185_001, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut engine);
    let target = relocate_to_battlefield(&mut engine, 1, "grizzly_bears", false);

    let saga = move_ready_to_battlefield(&mut engine, 0, "burn,_burn,_tree_and_fern");
    assert_eq!(
        engine.state.objects[&saga].counter_count(CounterKind::Lore),
        1
    );
    let pending = engine
        .state
        .pending_triggers
        .front()
        .expect("chapter I awaits its target");
    assert_eq!(
        pending.ability.trigger,
        TriggerCondition::SagaChapter { chapters: vec![1] }
    );

    engine
        .apply_command(0, &choose_trigger_targets(target_object(target)))
        .expect("choose chapter I target");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&target].zone, Zone::Graveyard);
}

#[test]
fn chapter_two_targets_and_destroys_an_opponents_artifact() {
    let decks = Some(vec![
        deck_with("mountain", &["burn,_burn,_tree_and_fern"]),
        deck_with("forest", &["bonesplitter"]),
    ]);
    let mut engine = GameEngine::new(185_009, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut engine);
    let saga = relocate_to_battlefield(&mut engine, 0, "burn,_burn,_tree_and_fern", false);
    engine
        .state
        .objects
        .get_mut(&saga)
        .expect("Saga")
        .set_counter(CounterKind::Lore, 1);
    let artifact = relocate_to_battlefield(&mut engine, 1, "bonesplitter", false);
    engine.state.turn_step = tricerules_core::TurnStep::Draw;
    engine.state.priority_idx = 0;
    engine.state.passes_since_stack_change = 0;

    pass_both_players(&mut engine);
    assert_eq!(
        engine.state.objects[&saga].counter_count(CounterKind::Lore),
        2
    );
    engine
        .apply_command(0, &choose_trigger_targets(target_object(artifact)))
        .expect("choose chapter II target");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&artifact].zone, Zone::Graveyard);
}

#[test]
fn precombat_lore_waits_for_the_final_chapter_then_sacrifices_the_saga() {
    let decks = Some(vec![
        deck_with("mountain", &["burn,_burn,_tree_and_fern"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(185_002, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut engine);
    let saga = relocate_to_battlefield(&mut engine, 0, "burn,_burn,_tree_and_fern", false);
    engine
        .state
        .objects
        .get_mut(&saga)
        .expect("Saga")
        .set_counter(CounterKind::Lore, 3);
    engine.state.turn_step = tricerules_core::TurnStep::Draw;
    engine.state.priority_idx = 0;
    engine.state.passes_since_stack_change = 0;

    pass_both_players(&mut engine);
    assert_eq!(engine.state.turn_step, tricerules_core::TurnStep::Main1);
    assert_eq!(
        engine.state.objects[&saga].counter_count(CounterKind::Lore),
        4
    );
    assert_eq!(engine.state.objects[&saga].zone, Zone::Battlefield);
    assert_eq!(
        engine.state.stack.len(),
        1,
        "chapter IV must be on the stack"
    );

    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.players[0].mana_pool.red, 1);
    assert_eq!(engine.state.objects[&saga].zone, Zone::Graveyard);
}

#[test]
fn read_ahead_uses_a_logged_branch_and_suppresses_skipped_chapters() {
    let decks = Some(vec![
        deck_with("mountain", &["burn,_burn,_tree_and_fern"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(185_003, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut engine);
    let saga = engine
        .state
        .objects
        .iter()
        .find_map(|(object_id, object)| {
            (object.owner == 0 && object.card_id == "burn,_burn,_tree_and_fern")
                .then_some(*object_id)
        })
        .expect("Saga in a player-owned zone");
    let starting_zone = engine.state.objects[&saga].zone;
    let mut face = tricerules_cards::CardRegistry::global()
        .get("burn,_burn,_tree_and_fern")
        .expect("Burn")
        .primary_face()
        .clone();
    face.keywords.push(Keyword::ReadAhead);
    face.static_abilities
        .retain(|ability| ability.ability_id.as_str() != "intrinsic_saga_lore");
    engine
        .state
        .objects
        .get_mut(&saga)
        .expect("Saga")
        .copiable_values = Some(CopiableValues {
        source_card_id: "burn,_burn,_tree_and_fern".into(),
        source_face_index: 0,
        display_name: "Burn, Burn, Tree and Fern".into(),
        room_faces: None,
        face,
    });

    engine.enable_dev_commands();
    let batch = engine
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::DevCommand(DevCommand {
                    target_player_id: 0,
                    dev: Some(dev_command::Dev::MoveCard(DevMoveCard {
                        card_name: "Burn, Burn, Tree and Fern".into(),
                        zone: DevZone::Battlefield as i32,
                        ready: true,
                    })),
                })),
            },
        )
        .expect("begin read-ahead entry");
    let choice = batch
        .events
        .iter()
        .find_map(|event| match event.ev.as_ref() {
            Some(Ev::ResolutionChoiceRequired(choice)) => Some(choice),
            _ => None,
        })
        .expect("read-ahead choice");
    assert_eq!(choice.resolution_branches.len(), 4);
    assert_eq!(
        choice
            .resolution_branches
            .iter()
            .map(|branch| branch.label.as_str())
            .collect::<Vec<_>>(),
        vec!["I", "II", "III", "IV"]
    );
    assert_eq!(engine.state.objects[&saga].zone, starting_zone);

    assert!(engine.apply_command(1, &choose_read_ahead(2)).is_err());
    assert_eq!(engine.state.objects[&saga].zone, starting_zone);
    assert!(engine.apply_command(0, &choose_read_ahead(4)).is_err());
    assert_eq!(engine.state.objects[&saga].zone, starting_zone);
    engine
        .apply_command(0, &choose_read_ahead(2))
        .expect("choose chapter III");
    assert_eq!(engine.state.objects[&saga].zone, Zone::Battlefield);
    assert_eq!(
        engine.state.objects[&saga].counter_count(CounterKind::Lore),
        3
    );
    assert_eq!(engine.state.stack.len(), 1, "only chapter III triggers");
    assert_eq!(
        engine.state.stack[0]
            .triggered_ability
            .as_ref()
            .expect("chapter ability")
            .trigger,
        TriggerCondition::SagaChapter {
            chapters: vec![3, 4]
        }
    );
}

#[test]
fn countering_the_final_chapter_allows_the_next_sba_to_sacrifice() {
    let decks = Some(vec![
        deck_with("mountain", &["burn,_burn,_tree_and_fern"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(185_004, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut engine);
    let saga = relocate_to_battlefield(&mut engine, 0, "burn,_burn,_tree_and_fern", false);
    engine
        .state
        .objects
        .get_mut(&saga)
        .expect("Saga")
        .set_counter(CounterKind::Lore, 3);
    engine.state.turn_step = tricerules_core::TurnStep::Draw;
    engine.state.priority_idx = 0;
    engine.state.passes_since_stack_change = 0;
    pass_both_players(&mut engine);

    let countered = engine.state.stack.pop().expect("final chapter on stack");
    engine.state.stack_presentations.remove(&countered.id);
    engine
        .apply_command(0, &pass())
        .expect("next priority action checks SBAs");
    assert_eq!(engine.state.objects[&saga].zone, Zone::Graveyard);
}

#[test]
fn removing_lore_before_the_final_chapter_leaves_the_stack_prevents_sacrifice() {
    let decks = Some(vec![
        deck_with("mountain", &["burn,_burn,_tree_and_fern"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(185_005, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut engine);
    let saga = relocate_to_battlefield(&mut engine, 0, "burn,_burn,_tree_and_fern", false);
    engine
        .state
        .objects
        .get_mut(&saga)
        .expect("Saga")
        .set_counter(CounterKind::Lore, 3);
    engine.state.turn_step = tricerules_core::TurnStep::Draw;
    engine.state.priority_idx = 0;
    engine.state.passes_since_stack_change = 0;
    pass_both_players(&mut engine);
    engine
        .state
        .objects
        .get_mut(&saga)
        .expect("Saga")
        .set_counter(CounterKind::Lore, 3);

    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&saga].zone, Zone::Battlefield);
    assert_eq!(
        engine.state.objects[&saga].counter_count(CounterKind::Lore),
        3
    );
}

#[test]
fn chapter_controller_is_locked_at_trigger_time_but_the_current_controller_sacrifices() {
    let decks = Some(vec![
        deck_with("mountain", &["burn,_burn,_tree_and_fern"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(185_006, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut engine);
    let saga = relocate_to_battlefield(&mut engine, 0, "burn,_burn,_tree_and_fern", false);
    {
        let object = engine.state.objects.get_mut(&saga).expect("Saga");
        object.base_controller = 1;
        object.controller = 1;
        object.set_counter(CounterKind::Lore, 3);
    }
    engine.state.players[0]
        .battlefield
        .retain(|object_id| *object_id != saga);
    engine.state.players[1].battlefield.push(saga);
    engine.state.active_player_idx = 1;
    engine.state.priority_idx = 1;
    engine.state.turn_step = tricerules_core::TurnStep::Draw;
    engine.state.passes_since_stack_change = 0;
    pass_both_players(&mut engine);
    assert_eq!(engine.state.stack[0].controller, 1);

    engine
        .state
        .objects
        .get_mut(&saga)
        .expect("Saga")
        .base_controller = 0;
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&saga].zone, Zone::Graveyard);
    assert!(engine
        .state
        .turn_history
        .current
        .permanents_sacrificed
        .iter()
        .any(|fact| fact.object_id == saga && fact.player == 0));
}

#[test]
fn saga_leave_and_reentry_starts_a_fresh_generation_at_chapter_one() {
    let decks = Some(vec![
        deck_with("mountain", &["burn,_burn,_tree_and_fern"]),
        deck_with("forest", &["grizzly_bears", "grizzly_bears"]),
    ]);
    let mut engine = GameEngine::new(185_007, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut engine);
    let first_target = relocate_to_battlefield(&mut engine, 1, "grizzly_bears", false);
    let second_target = relocate_to_battlefield(&mut engine, 1, "grizzly_bears", false);
    let saga = move_ready_to_battlefield(&mut engine, 0, "burn,_burn,_tree_and_fern");
    engine
        .apply_command(0, &choose_trigger_targets(target_object(first_target)))
        .expect("first chapter I target");
    resolve_entire_stack_two_player(&mut engine);
    let first_generation = engine
        .state
        .zone_change_generation
        .get(&saga)
        .copied()
        .unwrap_or(0);

    engine
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::DevCommand(DevCommand {
                    target_player_id: 0,
                    dev: Some(dev_command::Dev::MoveCard(DevMoveCard {
                        card_name: "Burn, Burn, Tree and Fern".into(),
                        zone: DevZone::Graveyard as i32,
                        ready: false,
                    })),
                })),
            },
        )
        .expect("move Saga away");
    assert_eq!(engine.state.objects[&saga].zone, Zone::Graveyard);
    assert_eq!(
        engine.state.objects[&saga].counter_count(CounterKind::Lore),
        0
    );

    let returned = move_ready_to_battlefield(&mut engine, 0, "burn,_burn,_tree_and_fern");
    assert_eq!(
        returned, saga,
        "physical object id is retained across the move"
    );
    assert!(
        engine
            .state
            .zone_change_generation
            .get(&saga)
            .copied()
            .unwrap_or(0)
            > first_generation
    );
    assert_eq!(
        engine.state.objects[&saga].counter_count(CounterKind::Lore),
        1
    );
    assert_eq!(engine.state.pending_triggers.len(), 1);
    engine
        .apply_command(0, &choose_trigger_targets(target_object(second_target)))
        .expect("new chapter I target");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&second_target].zone, Zone::Graveyard);
}

#[test]
fn precombat_lore_is_controller_scoped_and_final_triggers_wait_in_ordering() {
    let decks = Some(vec![
        deck_with("mountain", &["burn,_burn,_tree_and_fern"]),
        deck_with(
            "mountain",
            &["burn,_burn,_tree_and_fern", "burn,_burn,_tree_and_fern"],
        ),
    ]);
    let mut engine = GameEngine::new(185_008, &[0, 1], 20, decks, true).expect("new");
    let inactive = relocate_to_battlefield(&mut engine, 0, "burn,_burn,_tree_and_fern", false);
    let active_first = relocate_to_battlefield(&mut engine, 1, "burn,_burn,_tree_and_fern", false);
    let active_second = relocate_to_battlefield(&mut engine, 1, "burn,_burn,_tree_and_fern", false);
    for saga in [inactive, active_first, active_second] {
        engine
            .state
            .objects
            .get_mut(&saga)
            .expect("Saga")
            .set_counter(CounterKind::Lore, 3);
    }
    engine.state.active_player_idx = 1;
    engine.state.priority_idx = 1;
    engine.state.turn_step = tricerules_core::TurnStep::Draw;
    engine.state.passes_since_stack_change = 0;
    for _ in 0..2 {
        let player = engine.state.priority_player_id();
        engine
            .apply_command(player, &pass())
            .expect("priority pass");
    }

    assert_eq!(
        engine.state.objects[&inactive].counter_count(CounterKind::Lore),
        3,
        "another player's Saga does not receive the turn-based lore counter"
    );
    for saga in [active_first, active_second] {
        assert_eq!(
            engine.state.objects[&saga].counter_count(CounterKind::Lore),
            4
        );
        assert_eq!(engine.state.objects[&saga].zone, Zone::Battlefield);
    }
    let order = engine
        .state
        .pending_trigger_order
        .as_ref()
        .expect("active player orders simultaneous chapters");
    assert_eq!(order.deciding_player, 1);
    assert_eq!(order.candidates.len(), 2);
}
