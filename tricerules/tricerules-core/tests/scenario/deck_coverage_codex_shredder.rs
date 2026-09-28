//! Frozen four-deck coverage for Codex Shredder.
//!
//! Oracle and rulings checked against Scryfall on 2026-09-28. The ruling clarifies that the
//! recovery target is chosen before costs, so Codex Shredder cannot target itself. CR 701.17a-b
//! govern milling; CR 602.2b and 601.2c/601.2h govern choosing targets before paying activation
//! costs; CR 701.21a governs sacrificing the source as a cost.

use super::helpers::*;
use tricerules_core::{GameEngine, Zone};

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
fn codex_shredder_mills_one_card_from_target_player() {
    let mut engine = engine(20_260_930);
    let shredder = inject_permanent_on_battlefield(&mut engine, 0, "codex_shredder");
    let opponent = engine.state.players[1].id;
    let milled = *engine.state.players[1]
        .library
        .front()
        .expect("opponent library is nonempty");
    let library_size = engine.state.players[1].library.len();
    let graveyard_size = engine.state.players[1].graveyard.len();

    apply_ability(&mut engine, 0, shredder, 0, target_player(opponent))
        .expect("activate Codex Shredder targeting an opponent");
    assert!(engine.state.objects[&shredder].tapped);
    assert_eq!(engine.state.stack.len(), 1);

    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(engine.state.players[1].library.len(), library_size - 1);
    assert_eq!(engine.state.players[1].graveyard.len(), graveyard_size + 1);
    assert_eq!(engine.state.objects[&milled].zone, Zone::Graveyard);
}

#[test]
fn codex_shredder_recovers_any_own_graveyard_card_after_sacrificing() {
    let mut engine = engine(20_260_931);
    let shredder = inject_permanent_on_battlefield(&mut engine, 0, "codex_shredder");
    let own_creature = inject_graveyard_card(&mut engine, 0, "grizzly_bears");
    let opponent_card = inject_graveyard_card(&mut engine, 1, "chromatic_star");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 5,
            ..Default::default()
        },
    );

    for illegal_target in [opponent_card, shredder] {
        engine
            .apply_command(
                0,
                &activate_ability_for(&engine, shredder, 1, graveyard_target(illegal_target)),
            )
            .expect_err("only a card in the controller's graveyard is a legal target");
        assert_eq!(engine.state.objects[&shredder].zone, Zone::Battlefield);
        assert!(!engine.state.objects[&shredder].tapped);
        assert_eq!(engine.state.players[0].mana_pool.colorless, 5);
        assert!(engine.state.stack.is_empty());
    }

    apply_ability(&mut engine, 0, shredder, 1, graveyard_target(own_creature))
        .expect("pay {5}, tap, and sacrifice Codex Shredder");
    assert_eq!(engine.state.players[0].mana_pool.colorless, 0);
    assert_eq!(engine.state.objects[&shredder].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&own_creature].zone, Zone::Graveyard);
    assert_eq!(engine.state.stack.len(), 1);

    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(engine.state.objects[&own_creature].zone, Zone::Hand);
    assert!(engine.state.players[0].hand.contains(&own_creature));
    assert_eq!(engine.state.objects[&opponent_card].zone, Zone::Graveyard);
}
