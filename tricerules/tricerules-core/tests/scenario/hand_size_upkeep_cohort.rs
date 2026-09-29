//! Actual-card upkeep scenarios for Ebony Owl Netsuke, Iron Maiden, Misers' Cage, and Viseling.
#![allow(unused_imports)] // The shared scenario helper module re-exports many helpers.

use super::helpers::*;
use tricerules_cards::{CardRegistry, Layout};
use tricerules_core::{GameEngine, TurnStep, Zone};

fn set_hand_size(engine: &mut GameEngine, player: usize, count: usize) {
    while engine.state.players[player].hand.len() > count {
        let object = engine.state.players[player]
            .hand
            .pop()
            .expect("card in hand");
        engine.state.objects.get_mut(&object).unwrap().zone = Zone::Graveyard;
        engine.state.players[player].graveyard.push(object);
    }
    while engine.state.players[player].hand.len() < count {
        inject_card_into_hand(engine, player, "forest");
    }
    assert_eq!(engine.state.players[player].hand.len(), count);
}

fn two_player_game_with_source(card_id: &str, seed: u64) -> GameEngine {
    let decks = Some(vec![island_only_deck(), island_only_deck()]);
    let mut engine = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    inject_permanent_on_battlefield(&mut engine, 0, card_id);
    engine
}

fn reach_opponent_upkeep(engine: &mut GameEngine) {
    end_active_turn(engine, 0);
    assert_eq!(engine.state.active_player_id(), 1);
    assert_eq!(engine.state.turn_step, TurnStep::Upkeep);
}

#[test]
fn hand_size_upkeep_cards_have_reviewed_identity_and_characteristics() {
    let registry = CardRegistry::global();
    for (id, name, mana_cost, types, power, toughness) in [
        (
            "ebony_owl_netsuke",
            "Ebony Owl Netsuke",
            "{2}",
            &["Artifact"][..],
            None,
            None,
        ),
        (
            "iron_maiden",
            "Iron Maiden",
            "{3}",
            &["Artifact"][..],
            None,
            None,
        ),
        (
            "misers_cage",
            "Misers' Cage",
            "{3}",
            &["Artifact"][..],
            None,
            None,
        ),
        (
            "viseling",
            "Viseling",
            "{4}",
            &["Artifact", "Creature", "Phyrexian", "Construct"][..],
            Some(2),
            Some(2),
        ),
    ] {
        let card = registry
            .get(id)
            .unwrap_or_else(|| panic!("{name} is registered"));
        assert_eq!(card.id, id);
        assert_eq!(card.name, name);
        assert_eq!(card.layout, Layout::Normal);
        assert_eq!(card.face_count(), 1);
        let face = card.primary_face();
        assert_eq!(face.face_id.as_str(), id);
        assert_eq!(face.name, name);
        assert_eq!(face.mana_cost.to_string(), mana_cost);
        assert_eq!(
            face.types,
            types
                .iter()
                .map(|kind| kind.to_string())
                .collect::<Vec<_>>()
        );
        assert!(face.colors().is_empty());
        assert_eq!(face.power, power);
        assert_eq!(face.toughness, toughness);
        assert!(face.keywords.is_empty());
        assert_eq!(face.triggered_abilities.len(), 1);
    }
}

#[test]
fn ebony_owl_netsuke_checks_seven_cards_at_opponents_upkeep_and_resolution() {
    let mut below = two_player_game_with_source("ebony_owl_netsuke", 202_609_291);
    set_hand_size(&mut below, 1, 6);
    reach_opponent_upkeep(&mut below);
    assert!(
        below.state.stack.is_empty(),
        "six cards do not trigger Netsuke"
    );
    assert_eq!(below.state.players[1].life, 20);

    let mut threshold = two_player_game_with_source("ebony_owl_netsuke", 202_609_292);
    set_hand_size(&mut threshold, 1, 7);
    reach_opponent_upkeep(&mut threshold);
    assert_eq!(
        threshold.state.stack.len(),
        1,
        "seven cards trigger Netsuke"
    );
    resolve_entire_stack_two_player(&mut threshold);
    assert_eq!(threshold.state.players[0].life, 20);
    assert_eq!(threshold.state.players[1].life, 16);

    let mut changed = two_player_game_with_source("ebony_owl_netsuke", 202_609_293);
    set_hand_size(&mut changed, 1, 7);
    reach_opponent_upkeep(&mut changed);
    assert_eq!(changed.state.stack.len(), 1);
    set_hand_size(&mut changed, 1, 6);
    resolve_entire_stack_two_player(&mut changed);
    assert_eq!(
        changed.state.players[1].life, 20,
        "the intervening condition is rechecked"
    );
}

