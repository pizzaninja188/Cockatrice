//! Issue #423 — Station threshold batch scenario coverage.
//!
//! Oracle and the current Comprehensive Rules were checked 2026-09-19 against the pinned
//! Scryfall `oracle_cards` snapshot. Station (CR 702.184) is an activated ability whose cost
//! taps another creature you control and whose resolution reads that creature's power; the
//! threshold lines are continuously reevaluated static abilities (CR 611.3/613) with CR 721.2a
//! base P/T for the artifact-creature animation. Sorcery-speed timing follows CR 602.2/702.184a,
//! threshold trigger collection follows CR 603, the optional sacrifice/search follows CR 701.23,
//! the bounce follows CR 400.7 hidden-information-safe return, the mill follows CR 701.13, and
//! the defending-player trigger follows CR 506.2/508.1.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_cards::{CounterKind, Keyword};
use tricerules_core::{TurnStep, Zone};
use tricerules_proto::ruled::v1::{
    cost_selection::Selection, ChoiceKind, CostObjectRef, CostObjectRefs, CostSelection,
    ResolutionChoiceDecision, SubmitResolutionChoice, TargetRef, TargetRefKind,
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

fn activate_with_generation(
    engine: &GameEngine,
    source: u32,
    ability_index: u32,
    cost_selections: Vec<CostSelection>,
) -> RuledCommand {
    let mut command =
        activate_ability_with_costs(source, ability_index, Vec::new(), cost_selections);
    let Some(Cmd::ActivateAbility(activation)) = command.cmd.as_mut() else {
        unreachable!()
    };
    activation.expected_zone_change_generation = generation(engine, source);
    command
}

fn choose_trigger_targets(targets: Vec<TargetRef>) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ChooseTriggerTarget(
            tricerules_proto::ruled::v1::ChooseTriggerTarget {
                targets,
                ..Default::default()
            },
        )),
    }
}

fn trigger_target(object_id: u32) -> TargetRef {
    TargetRef {
        object_id,
        group_index: 0,
        kind: TargetRefKind::Permanent as i32,
        ..Default::default()
    }
}

fn select_branch(index: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
            resolution_prompt_id: 0,
            decision: ResolutionChoiceDecision::SelectBranch as i32,
            selected_branch_index: index,
            ..Default::default()
        })),
    }
}

fn issue_423_engine(seed: u64, card_id: &str) -> GameEngine {
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1],
        20,
        Some(vec![
            deck_with("forest", &[card_id]),
            deck_with("island", &["grizzly_bears"]),
        ]),
        true,
    )
    .unwrap_or_else(|error| panic!("{card_id} must be registered: {error}"));
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn larval_search_privacy_engine(seed: u64) -> GameEngine {
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1],
        20,
        Some(vec![
            deck_with("grizzly_bears", &["larval_scoutlander"]),
            deck_with("grizzly_bears", &["grizzly_bears"]),
        ]),
        true,
    )
    .unwrap_or_else(|error| panic!("Larval Scoutlander privacy fixture must be valid: {error}"));
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn deploy(engine: &mut GameEngine, card_id: &str) -> u32 {
    move_ready_to_battlefield(engine, 0, card_id)
}

/// Resolve a permanent's non-targeted entry trigger and return to a clean priority state.
fn settle_entry(engine: &mut GameEngine) {
    answer_trigger_order_in_engine_order(engine);
    resolve_entire_stack_two_player(engine);
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.pending_triggers.is_empty());
    assert!(engine.state.staged_trigger_groups.is_empty());
}

fn resolve_larval_scoutlander_entry_trigger(engine: &mut GameEngine) -> (u32, RuledEventBatch) {
    let larval = deploy(engine, "larval_scoutlander");
    answer_trigger_order_in_engine_order(engine);
    engine
        .apply_scenario_command(0, &pass())
        .expect("controller passes priority");
    let branch_batch = engine
        .apply_scenario_command(1, &pass())
        .expect("resolve Larval Scoutlander's entry trigger");
    (larval, branch_batch)
}

fn shuffle_log_count(batch: &RuledEventBatch, player: i32) -> usize {
    batch
        .events
        .iter()
        .filter(|event| {
            matches!(
                &event.ev,
                Some(Ev::Log(log)) if log.text == format!("P{player} shuffles their library.")
            )
        })
        .count()
}

fn assert_no_search_reveal(batch: &RuledEventBatch) {
    assert!(batch
        .events
        .iter()
        .all(|event| !matches!(&event.ev, Some(Ev::CardsRevealed(_)))));
}

fn assert_one_search_observer(engine: &GameEngine, wan: u32) {
    assert_eq!(engine.state.stack.len(), 1, "one LibrarySearched observer");
    assert_eq!(engine.state.stack[0].source_permanent_id, Some(wan));
}

