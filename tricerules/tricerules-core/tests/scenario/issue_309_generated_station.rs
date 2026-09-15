//! Issue #309 — the reviewed two-card Station-8 Flying Spacecraft cohort.
//!
//! Oracle and the current Comprehensive Rules were checked 2026-09-15.  The
//! Station payment is an activated sorcery-speed cost (CR 602.1/602.2 and
//! 702.184a), its power is read when the effect resolves, and the generated
//! `{8+}` static ability is the printed Station threshold (CR 721.2a-b).
//! The entry effects exercise mandatory draw-then-discard ordering (CR
//! 121.1-2 and 603.2), any-target revalidation (CR 115.1d and 608.2b), and
//! zone-change generations (CR 400.7).

use super::helpers::*;
use prost::Message;
use tricerules_cards::primitives::{ContinuousEffectKind, EffectDuration};
use tricerules_cards::{CounterKind, Keyword};
use tricerules_core::{AffectedScope, ContinuousEffect, TurnStep, Zone};
use tricerules_proto::ruled::v1::{
    cost_selection::Selection, CanonicalGameplayCommand, CostObjectRef, CostObjectRefs,
    CostSelection, TargetRef, TargetRefKind,
};

fn generation(engine: &GameEngine, object_id: u32) -> u64 {
    engine
        .state
        .zone_change_generation
        .get(&object_id)
        .copied()
        .unwrap_or(0)
}

fn tap_selection(engine: &GameEngine, object_id: u32) -> CostSelection {
    CostSelection {
        cost_index: 0,
        selection: Some(Selection::BattlefieldObjects(CostObjectRefs {
            objects: vec![CostObjectRef {
                object_id,
                zone_change_generation: generation(engine, object_id),
            }],
        })),
    }
}

fn activate_station(engine: &GameEngine, source: u32, crew: u32) -> RuledCommand {
    let mut command =
        activate_ability_with_costs(source, 0, Vec::new(), vec![tap_selection(engine, crew)]);
    let Some(Cmd::ActivateAbility(activation)) = command.cmd.as_mut() else {
        unreachable!()
    };
    activation.expected_zone_change_generation = generation(engine, source);
    command
}

fn station_engine(seed: u64, card_id: &str) -> (GameEngine, u32, u32) {
    let mut engine = GameEngine::new(
        seed,
        &[0, 1],
        20,
        Some(vec![
            deck_with("island", &[card_id]),
            deck_with("forest", &["grizzly_bears"]),
        ]),
        true,
    )
    .expect("generated Station card must be registered");
    advance_to_main1_from_game_start(&mut engine);
    // Enter through the engine so the printed static ability is materialized, then
    // settle the card's ETB so Station-only cases begin from a normal priority state.
    let source = move_ready_to_battlefield(&mut engine, 0, card_id);
    settle_entry_trigger(&mut engine, card_id);
    let crew = inject_creature_on_battlefield(&mut engine, 0, "storm_crow");
    (engine, source, crew)
}

fn trigger_target(object_id: u32) -> TargetRef {
    TargetRef {
        object_id,
        group_index: 0,
        kind: TargetRefKind::Permanent as i32,
        ..Default::default()
    }
}

fn choose_trigger_target(targets: Vec<TargetRef>) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ChooseTriggerTarget(
            tricerules_proto::ruled::v1::ChooseTriggerTarget {
                targets,
                ..Default::default()
            },
        )),
    }
}

fn resolve_top_stack(engine: &mut GameEngine) -> RuledEventBatch {
    let first = engine.state.priority_player_id();
    engine.apply_command(first, &pass()).expect("first pass");
    let second = engine.state.priority_player_id();
    engine
        .apply_command(second, &pass())
        .expect("second pass resolves stack item")
}

fn settle_entry_trigger(engine: &mut GameEngine, card_id: &str) {
    if card_id == "uthros_scanship" {
        inject_card_into_hand(engine, 0, "grizzly_bears");
        let resolved = resolve_top_stack(engine);
        let choice = find_resolution_choice(&resolved).expect("Uthros discard choice");
        let discard = choice
            .candidate_object_ids
            .first()
            .copied()
            .expect("Uthros has a discard candidate");
        engine
            .apply_command(0, &submit_resolution_choice(vec![discard]))
            .expect("settle Uthros entry discard");
    } else {
        engine
            .apply_command(0, &choose_trigger_target(target_player(1)))
            .expect("settle Debris entry target");
        resolve_entire_stack_two_player(engine);
    }
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.pending_triggers.is_empty());
    assert!(engine.state.staged_trigger_groups.is_empty());
}

