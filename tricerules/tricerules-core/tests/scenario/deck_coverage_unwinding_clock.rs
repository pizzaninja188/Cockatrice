//! Unwinding Clock: simultaneous, mandatory controlled-artifact untap on other players' turns.
use crate::helpers::*;
use tricerules_cards::primitives::{ContinuousEffectKind, EffectDuration};
use tricerules_cards::CardRegistry;
use tricerules_core::{AffectedScope, ContinuousEffect, TurnStep, Zone};
use tricerules_proto::ruled::v1::{dev_command, DevCommand, DevMoveCard, DevZone};

const CLOCK: &str = "unwinding_clock";

#[test]
fn clock_actual_cast_untaps_only_its_controllers_artifacts_on_another_players_turn() {
    assert!(
        CardRegistry::global().get(CLOCK).is_some(),
        "missing exact Unwinding Clock"
    );
    let deck = deck_with("forest", &["sol_ring", "grizzly_bears"]);
    let mut engine = GameEngine::new(2026093101, &[0, 1], 20, Some(vec![deck; 2]), true).unwrap();
    advance_to_main1_from_game_start(&mut engine);
    let ring = move_ready_to_battlefield(&mut engine, 0, "sol_ring");
    let bear = move_ready_to_battlefield(&mut engine, 0, "grizzly_bears");
    let other_ring = move_ready_to_battlefield(&mut engine, 1, "sol_ring");
    let source = inject_card_into_hand(&mut engine, 0, CLOCK);
    let slot = hand_index_for_card(&engine, 0, CLOCK);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 3,
            ..Default::default()
        },
    );
    let before = serde_json::to_value(&engine.state).unwrap();
    assert!(engine.apply_command(0, &cast_spell(slot, vec![])).is_err());
    assert_eq!(serde_json::to_value(&engine.state).unwrap(), before);
    engine.state.players[0].mana_pool.colorless = 0;
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 4,
            ..Default::default()
        },
    );
    engine.apply_command(0, &cast_spell(slot, vec![])).unwrap();
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.players[0].mana_pool.colorless, 0);
    let projected = engine.initial_response_batch();
    let clock_view = projected
        .events
        .iter()
        .find_map(|event| match &event.ev {
            Some(Ev::ZoneView(view)) => view
                .per_player
                .iter()
                .flat_map(|player| &player.battlefield_objects)
                .find(|object| object.object_id == source),
            _ => None,
        })
        .unwrap();
    assert!(clock_view.activated_abilities.is_empty());
    let before = serde_json::to_value(&engine.state).unwrap();
    assert!(engine
        .apply_command(0, &activate_ability_for(&engine, source, 0, vec![]))
        .is_err());
    assert_eq!(serde_json::to_value(&engine.state).unwrap(), before);
    for oid in [source, ring, bear, other_ring] {
        engine.state.objects.get_mut(&oid).unwrap().tapped = true;
    }
    engine.state.turn_step = TurnStep::EndStep;
    engine.state.active_player_idx = 0;
    engine.state.priority_idx = 0;
    engine.state.passes_since_stack_change = 0;
    let _ = engine.initial_response_batch();
    let batch = engine.apply_command(0, &primitive_yield()).unwrap();
    assert_eq!(engine.state.active_player_id(), 1);
    assert!(!engine.state.objects[&source].tapped);
    assert!(!engine.state.objects[&ring].tapped);
    assert!(engine.state.objects[&bear].tapped);
    assert!(!engine.state.objects[&other_ring].tapped);
    let mut untapped = batch
        .events
        .iter()
        .filter_map(|event| match &event.ev {
            Some(Ev::PermanentsUntapped(edge)) => Some(edge.object_ids.clone()),
            _ => None,
        })
        .flatten()
        .collect::<Vec<_>>();
    untapped.sort_unstable();
    let mut expected = vec![source, ring, other_ring];
    expected.sort_unstable();
    assert_eq!(untapped, expected);
}

