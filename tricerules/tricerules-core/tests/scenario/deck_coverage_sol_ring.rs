use super::helpers::*;
use tricerules_core::GameEngine;

#[test]
fn sol_ring_casts_then_its_tap_ability_adds_two_colorless_mana() {
    let mut engine = GameEngine::new(
        20_260_925,
        &[0, 1],
        20,
        Some(vec![deck_with("island", &["sol_ring"]), forest_only_deck()]),
        true,
    )
    .expect("Sol Ring is registered");
    advance_to_main1_from_game_start(&mut engine);
    ensure_card_in_hand(&mut engine, 0, "sol_ring");
    engine.state.players[0].mana_pool.colorless = 1;

    let ring_in_hand = hand_index_for_card(&engine, 0, "sol_ring");
    engine
        .apply_command(0, &cast_spell(ring_in_hand, vec![]))
        .expect("cast Sol Ring for {1}");
    resolve_entire_stack_two_player(&mut engine);
    let ring = battlefield_object_for_card(&engine, 0, "sol_ring");
    assert_eq!(engine.state.players[0].mana_pool.colorless, 0);

    apply_ability(&mut engine, 0, ring, 0, vec![])
        .expect("Sol Ring's mana ability is legal while untapped");
    assert_eq!(engine.state.players[0].mana_pool.colorless, 2);
    assert!(engine.state.objects[&ring].tapped);
    assert!(
        engine.state.stack.is_empty(),
        "mana ability does not use the stack"
    );

    let mana_after_first_activation = engine.state.players[0].mana_pool.colorless;
    assert!(apply_ability(&mut engine, 0, ring, 0, vec![]).is_err());
    assert_eq!(
        engine.state.players[0].mana_pool.colorless, mana_after_first_activation,
        "a rejected activation while tapped produces no mana"
    );
}
