//! Issue #263 — exact generated creature-trigger recipes reuse engine-owned ETB, cast, draw,
//! attack, target, counter, life, hand-return, and private-library movement contracts.
//!
//! Oracle and rulings checked 2026-09-12. CR 115.1d and 608.2b govern trigger targets and
//! resolution revalidation; CR 121.1-2 and 122.1a govern individual draws and +1/+1 counters;
//! CR 400.7 governs zone-change identity; CR 508.1m/508.2a govern attack triggers; CR
//! 603.2/603.3b/603.5 govern staging, APNAP ordering, and optional effects; and CR 701.17
//! governs mill, including the inability to choose an optional mill larger than the library.

use super::helpers::*;
use tricerules_cards::CounterKind;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::{
    permanent_moved, ChoiceKind, ChooseTriggerTarget, ResolutionChoiceDecision,
    SubmitResolutionChoice, TargetRef, TargetRefKind,
};

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
        .expect("second pass resolves stack item")
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

fn choose_trigger_targets(targets: Vec<TargetRef>, decline: bool) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
            decline,
            selected_modes: Vec::new(),
            targets,
        })),
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

fn select_optional_effect() -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
            decision: ResolutionChoiceDecision::SelectBranch as i32,
            selected_branch_index: 0,
            ..Default::default()
        })),
    }
}

fn decline_optional_effect() -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
            decision: ResolutionChoiceDecision::Decline as i32,
            ..Default::default()
        })),
    }
}

fn advance_to_next_main1(engine: &mut GameEngine, player: i32) {
    let starting_turn = engine.state.turn_instance;
    for _ in 0..240 {
        if engine.state.turn_instance > starting_turn
            && engine.state.active_player_id() == player
            && engine.state.turn_step == tricerules_core::TurnStep::Main1
            && engine.state.priority_player_id() == player
        {
            return;
        }
        resolve_cleanup_discards_if_any(engine);
        let priority = engine.state.priority_player_id();
        engine
            .apply_command(priority, &pass())
            .expect("advance turn");
    }
    panic!("turn advancement stalled before P{player}'s next main phase");
}

fn ensure_copies_in_hand(engine: &mut GameEngine, player: usize, card_id: &str, count: usize) {
    while engine.state.players[player]
        .hand
        .iter()
        .filter(|object_id| engine.state.objects[object_id].card_id == card_id)
        .count()
        < count
    {
        take_card_from_library_to_hand(engine, player, card_id);
    }
}

