//! Issue #424 — the eight retained triggered-modal Standard identities.
//!
//! The retained cohort's Oracle and rule citations were checked 2026-09-19 against the pinned
//! Scryfall snapshot. Lord Skitter's Butcher's pinned Oracle and current rules were checked
//! 2026-10-19. CR 603.3c/700.2b (mode announcement), CR 122 (counters), CR 400.7/701.13
//! (graveyard return and exile), CR 701.8a-c (destroy), CR 701.21a (sacrifice),
//! CR 701.26 (tap/untap), CR 701.22a (scry), CR 121.1 (draw), CR 608.2c (resolution order),
//! CR 611.2c/613.1f (until-end-of-turn layer-6 effects), and CR 702.111 (menace) govern the
//! exercised behavior.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_cards::{
    ContinuousEffectKind, CounterKind, EffectDuration, PermanentTypeFilter, TypeLineAddition,
    TypeLineReplacement,
};
use tricerules_core::{AffectedScope, ContinuousEffect, Zone};
use tricerules_proto::ruled::v1::{
    ruled_command::Cmd, ChoiceKind, ChooseTriggerTarget, ResolutionChoiceDecision, RuledCommand,
    RuledEventBatch, SelectedSpellMode, SubmitResolutionChoice, TargetRef, TargetRefKind,
};

fn modal_engine(seed: u64) -> GameEngine {
    // Explicit land-only decks keep the default grizzly_bears/library fixtures out of the
    // opening hand so injected fixture names stay unique.
    let decks = Some(vec![deck_with("mountain", &[]), deck_with("forest", &[])]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn fund(engine: &mut GameEngine, player: i32) {
    give_mana(
        engine,
        player,
        ManaGift {
            w: 4,
            u: 4,
            b: 4,
            r: 4,
            g: 4,
            c: 4,
        },
    );
}

fn prepare_creature(engine: &mut GameEngine, card_id: &str) {
    inject_card_into_hand(engine, 0, card_id);
    fund(engine, 0);
}

fn cast_and_announce(engine: &mut GameEngine, card_id: &str) {
    prepare_creature(engine, card_id);
    let slot = hand_index_for_card(engine, 0, card_id);
    engine
        .apply_scenario_command(0, &cast_spell(slot, Vec::new()))
        .expect("cast the modal creature");
    pass_both_players(engine);
}

fn choose_trigger_mode(mode_index: u32, targets: Vec<TargetRef>) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
            decline: false,
            selected_modes: vec![SelectedSpellMode {
                mode_index,
                targets,
            }],
            targets: Vec::new(),
        })),
    }
}

fn resolve_top_stack(engine: &mut GameEngine) -> RuledEventBatch {
    let first = engine.state.priority_player_id();
    let second = if first == engine.state.players[0].id {
        engine.state.players[1].id
    } else {
        engine.state.players[0].id
    };
    engine
        .apply_scenario_command(first, &pass())
        .expect("first pass");
    engine
        .apply_scenario_command(second, &pass())
        .expect("second pass resolves the triggered ability")
}

fn graveyard_target(object_id: u32) -> TargetRef {
    TargetRef {
        object_id,
        kind: TargetRefKind::Graveyard as i32,
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

fn mark_damaged(engine: &mut GameEngine, object_id: u32) {
    let generation = engine
        .state
        .zone_change_generation
        .get(&object_id)
        .copied()
        .unwrap_or(0);
    engine
        .state
        .turn_history
        .current
        .damaged_objects
        .push((object_id, generation));
}

fn add_artifact_type(engine: &mut GameEngine, object_id: u32) {
    engine.state.continuous_effects.push(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: None,
        affected: AffectedScope::Single(object_id),
        kind: ContinuousEffectKind::Layer4AddTypes(TypeLineAddition {
            card_types: vec![PermanentTypeFilter::Artifact],
            ..TypeLineAddition::default()
        }),
        condition: None,
        duration: EffectDuration::Indefinite,
        timestamp: engine.state.command_index,
    });
}

fn replace_type_line(
    engine: &mut GameEngine,
    object_id: u32,
    card_types: Vec<PermanentTypeFilter>,
) {
    engine.state.continuous_effects.push(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: None,
        affected: AffectedScope::Single(object_id),
        kind: ContinuousEffectKind::Layer4SetTypeLine(TypeLineReplacement {
            card_types,
            creature_types: Vec::new(),
            land_types: Vec::new(),
        }),
        condition: None,
        duration: EffectDuration::Indefinite,
        timestamp: engine.state.command_index,
    });
}

fn move_battlefield_card_to_hand_for_fixture(
    engine: &mut GameEngine,
    owner: usize,
    object_id: u32,
) {
    engine.state.players[owner]
        .battlefield
        .retain(|candidate| *candidate != object_id);
    engine.state.players[owner].hand.push(object_id);
    engine.state.objects.get_mut(&object_id).unwrap().zone = Zone::Hand;
    *engine
        .state
        .zone_change_generation
        .entry(object_id)
        .or_default() += 1;
}

