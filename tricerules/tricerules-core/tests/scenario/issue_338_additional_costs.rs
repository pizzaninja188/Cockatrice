//! Issue #338 — command-boundary scenarios for the reviewed additional-cost cast assembly.
//!
//! Every expectation is the reviewed printed Oracle behavior. Exact Scryfall records and
//! `rulings_uri` responses were fetched 2026-09-20 into `build/campaign/scryfall-batch338/`; the
//! returned rulings were reviewed and are consistent with these assertions (Corrupted Conviction
//! requires exactly one sacrifice; Guardian of the Great Door may tap any four untapped
//! artifacts/creatures/lands; Bogslither's Embrace places all -1/-1 counters on one creature and
//! cannot be chosen without a creature; Laughing Mad still pays additional costs on a flashback
//! cast). Governing CR concepts: CR 601.2b (announced additional costs), 601.2f (total cost),
//! 701.4 (behold), 701.30 (blight), 701.16 (discard), and 602.5 (announced cost receipts).

use super::helpers::*;
use tricerules_cards::CounterKind;
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
    hand_option(0, hand_index)
}

fn hand_option(option_index: u32, hand_index: usize) -> CastCostGroupSelection {
    CastCostGroupSelection {
        group_index: 0,
        option_index,
        selected_object: Some(SelectedObject::HandIndex(hand_index as u32)),
        ..Default::default()
    }
}

fn permanent_option(
    engine: &GameEngine,
    option_index: u32,
    object_id: u32,
) -> CastCostGroupSelection {
    CastCostGroupSelection {
        group_index: 0,
        option_index,
        selected_object: Some(SelectedObject::PermanentId(object_id)),
        expected_zone_change_generation: engine
            .state
            .zone_change_generation
            .get(&object_id)
            .copied()
            .unwrap_or(0),
        ..Default::default()
    }
}

#[test]
fn corrupted_conviction_requires_a_creature_sacrifice_and_draws_two() {
    let decks = Some(vec![
        deck_with("swamp", &["corrupted_conviction", "grizzly_bears"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(338_001, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    let fodder = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    ensure_card_in_hand(&mut engine, 0, "corrupted_conviction");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            b: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "corrupted_conviction");
    assert!(
        engine.apply_command(0, &cast_spell(slot, vec![])).is_err(),
        "the mandatory sacrifice cannot be skipped"
    );
    let slot = hand_index_for_card(&engine, 0, "corrupted_conviction");
    let hand_before = engine.state.players[0].hand.len();
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
    let receipt = &engine
        .state
        .stack
        .last()
        .expect("spell on stack")
        .cast_cost_receipts[0];
    assert_eq!(
        receipt.option_id.as_ref().expect("option id").as_str(),
        "sacrifice_creature"
    );
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        engine.state.players[0].hand.len(),
        hand_before - 1 + 2,
        "the spell resolves and draws two cards"
    );
}

#[test]
fn pumpkin_bombardment_supports_the_mana_and_discard_options() {
    // Mana option: pay the printed {2} on top of the spell's mana cost.
    let decks = Some(vec![
        deck_with("mountain", &["pumpkin_bombardment", "grizzly_bears"]),
        deck_with("forest", &["grizzly_bears"]),
    ]);
    let mut engine = GameEngine::new(338_002, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    let target = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    ensure_card_in_hand(&mut engine, 0, "pumpkin_bombardment");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            r: 1,
            c: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "pumpkin_bombardment");
    engine
        .apply_command(
            0,
            &cast_spell_with_cast_cost_groups(slot, target_object(target), vec![option(1)]),
        )
        .expect("paying {2} pays the additional cost");
    let receipt = &engine
        .state
        .stack
        .last()
        .expect("spell on stack")
        .cast_cost_receipts[0];
    assert_eq!(
        receipt.option_id.as_ref().expect("option id").as_str(),
        "pay_mana"
    );
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&target].zone, Zone::Graveyard);

    // Discard option: the same spell instead discards a card from hand.
    let decks = Some(vec![
        deck_with("mountain", &["pumpkin_bombardment", "grizzly_bears"]),
        deck_with("forest", &["grizzly_bears"]),
    ]);
    let mut engine = GameEngine::new(338_003, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    let target = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    ensure_card_in_hand(&mut engine, 0, "pumpkin_bombardment");
    ensure_card_in_hand(&mut engine, 0, "grizzly_bears");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "pumpkin_bombardment");
    let fodder_slot = hand_index_for_card(&engine, 0, "grizzly_bears");
    let fodder = engine.state.players[0].hand[fodder_slot];
    engine
        .apply_command(
            0,
            &cast_spell_with_cast_cost_groups(
                slot,
                target_object(target),
                vec![discard_option(fodder_slot)],
            ),
        )
        .expect("discarding a card pays the additional cost");
    assert_eq!(engine.state.objects[&fodder].zone, Zone::Graveyard);
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&target].zone, Zone::Graveyard);
}

