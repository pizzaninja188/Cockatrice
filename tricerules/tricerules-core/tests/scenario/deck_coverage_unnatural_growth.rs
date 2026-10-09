use super::helpers::*;
use prost::Message;
use tricerules_cards::{ContinuousEffectKind, ControllerReference, CounterKind, EffectDuration};
use tricerules_core::{AffectedScope, ContinuousEffect, EngineError, TurnStep, Zone};
use tricerules_proto::ruled::v1 as rv1;

fn setup(seed: u64) -> GameEngine {
    let deck = deck_with("forest", &["unnatural_growth", "bottle_gnomes"]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[4, 9, 27],
        20,
        Some(vec![deck.clone(), deck.clone(), deck]),
        true,
    )
    .expect("complete Unnatural Growth must be admitted");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn resolve_one(engine: &mut GameEngine) {
    answer_trigger_order_in_engine_order(engine);
    let before = engine.state.stack.len();
    assert!(before > 0);
    for _ in 0..8 {
        let actor = engine.state.priority_player_id();
        engine.apply_command(actor, &pass()).unwrap();
        if engine.state.stack.len() < before {
            return;
        }
        assert!(engine.state.blocking_choice().is_none());
    }
    panic!("one stack item did not finish within the bound");
}

fn add_effect(engine: &mut GameEngine, oid: u32, kind: ContinuousEffectKind) {
    engine.state.continuous_effects.push(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: None,
        affected: AffectedScope::Single(oid),
        kind,
        condition: None,
        duration: EffectDuration::UntilEndOfTurn,
        timestamp: engine.state.command_index,
    });
}

fn begin_combat(engine: &mut GameEngine) {
    let actor = engine.state.active_player_id();
    engine.apply_command(actor, &primitive_yield()).unwrap();
    assert_eq!(engine.state.turn_step, TurnStep::BeginCombat);
    answer_trigger_order_in_engine_order(engine);
}

fn cast_instant(engine: &mut GameEngine, card: &str, target: u32) {
    let oid = inject_card_into_hand(engine, 0, card);
    let actor = engine.state.priority_player_id();
    assert_eq!(actor, 4);
    give_mana(
        engine,
        4,
        ManaGift {
            c: 3,
            w: 1,
            u: 1,
            g: 1,
            ..Default::default()
        },
    );
    let slot = engine.state.players[0]
        .hand
        .iter()
        .position(|id| *id == oid)
        .unwrap();
    engine
        .apply_command(actor, &cast_spell(slot, target_object(target)))
        .unwrap();
    resolve_one(engine);
}

fn pt(engine: &GameEngine, oid: u32) -> (u32, u32) {
    let c = engine.characteristics(oid).unwrap();
    (c.power.unwrap(), c.toughness.unwrap())
}

fn paid_growth(engine: &mut GameEngine) -> u32 {
    let oid = inject_card_into_hand(engine, 0, "unnatural_growth");
    give_mana(
        engine,
        4,
        ManaGift {
            c: 1,
            g: 4,
            ..Default::default()
        },
    );
    let slot = engine.state.players[0]
        .hand
        .iter()
        .position(|id| *id == oid)
        .unwrap();
    engine.apply_command(4, &cast_spell(slot, vec![])).unwrap();
    assert_eq!(engine.state.players[0].mana_pool.colorless, 0);
    assert_eq!(engine.state.players[0].mana_pool.green, 0);
    resolve_one(engine);
    assert_eq!(engine.state.objects[&oid].zone, Zone::Battlefield);
    oid
}

