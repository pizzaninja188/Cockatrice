use crate::helpers::ScenarioCommandExt;
use crate::helpers::*;
use tricerules_cards::{CounterKind, Keyword};
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::{
    dev_command, ruled_command::Cmd, ChooseTriggerTarget, DevCommand, DevMoveCard,
    DevPutCardInZone, DevZone, RuledCommand,
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
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("new");
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

fn counter_receipt_program_fixture(
    id: &str,
    producer_steps: &str,
    prohibit_counters: bool,
) -> String {
    let static_abilities = if prohibit_counters {
        r#"static_abilities: [(ability_id: "static_01", presentation: Fallback,
            definition: ProhibitCounters(affected: Self_))],"#
    } else {
        ""
    };
    format!(
        r#"(
            id: "{id}", name: "Counter Receipt Runtime Fixture", face_id: "{id}",
            types: ["Creature"], power: 2, toughness: 2,
            {static_abilities}
            triggered_abilities: [(ability_id: "triggered_01", presentation: Fallback,
                trigger: WhenSelfEntersBattlefield, effect: [],
                program: Some((body: (steps: [
                    {producer_steps}
                    (id: "reflexive", input: Some("placed"), kind: Perform(
                        CreateReflexiveTrigger(
                            when: Some(CountersPlaced(counter: Charge, object: Source)),
                            ability: (
                                ability_id: "reflexive_01", presentation: Fallback,
                                intervening_if: None, effect: [GainLife(amount: 1)],
                                targeting: None,
                            ),
                        ),
                    )),
                    (id: "tail", kind: Perform(GainLife(amount: 1))),
                ], yields: []))),
                modal: None, targeting: None, may: false, intervening_if: None,
                triggers_only_once: false, max_triggers_per_turn: None,
            )]
        )"#
    )
}

fn counter_receipt_runtime_engine(seed: u64, fixture: &str, id: &str) -> GameEngine {
    let registry = Box::leak(Box::new(
        tricerules_card_model::CardRegistry::from_chunks_and_tokens(
            &[
                fixture,
                include_str!("../../../tricerules-cards/data/doubling_season.ron"),
            ],
            &[],
        )
        .expect("test-only typed counter receipt fixture compiles"),
    ));
    let deck = vec![id.to_string(); 20];
    let mut engine = GameEngine::new(
        registry,
        seed,
        &[0, 1],
        20,
        Some(vec![deck.clone(), deck]),
        true,
    )
    .expect("counter receipt fixture game");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn move_counter_receipt_fixture_to_battlefield(engine: &mut GameEngine, player: usize) -> u32 {
    let before = engine.state.players[player].battlefield.clone();
    let player_id = engine.state.players[player].id;
    engine.enable_dev_commands();
    engine
        .apply_scenario_command(
            player_id,
            &RuledCommand {
                cmd: Some(Cmd::DevCommand(DevCommand {
                    target_player_id: player_id,
                    dev: Some(dev_command::Dev::MoveCard(DevMoveCard {
                        card_name: "Counter Receipt Runtime Fixture".into(),
                        zone: DevZone::Battlefield as i32,
                        ready: true,
                    })),
                })),
            },
        )
        .expect("enter test-only counter receipt source through engine");
    let object_id = *engine.state.players[player]
        .battlefield
        .iter()
        .find(|object_id| !before.contains(object_id))
        .expect("test-only source entered the battlefield");
    object_id
}

fn conjure_doubling_season(engine: &mut GameEngine, player: usize) {
    let player_id = engine.state.players[player].id;
    engine.enable_dev_commands();
    engine
        .apply_scenario_command(
            player_id,
            &RuledCommand {
                cmd: Some(Cmd::DevCommand(DevCommand {
                    target_player_id: player_id,
                    dev: Some(dev_command::Dev::PutCardInZone(DevPutCardInZone {
                        card_name: "Doubling Season".into(),
                        zone: DevZone::Battlefield as i32,
                        ready: true,
                    })),
                })),
            },
        )
        .expect("put the existing Doubling Season replacement onto the battlefield");
}

#[test]
fn dynamic_source_counter_amount_emits_one_charge_receipt_and_resumes_tail_once() {
    let id = "counter_receipt_dynamic_fixture";
    let fixture = counter_receipt_program_fixture(
        id,
        r#"(id: "seed_charge", kind: Perform(
                PutCounters(counter: Charge, count: 2, subject: Source),
            )),
            (id: "place_dynamic_charge", bind: ["placed"], kind: Perform(
                PutCounters(
                    counter: Charge,
                    count: Count(SourceCounterCount(counter: Charge)),
                    subject: Source,
                ),
            )),"#,
        false,
    );
    let mut engine = counter_receipt_runtime_engine(211_090, &fixture, id);
    let source = move_counter_receipt_fixture_to_battlefield(&mut engine, 0);
    let source_generation = engine.state.zone_change_generation[&source];
    assert_eq!(engine.state.stack.len(), 1);

    pass_both_players(&mut engine);
    assert_eq!(
        engine.state.stack.len(),
        1,
        "the committed placement stages one reflexive trigger after the original ability"
    );
    pass_both_players(&mut engine);

    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::Charge),
        4
    );
    assert_eq!(
        engine.state.players[0].life, 22,
        "one reflexive effect and one Program tail"
    );
    assert_eq!(
        engine.state.zone_change_generation[&source],
        source_generation
    );
    assert!(engine.state.pending_triggers.is_empty());
    assert!(engine.state.stack.is_empty());
}