fn blink_for_fixture(engine: &mut GameEngine, owner: usize, object_id: u32) {
    engine.state.players[owner]
        .battlefield
        .retain(|candidate| *candidate != object_id);
    engine.state.players[owner].graveyard.push(object_id);
    engine.state.objects.get_mut(&object_id).unwrap().zone = Zone::Graveyard;
    *engine
        .state
        .zone_change_generation
        .entry(object_id)
        .or_default() += 1;

    engine.state.players[owner]
        .graveyard
        .retain(|candidate| *candidate != object_id);
    engine.state.players[owner].battlefield.push(object_id);
    engine.state.objects.get_mut(&object_id).unwrap().zone = Zone::Battlefield;
    *engine
        .state
        .zone_change_generation
        .entry(object_id)
        .or_default() += 1;
}

fn choose_myr_retriever_graveyard_target(object_id: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
            targets: vec![TargetRef {
                object_id,
                group_index: 0,
                kind: TargetRefKind::Graveyard as i32,
                ..Default::default()
            }],
            ..Default::default()
        })),
    }
}

fn counters(engine: &GameEngine, object_id: u32) -> u32 {
    engine.state.objects[&object_id].counter_count(CounterKind::PlusOnePlusOne)
}

#[test]
fn issue_424_daily_bugle_reporters_counts_two_creatures_or_returns_a_small_creature() {
    let mut counters_engine = modal_engine(424_001);
    let ours = inject_creature_on_battlefield(&mut counters_engine, 0, "grizzly_bears");
    let theirs = inject_creature_on_battlefield(&mut counters_engine, 1, "storm_crow");
    cast_and_announce(&mut counters_engine, "daily_bugle_reporters");
    let mut targets = target_object(ours);
    targets.extend(target_object(theirs));
    counters_engine
        .apply_scenario_command(0, &choose_trigger_mode(0, targets))
        .expect("target both creatures");
    pass_both_players(&mut counters_engine);
    assert_eq!(counters(&counters_engine, ours), 1);
    assert_eq!(counters(&counters_engine, theirs), 1);

    let mut return_engine = modal_engine(424_002);
    let small = inject_graveyard_card(&mut return_engine, 0, "grizzly_bears");
    let large = inject_graveyard_card(&mut return_engine, 0, "hill_giant");
    cast_and_announce(&mut return_engine, "daily_bugle_reporters");
    assert!(
        return_engine
            .apply_scenario_command(0, &choose_trigger_mode(1, vec![graveyard_target(large)]),)
            .is_err(),
        "mana value 4 is not legal for the two-or-less return"
    );
    return_engine
        .apply_scenario_command(0, &choose_trigger_mode(1, vec![graveyard_target(small)]))
        .expect("return the small creature card");
    pass_both_players(&mut return_engine);
    assert_eq!(return_engine.state.objects[&small].zone, Zone::Hand);
    assert_eq!(return_engine.state.objects[&large].zone, Zone::Graveyard);
}

#[test]
fn issue_424_damage_control_crew_returns_a_large_card_or_exiles_an_artifact_or_enchantment() {
    let mut repair_engine = modal_engine(424_010);
    let small = inject_graveyard_card(&mut repair_engine, 0, "grizzly_bears");
    let large = inject_graveyard_card(&mut repair_engine, 0, "hill_giant");
    cast_and_announce(&mut repair_engine, "damage_control_crew");
    assert!(
        repair_engine
            .apply_scenario_command(0, &choose_trigger_mode(0, vec![graveyard_target(small)]),)
            .is_err(),
        "mana value 2 is not legal for the four-or-greater return"
    );
    repair_engine
        .apply_scenario_command(0, &choose_trigger_mode(0, vec![graveyard_target(large)]))
        .expect("return the large card");
    pass_both_players(&mut repair_engine);
    assert_eq!(repair_engine.state.objects[&large].zone, Zone::Hand);
    assert_eq!(repair_engine.state.objects[&small].zone, Zone::Graveyard);

    let mut impound_engine = modal_engine(424_011);
    let sword = inject_permanent_on_battlefield(&mut impound_engine, 1, "short_sword");
    let anthem = inject_permanent_on_battlefield(&mut impound_engine, 1, "glorious_anthem");
    let bear = inject_creature_on_battlefield(&mut impound_engine, 1, "grizzly_bears");
    cast_and_announce(&mut impound_engine, "damage_control_crew");
    assert!(
        impound_engine
            .apply_scenario_command(0, &choose_trigger_mode(1, target_object(bear)))
            .is_err(),
        "a creature is neither an artifact nor an enchantment"
    );
    impound_engine
        .apply_scenario_command(0, &choose_trigger_mode(1, target_object(sword)))
        .expect("exile the artifact");
    pass_both_players(&mut impound_engine);
    // A second copy of the same identity may exile the enchantment during the same turn.
    cast_and_announce(&mut impound_engine, "damage_control_crew");
    impound_engine
        .apply_scenario_command(0, &choose_trigger_mode(1, target_object(anthem)))
        .expect("exile the enchantment");
    pass_both_players(&mut impound_engine);
    assert_eq!(impound_engine.state.objects[&sword].zone, Zone::Exile);
    assert_eq!(impound_engine.state.objects[&anthem].zone, Zone::Exile);
    assert_eq!(impound_engine.state.objects[&bear].zone, Zone::Battlefield);
}