#[test]
fn misers_cage_checks_five_cards_at_opponents_upkeep_and_resolution() {
    let mut above = two_player_game_with_source("misers_cage", 202_609_294);
    set_hand_size(&mut above, 1, 4);
    reach_opponent_upkeep(&mut above);
    assert!(
        above.state.stack.is_empty(),
        "four cards do not trigger Cage"
    );

    let mut threshold = two_player_game_with_source("misers_cage", 202_609_295);
    set_hand_size(&mut threshold, 1, 5);
    reach_opponent_upkeep(&mut threshold);
    assert_eq!(threshold.state.stack.len(), 1, "five cards trigger Cage");
    resolve_entire_stack_two_player(&mut threshold);
    assert_eq!(threshold.state.players[0].life, 20);
    assert_eq!(threshold.state.players[1].life, 18);

    let mut changed = two_player_game_with_source("misers_cage", 202_609_296);
    set_hand_size(&mut changed, 1, 5);
    reach_opponent_upkeep(&mut changed);
    assert_eq!(changed.state.stack.len(), 1);
    set_hand_size(&mut changed, 1, 4);
    resolve_entire_stack_two_player(&mut changed);
    assert_eq!(
        changed.state.players[1].life, 20,
        "the intervening condition is rechecked"
    );
}

#[test]
fn iron_maiden_and_viseling_read_live_hand_size_and_clamp_negative_damage() {
    for (index, card_id) in ["iron_maiden", "viseling"].into_iter().enumerate() {
        let mut growing = two_player_game_with_source(card_id, 202_609_297 + index as u64);
        set_hand_size(&mut growing, 1, 5);
        reach_opponent_upkeep(&mut growing);
        assert_eq!(growing.state.stack.len(), 1, "{card_id} triggers on upkeep");
        set_hand_size(&mut growing, 1, 7);
        resolve_entire_stack_two_player(&mut growing);
        assert_eq!(growing.state.players[0].life, 20);
        assert_eq!(
            growing.state.players[1].life, 17,
            "{card_id} reads seven cards on resolution"
        );

        let mut shrinking = two_player_game_with_source(card_id, 202_609_299 + index as u64);
        set_hand_size(&mut shrinking, 1, 5);
        reach_opponent_upkeep(&mut shrinking);
        assert_eq!(shrinking.state.stack.len(), 1);
        set_hand_size(&mut shrinking, 1, 3);
        resolve_entire_stack_two_player(&mut shrinking);
        assert_eq!(
            shrinking.state.players[1].life, 20,
            "{card_id} clamps damage to zero"
        );
    }
}

fn pass_all_players(engine: &mut GameEngine) {
    let count = engine
        .state
        .players
        .iter()
        .filter(|player| !player.has_lost)
        .count();
    for _ in 0..count {
        answer_trigger_order_in_engine_order(engine);
        let player = engine.state.priority_player_id();
        engine
            .apply_command(player, &pass())
            .expect("pass priority");
    }
}

#[test]
fn opponent_upkeep_damage_covers_each_opponent_in_three_player_game() {
    let decks = Some(vec![
        island_only_deck(),
        island_only_deck(),
        island_only_deck(),
    ]);
    let mut engine =
        GameEngine::new(202_609_301, &[0, 1, 2], 20, decks, true).expect("new three-player game");
    inject_permanent_on_battlefield(&mut engine, 0, "viseling");
    set_hand_size(&mut engine, 0, 7);
    set_hand_size(&mut engine, 1, 5);
    set_hand_size(&mut engine, 2, 6);

    pass_all_players(&mut engine); // P0 upkeep to draw.
    pass_all_players(&mut engine); // P0 draw to main one.
    assert_eq!(engine.state.turn_step, TurnStep::Main1);
    assert_eq!(
        engine.state.players[0].life, 20,
        "controller's upkeep is unaffected"
    );

    end_active_turn(&mut engine, 0);
    assert_eq!(engine.state.active_player_id(), 1);
    assert_eq!(engine.state.stack.len(), 1);
    pass_all_players(&mut engine);
    assert_eq!(engine.state.players[1].life, 19);
    assert_eq!(engine.state.players[0].life, 20);
    assert_eq!(engine.state.players[2].life, 20);

    pass_all_players(&mut engine); // P1 upkeep to draw.
    pass_all_players(&mut engine); // P1 draw to main one.
    assert_eq!(engine.state.turn_step, TurnStep::Main1);
    end_active_turn(&mut engine, 1);
    assert_eq!(engine.state.active_player_id(), 2);
    assert_eq!(engine.state.stack.len(), 1);
    pass_all_players(&mut engine);
    assert_eq!(engine.state.players[2].life, 18);
    assert_eq!(engine.state.players[0].life, 20);
    assert_eq!(engine.state.players[1].life, 19);
}