#[test]
fn zero_counter_amount_writes_none_and_resumes_tail_once() {
    let id = "counter_receipt_zero_fixture";
    let fixture = counter_receipt_program_fixture(
        id,
        r#"(id: "place_zero_charge", bind: ["placed"], kind: Perform(
                PutCounters(counter: Charge, count: 0, subject: Source),
            )),"#,
        false,
    );
    let mut engine = counter_receipt_runtime_engine(211_091, &fixture, id);
    let source = move_counter_receipt_fixture_to_battlefield(&mut engine, 0);
    assert_eq!(engine.state.stack.len(), 1);

    pass_both_players(&mut engine);

    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::Charge),
        0
    );
    assert_eq!(
        engine.state.players[0].life, 21,
        "only the outer Program tail runs"
    );
    assert!(engine.state.pending_triggers.is_empty());
    assert!(engine.state.stack.is_empty());
}

#[test]
fn tatterkite_counter_prevention_captures_none_and_resumes_tail_once() {
    let id = "counter_receipt_tatterkite_fixture";
    let fixture = counter_receipt_program_fixture(
        id,
        r#"(id: "place_charge", bind: ["placed"], kind: Perform(
                PutCounters(counter: Charge, count: 2, subject: Source),
            )),"#,
        true,
    );
    let mut engine = counter_receipt_runtime_engine(211_092, &fixture, id);
    let source = move_counter_receipt_fixture_to_battlefield(&mut engine, 0);
    assert_eq!(engine.state.stack.len(), 1);

    pass_both_players(&mut engine);

    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::Charge),
        0
    );
    assert_eq!(
        engine.state.players[0].life, 21,
        "only the outer Program tail runs"
    );
    assert!(engine.state.pending_triggers.is_empty());
    assert!(engine.state.stack.is_empty());
}

#[test]
fn replacement_doubled_charge_placement_projects_one_receipt_and_reflexive_trigger() {
    let id = "counter_receipt_doubled_fixture";
    let fixture = counter_receipt_program_fixture(
        id,
        r#"(id: "place_charge", bind: ["placed"], kind: Perform(
                PutCounters(counter: Charge, count: 2, subject: Source),
            )),"#,
        false,
    );
    let mut engine = counter_receipt_runtime_engine(211_093, &fixture, id);
    conjure_doubling_season(&mut engine, 0);
    let source = move_counter_receipt_fixture_to_battlefield(&mut engine, 0);
    assert_eq!(engine.state.stack.len(), 1);

    pass_both_players(&mut engine);

    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::Charge),
        4
    );
    assert_eq!(
        engine.state.stack.len(),
        1,
        "replacement-expanded placement creates one reflexive trigger receipt"
    );
    pass_both_players(&mut engine);

    assert_eq!(
        engine.state.players[0].life, 22,
        "one reflexive effect and one tail"
    );
    assert!(engine.state.stack.is_empty());
}

#[test]
fn earthbender_ascension_etb_earthbends_then_searches_and_landfalls() {
    let decks = Some(vec![
        deck_with("forest", &["earthbender_ascension"]),
        vec!["island".into(); 20],
    ]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        211_000,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("new");
    advance_to_main1_from_game_start(&mut engine);
    let land = inject_permanent_on_battlefield(&mut engine, 0, "forest");
    let searched_land = inject_library_card(&mut engine, 0, "plains");

    let ascension = move_ready_to_battlefield(&mut engine, 0, "earthbender_ascension");
    assert_eq!(engine.state.pending_triggers.len(), 1);
    engine
        .apply_scenario_command(0, &choose_trigger_target(land))
        .expect("choose land to earthbend");
    let first = engine.state.priority_player_id();
    let second = if first == 0 { 1 } else { 0 };
    engine
        .apply_scenario_command(first, &pass())
        .expect("first player passes");
    let search_batch = engine
        .apply_scenario_command(second, &pass())
        .expect("second player passes and starts resolution");
    let choice = find_resolution_choice(&search_batch).expect("basic-land search choice");
    assert!(choice.candidate_object_ids.contains(&searched_land));

    engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![searched_land]))
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
        .apply_scenario_command(0, &play_land(forest_slot))
        .expect("play Forest");
    pass_both_players(&mut engine);

    assert_eq!(
        engine.state.objects[&ascension].counter_count(CounterKind::Quest),
        4
    );
    assert_eq!(engine.state.pending_triggers.len(), 1);
    engine
        .apply_scenario_command(0, &choose_trigger_target(creature))
        .expect("target controlled creature");
    pass_both_players(&mut engine);

    assert_eq!(
        engine.state.objects[&creature].counter_count(CounterKind::PlusOnePlusOne),
        1
    );
    assert!(engine.effective_has_keyword(creature, Keyword::Trample));
}