#[test]
fn issue_424_gearbane_orangutan_destroys_an_artifact_or_mandatorily_sacrifices_one() {
    let mut destroy_engine = modal_engine(424_020);
    let sword = inject_permanent_on_battlefield(&mut destroy_engine, 1, "short_sword");
    cast_and_announce(&mut destroy_engine, "gearbane_orangutan");
    destroy_engine
        .apply_scenario_command(0, &choose_trigger_mode(0, target_object(sword)))
        .expect("destroy the artifact");
    pass_both_players(&mut destroy_engine);
    assert_eq!(destroy_engine.state.objects[&sword].zone, Zone::Graveyard);

    let mut sacrifice_engine = modal_engine(424_021);
    let artifact = inject_permanent_on_battlefield(&mut sacrifice_engine, 0, "short_sword");
    let opponent_artifact =
        inject_permanent_on_battlefield(&mut sacrifice_engine, 1, "short_sword");
    cast_and_announce(&mut sacrifice_engine, "gearbane_orangutan");
    let source = battlefield_object_for_card(&sacrifice_engine, 0, "gearbane_orangutan");
    sacrifice_engine
        .apply_scenario_command(0, &choose_trigger_mode(1, Vec::new()))
        .expect("announce the sacrifice mode");
    let branch_batch = resolve_top_stack(&mut sacrifice_engine);
    let branch = find_resolution_choice(&branch_batch).expect("mandatory branch selection");
    assert_eq!(branch.choice_kind(), ChoiceKind::ResolutionBranch);
    assert_eq!((branch.min, branch.max), (1, 1));
    let mut decline_branch = select_branch(0);
    if let Some(Cmd::SubmitResolutionChoice(answer)) = decline_branch.cmd.as_mut() {
        answer.decision = ResolutionChoiceDecision::Decline as i32;
    }
    assert!(
        sacrifice_engine
            .apply_scenario_command(0, &decline_branch)
            .is_err(),
        "the payable mandatory branch cannot be declined"
    );
    let payment_batch = sacrifice_engine
        .apply_scenario_command(0, &select_branch(0))
        .expect("select the mandatory sacrifice branch");
    let payment = find_resolution_choice(&payment_batch).expect("mandatory sacrifice payment");
    assert_eq!((payment.min, payment.max), (1, 1));
    assert_eq!(payment.prompt_text, "Choose a permanent to sacrifice.");
    assert!(payment.candidate_object_ids.contains(&artifact));
    assert!(!payment.candidate_object_ids.contains(&opponent_artifact));
    assert!(
        !payment.candidate_object_ids.contains(&source),
        "the Ape is not an artifact"
    );
    let prompt_id = payment.resolution_prompt_id;
    let state_before_invalid_answers = format!("{:#?}", sacrifice_engine.state);
    for (label, invalid_selection) in [
        ("foreign artifact", vec![opponent_artifact]),
        ("empty payment", Vec::new()),
        ("duplicate artifact", vec![artifact, artifact]),
    ] {
        let result = sacrifice_engine
            .apply_scenario_command(0, &submit_resolution_choice(invalid_selection.clone()));
        assert!(
            result.is_err(),
            "{label} selection {invalid_selection:?} must be rejected; accepted result: {result:?}"
        );
        assert_eq!(
            format!("{:#?}", sacrifice_engine.state),
            state_before_invalid_answers,
            "{label} rejection preserves the full game state and pending prompt"
        );
        let pending = sacrifice_engine
            .state
            .pending_resolution
            .as_ref()
            .expect("invalid payment answers preserve the offered choice");
        assert_eq!(pending.resolution_prompt_id.get(), prompt_id);
        assert_eq!((pending.presentation.min, pending.presentation.max), (1, 1));
        assert_eq!(
            pending.presentation.prompt,
            "Choose a permanent to sacrifice."
        );
        assert!(pending.presentation.candidates.contains(&artifact));
    }
    let explicit_decline = RuledCommand {
        cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
            resolution_prompt_id: prompt_id,
            decision: ResolutionChoiceDecision::Decline as i32,
            ..Default::default()
        })),
    };
    let decline_result = sacrifice_engine.apply_scenario_command(0, &explicit_decline);
    assert!(
        decline_result.is_err(),
        "an explicit decline cannot answer the mandatory sacrifice payment: {decline_result:?}"
    );
    assert_eq!(
        format!("{:#?}", sacrifice_engine.state),
        state_before_invalid_answers,
        "decline rejection preserves the full game state and pending prompt"
    );
    sacrifice_engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![artifact]))
        .expect("sacrifice the artifact");
    assert_eq!(
        sacrifice_engine.state.objects[&artifact].zone,
        Zone::Graveyard
    );
    assert_eq!(
        counters(&sacrifice_engine, source),
        2,
        "if you do, two counters land on the source"
    );
    assert!(
        sacrifice_engine
            .apply_scenario_command(0, &submit_resolution_choice(vec![artifact]))
            .is_err(),
        "the completed payment cannot be replayed"
    );
    assert_eq!(counters(&sacrifice_engine, source), 2);

    // With no artifact available the mandatory branch is skipped and no counters are placed.
    let mut empty_engine = modal_engine(424_022);
    cast_and_announce(&mut empty_engine, "gearbane_orangutan");
    let source = battlefield_object_for_card(&empty_engine, 0, "gearbane_orangutan");
    empty_engine
        .apply_scenario_command(0, &choose_trigger_mode(1, Vec::new()))
        .expect("announce the sacrifice mode with nothing to sacrifice");
    pass_both_players(&mut empty_engine);
    assert_eq!(counters(&empty_engine, source), 0);
}

