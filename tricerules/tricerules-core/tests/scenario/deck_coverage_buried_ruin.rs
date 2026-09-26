//! Actual-card coverage for Buried Ruin's colorless mana and artifact recovery abilities.
//!
//! Oracle and rulings were checked against Scryfall on 2026-09-26; no rulings are published.
//! CR 605.1a/605.3b cover the mana ability, 602.2a-b and 701.21a cover activation costs and sacrifice,
//! and 115.1c/608.2b cover its target.

use super::helpers::*;
use tricerules_core::{GameEngine, Zone};
use tricerules_proto::ruled::v1::{TargetRef, TargetRefKind};

const BURIED_RUIN: &str = "buried_ruin";

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("island", &[]), deck_with("forest", &[])]);
    let mut engine = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn graveyard_target(object_id: u32) -> Vec<TargetRef> {
    vec![TargetRef {
        kind: TargetRefKind::Graveyard as i32,
        object_id,
        group_index: 0,
        ..Default::default()
    }]
}

#[test]
fn buried_ruin_taps_for_colorless_and_recovers_an_artifact_after_sacrificing() {
    let mut engine = engine(202_609_310);
    let mana_source = inject_permanent_on_battlefield(&mut engine, 0, BURIED_RUIN);
    let recovery_source = inject_permanent_on_battlefield(&mut engine, 0, BURIED_RUIN);
    let own_artifact = inject_graveyard_card(&mut engine, 0, "chromatic_star");
    let own_nonartifact = inject_graveyard_card(&mut engine, 0, "grizzly_bears");
    let opponent_artifact = inject_graveyard_card(&mut engine, 1, "chromatic_star");

    apply_ability(&mut engine, 0, mana_source, 0, vec![])
        .expect("activate Buried Ruin's colorless mana ability");
    assert_eq!(engine.state.players[0].mana_pool.colorless, 1);
    assert!(engine.state.objects[&mana_source].tapped);
    assert!(
        engine.state.stack.is_empty(),
        "the mana ability resolves immediately"
    );

    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );
    for illegal_target in [own_nonartifact, opponent_artifact] {
        engine
            .apply_command(
                0,
                &activate_ability_for(
                    &engine,
                    recovery_source,
                    1,
                    graveyard_target(illegal_target),
                ),
            )
            .expect_err("only an artifact from the controller's graveyard is legal");
        assert_eq!(
            engine.state.objects[&recovery_source].zone,
            Zone::Battlefield
        );
        assert!(!engine.state.objects[&recovery_source].tapped);
    }

    apply_ability(
        &mut engine,
        0,
        recovery_source,
        1,
        graveyard_target(own_artifact),
    )
    .expect("pay {2}, tap, and sacrifice Buried Ruin");
    assert_eq!(engine.state.objects[&recovery_source].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&own_artifact].zone, Zone::Graveyard);
    assert_eq!(engine.state.stack.len(), 1);

    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&own_artifact].zone, Zone::Hand);
    assert!(engine.state.players[0].hand.contains(&own_artifact));
    assert_eq!(engine.state.objects[&own_nonartifact].zone, Zone::Graveyard);
    assert_eq!(
        engine.state.objects[&opponent_artifact].zone,
        Zone::Graveyard
    );
}