fn advance_main1_to_declare_attackers(engine: &mut GameEngine) {
    engine
        .apply_scenario_command(0, &primitive_yield())
        .expect("main phase to beginning of combat");
    engine
        .apply_scenario_command(0, &pass())
        .expect("active player passes in beginning of combat");
    engine
        .apply_scenario_command(1, &pass())
        .expect("defender passes in beginning of combat");
    assert_eq!(engine.state.turn_step, TurnStep::DeclareAttackers);
}

fn set_charge(engine: &mut GameEngine, object_id: u32, count: u32) {
    engine
        .state
        .objects
        .get_mut(&object_id)
        .unwrap()
        .set_counter(CounterKind::Charge, count);
    engine.initial_response_batch();
}

#[test]
fn issue_423_station_crews_with_the_tapped_creatures_power_at_sorcery_speed() {
    let mut engine = issue_423_engine(423_001, "atmospheric_greenhouse");
    let greenhouse = deploy(&mut engine, "atmospheric_greenhouse");
    settle_entry(&mut engine);
    let crew = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    engine.state.objects.get_mut(&crew).unwrap().summoning_sick = true;

    let initial = engine.characteristics(greenhouse).unwrap();
    assert!(initial.has_type("Artifact"));
    assert!(!initial.has_type("Creature"));
    assert_eq!((initial.power, initial.toughness), (None, None));

    engine
        .apply_scenario_command(0, &activate_station(&engine, greenhouse, crew))
        .expect("a summoning-sick creature may pay the Station tap cost");
    assert!(engine.state.objects[&crew].tapped);
    assert!(!engine.state.objects[&greenhouse].tapped);
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        engine.state.objects[&greenhouse].counter_count(CounterKind::Charge),
        2,
        "Station puts charge counters equal to the tapped creature's power"
    );

    let below = engine.characteristics(greenhouse).unwrap();
    assert!(!below.has_type("Creature"));
    assert_eq!((below.power, below.toughness), (None, None));

    set_charge(&mut engine, greenhouse, 8);
    let at_threshold = engine.characteristics(greenhouse).unwrap();
    assert!(at_threshold.has_type("Artifact") && at_threshold.has_type("Creature"));
    assert_eq!(
        (at_threshold.power, at_threshold.toughness),
        (Some(5), Some(4))
    );
    assert!(at_threshold.has_keyword(Keyword::Flying));
    assert!(at_threshold.has_keyword(Keyword::Trample));

    set_charge(&mut engine, greenhouse, 7);
    assert!(!engine
        .characteristics(greenhouse)
        .unwrap()
        .has_type("Creature"));

    engine.state.objects.get_mut(&crew).unwrap().tapped = false;
    engine.state.turn_step = TurnStep::Upkeep;
    let before = engine.state.objects[&greenhouse].counter_count(CounterKind::Charge);
    assert!(
        engine
            .apply_scenario_command(0, &activate_station(&engine, greenhouse, crew))
            .is_err(),
        "Station is sorcery-speed only (CR 702.184a)"
    );
    assert_eq!(
        engine.state.objects[&greenhouse].counter_count(CounterKind::Charge),
        before
    );
    assert!(!engine.state.objects[&crew].tapped);
}

#[test]
fn issue_423_threshold_statics_switch_on_at_exactly_n() {
    let mut engine = issue_423_engine(423_002, "lumen-class_frigate");
    let lumen = deploy(&mut engine, "lumen-class_frigate");
    let bears = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");

    set_charge(&mut engine, lumen, 1);
    assert_eq!(
        (
            engine.effective_power(bears),
            engine.effective_toughness(bears)
        ),
        (Some(2), Some(2)),
        "the 2+ anthem is off below its threshold"
    );
    set_charge(&mut engine, lumen, 2);
    assert_eq!(
        (
            engine.effective_power(bears),
            engine.effective_toughness(bears)
        ),
        (Some(3), Some(3)),
        "the 2+ anthem boosts other creatures you control at exactly 2"
    );
    assert!(
        !engine.characteristics(lumen).unwrap().has_type("Creature"),
        "the 12+ animation is off at 2 counters"
    );
    set_charge(&mut engine, lumen, 12);
    let animated = engine.characteristics(lumen).unwrap();
    assert!(animated.has_type("Creature"));
    assert_eq!((animated.power, animated.toughness), (Some(3), Some(5)));
    assert!(animated.has_keyword(Keyword::Flying));
    assert!(animated.has_keyword(Keyword::Lifelink));

    // Specimen Freighter and Larval Scoutlander gate their entry choices; their thresholds are
    // pinned by the companion scenarios instead.
    for (offset, card_id, threshold, power, toughness) in [
        (10, "synthesizer_labship", 9, 4, 4),
        (20, "sledge-class_seedship", 7, 4, 5),
        (40, "dawnsire,_sunstar_dreadnought", 20, 20, 20),
    ] {
        let mut engine = issue_423_engine(423_100 + offset as u64, card_id);
        let space = deploy(&mut engine, card_id);
        set_charge(&mut engine, space, threshold - 1);
        assert!(
            !engine.characteristics(space).unwrap().has_type("Creature"),
            "{card_id} must not animate below {threshold}+"
        );
        set_charge(&mut engine, space, threshold);
        let animated = engine.characteristics(space).unwrap();
        assert!(
            animated.has_type("Creature"),
            "{card_id} must animate at exactly {threshold}+"
        );
        assert_eq!(
            (animated.power, animated.toughness),
            (Some(power), Some(toughness)),
            "{card_id}"
        );
    }
}

