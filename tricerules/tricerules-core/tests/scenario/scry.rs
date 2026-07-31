//! CR 701.18 scry — the `Scry` primitive and the two cards built on it, Opt and Preordain.
//!
//! Both cards are `[Scry, Draw]`: the scry suspends resolution for the player's decision and the
//! draw runs when it resumes, so these also exercise the effect-tail resume that `docs/issues.md`
//! #36 tracked (its own regression test lives in `spell_effects.rs`).

use crate::helpers::*;
use tricerules_proto::ruled::v1::ChoiceKind;

/// Put `card_ids` on top of `player`'s library, first entry on top, and return their object ids.
/// `inject_library_card` appends to the bottom, so the cards are re-seated at the front.
fn seat_on_top(e: &mut GameEngine, player: usize, card_ids: &[&str]) -> Vec<u32> {
    let oids: Vec<u32> = card_ids
        .iter()
        .map(|cid| inject_library_card(e, player, cid))
        .collect();
    e.state.players[player]
        .library
        .retain(|o| !oids.contains(o));
    for &oid in oids.iter().rev() {
        e.state.players[player].library.push_front(oid);
    }
    oids
}

fn island_deck_with(card: &str) -> Option<Vec<Vec<String>>> {
    Some(vec![
        deck_with("island", &[card]),
        vec!["forest".into(); 20],
    ])
}

fn blue_mana() -> ManaGift {
    ManaGift {
        u: 1,
        ..Default::default()
    }
}

// ---------------------------------------------------------------------------
// Opt — Scry 1, then draw a card.