fn move_to_hand(engine: &mut GameEngine, player: usize, object_id: u32) {
    engine.state.players[player]
        .battlefield
        .retain(|candidate| *candidate != object_id);
    engine.state.players[player].hand.push(object_id);
    engine.state.objects.get_mut(&object_id).unwrap().zone = Zone::Hand;
    *engine
        .state
        .zone_change_generation
        .entry(object_id)
        .or_default() += 1;
}

fn move_named_card_to_hand_via_engine(engine: &mut GameEngine, player: usize, card_id: &str) {
    use tricerules_proto::ruled::v1::{dev_command, DevCommand, DevMoveCard, DevZone};

    let player_id = engine.state.players[player].id;
    let card_name = tricerules_cards::CardRegistry::global()
        .get(card_id)
        .expect("known card for engine move")
        .name
        .clone();
    engine.enable_dev_commands();
    engine
        .apply_command(
            player_id,
            &RuledCommand {
                cmd: Some(Cmd::DevCommand(DevCommand {
                    target_player_id: player_id,
                    dev: Some(dev_command::Dev::MoveCard(DevMoveCard {
                        card_name,
                        zone: DevZone::Hand as i32,
                        ready: false,
                    })),
                })),
            },
        )
        .expect("move the paid permanent through the engine");
}

fn seat_on_top(engine: &mut GameEngine, player: usize, card_ids: &[&str]) -> Vec<u32> {
    let object_ids = card_ids
        .iter()
        .map(|card_id| inject_library_card(engine, player, card_id))
        .collect::<Vec<_>>();
    engine.state.players[player]
        .library
        .retain(|object_id| !object_ids.contains(object_id));
    for object_id in object_ids.iter().rev() {
        engine.state.players[player].library.push_front(*object_id);
    }
    object_ids
}

fn append_third_player(engine: &mut GameEngine) {
    // The constructor intentionally remains two-seat, but the state and target
    // filters are player-set-generic once a scenario supplies a third seat.
    let mut third = engine.state.players[1].clone();
    third.id = 2;
    third.battlefield.clear();
    third.hand.clear();
    third.library.clear();
    third.graveyard.clear();
    third.exile.clear();
    engine.state.players.push(third);
}

fn resolve_entire_stack_all_players(engine: &mut GameEngine) {
    while !engine.state.stack.is_empty() {
        answer_trigger_order_in_engine_order(engine);
        let player_count = engine.state.players.len();
        for _ in 0..player_count {
            if engine.state.stack.is_empty() {
                break;
            }
            let player = engine.state.priority_player_id();
            engine
                .apply_command(player, &pass())
                .expect("player passes priority");
        }
    }
}

fn canonical_command(inner: RuledCommand) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::CanonicalGameplay(CanonicalGameplayCommand {
            command: inner.encode_to_vec(),
            auto_pass_policies: Vec::new(),
        })),
    }
}

#[test]
fn issue_309_station_publishes_only_controlled_untapped_creatures_and_reads_power_on_resolution() {
    let (mut engine, station, crew) = station_engine(309_001, "uthros_scanship");
    let opposing = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let timestamp = engine.state.command_index;
    engine.state.objects.get_mut(&crew).unwrap().add_counters(
        CounterKind::PlusOnePlusOne,
        1,
        timestamp,
    );

    let key = u64::from(station) << 32;
    let choices =
        &engine.initial_response_batch().legal_by_player[&0].cost_choices_by_ability[&key];
    assert!(choices.non_mana_costs_payable);
    assert_eq!(choices.choices.len(), 1);
    assert_eq!(choices.choices[0].candidate_ids, [crew]);
    assert!(!choices.choices[0].candidate_ids.contains(&station));
    assert!(!choices.choices[0].candidate_ids.contains(&opposing));
    assert_eq!(
        choices.choices[0].candidate_objects[0]
            .object
            .as_ref()
            .expect("generation-bound payment")
            .zone_change_generation,
        generation(&engine, crew)
    );

    let payment_generation = generation(&engine, crew);
    assert_eq!(engine.effective_power(crew), Some(3));
    engine
        .apply_command(0, &activate_station(&engine, station, crew))
        .expect("controlled untapped creature pays Station");
    assert!(engine.state.objects[&crew].tapped);
    move_named_card_to_hand_via_engine(&mut engine, 0, "storm_crow");
    let departed_generation = generation(&engine, crew);
    assert!(departed_generation > payment_generation);
    let reentered = move_ready_to_battlefield(&mut engine, 0, "storm_crow");
    assert_eq!(
        reentered, crew,
        "the paid creature re-enters as the same physical object"
    );
    assert!(generation(&engine, crew) > departed_generation);
    assert_eq!(engine.effective_power(crew), Some(1));
    assert_eq!(
        engine.state.objects[&station].counter_count(CounterKind::Charge),
        0
    );
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        engine.state.objects[&station].counter_count(CounterKind::Charge),
        3,
        "Station uses the paid generation's recorded 3 power, not the re-entered creature's current 1 power"
    );
}