#[test]
fn issue_423_dawnsire_attack_trigger_needs_ten_charge_counters() {
    // Below the 10+ threshold no trigger is collected; the Spacecraft itself is not animated
    // until 20+, so the trigger watches the controller's whole attack declaration.
    let mut below = issue_423_engine(423_010, "dawnsire,_sunstar_dreadnought");
    let dawnsire = deploy(&mut below, "dawnsire,_sunstar_dreadnought");
    set_charge(&mut below, dawnsire, 9);
    let own = inject_creature_on_battlefield(&mut below, 0, "grizzly_bears");
    let prey = inject_creature_on_battlefield(&mut below, 1, "grizzly_bears");
    advance_main1_to_declare_attackers(&mut below);
    below
        .apply_scenario_command(0, &declare_attackers(vec![own]))
        .expect("declare the only attacker");
    resolve_entire_stack_two_player(&mut below);
    assert_eq!(below.state.objects[&prey].damage, 0);
    assert_eq!(below.state.objects[&prey].zone, Zone::Battlefield);

    let mut above = issue_423_engine(423_011, "dawnsire,_sunstar_dreadnought");
    let dawnsire = deploy(&mut above, "dawnsire,_sunstar_dreadnought");
    set_charge(&mut above, dawnsire, 10);
    let own = inject_creature_on_battlefield(&mut above, 0, "grizzly_bears");
    let prey = inject_creature_on_battlefield(&mut above, 1, "grizzly_bears");
    advance_main1_to_declare_attackers(&mut above);
    above
        .apply_scenario_command(0, &declare_attackers(vec![own]))
        .expect("declare the only attacker");
    above
        .apply_scenario_command(0, &choose_trigger_targets(vec![trigger_target(prey)]))
        .expect("choose the creature or planeswalker target");
    resolve_entire_stack_two_player(&mut above);
    assert!(
        above.state.objects[&prey].damage == 100
            || above.state.objects[&prey].zone == Zone::Graveyard,
        "Dawnsire deals 100 damage to the chosen target"
    );
}

#[test]
fn issue_423_synthesizer_animates_one_other_artifact_at_beginning_of_combat() {
    let mut engine = issue_423_engine(423_020, "synthesizer_labship");
    let labs = deploy(&mut engine, "synthesizer_labship");
    let sword = inject_permanent_on_battlefield(&mut engine, 0, "short_sword");
    set_charge(&mut engine, labs, 2);
    engine
        .apply_scenario_command(0, &primitive_yield())
        .expect("main phase to beginning of combat");
    engine
        .apply_scenario_command(0, &choose_trigger_targets(vec![trigger_target(sword)]))
        .expect("choose up to one other target artifact you control");
    for _ in 0..4 {
        if engine.state.turn_step == TurnStep::DeclareAttackers {
            break;
        }
        let priority = engine.state.priority_player_id();
        engine
            .apply_scenario_command(priority, &pass())
            .expect("pass through the beginning of combat");
    }
    assert_eq!(engine.state.turn_step, TurnStep::DeclareAttackers);
    let animated = engine
        .characteristics(sword)
        .expect("the artifact survives");
    assert!(animated.has_type("Artifact") && animated.has_type("Creature"));
    assert_eq!((animated.power, animated.toughness), (Some(2), Some(2)));
    assert!(animated.has_keyword(Keyword::Flying));
}

