//! Actual-card coverage for Myr Retriever's targeted dies trigger.
//!
//! The exact Oracle text and Scryfall ruling were fetched 2026-09-26. CR 603.10a covers looking
//! back for a dies trigger; CR 115.1d and 603.3d cover choosing its target as it goes on the stack;
//! CR 400.7e allows a simultaneously deceased artifact to be found; CR 608.2b checks the target
//! again on resolution.

use super::helpers::*;
use tricerules_core::{GameEngine, Zone};
use tricerules_proto::ruled::v1::{
    ruled_command::Cmd, ChooseTriggerTarget, RuledCommand, TargetRef, TargetRefKind,
};

const MYR_RETRIEVER: &str = "myr_retriever";
const ORNITHOPTER: &str = "ornithopter";

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("island", &[]), deck_with("forest", &[])]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    grant_pool(&mut engine, 0);
    grant_pool(&mut engine, 1);
    engine
}

fn choose_graveyard_target(object_id: u32) -> RuledCommand {
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

fn cast_wrath_of_god(engine: &mut GameEngine) {
    inject_card_into_hand(engine, 0, "wrath_of_god");
    let slot = hand_index_for_card(engine, 0, "wrath_of_god");
    engine
        .apply_command(0, &cast_spell(slot, Vec::new()))
        .expect("cast Wrath of God");
    pass_both_players(engine);
}

#[test]
fn myr_retriever_targets_another_artifact_that_dies_at_the_same_time() {
    let mut engine = engine(202_609_290);
    let retriever = inject_creature_on_battlefield(&mut engine, 0, MYR_RETRIEVER);
    let other_artifact = inject_creature_on_battlefield(&mut engine, 0, ORNITHOPTER);
    let own_nonartifact = inject_graveyard_card(&mut engine, 0, "grizzly_bears");
    let opponent_artifact = inject_graveyard_card(&mut engine, 1, "chromatic_star");

    cast_wrath_of_god(&mut engine);
    assert_eq!(engine.state.objects[&retriever].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&other_artifact].zone, Zone::Graveyard);
    assert_eq!(engine.state.pending_triggers.len(), 1);

    let legal = engine.initial_response_batch();
    let ability_key = u64::from(retriever) << 32;
    let candidates = &legal.legal_by_player[&0].valid_targets_by_ability[&ability_key].groups[0]
        .valid_graveyard_ids;
    assert!(
        candidates.contains(&other_artifact),
        "an artifact that died simultaneously is a legal target"
    );
    assert!(
        !candidates.contains(&retriever),
        "another excludes this card"
    );
    assert!(
        !candidates.contains(&own_nonartifact),
        "the target must be an artifact card"
    );
    assert!(
        !candidates.contains(&opponent_artifact),
        "your graveyard means the trigger controller's graveyard"
    );

    engine
        .apply_command(0, &choose_graveyard_target(retriever))
        .expect_err("Myr Retriever cannot target its own graveyard incarnation");
    semantic::accepted(&mut engine, 0, &choose_graveyard_target(other_artifact));
    resolve_entire_stack_two_player(&mut engine);

    assert!(engine.state.players[0].hand.contains(&other_artifact));
    assert_eq!(engine.state.objects[&other_artifact].zone, Zone::Hand);
    assert_eq!(engine.state.objects[&retriever].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&own_nonartifact].zone, Zone::Graveyard);
    assert_eq!(
        engine.state.objects[&opponent_artifact].zone,
        Zone::Graveyard
    );
}

#[test]
fn myr_retriever_uses_its_controller_at_death_and_not_its_owner_for_graveyard_targets() {
    let mut engine = engine(2026093008);
    let retriever = inject_creature_under_foreign_control(&mut engine, 0, 1, MYR_RETRIEVER);
    let owners_artifact = inject_graveyard_card(&mut engine, 0, "chromatic_star");
    let controllers_artifact = inject_graveyard_card(&mut engine, 1, "mind_stone");
    cast_wrath_of_god(&mut engine);
    assert_eq!(engine.state.objects[&retriever].zone, Zone::Graveyard);
    assert!(engine.state.players[0].graveyard.contains(&retriever));
    assert!(!engine.state.players[1].graveyard.contains(&retriever));
    assert_eq!(engine.state.pending_triggers.len(), 1);
    let legal = engine.initial_response_batch();
    let ability_key = u64::from(retriever) << 32;
    let candidates = &legal.legal_by_player[&1].valid_targets_by_ability[&ability_key].groups[0]
        .valid_graveyard_ids;
    assert!(candidates.contains(&controllers_artifact));
    assert!(!candidates.contains(&owners_artifact));
    assert!(!candidates.contains(&retriever));
    let pending = format!("{:?}", engine.state.pending_triggers);
    engine
        .apply_command(0, &choose_graveyard_target(controllers_artifact))
        .expect_err("only the event-time controller chooses");
    assert_eq!(format!("{:?}", engine.state.pending_triggers), pending);
    engine
        .apply_command(1, &choose_graveyard_target(owners_artifact))
        .expect_err("source owner's graveyard is not your graveyard");
    assert_eq!(format!("{:?}", engine.state.pending_triggers), pending);
    engine
        .apply_command(1, &choose_graveyard_target(controllers_artifact))
        .unwrap();
    assert_eq!(engine.state.stack.last().unwrap().controller, 1);
    resolve_entire_stack_two_player(&mut engine);
    assert!(engine.state.players[1].hand.contains(&controllers_artifact));
    assert!(!engine.state.players[0].hand.contains(&controllers_artifact));
    assert_eq!(engine.state.objects[&owners_artifact].zone, Zone::Graveyard);
}