#[test]
fn guardian_of_the_great_door_taps_exactly_four_permanents() {
    let decks = Some(vec![
        deck_with("plains", &["guardian_of_the_great_door", "grizzly_bears"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(338_004, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    let creatures: Vec<u32> = (0..3)
        .map(|_| inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears"))
        .collect();
    let land = inject_permanent_on_battlefield(&mut engine, 0, "plains");
    let mut tapped = creatures.clone();
    tapped.push(land);
    ensure_card_in_hand(&mut engine, 0, "guardian_of_the_great_door");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            w: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "guardian_of_the_great_door");
    assert!(
        engine
            .apply_command(
                0,
                &cast_spell_with_cast_cost_groups(
                    slot,
                    vec![],
                    vec![object_cost(&engine, 0, &tapped[..3])],
                ),
            )
            .is_err(),
        "exactly four permanents must be tapped"
    );
    let slot = hand_index_for_card(&engine, 0, "guardian_of_the_great_door");
    engine
        .apply_command(
            0,
            &cast_spell_with_cast_cost_groups(slot, vec![], vec![object_cost(&engine, 0, &tapped)]),
        )
        .expect("tapping four permanents pays the additional cost");
    for object_id in &tapped {
        assert!(
            engine.state.objects[object_id].tapped,
            "each selected permanent is tapped"
        );
    }
    resolve_entire_stack_two_player(&mut engine);
    let guardian = battlefield_object_for_card(&engine, 0, "guardian_of_the_great_door");
    assert!(engine.effective_has_keyword(guardian, tricerules_cards::Keyword::Flying));
}

#[test]
fn bogslithers_embrace_blights_a_creature_instead_of_paying_mana() {
    let decks = Some(vec![
        deck_with("swamp", &["bogslithers_embrace", "grizzly_bears"]),
        deck_with("forest", &["grizzly_bears"]),
    ]);
    let mut engine = GameEngine::new(338_005, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    let blighted = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let target = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    ensure_card_in_hand(&mut engine, 0, "bogslithers_embrace");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            b: 1,
            c: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "bogslithers_embrace");
    engine
        .apply_command(
            0,
            &cast_spell_with_cast_cost_groups(
                slot,
                target_object(target),
                vec![permanent_option(&engine, 0, blighted)],
            ),
        )
        .expect("blighting a creature pays the additional cost");
    let receipt = &engine
        .state
        .stack
        .last()
        .expect("spell on stack")
        .cast_cost_receipts[0];
    assert_eq!(
        receipt.option_id.as_ref().expect("option id").as_str(),
        "blight"
    );
    assert_eq!(
        engine.state.objects[&blighted]
            .counters
            .get(&CounterKind::MinusOneMinusOne),
        Some(&1)
    );
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&target].zone, Zone::Exile);
}

#[test]
fn deadly_precision_pays_four_mana_or_sacrifices() {
    let decks = Some(vec![
        deck_with("swamp", &["deadly_precision", "grizzly_bears"]),
        deck_with("forest", &["grizzly_bears"]),
    ]);
    let mut engine = GameEngine::new(338_006, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    let target = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    ensure_card_in_hand(&mut engine, 0, "deadly_precision");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            b: 1,
            c: 4,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "deadly_precision");
    engine
        .apply_command(
            0,
            &cast_spell_with_cast_cost_groups(slot, target_object(target), vec![option(0)]),
        )
        .expect("paying {4} pays the additional cost");
    let receipt = &engine
        .state
        .stack
        .last()
        .expect("spell on stack")
        .cast_cost_receipts[0];
    assert_eq!(
        receipt.option_id.as_ref().expect("option id").as_str(),
        "pay_mana"
    );
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&target].zone, Zone::Graveyard);

    // The alternative sacrifice option is published for the same group.
    let decks = Some(vec![
        deck_with("swamp", &["deadly_precision", "grizzly_bears"]),
        deck_with("forest", &["grizzly_bears"]),
    ]);
    let mut engine = GameEngine::new(338_007, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    let target = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let fodder = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    ensure_card_in_hand(&mut engine, 0, "deadly_precision");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            b: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "deadly_precision");
    engine
        .apply_command(
            0,
            &cast_spell_with_cast_cost_groups(
                slot,
                target_object(target),
                vec![object_cost(&engine, 1, &[fodder])],
            ),
        )
        .expect("sacrificing an artifact or creature pays the additional cost");
    assert_eq!(engine.state.objects[&fodder].zone, Zone::Graveyard);
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&target].zone, Zone::Graveyard);
}

