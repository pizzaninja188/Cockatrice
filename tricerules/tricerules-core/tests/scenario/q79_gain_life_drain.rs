//! Actual-card scenarios for the queue-v79 01-gain-life-drain cohort.
//!
//! Expectations are printed Oracle values. CR 118.3 makes each life-gain amount one event;
//! CR 603.2 puts one trigger per event on the stack; CR 119.3 makes each opponent lose exactly
//! 1 life. The pinned 2016-01-22, 2019-07-12 and 2021-04-16 rulings confirm one trigger per
//! event regardless of the amount, a fixed 1-life loss, and that Dina's X is the sacrificed
//! creature's battlefield power.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![
        deck_with(
            "swamp",
            &[
                "epicure_of_blood",
                "cliffhaven_vampire",
                "dina,_soul_steeper",
                "hill_giant",
                "grizzly_bears",
            ],
        ),
        deck_with("plains", &[]),
    ]);
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("engine");
    advance_to_main1_from_game_start(&mut e);
    e
}

fn cast_gain_seven(e: &mut GameEngine) {
    inject_card_into_hand(e, 0, "angels_mercy");
    give_mana(
        e,
        0,
        ManaGift {
            w: 2,
            c: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(e, 0, "angels_mercy");
    e.apply_scenario_command(0, &cast_spell(slot, vec![]))
        .expect("cast Angels Mercy");
}

#[test]
fn q79_gain_life_drain_canary_simplest_epicure_drains_once_for_one_event() {
    let mut e = engine(79_100_001);
    relocate_to_battlefield(&mut e, 0, "epicure_of_blood", false);
    let spell = {
        cast_gain_seven(&mut e);
        e.state.stack.last().expect("Angels Mercy").id
    };
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.players[0].life, 27, "the controller gains 7 life");
    assert_eq!(
        e.state.players[1].life, 19,
        "one life-gain event drains each opponent exactly once"
    );
    assert_eq!(e.state.objects[&spell].zone, Zone::Graveyard);
    assert!(e.state.stack.is_empty());
}

#[test]
fn q79_gain_life_drain_canary_hardest_dina_sacrifices_another_creature_and_pumps() {
    let mut e = engine(79_100_002);
    let dina = relocate_to_battlefield(&mut e, 0, "dina,_soul_steeper", false);
    let giant = relocate_to_battlefield(&mut e, 0, "hill_giant", false);
    give_mana(
        &mut e,
        0,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );
    e.apply_scenario_command(
        0,
        &activate_ability_with_costs(dina, 0, vec![], vec![permanent_cost_selection(1, giant)]),
    )
    .expect("activate Dina, sacrificing the Hill Giant");
    assert_eq!(
        e.state.objects[&giant].zone,
        Zone::Graveyard,
        "the sacrificed creature is in the graveyard as a cost"
    );
    assert_eq!(
        e.effective_power(dina),
        Some(1),
        "pump waits for resolution"
    );
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.effective_power(dina),
        Some(4),
        "X is the sacrificed creature's power 3"
    );
    assert_eq!(e.effective_toughness(dina), Some(3));

    cast_gain_seven(&mut e);
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.players[0].life, 27);
    assert_eq!(e.state.players[1].life, 19);
}

#[test]
fn q79_gain_life_drain_cards_all_drain_each_opponent_once_per_event() {
    for (index, id) in [
        "epicure_of_blood",
        "cliffhaven_vampire",
        "dina,_soul_steeper",
    ]
    .into_iter()
    .enumerate()
    {
        let mut e = engine(79_101_000 + index as u64);
        let permanent = relocate_to_battlefield(&mut e, 0, id, false);
        cast_gain_seven(&mut e);
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.players[0].life, 27,
            "{id}: controller gains the 7 life"
        );
        assert_eq!(
            e.state.players[1].life, 19,
            "{id}: the opponent loses exactly 1 life"
        );
        assert_eq!(
            e.state.objects[&permanent].zone,
            Zone::Battlefield,
            "{id}: the draining source remains"
        );
    }
}

