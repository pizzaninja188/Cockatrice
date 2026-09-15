//! Issue #310 — the reviewed Station-3/9 keyword Spacecraft cohort.
//!
//! The Station behavior itself is shared with the #309 scenarios.  These tests
//! pin the two new printed variants at their exact thresholds, preserve their
//! keyword and base-characteristic payloads, and exercise Wedgelight Rammer's
//! mandatory Robot entry effect through the engine.
//!
//! Oracle and the current Comprehensive Rules were checked 2026-09-15.  The
//! Station payment is an activated sorcery-speed ability (CR 602.1/602.2 and
//! 702.184a), the threshold is checked continuously (CR 721.2a-b), and the
//! entry trigger is governed by CR 603.1-2.  Physical-object generations and
//! source-bound resolution remain covered by the shared #309 Station tests.

use super::helpers::*;
use tricerules_cards::CounterKind;
use tricerules_cards::Keyword;
use tricerules_core::{GameEngine, TurnStep, Zone};

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
        selection: Some(
            tricerules_proto::ruled::v1::cost_selection::Selection::BattlefieldObjects(
                tricerules_proto::ruled::v1::CostObjectRefs {
                    objects: vec![tricerules_proto::ruled::v1::CostObjectRef {
                        object_id,
                        zone_change_generation: generation(engine, object_id),
                    }],
                },
            ),
        ),
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
    let station = move_ready_to_battlefield(&mut engine, 0, card_id);
    // Galvanizing has no entry trigger.  Wedgelight's mandatory Robot trigger
    // is deliberately settled before the Station assertions begin.
    if card_id == "wedgelight_rammer" {
        resolve_entire_stack_two_player(&mut engine);
    }
    let crew = inject_creature_on_battlefield(&mut engine, 0, "storm_crow");
    (engine, station, crew)
}

fn append_third_player(engine: &mut GameEngine) {
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
        .expect("move named permanent to hand through engine");
}

fn move_owned_card_into_controller_hand(
    engine: &mut GameEngine,
    owner: usize,
    controller: usize,
    card_id: &str,
) {
    let object_id = take_oid_from_library_or_hand(engine, owner, card_id);
    engine.state.players[controller].hand.push(object_id);
    engine.state.objects.get_mut(&object_id).unwrap().zone = Zone::Hand;
}

fn assert_robot_token_for_player(engine: &GameEngine, player: usize) -> u32 {
    let robots = battlefield_token_oids(engine, player, "robot_c_2_2");
    assert_eq!(robots.len(), 1, "the ETB creates exactly one Robot");
    let robot = robots[0];
    let object = &engine.state.objects[&robot];
    assert!(object.is_token());
    assert_eq!(object.owner, engine.state.players[player].id);
    assert_eq!(object.controller, engine.state.players[player].id);
    assert_eq!(object.zone, Zone::Battlefield);
    assert!(!object.tapped);
    assert_eq!(engine.effective_power(robot), Some(2));
    assert_eq!(engine.effective_toughness(robot), Some(2));
    let characteristics = engine
        .characteristics(robot)
        .expect("Robot characteristics");
    assert!(characteristics.has_type("Artifact"));
    assert!(characteristics.has_type("Creature"));
    assert!(characteristics.has_type("Robot"));
    robot
}

