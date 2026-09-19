//! Issue #370: graveyard-count self-cost reduction resolves from public state at cost
//! determination (CR 601.2f, 404.2), not at resolution.
//!
//! Chitin Gravestalker counts artifact and/or creature cards (pure OR), Gargantuan Leech sums
//! Caves its controller controls plus Cave cards in its controller's graveyard, and Tolarian
//! Terror counts instant and sorcery cards. Every published reduction and every payment in these
//! scenarios is computed before the spell changes zones, so the spell's own card can never be
//! part of its own count.

use super::helpers::*;
use tricerules_core::GameEngine;

#[track_caller]
fn published_reduction(engine: &mut GameEngine, player: usize, card_id: &str) -> u32 {
    let hand_index = hand_index_for_card(engine, player, card_id) as u32;
    let batch = engine.initial_response_batch();
    let actions = &batch.legal_by_player[&(player as i32)].hand_actions;
    actions
        .iter()
        .find(|action| action.hand_index == hand_index)
        .unwrap_or_else(|| {
            panic!(
                "missing cast action for {card_id}; published hand indices: {:?}",
                actions
                    .iter()
                    .map(|action| (action.hand_index, action.card_name.as_str()))
                    .collect::<Vec<_>>()
            )
        })
        .generic_cost_reduction
}

#[test]
fn issue_370_chitin_gravestalker_counts_own_artifact_or_creature_cards_only() {
    let decks = Some(vec![
        deck_with("swamp", &["chitin_gravestalker"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(370_001, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    ensure_in_hand(&mut engine, 0, "chitin_gravestalker");

    inject_graveyard_card(&mut engine, 0, "gold_pan");
    inject_graveyard_card(&mut engine, 0, "grizzly_bears");
    inject_graveyard_card(&mut engine, 0, "abrade");
    inject_graveyard_card(&mut engine, 1, "grizzly_bears");

    assert_eq!(
        published_reduction(&mut engine, 0, "chitin_gravestalker"),
        2,
        "one artifact and one creature card count; the instant and the opponent's graveyard do not"
    );

    engine.state.players[0].mana_pool.black = 1;
    engine.state.players[0].mana_pool.colorless = 3;
    let gravestalker = hand_index_for_card(&engine, 0, "chitin_gravestalker");
    engine
        .apply_command(0, &cast_spell(gravestalker, vec![]))
        .expect("Chitin Gravestalker costs {3}{B} with two matching graveyard cards");
    assert_eq!(engine.state.players[0].mana_pool.black, 0);
    assert_eq!(
        engine.state.players[0].mana_pool.colorless, 0,
        "the payment consumes exactly the reduced generic cost"
    );
}

#[test]
fn issue_370_gargantuan_leech_sums_caves_you_control_and_cave_cards_in_graveyard() {
    let decks = Some(vec![
        deck_with("swamp", &["gargantuan_leech"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(370_002, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    ensure_in_hand(&mut engine, 0, "gargantuan_leech");

    inject_permanent_on_battlefield(&mut engine, 0, "promising_vein");
    inject_graveyard_card(&mut engine, 0, "promising_vein");
    inject_graveyard_card(&mut engine, 0, "grizzly_bears");
    inject_permanent_on_battlefield(&mut engine, 1, "promising_vein");
    inject_graveyard_card(&mut engine, 1, "promising_vein");

    assert_eq!(
        published_reduction(&mut engine, 0, "gargantuan_leech"),
        2,
        "one controlled Cave plus one Cave card counts; non-Caves and opposing Caves do not"
    );

    engine.state.players[0].mana_pool.black = 1;
    engine.state.players[0].mana_pool.colorless = 5;
    let leech = hand_index_for_card(&engine, 0, "gargantuan_leech");
    engine
        .apply_command(0, &cast_spell(leech, vec![]))
        .expect("Gargantuan Leech costs {5}{B} with two matching Cave quantities");
    assert_eq!(engine.state.players[0].mana_pool.black, 0);
    assert_eq!(
        engine.state.players[0].mana_pool.colorless, 0,
        "the affine Cave reduction applies to the determined cost"
    );
}

#[test]
fn issue_370_tolarian_terror_counts_instant_and_sorcery_cards_and_not_itself() {
    let decks = Some(vec![
        deck_with("island", &["tolarian_terror"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(370_003, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    ensure_in_hand(&mut engine, 0, "tolarian_terror");

    inject_graveyard_card(&mut engine, 0, "abrade");
    inject_graveyard_card(&mut engine, 0, "divination");
    inject_graveyard_card(&mut engine, 0, "grizzly_bears");
    inject_graveyard_card(&mut engine, 0, "promising_vein");
    let graveyard_before = engine.state.players[0].graveyard.clone();

    assert_eq!(
        published_reduction(&mut engine, 0, "tolarian_terror"),
        2,
        "only the instant and sorcery cards count"
    );

    engine.state.players[0].mana_pool.blue = 1;
    engine.state.players[0].mana_pool.colorless = 4;
    let terror = hand_index_for_card(&engine, 0, "tolarian_terror");
    engine
        .apply_command(0, &cast_spell(terror, vec![]))
        .expect("Tolarian Terror costs {4}{U}; the spell on the stack is never in a graveyard");
    assert_eq!(engine.state.players[0].mana_pool.blue, 0);
    assert_eq!(
        engine.state.players[0].mana_pool.colorless, 0,
        "counting the spell itself would have made the generic cost smaller than four"
    );
    assert_eq!(
        engine.state.players[0].graveyard, graveyard_before,
        "the cast moves the card to the stack, not to a graveyard"
    );
}
