use crate::helpers::*;
use tricerules_cards::{CounterKind, Keyword};
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::{
    dev_command, ruled_command::Cmd, ChooseTriggerTarget, DevCommand, DevMoveCard, DevZone,
    RuledCommand,
};

fn choose_trigger_target(object_id: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
            decline: false,
            selected_modes: Vec::new(),
            targets: target_object(object_id),
        })),
    }
}

fn move_ascension_to(zone: DevZone) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::DevCommand(DevCommand {
            target_player_id: 0,
            dev: Some(dev_command::Dev::MoveCard(DevMoveCard {
                card_name: "Earthbender Ascension".into(),
                zone: zone as i32,
                ready: false,
            })),
        })),
    }
}

fn threshold_engine(seed: u64) -> (GameEngine, u32, u32) {
    let decks = Some(vec![
        deck_with("forest", &["earthbender_ascension", "grizzly_bears"]),
        vec!["island".into(); 20],
    ]);
    let mut engine = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut engine);
    let ascension = relocate_to_battlefield(&mut engine, 0, "earthbender_ascension", false);
    let creature = relocate_to_battlefield(&mut engine, 0, "grizzly_bears", false);
    engine
        .state
        .objects
        .get_mut(&ascension)
        .expect("Earthbender Ascension")
        .counters
        .insert(CounterKind::Quest, 3);
    (engine, ascension, creature)
}

#[test]
fn earthbender_ascension_etb_earthbends_then_searches_and_landfalls() {
    let decks = Some(vec![
        deck_with("forest", &["earthbender_ascension"]),
        vec!["island".into(); 20],
    ]);
    let mut engine = GameEngine::new(211_000, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut engine);
    let land = inject_permanent_on_battlefield(&mut engine, 0, "forest");
    let searched_land = inject_library_card(&mut engine, 0, "plains");

    let ascension = move_ready_to_battlefield(&mut engine, 0, "earthbender_ascension");
    assert_eq!(engine.state.pending_triggers.len(), 1);
    engine
        .apply_command(0, &choose_trigger_target(land))
        .expect("choose land to earthbend");
    let first = engine.state.priority_player_id();
    let second = if first == 0 { 1 } else { 0 };
    engine
        .apply_command(first, &pass())
        .expect("first player passes");
    let search_batch = engine
        .apply_command(second, &pass())
        .expect("second player passes and starts resolution");
    let choice = find_resolution_choice(&search_batch).expect("basic-land search choice");
    assert!(choice.candidate_object_ids.contains(&searched_land));

    engine
        .apply_command(0, &submit_resolution_choice(vec![searched_land]))
        .expect("put Plains onto the battlefield");

    let earthbent = engine.characteristics(land).expect("earthbent land");
    assert!(earthbent.is_creature());
    assert_eq!((earthbent.power, earthbent.toughness), (Some(2), Some(2)));
    assert!(engine.effective_has_keyword(land, Keyword::Haste));
    assert_eq!(engine.state.objects[&searched_land].zone, Zone::Battlefield);
    assert!(engine.state.objects[&searched_land].tapped);
    assert_eq!(engine.state.stack.len(), 1, "searched land causes landfall");

    pass_both_players(&mut engine);
    assert_eq!(
        engine.state.objects[&ascension].counter_count(CounterKind::Quest),
        1
    );
    assert!(engine.state.pending_triggers.is_empty());
}

#[test]
fn fourth_quest_counter_creates_a_targeted_reflexive_trigger() {
    let (mut engine, ascension, creature) = threshold_engine(211_001);
    let forest = relocate_to_hand(&mut engine, 0, "forest");
    let forest_slot = engine.state.players[0]
        .hand
        .iter()
        .position(|candidate| *candidate == forest)
        .expect("Forest in hand");

    engine
        .apply_command(0, &play_land(forest_slot))
        .expect("play Forest");
    pass_both_players(&mut engine);

    assert_eq!(
        engine.state.objects[&ascension].counter_count(CounterKind::Quest),
        4
    );
    assert_eq!(engine.state.pending_triggers.len(), 1);
    engine
        .apply_command(0, &choose_trigger_target(creature))
        .expect("target controlled creature");
    pass_both_players(&mut engine);

    assert_eq!(
        engine.state.objects[&creature].counter_count(CounterKind::PlusOnePlusOne),
        1
    );
    assert!(engine.effective_has_keyword(creature, Keyword::Trample));
}

#[test]
fn reflexive_intervening_if_is_rechecked_on_resolution() {
    let (mut engine, ascension, creature) = threshold_engine(211_002);
    let forest = relocate_to_hand(&mut engine, 0, "forest");
    let forest_slot = engine.state.players[0]
        .hand
        .iter()
        .position(|candidate| *candidate == forest)
        .expect("Forest in hand");

    engine
        .apply_command(0, &play_land(forest_slot))
        .expect("play Forest");
    pass_both_players(&mut engine);
    engine
        .apply_command(0, &choose_trigger_target(creature))
        .expect("target controlled creature");
    engine
        .state
        .objects
        .get_mut(&ascension)
        .expect("Earthbender Ascension")
        .counters
        .insert(CounterKind::Quest, 3);
    pass_both_players(&mut engine);

    assert_eq!(
        engine.state.objects[&creature].counter_count(CounterKind::PlusOnePlusOne),
        0
    );
    assert!(!engine.effective_has_keyword(creature, Keyword::Trample));
}