#[test]
fn issue_424_gearbane_destroy_program_rechecks_optional_targets_at_resolution() {
    let mut zero_target = modal_engine(424_023);
    let sword = inject_permanent_on_battlefield(&mut zero_target, 1, "short_sword");
    cast_and_announce(&mut zero_target, "gearbane_orangutan");
    zero_target
        .apply_scenario_command(0, &choose_trigger_mode(0, Vec::new()))
        .expect("choose zero targets for the optional artifact mode");
    pass_both_players(&mut zero_target);
    assert_eq!(zero_target.state.objects[&sword].zone, Zone::Battlefield);

    let mut changed_type = modal_engine(424_024);
    let sword = inject_permanent_on_battlefield(&mut changed_type, 1, "short_sword");
    cast_and_announce(&mut changed_type, "gearbane_orangutan");
    changed_type
        .apply_scenario_command(0, &choose_trigger_mode(0, target_object(sword)))
        .expect("the target is an artifact when mode 01 is announced");
    replace_type_line(
        &mut changed_type,
        sword,
        vec![PermanentTypeFilter::Enchantment],
    );
    assert!(!changed_type
        .characteristics(sword)
        .expect("the current target characteristics")
        .has_type("Artifact"));
    pass_both_players(&mut changed_type);
    assert_eq!(changed_type.state.objects[&sword].zone, Zone::Battlefield);

    let mut stale = modal_engine(424_025);
    let sword = inject_permanent_on_battlefield(&mut stale, 1, "short_sword");
    cast_and_announce(&mut stale, "gearbane_orangutan");
    stale
        .apply_scenario_command(0, &choose_trigger_mode(0, target_object(sword)))
        .expect("the target is legal when mode 01 is announced");
    move_battlefield_card_to_hand_for_fixture(&mut stale, 1, sword);
    pass_both_players(&mut stale);
    assert_eq!(stale.state.objects[&sword].zone, Zone::Hand);
    assert!(stale.state.pending_resolution.is_none());
}

#[test]
fn issue_424_gearbane_destroy_program_keeps_indestructible_and_observer_continuations() {
    let mut indestructible_engine = modal_engine(424_026);
    let myr = inject_creature_on_battlefield(&mut indestructible_engine, 1, "darksteel_myr");
    cast_and_announce(&mut indestructible_engine, "gearbane_orangutan");
    indestructible_engine
        .apply_scenario_command(0, &choose_trigger_mode(0, target_object(myr)))
        .expect("Darksteel Myr is an artifact target");
    pass_both_players(&mut indestructible_engine);
    assert_eq!(
        indestructible_engine.state.objects[&myr].zone,
        Zone::Battlefield
    );
    assert!(indestructible_engine.state.pending_resolution.is_none());

    let mut observer_engine = modal_engine(424_027);
    let recoverable = inject_graveyard_card(&mut observer_engine, 1, "short_sword");
    let retriever = inject_creature_on_battlefield(&mut observer_engine, 1, "myr_retriever");
    cast_and_announce(&mut observer_engine, "gearbane_orangutan");
    observer_engine
        .apply_scenario_command(0, &choose_trigger_mode(0, target_object(retriever)))
        .expect("destroy the artifact creature with a dies observer");
    pass_both_players(&mut observer_engine);
    assert_eq!(
        observer_engine.state.objects[&retriever].zone,
        Zone::Graveyard
    );
    assert!(observer_engine.state.pending_resolution.is_none());
    assert_eq!(observer_engine.state.pending_triggers.len(), 1);
    let legal = observer_engine.initial_response_batch();
    let ability_key = u64::from(retriever) << 32;
    assert!(
        legal.legal_by_player[&1].valid_targets_by_ability[&ability_key].groups[0]
            .valid_graveyard_ids
            .contains(&recoverable)
    );
    observer_engine
        .apply_scenario_command(1, &choose_myr_retriever_graveyard_target(recoverable))
        .expect("the dies observer selects the other artifact card");
    resolve_entire_stack_two_player(&mut observer_engine);
    assert_eq!(observer_engine.state.objects[&recoverable].zone, Zone::Hand);
}

