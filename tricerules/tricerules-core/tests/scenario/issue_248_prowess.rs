//! Issue #248 — generated Prowess cards reuse the established cast-trigger and pump semantics.
//!
//! Oracle checked 2026-09-10. CR 702.108a-b defines Prowess as one controller-relative trigger
//! for each noncreature spell cast; CR 514.2 ends its until-end-of-turn pump at cleanup.

use super::helpers::*;

fn engine_with(spells: &[&str], seed: u64) -> GameEngine {
    let mut cards = vec!["agent_of_atlas"];
    cards.extend_from_slice(spells);
    let decks = Some(vec![deck_with("plains", &cards), deck_with("forest", &[])]);
    let mut engine = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

#[test]
fn generated_prowess_triggers_independently_for_noncreature_spells_and_expires() {
    let mut engine = engine_with(&["bonesplitter", "bonesplitter", "grizzly_bears"], 24801);
    let adept = relocate_to_battlefield(&mut engine, 0, "agent_of_atlas", false);
    grant_pool(&mut engine, 0);

    for spell in ["bonesplitter", "bonesplitter"] {
        ensure_in_hand(&mut engine, 0, spell);
        let slot = hand_index_for_card(&engine, 0, spell);
        engine
            .apply_command(0, &cast_spell(slot, vec![]))
            .unwrap_or_else(|error| panic!("cast {spell}: {error}"));
        assert_eq!(
            engine.state.stack.len(),
            2,
            "{spell} creates one Prowess trigger"
        );
        resolve_entire_stack_two_player(&mut engine);
    }

    assert_eq!(engine.effective_power(adept), Some(4));
    assert_eq!(engine.effective_toughness(adept), Some(4));

    ensure_in_hand(&mut engine, 0, "grizzly_bears");
    let creature = hand_index_for_card(&engine, 0, "grizzly_bears");
    engine
        .apply_command(0, &cast_spell(creature, vec![]))
        .expect("cast creature spell");
    assert_eq!(
        engine.state.stack.len(),
        1,
        "creature spells do not trigger Prowess"
    );
    resolve_entire_stack_two_player(&mut engine);

    end_active_turn(&mut engine, 0);
    assert_eq!(engine.effective_power(adept), Some(2));
    assert_eq!(engine.effective_toughness(adept), Some(2));
}

#[test]
fn generated_prowess_trigger_does_not_follow_a_new_source_generation() {
    let mut engine = engine_with(&["bonesplitter"], 24802);
    let adept = relocate_to_battlefield(&mut engine, 0, "agent_of_atlas", false);
    ensure_in_hand(&mut engine, 0, "bonesplitter");
    grant_pool(&mut engine, 0);

    let spell = hand_index_for_card(&engine, 0, "bonesplitter");
    engine
        .apply_command(0, &cast_spell(spell, vec![]))
        .expect("cast noncreature spell");
    *engine
        .state
        .zone_change_generation
        .entry(adept)
        .or_default() += 2;
    pass_both_players(&mut engine);

    assert_eq!(engine.effective_power(adept), Some(2));
    assert_eq!(engine.effective_toughness(adept), Some(2));
}
