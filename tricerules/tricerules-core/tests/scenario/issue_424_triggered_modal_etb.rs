//! Issue #424 — the eight retained triggered-modal Standard identities.
//!
//! Oracle and the current Comprehensive Rules were checked 2026-09-19 against the pinned
//! Scryfall snapshot. CR 603.3c/700.2b (mode announcement), CR 122 (counters), CR 400.7/701.13
//! (graveyard return and exile), CR 701.7 (destroy), CR 118.12/701.17 (mandatory sacrifice),
//! CR 701.26 (tap/untap), and CR 701.18 (scry) govern the exercised behavior.

use super::helpers::*;
use tricerules_cards::CounterKind;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::{
    ruled_command::Cmd, ChoiceKind, ChooseTriggerTarget, ResolutionChoiceDecision, RuledCommand,
    RuledEventBatch, SelectedSpellMode, SubmitResolutionChoice, TargetRef, TargetRefKind,
};

fn modal_engine(seed: u64) -> GameEngine {
    // Explicit land-only decks keep the default grizzly_bears/library fixtures out of the
    // opening hand so injected fixture names stay unique.
    let decks = Some(vec![deck_with("mountain", &[]), deck_with("forest", &[])]);
    let mut engine = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("engine");
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
        .apply_command(0, &cast_spell(slot, Vec::new()))
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
    engine.apply_command(first, &pass()).expect("first pass");
    engine
        .apply_command(second, &pass())
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
        .apply_command(0, &choose_trigger_mode(0, targets))
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
            .apply_command(0, &choose_trigger_mode(1, vec![graveyard_target(large)]),)
            .is_err(),
        "mana value 4 is not legal for the two-or-less return"
    );
    return_engine
        .apply_command(0, &choose_trigger_mode(1, vec![graveyard_target(small)]))
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
            .apply_command(0, &choose_trigger_mode(0, vec![graveyard_target(small)]),)
            .is_err(),
        "mana value 2 is not legal for the four-or-greater return"
    );
    repair_engine
        .apply_command(0, &choose_trigger_mode(0, vec![graveyard_target(large)]))
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
            .apply_command(0, &choose_trigger_mode(1, target_object(bear)))
            .is_err(),
        "a creature is neither an artifact nor an enchantment"
    );
    impound_engine
        .apply_command(0, &choose_trigger_mode(1, target_object(sword)))
        .expect("exile the artifact");
    pass_both_players(&mut impound_engine);
    // A second copy of the same identity may exile the enchantment during the same turn.
    cast_and_announce(&mut impound_engine, "damage_control_crew");
    impound_engine
        .apply_command(0, &choose_trigger_mode(1, target_object(anthem)))
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
        .apply_command(0, &choose_trigger_mode(0, target_object(sword)))
        .expect("destroy the artifact");
    pass_both_players(&mut destroy_engine);
    assert_eq!(destroy_engine.state.objects[&sword].zone, Zone::Graveyard);

    let mut sacrifice_engine = modal_engine(424_021);
    let artifact = inject_permanent_on_battlefield(&mut sacrifice_engine, 0, "short_sword");
    cast_and_announce(&mut sacrifice_engine, "gearbane_orangutan");
    let source = battlefield_object_for_card(&sacrifice_engine, 0, "gearbane_orangutan");
    sacrifice_engine
        .apply_command(0, &choose_trigger_mode(1, Vec::new()))
        .expect("announce the sacrifice mode");
    let branch_batch = resolve_top_stack(&mut sacrifice_engine);
    let branch = find_resolution_choice(&branch_batch).expect("mandatory branch selection");
    assert_eq!(branch.choice_kind(), ChoiceKind::ResolutionBranch);
    assert_eq!((branch.min, branch.max), (1, 1));
    let payment_batch = sacrifice_engine
        .apply_command(0, &select_branch(0))
        .expect("select the mandatory sacrifice branch");
    let payment = find_resolution_choice(&payment_batch).expect("mandatory sacrifice payment");
    assert!(payment.candidate_object_ids.contains(&artifact));
    assert!(
        !payment.candidate_object_ids.contains(&source),
        "the Ape is not an artifact"
    );
    sacrifice_engine
        .apply_command(0, &submit_resolution_choice(vec![artifact]))
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

    // With no artifact available the mandatory branch is skipped and no counters are placed.
    let mut empty_engine = modal_engine(424_022);
    cast_and_announce(&mut empty_engine, "gearbane_orangutan");
    let source = battlefield_object_for_card(&empty_engine, 0, "gearbane_orangutan");
    empty_engine
        .apply_command(0, &choose_trigger_mode(1, Vec::new()))
        .expect("announce the sacrifice mode with nothing to sacrifice");
    pass_both_players(&mut empty_engine);
    assert_eq!(counters(&empty_engine, source), 0);
}