#[test]
fn issue_310_galvanizing_station_has_exact_three_threshold_and_remains_usable_above_it() {
    let (mut engine, station, crew) = station_engine(310_001, "galvanizing_sawship");

    let initial = engine
        .characteristics(station)
        .expect("initial characteristics");
    assert!(initial.has_type("Artifact"));
    assert!(!initial.has_type("Creature"));
    assert_eq!((initial.power, initial.toughness), (None, None));
    assert!(!initial.has_keyword(Keyword::Flying));
    assert!(!initial.has_keyword(Keyword::Haste));

    engine
        .state
        .objects
        .get_mut(&station)
        .unwrap()
        .set_counter(CounterKind::Charge, 2);
    let below = engine.characteristics(station).expect("below threshold");
    assert!(!below.has_type("Creature"));
    assert_eq!((below.power, below.toughness), (None, None));
    assert!(!below.has_keyword(Keyword::Flying));
    assert!(!below.has_keyword(Keyword::Haste));

    engine
        .state
        .objects
        .get_mut(&station)
        .unwrap()
        .set_counter(CounterKind::Charge, 3);
    let at = engine.characteristics(station).expect("at threshold");
    assert!(at.has_type("Artifact") && at.has_type("Creature"));
    assert_eq!((at.power, at.toughness), (Some(6), Some(5)));
    assert!(at.has_keyword(Keyword::Flying));
    assert!(at.has_keyword(Keyword::Haste));

    // Station remains an activated ability once the threshold is met.  The
    // injected 2-power creature adds exactly two charge counters on resolution.
    engine
        .apply_command(0, &activate_station(&engine, station, crew))
        .expect("Station is activatable at its threshold");
    assert!(engine.state.objects[&crew].tapped);
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        engine.state.objects[&station].counter_count(CounterKind::Charge),
        5
    );

    // A value above the threshold keeps the same continuous characteristic
    // payload and does not disable the printed Station ability.
    engine
        .state
        .objects
        .get_mut(&station)
        .unwrap()
        .set_counter(CounterKind::Charge, 4);
    let above = engine.characteristics(station).expect("above threshold");
    assert!(above.has_type("Creature"));
    assert_eq!((above.power, above.toughness), (Some(6), Some(5)));
    assert!(above.has_keyword(Keyword::Flying));
    assert!(above.has_keyword(Keyword::Haste));

    // The generated timing is sorcery-speed, not a generic normal-timing
    // ability.  The attempted upkeep activation must be atomic.
    engine.state.objects.get_mut(&crew).unwrap().tapped = false;
    engine.state.turn_step = TurnStep::Upkeep;
    let before = engine.state.objects[&station].counter_count(CounterKind::Charge);
    engine
        .apply_command(0, &activate_station(&engine, station, crew))
        .expect_err("Station is sorcery-speed outside a main phase");
    assert_eq!(
        engine.state.objects[&station].counter_count(CounterKind::Charge),
        before
    );
    assert!(!engine.state.objects[&crew].tapped);
    assert!(engine.state.stack.is_empty());
}

#[test]
fn issue_310_wedgelight_entry_creates_one_untapped_robot_and_unlocks_first_strike_at_nine() {
    let (mut engine, station, crew) = station_engine(310_002, "wedgelight_rammer");

    let robots = battlefield_token_oids(&engine, 0, "robot_c_2_2");
    assert_eq!(
        robots.len(),
        1,
        "Wedgelight creates exactly one Robot token"
    );
    let robot = robots[0];
    let robot_object = &engine.state.objects[&robot];
    assert!(robot_object.is_token());
    assert_eq!(robot_object.owner, engine.state.players[0].id);
    assert_eq!(robot_object.controller, engine.state.players[0].id);
    assert!(!robot_object.tapped);
    assert_eq!(engine.effective_power(robot), Some(2));
    assert_eq!(engine.effective_toughness(robot), Some(2));
    let robot_characteristics = engine
        .characteristics(robot)
        .expect("Robot characteristics");
    assert!(robot_characteristics.has_type("Artifact"));
    assert!(robot_characteristics.has_type("Creature"));
    assert!(robot_characteristics.has_type("Robot"));

    let initial = engine
        .characteristics(station)
        .expect("initial characteristics");
    assert!(!initial.has_type("Creature"));
    assert_eq!((initial.power, initial.toughness), (None, None));
    assert!(!initial.has_keyword(Keyword::Flying));
    assert!(!initial.has_keyword(Keyword::FirstStrike));

    engine
        .state
        .objects
        .get_mut(&station)
        .unwrap()
        .set_counter(CounterKind::Charge, 8);
    let below = engine
        .characteristics(station)
        .expect("below nine threshold");
    assert!(!below.has_type("Creature"));
    assert_eq!((below.power, below.toughness), (None, None));
    assert!(!below.has_keyword(Keyword::Flying));
    assert!(!below.has_keyword(Keyword::FirstStrike));

    engine
        .state
        .objects
        .get_mut(&station)
        .unwrap()
        .set_counter(CounterKind::Charge, 9);
    let at = engine.characteristics(station).expect("at nine threshold");
    assert!(at.has_type("Artifact") && at.has_type("Creature"));
    assert_eq!((at.power, at.toughness), (Some(3), Some(4)));
    assert!(at.has_keyword(Keyword::Flying));
    assert!(at.has_keyword(Keyword::FirstStrike));

    engine
        .apply_command(0, &activate_station(&engine, station, crew))
        .expect("Station remains activatable at nine counters");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        engine.state.objects[&station].counter_count(CounterKind::Charge),
        11
    );
    assert_eq!(
        battlefield_token_oids(&engine, 0, "robot_c_2_2").len(),
        1,
        "Station does not duplicate Wedgelight's entry token"
    );
}