#[test]
fn issue_424_gearbane_program_source_generation_controls_counter_tail() {
    let mut blinked_source = modal_engine(424_028);
    let artifact = inject_permanent_on_battlefield(&mut blinked_source, 0, "short_sword");
    cast_and_announce(&mut blinked_source, "gearbane_orangutan");
    let source = battlefield_object_for_card(&blinked_source, 0, "gearbane_orangutan");
    let old_generation = blinked_source
        .state
        .zone_change_generation
        .get(&source)
        .copied()
        .unwrap_or(0);
    blinked_source
        .apply_scenario_command(0, &choose_trigger_mode(1, Vec::new()))
        .expect("choose the mandatory sacrifice mode");
    blink_for_fixture(&mut blinked_source, 0, source);
    let branch_batch = resolve_top_stack(&mut blinked_source);
    find_resolution_choice(&branch_batch).expect("the mandatory branch choice");
    let payment_batch = blinked_source
        .apply_scenario_command(0, &select_branch(0))
        .expect("select the branch after the source changed incarnation");
    let payment = find_resolution_choice(&payment_batch).expect("the own artifact payment");
    assert!(payment.candidate_object_ids.contains(&artifact));
    blinked_source
        .apply_scenario_command(0, &submit_resolution_choice(vec![artifact]))
        .expect("pay while the old trigger source is absent");
    assert_eq!(
        blinked_source.state.objects[&source].zone,
        Zone::Battlefield
    );
    assert!(
        blinked_source
            .state
            .zone_change_generation
            .get(&source)
            .copied()
            .unwrap_or(0)
            >= old_generation + 2
    );
    assert_eq!(counters(&blinked_source, source), 0);

    let mut self_sacrifice = modal_engine(424_029);
    cast_and_announce(&mut self_sacrifice, "gearbane_orangutan");
    let source = battlefield_object_for_card(&self_sacrifice, 0, "gearbane_orangutan");
    add_artifact_type(&mut self_sacrifice, source);
    assert!(self_sacrifice
        .characteristics(source)
        .expect("the current source characteristics")
        .has_type("Artifact"));
    self_sacrifice
        .apply_scenario_command(0, &choose_trigger_mode(1, Vec::new()))
        .expect("choose the mandatory sacrifice mode");
    let branch_batch = resolve_top_stack(&mut self_sacrifice);
    find_resolution_choice(&branch_batch).expect("the mandatory branch choice");
    let payment_batch = self_sacrifice
        .apply_scenario_command(0, &select_branch(0))
        .expect("select the legal source-sacrifice branch");
    let payment = find_resolution_choice(&payment_batch).expect("the artifact payment");
    assert_eq!(payment.candidate_object_ids, vec![source]);
    self_sacrifice
        .apply_scenario_command(0, &submit_resolution_choice(vec![source]))
        .expect("sacrifice the Artifact source");
    assert_eq!(self_sacrifice.state.objects[&source].zone, Zone::Graveyard);
    assert_eq!(counters(&self_sacrifice, source), 0);
    assert!(self_sacrifice.state.pending_resolution.is_none());
}

#[test]
fn issue_424_tap_and_untap_pairs_reject_lands_and_toggle_their_targets() {
    let mut ant_engine = modal_engine(424_030);
    let bear = inject_creature_on_battlefield(&mut ant_engine, 1, "grizzly_bears");
    let mountain = inject_permanent_on_battlefield(&mut ant_engine, 1, "mountain");
    cast_and_announce(&mut ant_engine, "giant-sized_flying_ant");
    assert!(
        ant_engine
            .apply_scenario_command(0, &choose_trigger_mode(0, target_object(mountain)))
            .is_err(),
        "a land is not a nonland permanent"
    );
    ant_engine
        .apply_scenario_command(0, &choose_trigger_mode(0, target_object(bear)))
        .expect("tap the nonland permanent");
    pass_both_players(&mut ant_engine);
    assert!(ant_engine.state.objects[&bear].tapped);

    let mut untap_engine = modal_engine(424_031);
    let own = inject_creature_on_battlefield(&mut untap_engine, 0, "grizzly_bears");
    untap_engine.state.objects.get_mut(&own).unwrap().tapped = true;
    cast_and_announce(&mut untap_engine, "giant-sized_flying_ant");
    untap_engine
        .apply_scenario_command(0, &choose_trigger_mode(1, target_object(own)))
        .expect("untap the nonland permanent");
    pass_both_players(&mut untap_engine);
    assert!(!untap_engine.state.objects[&own].tapped);

    let mut mite_engine = modal_engine(424_032);
    let victim = inject_creature_on_battlefield(&mut mite_engine, 1, "grizzly_bears");
    let sword = inject_permanent_on_battlefield(&mut mite_engine, 1, "short_sword");
    cast_and_announce(&mut mite_engine, "glamermite");
    assert!(
        mite_engine
            .apply_scenario_command(0, &choose_trigger_mode(0, target_object(sword)))
            .is_err(),
        "the creature-only tap mode rejects an artifact"
    );
    mite_engine
        .apply_scenario_command(0, &choose_trigger_mode(0, target_object(victim)))
        .expect("tap the creature");
    pass_both_players(&mut mite_engine);
    assert!(mite_engine.state.objects[&victim].tapped);
}

