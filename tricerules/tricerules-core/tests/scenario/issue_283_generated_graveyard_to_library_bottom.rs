//! Issue #283 — the generated permanent ability targets only a controller-owned graveyard card,
//! revalidates that exact current-generation object, and puts it at the bottom of its owner's
//! library.
//!
//! Oracle and rulings checked 2026-09-14. CR 115.1c/115.2, 400.3/400.5/400.7, 401, 602.1, and
//! 608.2b govern the targeted activated ability, owned zones, library order, zone-change identity,
//! activation, and resolution revalidation.

use super::helpers::*;
use tricerules_core::{EngineError, Zone};
use tricerules_proto::ruled::v1::TargetRefKind;

fn issue_283_engine(seed: u64) -> GameEngine {
    let decks = Some(vec![
        deck_with("forest", &["barkform_harvester"]),
        deck_with("island", &[]),
    ]);
    let mut engine = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("issue #283 engine");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn graveyard_target(object_id: u32) -> Vec<TargetRef> {
    vec![TargetRef {
        object_id,
        kind: TargetRefKind::Graveyard as i32,
        ..Default::default()
    }]
}

fn published_graveyard_targets(engine: &mut GameEngine, source: u32) -> Vec<u32> {
    let key = u64::from(source) << 32;
    engine.initial_response_batch().legal_by_player[&0].valid_targets_by_ability[&key].groups[0]
        .valid_graveyard_ids
        .clone()
}

fn move_graveyard_to_exile(engine: &mut GameEngine, player: usize, object_id: u32) {
    engine.state.players[player]
        .graveyard
        .retain(|id| *id != object_id);
    engine.state.players[player].exile.push(object_id);
    engine
        .state
        .objects
        .get_mut(&object_id)
        .expect("object")
        .zone = Zone::Exile;
    *engine
        .state
        .zone_change_generation
        .entry(object_id)
        .or_default() += 1;
}

fn return_exile_to_graveyard(engine: &mut GameEngine, player: usize, object_id: u32) {
    engine.state.players[player]
        .exile
        .retain(|id| *id != object_id);
    engine.state.players[player].graveyard.push(object_id);
    engine
        .state
        .objects
        .get_mut(&object_id)
        .expect("object")
        .zone = Zone::Graveyard;
    *engine
        .state
        .zone_change_generation
        .entry(object_id)
        .or_default() += 1;
}

#[test]
fn issue_283_targets_only_controller_graveyard_and_bottoms_owned_library() {
    let mut engine = issue_283_engine(283_001);
    let own_first = inject_graveyard_card(&mut engine, 0, "lightning_bolt");
    let own_second = inject_graveyard_card(&mut engine, 0, "grizzly_bears");
    let opposing = inject_graveyard_card(&mut engine, 1, "counterspell");
    let in_hand = inject_card_into_hand(&mut engine, 0, "divination");
    let in_library = inject_library_card(&mut engine, 0, "forest");
    let existing_bottom = inject_library_card(&mut engine, 0, "mountain");
    let opponent_library = inject_library_card(&mut engine, 1, "island");
    let source = move_ready_to_battlefield(&mut engine, 0, "barkform_harvester");

    assert_eq!(
        published_graveyard_targets(&mut engine, source),
        vec![own_first, own_second]
    );
    for illegal in [opposing, in_hand, in_library] {
        give_mana(
            &mut engine,
            0,
            ManaGift {
                c: 2,
                ..Default::default()
            },
        );
        let error = engine
            .apply_command(
                0,
                &activate_ability_for(&engine, source, 0, graveyard_target(illegal)),
            )
            .expect_err("opponent or other-zone object must not be a graveyard target");
        assert!(matches!(error, EngineError::Illegal(_)));
        assert!(engine.state.stack.is_empty());
    }

    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );
    engine
        .apply_command(
            0,
            &activate_ability_for(&engine, source, 0, graveyard_target(own_first)),
        )
        .expect("controller can activate the targeted ability");
    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(engine.state.objects[&own_first].zone, Zone::Library);
    assert!(!engine.state.players[0].graveyard.contains(&own_first));
    assert_eq!(engine.state.players[0].library.back(), Some(&own_first));
    assert!(engine.state.players[0].library.contains(&existing_bottom));
    assert!(engine.state.players[0].library.contains(&in_library));
    assert!(engine.state.players[1].library.contains(&opponent_library));
    assert!(!engine.state.players[1].library.contains(&own_first));
    assert_eq!(engine.state.objects[&own_second].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&in_hand].zone, Zone::Hand);
}

#[test]
fn issue_283_revalidates_current_graveyard_generation_before_resolution() {
    let mut engine = issue_283_engine(283_002);
    let target = inject_graveyard_card(&mut engine, 0, "lightning_bolt");
    let source = move_ready_to_battlefield(&mut engine, 0, "barkform_harvester");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );
    engine
        .apply_command(
            0,
            &activate_ability_for(&engine, source, 0, graveyard_target(target)),
        )
        .expect("activate with the current graveyard generation");

    move_graveyard_to_exile(&mut engine, 0, target);
    return_exile_to_graveyard(&mut engine, 0, target);
    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(engine.state.objects[&target].zone, Zone::Graveyard);
    assert!(engine.state.players[0].graveyard.contains(&target));
    assert!(!engine.state.players[0].library.contains(&target));
    assert!(engine.state.stack.is_empty());
}

#[test]
fn issue_283_sole_target_removal_fizzles_without_moving_card() {
    let mut engine = issue_283_engine(283_003);
    let target = inject_graveyard_card(&mut engine, 0, "lightning_bolt");
    let source = move_ready_to_battlefield(&mut engine, 0, "barkform_harvester");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );
    engine
        .apply_command(
            0,
            &activate_ability_for(&engine, source, 0, graveyard_target(target)),
        )
        .expect("activate with the current graveyard target");

    move_graveyard_to_exile(&mut engine, 0, target);
    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(engine.state.objects[&target].zone, Zone::Exile);
    assert!(engine.state.players[0].exile.contains(&target));
    assert!(!engine.state.players[0].library.contains(&target));
    assert!(engine.state.stack.is_empty());
}
