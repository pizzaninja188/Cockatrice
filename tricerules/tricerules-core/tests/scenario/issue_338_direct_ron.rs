//! Issue #338 — command-boundary scenarios for the reviewed direct-RON additional-cost batch.
//!
//! Worthy Cost, Eaten Alive, Seize the Spoils, Duty Beyond Death and Arbiter of Woe were promoted
//! after complete-definition review against the pinned Scryfall snapshot. The exact records and
//! `rulings_uri` were fetched 2026-09-20; none returned rulings. Every expectation is the reviewed
//! printed Oracle behavior. Governing CR concepts: CR 118.8/601.2b/f-h (announced additional costs
//! and total cost), 701.16 (discard), 701.9 (sacrifice), 119.3/119.4 (life loss and gain),
//! 121.1 (draw), 111.10a (Treasure), 608.2c (printed instruction order), 613 layer 6 and 122.1
//! (keyword grants and counters), 603.6a and 700.2 (entry triggers and player-set recipients).

use super::helpers::*;
use tricerules_cards::{CounterKind, Keyword};
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::{
    cast_cost_group_selection::SelectedObject, CastCostGroupSelection, CostObjectRefs,
};

fn object_ref(engine: &GameEngine, object_id: u32) -> tricerules_proto::ruled::v1::CostObjectRef {
    tricerules_proto::ruled::v1::CostObjectRef {
        object_id,
        zone_change_generation: engine
            .state
            .zone_change_generation
            .get(&object_id)
            .copied()
            .unwrap_or(0),
    }
}

fn object_cost(engine: &GameEngine, option_index: u32, objects: &[u32]) -> CastCostGroupSelection {
    CastCostGroupSelection {
        group_index: 0,
        option_index,
        battlefield_objects: Some(CostObjectRefs {
            objects: objects.iter().map(|oid| object_ref(engine, *oid)).collect(),
        }),
        ..Default::default()
    }
}

fn option(option_index: u32) -> CastCostGroupSelection {
    CastCostGroupSelection {
        group_index: 0,
        option_index,
        ..Default::default()
    }
}

fn discard_option(hand_index: usize) -> CastCostGroupSelection {
    CastCostGroupSelection {
        group_index: 0,
        option_index: 0,
        selected_object: Some(SelectedObject::HandIndex(hand_index as u32)),
        ..Default::default()
    }
}

fn receipt_option(engine: &GameEngine) -> String {
    engine
        .state
        .stack
        .last()
        .expect("spell on stack")
        .cast_cost_receipts[0]
        .option_id
        .as_ref()
        .expect("committed option id")
        .as_str()
        .to_string()
}