#[test]
fn growth_paid_card_doubles_unequal_creatures_on_every_players_combat() {
    for active in 0..3 {
        let mut engine = setup(104_900 + active as u64);
        paid_growth(&mut engine);
        let own = inject_creature_with_stats(&mut engine, 0, "bottle_gnomes", 1, 3);
        let opponent = inject_creature_with_stats(&mut engine, 1, "bottle_gnomes", 1, 3);
        let other = inject_creature_with_stats(&mut engine, 2, "bottle_gnomes", 1, 3);
        let land = inject_permanent_on_battlefield(&mut engine, 0, "forest");
        engine.state.turn_step = TurnStep::Main1;
        engine.state.active_player_idx = active;
        engine.state.priority_idx = active;
        let before = engine.diagnostic_snapshot().unwrap();
        assert!(engine
            .apply_command(engine.state.players[(active + 1) % 3].id, &pass())
            .is_err());
        assert_eq!(engine.diagnostic_snapshot().unwrap(), before);
        engine
            .apply_command(engine.state.players[active].id, &primitive_yield())
            .unwrap();
        assert_eq!(engine.state.turn_step, TurnStep::BeginCombat);
        assert_eq!(engine.state.stack.len(), 1);
        resolve_one(&mut engine);
        let c = engine.characteristics(own).unwrap();
        assert_eq!((c.power, c.toughness), (Some(2), Some(6)));
        for oid in [opponent, other] {
            let c = engine.characteristics(oid).unwrap();
            assert_eq!((c.power, c.toughness), (Some(1), Some(3)));
        }
        assert!(!engine.characteristics(land).unwrap().is_creature());
    }
}

#[test]
fn growth_two_triggers_snapshot_membership_at_each_resolution() {
    let mut engine = setup(104_903);
    paid_growth(&mut engine);
    paid_growth(&mut engine);
    let first = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    begin_combat(&mut engine);
    assert_eq!(engine.state.stack.len(), 2);
    resolve_one(&mut engine);
    assert_eq!(pt(&engine, first), (4, 4));
    // Direct physical fixture insertion represents an entrant between independent resolutions.
    let between = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    resolve_one(&mut engine);
    assert_eq!(pt(&engine, first), (8, 8));
    assert_eq!(pt(&engine, between), (4, 4));
    let late = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    assert_eq!(pt(&engine, late), (2, 2));
}

#[test]
fn growth_uses_saved_trigger_controller_current_control_and_survives_source_departure() {
    let mut engine = setup(104_904);
    let growth = paid_growth(&mut engine);
    let lost = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let gained = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    begin_combat(&mut engine);
    // The trigger belongs to P4 even if its source and creatures change control afterwards.
    for (oid, controller) in [(growth, 9), (lost, 9), (gained, 4)] {
        add_effect(
            &mut engine,
            oid,
            ContinuousEffectKind::Layer2Control {
                controller: ControllerReference::Fixed(controller),
            },
        );
    }
    cast_instant(&mut engine, "disenchant", growth);
    assert_eq!(engine.state.objects[&growth].zone, Zone::Graveyard);
    assert_eq!(engine.state.stack.len(), 1);
    resolve_one(&mut engine);
    assert_eq!(pt(&engine, lost), (2, 2));
    assert_eq!(pt(&engine, gained), (4, 4));
}

#[test]
fn growth_composes_with_setters_counters_later_pumps_and_cleanup() {
    let mut engine = setup(104_905);
    paid_growth(&mut engine);
    inject_permanent_on_battlefield(&mut engine, 0, "reliquary_tower");
    let own = inject_creature_with_stats(&mut engine, 0, "bottle_gnomes", 1, 3);
    add_effect(
        &mut engine,
        own,
        ContinuousEffectKind::Layer7bSetPt {
            power: 3,
            toughness: 5,
        },
    );
    add_effect(
        &mut engine,
        own,
        ContinuousEffectKind::PtModify {
            delta_power: 2,
            delta_toughness: 1,
        },
    );
    engine
        .state
        .objects
        .get_mut(&own)
        .unwrap()
        .counters
        .insert(CounterKind::PlusOnePlusOne, 1);
    assert_eq!(pt(&engine, own), (6, 7));
    begin_combat(&mut engine);
    resolve_one(&mut engine);
    assert_eq!(pt(&engine, own), (12, 14));
    cast_instant(&mut engine, "giant_growth", own);
    assert_eq!(pt(&engine, own), (15, 17));
    let old_turn = engine.state.turn_instance;
    for _ in 0..12 {
        if engine.state.turn_instance != old_turn {
            break;
        }
        let active = engine.state.active_player_id();
        engine.apply_command(active, &primitive_yield()).unwrap();
    }
    assert_eq!(engine.state.turn_instance, old_turn + 1);
    // Setter, pre-pump, doubling and Giant Growth all expired; the physical counter remains.
    assert_eq!(pt(&engine, own), (2, 4));
}

