//! Issue #458 - the pump-and-life spell family and the seven-lands conditional self-pump family
//! through the command boundary.
//!
//! Every expectation is the reviewed printed Oracle behavior, not a copy of generator output.
//! Exact Scryfall records and `rulings_uri` responses were fetched 2026-09-20 against pinned
//! snapshot `27bf3214-1271-490b-bdfe-c0be6c23d02e`; the Moment of Craving ruling confirms the
//! spell fizzles entirely when its only target is illegal, so no life is gained. Governing CR
//! concepts: CR 115.1a (one declared creature target), CR 608.2b (target revalidation on
//! resolution), CR 608.2c (printed instruction order), CR 611.2a / 613.4c (until-end-of-turn P/T
//! modifiers), CR 118.1 (life gain), CR 604.1 / 604.2 (static abilities), CR 611.3 (a continuous
//! effect with a live condition), and CR 109.2 (land is a card type).

use super::helpers::*;
use tricerules_core::{GameEngine, Zone};

fn main1(seed: u64) -> GameEngine {
    semantic::main_phase(seed)
}

/// A deck that contains the reviewed creature so it can enter through the engine.
fn creature_engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![deck_with("forest", &[card]), deck_with("island", &[])]);
    let mut engine = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn stats(engine: &GameEngine, object: u32) -> (u32, u32) {
    let characteristics = engine.characteristics(object).expect("characteristics");
    (
        characteristics.power.expect("power"),
        characteristics.toughness.expect("toughness"),
    )
}

fn remove_from_battlefield(engine: &mut GameEngine, player: usize, object_id: u32) {
    engine.state.players[player]
        .battlefield
        .retain(|candidate| *candidate != object_id);
    let object = engine.state.objects.get_mut(&object_id).expect("object");
    object.zone = Zone::Exile;
    *engine
        .state
        .zone_change_generation
        .entry(object_id)
        .or_default() += 1;
}

#[test]
fn issue_458_pump_life_spells_shrink_the_target_and_gain_exactly_two() {
    for (index, (card, delta_power, delta_toughness)) in
        [("moment_of_craving", 2, 2), ("syphon_fuel", 6, 6)]
            .into_iter()
            .enumerate()
    {
        let mut engine = main1(458_100 + index as u64);
        // A ten-toughness target survives both reviewed deltas and stays on the battlefield.
        let target = inject_creature_with_stats(&mut engine, 1, "grizzly_bears", 10, 10);
        let source = inject_card_into_hand(&mut engine, 0, card);
        let slot = hand_index_for_card(&engine, 0, card);
        let generation = semantic::generation(&engine, source);
        grant_pool(&mut engine, 0);

        semantic::accepted(&mut engine, 0, &cast_spell(slot, target_object(target)));
        semantic::complete(&mut engine, 24, |_| None).require_exercised();
        semantic::assert_main_priority(&engine, 0);

        assert_eq!(
            engine.effective_power(target),
            Some(10 - delta_power),
            "{card} must shrink power by exactly {delta_power}"
        );
        assert_eq!(
            engine.effective_toughness(target),
            Some(10 - delta_toughness),
            "{card} must shrink toughness by exactly {delta_toughness}"
        );
        assert_eq!(
            engine.state.players[0].life, 22,
            "{card} must gain its controller exactly two life"
        );
        assert_eq!(
            engine.state.players[1].life, 20,
            "{card} must not change the opponent's life"
        );
        assert_eq!(engine.state.objects[&target].zone, Zone::Battlefield);
        semantic::assert_object(&engine, source, card, 0, 0, Zone::Graveyard, generation + 2);
    }
}

