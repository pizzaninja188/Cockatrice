//! Issue #433 — Infinite Guideline Station counts multicolored permanents you control.
//!
//! Oracle (pinned `cda39514-86c9-483e-8d95-fd08138ce33a`, no rulings) checked 2026-11-23 with
//! the 2026-11-06 Comprehensive Rules. A multicolored object is two or more colors
//! (CR 105.2b); colorless objects have none (CR 105.2c). Colors are read after copy and
//! color-setting effects (CR 613.1a/613.1e), and the count is taken when each effect is applied
//! (CR 608.2h). The attack trigger is printed in the 12+ striation, so it is part of that station
//! symbol's static ability (CR 722.2-722.2b). Tokens follow CR 111.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_cards::{Color, ContinuousEffectKind, CounterKind, EffectDuration};
use tricerules_core::{AffectedScope, ContinuousEffect, TurnStep};

const STATION: &str = "infinite_guideline_station";

fn station_engine(seed: u64, players: i32) -> GameEngine {
    let ids: Vec<i32> = (0..players).collect();
    let mut decks = vec![deck_with("forest", &[STATION])];
    decks.extend((1..players).map(|_| deck_with("island", &[])));
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &ids,
        20,
        Some(decks),
        true,
    )
    .expect("Infinite Guideline Station must be registered");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn resolve_stack(engine: &mut GameEngine) {
    loop {
        answer_trigger_order_in_engine_order(engine);
        if engine.state.stack.is_empty() {
            break;
        }
        pass_priority_round(engine);
    }
    assert!(engine.state.pending_resolution.is_none());
}

fn robots(engine: &GameEngine, player: usize) -> Vec<u32> {
    engine.state.players[player]
        .battlefield
        .iter()
        .copied()
        .filter(|oid| engine.state.objects[oid].card_id == "robot_c_2_2")
        .collect()
}

fn set_colors(engine: &mut GameEngine, oid: u32, colors: Vec<Color>) {
    engine.state.continuous_effects.push(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: None,
        affected: AffectedScope::Single(oid),
        kind: ContinuousEffectKind::Layer5SetColors(colors),
        condition: None,
        duration: EffectDuration::UntilEndOfTurn,
        timestamp: engine.state.command_index,
    });
    engine.initial_response_batch();
}

/// Monocolored Grizzly Bears, two-color Bartolome, three-color Jacques, a colorless Memnite and
/// Boros Signet, and the green-blue artifact creature Shardless Agent.
struct Board {
    grizzly: u32,
    bartolome: u32,
    jacques: u32,
}

fn mixed_board(engine: &mut GameEngine) -> Board {
    let board = Board {
        grizzly: inject_creature_on_battlefield(engine, 0, "grizzly_bears"),
        bartolome: inject_creature_on_battlefield(engine, 0, "bartolome_del_presidio"),
        jacques: inject_creature_on_battlefield(engine, 0, "jacques_le_vert"),
    };
    inject_creature_on_battlefield(engine, 0, "memnite");
    inject_creature_on_battlefield(engine, 0, "shardless_agent");
    inject_permanent_on_battlefield(engine, 0, "boros_signet");
    board
}

#[test]
fn issue_433_entry_creates_a_tapped_robot_for_each_multicolored_permanent_you_control() {
    let mut engine = station_engine(433_001, 2);
    mixed_board(&mut engine);
    // An opponent's multicolored permanent is outside "you control".
    inject_creature_on_battlefield(&mut engine, 1, "bartolome_del_presidio");

    let station = move_ready_to_battlefield(&mut engine, 0, STATION);
    resolve_stack(&mut engine);

    // Bartolome, Jacques, Shardless Agent (counted once as an artifact creature), and the
    // five-color Station itself. Grizzly Bears, Memnite, and Boros Signet do not count.
    let created = robots(&engine, 0);
    assert_eq!(created.len(), 4);
    assert!(created.iter().all(|oid| engine.state.objects[oid].tapped));
    assert!(robots(&engine, 1).is_empty());
    let characteristics = engine.characteristics(station).expect("Station");
    assert!(!characteristics.has_type("Creature"));
}

#[test]
fn issue_433_color_changes_made_in_response_change_the_resolution_count() {
    let mut engine = station_engine(433_002, 2);
    let board = mixed_board(&mut engine);

    move_ready_to_battlefield(&mut engine, 0, STATION);
    answer_trigger_order_in_engine_order(&mut engine);
    assert_eq!(
        engine.state.stack.len(),
        1,
        "entry trigger waits on the stack"
    );

    // CR 613.1e: monocolored Grizzly Bears becomes white and blue; three-color Jacques becomes
    // monocolored red; Bartolome becomes colorless.
    set_colors(&mut engine, board.grizzly, vec![Color::White, Color::Blue]);
    set_colors(&mut engine, board.jacques, vec![Color::Red]);
    set_colors(&mut engine, board.bartolome, Vec::new());
    resolve_stack(&mut engine);

    // Grizzly Bears, Shardless Agent, and the Station.
    assert_eq!(robots(&engine, 0).len(), 3);
}

#[test]
fn issue_433_station_twelve_attacks_and_draws_for_each_multicolored_permanent() {
    let mut engine = station_engine(433_003, 2);
    mixed_board(&mut engine);
    let station = move_ready_to_battlefield(&mut engine, 0, STATION);
    resolve_stack(&mut engine);
    assert_eq!(robots(&engine, 0).len(), 4);

    engine
        .state
        .objects
        .get_mut(&station)
        .unwrap()
        .set_counter(CounterKind::Charge, 11);
    engine.initial_response_batch();
    assert!(!engine
        .characteristics(station)
        .unwrap()
        .has_type("Creature"));

    engine
        .state
        .objects
        .get_mut(&station)
        .unwrap()
        .set_counter(CounterKind::Charge, 12);
    engine
        .state
        .objects
        .get_mut(&station)
        .unwrap()
        .summoning_sick = false;
    engine.initial_response_batch();
    let animated = engine.characteristics(station).unwrap();
    assert!(animated.has_type("Creature"));
    assert_eq!((animated.power, animated.toughness), (Some(7), Some(15)));

    engine
        .apply_scenario_command(0, &primitive_yield())
        .expect("main phase to beginning of combat");
    pass_priority_round(&mut engine);
    assert_eq!(engine.state.turn_step, TurnStep::DeclareAttackers);
    let hand_before = engine.state.players[0].hand.len();
    engine
        .apply_scenario_command(0, &declare_attackers(vec![station]))
        .expect("the 12+ Station attacks");
    resolve_stack(&mut engine);

    // The colorless Robot tokens add nothing; the same four multicolored permanents remain.
    assert_eq!(engine.state.players[0].hand.len(), hand_before + 4);
}

#[test]
fn issue_433_multiplayer_counts_only_the_controllers_multicolored_permanents() {
    let mut engine = station_engine(433_004, 3);
    inject_creature_on_battlefield(&mut engine, 0, "bartolome_del_presidio");
    inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    inject_creature_on_battlefield(&mut engine, 0, "memnite");
    inject_creature_on_battlefield(&mut engine, 1, "jacques_le_vert");
    inject_creature_on_battlefield(&mut engine, 2, "shardless_agent");
    inject_creature_on_battlefield(&mut engine, 2, "bartolome_del_presidio");

    move_ready_to_battlefield(&mut engine, 0, STATION);
    resolve_stack(&mut engine);

    // Bartolome and the Station; both opponents' multicolored permanents are excluded.
    assert_eq!(robots(&engine, 0).len(), 2);
    assert!(robots(&engine, 1).is_empty());
    assert!(robots(&engine, 2).is_empty());
}