#[test]
fn issue_424_glamermite_untaps_a_creature_and_oltec_scries_three() {
    let mut mite_engine = modal_engine(424_040);
    let own = inject_creature_on_battlefield(&mut mite_engine, 0, "grizzly_bears");
    mite_engine.state.objects.get_mut(&own).unwrap().tapped = true;
    cast_and_announce(&mut mite_engine, "glamermite");
    mite_engine
        .apply_scenario_command(0, &choose_trigger_mode(1, target_object(own)))
        .expect("untap the creature");
    pass_both_players(&mut mite_engine);
    assert!(!mite_engine.state.objects[&own].tapped);

    let mut oltec_engine = modal_engine(424_041);
    let top = seat_top_cards(
        &mut oltec_engine,
        0,
        &["grizzly_bears", "storm_crow", "island"],
    );
    cast_and_announce(&mut oltec_engine, "oltec_archaeologists");
    oltec_engine
        .apply_scenario_command(0, &choose_trigger_mode(1, Vec::new()))
        .expect("announce scry 3");
    let batch = resolve_top_stack(&mut oltec_engine);
    let choice = find_resolution_choice(&batch).expect("scry choice");
    assert_eq!(choice.choice_kind(), ChoiceKind::LibraryTop);
    assert_eq!((choice.min, choice.max), (0, 3));
    assert_eq!(choice.candidate_object_ids, top);
    oltec_engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![top[0], top[1]]))
        .expect("bottom two and keep one on top");
    assert!(oltec_engine.state.pending_resolution.is_none());
    for object_id in [top[0], top[1], top[2]] {
        assert_eq!(
            oltec_engine.state.objects[&object_id].zone,
            Zone::Library,
            "CR 701.18: scried cards never leave the library"
        );
    }
    let library = &oltec_engine.state.players[0].library;
    assert_eq!(
        library.front().copied(),
        Some(top[2]),
        "one card stays on top"
    );
    assert_eq!(library.back().copied(), Some(top[1]));
    assert_eq!(
        library.get(library.len() - 2).copied(),
        Some(top[0]),
        "the bottom pile keeps the submitted order"
    );
}

fn seat_top_cards(engine: &mut GameEngine, player: usize, card_ids: &[&str]) -> Vec<u32> {
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

#[test]
fn issue_424_oltec_returns_an_artifact_card_and_qutrub_exiles_two_from_one_graveyard() {
    let mut oltec_engine = modal_engine(424_050);
    let sword = inject_graveyard_card(&mut oltec_engine, 0, "short_sword");
    let bear = inject_graveyard_card(&mut oltec_engine, 0, "grizzly_bears");
    cast_and_announce(&mut oltec_engine, "oltec_archaeologists");
    assert!(
        oltec_engine
            .apply_scenario_command(0, &choose_trigger_mode(0, vec![graveyard_target(bear)]))
            .is_err(),
        "a creature card is not an artifact card"
    );
    oltec_engine
        .apply_scenario_command(0, &choose_trigger_mode(0, vec![graveyard_target(sword)]))
        .expect("return the artifact card");
    pass_both_players(&mut oltec_engine);
    assert_eq!(oltec_engine.state.objects[&sword].zone, Zone::Hand);
    assert_eq!(oltec_engine.state.objects[&bear].zone, Zone::Graveyard);

    let mut qutrub_engine = modal_engine(424_051);
    let ours = inject_graveyard_card(&mut qutrub_engine, 0, "grizzly_bears");
    let ours_second = inject_graveyard_card(&mut qutrub_engine, 0, "hill_giant");
    let theirs = inject_graveyard_card(&mut qutrub_engine, 1, "storm_crow");
    cast_and_announce(&mut qutrub_engine, "qutrub_forayer");
    assert!(
        qutrub_engine
            .apply_scenario_command(
                0,
                &choose_trigger_mode(1, vec![graveyard_target(ours), graveyard_target(theirs)]),
            )
            .is_err(),
        "the two chosen cards must share one graveyard"
    );
    qutrub_engine
        .apply_scenario_command(
            0,
            &choose_trigger_mode(
                1,
                vec![graveyard_target(ours), graveyard_target(ours_second)],
            ),
        )
        .expect("exile up to two cards from one graveyard");
    pass_both_players(&mut qutrub_engine);
    assert_eq!(qutrub_engine.state.objects[&ours].zone, Zone::Exile);
    assert_eq!(qutrub_engine.state.objects[&ours_second].zone, Zone::Exile);
    assert_eq!(qutrub_engine.state.objects[&theirs].zone, Zone::Graveyard);
}

#[test]
fn issue_424_qutrub_destroys_only_a_creature_damaged_this_turn() {
    let mut engine = modal_engine(424_060);
    let marked = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let undamaged = inject_creature_on_battlefield(&mut engine, 1, "storm_crow");
    let own = inject_creature_on_battlefield(&mut engine, 0, "hill_giant");
    mark_damaged(&mut engine, marked);
    cast_and_announce(&mut engine, "qutrub_forayer");
    for illegal in [undamaged, own] {
        assert!(
            engine
                .apply_scenario_command(0, &choose_trigger_mode(0, target_object(illegal)))
                .is_err(),
            "an undamaged or own creature is not a legal target"
        );
    }
    engine
        .apply_scenario_command(0, &choose_trigger_mode(0, target_object(marked)))
        .expect("destroy the damage-marked creature");
    pass_both_players(&mut engine);
    assert_eq!(engine.state.objects[&marked].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&undamaged].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&own].zone, Zone::Battlefield);
}