#[test]
fn issue_423_companion_entry_and_attack_abilities_resolve() {
    // Atmospheric Greenhouse's entry counters every creature you control.
    let mut engine = issue_423_engine(423_030, "atmospheric_greenhouse");
    let bears = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let opponent_bears = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let greenhouse = deploy(&mut engine, "atmospheric_greenhouse");
    settle_entry(&mut engine);
    assert_eq!(
        (
            engine.effective_power(bears),
            engine.effective_toughness(bears)
        ),
        (Some(3), Some(3))
    );
    assert_eq!(
        (
            engine.effective_power(opponent_bears),
            engine.effective_toughness(opponent_bears)
        ),
        (Some(2), Some(2)),
        "the entry counters only creatures you control"
    );
    assert_eq!(
        engine.state.objects[&greenhouse].counter_count(CounterKind::PlusOnePlusOne),
        0
    );

    // Specimen Freighter's entry returns up to two non-Spacecraft creatures and its attack
    // trigger mills the defending player.
    let mut engine = issue_423_engine(423_031, "specimen_freighter");
    let first = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let second = inject_creature_on_battlefield(&mut engine, 1, "storm_crow");
    let freighter = deploy(&mut engine, "specimen_freighter");
    engine
        .apply_scenario_command(
            0,
            &choose_trigger_targets(vec![trigger_target(first), trigger_target(second)]),
        )
        .expect("choose both non-Spacecraft creatures");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&first].zone, Zone::Hand);
    assert_eq!(engine.state.objects[&second].zone, Zone::Hand);
    set_charge(&mut engine, freighter, 8);
    assert!(
        !engine
            .characteristics(freighter)
            .unwrap()
            .has_type("Creature"),
        "Specimen Freighter must stay unanimated below 9+"
    );
    set_charge(&mut engine, freighter, 9);
    let library_before = engine.state.players[1].library.len();
    advance_main1_to_declare_attackers(&mut engine);
    engine
        .apply_scenario_command(0, &declare_attackers(vec![freighter]))
        .expect("the animated Freighter attacks");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.players[1].library.len(), library_before - 4);

    // Sledge-Class Seedship may put a creature card from hand onto the battlefield.
    let mut engine = issue_423_engine(423_032, "sledge-class_seedship");
    let sledge = deploy(&mut engine, "sledge-class_seedship");
    engine
        .state
        .objects
        .get_mut(&sledge)
        .unwrap()
        .summoning_sick = false;
    set_charge(&mut engine, sledge, 7);
    let hand_creature = inject_card_into_hand(&mut engine, 0, "grizzly_bears");
    advance_main1_to_declare_attackers(&mut engine);
    engine
        .apply_scenario_command(0, &declare_attackers(vec![sledge]))
        .expect("the animated Seedship attacks");
    engine
        .apply_scenario_command(0, &pass())
        .expect("controller pass");
    let branch_batch = engine
        .apply_scenario_command(1, &pass())
        .expect("trigger resolves");
    let branch = find_resolution_choice(&branch_batch).expect("optional search branch");
    assert_eq!(branch.choice_kind(), ChoiceKind::ResolutionBranch);
    let search_batch = engine
        .apply_scenario_command(0, &select_branch(0))
        .expect("take the search branch");
    let search = find_resolution_choice(&search_batch).expect("private hand choice");
    assert_eq!(
        search.choice_kind(),
        ChoiceKind::ZoneSearch,
        "unexpected Sledge choice: {}",
        search.prompt_text
    );
    assert!(search.candidate_object_ids.contains(&hand_creature));
    engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![hand_creature]))
        .expect("put the creature card onto the battlefield");
    assert_eq!(engine.state.objects[&hand_creature].zone, Zone::Battlefield);
}

#[test]
fn issue_423_larval_scoutlander_may_sacrifice_a_land_to_search_two_basics_tapped() {
    let mut engine = issue_423_engine(423_040, "larval_scoutlander");
    let land = inject_permanent_on_battlefield(&mut engine, 0, "forest");
    let basic_a = inject_library_card(&mut engine, 0, "island");
    let basic_b = inject_library_card(&mut engine, 0, "mountain");
    let nonbasic = inject_library_card(&mut engine, 0, "grizzly_bears");
    let larval = deploy(&mut engine, "larval_scoutlander");
    answer_trigger_order_in_engine_order(&mut engine);
    engine
        .apply_scenario_command(0, &pass())
        .expect("controller pass");
    let branch_batch = engine
        .apply_scenario_command(1, &pass())
        .expect("trigger resolves");
    let branch = find_resolution_choice(&branch_batch).expect("optional ETB branch");
    assert_eq!(branch.choice_kind(), ChoiceKind::ResolutionBranch);
    assert_eq!((branch.min, branch.max), (0, 1));
    let payment_batch = engine
        .apply_scenario_command(0, &select_branch(0))
        .expect("select the sacrifice branch");
    let payment = find_resolution_choice(&payment_batch).expect("sacrifice payment choice");
    assert!(payment.candidate_object_ids.contains(&land));
    assert!(!payment.candidate_object_ids.contains(&larval));
    let search_batch = engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![land]))
        .expect("sacrifice the Forest");
    assert_eq!(engine.state.objects[&land].zone, Zone::Graveyard);
    let search = find_resolution_choice(&search_batch).expect("private basic-land search");
    assert_eq!(search.choice_kind(), ChoiceKind::LibrarySearch);
    assert_eq!((search.min, search.max), (0, 2));
    assert!(search.candidate_object_ids.contains(&basic_a));
    assert!(search.candidate_object_ids.contains(&basic_b));
    assert!(!search.candidate_object_ids.contains(&nonbasic));
    engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![basic_a, basic_b]))
        .expect("put two basic lands onto the battlefield tapped");
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .presentation
            .choice_kind,
        ChoiceKind::SimultaneousEntryOrder
    );
    assert_eq!(engine.state.objects[&basic_a].zone, Zone::Library);
    assert_eq!(engine.state.objects[&basic_b].zone, Zone::Library);
    answer_simultaneous_entry_order_in_engine_order(&mut engine);
    assert_eq!(engine.state.objects[&basic_a].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&basic_b].zone, Zone::Battlefield);
    assert!(engine.state.objects[&basic_a].tapped);
    assert!(engine.state.objects[&basic_b].tapped);

    set_charge(&mut engine, larval, 6);
    assert!(
        !engine.characteristics(larval).unwrap().has_type("Creature"),
        "Larval Scoutlander must stay unanimated below 7+"
    );
    set_charge(&mut engine, larval, 7);
    let animated = engine.characteristics(larval).unwrap();
    assert!(animated.has_type("Creature"));
    assert_eq!((animated.power, animated.toughness), (Some(3), Some(3)));
    assert!(animated.has_keyword(Keyword::Flying));
}

