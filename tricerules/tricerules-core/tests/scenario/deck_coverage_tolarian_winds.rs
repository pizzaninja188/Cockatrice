//! Exact pinned-deck coverage for Tolarian Winds.
//!
//! Oracle and rulings checked 2026-09-30. CR 701.9a defines a discard; CR 121.2 governs each
//! draw. The resolving spell is already on the stack and is not part of the discarded cohort.

use super::helpers::*;
use tricerules_core::Zone;

const TOLARIAN_WINDS: &str = "tolarian_winds";

fn winds_engine(seed: u64) -> GameEngine {
    let mut engine = GameEngine::new(seed, &[0, 1], 20, None, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    clear_hand(&mut engine, 0);
    engine
}

fn clear_hand(engine: &mut GameEngine, player: usize) {
    while let Some(object_id) = engine.state.players[player].hand.pop() {
        engine.state.objects.get_mut(&object_id).unwrap().zone = Zone::Graveyard;
        engine.state.players[player].graveyard.push(object_id);
    }
    assert!(engine.state.players[player].hand.is_empty());
}

fn seat_on_top(engine: &mut GameEngine, card_ids: &[&str]) -> Vec<u32> {
    let objects: Vec<u32> = card_ids
        .iter()
        .map(|card_id| inject_library_card(engine, 0, card_id))
        .collect();
    engine.state.players[0]
        .library
        .retain(|object_id| !objects.contains(object_id));
    for object_id in objects.iter().rev() {
        engine.state.players[0].library.push_front(*object_id);
    }
    objects
}

fn resolve_top_spell(engine: &mut GameEngine) {
    let first = engine.state.priority_player_id();
    let second = if first == engine.state.players[0].id {
        engine.state.players[1].id
    } else {
        engine.state.players[0].id
    };
    engine
        .apply_command(first, &pass())
        .expect("first player passes");
    engine
        .apply_command(second, &pass())
        .expect("second player passes and the spell resolves");
}

#[test]
fn tolarian_winds_draws_the_number_of_cards_actually_discarded() {
    let mut engine = winds_engine(20_260_930);
    let drawn = seat_on_top(&mut engine, &["forest", "island", "hill_giant"]);
    let spell = inject_card_into_hand(&mut engine, 0, TOLARIAN_WINDS);
    let discarded: Vec<u32> = ["hill_giant", "mountain", "grizzly_bears"]
        .into_iter()
        .map(|card_id| inject_card_into_hand(&mut engine, 0, card_id))
        .collect();
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 1,
            u: 1,
            ..Default::default()
        },
    );

    let slot = hand_index_for_card(&engine, 0, TOLARIAN_WINDS);
    semantic::accepted(&mut engine, 0, &cast_spell_face(slot, vec![], 0));
    assert_eq!(engine.state.objects[&spell].zone, Zone::Stack);
    assert_eq!(engine.state.players[0].hand.len(), 3);

    resolve_top_spell(&mut engine);

    assert_eq!(engine.state.players[0].hand.len(), 3);
    assert!(discarded
        .iter()
        .all(|object_id| engine.state.objects[object_id].zone == Zone::Graveyard));
    assert!(drawn
        .iter()
        .all(|object_id| engine.state.objects[object_id].zone == Zone::Hand));
    assert_eq!(engine.state.objects[&spell].zone, Zone::Graveyard);
}

#[test]
fn tolarian_winds_does_not_count_itself_and_does_nothing_with_no_hand_cards() {
    let mut engine = winds_engine(20_260_931);
    let library_before = engine.state.players[0].library.len();
    let spell = inject_card_into_hand(&mut engine, 0, TOLARIAN_WINDS);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 1,
            u: 1,
            ..Default::default()
        },
    );

    let slot = hand_index_for_card(&engine, 0, TOLARIAN_WINDS);
    semantic::accepted(&mut engine, 0, &cast_spell_face(slot, vec![], 0));
    assert_eq!(engine.state.objects[&spell].zone, Zone::Stack);
    assert!(engine.state.players[0].hand.is_empty());
    resolve_top_spell(&mut engine);

    assert_eq!(engine.state.players[0].library.len(), library_before);
    assert!(engine.state.players[0].hand.is_empty());
    assert_eq!(engine.state.objects[&spell].zone, Zone::Graveyard);
}