fn clock_engine(seed: u64, players: &[i32]) -> GameEngine {
    let deck = deck_with(
        "forest",
        &[
            "sol_ring",
            "ornithopter",
            "grizzly_bears",
            "sculpting_steel",
        ],
    );
    let mut engine =
        GameEngine::new(seed, players, 20, Some(vec![deck; players.len()]), true).unwrap();
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn enter_clock(engine: &mut GameEngine) -> u32 {
    inject_card_into_hand(engine, 0, CLOCK);
    move_ready_to_battlefield(engine, 0, CLOCK)
}

fn next_turn_edges(engine: &mut GameEngine) -> Vec<u32> {
    engine.state.turn_step = TurnStep::EndStep;
    engine.state.active_player_idx = 0;
    engine.state.priority_idx = 0;
    engine.state.passes_since_stack_change = 0;
    let _ = engine.initial_response_batch();
    let actor = engine.state.active_player_id();
    let mut batch = engine.apply_command(actor, &primitive_yield()).unwrap();
    if let Some(player) = engine.state.cleanup_discard_player {
        let index = engine.state.player_idx(player).unwrap();
        // This fixture has no hand-size modifiers; finish its ordinary seven-card cleanup.
        let excess = engine.state.players[index].hand.len() - 7;
        let cleanup = engine
            .apply_command(player, &discard_cleanup_batch((0..excess as u32).collect()))
            .unwrap();
        batch.events.extend(cleanup.events);
    }
    assert_eq!(
        engine.state.active_player_idx, 1,
        "fixture reaches the next turn"
    );
    assert!(engine.state.blocking_choice().is_none());
    let mut in_untap = false;
    for event in &batch.events {
        match &event.ev {
            Some(Ev::PhaseChanged(phase)) => {
                in_untap = phase.phase_id == tricerules_proto::ruled::v1::PhaseId::Untap as i32
            }
            Some(Ev::PriorityChanged(_)) => assert!(!in_untap, "no priority in the untap step"),
            _ => {}
        }
    }
    let mut edges = batch
        .events
        .iter()
        .filter_map(|event| match &event.ev {
            Some(Ev::PermanentsUntapped(edge)) => Some(edge.object_ids.clone()),
            _ => None,
        })
        .flatten()
        .collect::<Vec<_>>();
    edges.sort_unstable();
    edges
}

fn tap(engine: &mut GameEngine, objects: &[u32]) {
    for oid in objects {
        engine.state.objects.get_mut(oid).unwrap().tapped = true;
    }
}

fn suppress(engine: &mut GameEngine, source: u32, duration: EffectDuration) {
    engine.state.continuous_effects.push(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: None,
        affected: AffectedScope::Single(source),
        kind: ContinuousEffectKind::Layer6RemoveAllAbilities,
        condition: None,
        duration,
        timestamp: engine.state.command_index,
    });
}

fn move_clock_out(engine: &mut GameEngine) {
    engine.enable_dev_commands();
    engine
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::DevCommand(DevCommand {
                    target_player_id: 0,
                    dev: Some(dev_command::Dev::MoveCard(DevMoveCard {
                        card_name: "Unwinding Clock".into(),
                        zone: DevZone::Graveyard as i32,
                        ready: false,
                    })),
                })),
            },
        )
        .unwrap();
}

#[test]
fn clock_actual_card_scopes_artifact_creatures_noncreatures_and_nonconsecutive_players() {
    let mut engine = clock_engine(502_321, &[10, 20, 30]);
    let clock = enter_clock(&mut engine);
    let ring = move_ready_to_battlefield(&mut engine, 0, "sol_ring");
    let ornithopter = move_ready_to_battlefield(&mut engine, 0, "ornithopter");
    let bear = move_ready_to_battlefield(&mut engine, 0, "grizzly_bears");
    let foreign_ring = move_ready_to_battlefield(&mut engine, 2, "sol_ring");
    tap(&mut engine, &[clock, ring, ornithopter, bear, foreign_ring]);
    let mut expected = vec![clock, ring, ornithopter];
    expected.sort_unstable();
    assert_eq!(next_turn_edges(&mut engine), expected);
    assert_eq!(engine.state.active_player_id(), 20);
    assert!(engine.state.objects[&bear].tapped);
    assert!(engine.state.objects[&foreign_ring].tapped);
}

#[test]
fn clock_suppression_and_turn_start_restoration_follow_the_current_source() {
    for expires in [false, true] {
        let mut engine = clock_engine(502_322 + u64::from(expires), &[0, 1]);
        let clock = enter_clock(&mut engine);
        let ring = move_ready_to_battlefield(&mut engine, 0, "sol_ring");
        tap(&mut engine, &[clock, ring]);
        suppress(
            &mut engine,
            clock,
            if expires {
                EffectDuration::UntilTurnStart(1)
            } else {
                EffectDuration::Indefinite
            },
        );
        let mut expected = if expires { vec![clock, ring] } else { vec![] };
        expected.sort_unstable();
        assert_eq!(next_turn_edges(&mut engine), expected);
        assert_eq!(engine.state.objects[&ring].tapped, !expires);
    }
}

#[test]
fn clock_departure_and_reentry_use_the_same_physical_card_with_a_new_generation() {
    let mut engine = clock_engine(502_324, &[0, 1]);
    let clock = enter_clock(&mut engine);
    let ring = move_ready_to_battlefield(&mut engine, 0, "sol_ring");
    let generation = engine.state.zone_change_generation[&clock];
    move_clock_out(&mut engine);
    assert_eq!(engine.state.objects[&clock].zone, Zone::Graveyard);
    tap(&mut engine, &[ring]);
    assert!(next_turn_edges(&mut engine).is_empty());
    let returned = move_ready_to_battlefield(&mut engine, 0, CLOCK);
    assert_eq!(returned, clock);
    assert_eq!(engine.state.zone_change_generation[&clock], generation + 2);
    tap(&mut engine, &[clock, ring]);
    let mut expected = vec![clock, ring];
    expected.sort_unstable();
    assert_eq!(next_turn_edges(&mut engine), expected);
}