#[test]
fn issue_309_station_uses_current_controller_not_owner_in_multiplayer() {
    let (mut engine, station, crew) = station_engine(309_005, "uthros_scanship");
    append_third_player(&mut engine);
    let foreign_owned_controlled =
        inject_creature_under_foreign_control(&mut engine, 1, 0, "grizzly_bears");
    engine
        .state
        .objects
        .get_mut(&foreign_owned_controlled)
        .unwrap()
        .power = Some(3);
    engine
        .state
        .objects
        .get_mut(&foreign_owned_controlled)
        .unwrap()
        .toughness = Some(3);
    let third_player_creature = inject_creature_on_battlefield(&mut engine, 2, "grizzly_bears");
    let p0_owned_p1_controlled =
        inject_creature_under_foreign_control(&mut engine, 0, 1, "grizzly_bears");

    let key = u64::from(station) << 32;
    let choices =
        &engine.initial_response_batch().legal_by_player[&0].cost_choices_by_ability[&key];
    assert_eq!(
        choices.choices[0].candidate_ids,
        [crew, foreign_owned_controlled],
        "Station follows controller, not owner, across a three-seat board"
    );
    assert!(!choices.choices[0]
        .candidate_ids
        .contains(&third_player_creature));
    assert!(!choices.choices[0]
        .candidate_ids
        .contains(&p0_owned_p1_controlled));

    // A control-changing effect updates the permanent's authoritative controller and the
    // control index.  Owner remains player 0, but the object must no longer be offered as a
    // payment candidate to player 0.
    engine.state.players[0]
        .battlefield
        .retain(|object_id| *object_id != crew);
    engine.state.players[1].battlefield.push(crew);
    engine.state.objects.get_mut(&crew).unwrap().base_controller = 1;
    engine.state.objects.get_mut(&crew).unwrap().controller = 1;
    let controlled_away = &engine.initial_response_batch().legal_by_player[&0]
        .cost_choices_by_ability[&key]
        .choices[0]
        .candidate_ids;
    assert_eq!(*controlled_away, [foreign_owned_controlled]);

    engine
        .apply_command(
            0,
            &activate_station(&engine, station, foreign_owned_controlled),
        )
        .expect("a creature controlled by the activating player can pay Station");
    resolve_entire_stack_all_players(&mut engine);
    assert_eq!(
        engine.state.objects[&station].counter_count(CounterKind::Charge),
        3
    );
}

#[test]
fn issue_309_station_canonical_command_log_replays_identically() {
    fn run() -> (Vec<Vec<u8>>, (u32, bool, u64)) {
        let (mut engine, station, crew) = station_engine(309_006, "uthros_scanship");
        let activation = canonical_command(activate_station(&engine, station, crew));
        let first_pass_player = engine.state.priority_player_id();
        let second_pass_player = if first_pass_player == 0 { 1 } else { 0 };
        let commands = [
            (0, activation),
            (first_pass_player, canonical_command(pass())),
            (second_pass_player, canonical_command(pass())),
        ];
        let mut command_log = Vec::new();
        let mut response_log = Vec::new();
        for (player, command) in commands {
            command_log.push((player, command.encode_to_vec()));
            response_log.push(
                engine
                    .apply_command(player, &command)
                    .expect("replay command accepted")
                    .encode_to_vec(),
            );
        }
        assert!(!command_log.is_empty());
        (
            response_log,
            (
                engine.state.objects[&station].counter_count(CounterKind::Charge),
                engine.state.objects[&crew].tapped,
                engine.state.command_index,
            ),
        )
    }

    assert_eq!(run(), run());
}