#[test]
fn kinsbaile_aspirant_beholds_a_kithkin_permanent() {
    let decks = Some(vec![
        deck_with("plains", &["kinsbaile_aspirant", "grizzly_bears"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(338_008, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    let kithkin = inject_creature_on_battlefield(&mut engine, 0, "kinsbaile_aspirant");
    let non_kithkin = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    ensure_card_in_hand(&mut engine, 0, "kinsbaile_aspirant");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            w: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "kinsbaile_aspirant");
    assert!(
        engine
            .apply_command(
                0,
                &cast_spell_with_cast_cost_groups(
                    slot,
                    vec![],
                    vec![permanent_option(&engine, 0, non_kithkin)],
                ),
            )
            .is_err(),
        "only a Kithkin can be beheld"
    );
    let slot = hand_index_for_card(&engine, 0, "kinsbaile_aspirant");
    engine
        .apply_command(
            0,
            &cast_spell_with_cast_cost_groups(
                slot,
                vec![],
                vec![permanent_option(&engine, 0, kithkin)],
            ),
        )
        .expect("beholding a Kithkin pays the additional cost");
    let receipt = &engine
        .state
        .stack
        .last()
        .expect("spell on stack")
        .cast_cost_receipts[0];
    assert_eq!(
        receipt.option_id.as_ref().expect("option id").as_str(),
        "behold"
    );
    assert_eq!(
        engine.state.objects[&kithkin].zone,
        Zone::Battlefield,
        "beholding does not sacrifice the chosen permanent"
    );
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        engine.state.players[0]
            .battlefield
            .iter()
            .filter(|oid| engine.state.objects[oid].card_id == "kinsbaile_aspirant")
            .count(),
        2,
        "the cast Aspirant enters alongside the beheld one"
    );
}

#[test]
fn demand_answers_sacrifices_an_artifact_or_discards_a_card() {
    // Artifact branch.
    let decks = Some(vec![
        deck_with("mountain", &["demand_answers", "grizzly_bears"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(338_009, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    let artifact = inject_permanent_on_battlefield(&mut engine, 0, "swiftfoot_boots");
    ensure_card_in_hand(&mut engine, 0, "demand_answers");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            r: 1,
            c: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "demand_answers");
    let hand_before = engine.state.players[0].hand.len();
    engine
        .apply_command(
            0,
            &cast_spell_with_cast_cost_groups(
                slot,
                vec![],
                vec![object_cost(&engine, 0, &[artifact])],
            ),
        )
        .expect("sacrificing an artifact pays the additional cost");
    assert_eq!(engine.state.objects[&artifact].zone, Zone::Graveyard);
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.players[0].hand.len(), hand_before - 1 + 2);

    // Discard branch.
    let decks = Some(vec![
        deck_with("mountain", &["demand_answers", "grizzly_bears"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(338_010, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    ensure_card_in_hand(&mut engine, 0, "demand_answers");
    ensure_card_in_hand(&mut engine, 0, "grizzly_bears");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            r: 1,
            c: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "demand_answers");
    let fodder_slot = hand_index_for_card(&engine, 0, "grizzly_bears");
    let fodder = engine.state.players[0].hand[fodder_slot];
    engine
        .apply_command(
            0,
            &cast_spell_with_cast_cost_groups(slot, vec![], vec![hand_option(1, fodder_slot)]),
        )
        .expect("discarding a card pays the additional cost");
    assert_eq!(engine.state.objects[&fodder].zone, Zone::Graveyard);
}

#[test]
fn fear_of_exposure_taps_exactly_two_creatures_or_lands() {
    let decks = Some(vec![
        deck_with("forest", &["fear_of_exposure", "grizzly_bears"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(338_011, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    let creature = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let land = inject_permanent_on_battlefield(&mut engine, 0, "forest");
    ensure_card_in_hand(&mut engine, 0, "fear_of_exposure");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            g: 1,
            c: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "fear_of_exposure");
    assert!(
        engine
            .apply_command(
                0,
                &cast_spell_with_cast_cost_groups(
                    slot,
                    vec![],
                    vec![object_cost(&engine, 0, &[creature])],
                ),
            )
            .is_err(),
        "exactly two permanents must be tapped"
    );
    let slot = hand_index_for_card(&engine, 0, "fear_of_exposure");
    engine
        .apply_command(
            0,
            &cast_spell_with_cast_cost_groups(
                slot,
                vec![],
                vec![object_cost(&engine, 0, &[creature, land])],
            ),
        )
        .expect("tapping a creature and a land pays the additional cost");
    assert!(engine.state.objects[&creature].tapped);
    assert!(engine.state.objects[&land].tapped);
}

#[test]
fn final_vengeance_sacrifices_a_creature_and_exiles_the_target() {
    let decks = Some(vec![
        deck_with("swamp", &["final_vengeance", "grizzly_bears"]),
        deck_with("forest", &["grizzly_bears"]),
    ]);
    let mut engine = GameEngine::new(338_012, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    let fodder = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let target = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    ensure_card_in_hand(&mut engine, 0, "final_vengeance");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            b: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "final_vengeance");
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
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&target].zone, Zone::Exile);
}

#[test]
fn laughing_mad_discards_a_card_and_draws_two() {
    let decks = Some(vec![
        deck_with("mountain", &["laughing_mad", "grizzly_bears"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(338_013, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    ensure_card_in_hand(&mut engine, 0, "laughing_mad");
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
    let slot = hand_index_for_card(&engine, 0, "laughing_mad");
    let fodder_slot = hand_index_for_card(&engine, 0, "grizzly_bears");
    let fodder = engine.state.players[0].hand[fodder_slot];
    let hand_before = engine.state.players[0].hand.len();
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
        "the discarded card and the cast card leave hand, then two are drawn"
    );
}