#[test]
fn sculpting_steel_copy_of_clock_survives_donor_departure_and_owns_its_hook() {
    let mut engine = clock_engine(502_325, &[0, 1]);
    let clock = enter_clock(&mut engine);
    let ring = move_ready_to_battlefield(&mut engine, 0, "sol_ring");
    let copy = inject_card_into_hand(&mut engine, 0, "sculpting_steel");
    let slot = engine.state.players[0]
        .hand
        .iter()
        .position(|oid| *oid == copy)
        .unwrap();
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 3,
            ..Default::default()
        },
    );
    engine.apply_command(0, &cast_spell(slot, vec![])).unwrap();
    pass_both_players(&mut engine);
    engine
        .apply_command(0, &submit_resolution_choice(vec![clock]))
        .unwrap();
    assert_eq!(
        engine.characteristics(copy).unwrap().names,
        ["Unwinding Clock"]
    );
    assert_eq!(engine.state.objects[&copy].card_id, "sculpting_steel");
    move_clock_out(&mut engine);
    tap(&mut engine, &[copy, ring]);
    let mut expected = vec![copy, ring];
    expected.sort_unstable();
    assert_eq!(next_turn_edges(&mut engine), expected);
    assert_eq!(engine.state.objects[&clock].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&copy].card_id, "sculpting_steel");
    suppress(&mut engine, copy, EffectDuration::Indefinite);
    tap(&mut engine, &[copy, ring]);
    assert!(next_turn_edges(&mut engine).is_empty());
}

#[test]
fn clock_logged_real_mana_activation_and_turn_boundary_replay_identically() {
    use tricerules_proto::ruled::v1::DevPutCardInZone;
    fn fresh() -> GameEngine {
        let deck = deck_with("forest", &[]);
        let mut engine = GameEngine::new(502_326, &[0, 1], 20, Some(vec![deck; 2]), true).unwrap();
        engine.enable_dev_commands();
        engine
    }
    fn record(
        engine: &mut GameEngine,
        log: &mut Vec<(i32, RuledCommand, RuledEventBatch)>,
        actor: i32,
        command: RuledCommand,
    ) {
        let batch = engine.apply_command(actor, &command).unwrap();
        log.push((actor, command, batch));
    }
    let mut engine = fresh();
    let mut log = Vec::new();
    for actor in [0, 1, 0, 1] {
        record(&mut engine, &mut log, actor, pass());
    }
    assert_eq!(engine.state.turn_step, TurnStep::Main1);
    for name in ["Unwinding Clock", "Sol Ring"] {
        record(
            &mut engine,
            &mut log,
            0,
            RuledCommand {
                cmd: Some(Cmd::DevCommand(DevCommand {
                    target_player_id: 0,
                    dev: Some(dev_command::Dev::PutCardInZone(DevPutCardInZone {
                        card_name: name.into(),
                        zone: DevZone::Battlefield as i32,
                        ready: true,
                    })),
                })),
            },
        );
    }
    let clock = engine.state.players[0].battlefield[0];
    let ring = engine.state.players[0].battlefield[1];
    let generation = engine.state.zone_change_generation[&ring];
    let activation = activate_ability_for(&engine, ring, 0, vec![]);
    record(&mut engine, &mut log, 0, activation);
    assert!(engine.state.objects[&ring].tapped);
    assert_eq!(engine.state.players[0].mana_pool.colorless, 2);
    for _ in 0..12 {
        if engine.state.active_player_id() != 0 {
            break;
        }
        let command = if engine.state.cleanup_discard_player == Some(0) {
            discard_cleanup_batch((0..(engine.state.players[0].hand.len() - 7) as u32).collect())
        } else {
            primitive_yield()
        };
        record(&mut engine, &mut log, 0, command);
    }
    assert_eq!(engine.state.active_player_id(), 1);
    assert!(!engine.state.objects[&ring].tapped);
    assert!(!engine.state.objects[&clock].tapped);
    assert_eq!(engine.state.zone_change_generation[&ring], generation);
    let edges = log
        .iter()
        .flat_map(|(_, _, batch)| &batch.events)
        .filter_map(|event| match &event.ev {
            Some(Ev::PermanentsUntapped(edge)) => Some(edge.object_ids.clone()),
            _ => None,
        })
        .flatten()
        .collect::<Vec<_>>();
    assert_eq!(
        edges,
        [ring],
        "only the actually tapped artifact emits an edge"
    );
    let mut replay = fresh();
    for (actor, command, expected) in &log {
        assert_eq!(&replay.apply_command(*actor, command).unwrap(), expected);
    }
    assert_eq!(
        replay.initial_response_batch(),
        engine.initial_response_batch()
    );
    assert_eq!(
        serde_json::to_value(&replay.state).unwrap(),
        serde_json::to_value(&engine.state).unwrap()
    );
}