#[test]
fn issue_309_station_is_sorcery_speed_and_fails_atomically_without_a_payment_candidate() {
    let (mut engine, station, crew) = station_engine(309_002, "debris_field_crusher");
    let key = u64::from(station) << 32;
    let mana_before = engine.state.players[0].mana_pool;
    engine.state.turn_step = TurnStep::Upkeep;
    engine
        .apply_command(0, &activate_station(&engine, station, crew))
        .expect_err("Station is sorcery-speed");
    assert_eq!(engine.state.players[0].mana_pool, mana_before);
    assert!(!engine.state.objects[&crew].tapped);
    assert!(engine.state.stack.is_empty());

    engine.state.turn_step = TurnStep::Main1;
    engine.state.objects.get_mut(&crew).unwrap().tapped = true;
    let legal_batch = engine.initial_response_batch();
    let legal = legal_batch.legal_by_player[&0]
        .cost_choices_by_ability
        .get(&key)
        .expect("Station ability remains published");
    assert!(!legal.non_mana_costs_payable);
    assert!(legal.choices[0].candidate_ids.is_empty());
    engine
        .apply_command(0, &activate_station(&engine, station, crew))
        .expect_err("a tapped creature cannot pay Station");
    assert_eq!(
        engine.state.objects[&station].counter_count(CounterKind::Charge),
        0
    );
    assert!(engine.state.stack.is_empty());
}

#[test]
fn issue_309_station_threshold_is_exactly_eight_and_ability_remains_usable_above_it() {
    let (mut engine, station, crew) = station_engine(309_003, "uthros_scanship");
    let object = engine
        .characteristics(station)
        .expect("Station characteristics");
    assert!(object.has_type("Artifact"));
    assert!(!object.has_type("Creature"));
    assert_eq!((object.power, object.toughness), (None, None));
    assert!(!object.has_keyword(Keyword::Flying));

    engine
        .state
        .objects
        .get_mut(&station)
        .unwrap()
        .set_counter(CounterKind::Charge, 7);
    let below = engine.characteristics(station).expect("below threshold");
    assert!(!below.has_type("Creature"));
    assert!(!below.has_keyword(Keyword::Flying));
    assert_eq!((below.power, below.toughness), (None, None));

    engine
        .state
        .objects
        .get_mut(&station)
        .unwrap()
        .set_counter(CounterKind::Charge, 8);
    let at = engine.characteristics(station).expect("at threshold");
    assert!(at.has_type("Artifact") && at.has_type("Creature"));
    assert!(at.has_keyword(Keyword::Flying));
    assert_eq!((at.power, at.toughness), (Some(4), Some(4)));

    engine.state.continuous_effects.push(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: None,
        affected: AffectedScope::Single(station),
        kind: ContinuousEffectKind::Layer7bSetPt {
            power: 7,
            toughness: 6,
        },
        condition: None,
        duration: EffectDuration::UntilEndOfTurn,
        timestamp: engine.state.command_index,
    });
    let independently_set = engine.characteristics(station).expect("independent set");
    assert_eq!(
        (independently_set.power, independently_set.toughness),
        (Some(7), Some(6)),
        "an independent layer-7b set is applied after the Station base set"
    );
    engine.state.continuous_effects.pop();
    engine.state.continuous_effects.push(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: None,
        affected: AffectedScope::Single(station),
        kind: ContinuousEffectKind::PtModify {
            delta_power: 2,
            delta_toughness: 1,
        },
        condition: None,
        duration: EffectDuration::UntilEndOfTurn,
        timestamp: engine.state.command_index,
    });
    let independently_modified = engine.characteristics(station).expect("independent modify");
    assert_eq!(
        (
            independently_modified.power,
            independently_modified.toughness
        ),
        (Some(6), Some(5)),
        "an independent layer-7c modifier applies to the threshold base P/T"
    );
    engine.state.continuous_effects.pop();

    engine
        .apply_command(0, &activate_station(&engine, station, crew))
        .expect("Station remains activatable at and above its threshold");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        engine.state.objects[&station].counter_count(CounterKind::Charge),
        10
    );
}

#[test]
fn issue_309_station_old_generation_cannot_counter_a_reentered_source() {
    let (mut engine, station, crew) = station_engine(309_004, "uthros_scanship");
    engine
        .apply_command(0, &activate_station(&engine, station, crew))
        .expect("activate Station");
    move_to_hand(&mut engine, 0, station);
    let departed_generation = generation(&engine, station);
    let reentered = move_ready_to_battlefield(&mut engine, 0, "uthros_scanship");
    assert_eq!(reentered, station, "the same physical card re-enters");
    assert!(generation(&engine, station) > departed_generation);
    settle_entry_trigger(&mut engine, "uthros_scanship");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        engine.state.objects[&station].counter_count(CounterKind::Charge),
        0,
        "the old Station source generation cannot affect the new incarnation"
    );
}