#[test]
fn issue_423_larval_scoutlander_may_pay_with_lander_and_searches_only_its_controllers_library() {
    let mut engine = issue_423_engine(423_041, "larval_scoutlander");
    let controller_land = inject_permanent_on_battlefield(&mut engine, 0, "forest");
    let controller_lander = inject_permanent_on_battlefield(&mut engine, 0, "lander");
    let opponent_land = inject_permanent_on_battlefield(&mut engine, 1, "forest");
    let opponent_lander = inject_permanent_on_battlefield(&mut engine, 1, "lander");
    let selected_basic = inject_library_card(&mut engine, 0, "island");
    let other_basic = inject_library_card(&mut engine, 0, "mountain");
    let nonbasic = inject_library_card(&mut engine, 0, "grizzly_bears");
    let opponent_basic = inject_library_card(&mut engine, 1, "island");
    let wan = inject_permanent_on_battlefield(&mut engine, 1, "wan_shi_tong,_librarian");
    let (larval, branch_batch) = resolve_larval_scoutlander_entry_trigger(&mut engine);

    let branch = find_resolution_choice(&branch_batch).expect("optional entry branch");
    assert_eq!(branch.choice_kind(), ChoiceKind::ResolutionBranch);
    assert_eq!(
        branch.deciding_player_id, 0,
        "the controller chooses whether to pay"
    );
    assert_eq!((branch.min, branch.max), (0, 1));

    let payment_batch = engine
        .apply_scenario_command(0, &select_branch(0))
        .expect("choose the optional sacrifice branch");
    let payment = find_resolution_choice(&payment_batch).expect("Land-or-Lander payment choice");
    assert_eq!(
        payment.deciding_player_id, 0,
        "the controller chooses the sacrifice"
    );
    assert!(payment.candidate_object_ids.contains(&controller_land));
    assert!(payment.candidate_object_ids.contains(&controller_lander));
    assert!(!payment.candidate_object_ids.contains(&opponent_land));
    assert!(!payment.candidate_object_ids.contains(&opponent_lander));
    assert!(!payment.candidate_object_ids.contains(&larval));

    let rejected =
        engine.apply_scenario_command(1, &submit_resolution_choice(vec![controller_lander]));
    assert!(
        rejected.is_err(),
        "an opponent cannot make the controller's payment choice"
    );
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .map(|pending| pending.deciding_player),
        Some(0),
        "the rejected command preserves the controller's pending offer"
    );

    let search_batch = engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![controller_lander]))
        .expect("sacrifice the Lander token");
    assert_eq!(
        engine.state.objects[&controller_lander].zone,
        Zone::Graveyard
    );
    assert_eq!(
        engine.state.objects[&controller_land].zone,
        Zone::Battlefield
    );
    assert_eq!(engine.state.objects[&opponent_land].zone, Zone::Battlefield);
    assert_eq!(
        engine.state.objects[&opponent_lander].zone,
        Zone::Battlefield
    );
    let search = find_resolution_choice(&search_batch).expect("private BasicLand search");
    assert_eq!(search.choice_kind(), ChoiceKind::LibrarySearch);
    assert_eq!(
        search.deciding_player_id, 0,
        "only the searching controller decides"
    );
    assert_eq!((search.min, search.max), (0, 2));
    assert!(search.candidate_object_ids.contains(&selected_basic));
    assert!(search.candidate_object_ids.contains(&other_basic));
    assert!(!search.candidate_object_ids.contains(&nonbasic));
    assert!(!search.candidate_object_ids.contains(&opponent_basic));
    assert_no_search_reveal(&search_batch);

    let completion = engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![selected_basic]))
        .expect("find one Basic Land");
    assert_eq!(
        engine.state.objects[&selected_basic].zone,
        Zone::Battlefield
    );
    assert!(engine.state.objects[&selected_basic].tapped);
    assert_eq!(engine.state.objects[&other_basic].zone, Zone::Library);
    assert_eq!(engine.state.objects[&nonbasic].zone, Zone::Library);
    assert_eq!(shuffle_log_count(&completion, 0), 1);
    assert_no_search_reveal(&completion);
    assert_one_search_observer(&engine, wan);
}

