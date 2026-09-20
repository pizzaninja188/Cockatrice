//! Issue #452 — reviewed direct-RON scenarios for Stand Up for Yourself and Oracle's Restoration.
//!
//! Exact Scryfall records and `rulings_uri` were fetched 2026-09-19 against the pinned snapshot.
//! Oracle's Restoration carries the 2026-03-20 ruling that an illegal target on resolution means
//! none of its effects happen. CR 115.1a declares one creature target, CR 608.2b rechecks target
//! legality on resolution, CR 701.8 destroys, CR 611.2c/613.4 keeps the +1/+1 until end of turn,
//! and CR 121.1/118.1 cover the draw and life gain.

use super::helpers::*;
use tricerules_core::{GameEngine, Zone};

fn spell_engine(seed: u64) -> GameEngine {
    // Explicit land-only decks keep the default grizzly_bears/library fixtures out of the
    // opening hand so injected fixture identities stay unique.
    let decks = Some(vec![deck_with("plains", &[]), deck_with("forest", &[])]);
    let mut engine = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn prepare_spell(engine: &mut GameEngine, card_id: &str) -> usize {
    inject_card_into_hand(engine, 0, card_id);
    grant_pool(engine, 0);
    hand_index_for_card(engine, 0, card_id)
}

fn put_on_top(engine: &mut GameEngine, player: usize, card_id: &str) -> u32 {
    let object = inject_library_card(engine, player, card_id);
    engine.state.players[player]
        .library
        .retain(|object_id| *object_id != object);
    engine.state.players[player].library.push_front(object);
    object
}

#[test]
fn issue_452_stand_up_for_yourself_destroys_only_power_three_or_greater() {
    let mut engine = spell_engine(452_401);
    let legal_target = inject_creature_with_stats(&mut engine, 1, "grizzly_bears", 3, 3);
    let small = inject_creature_with_stats(&mut engine, 1, "grizzly_bears", 2, 2);
    let slot = prepare_spell(&mut engine, "stand_up_for_yourself");
    let source = engine.state.players[0].hand[slot];
    let generation = semantic::generation(&engine, source);

    // CR 115.1a: a power-two creature and a player are not legal targets.
    let commands_before = engine.state.command_index;
    engine
        .apply_command(0, &cast_spell(slot, target_object(small)))
        .expect_err("power two is below the printed bound");
    engine
        .apply_command(0, &cast_spell(slot, target_player(1)))
        .expect_err("players are never legal targets");
    assert_eq!(engine.state.command_index, commands_before);

    semantic::accepted(
        &mut engine,
        0,
        &cast_spell(slot, target_object(legal_target)),
    );
    semantic::complete(&mut engine, 8, |_| None).require_exercised();

    semantic::assert_object(
        &engine,
        source,
        "stand_up_for_yourself",
        0,
        0,
        Zone::Graveyard,
        generation + 2,
    );
    assert_eq!(engine.state.objects[&legal_target].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&small].zone, Zone::Battlefield);
    assert_eq!(engine.state.players[1].life, 20);
}

#[test]
fn issue_452_stand_up_for_yourself_fizzles_after_an_illegal_target_on_resolution() {
    let mut engine = spell_engine(452_402);
    let target = inject_creature_with_stats(&mut engine, 1, "grizzly_bears", 3, 3);
    let slot = prepare_spell(&mut engine, "stand_up_for_yourself");
    let source = engine.state.players[0].hand[slot];
    let generation = semantic::generation(&engine, source);
    semantic::accepted(&mut engine, 0, &cast_spell(slot, target_object(target)));

    inject_card_into_hand(&mut engine, 0, "unsummon");
    let unsummon = hand_index_for_card(&engine, 0, "unsummon");
    semantic::accepted(&mut engine, 0, &cast_spell(unsummon, target_object(target)));
    resolve_entire_stack_two_player(&mut engine);

    // CR 608.2b: the only target is illegal, so the spell leaves the stack without resolving.
    assert_eq!(engine.state.objects[&target].zone, Zone::Hand);
    semantic::assert_object(
        &engine,
        source,
        "stand_up_for_yourself",
        0,
        0,
        Zone::Graveyard,
        generation + 2,
    );
}