#[test]
fn q79_gain_life_drain_two_events_trigger_twice() {
    let mut e = engine(79_102_001);
    relocate_to_battlefield(&mut e, 0, "epicure_of_blood", false);
    cast_gain_seven(&mut e);
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.players[1].life, 19);
    cast_gain_seven(&mut e);
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.players[0].life, 34, "two 7-life gains");
    assert_eq!(
        e.state.players[1].life, 18,
        "the second, separate event drains once more"
    );
}

#[test]
fn q79_gain_life_drain_dina_rejects_sacrificing_itself() {
    let mut e = engine(79_103_001);
    let dina = relocate_to_battlefield(&mut e, 0, "dina,_soul_steeper", false);
    give_mana(
        &mut e,
        0,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );
    let pool_before = (
        e.state.players[0].mana_pool.colorless,
        e.state.players[0].life,
    );
    e.apply_scenario_command(
        0,
        &activate_ability_with_costs(dina, 0, vec![], vec![permanent_cost_selection(1, dina)]),
    )
    .expect_err("another creature is required");
    assert_eq!(
        e.state.objects[&dina].zone,
        Zone::Battlefield,
        "Dina is not sacrificed"
    );
    assert_eq!(
        (
            e.state.players[0].mana_pool.colorless,
            e.state.players[0].life
        ),
        pool_before,
        "the rejected activation spends nothing"
    );
    assert!(e.state.stack.is_empty());
}

#[test]
fn q79_gain_life_drain_dina_keeps_last_known_power_after_the_sacrifice() {
    let mut e = engine(79_104_001);
    let dina = relocate_to_battlefield(&mut e, 0, "dina,_soul_steeper", false);
    let giant = relocate_to_battlefield(&mut e, 0, "hill_giant", false);
    let counter = relocate_to_battlefield(&mut e, 0, "grizzly_bears", false);
    give_mana(
        &mut e,
        0,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );
    e.apply_scenario_command(
        0,
        &activate_ability_with_costs(dina, 0, vec![], vec![permanent_cost_selection(1, giant)]),
    )
    .expect("activate Dina");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.effective_power(dina), Some(4));
    assert_eq!(e.effective_power(counter), Some(2), "only Dina is pumped");
}

#[test]
fn q79_gain_life_drain_dina_reads_a_controlled_but_not_owned_sacrifice() {
    let mut e = engine(79_105_001);
    let dina = relocate_to_battlefield(&mut e, 0, "dina,_soul_steeper", false);
    let borrowed = inject_creature_under_foreign_control(&mut e, 1, 0, "hill_giant");
    give_mana(
        &mut e,
        0,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );
    e.apply_scenario_command(
        0,
        &activate_ability_with_costs(dina, 0, vec![], vec![permanent_cost_selection(1, borrowed)]),
    )
    .expect("sacrifice a controlled creature owned by the opponent");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.effective_power(dina),
        Some(4),
        "the payment cohort reads the exact sacrificed object regardless of owner"
    );
}

#[test]
fn q79_gain_life_drain_does_not_trigger_on_an_opponent_gaining_life() {
    let mut e = engine(79_106_001);
    relocate_to_battlefield(&mut e, 0, "epicure_of_blood", false);
    e.apply_scenario_command(0, &pass()).expect("active passes");
    inject_card_into_hand(&mut e, 1, "angels_mercy");
    give_mana(
        &mut e,
        1,
        ManaGift {
            w: 2,
            c: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&e, 1, "angels_mercy");
    e.apply_scenario_command(1, &cast_spell(slot, vec![]))
        .expect("opponent casts Angels Mercy");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.players[1].life, 27, "the opponent gains 7 life");
    assert_eq!(
        e.state.players[0].life, 20,
        "Epicure watches only its controller's life gain"
    );
}
