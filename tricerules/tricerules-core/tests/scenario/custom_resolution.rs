use crate::helpers::*;

#[test]
fn brainstorm_draws_three_then_returns_two_in_chosen_order() {
    let decks = Some(vec![
        {
            let mut d = vec!["brainstorm".to_string()];
            d.extend(std::iter::repeat_n("island".to_string(), 29));
            d
        },
        vec!["forest".into(); 30],
    ]);
    let mut e = GameEngine::new(42, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    ensure_in_hand(&mut e, 0, "brainstorm");

    let hand_before = e.state.players[0].hand.len();
    let lib_before = e.state.players[0].library.len();
    let batch = cast_instant_and_resolve(
        &mut e,
        0,
        "brainstorm",
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );

    // Parked on a choice; the spell drew three and asks for two back, in order.
    let req = find_resolution_choice(&batch).expect("resolution choice required");
    assert_eq!(req.deciding_player_id, 0);
    assert_eq!((req.min, req.max), (2, 2));
    assert!(req.ordered);
    assert_eq!(req.choice_kind(), ChoiceKind::HandCards);
    assert!(e.state.pending_resolution.is_some());
    // Cast removed brainstorm from hand; begin drew three.
    assert_eq!(e.state.players[0].hand.len(), hand_before - 1 + 3);
    assert_eq!(e.state.players[0].library.len(), lib_before - 3);

    // Put two specific hand cards on top: last chosen = top (intuitive "place A, then B on top").
    let chosen: Vec<u32> = e.state.players[0].hand.iter().take(2).copied().collect();
    let (first, second) = (chosen[0], chosen[1]);
    e.apply_command(0, &submit_resolution_choice(chosen))
        .expect("submit brainstorm choice");

    assert!(e.state.pending_resolution.is_none(), "resolution completed");
    assert_eq!(e.state.players[0].library[0], second, "last chosen on top");
    assert_eq!(e.state.players[0].library[1], first, "first chosen below");
    assert_eq!(e.state.players[0].hand.len(), hand_before - 1 + 3 - 2);
    assert_eq!(count_card_id_in_graveyard(&e, 0, "brainstorm"), 1);
}

/// Putting cards back on top of the library must emit **no** `PermanentMoved`. That event is
/// `FIELD_VISIBILITY_PUBLIC` down to its `card_id`, so announcing a hand → library move would
/// tell the opponent exactly which two cards Brainstorm hid on top — hand and library are hidden
/// zones (CR 400.2) and reach each player only through the redacted per-player zone view.
/// The silence is the feature; this test is what stops it being "fixed" into a leak.
#[test]
fn brainstorm_put_back_emits_no_public_move_event() {
    let decks = Some(vec![
        {
            let mut d = vec!["brainstorm".to_string()];
            d.extend(std::iter::repeat_n("island".to_string(), 29));
            d
        },
        vec!["forest".into(); 30],
    ]);
    let mut e = GameEngine::new(42, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    ensure_in_hand(&mut e, 0, "brainstorm");

    cast_instant_and_resolve(
        &mut e,
        0,
        "brainstorm",
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );

    let chosen: Vec<u32> = e.state.players[0].hand.iter().take(2).copied().collect();
    let card_ids: Vec<String> = chosen
        .iter()
        .map(|oid| e.state.objects[oid].card_id.clone())
        .collect();
    let batch = e
        .apply_command(0, &submit_resolution_choice(chosen.clone()))
        .expect("submit brainstorm choice");

    for moved in permanents_moved_in(&batch) {
        assert!(
            !chosen.contains(&moved.object_id),
            "put-back object {} was announced publicly (destination {:?})",
            moved.object_id,
            moved.destination()
        );
        assert!(
            !card_ids.contains(&moved.card_id),
            "put-back card '{}' was announced publicly (destination {:?})",
            moved.card_id,
            moved.destination()
        );
    }
}

#[test]
fn brainstorm_rejects_card_not_in_hand_without_mutating() {
    let decks = Some(vec![
        {
            let mut d = vec!["brainstorm".to_string()];
            d.extend(std::iter::repeat_n("island".to_string(), 29));
            d
        },
        vec!["forest".into(); 30],
    ]);
    let mut e = GameEngine::new(42, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    cast_instant_and_resolve(
        &mut e,
        0,
        "brainstorm",
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    assert!(e.state.pending_resolution.is_some());

    let hand_snapshot = e.state.players[0].hand.clone();
    let lib_snapshot: Vec<u32> = e.state.players[0].library.iter().copied().collect();

    // One legal hand card + one library card (not a legal candidate) -> rejected, no mutation.
    let legal = e.state.players[0].hand[0];
    let library_card = e.state.players[0].library[0];
    let res = e.apply_command(0, &submit_resolution_choice(vec![legal, library_card]));
    assert!(res.is_err(), "returning a card not in hand is illegal");

    assert!(e.state.pending_resolution.is_some(), "still parked");
    assert_eq!(e.state.players[0].hand, hand_snapshot, "hand unchanged");
    assert_eq!(
        e.state.players[0]
            .library
            .iter()
            .copied()
            .collect::<Vec<u32>>(),
        lib_snapshot,
        "library unchanged"
    );

    // Wrong count is also rejected.
    assert!(
        e.apply_command(0, &submit_resolution_choice(vec![legal]))
            .is_err(),
        "must choose exactly two"
    );
    // The opponent cannot answer the controller's choice.
    assert!(
        e.apply_command(
            1,
            &submit_resolution_choice(vec![legal, e.state.players[0].hand[1]])
        )
        .is_err(),
        "only the deciding player may submit"
    );
}

#[test]
fn brainstorm_resolution_is_deterministic() {
    let make = || {
        // Deck must stay large enough that Brainstorm's "draw three" never reaches an empty
        // library — otherwise the caster decks out (CR 104.3c) and the resolution is mooted.
        let decks = Some(vec![
            {
                let mut d = vec!["brainstorm".to_string()];
                d.extend(
                    ["island", "forest", "swamp", "mountain", "plains"]
                        .iter()
                        .cycle()
                        .take(29)
                        .map(|s| s.to_string()),
                );
                d
            },
            vec!["forest".into(); 30],
        ]);
        let mut e = GameEngine::new(1234, &[0, 1], 20, decks, true).expect("new");
        advance_to_main1_from_game_start(&mut e);
        cast_instant_and_resolve(
            &mut e,
            0,
            "brainstorm",
            ManaGift {
                u: 1,
                ..Default::default()
            },
        );
        // Same choice command (indices into hand) on both runs.
        let chosen: Vec<u32> = e.state.players[0].hand.iter().take(2).copied().collect();
        e.apply_command(0, &submit_resolution_choice(chosen))
            .unwrap();
        e.state.players[0]
            .library
            .iter()
            .map(|&o| e.state.objects[&o].card_id.clone())
            .collect::<Vec<String>>()
    };
    assert_eq!(make(), make(), "same seed + same choices => same library");
}

#[test]
fn gifts_ungiven_opponent_chooses_the_split() {
    use tricerules_core::Zone;
    // Minimal deck: inject specific distinct-name cards after setup so shuffle order
    // cannot affect which names are available in the library for the search.
    let decks = Some(vec![
        {
            let mut d = vec!["gifts_ungiven".to_string()];
            d.extend(std::iter::repeat_n("island".to_string(), 29));
            d
        },
        vec!["forest".into(); 30],
    ]);
    let mut e = GameEngine::new(7, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    // Inject four distinct-name cards so we always have exactly 4 to choose from.
    let lb_id = inject_library_card(&mut e, 0, "lightning_bolt");
    let div_id = inject_library_card(&mut e, 0, "divination");
    let ctsp_id = inject_library_card(&mut e, 0, "counterspell");
    let murd_id = inject_library_card(&mut e, 0, "murder");
    let found = vec![lb_id, div_id, ctsp_id, murd_id];

    let batch = cast_instant_and_resolve(
        &mut e,
        0,
        "gifts_ungiven",
        ManaGift {
            u: 1,
            c: 3,
            ..Default::default()
        },
    );

    // First interrupt: the controller searches their library (up to four). This is a *private*
    // library search (ChoiceKind::LibrarySearch), so the relay redacts the candidate library
    // cards from the opponent — the library must not leak. Only the chosen cards become public.
    let search = find_resolution_choice(&batch).expect("search choice");
    assert_eq!(search.deciding_player_id, 0);
    assert_eq!(
        search.choice_kind(),
        ChoiceKind::LibrarySearch,
        "private library search (not public-revealed)"
    );
    for &oid in &found {
        assert!(
            search.candidate_object_ids.contains(&oid),
            "injected card must be a search candidate"
        );
    }

    let batch2 = e
        .apply_command(0, &submit_resolution_choice(found.clone()))
        .expect("search submit");

    // Second interrupt: the OPPONENT chooses which two go to the controller's graveyard.
    let split = find_resolution_choice(&batch2).expect("opponent split choice");
    assert_eq!(split.deciding_player_id, 1, "opponent decides the split");
    assert_eq!((split.min, split.max), (2, 2));
    assert_eq!(
        split.candidate_object_ids, found,
        "split is over the revealed set"
    );

    let to_grave: Vec<u32> = found.iter().take(2).copied().collect();
    let to_hand: Vec<u32> = found.iter().skip(2).copied().collect();
    // The controller cannot make the opponent's choice.
    assert!(
        e.apply_command(0, &submit_resolution_choice(to_grave.clone()))
            .is_err(),
        "only the deciding (opponent) player may submit"
    );
    e.apply_command(1, &submit_resolution_choice(to_grave.clone()))
        .expect("opponent submits split");

    assert!(e.state.pending_resolution.is_none(), "resolution completed");
    for oid in &to_grave {
        assert_eq!(
            e.state.objects[oid].zone,
            Zone::Graveyard,
            "chosen -> graveyard"
        );
        assert!(e.state.players[0].graveyard.contains(oid));
    }
    for oid in &to_hand {
        assert_eq!(e.state.objects[oid].zone, Zone::Hand, "rest -> hand");
        assert!(e.state.players[0].hand.contains(oid));
    }
    assert_eq!(count_card_id_in_graveyard(&e, 0, "gifts_ungiven"), 1);
}

/// Gifts Ungiven: submitting two library cards with the same name must be rejected (Oracle:
/// "up to four cards with *different* names"). The engine enforces this at choice submission
/// via `PendingResolution::unique_names`; the pending state must be restored on failure.
#[test]
fn gifts_ungiven_rejects_same_name_in_search() {
    // Minimal deck; we inject the cards we need deterministically after setup so shuffle
    // order cannot affect which cards are in the library when the search fires.
    let decks = Some(vec![
        {
            let mut d = vec!["gifts_ungiven".to_string()];
            d.extend(std::iter::repeat_n("island".to_string(), 29));
            d
        },
        vec!["forest".into(); 30],
    ]);
    let mut e = GameEngine::new(8, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    // Inject two Lightning Bolts (same name, different objects) plus three distinct others so
    // the test has 2 duplicate-name candidates and ≥4 distinct-name candidates available.
    let bolt1_id = inject_library_card(&mut e, 0, "lightning_bolt");
    let bolt2_id = inject_library_card(&mut e, 0, "lightning_bolt");
    let div_id = inject_library_card(&mut e, 0, "divination");
    let ctsp_id = inject_library_card(&mut e, 0, "counterspell");
    let murd_id = inject_library_card(&mut e, 0, "murder");

    let batch = cast_instant_and_resolve(
        &mut e,
        0,
        "gifts_ungiven",
        ManaGift {
            u: 1,
            c: 3,
            ..Default::default()
        },
    );

    let search = find_resolution_choice(&batch).expect("search choice");
    assert_eq!(search.deciding_player_id, 0);

    // bolt1 and bolt2 are both valid candidates (they are in the library).
    assert!(
        search.candidate_object_ids.contains(&bolt1_id),
        "bolt1 must be in candidates"
    );
    assert!(
        search.candidate_object_ids.contains(&bolt2_id),
        "bolt2 must be in candidates"
    );

    // Illegal: two cards with the same name (both Lightning Bolts) plus one other.
    let bad_choice = vec![bolt1_id, bolt2_id, div_id];
    let err = e
        .apply_command(0, &submit_resolution_choice(bad_choice))
        .expect_err("same-name cards must be rejected");
    assert!(
        matches!(err, tricerules_core::EngineError::Illegal(_)),
        "expected Illegal, got {err:?}"
    );

    // Pending resolution must still be set (state rolled back) so the player can retry.
    assert!(
        e.state.pending_resolution.is_some(),
        "pending resolution must be restored after rejection"
    );

    // Happy path: submit 4 cards with all different names — should succeed.
    // bolt1 (Lightning Bolt) + divination + counterspell + murder
    let good_choice = vec![bolt1_id, div_id, ctsp_id, murd_id];
    e.apply_command(0, &submit_resolution_choice(good_choice))
        .expect("four different-name cards must be accepted");

    // Resolution now needs the opponent to choose which 2 go to graveyard.
    assert!(
        e.state.pending_resolution.is_some(),
        "opponent split still pending"
    );
}

/// Registry `custom_effect` key → the card ids claiming it, for the two directions below.
fn custom_effect_claims() -> std::collections::BTreeMap<&'static str, Vec<&'static str>> {
    let reg = tricerules_cards::CardRegistry::global();
    let mut claims: std::collections::BTreeMap<&str, Vec<&str>> = Default::default();
    for def in reg.definitions() {
        for face in def.faces_iter() {
            if let Some(key) = face.custom_effect.as_deref() {
                claims.entry(key).or_default().push(def.id.as_str());
            }
        }
    }
    claims
}

/// Every `custom_effect` key in the card registry must resolve to a registered `CardEffect`
/// (an unregistered key is uncastable card data), and no two cards may claim one key. The "one
/// resolution owner *per face*" rule is enforced in the cards crate; this is the core-side half:
/// the impl must exist, and it must belong to exactly one card.
///
/// Custom effects are 1:1 with card ids, like RON data cards — two cards wanting one algorithm
/// is the signal to widen a primitive, not to share an impl. Without this check, two cards
/// accidentally set to `custom_effect: "brainstorm"` would both silently resolve as Brainstorm.
/// Two faces of one card sharing a key fails too: the impl cannot tell faces apart either.
#[test]
fn every_custom_effect_key_has_exactly_one_card_and_one_impl() {
    for (key, claimants) in custom_effect_claims() {
        assert!(
            tricerules_core::custom::lookup(key).is_some(),
            "card '{}' has custom_effect '{key}' with no registered CardEffect",
            claimants[0]
        );
        assert_eq!(
            claimants.len(),
            1,
            "custom_effect '{key}' is claimed by {} cards ({}); each key belongs to exactly one \
             card. If these really share an algorithm, that algorithm is expressible as \
             (effect_kind, parameters) and belongs in a primitive.",
            claimants.len(),
            claimants.join(", ")
        );
        assert_eq!(
            claimants[0], key,
            "custom_effect '{key}' is claimed by card '{}'; the custom-effect file stem, RON \
             custom_effect, and claiming card id must all match",
            claimants[0]
        );
    }
}

/// The reverse direction: every registered impl is claimed by a card. A registered key is a file
/// stem under `src/custom/`, so an orphan means a typo'd filename or a deleted RON — neither of
/// which the forward check above can see, since it only walks keys the registry already has.
#[test]
fn every_registered_impl_is_claimed_by_a_card() {
    let claims = custom_effect_claims();
    for key in tricerules_core::custom::keys() {
        assert!(
            claims.contains_key(key),
            "custom effect `{key}` (from `src/custom/{key}.rs`) is claimed by no registry card. \
             The file stem must equal the card id whose RON sets custom_effect: \"{key}\"."
        );
    }
}

// ── Summoning sickness on re-entry ─────────────────────────────────────────────

#[test]
fn recast_bounced_creature_is_summoning_sick() {
    // CR 302.6: a creature that left and re-entered the battlefield has not been controlled
    // continuously since its controller's turn began, so it is summoning sick again. Regression:
    // bouncing a creature cleared `summoning_sick`, and entering the battlefield never re-asserted
    // it, so a bounced-and-recast creature could attack/tap the same turn. Entry now re-asserts it.
    use tricerules_core::Zone;
    let decks = Some(vec![
        {
            let mut d = vec!["grizzly_bears".to_string(), "unsummon".to_string()];
            d.extend(std::iter::repeat_n("forest".to_string(), 20));
            d.extend(std::iter::repeat_n("island".to_string(), 8));
            d
        },
        vec!["forest".into(); 30],
    ]);
    let mut e = GameEngine::new(4242, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    // A grizzly already established on the battlefield (no longer summoning sick).
    ensure_in_hand(&mut e, 0, "grizzly_bears");
    let grizzly = put_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    assert!(
        !e.state.objects[&grizzly].summoning_sick,
        "an established creature is not summoning sick"
    );

    // Bounce it back to hand with Unsummon ({U}).
    ensure_in_hand(&mut e, 0, "unsummon");
    give_mana(
        &mut e,
        0,
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    let unsummon_idx = hand_index_for_card(&e, 0, "unsummon");
    e.apply_command(
        0,
        &cast_spell(
            unsummon_idx,
            vec![TargetRef {
                object_id: grizzly,
                damage_amount: 0,
            }],
        ),
    )
    .expect("cast unsummon at own grizzly");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.state.objects[&grizzly].zone,
        Zone::Hand,
        "grizzly bounced to hand"
    );

    // Recast the same grizzly ({1}{G}). The object id is reused across the zone change.
    give_mana(
        &mut e,
        0,
        ManaGift {
            g: 1,
            c: 1,
            ..Default::default()
        },
    );
    let grizzly_idx = hand_index_for_card(&e, 0, "grizzly_bears");
    e.apply_command(0, &cast_spell(grizzly_idx, vec![]))
        .expect("recast grizzly");
    resolve_entire_stack_two_player(&mut e);

    assert_eq!(
        e.state.objects[&grizzly].zone,
        Zone::Battlefield,
        "grizzly recast onto the battlefield"
    );
    assert!(
        e.state.objects[&grizzly].summoning_sick,
        "the recast creature is summoning sick again (CR 302.6)"
    );
}