#[test]
fn generated_optional_bounce_restricts_targets_and_revalidates_target_generation() {
    let decks = Some(vec![
        deck_with("plains", &["exosuit_savior", "mischievous_pup"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(263_001, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    let own_artifact = inject_permanent_on_battlefield(&mut engine, 0, "icy_manipulator");
    let opponent_artifact = inject_permanent_on_battlefield(&mut engine, 1, "icy_manipulator");

    let savior = move_ready_to_battlefield(&mut engine, 0, "exosuit_savior");
    assert_eq!(engine.state.pending_triggers.len(), 1);
    assert!(engine
        .apply_command(
            0,
            &choose_trigger_targets(vec![trigger_target(savior)], false),
        )
        .is_err());
    assert!(engine
        .apply_command(
            0,
            &choose_trigger_targets(vec![trigger_target(opponent_artifact)], false),
        )
        .is_err());
    engine
        .apply_command(
            0,
            &choose_trigger_targets(vec![trigger_target(own_artifact)], false),
        )
        .expect("choose another permanent controlled by the trigger controller");

    let captured_generation = engine
        .state
        .zone_change_generation
        .get(&own_artifact)
        .copied()
        .unwrap_or(0);
    engine
        .state
        .zone_change_generation
        .insert(own_artifact, captured_generation + 2);
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&own_artifact].zone, Zone::Battlefield);

    move_ready_to_battlefield(&mut engine, 0, "mischievous_pup");
    engine
        .apply_command(0, &choose_trigger_targets(Vec::new(), false))
        .expect("up to one permits choosing no target without declining a may effect");
    resolve_entire_stack_two_player(&mut engine);
}

#[test]
fn generated_may_mill_can_be_declined_or_move_exactly_two_cards_in_library_order() {
    let decks = Some(vec![
        deck_with("swamp", &["daggerfang_duo", "deathcap_marionette"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(263_002, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);

    move_ready_to_battlefield(&mut engine, 0, "daggerfang_duo");
    assert_eq!(engine.state.stack.len(), 1);
    let decline_prompt = resolve_top_stack(&mut engine);
    assert_eq!(
        find_resolution_choice(&decline_prompt)
            .expect("optional effect choice")
            .choice_kind(),
        ChoiceKind::ResolutionBranch
    );
    engine
        .apply_command(0, &decline_optional_effect())
        .expect("decline optional mill");
    assert!(engine.state.stack.is_empty());

    let top = seat_on_top(&mut engine, 0, &["storm_crow", "grizzly_bears"]);
    move_ready_to_battlefield(&mut engine, 0, "deathcap_marionette");
    let accept_prompt = resolve_top_stack(&mut engine);
    assert_eq!(
        find_resolution_choice(&accept_prompt)
            .expect("optional effect choice")
            .choice_kind(),
        ChoiceKind::ResolutionBranch
    );
    let resolution = engine
        .apply_command(0, &select_optional_effect())
        .expect("accept optional mill");
    assert!(top
        .iter()
        .all(|object_id| engine.state.objects[object_id].zone == Zone::Graveyard));
    let moves = permanents_moved_in(&resolution)
        .into_iter()
        .filter(|moved| top.contains(&moved.object_id))
        .collect::<Vec<_>>();
    assert_eq!(
        moves
            .iter()
            .map(|moved| moved.object_id)
            .collect::<Vec<_>>(),
        top
    );
    assert!(moves
        .iter()
        .all(|moved| moved.destination() == permanent_moved::Destination::Graveyard));

    let decks = Some(vec![
        deck_with("forest", &["mineshaft_spider"]),
        deck_with("forest", &[]),
    ]);
    let mut short_library =
        GameEngine::new(263_003, &[0, 1], 20, decks, true).expect("short-library engine");
    advance_to_main1_from_game_start(&mut short_library);
    ensure_in_hand(&mut short_library, 0, "mineshaft_spider");
    let only_card = inject_library_card(&mut short_library, 0, "storm_crow");
    short_library.state.players[0].library.clear();
    short_library.state.players[0].library.push_back(only_card);
    move_ready_to_battlefield(&mut short_library, 0, "mineshaft_spider");
    let resolution = resolve_top_stack(&mut short_library);
    assert!(
        find_resolution_choice(&resolution).is_none(),
        "mill two is not a selectable may action with only one library card"
    );
    assert_eq!(short_library.state.objects[&only_card].zone, Zone::Library);
}

#[test]
fn generated_noncreature_cast_triggers_use_ordering_and_exact_source_generations() {
    let decks = Some(vec![
        deck_with(
            "mountain",
            &[
                "boar-q-pine",
                "tempest_angler",
                "grizzly_bears",
                "bonesplitter",
            ],
        ),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(263_003, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    let boar = relocate_to_battlefield(&mut engine, 0, "boar-q-pine", false);
    let angler = relocate_to_battlefield(&mut engine, 0, "tempest_angler", false);
    ensure_in_hand(&mut engine, 0, "grizzly_bears");
    ensure_in_hand(&mut engine, 0, "bonesplitter");
    grant_pool(&mut engine, 0);

    let creature = hand_index_for_card(&engine, 0, "grizzly_bears");
    engine
        .apply_command(0, &cast_spell(creature, vec![]))
        .expect("cast creature spell");
    assert_eq!(engine.state.stack.len(), 1, "creature casts do not trigger");
    resolve_entire_stack_two_player(&mut engine);

    let equipment = hand_index_for_card(&engine, 0, "bonesplitter");
    engine
        .apply_command(0, &cast_spell(equipment, vec![]))
        .expect("cast noncreature spell");
    assert!(
        engine.state.pending_trigger_order.is_some(),
        "simultaneous generated triggers enter the shared CR 603.3b ordering path"
    );
    answer_trigger_order_in_engine_order(&mut engine);
    assert_eq!(engine.state.stack.len(), 3);

    let old_generation = engine
        .state
        .zone_change_generation
        .get(&boar)
        .copied()
        .unwrap_or(0);
    engine
        .state
        .zone_change_generation
        .insert(boar, old_generation + 2);
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        engine.state.objects[&boar].counter_count(CounterKind::PlusOnePlusOne),
        0,
        "a trigger cannot counter a new generation represented by the same object id"
    );
    assert_eq!(
        engine.state.objects[&angler].counter_count(CounterKind::PlusOnePlusOne),
        1
    );
}

#[test]
fn generated_second_draw_trigger_fires_once_each_turn_and_resets_next_turn() {
    let decks = Some(vec![
        deck_with("island", &["atlantean_cavalry", "divination", "divination"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(263_004, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    let cavalry = relocate_to_battlefield(&mut engine, 0, "atlantean_cavalry", false);
    ensure_copies_in_hand(&mut engine, 0, "divination", 2);
    grant_pool(&mut engine, 0);

    let first = hand_index_for_card(&engine, 0, "divination");
    engine
        .apply_command(0, &cast_spell(first, vec![]))
        .expect("cast first Divination");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        engine.state.objects[&cavalry].counter_count(CounterKind::PlusOnePlusOne),
        1,
        "the two individual draws produce exactly the second-draw trigger"
    );

    advance_to_next_main1(&mut engine, 0);
    assert_eq!(engine.state.active_player_id(), 0);
    assert_eq!(engine.state.turn_step, tricerules_core::TurnStep::Main1);
    assert_eq!(engine.state.priority_player_id(), 0);
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.pending_trigger_order.is_none());
    assert!(engine.state.pending_triggers.is_empty());
    grant_pool(&mut engine, 0);
    let second = hand_index_for_card(&engine, 0, "divination");
    engine
        .apply_command(0, &cast_spell(second, vec![]))
        .expect("cast second-turn Divination");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        engine.state.objects[&cavalry].counter_count(CounterKind::PlusOnePlusOne),
        2,
        "draw ordinals reset when the turn changes"
    );
}

#[test]
fn generated_attack_triggers_only_stage_for_declared_attackers() {
    let mut engine = GameEngine::new(263_005, &[0, 1], 20, None, true).expect("engine");
    advance_to_declare_attackers(&mut engine);
    let herald = inject_creature_on_battlefield(&mut engine, 0, "herald_of_faith");
    let phantasm = inject_creature_on_battlefield(&mut engine, 0, "mysterios_phantasm");
    inject_creature_on_battlefield(&mut engine, 0, "shopkeepers_bane");
    let top = seat_on_top(&mut engine, 0, &["storm_crow"])[0];

    assert!(engine.state.stack.is_empty());
    engine
        .apply_command(0, &declare_attackers(vec![herald, phantasm]))
        .expect("declare two generated attackers");
    assert!(engine.state.pending_trigger_order.is_some());
    answer_trigger_order_in_engine_order(&mut engine);
    assert_eq!(
        engine.state.stack.len(),
        2,
        "the nonattacking generated creature does not trigger"
    );
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.players[0].life, 22);
    assert_eq!(engine.state.objects[&top].zone, Zone::Graveyard);
}