#[test]
fn issue_309_uthros_enters_with_private_mandatory_draw_then_discard() {
    let mut engine = GameEngine::new(
        309_010,
        &[0, 1],
        20,
        Some(vec![
            deck_with("island", &["uthros_scanship"]),
            forest_only_deck(),
        ]),
        true,
    )
    .expect("Uthros engine");
    advance_to_main1_from_game_start(&mut engine);
    ensure_in_hand(&mut engine, 0, "uthros_scanship");
    let discard = inject_card_into_hand(&mut engine, 0, "grizzly_bears");
    let drawn = seat_on_top(&mut engine, 0, &["storm_crow", "mountain"]);
    let hand_before = engine.state.players[0].hand.clone();
    let library_before = engine.state.players[0].library.len();
    move_ready_to_battlefield(&mut engine, 0, "uthros_scanship");

    let resolved = resolve_top_stack(&mut engine);
    let choice = find_resolution_choice(&resolved).expect("mandatory discard choice");
    assert_eq!(choice.deciding_player_id, 0);
    assert_eq!(choice.choice_kind(), ChoiceKind::HandCards);
    assert_eq!((choice.min, choice.max), (1, 1));
    assert_eq!(engine.state.players[0].library.len(), library_before - 2);
    assert_eq!(engine.state.players[0].hand.len(), hand_before.len() + 1);
    assert!(choice.candidate_object_ids.contains(&discard));
    assert!(choice.candidate_object_ids.contains(&drawn[0]));
    assert!(choice.candidate_object_ids.contains(&drawn[1]));
    let zone_view = resolved
        .events
        .iter()
        .find_map(|event| match event.ev.as_ref() {
            Some(Ev::ZoneView(view)) => Some(view),
            _ => None,
        })
        .expect("Uthros publishes a private hand view");
    let controller_view = zone_view
        .per_player
        .iter()
        .find(|view| view.player_id == 0)
        .expect("controller's zone view");
    let opponent_view = zone_view
        .per_player
        .iter()
        .find(|view| view.player_id == 1)
        .expect("opponent's zone view");
    assert!(choice.candidate_object_ids.iter().all(|object_id| {
        controller_view
            .hand_cards
            .iter()
            .any(|card| card.object_id == *object_id)
    }));
    assert!(choice.candidate_object_ids.iter().all(|object_id| {
        opponent_view
            .hand_cards
            .iter()
            .all(|card| card.object_id != *object_id)
    }));
    assert!(engine
        .apply_command(1, &submit_resolution_choice(vec![discard]))
        .is_err());
    engine
        .apply_command(0, &submit_resolution_choice(vec![discard]))
        .expect("controller chooses the mandatory discard");
    assert_eq!(engine.state.objects[&discard].zone, Zone::Graveyard);
    assert!(engine.state.stack.is_empty());
}

#[test]
fn issue_309_uthros_short_library_draws_as_much_as_possible_before_discard() {
    let mut engine = GameEngine::new(
        309_011,
        &[0, 1],
        20,
        Some(vec![
            deck_with("island", &["uthros_scanship"]),
            forest_only_deck(),
        ]),
        true,
    )
    .expect("short-library Uthros engine");
    advance_to_main1_from_game_start(&mut engine);
    ensure_in_hand(&mut engine, 0, "uthros_scanship");
    let only_card = inject_library_card(&mut engine, 0, "storm_crow");
    engine.state.players[0].library.clear();
    engine.state.players[0].library.push_back(only_card);
    let discard = inject_card_into_hand(&mut engine, 0, "grizzly_bears");
    move_ready_to_battlefield(&mut engine, 0, "uthros_scanship");
    let resolved = resolve_top_stack(&mut engine);
    let choice = find_resolution_choice(&resolved).expect("mandatory discard after one draw");
    assert_eq!(choice.deciding_player_id, 0);
    assert_eq!(choice.choice_kind(), ChoiceKind::HandCards);
    assert_eq!((choice.min, choice.max), (1, 1));
    assert_eq!(engine.state.objects[&only_card].zone, Zone::Hand);
    assert_eq!(engine.state.objects[&discard].zone, Zone::Hand);
    assert!(
        !engine.state.players[0].has_lost,
        "the draw-from-empty loss waits until Uthros's mandatory discard resolves"
    );
    assert!(
        engine.state.winner.is_none(),
        "state-based loss cannot end the game mid-resolution"
    );
    engine
        .apply_command(0, &submit_resolution_choice(vec![discard]))
        .expect("the mandatory discard completes before the deferred deck loss");
    assert_eq!(engine.state.objects[&discard].zone, Zone::Graveyard);
    assert!(engine.state.players[0].has_lost);
    assert_eq!(engine.state.winner, Some(1));
}

