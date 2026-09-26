//! Actual-card coverage for Voltaic Key's `{1}, {T}: Untap target artifact` ability.

use super::helpers::*;

fn key_engine(seed: u64) -> (GameEngine, u32) {
    let decks = Some(vec![
        deck_with("island", &["voltaic_key"]),
        island_only_deck(),
    ]);
    let mut engine = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    let key = relocate_to_battlefield(&mut engine, 0, "voltaic_key", false);
    (engine, key)
}

fn target(object_id: u32) -> Vec<TargetRef> {
    vec![TargetRef {
        object_id,
        damage_amount: 0,
        group_index: 0,
        kind: 0,
    }]
}

fn set_tapped(engine: &mut GameEngine, object_id: u32, tapped: bool) {
    engine
        .state
        .objects
        .get_mut(&object_id)
        .expect("object")
        .tapped = tapped;
}

fn is_tapped(engine: &GameEngine, object_id: u32) -> bool {
    engine.state.objects.get(&object_id).expect("object").tapped
}

#[test]
fn voltaic_key_pays_and_taps_as_cost_then_untaps_an_opponents_artifact() {
    let (mut engine, key) = key_engine(202_609_261);
    let target_key = inject_permanent_on_battlefield(&mut engine, 1, "voltaic_key");
    set_tapped(&mut engine, target_key, true);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );

    engine
        .apply_command(0, &activate_ability(key, 0, target(target_key)))
        .expect("activate targeting an opponent's artifact");
    assert!(
        is_tapped(&engine, key),
        "the tap cost is paid on activation"
    );
    assert!(
        is_tapped(&engine, target_key),
        "the target waits for resolution"
    );
    assert_eq!(engine.state.players[0].mana_pool.colorless, 0);

    resolve_entire_stack_two_player(&mut engine);

    assert!(
        is_tapped(&engine, key),
        "resolving does not untap the source"
    );
    assert!(
        !is_tapped(&engine, target_key),
        "the target artifact is untapped"
    );
}

#[test]
fn voltaic_key_can_target_itself_and_untaps_after_paying_its_tap_cost() {
    let (mut engine, key) = key_engine(202_609_262);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );

    engine
        .apply_command(0, &activate_ability(key, 0, target(key)))
        .expect("the artifact may target itself");
    assert!(
        is_tapped(&engine, key),
        "the tap cost is paid before resolution"
    );

    resolve_entire_stack_two_player(&mut engine);

    assert!(
        !is_tapped(&engine, key),
        "the legal target is untapped on resolution"
    );
}

#[test]
fn voltaic_key_may_target_an_already_untapped_artifact() {
    let (mut engine, key) = key_engine(202_609_263);
    let target_key = inject_permanent_on_battlefield(&mut engine, 1, "voltaic_key");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );

    engine
        .apply_command(0, &activate_ability(key, 0, target(target_key)))
        .expect("an untapped artifact remains a legal target");
    resolve_entire_stack_two_player(&mut engine);

    assert!(is_tapped(&engine, key), "the activation cost was paid");
    assert!(
        !is_tapped(&engine, target_key),
        "untapping an untapped target is a no-op"
    );
}

#[test]
fn voltaic_key_rejects_a_nonartifact_without_paying_any_cost() {
    let (mut engine, key) = key_engine(202_609_264);
    let land = inject_permanent_on_battlefield(&mut engine, 1, "forest");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );

    let err = engine
        .apply_command(0, &activate_ability(key, 0, target(land)))
        .expect_err("a land without the artifact type is not a legal target");

    assert!(matches!(err, tricerules_core::EngineError::Illegal(_)));
    assert!(
        !is_tapped(&engine, key),
        "rejected activation does not pay the tap cost"
    );
    assert_eq!(engine.state.players[0].mana_pool.colorless, 1);
}