#[test]
fn replacement_choices_resume_one_counter_receipt_and_one_reflexive_trigger() {
    let (mut engine, ascension, creature) = threshold_engine(211_006);
    inject_permanent_on_battlefield(&mut engine, 0, "doubling_season");
    inject_permanent_on_battlefield(&mut engine, 0, "doubling_season");
    let forest = relocate_to_hand(&mut engine, 0, "forest");
    let forest_slot = engine.state.players[0]
        .hand
        .iter()
        .position(|candidate| *candidate == forest)
        .expect("Forest in hand");

    engine
        .apply_scenario_command(0, &play_land(forest_slot))
        .expect("play Forest");
    pass_both_players(&mut engine);

    let choice = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("counter replacement ordering parks the typed Program");
    assert_eq!(
        choice.presentation.choice_kind,
        tricerules_proto::ruled::v1::ChoiceKind::ReplacementEffect
    );
    assert_eq!(choice.presentation.candidates.len(), 2);
    assert_eq!(
        engine.state.objects[&ascension].counter_count(CounterKind::Quest),
        3,
        "the Quest placement remains uncommitted while its replacement order is chosen"
    );
    let selected = choice.presentation.candidates[0];
    engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![selected]))
        .expect("finish the parked counter replacement batch");

    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(
        engine.state.objects[&ascension].counter_count(CounterKind::Quest),
        7,
        "both Doubling Seasons modify the one committed placement"
    );
    assert_eq!(
        engine.state.pending_triggers.len(),
        1,
        "the replacement-expanded placement produces exactly one reflexive event"
    );
    engine
        .apply_scenario_command(0, &choose_trigger_target(creature))
        .expect("target controlled creature");
    pass_both_players(&mut engine);
    let choice = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("replacement ordering parks the single reflexive counter placement");
    assert_eq!(
        choice.presentation.choice_kind,
        tricerules_proto::ruled::v1::ChoiceKind::ReplacementEffect
    );
    assert_eq!(choice.presentation.candidates.len(), 2);
    assert_eq!(
        engine.state.objects[&creature].counter_count(CounterKind::PlusOnePlusOne),
        0,
        "the reflexive placement remains uncommitted while its replacement order is chosen"
    );
    let selected = choice.presentation.candidates[0];
    engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![selected]))
        .expect("finish the parked reflexive counter placement");

    assert_eq!(
        engine.state.objects[&creature].counter_count(CounterKind::PlusOnePlusOne),
        4,
        "the parked Program tail resumes once"
    );
    assert!(engine.effective_has_keyword(creature, Keyword::Trample));
    assert!(engine.state.pending_triggers.is_empty());
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
        .apply_scenario_command(0, &play_land(forest_slot))
        .expect("play Forest");
    pass_both_players(&mut engine);
    engine
        .apply_scenario_command(0, &choose_trigger_target(creature))
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
        .apply_scenario_command(0, &play_land(forest_slot))
        .expect("play Forest");
    pass_both_players(&mut engine);
    engine
        .apply_scenario_command(0, &choose_trigger_target(creature))
        .expect("target controlled creature");
    engine.enable_dev_commands();
    engine
        .apply_scenario_command(0, &move_ascension_to(DevZone::Graveyard))
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
            engine.apply_scenario_command(0, &play_land(slot)).unwrap();
            pass_both_players(&mut engine);
            engine
                .apply_scenario_command(0, &choose_trigger_target(creature))
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
                .apply_scenario_command(0, &move_ascension_to(DevZone::Graveyard))
                .unwrap();
            engine
                .apply_scenario_command(0, &move_ascension_to(DevZone::Battlefield))
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
                .apply_scenario_command(0, &choose_trigger_target(forest))
                .unwrap();
            pass_both_players(&mut engine);
            engine
                .apply_scenario_command(0, &submit_resolution_choice(vec![]))
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
        .apply_scenario_command(0, &play_land(forest_slot))
        .expect("play Forest");
    pass_both_players(&mut engine);

    assert_eq!(
        engine.state.objects[&ascension].counter_count(CounterKind::Quest),
        2
    );
    assert!(engine.state.pending_triggers.is_empty());
    assert!(engine.state.stack.is_empty());
}
