use super::helpers::*;
use tricerules_core::GameEngine;

fn hidden_lair_engine(seed: u64) -> GameEngine {
    let decks = Some(vec![
        deck_with("tropical_island", &["hidden_lair"]),
        deck_with("mountain", &[]),
    ]);
    let mut engine = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn mana_option(engine: &GameEngine, source: u32, ability_index: u32, option: u32) -> RuledCommand {
    let mut command = activate_ability_for(engine, source, ability_index, vec![]);
    let Some(Cmd::ActivateAbility(ability)) = command.cmd.as_mut() else {
        unreachable!()
    };
    ability.mana_option_index = option;
    command
}

#[test]
fn hidden_lair_colored_mana_is_legal_during_its_entry_turn() {
    let mut engine = hidden_lair_engine(202_001);
    let source = move_ready_to_battlefield(&mut engine, 0, "hidden_lair");

    assert_eq!(
        zone_view_ability_flags(&mut engine, 0, source),
        [true, true]
    );
    let priority_before = engine.state.priority_player_id();
    engine
        .apply_command(0, &mana_option(&engine, source, 1, 0))
        .expect("entry-turn branch enables blue mana");
    assert!(engine.state.objects[&source].tapped);
    assert_eq!(engine.state.players[0].mana_pool.blue, 1);
    assert!(
        engine.state.stack.is_empty(),
        "mana ability does not use the stack"
    );
    assert_eq!(engine.state.priority_player_id(), priority_before);
}

#[test]
fn hidden_lair_requires_either_its_entry_generation_or_a_controlled_basic() {
    let mut engine = hidden_lair_engine(202_002);
    let source = move_ready_to_battlefield(&mut engine, 0, "hidden_lair");
    engine.state.turn_history.finish_turn();

    assert_eq!(
        zone_view_ability_flags(&mut engine, 0, source),
        [true, false]
    );
    let mana_before = engine.state.players[0].mana_pool;
    let command_before = engine.state.command_index;
    engine
        .apply_command(0, &mana_option(&engine, source, 1, 0))
        .expect_err("neither branch holds");
    assert!(!engine.state.objects[&source].tapped);
    assert_eq!(engine.state.players[0].mana_pool, mana_before);
    assert_eq!(engine.state.command_index, command_before);

    inject_permanent_on_battlefield(&mut engine, 1, "mountain");
    assert_eq!(
        zone_view_ability_flags(&mut engine, 0, source),
        [true, false],
        "an opponent's basic does not satisfy 'you control'"
    );

    inject_permanent_on_battlefield(&mut engine, 0, "mountain");
    assert_eq!(
        zone_view_ability_flags(&mut engine, 0, source),
        [true, true]
    );
    engine
        .apply_command(0, &mana_option(&engine, source, 1, 1))
        .expect("controlled-basic branch enables black mana");
    assert_eq!(engine.state.players[0].mana_pool.black, 1);
}

#[test]
fn hidden_lair_entry_branch_tracks_exact_generation_across_control_changes() {
    let mut engine = hidden_lair_engine(202_003);
    let source = move_ready_to_battlefield(&mut engine, 0, "hidden_lair");

    engine.state.players[0]
        .battlefield
        .retain(|object_id| *object_id != source);
    engine.state.players[1].battlefield.push(source);
    let object = engine.state.objects.get_mut(&source).expect("Hidden Lair");
    object.base_controller = 1;
    object.controller = 1;
    engine.state.priority_idx = 1;
    assert_eq!(
        zone_view_ability_flags(&mut engine, 1, source),
        [true, true]
    );

    let stale = mana_option(&engine, source, 1, 0);
    *engine
        .state
        .zone_change_generation
        .entry(source)
        .or_insert(0) += 1;
    assert_eq!(
        zone_view_ability_flags(&mut engine, 1, source),
        [true, false]
    );
    engine
        .apply_command(1, &stale)
        .expect_err("old generation cannot activate the new object");
    assert!(!engine.state.objects[&source].tapped);
    assert_eq!(engine.state.players[1].mana_pool.blue, 0);
}
