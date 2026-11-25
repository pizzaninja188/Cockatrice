//! Actual-card scenarios for the queue-v80 03-kicker-spells cohort.
//!
//! Expectations are printed Oracle values. CR 702.33a-e (kicker), 601.2b/f-h (additional costs),
//! 608.2b (target revalidation), 400.3 (owner's zone), and 121.1-121.2 (draw) govern. The pinned
//! 2018-04-27 and 2024-11-08 rulings confirm a kicked spell is kicked, a copied kicked spell is
//! kicked, and an illegal sole target prevents Blink of an Eye's kicked draw.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::{CastCostGroupSelection, CostObjectRefs};

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![
        deck_with("swamp", &["grizzly_bears", "autochthon_wurm"]),
        deck_with("island", &["grizzly_bears", "autochthon_wurm"]),
    ]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn mana_kicker() -> Vec<CastCostGroupSelection> {
    vec![CastCostGroupSelection {
        group_index: 0,
        option_index: 0,
        ..Default::default()
    }]
}

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

fn sacrifice_kicker(engine: &GameEngine, object: u32) -> Vec<CastCostGroupSelection> {
    vec![CastCostGroupSelection {
        group_index: 0,
        option_index: 0,
        battlefield_objects: Some(CostObjectRefs {
            objects: vec![object_ref(engine, object)],
        }),
        ..Default::default()
    }]
}

#[test]
fn q80_kicker_canary_simplest_roil_eruption_deals_three_or_five() {
    for (index, kicked) in [false, true].into_iter().enumerate() {
        let mut e = engine(80_300_000 + index as u64);
        inject_card_into_hand(&mut e, 0, "roil_eruption");
        give_mana(
            &mut e,
            0,
            ManaGift {
                r: 1,
                c: if kicked { 6 } else { 1 },
                ..Default::default()
            },
        );
        let slot = hand_index_for_card(&e, 0, "roil_eruption");
        let selections = if kicked { mana_kicker() } else { Vec::new() };
        e.apply_scenario_command(
            0,
            &cast_spell_with_cast_cost_groups(slot, target_player(1), selections),
        )
        .expect("cast Roil Eruption");
        let stack = e.state.stack.last().expect("spell on the stack");
        assert_eq!(stack.cast_cost_receipts.len(), usize::from(kicked));
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.players[1].life,
            if kicked { 15 } else { 17 },
            "kicked={kicked}"
        );
        assert_eq!(
            e.state.players[0].mana_pool.red, 0,
            "the printed red cost is spent"
        );
        assert_eq!(
            e.state.players[0].mana_pool.colorless, 0,
            "the printed generic cost is spent"
        );
    }
}