#[test]
fn reflexive_trigger_uses_last_known_counters_after_its_source_leaves() {
    let (mut engine, ascension, creature) = threshold_engine(211_004);
    let forest = relocate_to_hand(&mut engine, 0, "forest");
    let forest_slot = engine.state.players[0]
        .hand
        .iter()
        .position(|candidate| *candidate == forest)
        .expect("Forest in hand");

    engine
        .apply_command(0, &play_land(forest_slot))
        .expect("play Forest");
    pass_both_players(&mut engine);
    engine
        .apply_command(0, &choose_trigger_target(creature))
        .expect("target controlled creature");
    engine.enable_dev_commands();
    engine
        .apply_command(0, &move_ascension_to(DevZone::Graveyard))
        .expect("move source before reflexive trigger resolves");
    assert_eq!(engine.state.objects[&ascension].zone, Zone::Graveyard);
    pass_both_players(&mut engine);

    assert_eq!(
        engine.state.objects[&creature].counter_count(CounterKind::PlusOnePlusOne),
        1
    );
    assert!(engine.effective_has_keyword(creature, Keyword::Trample));
}

#[test]
fn reflexive_intervening_if_uses_departed_generation_not_returned_counters() {
    // CR 603.4 and 608.2h: recheck the original source using its exact-generation LKI.
    for departed_count in [3, 4] {
        for returned_count in [0, 9] {
            let (mut engine, ascension, creature) = threshold_engine(211_005);
            let forest = relocate_to_hand(&mut engine, 0, "forest");
            let slot = engine.state.players[0]
                .hand
                .iter()
                .position(|candidate| *candidate == forest)
                .unwrap();
            engine.apply_command(0, &play_land(slot)).unwrap();
            pass_both_players(&mut engine);
            engine
                .apply_command(0, &choose_trigger_target(creature))
                .unwrap();
            assert_eq!(engine.state.stack.len(), 1);
            let generation = engine
                .state
                .zone_change_generation
                .get(&ascension)
                .copied()
                .unwrap_or(0);
            engine
                .state
                .objects
                .get_mut(&ascension)
                .unwrap()
                .counters
                .insert(CounterKind::Quest, departed_count);
            engine.enable_dev_commands();
            engine
                .apply_command(0, &move_ascension_to(DevZone::Graveyard))
                .unwrap();
            engine
                .apply_command(0, &move_ascension_to(DevZone::Battlefield))
                .unwrap();
            assert_ne!(engine.state.zone_change_generation[&ascension], generation);
            engine
                .state
                .objects
                .get_mut(&ascension)
                .unwrap()
                .counters
                .insert(CounterKind::Quest, returned_count);

            // Resolve only the returned incarnation's entry ability first. Search for zero
            // cards so no fresh landfall can change its counters or create another reflexive.
            assert_eq!(engine.state.pending_triggers.len(), 1);
            engine
                .apply_command(0, &choose_trigger_target(forest))
                .unwrap();
            pass_both_players(&mut engine);
            engine
                .apply_command(0, &submit_resolution_choice(vec![]))
                .unwrap();
            assert_eq!(
                engine.state.stack.len(),
                1,
                "only the original reflexive remains"
            );
            pass_both_players(&mut engine);
            assert!(engine.state.pending_triggers.is_empty());
            assert!(engine.state.stack.is_empty());
            assert_eq!(
                engine.state.objects[&creature].counter_count(CounterKind::PlusOnePlusOne),
                u32::from(departed_count == 4)
            );
            assert_eq!(
                engine.effective_has_keyword(creature, Keyword::Trample),
                departed_count == 4
            );
            assert_eq!(
                engine.state.objects[&ascension].counter_count(CounterKind::Quest),
                returned_count
            );
        }
    }
}

#[test]
fn fewer_than_four_quest_counters_do_not_stage_the_reflexive_trigger() {
    let (mut engine, ascension, _creature) = threshold_engine(211_003);
    engine
        .state
        .objects
        .get_mut(&ascension)
        .expect("Earthbender Ascension")
        .counters
        .insert(CounterKind::Quest, 1);
    let forest = relocate_to_hand(&mut engine, 0, "forest");
    let forest_slot = engine.state.players[0]
        .hand
        .iter()
        .position(|candidate| *candidate == forest)
        .expect("Forest in hand");

    engine
        .apply_command(0, &play_land(forest_slot))
        .expect("play Forest");
    pass_both_players(&mut engine);

    assert_eq!(
        engine.state.objects[&ascension].counter_count(CounterKind::Quest),
        2
    );
    assert!(engine.state.pending_triggers.is_empty());
    assert!(engine.state.stack.is_empty());
}
