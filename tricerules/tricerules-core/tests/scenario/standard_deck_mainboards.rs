use super::helpers::*;
use tricerules_cards::CounterKind;
use tricerules_proto::ruled::v1::{
    cast_cost_group_selection::SelectedObject, CastCostGroupSelection,
};

#[test]
fn dream_beavers_resolves_its_entire_etb_sequence() {
    let mut engine = GameEngine::new(
        2026090201,
        &[0, 1],
        20,
        Some(vec![
            deck_with("swamp", &["dream_beavers"]),
            deck_with("island", &[]),
        ]),
        true,
    )
    .expect("new game");
    advance_to_main1_from_game_start(&mut engine);

    move_ready_to_battlefield(&mut engine, 0, "dream_beavers");
    assert_eq!(engine.state.stack.len(), 1, "ETB trigger is on the stack");
    pass_both_players(&mut engine);

    assert_eq!(engine.state.players[0].life, 21);
    assert_eq!(engine.state.players[1].life, 19);
    assert!(
        engine.state.pending_resolution.is_some(),
        "scry choice is pending"
    );
    engine
        .apply_command(0, &submit_resolution_choice(vec![]))
        .expect("put the looked-at card on the bottom");
    assert!(engine.state.pending_resolution.is_none());
}

#[test]
fn sazhs_chocobo_gets_a_counter_for_its_controllers_landfall() {
    let mut engine = GameEngine::new(
        2026090202,
        &[0, 1],
        20,
        Some(vec![
            deck_with("forest", &["sazhs_chocobo"]),
            deck_with("island", &[]),
        ]),
        true,
    )
    .expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    let chocobo = move_ready_to_battlefield(&mut engine, 0, "sazhs_chocobo");
    ensure_card_in_hand(&mut engine, 0, "forest");
    let forest = hand_index_for_card(&engine, 0, "forest");

    engine
        .apply_command(0, &play_land(forest))
        .expect("play a Forest");
    assert_eq!(
        engine.state.stack.len(),
        1,
        "landfall trigger is on the stack"
    );
    pass_both_players(&mut engine);

    assert_eq!(
        engine.state.objects[&chocobo].counter_count(CounterKind::PlusOnePlusOne),
        1
    );
}

#[test]
fn requiting_hex_rewards_the_optional_blight_payment() {
    let mut engine = GameEngine::new(
        2026090203,
        &[0, 1],
        20,
        Some(vec![
            deck_with("swamp", &["requiting_hex", "grizzly_bears"]),
            deck_with("island", &["grizzly_bears"]),
        ]),
        true,
    )
    .expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    ensure_card_in_hand(&mut engine, 0, "requiting_hex");
    let blighted = move_ready_to_battlefield(&mut engine, 0, "grizzly_bears");
    let target = move_ready_to_battlefield(&mut engine, 1, "grizzly_bears");
    engine.state.players[0].mana_pool.black = 1;
    let generation = engine
        .state
        .zone_change_generation
        .get(&blighted)
        .copied()
        .unwrap_or(0);
    let slot = hand_index_for_card(&engine, 0, "requiting_hex");

    engine
        .apply_command(
            0,
            &cast_spell_with_cast_cost_groups(
                slot,
                target_object(target),
                vec![CastCostGroupSelection {
                    group_index: 0,
                    option_index: 0,
                    selected_object: Some(SelectedObject::PermanentId(blighted)),
                    expected_zone_change_generation: generation,
                    battlefield_objects: None,
                }],
            ),
        )
        .expect("cast Requiting Hex with blight");
    pass_both_players(&mut engine);

    assert_eq!(engine.state.players[0].life, 22);
    assert!(engine.state.players[1].graveyard.contains(&target));
    assert_eq!(
        engine.state.objects[&blighted].counter_count(CounterKind::MinusOneMinusOne),
        1
    );
}
