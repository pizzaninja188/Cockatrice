//! Actual-card scenarios for the queue-v41 ETB life-loss and destroy-plus-scry cohorts.
//!
//! Expectations are printed Oracle values. CR 603.6a stages the entry trigger, CR 119.3 covers
//! life loss, CR 701.8 destroys, and CR 701.22a-b scries after the destruction. A stale sole
//! target stops the whole spell (CR 608.2b), so no scry happens.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::ChoiceKind;

fn engine(seed: u64, card: &str, basic: &str) -> GameEngine {
    let decks = Some(vec![deck_with(basic, &[card]), deck_with("forest", &[])]);
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

#[test]
fn q41_etb_each_loses_life_canary_simplest_hits_both_players() {
    let mut e = engine(41_040_001, "blood-toll_harpy", "swamp");
    move_ready_to_battlefield(&mut e, 0, "blood-toll_harpy");
    answer_trigger_order_in_engine_order(&mut e);
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.players[0].life, 19, "the controller also loses");
    assert_eq!(e.state.players[1].life, 19);
}

#[test]
fn q41_etb_each_loses_life_cards_apply_their_printed_amount() {
    for (index, (card, amount)) in [
        ("blood-toll_harpy", 1),
        ("howling_banshee", 3),
        ("rabid_bloodsucker", 2),
    ]
    .into_iter()
    .enumerate()
    {
        let mut e = engine(41_041_000 + index as u64, card, "swamp");
        move_ready_to_battlefield(&mut e, 0, card);
        answer_trigger_order_in_engine_order(&mut e);
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(e.state.players[0].life, 20 - amount, "{card}: controller");
        assert_eq!(e.state.players[1].life, 20 - amount, "{card}: opponent");
        assert_eq!(
            e.state.objects[&battlefield_object_for_card(&e, 0, card)].zone,
            Zone::Battlefield,
            "{card}: the source stays"
        );
    }
}

#[test]
fn q41_etb_each_loses_life_canary_hardest_three_life() {
    let mut e = engine(41_043_001, "howling_banshee", "swamp");
    move_ready_to_battlefield(&mut e, 0, "howling_banshee");
    answer_trigger_order_in_engine_order(&mut e);
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.players[0].life, 17);
    assert_eq!(e.state.players[1].life, 17);
}

#[test]
fn q41_etb_each_loses_life_has_no_target_and_does_not_draw() {
    let mut e = engine(41_042_001, "howling_banshee", "swamp");
    let hand_before = e.state.players[0].hand.len();
    move_ready_to_battlefield(&mut e, 0, "howling_banshee");
    answer_trigger_order_in_engine_order(&mut e);
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.players[0].hand.len(), hand_before, "no draw");
    assert!(
        e.state.pending_resolution.is_none(),
        "the untargeted trigger never asks for a target"
    );
    assert!(
        e.state
            .stack
            .iter()
            .all(|item| item.ability_text.as_deref() != Some("Scry")),
        "the card has no scry"
    );
}

fn cast_destroy_spell(e: &mut GameEngine, card: &str, mana: ManaGift, target: u32) {
    inject_card_into_hand(e, 0, card);
    give_mana(e, 0, mana);
    let slot = hand_index_for_card(e, 0, card);
    e.apply_scenario_command(0, &cast_spell(slot, target_object(target)))
        .expect("cast the destroy spell");
}

#[test]
fn q41_destroy_power_scry_canary_simplest_destroys_then_scries() {
    let mut e = engine(41_050_001, "vanquish_the_foul", "plains");
    let target = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 4, 4);
    let library_before = e.state.players[0].library.clone();
    cast_destroy_spell(
        &mut e,
        "vanquish_the_foul",
        ManaGift {
            w: 1,
            c: 5,
            ..Default::default()
        },
        target,
    );
    let batch = pass_both_players_with_batch(&mut e);
    assert_eq!(e.state.objects[&target].zone, Zone::Graveyard);
    let choice = find_resolution_choice(&batch).expect("scry pauses after the destruction");
    assert_eq!(choice.choice_kind(), ChoiceKind::LibraryTop);
    assert_eq!(choice.deciding_player_id, 0);
    assert_eq!((choice.min, choice.max), (0, 1));
    assert_eq!(e.state.players[0].library, library_before);
    e.apply_scenario_command(0, &submit_resolution_choice(vec![]))
        .expect("keep the scried card on top");
    assert!(e.state.pending_resolution.is_none());
    assert_eq!(e.state.players[0].library, library_before);
}

