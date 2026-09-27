//! Complete-card scenarios for Brokers Ascendancy in the pinned deck corpus.
//!
//! Oracle and the official Streets of New Capenna release notes confirm that a permanent that is
//! both a creature and a planeswalker gets both counter types. CR 122.1, 306.5, 513.1-513.2,
//! 603.2b-c, and 608.2h govern counters, planeswalker state-based actions, the end step, and
//! resolution-time battlefield characteristics.

use super::helpers::*;
use tricerules_cards::{CardRegistry, CounterKind};
use tricerules_core::{state::CopiableValues, GameEngine, TurnStep, Zone};

const BROKERS_ASCENDANCY: &str = "brokers_ascendancy";

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("plains", &[]), deck_with("island", &[])]);
    let mut engine = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn add_brokers(engine: &mut GameEngine, player: usize) -> u32 {
    inject_permanent_on_battlefield(engine, player, BROKERS_ASCENDANCY)
}

fn planeswalker_with_loyalty(
    engine: &mut GameEngine,
    player: usize,
    card_id: &str,
    loyalty: u32,
) -> u32 {
    let object = inject_permanent_on_battlefield(engine, player, card_id);
    engine
        .state
        .objects
        .get_mut(&object)
        .expect("planeswalker object")
        .counters
        .insert(CounterKind::Loyalty, loyalty);
    object
}

fn advance_to_end_step(engine: &mut GameEngine, active_player: i32) {
    assert_eq!(engine.state.turn_step, TurnStep::Main1);
    engine
        .apply_command(active_player, &primitive_yield())
        .expect("main 1 to beginning of combat");
    engine
        .apply_command(active_player, &primitive_yield())
        .expect("beginning of combat advance");
    if engine.state.turn_step == TurnStep::DeclareAttackers {
        engine
            .apply_command(active_player, &primitive_yield())
            .expect("declare no attackers");
    }
    engine
        .apply_command(active_player, &primitive_yield())
        .expect("end combat to main 2");
    engine
        .apply_command(active_player, &primitive_yield())
        .expect("main 2 to end step");
    assert_eq!(engine.state.turn_step, TurnStep::EndStep);
}

fn advance_to_main1_for(engine: &mut GameEngine, player: i32) {
    for _ in 0..120 {
        if engine.state.turn_step == TurnStep::Main1 && engine.state.active_player_id() == player {
            return;
        }
        if let Some(cleanup_player) = engine.state.cleanup_discard_player {
            let player_index = engine
                .state
                .player_idx(cleanup_player)
                .expect("cleanup player");
            let excess = engine.state.players[player_index].hand.len() - 7;
            engine
                .apply_command(
                    cleanup_player,
                    &discard_cleanup_batch((0..excess as u32).collect()),
                )
                .expect("discard to maximum hand size");
        } else {
            let priority_player = engine.state.priority_player_id();
            engine
                .apply_command(priority_player, &pass())
                .expect("pass toward the requested main phase");
        }
    }
    panic!("game did not reach player {player}'s main phase");
}

fn change_controller(engine: &mut GameEngine, object: u32, from: usize, to: usize) {
    engine.state.players[from]
        .battlefield
        .retain(|object_id| *object_id != object);
    engine.state.players[to].battlefield.push(object);
    let new_controller = engine.state.players[to].id;
    let changed = engine
        .state
        .objects
        .get_mut(&object)
        .expect("controlled permanent");
    changed.base_controller = new_controller;
    changed.controller = new_controller;
}

fn change_card_id(engine: &mut GameEngine, object: u32, card_id: &str) {
    engine
        .state
        .objects
        .get_mut(&object)
        .expect("permanent")
        .card_id = card_id.to_string();
}

#[test]
fn end_step_counters_are_delayed_and_limited_to_the_controller() {
    let mut engine = engine(202_609_270);
    add_brokers(&mut engine, 0);
    let own_creature = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let opponent_creature = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let own_planeswalker = planeswalker_with_loyalty(&mut engine, 0, "jace_beleren", 3);
    let opponent_planeswalker = planeswalker_with_loyalty(&mut engine, 1, "jace_beleren", 3);

    assert_eq!(engine.state.turn_step, TurnStep::Main1);
    assert_eq!(
        engine.state.objects[&own_creature].counter_count(CounterKind::PlusOnePlusOne),
        0
    );
    assert_eq!(
        engine.state.objects[&own_planeswalker].counter_count(CounterKind::Loyalty),
        3
    );

    advance_to_end_step(&mut engine, 0);
    assert_eq!(
        engine.state.stack.len(),
        1,
        "your end step creates one trigger"
    );
    assert_eq!(
        engine.state.objects[&own_creature].counter_count(CounterKind::PlusOnePlusOne),
        0
    );
    assert_eq!(
        engine.state.objects[&own_planeswalker].counter_count(CounterKind::Loyalty),
        3
    );
    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(
        engine.state.objects[&own_creature].counter_count(CounterKind::PlusOnePlusOne),
        1
    );
    assert_eq!(
        engine.state.objects[&own_planeswalker].counter_count(CounterKind::Loyalty),
        4
    );
    assert_eq!(
        engine.state.objects[&opponent_creature].counter_count(CounterKind::PlusOnePlusOne),
        0
    );
    assert_eq!(
        engine.state.objects[&opponent_planeswalker].counter_count(CounterKind::Loyalty),
        3
    );

    advance_to_main1_for(&mut engine, 1);
    advance_to_end_step(&mut engine, 1);
    assert!(
        engine.state.stack.is_empty(),
        "an opponent's end step does not trigger it"
    );
    assert_eq!(
        engine.state.objects[&opponent_creature].counter_count(CounterKind::PlusOnePlusOne),
        0
    );
    assert_eq!(
        engine.state.objects[&opponent_planeswalker].counter_count(CounterKind::Loyalty),
        3
    );
}