#[test]
fn issue_424_tap_and_untap_pairs_reject_lands_and_toggle_their_targets() {
    let mut ant_engine = modal_engine(424_030);
    let bear = inject_creature_on_battlefield(&mut ant_engine, 1, "grizzly_bears");
    let mountain = inject_permanent_on_battlefield(&mut ant_engine, 1, "mountain");
    cast_and_announce(&mut ant_engine, "giant-sized_flying_ant");
    assert!(
        ant_engine
            .apply_command(0, &choose_trigger_mode(0, target_object(mountain)))
            .is_err(),
        "a land is not a nonland permanent"
    );
    ant_engine
        .apply_command(0, &choose_trigger_mode(0, target_object(bear)))
        .expect("tap the nonland permanent");
    pass_both_players(&mut ant_engine);
    assert!(ant_engine.state.objects[&bear].tapped);

    let mut untap_engine = modal_engine(424_031);
    let own = inject_creature_on_battlefield(&mut untap_engine, 0, "grizzly_bears");
    untap_engine.state.objects.get_mut(&own).unwrap().tapped = true;
    cast_and_announce(&mut untap_engine, "giant-sized_flying_ant");
    untap_engine
        .apply_command(0, &choose_trigger_mode(1, target_object(own)))
        .expect("untap the nonland permanent");
    pass_both_players(&mut untap_engine);
    assert!(!untap_engine.state.objects[&own].tapped);

    let mut mite_engine = modal_engine(424_032);
    let victim = inject_creature_on_battlefield(&mut mite_engine, 1, "grizzly_bears");
    let sword = inject_permanent_on_battlefield(&mut mite_engine, 1, "short_sword");
    cast_and_announce(&mut mite_engine, "glamermite");
    assert!(
        mite_engine
            .apply_command(0, &choose_trigger_mode(0, target_object(sword)))
            .is_err(),
        "the creature-only tap mode rejects an artifact"
    );
    mite_engine
        .apply_command(0, &choose_trigger_mode(0, target_object(victim)))
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
        .apply_command(0, &choose_trigger_mode(1, target_object(own)))
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
        .apply_command(0, &choose_trigger_mode(1, Vec::new()))
        .expect("announce scry 3");
    let batch = resolve_top_stack(&mut oltec_engine);
    let choice = find_resolution_choice(&batch).expect("scry choice");
    assert_eq!(choice.choice_kind(), ChoiceKind::LibraryTop);
    assert_eq!((choice.min, choice.max), (0, 3));
    assert_eq!(choice.candidate_object_ids, top);
    oltec_engine
        .apply_command(0, &submit_resolution_choice(vec![top[0], top[1]]))
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
            .apply_command(0, &choose_trigger_mode(0, vec![graveyard_target(bear)]))
            .is_err(),
        "a creature card is not an artifact card"
    );
    oltec_engine
        .apply_command(0, &choose_trigger_mode(0, vec![graveyard_target(sword)]))
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
            .apply_command(
                0,
                &choose_trigger_mode(1, vec![graveyard_target(ours), graveyard_target(theirs)]),
            )
            .is_err(),
        "the two chosen cards must share one graveyard"
    );
    qutrub_engine
        .apply_command(
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
                .apply_command(0, &choose_trigger_mode(0, target_object(illegal)))
                .is_err(),
            "an undamaged or own creature is not a legal target"
        );
    }
    engine
        .apply_command(0, &choose_trigger_mode(0, target_object(marked)))
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
    cast_and_announce(&mut engine, "lord_skitters_butcher");
    let source = battlefield_object_for_card(&engine, 0, "lord_skitters_butcher");
    engine
        .apply_command(0, &choose_trigger_mode(1, Vec::new()))
        .expect("announce the sacrifice mode");
    let branch_batch = resolve_top_stack(&mut engine);
    let branch = find_resolution_choice(&branch_batch).expect("optional branch selection");
    assert_eq!(branch.choice_kind(), ChoiceKind::ResolutionBranch);
    assert_eq!((branch.min, branch.max), (0, 1));
    let payment_batch = engine
        .apply_command(0, &select_branch(0))
        .expect("select the optional sacrifice branch");
    let payment = find_resolution_choice(&payment_batch).expect("optional sacrifice payment");
    assert!(payment.candidate_object_ids.contains(&other));
    assert!(
        !payment.candidate_object_ids.contains(&source),
        "another creature excludes the entering Butcher"
    );
    let library_before = engine.state.players[0].library.len();
    let hand_before = engine.state.players[0].hand.len();
    let scry_batch = engine
        .apply_command(0, &submit_resolution_choice(vec![other]))
        .expect("sacrifice the other creature");
    assert_eq!(engine.state.objects[&other].zone, Zone::Graveyard);
    let scry = find_resolution_choice(&scry_batch).expect("scry 2 after the sacrifice");
    assert_eq!(scry.choice_kind(), ChoiceKind::LibraryTop);
    assert_eq!((scry.min, scry.max), (0, 2));
    assert_eq!(
        scry.candidate_object_ids.len(),
        2,
        "the scry sees two cards"
    );
    let bottom = scry.candidate_object_ids[0];
    engine
        .apply_command(0, &submit_resolution_choice(vec![bottom]))
        .expect("bottom one of the two scried cards");
    assert_eq!(
        engine.state.players[0].library.len(),
        library_before - 1,
        "the receipt's draw takes the remaining scried top card"
    );
    assert_eq!(engine.state.players[0].hand.len(), hand_before + 1);

    // Declining the optional sacrifice skips the whole "if you do" receipt.
    let mut decline_engine = modal_engine(424_081);
    let bystander = inject_creature_on_battlefield(&mut decline_engine, 0, "grizzly_bears");
    cast_and_announce(&mut decline_engine, "lord_skitters_butcher");
    decline_engine
        .apply_command(0, &choose_trigger_mode(1, Vec::new()))
        .expect("announce the sacrifice mode");
    let branch_batch = resolve_top_stack(&mut decline_engine);
    find_resolution_choice(&branch_batch).expect("optional branch selection");
    let hand_before = decline_engine.state.players[0].hand.len();
    decline_engine
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
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
}