#[test]
fn issue_310_wedgelight_etb_uses_trigger_controller_with_foreign_owner_in_multiplayer() {
    let mut engine = GameEngine::new(
        310_003,
        &[0, 1],
        20,
        Some(vec![
            deck_with("island", &["wedgelight_rammer"]),
            forest_only_deck(),
        ]),
        true,
    )
    .expect("generated Wedgelight engine");
    advance_to_main1_from_game_start(&mut engine);
    append_third_player(&mut engine);

    // Transfer only the physical card to player 1's hand.  The card remains
    // owned by player 0, then enters under player 1's control, which is the
    // meaningful owner/controller distinction for a triggered token effect.
    move_owned_card_into_controller_hand(&mut engine, 0, 1, "wedgelight_rammer");
    let source = move_ready_to_battlefield(&mut engine, 1, "wedgelight_rammer");
    assert_eq!(
        engine.state.objects[&source].owner,
        engine.state.players[0].id
    );
    assert_eq!(
        engine.state.objects[&source].controller,
        engine.state.players[1].id
    );
    assert_eq!(engine.state.stack.len(), 1, "the ETB trigger is pending");
    assert_eq!(
        engine.state.stack[0].controller, engine.state.players[1].id,
        "the trigger is controlled by the entering permanent's controller"
    );

    resolve_entire_stack_all_players(&mut engine);
    assert_robot_token_for_player(&engine, 1);
    assert!(battlefield_token_oids(&engine, 0, "robot_c_2_2").is_empty());
    assert!(battlefield_token_oids(&engine, 2, "robot_c_2_2").is_empty());
}

#[test]
fn issue_310_wedgelight_etb_still_creates_controller_robot_after_source_leaves() {
    let mut engine = GameEngine::new(
        310_004,
        &[0, 1],
        20,
        Some(vec![
            deck_with("island", &["wedgelight_rammer"]),
            forest_only_deck(),
        ]),
        true,
    )
    .expect("generated Wedgelight engine");
    advance_to_main1_from_game_start(&mut engine);
    let source = move_ready_to_battlefield(&mut engine, 0, "wedgelight_rammer");
    let entry_generation = generation(&engine, source);
    assert_eq!(engine.state.stack.len(), 1, "the ETB trigger is pending");
    assert_eq!(engine.state.stack[0].controller, engine.state.players[0].id);

    // A trigger on the stack is independent of its source (CR 603.3a/113.7a).
    // Move the actual physical card away through the engine so its generation
    // changes before the old ETB object resolves.
    move_named_card_to_hand_via_engine(&mut engine, 0, "wedgelight_rammer");
    assert_eq!(engine.state.objects[&source].zone, Zone::Hand);
    assert!(generation(&engine, source) > entry_generation);

    resolve_entire_stack_two_player(&mut engine);
    assert_robot_token_for_player(&engine, 0);
    assert!(battlefield_token_oids(&engine, 1, "robot_c_2_2").is_empty());
    assert!(
        engine.state.players[0]
            .battlefield
            .iter()
            .all(|object_id| *object_id != source),
        "the source remains out of play while its old trigger resolves"
    );
}