#[test]
fn opt_bottoming_the_scried_card_draws_the_next_one() {
    let mut e = GameEngine::new(7001, &[0, 1], 20, island_deck_with("opt"), true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    ensure_in_hand(&mut e, 0, "opt");
    let top = seat_on_top(&mut e, 0, &["grizzly_bears", "storm_crow"]);
    let (scried, next) = (top[0], top[1]);
    let lib_before = e.state.players[0].library.len();

    let batch = cast_instant_and_resolve(&mut e, 0, "opt", blue_mana());

    let req = find_resolution_choice(&batch).expect("scry parks for a choice");
    assert_eq!(req.choice_kind(), ChoiceKind::LibraryTop);
    assert_eq!(req.deciding_player_id, 0);
    assert_eq!(
        (req.min, req.max),
        (0, 1),
        "scry 1 bottoms zero or one card"
    );
    assert!(!req.ordered, "the bottom pile needs no ordering for scry 1");
    assert_eq!(req.candidate_object_ids, vec![scried]);
    assert!(
        !e.state.players[0].hand.contains(&scried),
        "scrying does not move the card out of the library"
    );

    e.apply_command(0, &submit_resolution_choice(vec![scried]))
        .expect("bottom the scried card");

    assert!(e.state.pending_resolution.is_none(), "scry 1 has no step 2");
    assert_eq!(
        *e.state.players[0].library.back().expect("library"),
        scried,
        "the bottomed card is now last"
    );
    assert!(
        e.state.players[0].hand.contains(&next),
        "the draw takes the card that was second from the top"
    );
    assert_eq!(
        e.state.players[0].library.len(),
        lib_before - 1,
        "one card left the library, via the draw"
    );
}

#[test]
fn opt_keeping_the_scried_card_on_top_draws_it() {
    let mut e = GameEngine::new(7002, &[0, 1], 20, island_deck_with("opt"), true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    ensure_in_hand(&mut e, 0, "opt");
    let scried = seat_on_top(&mut e, 0, &["grizzly_bears"])[0];

    cast_instant_and_resolve(&mut e, 0, "opt", blue_mana());
    // Keeping everything on top is submitting nothing (min is 0).
    e.apply_command(0, &submit_resolution_choice(vec![]))
        .expect("keep the scried card on top");

    assert!(e.state.pending_resolution.is_none());
    assert!(
        e.state.players[0].hand.contains(&scried),
        "the card kept on top is the one drawn"
    );
}

// ---------------------------------------------------------------------------
// Preordain — Scry 2, then draw a card.

#[test]
fn preordain_bottoming_both_skips_the_ordering_step() {
    let mut e =
        GameEngine::new(7003, &[0, 1], 20, island_deck_with("preordain"), true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    ensure_in_hand(&mut e, 0, "preordain");
    let top = seat_on_top(&mut e, 0, &["grizzly_bears", "storm_crow", "hill_giant"]);
    let (a, b, third) = (top[0], top[1], top[2]);

    let batch = cast_instant_and_resolve(&mut e, 0, "preordain", blue_mana());
    let req = find_resolution_choice(&batch).expect("scry parks");
    assert_eq!((req.min, req.max), (0, 2));
    assert_eq!(req.candidate_object_ids, vec![a, b]);

    e.apply_command(0, &submit_resolution_choice(vec![b, a]))
        .expect("bottom both");

    assert!(
        e.state.pending_resolution.is_none(),
        "nothing stays on top, so there is no order to choose"
    );
    let library: Vec<u32> = e.state.players[0].library.iter().copied().collect();
    assert_eq!(
        &library[library.len() - 2..],
        &[b, a],
        "both sit at the bottom in the submitted order"
    );
    assert!(
        e.state.players[0].hand.contains(&third),
        "the draw comes from what was the third card"
    );
}

#[test]
fn preordain_keeping_both_asks_for_an_order_then_draws_the_new_top() {
    let mut e =
        GameEngine::new(7004, &[0, 1], 20, island_deck_with("preordain"), true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    ensure_in_hand(&mut e, 0, "preordain");
    let top = seat_on_top(&mut e, 0, &["grizzly_bears", "storm_crow"]);
    let (a, b) = (top[0], top[1]);

    cast_instant_and_resolve(&mut e, 0, "preordain", blue_mana());
    let order_batch = e
        .apply_command(0, &submit_resolution_choice(vec![]))
        .expect("keep both on top");

    // Both stayed on top, so CR 701.18a's "in any order" is a real decision: step 2.
    let req = find_resolution_choice(&order_batch).expect("ordering choice required");
    assert_eq!(req.choice_kind(), ChoiceKind::LibraryTop);
    assert_eq!((req.min, req.max), (2, 2), "every kept card must be placed");
    assert!(req.ordered, "the order submitted is significant");
    assert_eq!(req.candidate_object_ids, vec![a, b]);
    assert!(e.state.pending_resolution.is_some(), "still parked");
    assert!(
        e.state.players[0].hand.is_empty() || !e.state.players[0].hand.contains(&a),
        "the draw has not happened yet"
    );

    // Swap them. The submitted list is bottom-first, so the *last* entry ends up on top —
    // the same convention as Brainstorm's put-back.
    e.apply_command(0, &submit_resolution_choice(vec![b, a]))
        .expect("submit the order");

    assert!(e.state.pending_resolution.is_none());
    assert!(
        e.state.players[0].hand.contains(&a),
        "the last card submitted went on top, so it is the one drawn"
    );
    assert_eq!(
        e.state.players[0].library[0], b,
        "the other kept card sits under it, now the top of the library"
    );
}

#[test]
fn preordain_bottoming_one_of_two_skips_the_ordering_step() {
    let mut e =
        GameEngine::new(7005, &[0, 1], 20, island_deck_with("preordain"), true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    ensure_in_hand(&mut e, 0, "preordain");
    let top = seat_on_top(&mut e, 0, &["grizzly_bears", "storm_crow"]);
    let (a, b) = (top[0], top[1]);

    cast_instant_and_resolve(&mut e, 0, "preordain", blue_mana());
    e.apply_command(0, &submit_resolution_choice(vec![a]))
        .expect("bottom the first");

    assert!(
        e.state.pending_resolution.is_none(),
        "one card left on top has only one possible order"
    );
    assert_eq!(*e.state.players[0].library.back().expect("library"), a);
    assert!(
        e.state.players[0].hand.contains(&b),
        "the kept card is drawn"
    );
}

// ---------------------------------------------------------------------------
// Edge cases

#[test]
fn scry_with_an_empty_library_does_not_park_and_the_draw_still_runs() {
    let mut e = GameEngine::new(7006, &[0, 1], 20, island_deck_with("opt"), true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    ensure_in_hand(&mut e, 0, "opt");
    e.state.players[0].library.clear();

    give_mana(&mut e, 0, blue_mana());
    let idx = hand_index_for_card(&e, 0, "opt");
    e.apply_command(0, &cast_spell(idx, vec![])).expect("cast");
    e.apply_command(0, &pass()).expect("p0 pass");
    let batch = e.apply_command(1, &pass()).expect("p1 pass — resolves");

    assert!(
        find_resolution_choice(&batch).is_none(),
        "nothing to look at, so no choice is asked for"
    );
    assert!(e.state.pending_resolution.is_none());
    // The `Draw` after the scry still runs, and drawing from an empty library loses the game
    // (CR 104.3c) — the point being that the second effect was not skipped.
    assert!(
        e.state.players[0].has_lost,
        "the draw ran and decked the caster"
    );
}

#[test]
fn scry_rejects_illegal_submissions_without_mutating_the_library() {
    let mut e =
        GameEngine::new(7007, &[0, 1], 20, island_deck_with("preordain"), true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    ensure_in_hand(&mut e, 0, "preordain");
    let top = seat_on_top(&mut e, 0, &["grizzly_bears", "storm_crow", "hill_giant"]);
    let (a, b, third) = (top[0], top[1], top[2]);

    cast_instant_and_resolve(&mut e, 0, "preordain", blue_mana());
    let library_before: Vec<u32> = e.state.players[0].library.iter().copied().collect();

    for (label, player, choice) in [
        ("wrong player", 1, vec![a]),
        ("card below the scried window", 0, vec![third]),
        ("more cards than were looked at", 0, vec![a, b, third]),
        ("the same card twice", 0, vec![a, a]),
    ] {
        assert!(
            e.apply_command(player, &submit_resolution_choice(choice))
                .is_err(),
            "{label} must be rejected"
        );
        assert!(
            e.state.pending_resolution.is_some(),
            "{label}: the choice is still outstanding"
        );
        assert_eq!(
            e.state.players[0]
                .library
                .iter()
                .copied()
                .collect::<Vec<u32>>(),
            library_before,
            "{label}: the library is untouched"
        );
    }

    // A legal answer still goes through afterwards.
    e.apply_command(0, &submit_resolution_choice(vec![a, b]))
        .expect("valid submission after the rejections");
    assert!(e.state.pending_resolution.is_none());
}

#[test]
fn scry_is_deterministic_for_the_same_seed_and_choices() {
    fn play() -> Vec<u32> {
        let mut e =
            GameEngine::new(7008, &[0, 1], 20, island_deck_with("preordain"), true).expect("new");
        advance_to_main1_from_game_start(&mut e);
        ensure_in_hand(&mut e, 0, "preordain");
        let top = seat_on_top(&mut e, 0, &["grizzly_bears", "storm_crow"]);
        cast_instant_and_resolve(&mut e, 0, "preordain", blue_mana());
        e.apply_command(0, &submit_resolution_choice(vec![top[0]]))
            .expect("bottom one");
        e.state.players[0].library.iter().copied().collect()
    }
    assert_eq!(play(), play(), "same seed + same choices ⇒ same library");
}