#[test]
fn resolution_uses_current_controller_and_permanent_types() {
    let mut engine = engine(202_609_271);
    add_brokers(&mut engine, 0);
    let own_creature_later_opponent_controlled =
        inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let opponent_creature_later_controller_controlled =
        inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let own_planeswalker_later_creature =
        planeswalker_with_loyalty(&mut engine, 0, "jace_beleren", 3);
    let own_creature_later_planeswalker =
        inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let opponent_planeswalker = planeswalker_with_loyalty(&mut engine, 1, "jace_beleren", 3);

    advance_to_end_step(&mut engine, 0);
    assert_eq!(engine.state.stack.len(), 1);

    change_controller(&mut engine, own_creature_later_opponent_controlled, 0, 1);
    change_controller(
        &mut engine,
        opponent_creature_later_controller_controlled,
        1,
        0,
    );
    change_card_id(
        &mut engine,
        own_planeswalker_later_creature,
        "grizzly_bears",
    );
    change_card_id(&mut engine, own_creature_later_planeswalker, "jace_beleren");
    engine
        .state
        .objects
        .get_mut(&own_creature_later_planeswalker)
        .expect("new planeswalker characteristics")
        .counters
        .insert(CounterKind::Loyalty, 3);

    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(
        engine.state.objects[&own_creature_later_opponent_controlled]
            .counter_count(CounterKind::PlusOnePlusOne),
        0
    );
    assert_eq!(
        engine.state.objects[&opponent_creature_later_controller_controlled]
            .counter_count(CounterKind::PlusOnePlusOne),
        1
    );
    assert_eq!(
        engine.state.objects[&own_planeswalker_later_creature]
            .counter_count(CounterKind::PlusOnePlusOne),
        1
    );
    assert_eq!(
        engine.state.objects[&own_planeswalker_later_creature].counter_count(CounterKind::Loyalty),
        3
    );
    assert_eq!(
        engine.state.objects[&own_creature_later_planeswalker]
            .counter_count(CounterKind::PlusOnePlusOne),
        0
    );
    assert_eq!(
        engine.state.objects[&own_creature_later_planeswalker].counter_count(CounterKind::Loyalty),
        4
    );
    assert_eq!(
        engine.state.objects[&opponent_planeswalker].counter_count(CounterKind::Loyalty),
        3
    );
}

#[test]
fn a_dual_creature_planeswalker_gets_both_counter_types() {
    let mut engine = engine(202_609_272);
    add_brokers(&mut engine, 0);
    let dual = planeswalker_with_loyalty(&mut engine, 0, "jace_beleren", 3);
    let mut face = CardRegistry::global()
        .get("jace_beleren")
        .expect("Jace Beleren")
        .primary_face()
        .clone();
    face.types.push("Creature".to_string());
    face.power = Some(1);
    face.toughness = Some(1);
    engine
        .state
        .objects
        .get_mut(&dual)
        .expect("dual-type permanent")
        .copiable_values = Some(CopiableValues {
        source_card_id: "jace_beleren".to_string(),
        source_face_index: 0,
        face,
        room_faces: None,
        display_name: "Dual-type Jace test permanent".to_string(),
    });
    let characteristics = engine.characteristics(dual).expect("dual characteristics");
    assert!(characteristics.types.iter().any(|kind| kind == "Creature"));
    assert!(characteristics
        .types
        .iter()
        .any(|kind| kind == "Planeswalker"));

    advance_to_end_step(&mut engine, 0);
    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(
        engine.state.objects[&dual].counter_count(CounterKind::PlusOnePlusOne),
        1
    );
    assert_eq!(
        engine.state.objects[&dual].counter_count(CounterKind::Loyalty),
        4
    );
}

#[test]
fn a_triggered_ability_resolves_after_brokers_ascendancy_leaves() {
    let mut engine = engine(202_609_273);
    let source = add_brokers(&mut engine, 0);
    let creature = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let planeswalker = planeswalker_with_loyalty(&mut engine, 0, "jace_beleren", 3);

    advance_to_end_step(&mut engine, 0);
    assert_eq!(engine.state.stack.len(), 1);
    engine.state.players[0]
        .battlefield
        .retain(|object_id| *object_id != source);
    engine.state.players[0].graveyard.push(source);
    engine.state.objects.get_mut(&source).expect("source").zone = Zone::Graveyard;

    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(
        engine.state.objects[&creature].counter_count(CounterKind::PlusOnePlusOne),
        1
    );
    assert_eq!(
        engine.state.objects[&planeswalker].counter_count(CounterKind::Loyalty),
        4
    );
}

#[test]
fn an_ascendancy_entering_after_the_end_step_begins_waits_until_its_next_end_step() {
    let mut engine = engine(202_609_274);
    advance_to_end_step(&mut engine, 0);
    assert!(
        engine.state.stack.is_empty(),
        "there was no source as the step began"
    );

    add_brokers(&mut engine, 0);
    let creature = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    assert!(
        engine.state.stack.is_empty(),
        "a late entry does not trigger retroactively"
    );

    advance_to_main1_for(&mut engine, 0);
    advance_to_end_step(&mut engine, 0);
    assert_eq!(
        engine.state.stack.len(),
        1,
        "the source is present at the next own end step"
    );
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        engine.state.objects[&creature].counter_count(CounterKind::PlusOnePlusOne),
        1
    );
}