#[test]
fn issue_424_butcher_offers_only_another_creature_and_receipts_scry_and_draw() {
    let mut engine = modal_engine(424_080);
    let other = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let opponent = inject_creature_on_battlefield(&mut engine, 1, "storm_crow");
    let [scry_bottom, scry_top] = seat_top_cards(&mut engine, 0, &["forest", "island"])
        .try_into()
        .expect("two known cards are placed on top");
    cast_and_announce(&mut engine, "lord_skitters_butcher");
    let source = battlefield_object_for_card(&engine, 0, "lord_skitters_butcher");
    engine
        .apply_scenario_command(0, &choose_trigger_mode(1, Vec::new()))
        .expect("announce the sacrifice mode");
    let branch_batch = resolve_top_stack(&mut engine);
    let branch = find_resolution_choice(&branch_batch).expect("optional branch selection");
    assert_eq!(branch.choice_kind(), ChoiceKind::ResolutionBranch);
    assert_eq!((branch.min, branch.max), (0, 1));
    let payment_batch = engine
        .apply_scenario_command(0, &select_branch(0))
        .expect("select the optional sacrifice branch");
    let payment = find_resolution_choice(&payment_batch).expect("optional sacrifice payment");
    assert!(payment.candidate_object_ids.contains(&other));
    assert!(
        !payment.candidate_object_ids.contains(&source),
        "another creature excludes the entering Butcher"
    );
    assert!(
        !payment.candidate_object_ids.contains(&opponent),
        "the sacrifice is limited to creatures the chooser controls"
    );
    let library_before = engine.state.players[0].library.len();
    let hand_before = engine.state.players[0].hand.len();
    let scry_batch = engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![other]))
        .expect("sacrifice the other creature");
    assert_eq!(engine.state.objects[&other].zone, Zone::Graveyard);
    let scry = find_resolution_choice(&scry_batch).expect("scry 2 after the sacrifice");
    assert_eq!(scry.choice_kind(), ChoiceKind::LibraryTop);
    assert_eq!((scry.min, scry.max), (0, 2));
    assert_eq!(scry.candidate_object_ids, [scry_bottom, scry_top]);
    engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![scry_bottom]))
        .expect("bottom one of the two scried cards");
    assert_eq!(
        engine.state.players[0].library.len(),
        library_before - 1,
        "the receipt's draw takes the remaining scried top card"
    );
    assert_eq!(engine.state.players[0].hand.len(), hand_before + 1);
    assert_eq!(engine.state.objects[&scry_bottom].zone, Zone::Library);
    assert_eq!(engine.state.objects[&scry_top].zone, Zone::Hand);
    assert_eq!(
        engine.state.players[0]
            .hand
            .iter()
            .filter(|object_id| **object_id == scry_top)
            .count(),
        1,
        "the known surviving top card is drawn exactly once"
    );

    // Declining the optional sacrifice skips the whole "if you do" receipt.
    let mut decline_engine = modal_engine(424_081);
    let bystander = inject_creature_on_battlefield(&mut decline_engine, 0, "grizzly_bears");
    cast_and_announce(&mut decline_engine, "lord_skitters_butcher");
    decline_engine
        .apply_scenario_command(0, &choose_trigger_mode(1, Vec::new()))
        .expect("announce the sacrifice mode");
    let branch_batch = resolve_top_stack(&mut decline_engine);
    find_resolution_choice(&branch_batch).expect("optional branch selection");
    let hand_before = decline_engine.state.players[0].hand.len();
    decline_engine
        .apply_scenario_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
                    resolution_prompt_id: 0,
                    decision: ResolutionChoiceDecision::Decline as i32,
                    ..Default::default()
                })),
            },
        )
        .expect("decline the optional sacrifice");
    assert_eq!(
        decline_engine.state.objects[&bystander].zone,
        Zone::Battlefield
    );
    assert_eq!(decline_engine.state.players[0].hand.len(), hand_before);
    assert!(decline_engine.state.pending_resolution.is_none());

    // An empty payment answer is the same explicit refusal for this optional branch.
    let mut empty_payment_engine = modal_engine(424_084);
    let empty_payment_bystander =
        inject_creature_on_battlefield(&mut empty_payment_engine, 0, "grizzly_bears");
    cast_and_announce(&mut empty_payment_engine, "lord_skitters_butcher");
    empty_payment_engine
        .apply_scenario_command(0, &choose_trigger_mode(1, Vec::new()))
        .expect("announce the sacrifice mode");
    let branch_batch = resolve_top_stack(&mut empty_payment_engine);
    find_resolution_choice(&branch_batch).expect("optional branch selection");
    let payment_batch = empty_payment_engine
        .apply_scenario_command(0, &select_branch(0))
        .expect("select the optional sacrifice branch");
    let payment = find_resolution_choice(&payment_batch).expect("optional sacrifice payment");
    assert!(payment
        .candidate_object_ids
        .contains(&empty_payment_bystander));
    let hand_before = empty_payment_engine.state.players[0].hand.len();
    empty_payment_engine
        .apply_scenario_command(0, &submit_resolution_choice(Vec::new()))
        .expect("decline the optional sacrifice with an empty payment answer");
    assert_eq!(
        empty_payment_engine.state.objects[&empty_payment_bystander].zone,
        Zone::Battlefield
    );
    assert_eq!(
        empty_payment_engine.state.players[0].hand.len(),
        hand_before
    );
    assert!(empty_payment_engine.state.pending_resolution.is_none());

    // The source is the only creature, so excluding it leaves no branch to offer.
    let mut no_candidate_engine = modal_engine(424_085);
    cast_and_announce(&mut no_candidate_engine, "lord_skitters_butcher");
    let source = battlefield_object_for_card(&no_candidate_engine, 0, "lord_skitters_butcher");
    no_candidate_engine
        .apply_scenario_command(0, &choose_trigger_mode(1, Vec::new()))
        .expect("announce the sacrifice mode");
    let resolution = resolve_top_stack(&mut no_candidate_engine);
    assert!(find_resolution_choice(&resolution).is_none());
    assert_eq!(
        no_candidate_engine.state.objects[&source].zone,
        Zone::Battlefield
    );
    assert!(no_candidate_engine.state.pending_resolution.is_none());
}

