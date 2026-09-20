//! Issue #452 — the reviewed spell-surface fixed-damage family cohort.
//!
//! The seven generated identities each print exactly `<name> deals N damage to target creature.`
//! Exact Scryfall records and rulings were fetched 2026-09-19 against the pinned snapshot; none
//! returned a ruling. CR 120.2b keeps the resolving spell as the damage source, CR 115.1a
//! requires a declared creature target, CR 608.2b rechecks target legality on resolution, and
//! CR 704.5g handles lethal damage as a state-based action.

use super::helpers::*;
use tricerules_core::{GameEngine, Zone};

const COHORT: [(&str, u32); 7] = [
    ("ragefire", 3),
    ("repulsor_rays", 3),
    ("scorching_shot", 5),
    ("command_the_storm", 5),
    ("concentrated_fire", 5),
    ("direct_hit", 5),
    ("engulfing_eruption", 5),
];

fn spell_engine(seed: u64) -> GameEngine {
    // Explicit land-only decks keep the default grizzly_bears/library fixtures out of the
    // opening hand so injected fixture identities stay unique.
    let decks = Some(vec![deck_with("mountain", &[]), deck_with("mountain", &[])]);
    let mut engine = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn prepare_spell(engine: &mut GameEngine, card_id: &str) -> usize {
    inject_card_into_hand(engine, 0, card_id);
    grant_pool(engine, 0);
    hand_index_for_card(engine, 0, card_id)
}

#[test]
fn issue_452_family_deals_exact_damage_from_the_resolving_spell() {
    for (index, (card_id, amount)) in COHORT.into_iter().enumerate() {
        let mut engine = spell_engine(452_100 + index as u64);
        let target = inject_creature_with_stats(&mut engine, 1, "hill_giant", 5, 10);
        let bystander = inject_creature_with_stats(&mut engine, 1, "grizzly_bears", 2, 2);
        let artifact = inject_permanent_on_battlefield(&mut engine, 1, "short_sword");
        let slot = prepare_spell(&mut engine, card_id);
        let source = engine.state.players[0].hand[slot];
        let generation = semantic::generation(&engine, source);

        semantic::accepted(&mut engine, 0, &cast_spell(slot, target_object(target)));
        assert_eq!(engine.state.stack.len(), 1);
        semantic::complete(&mut engine, 8, |_| None).require_exercised();

        semantic::assert_object(
            &engine,
            source,
            card_id,
            0,
            0,
            Zone::Graveyard,
            generation + 2,
        );
        assert_eq!(engine.state.objects[&target].damage, amount, "{card_id}");
        assert_eq!(engine.state.objects[&bystander].damage, 0, "{card_id}");
        assert_eq!(engine.state.objects[&artifact].damage, 0, "{card_id}");
        assert_eq!(engine.state.players[0].life, 20, "{card_id}");
        assert_eq!(engine.state.players[1].life, 20, "{card_id}");
    }
}

#[test]
fn issue_452_family_rejects_players_and_artifacts_as_targets() {
    for (card_id, _) in COHORT {
        let mut engine = spell_engine(452_200);
        let artifact = inject_permanent_on_battlefield(&mut engine, 1, "short_sword");
        let slot = prepare_spell(&mut engine, card_id);
        for illegal in [target_player(1), target_object(artifact)] {
            let before = engine.state.command_index;
            engine
                .apply_command(0, &cast_spell(slot, illegal.clone()))
                .expect_err("only creature targets are legal");
            assert_eq!(
                engine.state.command_index, before,
                "{card_id}: rejected cast must not mutate state"
            );
            assert!(engine.state.stack.is_empty(), "{card_id}");
        }
        // The same hand slot still casts after every rejected target.
        let target = inject_creature_with_stats(&mut engine, 1, "grizzly_bears", 2, 2);
        semantic::accepted(&mut engine, 0, &cast_spell(slot, target_object(target)));
    }
}

#[test]
fn issue_452_family_fizzles_after_the_target_leaves_before_resolution() {
    let mut engine = spell_engine(452_300);
    let target = inject_creature_with_stats(&mut engine, 1, "hill_giant", 5, 10);
    let slot = prepare_spell(&mut engine, "scorching_shot");
    let source = engine.state.players[0].hand[slot];
    let generation = semantic::generation(&engine, source);
    semantic::accepted(&mut engine, 0, &cast_spell(slot, target_object(target)));

    inject_card_into_hand(&mut engine, 0, "unsummon");
    let unsummon = hand_index_for_card(&engine, 0, "unsummon");
    semantic::accepted(&mut engine, 0, &cast_spell(unsummon, target_object(target)));
    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(engine.state.objects[&target].zone, Zone::Hand);
    assert_eq!(engine.state.objects[&target].damage, 0);
    semantic::assert_object(
        &engine,
        source,
        "scorching_shot",
        0,
        0,
        Zone::Graveyard,
        generation + 2,
    );
    assert_eq!(engine.state.players[1].life, 20);
}
