use super::helpers::*;
use tricerules_core::{GameEngine, Zone};

fn two_player_engine(seed: u64) -> GameEngine {
    let mut engine = GameEngine::new(
        seed,
        &[0, 1],
        20,
        Some(vec![deck_with("island", &[]), deck_with("forest", &[])]),
        true,
    )
    .expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn generic_cost_reduction(engine: &mut GameEngine, card_id: &str) -> u32 {
    let hand_index = hand_index_for_card(engine, 0, card_id) as u32;
    let batch = engine.initial_response_batch();
    batch.legal_by_player[&0]
        .hand_actions
        .iter()
        .find(|action| action.hand_index == hand_index)
        .unwrap_or_else(|| panic!("missing cast action for {card_id}"))
        .generic_cost_reduction
}

#[test]
fn both_wipes_count_creatures_on_every_players_battlefield_and_keep_colored_costs() {
    let mut act = two_player_engine(20_260_951);
    inject_card_into_hand(&mut act, 0, "blasphemous_act");
    for _ in 0..4 {
        inject_creature_on_battlefield(&mut act, 0, "grizzly_bears");
    }
    for _ in 0..5 {
        inject_creature_on_battlefield(&mut act, 1, "grizzly_bears");
    }
    give_mana(
        &mut act,
        0,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    assert_eq!(generic_cost_reduction(&mut act, "blasphemous_act"), 9);
    act.apply_command(
        0,
        &cast_spell(hand_index_for_card(&act, 0, "blasphemous_act"), vec![]),
    )
    .expect("nine creatures reduce the generic component to zero; {R} remains");
    assert_eq!(act.state.players[0].mana_pool.red, 0);
    assert_eq!(act.state.players[0].mana_pool.colorless, 0);

    let mut horde = two_player_engine(20_260_952);
    inject_card_into_hand(&mut horde, 0, "vanquish_the_horde");
    for _ in 0..2 {
        inject_creature_on_battlefield(&mut horde, 0, "grizzly_bears");
    }
    for _ in 0..5 {
        inject_creature_on_battlefield(&mut horde, 1, "grizzly_bears");
    }
    give_mana(
        &mut horde,
        0,
        ManaGift {
            w: 2,
            ..Default::default()
        },
    );
    assert_eq!(generic_cost_reduction(&mut horde, "vanquish_the_horde"), 7);
    horde
        .apply_command(
            0,
            &cast_spell(hand_index_for_card(&horde, 0, "vanquish_the_horde"), vec![]),
        )
        .expect("seven creatures reduce the generic component to zero; {W}{W} remains");
    assert_eq!(horde.state.players[0].mana_pool.white, 0);
    assert_eq!(horde.state.players[0].mana_pool.colorless, 0);
}

#[test]
fn blasphemous_act_deals_thirteen_to_each_creature_and_leaves_a_fourteen_toughness_one_alive() {
    let mut engine = two_player_engine(20_260_953);
    inject_card_into_hand(&mut engine, 0, "blasphemous_act");
    let small = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let opponent_small = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let large = inject_creature_with_stats(&mut engine, 1, "grizzly_bears", 14, 14);
    grant_pool(&mut engine, 0);

    engine
        .apply_command(
            0,
            &cast_spell(hand_index_for_card(&engine, 0, "blasphemous_act"), vec![]),
        )
        .expect("cast Blasphemous Act");
    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(engine.state.objects[&small].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&opponent_small].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&large].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&large].damage, 13);
}

#[test]
fn vanquish_the_horde_destroys_creatures_controlled_by_both_players_only() {
    let mut engine = two_player_engine(20_260_954);
    inject_card_into_hand(&mut engine, 0, "vanquish_the_horde");
    let own_creature = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let opposing_creature = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let land = inject_permanent_on_battlefield(&mut engine, 1, "forest");
    grant_pool(&mut engine, 0);

    engine
        .apply_command(
            0,
            &cast_spell(
                hand_index_for_card(&engine, 0, "vanquish_the_horde"),
                vec![],
            ),
        )
        .expect("cast Vanquish the Horde");
    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(engine.state.objects[&own_creature].zone, Zone::Graveyard);
    assert_eq!(
        engine.state.objects[&opposing_creature].zone,
        Zone::Graveyard
    );
    assert_eq!(engine.state.objects[&land].zone, Zone::Battlefield);
}