#[test]
fn growth_leaving_and_returning_same_physical_card_loses_old_incarnation_modifier() {
    let mut engine = setup(104_906);
    paid_growth(&mut engine);
    let own = inject_creature_with_stats(&mut engine, 0, "bottle_gnomes", 1, 3);
    begin_combat(&mut engine);
    resolve_one(&mut engine);
    assert_eq!(pt(&engine, own), (2, 6));
    let generation = engine
        .state
        .zone_change_generation
        .get(&own)
        .copied()
        .unwrap_or(0);
    cast_instant(&mut engine, "unsummon", own);
    assert_eq!(engine.state.objects[&own].zone, Zone::Hand);
    assert!(engine.state.zone_change_generation[&own] > generation);
    engine.state.turn_step = TurnStep::Main2;
    engine.state.priority_idx = 0;
    give_mana(
        &mut engine,
        4,
        ManaGift {
            c: 3,
            ..Default::default()
        },
    );
    let slot = engine.state.players[0]
        .hand
        .iter()
        .position(|id| *id == own)
        .unwrap();
    engine.apply_command(4, &cast_spell(slot, vec![])).unwrap();
    resolve_one(&mut engine);
    assert_eq!(engine.state.objects[&own].zone, Zone::Battlefield);
    assert_eq!(pt(&engine, own), (1, 3));
}

#[test]
fn growth_out_of_range_resolution_and_canonical_commands_restore_whole_command() {
    for canonical in [false, true] {
        let mut engine = setup(104_907);
        paid_growth(&mut engine);
        let valid = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
        let oversized =
            inject_creature_with_stats(&mut engine, 0, "grizzly_bears", 1_073_741_824, 3);
        begin_combat(&mut engine);
        for _ in 0..2 {
            let actor = engine.state.priority_player_id();
            engine.apply_command(actor, &pass()).unwrap();
        }
        let actor = engine.state.priority_player_id();
        let command = if canonical {
            RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::CanonicalGameplay(
                    rv1::CanonicalGameplayCommand {
                        command: pass().encode_to_vec(),
                        auto_pass_policies: [4, 9, 27]
                            .into_iter()
                            .map(|player_id| rv1::AutoPassPolicy {
                                player_id,
                                stop_on_own_turn: vec![rv1::PhaseId::Main2 as i32],
                                stop_on_opponent_turn: vec![rv1::PhaseId::Main2 as i32],
                            })
                            .collect(),
                    },
                )),
            }
        } else {
            pass()
        };
        let before = engine.diagnostic_snapshot().unwrap();
        assert!(matches!(
            engine.apply_command(actor, &command),
            Err(EngineError::PowerToughnessNumericRange(_))
        ));
        assert_eq!(engine.diagnostic_snapshot().unwrap(), before);
        assert_eq!(pt(&engine, valid), (2, 2));
        assert_eq!(pt(&engine, oversized), (1_073_741_824, 3));
        assert_eq!(engine.state.stack.len(), 1);
    }
}

#[test]
fn growth_replays_serialized_accepted_commands_from_identical_fixture() {
    let fixture = || {
        let mut engine = setup(104_908);
        let creature = inject_creature_with_stats(&mut engine, 0, "bottle_gnomes", 1, 3);
        let growth = inject_card_into_hand(&mut engine, 0, "unnatural_growth");
        give_mana(
            &mut engine,
            4,
            ManaGift {
                c: 1,
                g: 4,
                ..Default::default()
            },
        );
        let slot = engine.state.players[0]
            .hand
            .iter()
            .position(|id| *id == growth)
            .unwrap();
        (engine, creature, slot)
    };
    let (mut live, creature, slot) = fixture();
    let (mut replay, replay_creature, replay_slot) = fixture();
    assert_eq!((creature, slot), (replay_creature, replay_slot));
    let mut log = Vec::new();
    for command in [
        cast_spell(slot, vec![]),
        pass(),
        pass(),
        pass(),
        primitive_yield(),
        pass(),
        pass(),
        pass(),
    ] {
        let actor = live.state.priority_player_id();
        live.apply_command(actor, &command).unwrap();
        log.push((actor, command.encode_to_vec()));
    }
    assert_eq!(pt(&live, creature), (2, 6));
    for (actor, bytes) in log {
        replay
            .apply_command(actor, &RuledCommand::decode(bytes.as_slice()).unwrap())
            .unwrap();
    }
    assert_eq!(
        live.diagnostic_snapshot().unwrap(),
        replay.diagnostic_snapshot().unwrap()
    );
}