#[test]
fn issue_338_worthy_cost_pays_a_sacrifice_and_exiles_a_creature_or_planeswalker() {
    let decks = Some(vec![
        deck_with("swamp", &["worthy_cost", "grizzly_bears"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(338_101, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    let fodder = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let target = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let artifact = inject_permanent_on_battlefield(&mut engine, 1, "swiftfoot_boots");
    ensure_card_in_hand(&mut engine, 0, "worthy_cost");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            b: 1,
            ..Default::default()
        },
    );

    let slot = hand_index_for_card(&engine, 0, "worthy_cost");
    assert!(
        engine
            .apply_command(0, &cast_spell(slot, target_object(target)))
            .is_err(),
        "the mandatory sacrifice cannot be skipped"
    );

    // A noncreature, nonplaneswalker permanent is outside the target filter.
    let slot = hand_index_for_card(&engine, 0, "worthy_cost");
    assert!(
        engine
            .apply_command(
                0,
                &cast_spell_with_cast_cost_groups(
                    slot,
                    target_object(artifact),
                    vec![object_cost(&engine, 0, &[fodder])],
                ),
            )
            .is_err(),
        "an artifact is not a legal target"
    );

    let slot = hand_index_for_card(&engine, 0, "worthy_cost");
    engine
        .apply_command(
            0,
            &cast_spell_with_cast_cost_groups(
                slot,
                target_object(target),
                vec![object_cost(&engine, 0, &[fodder])],
            ),
        )
        .expect("sacrificing a creature pays the additional cost");
    assert_eq!(engine.state.objects[&fodder].zone, Zone::Graveyard);
    assert_eq!(receipt_option(&engine), "sacrifice_creature");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&target].zone, Zone::Exile);

    // A planeswalker is a legal target for the same clause.
    let decks = Some(vec![
        deck_with("swamp", &["worthy_cost", "grizzly_bears"]),
        deck_with("forest", &[]),
    ]);
    let mut walker_engine = GameEngine::new(338_102, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut walker_engine);
    let fodder = inject_creature_on_battlefield(&mut walker_engine, 0, "grizzly_bears");
    let walker = inject_permanent_on_battlefield(&mut walker_engine, 1, "jace_beleren");
    walker_engine
        .state
        .objects
        .get_mut(&walker)
        .expect("walker")
        .set_counter(CounterKind::Loyalty, 3);
    ensure_card_in_hand(&mut walker_engine, 0, "worthy_cost");
    give_mana(
        &mut walker_engine,
        0,
        ManaGift {
            b: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&walker_engine, 0, "worthy_cost");
    walker_engine
        .apply_command(
            0,
            &cast_spell_with_cast_cost_groups(
                slot,
                target_object(walker),
                vec![object_cost(&walker_engine, 0, &[fodder])],
            ),
        )
        .expect("a planeswalker is a legal target");
    resolve_entire_stack_two_player(&mut walker_engine);
    assert_eq!(walker_engine.state.objects[&walker].zone, Zone::Exile);
}

#[test]
fn issue_338_eaten_alive_accepts_either_payment_and_exiles() {
    // Sacrifice branch.
    let decks = Some(vec![
        deck_with("swamp", &["eaten_alive", "grizzly_bears"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(338_201, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    let fodder = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let target = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    ensure_card_in_hand(&mut engine, 0, "eaten_alive");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            b: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "eaten_alive");
    assert!(
        engine
            .apply_command(0, &cast_spell(slot, target_object(target)))
            .is_err(),
        "one announced additional cost is mandatory"
    );
    let slot = hand_index_for_card(&engine, 0, "eaten_alive");
    engine
        .apply_command(
            0,
            &cast_spell_with_cast_cost_groups(
                slot,
                target_object(target),
                vec![object_cost(&engine, 0, &[fodder])],
            ),
        )
        .expect("sacrificing a creature pays the additional cost");
    assert_eq!(receipt_option(&engine), "sacrifice_creature");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&target].zone, Zone::Exile);

    // {3}{B} branch.
    let decks = Some(vec![
        deck_with("swamp", &["eaten_alive", "grizzly_bears"]),
        deck_with("forest", &[]),
    ]);
    let mut mana_engine = GameEngine::new(338_202, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut mana_engine);
    let target = inject_creature_on_battlefield(&mut mana_engine, 1, "grizzly_bears");
    ensure_card_in_hand(&mut mana_engine, 0, "eaten_alive");
    give_mana(
        &mut mana_engine,
        0,
        ManaGift {
            b: 2,
            c: 3,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&mana_engine, 0, "eaten_alive");
    mana_engine
        .apply_command(
            0,
            &cast_spell_with_cast_cost_groups(slot, target_object(target), vec![option(1)]),
        )
        .expect("paying {3}{B} pays the additional cost");
    assert_eq!(receipt_option(&mana_engine), "pay_mana");
    resolve_entire_stack_two_player(&mut mana_engine);
    assert_eq!(mana_engine.state.objects[&target].zone, Zone::Exile);
}

#[test]
fn issue_338_seize_the_spoils_discards_and_draws_with_a_treasure() {
    let decks = Some(vec![
        deck_with("mountain", &["seize_the_spoils", "grizzly_bears"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(338_301, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    ensure_card_in_hand(&mut engine, 0, "seize_the_spoils");
    ensure_card_in_hand(&mut engine, 0, "grizzly_bears");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            r: 1,
            c: 2,
            ..Default::default()
        },
    );
    let hand_before = engine.state.players[0].hand.len();

    let slot = hand_index_for_card(&engine, 0, "seize_the_spoils");
    assert!(
        engine.apply_command(0, &cast_spell(slot, vec![])).is_err(),
        "the discard additional cost is mandatory"
    );

    let slot = hand_index_for_card(&engine, 0, "seize_the_spoils");
    let fodder_slot = hand_index_for_card(&engine, 0, "grizzly_bears");
    let fodder = engine.state.players[0].hand[fodder_slot];
    engine
        .apply_command(
            0,
            &cast_spell_with_cast_cost_groups(slot, vec![], vec![discard_option(fodder_slot)]),
        )
        .expect("discarding a card pays the additional cost");
    assert_eq!(engine.state.objects[&fodder].zone, Zone::Graveyard);
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        engine.state.players[0].hand.len(),
        hand_before - 2 + 2,
        "the cast card and the discarded card leave hand; two are drawn"
    );
    let treasure = battlefield_object_for_card(&engine, 0, "treasure");
    assert_eq!(
        engine.state.objects[&treasure].card_id, "treasure",
        "resolution creates the Treasure under the caster's control"
    );
}

#[test]
fn issue_338_duty_beyond_death_grants_indestructible_and_a_counter() {
    let decks = Some(vec![
        deck_with("plains", &["duty_beyond_death", "grizzly_bears"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(338_401, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    let fodder = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let own = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let opposing = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    ensure_card_in_hand(&mut engine, 0, "duty_beyond_death");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            w: 1,
            c: 1,
            ..Default::default()
        },
    );

    let slot = hand_index_for_card(&engine, 0, "duty_beyond_death");
    assert!(
        engine.apply_command(0, &cast_spell(slot, vec![])).is_err(),
        "the mandatory sacrifice cannot be skipped"
    );

    let slot = hand_index_for_card(&engine, 0, "duty_beyond_death");
    engine
        .apply_command(
            0,
            &cast_spell_with_cast_cost_groups(
                slot,
                vec![],
                vec![object_cost(&engine, 0, &[fodder])],
            ),
        )
        .expect("sacrificing a creature pays the additional cost");
    assert_eq!(engine.state.objects[&fodder].zone, Zone::Graveyard);
    resolve_entire_stack_two_player(&mut engine);

    assert!(
        engine.effective_has_keyword(own, Keyword::Indestructible),
        "each creature you control gains indestructible"
    );
    assert_eq!(engine.effective_power(own), Some(3));
    assert_eq!(engine.effective_toughness(own), Some(3));
    assert!(!engine.effective_has_keyword(opposing, Keyword::Indestructible));
    assert_eq!(engine.effective_power(opposing), Some(2));
    assert_eq!(engine.effective_toughness(opposing), Some(2));
}

#[test]
fn issue_338_arbiter_of_woe_etb_drains_each_opponent() {
    let decks = Some(vec![
        deck_with("swamp", &["arbiter_of_woe", "grizzly_bears"]),
        deck_with("forest", &["island"]),
    ]);
    let mut engine = GameEngine::new(338_601, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    let fodder = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    ensure_card_in_hand(&mut engine, 0, "arbiter_of_woe");
    let opponent_card = inject_card_into_hand(&mut engine, 1, "island");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            b: 2,
            c: 4,
            ..Default::default()
        },
    );
    let life_before = engine.state.players[0].life;
    let opponent_life_before = engine.state.players[1].life;

    let slot = hand_index_for_card(&engine, 0, "arbiter_of_woe");
    engine
        .apply_command(
            0,
            &cast_spell_with_cast_cost_groups(
                slot,
                vec![],
                vec![object_cost(&engine, 0, &[fodder])],
            ),
        )
        .expect("sacrificing a creature pays the additional cost");
    assert_eq!(engine.state.objects[&fodder].zone, Zone::Graveyard);
    pass_both_players(&mut engine);
    assert_eq!(
        engine.state.stack.len(),
        1,
        "the entry trigger is on the stack"
    );

    engine
        .apply_command(0, &pass())
        .expect("active player passes");
    let batch = engine
        .apply_command(1, &pass())
        .expect("resolve the trigger to the opponent's discard choice");
    let choice = find_resolution_choice(&batch).expect("each-opponent discard choice");
    assert_eq!(choice.deciding_player_id, 1);
    assert!(choice.candidate_object_ids.contains(&opponent_card));
    engine
        .apply_command(1, &submit_resolution_choice(vec![opponent_card]))
        .expect("the opponent discards a card");

    assert_eq!(engine.state.objects[&opponent_card].zone, Zone::Graveyard);
    assert_eq!(engine.state.players[1].life, opponent_life_before - 2);
    assert_eq!(engine.state.players[0].life, life_before + 2);
}