#[test]
fn issue_458_pump_life_fizzles_on_an_illegal_target_without_gaining_life() {
    for (index, card) in ["moment_of_craving", "syphon_fuel"].into_iter().enumerate() {
        let mut engine = main1(458_200 + index as u64);
        let target = inject_creature_with_stats(&mut engine, 1, "grizzly_bears", 3, 3);
        let source = inject_card_into_hand(&mut engine, 0, card);
        let slot = hand_index_for_card(&engine, 0, card);
        let generation = semantic::generation(&engine, source);
        grant_pool(&mut engine, 0);
        semantic::accepted(&mut engine, 0, &cast_spell(slot, target_object(target)));

        // The response bounces the only target before the pump-and-life spell resolves.
        inject_card_into_hand(&mut engine, 0, "unsummon");
        let unsummon = hand_index_for_card(&engine, 0, "unsummon");
        semantic::accepted(&mut engine, 0, &cast_spell(unsummon, target_object(target)));
        resolve_entire_stack_two_player(&mut engine);

        // CR 608.2b: the only target is illegal, so the whole spell leaves the stack without
        // resolving; the printed life gain must not happen.
        assert_eq!(engine.state.objects[&target].zone, Zone::Hand, "{card}");
        assert_eq!(
            engine.state.players[0].life, 20,
            "{card} must not gain life without a legal target"
        );
        assert_eq!(engine.state.players[1].life, 20, "{card}");
        semantic::assert_object(&engine, source, card, 0, 0, Zone::Graveyard, generation + 2);
    }
}

#[test]
fn issue_458_seven_lands_creatures_gain_the_exact_delta_at_seven_lands() {
    for (index, (card, printed_power, printed_toughness, delta_power, delta_toughness)) in
        [("gigantoad", 4, 4, 2, 2), ("scorpion_sentinel", 1, 4, 3, 0)]
            .into_iter()
            .enumerate()
    {
        let mut engine = creature_engine(458_300 + index as u64, card);
        let source = move_ready_to_battlefield(&mut engine, 0, card);
        let bystander = inject_creature_with_stats(&mut engine, 0, "grizzly_bears", 2, 2);
        assert_eq!(
            stats(&engine, source),
            (printed_power, printed_toughness),
            "{card} has no bonus before any land"
        );

        for _ in 0..6 {
            inject_permanent_on_battlefield(&mut engine, 0, "forest");
        }
        assert_eq!(
            stats(&engine, source),
            (printed_power, printed_toughness),
            "{card} needs seven lands, not six"
        );

        inject_permanent_on_battlefield(&mut engine, 0, "forest");
        assert_eq!(
            stats(&engine, source),
            (
                (printed_power as i32 + delta_power) as u32,
                (printed_toughness as i32 + delta_toughness) as u32
            ),
            "{card} at seven controlled lands"
        );
        assert_eq!(
            stats(&engine, bystander),
            (2, 2),
            "{card}'s modifier is source-only"
        );
    }
}

#[test]
fn issue_458_seven_lands_condition_recomputes_live_and_ignores_opponents_lands() {
    let mut engine = creature_engine(458_400, "gigantoad");
    let source = move_ready_to_battlefield(&mut engine, 0, "gigantoad");
    let mut own_lands = Vec::new();
    for _ in 0..7 {
        own_lands.push(inject_permanent_on_battlefield(&mut engine, 0, "forest"));
    }
    assert_eq!(stats(&engine, source), (6, 6));

    // An opponent's ten lands never satisfy "you control".
    for _ in 0..10 {
        inject_permanent_on_battlefield(&mut engine, 1, "forest");
    }
    assert_eq!(
        stats(&engine, source),
        (6, 6),
        "an opponent's lands never contribute"
    );

    // CR 604.1 / 604.2 / 611.3 / 613.4c: the condition and its continuous effect recompute live.
    remove_from_battlefield(&mut engine, 0, own_lands[0]);
    assert_eq!(
        stats(&engine, source),
        (4, 4),
        "removing one own land turns the bonus off in the same turn"
    );
    inject_permanent_on_battlefield(&mut engine, 0, "forest");
    assert_eq!(
        stats(&engine, source),
        (6, 6),
        "adding a seventh own land turns the bonus back on in the same turn"
    );
}