#[test]
fn q80_kicker_canary_hardest_final_flourish_pays_sacrifice_for_six() {
    let mut e = engine(80_300_101);
    let bears = move_ready_to_battlefield(&mut e, 0, "grizzly_bears");
    let target = move_ready_to_battlefield(&mut e, 1, "autochthon_wurm");
    inject_card_into_hand(&mut e, 0, "final_flourish");
    give_mana(
        &mut e,
        0,
        ManaGift {
            b: 1,
            c: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&e, 0, "final_flourish");
    e.apply_scenario_command(
        0,
        &cast_spell_with_cast_cost_groups(slot, target_object(target), sacrifice_kicker(&e, bears)),
    )
    .expect("cast kicked Final Flourish");
    assert_eq!(
        e.state.objects[&bears].zone,
        Zone::Graveyard,
        "the sacrificed creature is paid as a cost"
    );
    resolve_entire_stack_two_player(&mut e);
    assert!(e.state.stack.is_empty(), "the spell finished resolving");
    assert_eq!(
        e.effective_power(target),
        Some(3),
        "Autochthon Wurm is 9/14"
    );
    assert_eq!(e.effective_toughness(target), Some(8), "kicked is -6/-6");
    assert_eq!(e.state.objects[&target].zone, Zone::Battlefield);
}

#[test]
fn q80_kicker_cards_apply_base_and_kicked_modes() {
    for (index, card) in [
        "roil_eruption",
        "blink_of_an_eye",
        "final_flourish",
        "eject_the_warp_core",
    ]
    .into_iter()
    .enumerate()
    {
        for kicked in [false, true] {
            let mut e = engine(80_301_000 + (index * 2 + usize::from(kicked)) as u64);
            match card {
                "roil_eruption" => {
                    inject_card_into_hand(&mut e, 0, card);
                    give_mana(
                        &mut e,
                        0,
                        ManaGift {
                            r: 1,
                            c: if kicked { 6 } else { 1 },
                            ..Default::default()
                        },
                    );
                    let slot = hand_index_for_card(&e, 0, card);
                    let selections = if kicked { mana_kicker() } else { Vec::new() };
                    e.apply_scenario_command(
                        0,
                        &cast_spell_with_cast_cost_groups(slot, target_player(1), selections),
                    )
                    .expect("cast Roil Eruption");
                    resolve_entire_stack_two_player(&mut e);
                    assert_eq!(e.state.players[1].life, if kicked { 15 } else { 17 });
                }
                "blink_of_an_eye" => {
                    let target = move_ready_to_battlefield(&mut e, 1, "grizzly_bears");
                    inject_card_into_hand(&mut e, 0, card);
                    give_mana(
                        &mut e,
                        0,
                        ManaGift {
                            u: if kicked { 2 } else { 1 },
                            c: if kicked { 2 } else { 1 },
                            ..Default::default()
                        },
                    );
                    let hand_before = e.state.players[0].hand.len();
                    let slot = hand_index_for_card(&e, 0, card);
                    let selections = if kicked { mana_kicker() } else { Vec::new() };
                    e.apply_scenario_command(
                        0,
                        &cast_spell_with_cast_cost_groups(slot, target_object(target), selections),
                    )
                    .expect("cast Blink of an Eye");
                    resolve_entire_stack_two_player(&mut e);
                    assert_eq!(e.state.objects[&target].zone, Zone::Hand);
                    assert_eq!(
                        e.state.players[0].hand.len(),
                        hand_before - 1 + usize::from(kicked)
                    );
                }
                "final_flourish" | "eject_the_warp_core" => {
                    let fodder = move_ready_to_battlefield(&mut e, 0, "grizzly_bears");
                    let target = move_ready_to_battlefield(&mut e, 1, "autochthon_wurm");
                    inject_card_into_hand(&mut e, 0, card);
                    give_mana(
                        &mut e,
                        0,
                        ManaGift {
                            b: 1,
                            c: 1,
                            ..Default::default()
                        },
                    );
                    let slot = hand_index_for_card(&e, 0, card);
                    let selections = if kicked {
                        sacrifice_kicker(&e, fodder)
                    } else {
                        Vec::new()
                    };
                    e.apply_scenario_command(
                        0,
                        &cast_spell_with_cast_cost_groups(slot, target_object(target), selections),
                    )
                    .expect("cast pump kicker");
                    resolve_entire_stack_two_player(&mut e);
                    assert_eq!(
                        (e.effective_power(target), e.effective_toughness(target)),
                        (
                            Some(if kicked { 3 } else { 7 }),
                            Some(if kicked { 8 } else { 12 })
                        ),
                        "{card} kicked={kicked}"
                    );
                }
                other => panic!("unexpected card {other}"),
            }
        }
    }
}

#[test]
fn q80_kicker_blink_fizzles_without_kicked_draw_when_its_only_target_is_gone() {
    let mut e = engine(80_302_001);
    let target = move_ready_to_battlefield(&mut e, 1, "grizzly_bears");
    inject_card_into_hand(&mut e, 0, "blink_of_an_eye");
    give_mana(
        &mut e,
        0,
        ManaGift {
            u: 2,
            c: 2,
            ..Default::default()
        },
    );
    let hand_before = e.state.players[0].hand.len();
    let slot = hand_index_for_card(&e, 0, "blink_of_an_eye");
    e.apply_scenario_command(
        0,
        &cast_spell_with_cast_cost_groups(slot, target_object(target), mana_kicker()),
    )
    .expect("cast kicked Blink of an Eye");
    e.state.players[1].battlefield.retain(|id| *id != target);
    e.state.players[1].graveyard.push(target);
    e.state.objects.get_mut(&target).unwrap().zone = Zone::Graveyard;
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&target].zone, Zone::Graveyard);
    assert_eq!(
        e.state.players[0].hand.len(),
        hand_before - 1,
        "the kicked spell fizzles and draws nothing"
    );
}

#[test]
fn q80_kicker_final_flourish_rejects_kicker_without_a_sacrifice() {
    let mut e = engine(80_303_001);
    let target = move_ready_to_battlefield(&mut e, 1, "autochthon_wurm");
    inject_card_into_hand(&mut e, 0, "final_flourish");
    give_mana(
        &mut e,
        0,
        ManaGift {
            b: 1,
            c: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&e, 0, "final_flourish");
    assert!(
        e.apply_scenario_command(
            0,
            &cast_spell_with_cast_cost_groups(
                slot,
                target_object(target),
                vec![CastCostGroupSelection {
                    group_index: 0,
                    option_index: 0,
                    ..Default::default()
                }],
            ),
        )
        .is_err(),
        "the sacrifice option needs an object"
    );
    assert!(e.state.stack.is_empty());
    assert_eq!(e.state.players[0].mana_pool.black, 1);
}