#[test]
fn issue_452_stand_up_for_yourself_fizzles_when_power_drops_below_the_bound() {
    let mut engine = spell_engine(452_405);
    let target = inject_creature_with_stats(&mut engine, 1, "grizzly_bears", 5, 5);
    let slot = prepare_spell(&mut engine, "stand_up_for_yourself");
    let source = engine.state.players[0].hand[slot];
    let generation = semantic::generation(&engine, source);
    semantic::accepted(&mut engine, 0, &cast_spell(slot, target_object(target)));

    inject_card_into_hand(&mut engine, 0, "last_gasp");
    let last_gasp = hand_index_for_card(&engine, 0, "last_gasp");
    semantic::accepted(
        &mut engine,
        0,
        &cast_spell(last_gasp, target_object(target)),
    );
    resolve_entire_stack_two_player(&mut engine);

    // CR 608.2b rechecks the target's current characteristics: power two is below the bound,
    // so the spell leaves the stack with no effect while the shrunk creature survives.
    assert_eq!(engine.state.objects[&target].zone, Zone::Battlefield);
    assert_eq!(engine.effective_power(target), Some(2));
    assert_eq!(engine.effective_toughness(target), Some(2));
    semantic::assert_object(
        &engine,
        source,
        "stand_up_for_yourself",
        0,
        0,
        Zone::Graveyard,
        generation + 2,
    );
}

#[test]
fn issue_452_oracles_restoration_pumps_draws_and_gains_for_its_controller() {
    let mut engine = spell_engine(452_403);
    let target = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let drawn = put_on_top(&mut engine, 0, "forest");
    let slot = prepare_spell(&mut engine, "oracles_restoration");
    let source = engine.state.players[0].hand[slot];
    let generation = semantic::generation(&engine, source);
    let hand_before = engine.state.players[0].hand.len();
    let power_before = engine.effective_power(target);
    let toughness_before = engine.effective_toughness(target);

    semantic::accepted(&mut engine, 0, &cast_spell(slot, target_object(target)));
    semantic::complete(&mut engine, 8, |_| None).require_exercised();

    semantic::assert_object(
        &engine,
        source,
        "oracles_restoration",
        0,
        0,
        Zone::Graveyard,
        generation + 2,
    );
    assert_eq!(engine.state.objects[&target].zone, Zone::Battlefield);
    assert_eq!(engine.effective_power(target), power_before.map(|p| p + 1));
    assert_eq!(
        engine.effective_toughness(target),
        toughness_before.map(|t| t + 1)
    );
    assert_eq!(engine.state.objects[&drawn].zone, Zone::Hand);
    assert_eq!(engine.state.players[0].hand.len(), hand_before);
    assert_eq!(engine.state.players[0].life, 21);
    assert_eq!(engine.state.players[1].life, 20);
}

#[test]
fn issue_452_oracles_restoration_rejects_opponents_and_fizzles_on_illegal_target() {
    let mut engine = spell_engine(452_404);
    let own = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let opponent = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let slot = prepare_spell(&mut engine, "oracles_restoration");
    let commands_before = engine.state.command_index;
    engine
        .apply_command(0, &cast_spell(slot, target_object(opponent)))
        .expect_err("only creatures you control are legal targets");
    assert_eq!(engine.state.command_index, commands_before);

    let source = engine.state.players[0].hand[slot];
    let own_library = inject_library_card(&mut engine, 0, "forest");
    let generation = semantic::generation(&engine, source);
    semantic::accepted(&mut engine, 0, &cast_spell(slot, target_object(own)));
    inject_card_into_hand(&mut engine, 0, "unsummon");
    let unsummon = hand_index_for_card(&engine, 0, "unsummon");
    semantic::accepted(&mut engine, 0, &cast_spell(unsummon, target_object(own)));
    resolve_entire_stack_two_player(&mut engine);

    // 2026-03-20 ruling and CR 608.2b: no pump, no draw, no life without a legal target.
    assert_eq!(engine.state.objects[&own].zone, Zone::Hand);
    assert_eq!(engine.state.players[0].life, 20);
    assert_eq!(engine.state.objects[&own_library].zone, Zone::Library);
    semantic::assert_object(
        &engine,
        source,
        "oracles_restoration",
        0,
        0,
        Zone::Graveyard,
        generation + 2,
    );
}