#[test]
fn issue_309_debris_etb_hits_any_target_and_revalidates_a_departed_target() {
    let mut engine = GameEngine::new(
        309_020,
        &[0, 1],
        20,
        Some(vec![
            deck_with("mountain", &["debris_field_crusher"]),
            forest_only_deck(),
        ]),
        true,
    )
    .expect("Debris engine");
    advance_to_main1_from_game_start(&mut engine);
    let creature = inject_creature_with_stats(&mut engine, 1, "grizzly_bears", 2, 5);
    move_ready_to_battlefield(&mut engine, 0, "debris_field_crusher");
    engine
        .apply_command(0, &choose_trigger_target(vec![trigger_target(creature)]))
        .expect("choose legal creature any-target");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&creature].damage, 3);
    assert_eq!(engine.state.players[1].life, 20);

    let mut stale = GameEngine::new(
        309_021,
        &[0, 1],
        20,
        Some(vec![
            deck_with("mountain", &["debris_field_crusher"]),
            forest_only_deck(),
        ]),
        true,
    )
    .expect("stale-target Debris engine");
    advance_to_main1_from_game_start(&mut stale);
    let departed = inject_creature_with_stats(&mut stale, 1, "grizzly_bears", 2, 5);
    move_ready_to_battlefield(&mut stale, 0, "debris_field_crusher");
    stale
        .apply_command(0, &choose_trigger_target(vec![trigger_target(departed)]))
        .expect("choose target before it leaves");
    stale.state.players[1]
        .battlefield
        .retain(|candidate| *candidate != departed);
    stale.state.players[1].graveyard.push(departed);
    stale.state.objects.get_mut(&departed).unwrap().zone = Zone::Graveyard;
    *stale
        .state
        .zone_change_generation
        .entry(departed)
        .or_default() += 1;
    resolve_entire_stack_two_player(&mut stale);
    assert_eq!(stale.state.objects[&departed].damage, 0);
}

#[test]
fn issue_309_debris_etb_accepts_a_planeswalker_and_requires_exactly_one_target() {
    let mut engine = GameEngine::new(
        309_025,
        &[0, 1],
        20,
        Some(vec![
            deck_with("mountain", &["debris_field_crusher"]),
            forest_only_deck(),
        ]),
        true,
    )
    .expect("Debris planeswalker engine");
    advance_to_main1_from_game_start(&mut engine);
    let planeswalker = inject_permanent_on_battlefield(&mut engine, 1, "jace_beleren");
    engine
        .state
        .objects
        .get_mut(&planeswalker)
        .unwrap()
        .set_counter(CounterKind::Loyalty, 6);
    let creature = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    move_ready_to_battlefield(&mut engine, 0, "debris_field_crusher");

    assert!(
        engine
            .apply_command(0, &choose_trigger_target(Vec::new()))
            .is_err(),
        "the mandatory any-target group rejects zero targets"
    );
    assert!(
        engine
            .apply_command(
                0,
                &choose_trigger_target(vec![
                    trigger_target(planeswalker),
                    trigger_target(creature),
                ]),
            )
            .is_err(),
        "the exact-one any-target group rejects multiple targets"
    );
    engine
        .apply_command(
            0,
            &choose_trigger_target(vec![trigger_target(planeswalker)]),
        )
        .expect("a planeswalker is a legal any-target");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        engine.state.objects[&planeswalker].counter_count(CounterKind::Loyalty),
        3,
        "Debris damage removes loyalty from a planeswalker target"
    );
    assert_eq!(engine.state.objects[&creature].damage, 0);
}

#[test]
fn issue_309_debris_etb_can_damage_a_player_and_pump_accumulates_until_cleanup() {
    let mut engine = GameEngine::new(
        309_022,
        &[0, 1],
        20,
        Some(vec![
            deck_with("mountain", &["debris_field_crusher"]),
            forest_only_deck(),
        ]),
        true,
    )
    .expect("player-target Debris engine");
    advance_to_main1_from_game_start(&mut engine);
    move_ready_to_battlefield(&mut engine, 0, "debris_field_crusher");
    engine
        .apply_command(0, &choose_trigger_target(target_player(1)))
        .expect("choose opponent as any-target");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.players[1].life, 17);

    let mut pump = GameEngine::new(
        309_023,
        &[0, 1],
        20,
        Some(vec![
            deck_with("mountain", &["debris_field_crusher"]),
            forest_only_deck(),
        ]),
        true,
    )
    .expect("pump Debris engine");
    advance_to_main1_from_game_start(&mut pump);
    let debris = move_ready_to_battlefield(&mut pump, 0, "debris_field_crusher");
    settle_entry_trigger(&mut pump, "debris_field_crusher");
    pump.state
        .objects
        .get_mut(&debris)
        .unwrap()
        .set_counter(CounterKind::Charge, 8);
    assert_eq!(pump.effective_power(debris), Some(1));
    assert!(pump.effective_has_keyword(debris, Keyword::Flying));
    give_mana(
        &mut pump,
        0,
        ManaGift {
            r: 2,
            c: 2,
            ..Default::default()
        },
    );
    apply_ability(&mut pump, 0, debris, 1, Vec::new()).expect("first pump activation");
    assert_eq!(pump.state.players[0].mana_pool.red, 1);
    assert_eq!(pump.state.players[0].mana_pool.colorless, 1);
    assert!(!pump.state.objects[&debris].tapped);
    assert_eq!(pump.effective_power(debris), Some(1));
    resolve_entire_stack_two_player(&mut pump);
    assert_eq!(pump.effective_power(debris), Some(3));
    apply_ability(&mut pump, 0, debris, 1, Vec::new()).expect("second pump activation");
    assert_eq!(pump.state.players[0].mana_pool.red, 0);
    assert_eq!(pump.state.players[0].mana_pool.colorless, 0);
    assert!(!pump.state.objects[&debris].tapped);
    assert_eq!(pump.effective_power(debris), Some(3));
    resolve_entire_stack_two_player(&mut pump);
    assert_eq!(pump.effective_power(debris), Some(5));
    end_active_turn(&mut pump, 0);
    assert_eq!(pump.effective_power(debris), Some(1));
}

