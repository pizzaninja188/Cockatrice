//! Issue #363 — the four reviewed Standard Dragonstorm enchantments.
//!
//! These scenarios drive the generated definitions through the authoritative command path. CR
//! 603.6a/400.3 govern the shared return-self trigger on a controlled Dragon entry; CR
//! 603.6a/119.3/701.25 the Corroding drain plus surveil; CR 603.6a/701.23/614.1d the Encroaching
//! up-to-two basic-land search onto the battlefield tapped; CR 603.6a/121.1/701.9 the Roiling
//! draw-two-then-discard; and CR 603.6a/111.1 the Teeming two 2/2 white Soldier tokens.

use super::helpers::*;
use tricerules_core::Zone;

fn main1_engine(seed: u64, own: &[&str], opposing: &[&str]) -> GameEngine {
    let decks = Some(vec![
        deck_with("forest", own),
        deck_with("forest", opposing),
    ]);
    let mut engine = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

/// Put `card_ids` on top of `player`'s library, first entry on top, and return their OIDs.
fn seat_on_top(e: &mut GameEngine, player: usize, card_ids: &[&str]) -> Vec<u32> {
    let oids: Vec<u32> = card_ids
        .iter()
        .map(|card_id| inject_library_card(e, player, card_id))
        .collect();
    e.state.players[player]
        .library
        .retain(|oid| !oids.contains(oid));
    for &oid in oids.iter().rev() {
        e.state.players[player].library.push_front(oid);
    }
    oids
}

fn resolve_top_stack(e: &mut GameEngine) -> RuledEventBatch {
    answer_trigger_order_in_engine_order(e);
    let first = e.state.priority_player_id();
    let second = 1 - first;
    e.apply_command(first, &pass())
        .expect("first pass on stack item");
    e.apply_command(second, &pass())
        .expect("second pass resolves stack item")
}

#[test]
fn issue_363_controlled_dragon_returns_teeming_dragonstorm_to_hand() {
    let mut engine = main1_engine(363_001, &["teeming_dragonstorm", "boulderborn_dragon"], &[]);
    let storm = move_ready_to_battlefield(&mut engine, 0, "teeming_dragonstorm");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        battlefield_token_oids(&engine, 0, "soldier_w_2_2").len(),
        2,
        "the enchantment's own entry created its two Soldiers"
    );
    assert!(engine.state.players[0].battlefield.contains(&storm));

    let dragon = move_ready_to_battlefield(&mut engine, 0, "boulderborn_dragon");
    assert_eq!(engine.state.objects[&dragon].zone, Zone::Battlefield);
    assert!(
        !engine.state.stack.is_empty() || !engine.state.pending_triggers.is_empty(),
        "a controlled Dragon entry must trigger the return-self ability"
    );
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        engine.state.objects[&storm].zone,
        Zone::Hand,
        "CR 603.6a/400.3: the enchantment returns to its owner's hand"
    );
    assert!(engine.state.players[0].hand.contains(&storm));
    assert!(!engine.state.players[0].battlefield.contains(&storm));
}

#[test]
fn issue_363_opponents_dragon_does_not_return_the_enchantment() {
    let mut engine = main1_engine(363_002, &["teeming_dragonstorm"], &["boulderborn_dragon"]);
    let storm = move_ready_to_battlefield(&mut engine, 0, "teeming_dragonstorm");
    resolve_entire_stack_two_player(&mut engine);

    let dragon = move_ready_to_battlefield(&mut engine, 1, "boulderborn_dragon");
    assert_eq!(engine.state.objects[&dragon].zone, Zone::Battlefield);
    assert!(
        engine.state.stack.is_empty() && engine.state.pending_triggers.is_empty(),
        "CR 603.6a: an opponent's Dragon is outside the controlled scope"
    );
    assert_eq!(
        engine.state.objects[&storm].zone,
        Zone::Battlefield,
        "the enchantment stays on the battlefield for an opponent's Dragon"
    );
}

