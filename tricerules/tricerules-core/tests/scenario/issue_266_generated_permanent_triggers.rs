//! Issue #266 — generated permanent-trigger recipes reuse engine-owned life-event,
//! target-publication, private-discard, and zone-change-generation contracts.
//!
//! Oracle and rulings checked 2026-09-13. CR 115.1d and 608.2b govern triggered-ability
//! targets and resolution revalidation; CR 118.3 governs life gain; CR 122.1a governs
//! +1/+1 counters; CR 400.7 governs zone-change identity; CR 603 governs trigger staging
//! and ordering; and CR 701.9 governs discard.

use super::helpers::*;
use tricerules_cards::CounterKind;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::{ChoiceKind, ChooseTriggerTarget, TargetRef, TargetRefKind};

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

fn choose_trigger_targets(targets: Vec<TargetRef>) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
            decline: false,
            selected_modes: Vec::new(),
            targets,
        })),
    }
}

fn permanent_target(object_id: u32) -> TargetRef {
    TargetRef {
        object_id,
        group_index: 0,
        kind: TargetRefKind::Permanent as i32,
        ..Default::default()
    }
}

#[test]
fn generated_life_payoffs_observe_one_committed_life_gain_event() {
    let decks = Some(vec![
        deck_with(
            "plains",
            &[
                "ajanis_pridemate",
                "pest_mascot",
                "marauding_blight-priest",
                "angels_mercy",
            ],
        ),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(266_001, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    let pridemate = move_ready_to_battlefield(&mut engine, 0, "ajanis_pridemate");
    let mascot = move_ready_to_battlefield(&mut engine, 0, "pest_mascot");
    move_ready_to_battlefield(&mut engine, 0, "marauding_blight-priest");
    ensure_in_hand(&mut engine, 0, "angels_mercy");
    grant_pool(&mut engine, 0);

    let mercy = hand_index_for_card(&engine, 0, "angels_mercy");
    engine
        .apply_command(0, &cast_spell(mercy, Vec::new()))
        .expect("cast Angel's Mercy");
    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(engine.state.players[0].life, 27);
    assert_eq!(engine.state.players[1].life, 19);
    assert_eq!(
        engine.state.objects[&pridemate].counter_count(CounterKind::PlusOnePlusOne),
        1
    );
    assert_eq!(
        engine.state.objects[&mascot].counter_count(CounterKind::PlusOnePlusOne),
        1
    );
}

#[test]
fn generated_target_opponent_discard_is_chosen_by_the_affected_player() {
    let decks = Some(vec![
        deck_with("swamp", &["corrupt_court_official"]),
        deck_with("forest", &["storm_crow"]),
    ]);
    let mut engine = GameEngine::new(266_002, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    ensure_in_hand(&mut engine, 1, "storm_crow");
    let discarded = engine.state.players[1]
        .hand
        .iter()
        .copied()
        .find(|object_id| engine.state.objects[object_id].card_id == "storm_crow")
        .expect("Storm Crow in opponent hand");

    move_ready_to_battlefield(&mut engine, 0, "corrupt_court_official");
    assert!(engine
        .apply_command(0, &choose_trigger_targets(target_player(0)))
        .is_err());
    engine
        .apply_command(0, &choose_trigger_targets(target_player(1)))
        .expect("choose opponent");

    let batch = resolve_top_stack(&mut engine);
    let choice = find_resolution_choice(&batch).expect("private discard choice");
    assert_eq!(choice.deciding_player_id, 1);
    assert_eq!(choice.choice_kind(), ChoiceKind::HandCards);
    assert_eq!((choice.min, choice.max), (1, 1));
    assert!(choice.candidate_object_ids.contains(&discarded));
    assert!(engine
        .apply_command(0, &submit_resolution_choice(vec![discarded]))
        .is_err());
    engine
        .apply_command(1, &submit_resolution_choice(vec![discarded]))
        .expect("affected player chooses the discard");
    assert_eq!(engine.state.objects[&discarded].zone, Zone::Graveyard);
}

#[test]
fn generated_optional_bounce_excludes_source_and_revalidates_generation() {
    let decks = Some(vec![
        deck_with("island", &["rimekin_recluse"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(266_003, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    let target = inject_creature_on_battlefield(&mut engine, 1, "storm_crow");

    let recluse = move_ready_to_battlefield(&mut engine, 0, "rimekin_recluse");
    assert!(engine
        .apply_command(0, &choose_trigger_targets(vec![permanent_target(recluse)]))
        .is_err());
    engine
        .apply_command(0, &choose_trigger_targets(vec![permanent_target(target)]))
        .expect("choose another creature");

    let generation = engine
        .state
        .zone_change_generation
        .get(&target)
        .copied()
        .unwrap_or(0);
    engine
        .state
        .zone_change_generation
        .insert(target, generation + 1);
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&target].zone, Zone::Battlefield);
}
