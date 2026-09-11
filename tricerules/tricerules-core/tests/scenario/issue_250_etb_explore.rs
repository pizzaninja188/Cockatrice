//! Issue #250 — generated ETB Explore cards reuse the established Explore continuation.
//!
//! Oracle and rulings checked 2026-09-11. CR 701.44 governs Explore; CR 603 governs the
//! source-bound ETB trigger. The established action retains the source's last-known controller
//! if it leaves before resolution and cannot put a counter on a departed permanent.

use super::helpers::*;
use tricerules_cards::CounterKind;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::{ruled_event, ChoiceKind};

fn put_on_top(engine: &mut GameEngine, player: usize, card_id: &str) -> u32 {
    let object_id = inject_library_card(engine, player, card_id);
    engine.state.players[player]
        .library
        .retain(|candidate| *candidate != object_id);
    engine.state.players[player].library.push_front(object_id);
    object_id
}

fn resolve_cenote_scout(engine: &mut GameEngine) -> u32 {
    ensure_in_hand(engine, 0, "cenote_scout");
    give_mana(
        engine,
        0,
        ManaGift {
            g: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(engine, 0, "cenote_scout");
    engine
        .apply_command(0, &cast_spell(slot, vec![]))
        .expect("cast Cenote Scout");
    pass_both_players(engine);
    let scout = battlefield_object_for_card(engine, 0, "cenote_scout");
    assert!(engine
        .state
        .stack
        .last()
        .is_some_and(|item| item.is_triggered));
    scout
}

#[test]
fn generated_etb_explore_reveals_and_moves_a_land_to_hand() {
    let decks = Some(vec![
        deck_with("forest", &["cenote_scout"]),
        deck_with("swamp", &[]),
    ]);
    let mut engine = GameEngine::new(25001, &[0, 1], 20, decks, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    let scout = resolve_cenote_scout(&mut engine);
    let land = put_on_top(&mut engine, 0, "forest");

    let first = engine.state.priority_player_id();
    engine.apply_command(first, &pass()).expect("first pass");
    let batch = engine
        .apply_command(1 - first, &pass())
        .expect("resolve generated Explore trigger");
    let reveal = batch
        .events
        .iter()
        .find_map(|event| match &event.ev {
            Some(ruled_event::Ev::CardsRevealed(reveal)) => Some(reveal),
            _ => None,
        })
        .expect("Explore publishes the land reveal");

    assert_eq!(reveal.cards.len(), 1);
    assert_eq!(reveal.cards[0].object_id, land);
    assert_eq!(reveal.cards[0].card_name, "Forest");
    assert_eq!(engine.state.objects[&land].zone, Zone::Hand);
    assert_eq!(
        engine.state.objects[&scout].counter_count(CounterKind::PlusOnePlusOne),
        0
    );
}

#[test]
fn generated_etb_explore_uses_source_lki_after_the_scout_dies() {
    let decks = Some(vec![
        deck_with("forest", &["cenote_scout", "storm_crow"]),
        deck_with("swamp", &["murder"]),
    ]);
    let mut engine = GameEngine::new(25002, &[0, 1], 20, decks, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    let scout = resolve_cenote_scout(&mut engine);
    let nonland = put_on_top(&mut engine, 0, "storm_crow");
    ensure_in_hand(&mut engine, 1, "murder");

    engine
        .apply_command(0, &pass())
        .expect("yield to responder");
    give_mana(
        &mut engine,
        1,
        ManaGift {
            b: 2,
            c: 1,
            ..Default::default()
        },
    );
    let murder = hand_index_for_card(&engine, 1, "murder");
    engine
        .apply_command(1, &cast_spell(murder, target_object(scout)))
        .expect("destroy Scout before its trigger resolves");
    pass_both_players(&mut engine);
    assert_eq!(engine.state.objects[&scout].zone, Zone::Graveyard);

    let first = engine.state.priority_player_id();
    engine.apply_command(first, &pass()).expect("first pass");
    let batch = engine
        .apply_command(1 - first, &pass())
        .expect("resolve Explore with departed source");
    let choice = find_resolution_choice(&batch).expect("nonland Explore choice");
    assert_eq!(choice.choice_kind(), ChoiceKind::LibraryLook);
    assert_eq!(choice.deciding_player_id, 0);
    assert_eq!(choice.candidate_object_ids, [nonland]);
    assert!(choice.public_reveal.is_some());
    assert_eq!(
        engine.state.objects[&scout].counter_count(CounterKind::PlusOnePlusOne),
        0
    );

    engine
        .apply_command(0, &submit_resolution_choice(vec![nonland]))
        .expect("put the revealed nonland into the graveyard");
    assert_eq!(engine.state.objects[&nonland].zone, Zone::Graveyard);
}