#[test]
fn issue_423_larval_scoutlander_can_choose_no_basic_land_after_sacrificing() {
    let mut engine = issue_423_engine(423_042, "larval_scoutlander");
    let land = inject_permanent_on_battlefield(&mut engine, 0, "forest");
    let selected_basic = inject_library_card(&mut engine, 0, "island");
    let nonbasic = inject_library_card(&mut engine, 0, "grizzly_bears");
    let wan = inject_permanent_on_battlefield(&mut engine, 1, "wan_shi_tong,_librarian");
    let (_, branch_batch) = resolve_larval_scoutlander_entry_trigger(&mut engine);
    assert!(find_resolution_choice(&branch_batch).is_some());

    let payment_batch = engine
        .apply_scenario_command(0, &select_branch(0))
        .expect("choose the optional sacrifice branch");
    let payment = find_resolution_choice(&payment_batch).expect("sacrifice payment choice");
    assert_eq!(payment.deciding_player_id, 0);
    let search_batch = engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![land]))
        .expect("sacrifice the Land");
    assert_eq!(engine.state.objects[&land].zone, Zone::Graveyard);
    let search = find_resolution_choice(&search_batch).expect("BasicLand search with matches");
    assert_eq!(search.choice_kind(), ChoiceKind::LibrarySearch);
    assert_eq!(search.deciding_player_id, 0);
    assert_eq!((search.min, search.max), (0, 2));
    assert!(search.candidate_object_ids.contains(&selected_basic));
    assert!(!search.candidate_object_ids.contains(&nonbasic));
    assert_no_search_reveal(&search_batch);

    let completion = engine
        .apply_scenario_command(0, &submit_resolution_choice(Vec::new()))
        .expect("choose zero cards");
    assert_eq!(engine.state.objects[&selected_basic].zone, Zone::Library);
    assert_eq!(engine.state.objects[&nonbasic].zone, Zone::Library);
    assert_eq!(shuffle_log_count(&completion, 0), 1);
    assert_no_search_reveal(&completion);
    assert_one_search_observer(&engine, wan);
}

