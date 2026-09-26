//! Exact deck-corpus coverage for Quicksmith Genius.
//!
//! Oracle text and the Scryfall rulings endpoint were checked on 2026-09-26; no rulings were
//! returned. CR 603.2/603.3 governs artifact-entry triggers. CR 118.12 makes the optional discard
//! the resolving action, and CR 701.9/121.1 govern the discard and subsequent draw.

use super::helpers::*;
use tricerules_core::{GameEngine, Zone};
use tricerules_proto::ruled::v1::ChoiceKind;

const QUICKSMITH_GENIUS: &str = "quicksmith_genius";

fn quicksmith_engine(seed: u64) -> GameEngine {
    let mut engine = GameEngine::new(
        seed,
        &[0, 1],
        20,
        Some(vec![forest_only_deck(), forest_only_deck()]),
        true,
    )
    .expect("new Quicksmith Genius engine");
    advance_to_main1_from_game_start(&mut engine);
    inject_creature_on_battlefield(&mut engine, 0, QUICKSMITH_GENIUS);
    engine
}

fn resolve_top_stack_item(engine: &mut GameEngine) -> tricerules_proto::ruled::v1::RuledEventBatch {
    answer_trigger_order_in_engine_order(engine);
    let first = engine.state.priority_player_id();
    let second = if first == engine.state.players[0].id {
        engine.state.players[1].id
    } else {
        engine.state.players[0].id
    };
    engine
        .apply_command(first, &pass())
        .expect("first player passes priority");
    engine
        .apply_command(second, &pass())
        .expect("second player passes priority and the top item resolves")
}

fn cast_sol_ring_and_resolve_quicksmith_trigger(
    engine: &mut GameEngine,
) -> (u32, tricerules_proto::ruled::v1::RuledEventBatch) {
    let sol_ring = inject_card_into_hand(engine, 0, "sol_ring");
    give_mana(
        engine,
        0,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(engine, 0, "sol_ring");
    engine
        .apply_command(0, &cast_spell(slot, vec![]))
        .expect("cast Sol Ring");

    let _ = resolve_top_stack_item(engine);
    assert_eq!(engine.state.objects[&sol_ring].zone, Zone::Battlefield);
    assert_eq!(
        engine.state.pending_triggers.len() + engine.state.stack.len(),
        1,
        "the controller's artifact entry creates exactly one Quicksmith Genius trigger"
    );

    let trigger_resolution = resolve_top_stack_item(engine);
    (sol_ring, trigger_resolution)
}

#[test]
fn quicksmith_genius_loots_once_for_each_controlled_artifact_entry() {
    let mut engine = quicksmith_engine(202_609_261);
    let discarded = inject_card_into_hand(&mut engine, 0, "grizzly_bears");
    let hand_before = engine.state.players[0].hand.clone();
    let library_before = engine.state.players[0].library.len();
    let drawn = engine.state.players[0]
        .library
        .iter()
        .next()
        .copied()
        .expect("library card to draw");

    let (_sol_ring, choice_batch) = cast_sol_ring_and_resolve_quicksmith_trigger(&mut engine);
    let choice = find_resolution_choice(&choice_batch).expect("optional discard choice");
    assert_eq!(choice.deciding_player_id, 0);
    assert_eq!(choice.choice_kind(), ChoiceKind::HandCards);
    assert_eq!((choice.min, choice.max), (0, 1));
    assert!(choice.candidate_object_ids.contains(&discarded));
    assert_eq!(engine.state.players[0].library.len(), library_before);

    engine
        .apply_command(0, &submit_resolution_choice(vec![discarded]))
        .expect("discard the selected card");
    assert_eq!(engine.state.objects[&discarded].zone, Zone::Graveyard);
    assert_eq!(engine.state.players[0].library.len(), library_before - 1);
    assert!(engine.state.players[0].hand.contains(&drawn));
    assert_eq!(
        engine.state.players[0].hand.len(),
        hand_before.len(),
        "one card was cast, one was discarded, and one was drawn"
    );
    assert!(engine.state.stack.is_empty());
    assert!(engine.state.pending_resolution.is_none());
}

#[test]
fn quicksmith_genius_may_decline_the_optional_discard() {
    let mut engine = quicksmith_engine(202_609_262);
    let discarded = inject_card_into_hand(&mut engine, 0, "grizzly_bears");
    let library_before = engine.state.players[0].library.len();

    let (_sol_ring, choice_batch) = cast_sol_ring_and_resolve_quicksmith_trigger(&mut engine);
    let choice = find_resolution_choice(&choice_batch).expect("optional discard choice");
    assert_eq!((choice.min, choice.max), (0, 1));
    engine
        .apply_command(0, &submit_resolution_choice(vec![]))
        .expect("decline the optional discard");

    assert_eq!(engine.state.objects[&discarded].zone, Zone::Hand);
    assert_eq!(engine.state.players[0].library.len(), library_before);
    assert!(engine.state.stack.is_empty());
    assert!(engine.state.pending_resolution.is_none());
}