#[test]
fn issue_424_butcher_rat_token_and_team_menace_resolve() {
    let mut rat_engine = modal_engine(424_082);
    cast_and_announce(&mut rat_engine, "lord_skitters_butcher");
    rat_engine
        .apply_command(0, &choose_trigger_mode(0, Vec::new()))
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
    cast_and_announce(&mut menace_engine, "lord_skitters_butcher");
    menace_engine
        .apply_command(0, &choose_trigger_mode(2, Vec::new()))
        .expect("announce the team menace mode");
    pass_both_players(&mut menace_engine);
    let keywords = menace_engine
        .characteristics(bear)
        .expect("bear characteristics")
        .keywords
        .clone();
    assert!(
        keywords.contains(&tricerules_cards::Keyword::Menace),
        "the team grant covers each creature you control"
    );
}

#[test]
fn issue_424_modes_resolve_in_combination_across_the_cohort() {
    let mut engine = modal_engine(424_070);
    let victim = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let sword = inject_permanent_on_battlefield(&mut engine, 1, "short_sword");

    // Gearbane Orangutan destroys the artifact while Night's chosen mode resolves.
    cast_and_announce(&mut engine, "gearbane_orangutan");
    engine
        .apply_command(0, &choose_trigger_mode(0, target_object(sword)))
        .expect("destroy the artifact");
    pass_both_players(&mut engine);
    assert_eq!(engine.state.objects[&sword].zone, Zone::Graveyard);

    // Glamermite then taps the opposing creature in the same turn.
    cast_and_announce(&mut engine, "glamermite");
    engine
        .apply_command(0, &choose_trigger_mode(0, target_object(victim)))
        .expect("tap the creature");
    pass_both_players(&mut engine);
    assert!(engine.state.objects[&victim].tapped);
    assert_eq!(engine.state.objects[&sword].zone, Zone::Graveyard);
}