#[test]
fn issue_363_corroding_dragonstorm_drains_and_surveils() {
    let mut engine = main1_engine(363_003, &["corroding_dragonstorm"], &[]);
    let top = seat_on_top(&mut engine, 0, &["grizzly_bears", "storm_crow"]);
    let storm = move_ready_to_battlefield(&mut engine, 0, "corroding_dragonstorm");
    let resolution = resolve_top_stack(&mut engine);
    let choice = find_resolution_choice(&resolution).expect("surveil 2 choice");
    assert_eq!(choice.choice_kind(), ChoiceKind::LibraryLook);
    assert_eq!(choice.deciding_player_id, 0);
    assert_eq!((choice.min, choice.max), (0, 2));
    assert_eq!(choice.candidate_object_ids, top);

    engine
        .apply_command(0, &submit_resolution_choice(vec![top[0]]))
        .expect("put one surveilled card into the graveyard");
    assert_eq!(
        (engine.state.players[0].life, engine.state.players[1].life),
        (22, 18),
        "CR 119.3/701.25: each opponent loses 2, the controller gains 2, then surveils"
    );
    assert!(engine.state.players[0].graveyard.contains(&top[0]));
    assert!(engine.state.players[0].library.contains(&top[1]));
    assert_eq!(
        engine.state.objects[&storm].zone,
        Zone::Battlefield,
        "no Dragon entered, so the enchantment stays"
    );
}

#[test]
fn issue_363_encroaching_dragonstorm_searches_two_basic_lands_tapped() {
    let mut engine = main1_engine(363_004, &["encroaching_dragonstorm"], &[]);
    let storm = move_ready_to_battlefield(&mut engine, 0, "encroaching_dragonstorm");
    let library_before = engine.state.players[0].library.len();
    let resolution = resolve_top_stack(&mut engine);
    let choice = find_resolution_choice(&resolution).expect("basic-land search choice");
    assert_eq!((choice.min, choice.max), (0, 2));
    assert!(
        choice.candidate_object_ids.len() >= 2,
        "the library must expose at least two basic lands"
    );

    let chosen = vec![
        choice.candidate_object_ids[0],
        choice.candidate_object_ids[1],
    ];
    engine
        .apply_command(0, &submit_resolution_choice(chosen.clone()))
        .expect("find two basic lands");
    for oid in &chosen {
        assert_eq!(engine.state.objects[oid].zone, Zone::Battlefield);
        assert!(
            engine.state.objects[oid].tapped,
            "CR 614.1d: the searched lands enter tapped"
        );
        assert!(engine.state.players[0].battlefield.contains(oid));
    }
    assert_eq!(
        engine.state.players[0].library.len(),
        library_before - 2,
        "CR 701.23: both chosen lands left the library, which was then shuffled"
    );
    assert_eq!(engine.state.objects[&storm].zone, Zone::Battlefield);
}

#[test]
fn issue_363_roiling_dragonstorm_draws_two_then_discards_one() {
    let mut engine = main1_engine(363_005, &["roiling_dragonstorm"], &[]);
    let storm = move_ready_to_battlefield(&mut engine, 0, "roiling_dragonstorm");
    let hand_before = engine.state.players[0].hand.len();
    let library_before = engine.state.players[0].library.len();
    let resolution = resolve_top_stack(&mut engine);
    let choice = find_resolution_choice(&resolution).expect("discard choice");
    assert_eq!(choice.deciding_player_id, 0);
    let discard = choice
        .candidate_object_ids
        .first()
        .copied()
        .expect("a discardable card");
    engine
        .apply_command(0, &submit_resolution_choice(vec![discard]))
        .expect("discard one card");
    assert_eq!(
        engine.state.players[0].hand.len(),
        hand_before + 1,
        "CR 121.1/701.9: draw two then discard one"
    );
    assert_eq!(engine.state.players[0].library.len(), library_before - 2);
    assert!(engine.state.players[0].graveyard.contains(&discard));
    assert_eq!(engine.state.objects[&storm].zone, Zone::Battlefield);
}

#[test]
fn issue_363_teeming_dragonstorm_creates_two_white_soldier_tokens() {
    let mut engine = main1_engine(363_006, &["teeming_dragonstorm"], &[]);
    move_ready_to_battlefield(&mut engine, 0, "teeming_dragonstorm");
    resolve_entire_stack_two_player(&mut engine);
    let soldiers = battlefield_token_oids(&engine, 0, "soldier_w_2_2");
    assert_eq!(soldiers.len(), 2, "CR 111.1: exactly two Soldier tokens");
    for oid in soldiers {
        let object = &engine.state.objects[&oid];
        assert_eq!((object.power, object.toughness), (Some(2), Some(2)));
        assert!(engine.state.players[0].battlefield.contains(&oid));
    }
}