#[test]
fn issue_309_debris_pump_is_available_at_normal_timing_outside_sorcery_speed() {
    let mut engine = GameEngine::new(
        309_026,
        &[0, 1],
        20,
        Some(vec![
            deck_with("mountain", &["debris_field_crusher"]),
            forest_only_deck(),
        ]),
        true,
    )
    .expect("Debris timing engine");
    advance_to_main1_from_game_start(&mut engine);
    let debris = relocate_to_battlefield(&mut engine, 0, "debris_field_crusher", false);
    engine
        .state
        .objects
        .get_mut(&debris)
        .unwrap()
        .set_counter(CounterKind::Charge, 8);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            r: 1,
            c: 1,
            ..Default::default()
        },
    );
    engine.state.turn_step = TurnStep::Upkeep;
    apply_ability(&mut engine, 0, debris, 1, Vec::new())
        .expect("Debris's normal-timing pump is not sorcery-speed restricted");
    assert_eq!(engine.state.stack.len(), 1);
    assert!(!engine.state.objects[&debris].tapped);
}

#[test]
fn issue_309_debris_pump_old_generation_does_not_modify_the_reentered_source() {
    let mut engine = GameEngine::new(
        309_024,
        &[0, 1],
        20,
        Some(vec![
            deck_with("mountain", &["debris_field_crusher"]),
            forest_only_deck(),
        ]),
        true,
    )
    .expect("generation-bound pump engine");
    advance_to_main1_from_game_start(&mut engine);
    let debris = relocate_to_battlefield(&mut engine, 0, "debris_field_crusher", false);
    engine
        .state
        .objects
        .get_mut(&debris)
        .unwrap()
        .set_counter(CounterKind::Charge, 8);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            r: 1,
            c: 1,
            ..Default::default()
        },
    );
    apply_ability(&mut engine, 0, debris, 1, Vec::new()).expect("activate pump");
    move_to_hand(&mut engine, 0, debris);
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&debris].zone, Zone::Hand);
}

#[test]
fn issue_309_primitive_draw_loss_finishes_the_effect_tail_before_sweeping() {
    // Risky Research is an ordinary effect list: after its private Surveil choice,
    // the primitive draw is followed by a mandatory life-loss instruction.  With
    // only the two surveilled cards available, both draw attempts deck P0; the
    // trailing life loss must still commit before the deferred deck loss is swept.
    let mut engine = GameEngine::new(
        309_030,
        &[0, 1],
        20,
        Some(vec![
            deck_with("island", &["risky_research"]),
            forest_only_deck(),
        ]),
        true,
    )
    .expect("primitive draw-tail engine");
    advance_to_main1_from_game_start(&mut engine);
    ensure_in_hand(&mut engine, 0, "risky_research");
    let top = inject_library_card(&mut engine, 0, "forest");
    let second = inject_library_card(&mut engine, 0, "island");
    engine.state.players[0]
        .library
        .retain(|object_id| [top, second].contains(object_id));
    engine.state.players[0].library.clear();
    engine.state.players[0].library.push_front(second);
    engine.state.players[0].library.push_front(top);

    let resolving = cast_instant_and_resolve(
        &mut engine,
        0,
        "risky_research",
        ManaGift {
            b: 1,
            c: 2,
            ..Default::default()
        },
    );
    let choice = find_resolution_choice(&resolving).expect("Surveil choice");
    assert_eq!(choice.candidate_object_ids, [top, second]);
    engine
        .apply_command(0, &submit_resolution_choice(vec![top]))
        .expect("finish Surveil and resolve draw plus life-loss tail");

    assert_eq!(engine.state.players[0].life, 18);
    assert!(engine.state.players[0].has_lost);
    assert!(!engine.state.players[0].pending_library_loss);
    assert_eq!(engine.state.winner, Some(1));
    assert!(engine.state.pending_resolution.is_none());
}

