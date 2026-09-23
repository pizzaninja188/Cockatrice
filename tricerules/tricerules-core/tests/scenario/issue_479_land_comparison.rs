//! Actual-card scenarios for Ticket Tortoise and Sunstar Expansionist.
//!
//! The land comparison is checked at ETB trigger creation and again at resolution (CR 603.4).
//! Current land types and controller-relative opponents are evaluated from public battlefield
//! characteristics; the individual opponent counts are not combined. Sunstar's distinct landfall
//! pump uses the existing permanent-entry trigger and until-end-of-turn modifier (CR 603.6a,
//! 611.2a, 514.2). Token events carry public object identity and characteristics (CR 111).

use super::helpers::*;
use tricerules_core::{GameEngine, Zone};
use tricerules_proto::ruled::v1::TokenCreated;

fn main_phase(seed: u64) -> GameEngine {
    semantic::main_phase(seed)
}

fn add_lands(engine: &mut GameEngine, player: usize, count: usize, card: &str) -> Vec<u32> {
    (0..count)
        .map(|_| inject_permanent_on_battlefield(engine, player, card))
        .collect()
}

/// Cast a creature through the command boundary and resolve only that permanent spell.
fn cast_permanent(engine: &mut GameEngine, card: &str) -> u32 {
    let source = inject_card_into_hand(engine, 0, card);
    let hand_slot = hand_index_for_card(engine, 0, card);
    grant_pool(engine, 0);
    semantic::accepted(engine, 0, &cast_spell(hand_slot, vec![]));
    for _ in 0..2 {
        let priority = engine.state.priority_player_id();
        semantic::accepted(engine, priority, &pass());
    }
    assert_eq!(
        engine.state.objects[&source].zone,
        Zone::Battlefield,
        "{card} resolves through the real cast path"
    );
    source
}

fn remove_land(engine: &mut GameEngine, player: usize, object_id: u32) {
    engine.state.players[player]
        .battlefield
        .retain(|candidate| *candidate != object_id);
    engine.state.players[player].graveyard.push(object_id);
    engine
        .state
        .objects
        .get_mut(&object_id)
        .expect("land object")
        .zone = Zone::Graveyard;
    *engine
        .state
        .zone_change_generation
        .entry(object_id)
        .or_default() += 1;
}

fn resolve_and_collect_tokens(engine: &mut GameEngine) -> Vec<TokenCreated> {
    let mut created = Vec::new();
    for _ in 0..32 {
        if engine.state.stack.is_empty() {
            return created;
        }
        answer_trigger_order_in_engine_order(engine);
        for _ in 0..2 {
            if engine.state.stack.is_empty() {
                return created;
            }
            let priority = engine.state.priority_player_id();
            let batch = semantic::accepted(engine, priority, &pass());
            created.extend(token_created_events(&batch).into_iter().cloned());
        }
    }
    panic!("stack did not resolve within the bounded token scenario");
}

fn assert_created_token(engine: &GameEngine, created: &[TokenCreated], token_id: &str, name: &str) {
    assert_eq!(created.len(), 1, "one {name} TokenCreated event");
    let token = &created[0];
    assert_eq!(token.controller_player_id, 0);
    assert_eq!(token.card_id, token_id);
    let identity = token.identity.as_ref().expect("public token identity");
    assert_eq!(identity.name, name);
    let object = engine
        .state
        .objects
        .get(&token.object_id)
        .expect("token object");
    assert_eq!(object.card_id, token_id);
    assert_eq!(object.zone, Zone::Battlefield);
    assert!(engine.state.players[0]
        .battlefield
        .contains(&token.object_id));
}

#[test]
fn issue_479_ticket_tortoise_creates_treasure_only_when_one_opponent_has_more_lands() {
    let mut successful = main_phase(479_101);
    add_lands(&mut successful, 0, 1, "forest");
    add_lands(&mut successful, 1, 2, "island");
    cast_permanent(&mut successful, "ticket_tortoise");
    assert_eq!(
        successful.state.stack.len(),
        1,
        "the ETB trigger is created"
    );
    let created = resolve_and_collect_tokens(&mut successful);
    assert_created_token(&successful, &created, "treasure", "Treasure");

    // Equal counts are false at the ETB event. Adding a later opponent land does not create a
    // trigger retroactively.
    let mut false_at_entry = main_phase(479_102);
    add_lands(&mut false_at_entry, 0, 2, "forest");
    add_lands(&mut false_at_entry, 1, 2, "island");
    cast_permanent(&mut false_at_entry, "ticket_tortoise");
    assert!(
        false_at_entry.state.stack.is_empty(),
        "equality creates no trigger"
    );
    add_lands(&mut false_at_entry, 1, 1, "island");
    assert!(
        false_at_entry.state.stack.is_empty(),
        "a later count change is not retroactive"
    );
    assert!(resolve_and_collect_tokens(&mut false_at_entry).is_empty());
    assert!(battlefield_token_oids(&false_at_entry, 0, "treasure").is_empty());

    // The condition is rechecked at resolution; losing the opponent's excess land makes it false.
    let mut false_at_resolution = main_phase(479_103);
    add_lands(&mut false_at_resolution, 0, 1, "forest");
    let opponent_lands = add_lands(&mut false_at_resolution, 1, 2, "island");
    cast_permanent(&mut false_at_resolution, "ticket_tortoise");
    assert_eq!(
        false_at_resolution.state.stack.len(),
        1,
        "triggered while true at entry"
    );
    remove_land(&mut false_at_resolution, 1, opponent_lands[0]);
    assert!(
        resolve_and_collect_tokens(&mut false_at_resolution).is_empty(),
        "equal counts at resolution create no Treasure"
    );
    assert!(battlefield_token_oids(&false_at_resolution, 0, "treasure").is_empty());
}

#[test]
fn issue_479_sunstar_expansionist_creates_lander_and_keeps_its_landfall_pump() {
    let mut engine = main_phase(479_201);
    add_lands(&mut engine, 0, 1, "forest");
    add_lands(&mut engine, 1, 2, "island");
    let sunstar = cast_permanent(&mut engine, "sunstar_expansionist");
    assert_eq!(
        engine.state.stack.len(),
        1,
        "the conditioned ETB trigger is created"
    );

    let created = resolve_and_collect_tokens(&mut engine);
    assert_created_token(&engine, &created, "lander", "Lander");
    assert_eq!(engine.effective_power(sunstar), Some(2));
    assert_eq!(engine.effective_toughness(sunstar), Some(3));

    let land = inject_card_into_hand(&mut engine, 0, "forest");
    let hand_slot = hand_index_for_card(&engine, 0, "forest");
    semantic::accepted(&mut engine, 0, &play_land(hand_slot));
    assert!(
        !engine.state.stack.is_empty(),
        "the landfall trigger is created"
    );
    resolve_and_collect_tokens(&mut engine);
    assert_eq!(engine.state.objects[&land].zone, Zone::Battlefield);
    assert_eq!(engine.effective_power(sunstar), Some(3));
    assert_eq!(engine.effective_toughness(sunstar), Some(3));

    end_active_turn(&mut engine, 0);
    assert_eq!(
        engine.effective_power(sunstar),
        Some(2),
        "the pump expires at cleanup"
    );
    assert_eq!(engine.effective_toughness(sunstar), Some(3));
}