#[test]
fn issue_424_butcher_rat_token_and_team_menace_resolve() {
    let mut rat_engine = modal_engine(424_082);
    cast_and_announce(&mut rat_engine, "lord_skitters_butcher");
    rat_engine
        .apply_scenario_command(0, &choose_trigger_mode(0, Vec::new()))
        .expect("announce the Rat token mode");
    pass_both_players(&mut rat_engine);
    let token = rat_engine.state.players[0]
        .battlefield
        .iter()
        .copied()
        .find(|oid| rat_engine.state.objects[oid].card_id == "rat_b_1_1_cant_block")
        .expect("the Rat token entered");
    assert!(rat_engine.state.objects[&token].is_token());

    let mut menace_engine = modal_engine(424_083);
    let bear = inject_creature_on_battlefield(&mut menace_engine, 0, "grizzly_bears");
    let opponent_bear = inject_creature_on_battlefield(&mut menace_engine, 1, "storm_crow");
    cast_and_announce(&mut menace_engine, "lord_skitters_butcher");
    let source = battlefield_object_for_card(&menace_engine, 0, "lord_skitters_butcher");
    menace_engine
        .apply_scenario_command(0, &choose_trigger_mode(2, Vec::new()))
        .expect("announce the team menace mode");
    pass_both_players(&mut menace_engine);
    for current_friendly in [source, bear] {
        assert!(
            menace_engine.effective_has_keyword(current_friendly, tricerules_cards::Keyword::Menace),
            "the one-shot grant covers each creature you control at resolution, including its source"
        );
    }
    assert!(
        !menace_engine.effective_has_keyword(opponent_bear, tricerules_cards::Keyword::Menace),
        "the team grant excludes opposing creatures"
    );
    let later_friendly = inject_creature_on_battlefield(&mut menace_engine, 0, "hill_giant");
    assert!(
        !menace_engine.effective_has_keyword(later_friendly, tricerules_cards::Keyword::Menace),
        "the one-shot scope does not include creatures entering after resolution"
    );
    end_active_turn(&mut menace_engine, 0);
    for current_friendly in [source, bear] {
        assert!(
            !menace_engine
                .effective_has_keyword(current_friendly, tricerules_cards::Keyword::Menace),
            "the until-end-of-turn grant expires during cleanup"
        );
    }
}

#[test]
fn issue_424_modes_resolve_in_combination_across_the_cohort() {
    let mut engine = modal_engine(424_070);
    let victim = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let sword = inject_permanent_on_battlefield(&mut engine, 1, "short_sword");

    // Gearbane Orangutan destroys the artifact while Night's chosen mode resolves.
    cast_and_announce(&mut engine, "gearbane_orangutan");
    engine
        .apply_scenario_command(0, &choose_trigger_mode(0, target_object(sword)))
        .expect("destroy the artifact");
    pass_both_players(&mut engine);
    assert_eq!(engine.state.objects[&sword].zone, Zone::Graveyard);

    // Glamermite then taps the opposing creature in the same turn.
    cast_and_announce(&mut engine, "glamermite");
    engine
        .apply_scenario_command(0, &choose_trigger_mode(0, target_object(victim)))
        .expect("tap the creature");
    pass_both_players(&mut engine);
    assert!(engine.state.objects[&victim].tapped);
    assert_eq!(engine.state.objects[&sword].zone, Zone::Graveyard);
}