#[test]
fn issue_309_brainstorm_can_resume_after_deferred_library_loss() {
    // Brainstorm is tier-3 custom resolution.  Its draw phase may mark a
    // pending loss, but the private put-back choice remains answerable; only
    // completion of that continuation commits the loss and clears the marker.
    let mut engine = GameEngine::new(
        309_031,
        &[0, 1],
        20,
        Some(vec![
            deck_with("island", &["brainstorm"]),
            forest_only_deck(),
        ]),
        true,
    )
    .expect("custom draw-loss engine");
    advance_to_main1_from_game_start(&mut engine);
    ensure_in_hand(&mut engine, 0, "brainstorm");
    let first = inject_library_card(&mut engine, 0, "forest");
    let second = inject_library_card(&mut engine, 0, "island");
    engine.state.players[0].library.clear();
    engine.state.players[0].library.push_front(second);
    engine.state.players[0].library.push_front(first);

    let resolving = cast_instant_and_resolve(
        &mut engine,
        0,
        "brainstorm",
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    let choice = find_resolution_choice(&resolving).expect("Brainstorm put-back choice");
    assert_eq!(choice.deciding_player_id, 0);
    assert_eq!((choice.min, choice.max), (2, 2));
    assert!(engine.state.pending_resolution.is_some());
    assert!(engine.state.players[0].pending_library_loss);
    assert!(!engine.state.players[0].has_lost);

    let chosen = engine.state.players[0]
        .hand
        .iter()
        .take(2)
        .copied()
        .collect();
    engine
        .apply_command(0, &submit_resolution_choice(chosen))
        .expect("resume and finish Brainstorm");
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.players[0].has_lost);
    assert!(!engine.state.players[0].pending_library_loss);
    assert_eq!(engine.state.winner, Some(1));
}

#[test]
fn issue_309_plain_draw_commits_empty_library_loss_before_priority() {
    // Reach Through Mists has no parked continuation.  The empty-library loss
    // therefore commits in the resolution command itself, before a new priority
    // window can be published.
    let mut engine = GameEngine::new(
        309_032,
        &[0, 1],
        20,
        Some(vec![
            deck_with("island", &["reach_through_mists"]),
            forest_only_deck(),
        ]),
        true,
    )
    .expect("plain draw engine");
    advance_to_main1_from_game_start(&mut engine);
    ensure_in_hand(&mut engine, 0, "reach_through_mists");
    engine.state.players[0].library.clear();

    let resolved = cast_instant_and_resolve(
        &mut engine,
        0,
        "reach_through_mists",
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.players[0].has_lost);
    assert!(!engine.state.players[0].pending_library_loss);
    assert_eq!(engine.state.winner, Some(1));
    assert!(resolved.events.iter().any(|event| {
        matches!(
            event.ev.as_ref(),
            Some(Ev::Log(log)) if log.text.contains("Game over")
        )
    }));
}

#[test]
fn issue_309_pending_library_losses_are_deterministic_for_both_players() {
    fn commit(flags: [bool; 2]) -> ([bool; 2], [bool; 2], Option<i32>) {
        let mut engine = GameEngine::new(
            309_033,
            &[0, 1],
            20,
            Some(vec![forest_only_deck(), forest_only_deck()]),
            true,
        )
        .expect("two-player loss engine");
        advance_to_main1_from_game_start(&mut engine);
        for (player, pending) in flags.into_iter().enumerate() {
            engine.state.players[player].pending_library_loss = pending;
        }
        engine
            .apply_command(engine.state.priority_player_id(), &pass())
            .expect("commit pending losses at command boundary");
        (
            [
                engine.state.players[0].has_lost,
                engine.state.players[1].has_lost,
            ],
            [
                engine.state.players[0].pending_library_loss,
                engine.state.players[1].pending_library_loss,
            ],
            engine.state.winner,
        )
    }

    assert_eq!(
        commit([true, false]),
        ([true, false], [false, false], Some(1))
    );
    assert_eq!(
        commit([false, true]),
        ([false, true], [false, false], Some(0))
    );
    assert_eq!(commit([true, true]), ([true, true], [false, false], None));
    assert_eq!(commit([true, true]), commit([true, true]));
}