#[test]
fn q41_destroy_power_scry_cards_apply_their_printed_threshold_and_count() {
    for (index, (card, power, scry)) in [
        ("vanquish_the_foul", 4, 1),
        ("guiding_bolt", 4, 2),
        ("skywhalers_shot", 3, 1),
    ]
    .into_iter()
    .enumerate()
    {
        let mut e = engine(41_051_000 + index as u64, card, "plains");
        let target = inject_creature_with_stats(&mut e, 1, "grizzly_bears", power, power);
        cast_destroy_spell(
            &mut e,
            card,
            ManaGift {
                w: 1,
                c: if card == "vanquish_the_foul" { 5 } else { 2 },
                ..Default::default()
            },
            target,
        );
        let batch = pass_both_players_with_batch(&mut e);
        assert_eq!(e.state.objects[&target].zone, Zone::Graveyard, "{card}");
        let choice = find_resolution_choice(&batch).expect("printed scry count");
        assert_eq!((choice.min, choice.max), (0, scry), "{card}");
        assert_eq!(choice.candidate_object_ids.len(), scry as usize, "{card}");
        e.apply_scenario_command(0, &submit_resolution_choice(vec![]))
            .expect("keep the looked cards on top");
    }
}

#[test]
fn q41_destroy_power_scry_canary_hardest_destroys_and_scries_two() {
    let mut e = engine(41_054_001, "guiding_bolt", "plains");
    let target = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 4, 4);
    let library_before = e.state.players[0].library.clone();
    cast_destroy_spell(
        &mut e,
        "guiding_bolt",
        ManaGift {
            w: 1,
            c: 2,
            ..Default::default()
        },
        target,
    );
    let batch = pass_both_players_with_batch(&mut e);
    assert_eq!(e.state.objects[&target].zone, Zone::Graveyard);
    let choice = find_resolution_choice(&batch).expect("scry 2");
    assert_eq!((choice.min, choice.max), (0, 2));
    assert_eq!(choice.candidate_object_ids.len(), 2);
    e.apply_scenario_command(0, &submit_resolution_choice(vec![]))
        .expect("keep both on top");
    assert_eq!(e.state.players[0].library, library_before);
}

#[test]
fn q41_destroy_power_scry_rejects_a_creature_below_the_printed_power() {
    for (index, (card, power)) in [
        ("vanquish_the_foul", 3),
        ("guiding_bolt", 3),
        ("skywhalers_shot", 2),
    ]
    .into_iter()
    .enumerate()
    {
        let mut e = engine(41_052_000 + index as u64, card, "plains");
        let target = inject_creature_with_stats(&mut e, 1, "grizzly_bears", power, power);
        inject_card_into_hand(&mut e, 0, card);
        give_mana(
            &mut e,
            0,
            ManaGift {
                w: 1,
                c: if card == "vanquish_the_foul" { 5 } else { 2 },
                ..Default::default()
            },
        );
        let slot = hand_index_for_card(&e, 0, card);
        let before = format!("{:?}", e.state);
        assert!(
            e.apply_scenario_command(0, &cast_spell(slot, target_object(target)))
                .is_err(),
            "{card}: power {power} is below the printed threshold"
        );
        assert_eq!(format!("{:?}", e.state), before, "{card}");
    }
}

#[test]
fn q41_destroy_power_scry_fizzles_without_scrying_when_the_target_leaves() {
    for (index, card) in ["vanquish_the_foul", "guiding_bolt", "skywhalers_shot"]
        .into_iter()
        .enumerate()
    {
        let mut e = engine(41_053_000 + index as u64, card, "plains");
        let target = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 4, 4);
        let library_before = e.state.players[0].library.clone();
        cast_destroy_spell(
            &mut e,
            card,
            ManaGift {
                w: 1,
                c: if card == "vanquish_the_foul" { 5 } else { 2 },
                ..Default::default()
            },
            target,
        );
        e.state.players[1].battlefield.retain(|id| *id != target);
        e.state.players[1].graveyard.push(target);
        e.state.objects.get_mut(&target).unwrap().zone = Zone::Graveyard;
        *e.state.zone_change_generation.entry(target).or_default() += 1;
        let batch = pass_both_players_with_batch(&mut e);
        assert!(
            find_resolution_choice(&batch).is_none(),
            "{card}: no scry after the fizzle"
        );
        assert!(e.state.pending_resolution.is_none(), "{card}");
        assert_eq!(e.state.players[0].library, library_before, "{card}");
    }
}