#[test]
fn issue_423_larval_scoutlander_search_prompt_hides_eligible_basic_count() {
    const EXPECTED_PROMPT: &str = "P0: search Library for up to 2 matching card(s).";
    let mut expected_library_size = None;

    for eligible_count in 0..=2 {
        let mut engine = larval_search_privacy_engine(423_045);
        let land = inject_permanent_on_battlefield(&mut engine, 0, "forest");
        let wan = inject_permanent_on_battlefield(&mut engine, 1, "wan_shi_tong,_librarian");
        let (larval, branch_batch) = resolve_larval_scoutlander_entry_trigger(&mut engine);
        let branch = find_resolution_choice(&branch_batch).expect("optional entry branch");
        assert_eq!(branch.deciding_player_id, 0);
        let payment_batch = engine
            .apply_scenario_command(0, &select_branch(0))
            .expect("choose the optional sacrifice branch");
        let payment =
            find_resolution_choice(&payment_batch).expect("Land-or-Lander payment choice");
        assert!(payment.candidate_object_ids.contains(&land));
        assert!(!payment.candidate_object_ids.contains(&larval));

        let mut basics = Vec::new();
        let mut nonbasics = Vec::new();
        for position in 0..2 {
            let card_id = match (eligible_count, position) {
                (2, 0) => "island",
                (2, 1) => "mountain",
                (1, 0) => "island",
                _ => "grizzly_bears",
            };
            let object_id = inject_library_card(&mut engine, 0, card_id);
            if card_id != "grizzly_bears" {
                basics.push(object_id);
            } else {
                nonbasics.push(object_id);
            }
        }
        let own_nonbasic = inject_library_card(&mut engine, 0, "grizzly_bears");
        nonbasics.push(own_nonbasic);
        let opponent_basic = inject_library_card(&mut engine, 1, "island");
        let library_size = engine.state.players[0].library.len();
        assert!(library_size > 0, "the controller's Library is nonempty");
        if let Some(expected) = expected_library_size {
            assert_eq!(library_size, expected, "public Library size is constant");
        } else {
            expected_library_size = Some(library_size);
        }

        let search_batch = engine
            .apply_scenario_command(0, &submit_resolution_choice(vec![land]))
            .expect("sacrifice the Forest");
        assert_eq!(engine.state.objects[&land].zone, Zone::Graveyard);
        assert_eq!(engine.state.players[0].library.len(), library_size);
        let search = find_resolution_choice(&search_batch).expect("nonempty-library search offer");
        assert_eq!(search.choice_kind(), ChoiceKind::LibrarySearch);
        assert_eq!(search.deciding_player_id, 0);
        assert_eq!((search.min, search.max), (0, 2));
        assert_eq!(search.candidate_object_ids, basics);
        assert!(!search.candidate_object_ids.contains(&own_nonbasic));
        assert!(!search.candidate_object_ids.contains(&opponent_basic));
        assert_eq!(search.prompt_text.as_str(), EXPECTED_PROMPT);
        let public_search_logs = search_batch
            .events
            .iter()
            .filter_map(|event| match &event.ev {
                Some(Ev::Log(log)) if log.text.starts_with("P0: search ") => {
                    Some(log.text.as_str())
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(public_search_logs, vec![EXPECTED_PROMPT]);
        assert_no_search_reveal(&search_batch);

        let invalid_candidate = if eligible_count == 1 {
            opponent_basic
        } else {
            own_nonbasic
        };
        let pending_before = engine
            .state
            .pending_resolution
            .as_ref()
            .expect("the search offer remains pending");
        let pending_prompt_id = pending_before.resolution_prompt_id;
        let pending_decider = pending_before.deciding_player;
        let pending_candidates = pending_before.presentation.candidates.clone();
        let pending_bounds = (
            pending_before.presentation.min,
            pending_before.presentation.max,
        );
        let pending_prompt = pending_before.presentation.prompt.clone();
        let pending_choice_kind = pending_before.presentation.choice_kind;
        let rejected =
            engine.apply_scenario_command(0, &submit_resolution_choice(vec![invalid_candidate]));
        assert!(
            rejected.is_err(),
            "an ineligible card cannot satisfy the search"
        );
        let pending_after = engine
            .state
            .pending_resolution
            .as_ref()
            .expect("rejection preserves the pending search offer");
        assert_eq!(pending_after.resolution_prompt_id, pending_prompt_id);
        assert_eq!(pending_after.deciding_player, pending_decider);
        assert_eq!(pending_after.presentation.candidates, pending_candidates);
        assert_eq!(
            (
                pending_after.presentation.min,
                pending_after.presentation.max
            ),
            pending_bounds
        );
        assert_eq!(pending_after.presentation.prompt, pending_prompt);
        assert_eq!(pending_after.presentation.choice_kind, pending_choice_kind);

        let selected = engine
            .apply_scenario_command(0, &submit_resolution_choice(basics.clone()))
            .expect("choose zero, one, or two eligible Basic Lands");
        let mut completion_batches = vec![selected];
        if eligible_count == 2 {
            assert_eq!(
                engine
                    .state
                    .pending_resolution
                    .as_ref()
                    .expect("two entries require timestamp order")
                    .presentation
                    .choice_kind,
                ChoiceKind::SimultaneousEntryOrder
            );
            let entry_answers = answer_simultaneous_entry_order_in_engine_order(&mut engine);
            assert!(!entry_answers.is_empty());
            completion_batches.extend(entry_answers.into_iter().map(|(_, _, batch)| batch));
        }

        for basic in &basics {
            assert_eq!(engine.state.objects[basic].zone, Zone::Battlefield);
            assert!(engine.state.objects[basic].tapped);
        }
        for nonbasic in &nonbasics {
            assert_eq!(engine.state.objects[nonbasic].zone, Zone::Library);
        }
        assert_eq!(
            basics.len(),
            eligible_count,
            "the submitted selection matches the available eligible count"
        );
        assert_eq!(
            completion_batches
                .iter()
                .map(|batch| shuffle_log_count(batch, 0))
                .sum::<usize>(),
            1,
            "the completed search shuffles exactly once"
        );
        for batch in &completion_batches {
            assert_no_search_reveal(batch);
        }
        assert_one_search_observer(&engine, wan);
    }
}

#[test]
fn issue_423_larval_scoutlander_decline_and_no_eligible_cost_skip_the_search() {
    let mut declined = issue_423_engine(423_043, "larval_scoutlander");
    let land = inject_permanent_on_battlefield(&mut declined, 0, "forest");
    let (_, branch_batch) = resolve_larval_scoutlander_entry_trigger(&mut declined);
    let library_before_decline = declined.state.players[0].library.clone();
    let branch = find_resolution_choice(&branch_batch).expect("optional entry branch");
    assert_eq!(branch.deciding_player_id, 0);
    let decline_batch = declined
        .apply_scenario_command(
            0,
            &submit_resolution_decision(ResolutionChoiceDecision::Decline),
        )
        .expect("decline the optional sacrifice branch");
    assert_eq!(declined.state.objects[&land].zone, Zone::Battlefield);
    assert_eq!(declined.state.players[0].library, library_before_decline);
    assert!(declined.state.pending_resolution.is_none());
    assert_eq!(shuffle_log_count(&decline_batch, 0), 0);
    assert_no_search_reveal(&decline_batch);

    let mut no_cost = issue_423_engine(423_044, "larval_scoutlander");
    let library_before_no_cost = no_cost.state.players[0].library.clone();
    let (no_cost_larval, no_cost_batch) = resolve_larval_scoutlander_entry_trigger(&mut no_cost);
    let library_without_deployed_larval = library_before_no_cost
        .into_iter()
        .filter(|object_id| *object_id != no_cost_larval)
        .collect::<Vec<_>>();
    assert!(
        find_resolution_choice(&no_cost_batch).is_none(),
        "a Land-or-Lander branch without a controlled candidate is not offered"
    );
    assert!(no_cost.state.pending_resolution.is_none());
    assert!(no_cost.state.stack.is_empty());
    assert_eq!(
        no_cost.state.players[0].library,
        library_without_deployed_larval
    );
    assert_eq!(shuffle_log_count(&no_cost_batch, 0), 0);
    assert_no_search_reveal(&no_cost_batch);
}

#[test]
fn issue_423_planet_threshold_activations_require_twelve_charge_counters() {
    let mut engine = issue_423_engine(423_050, "kavaron,_memorial_world");
    let world = deploy(&mut engine, "kavaron,_memorial_world");
    // Kavaron enters tapped; untap it before paying its own {T} activation cost.
    engine.state.objects.get_mut(&world).unwrap().tapped = false;
    let sacrifice_land = inject_permanent_on_battlefield(&mut engine, 0, "forest");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            r: 1,
            c: 1,
            ..Default::default()
        },
    );

    // The 12+ activation is not legal below the threshold.
    assert!(
        engine
            .apply_scenario_command(
                0,
                &activate_with_generation(
                    &engine,
                    world,
                    3,
                    vec![permanent_cost_selection(2, sacrifice_land)],
                ),
            )
            .is_err(),
        "Kavaron's 12+ ability must stay off below the threshold"
    );

    set_charge(&mut engine, world, 12);
    let bears = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    engine
        .apply_scenario_command(
            0,
            &activate_with_generation(
                &engine,
                world,
                3,
                vec![permanent_cost_selection(2, sacrifice_land)],
            ),
        )
        .expect("activate Kavaron's 12+ ability");
    assert_eq!(engine.state.objects[&sacrifice_land].zone, Zone::Graveyard);
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        battlefield_token_oids(&engine, 0, "robot_c_2_2").len(),
        1,
        "Kavaron creates the Robot token"
    );
    assert_eq!(
        (
            engine.effective_power(bears),
            engine.effective_toughness(bears)
        ),
        (Some(3), Some(2)),
        "Kavaron pumps the team"
    );
    assert!(engine
        .characteristics(bears)
        .unwrap()
        .has_keyword(Keyword::Haste));
}

