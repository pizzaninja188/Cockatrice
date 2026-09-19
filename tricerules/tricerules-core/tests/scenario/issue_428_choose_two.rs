//! Issue #428 focused scenarios for Return from the Wilds, the retained `Choose two —` identity.
//!
//! Oracle and the current Comprehensive Rules were verified 2026-09-19 against the pinned
//! Scryfall snapshot. CR 700.2/700.2a (mode announcement at cast time), CR 701.23 (search and
//! shuffle), CR 110.5b/614.1d (tapped entry), and CR 111.1/111.4 (token creation and identity)
//! govern the exercised behavior. The five Command identities stay unregistered because their
//! remaining bullets lack shipped vocabulary; their retained halves are covered by the recipe
//! catalog tests.

use super::helpers::*;
use tricerules_core::{GameEngine, Zone};
use tricerules_proto::ruled::v1::ChoiceKind;

fn modal_engine(seed: u64) -> GameEngine {
    // Explicit land-only decks keep the default grizzly_bears/library fixtures out of the
    // opening hand so injected fixture names stay unique.
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

fn battlefield_token(engine: &GameEngine, player: usize, card_id: &str) -> u32 {
    engine.state.players[player]
        .battlefield
        .iter()
        .copied()
        .find(|oid| {
            engine.state.objects[oid].is_token() && engine.state.objects[oid].card_id == card_id
        })
        .unwrap_or_else(|| panic!("expected a {card_id} token on P{player}'s battlefield"))
}

#[test]
fn issue_428_return_from_the_wilds_searches_for_a_basic_land_and_makes_a_human() {
    let mut engine = modal_engine(428_001);
    let forest = inject_library_card(&mut engine, 0, "forest");
    let sword = inject_library_card(&mut engine, 0, "short_sword");
    let slot = prepare_spell(&mut engine, "return_from_the_wilds");
    engine
        .apply_command(0, &cast_modal_spell(slot, vec![(0, vec![]), (1, vec![])]))
        .expect("cast the search and Human modes");
    engine
        .apply_command(0, &pass())
        .expect("the caster passes priority");
    let parked = engine
        .apply_command(1, &pass())
        .expect("resolution parks for the private search");
    let choice = find_resolution_choice(&parked).expect("basic-land search");
    assert_eq!(choice.choice_kind(), ChoiceKind::LibrarySearch);
    assert_eq!(choice.deciding_player_id, 0);
    assert!(choice.candidate_object_ids.contains(&forest));
    assert!(
        !choice.candidate_object_ids.contains(&sword),
        "only basic land cards are legal search candidates"
    );
    engine
        .apply_command(0, &submit_resolution_choice(vec![forest]))
        .expect("find the basic land");

    let found = &engine.state.objects[&forest];
    assert_eq!(found.zone, Zone::Battlefield);
    assert_eq!(found.controller, 0);
    assert!(found.tapped, "CR 614.1d: the printed basic enters tapped");
    let human = battlefield_token(&engine, 0, "human_w_1_1");
    assert_eq!(engine.effective_power(human), Some(1));
    assert_eq!(engine.effective_toughness(human), Some(1));
    assert!(
        engine.state.pending_resolution.is_none(),
        "resolution completes after the search choice"
    );
}

#[test]
fn issue_428_return_from_the_wilds_searches_then_creates_the_food_token() {
    let mut engine = modal_engine(428_002);
    let forest = inject_library_card(&mut engine, 0, "forest");
    let slot = prepare_spell(&mut engine, "return_from_the_wilds");
    engine
        .apply_command(0, &cast_modal_spell(slot, vec![(0, vec![]), (2, vec![])]))
        .expect("cast the search and Food modes");
    engine
        .apply_command(0, &pass())
        .expect("the caster passes priority");
    let parked = engine
        .apply_command(1, &pass())
        .expect("resolution parks for the private search");
    assert!(find_resolution_choice(&parked).is_some());
    engine
        .apply_command(0, &submit_resolution_choice(vec![forest]))
        .expect("find the basic land");
    assert_eq!(engine.state.objects[&forest].zone, Zone::Battlefield);
    assert!(engine.state.objects[&forest].tapped);
    let food = battlefield_token(&engine, 0, "food");
    assert!(engine.state.objects[&food].is_token());
    assert!(
        engine.state.objects[&food]
            .token_origin
            .as_ref()
            .is_some_and(|origin| origin
                .face
                .types
                .iter()
                .any(|card_type| card_type == "Food")),
        "the created Food token keeps its predefined artifact subtype"
    );
}

#[test]
fn issue_428_return_from_the_wilds_creates_both_tokens_without_searching() {
    let mut engine = modal_engine(428_003);
    let slot = prepare_spell(&mut engine, "return_from_the_wilds");
    engine
        .apply_command(0, &cast_modal_spell(slot, vec![(1, vec![]), (2, vec![])]))
        .expect("cast the Human and Food modes");
    resolve_entire_stack_two_player(&mut engine);
    let human = battlefield_token(&engine, 0, "human_w_1_1");
    let food = battlefield_token(&engine, 0, "food");
    assert!(engine.state.objects[&human].is_token());
    assert!(engine.state.objects[&food].is_token());
    assert_eq!(engine.effective_power(human), Some(1));
    assert_eq!(engine.effective_toughness(food), None);
    assert!(engine.state.pending_resolution.is_none());
}

#[test]
fn issue_428_return_from_the_wilds_rejects_illegal_mode_selections() {
    let mut engine = modal_engine(428_004);
    let slot = prepare_spell(&mut engine, "return_from_the_wilds");
    for illegal in [
        // CR 700.2: the printed aggregate requires exactly two modes.
        vec![(0, vec![])],
        vec![(0, vec![]), (1, vec![]), (2, vec![])],
        // A mode cannot be chosen twice, and mode 3 does not print.
        vec![(1, vec![]), (1, vec![])],
        vec![(0, vec![]), (3, vec![])],
    ] {
        assert!(
            engine
                .apply_command(0, &cast_modal_spell(slot, illegal.clone()))
                .is_err(),
            "illegal mode selection {illegal:?} must fail closed"
        );
    }
    // The same hand slot still casts the legal reviewed pair after every rejection.
    engine
        .apply_command(0, &cast_modal_spell(slot, vec![(1, vec![]), (2, vec![])]))
        .expect("the legal pair still casts");
    resolve_entire_stack_two_player(&mut engine);
    assert!(engine.state.players[0]
        .battlefield
        .iter()
        .any(|oid| engine.state.objects[oid].card_id == "human_w_1_1"));
    assert!(engine.state.players[0]
        .battlefield
        .iter()
        .any(|oid| engine.state.objects[oid].card_id == "food"));
}