#[test]
fn issue_423_susur_secundi_draws_equal_to_the_sacrificed_creatures_power() {
    let mut engine = issue_423_engine(423_060, "susur_secundi,_void_altar");
    let altar = deploy(&mut engine, "susur_secundi,_void_altar");
    engine.state.objects.get_mut(&altar).unwrap().tapped = false;
    let victim = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let timestamp = engine.state.command_index;
    engine.state.objects.get_mut(&victim).unwrap().add_counters(
        CounterKind::PlusOnePlusOne,
        1,
        timestamp,
    );
    assert_eq!(engine.effective_power(victim), Some(3));
    give_mana(
        &mut engine,
        0,
        ManaGift {
            b: 1,
            c: 1,
            ..Default::default()
        },
    );
    set_charge(&mut engine, altar, 12);
    let life_before = engine.state.players[0].life;
    let hand_before = engine.state.players[0].hand.len();
    engine
        .apply_scenario_command(
            0,
            &activate_with_generation(&engine, altar, 3, vec![permanent_cost_selection(3, victim)]),
        )
        .expect("activate Susur Secundi's 12+ ability");
    assert_eq!(engine.state.objects[&victim].zone, Zone::Graveyard);
    assert_eq!(engine.state.players[0].life, life_before - 2);
    assert_eq!(
        engine.state.players[0].hand.len(),
        hand_before,
        "the draw happens only on resolution"
    );
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        engine.state.players[0].hand.len(),
        hand_before + 3,
        "the sacrificed creature's power sets the draw count"
    );
}
