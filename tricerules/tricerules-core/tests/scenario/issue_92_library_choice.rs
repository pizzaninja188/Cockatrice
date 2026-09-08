//! Issue #92: look at a bounded library window, optionally choose a matching card for hand, and
//! put the rest on the bottom in either a deterministic random order or a player-chosen order.

use crate::helpers::*;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::ChoiceKind;

fn green_deck_with(card: &str) -> Option<Vec<Vec<String>>> {
    Some(vec![
        deck_with("forest", &[card]),
        vec!["island".into(); 20],
    ])
}

/// Put `card_ids` on top, first entry first, without changing zones or generations.
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

fn green_mana() -> ManaGift {
    ManaGift {
        g: 1,
        ..Default::default()
    }
}

#[test]
fn issue_228_sleight_requires_one_private_card() {
    let mut e =
        GameEngine::new(22801, &[0, 1], 20, green_deck_with("sleight_of_hand"), true).unwrap();
    advance_to_main1_from_game_start(&mut e);
    ensure_in_hand(&mut e, 0, "sleight_of_hand");
    let top = seat_on_top(&mut e, 0, &["forest", "island", "mountain"]);
    let batch = cast_instant_and_resolve(
        &mut e,
        0,
        "sleight_of_hand",
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    let choice = find_resolution_choice(&batch).unwrap();
    assert_eq!((choice.min, choice.max), (1, 1));
    assert!(e
        .apply_command(0, &submit_resolution_choice(vec![]))
        .is_err());
    let batch = e
        .apply_command(0, &submit_resolution_choice(vec![top[0]]))
        .unwrap();
    assert!(!batch.events.iter().any(|event| matches!(
        &event.ev,
        Some(tricerules_proto::ruled::v1::ruled_event::Ev::CardsRevealed(
            _
        ))
    )));
    assert!(e.state.pending_resolution.is_none());
    assert!(e.state.players[0].hand.contains(&top[0]));
    assert_eq!(e.state.players[0].library.back(), Some(&top[1]));
    assert_eq!(e.state.players[0].library.front(), Some(&top[2]));
}

fn issue_228_engine(card: &str) -> GameEngine {
    let mut e = GameEngine::new(22802, &[0, 1], 20, green_deck_with(card), true).unwrap();
    advance_to_main1_from_game_start(&mut e);
    ensure_in_hand(&mut e, 0, card);
    e
}

fn issue_228_resolve(e: &mut GameEngine, card: &str) -> RuledEventBatch {
    cast_instant_and_resolve(
        e,
        0,
        card,
        ManaGift {
            u: 1,
            c: 1,
            ..Default::default()
        },
    )
}

fn assert_no_public_library_names(batch: &RuledEventBatch) {
    use tricerules_proto::ruled::v1::ruled_event::Ev;
    for event in &batch.events {
        match &event.ev {
            Some(Ev::CardsRevealed(_)) => panic!("selection is not a reveal"),
            Some(Ev::Log(log)) if log.visible_to_player_id.is_none() => {
                for name in ["Forest", "Island", "Mountain"] {
                    assert!(!log.text.contains(name), "public name leak: {}", log.text);
                }
            }
            _ => {}
        }
    }
}

#[test]
fn issue_228_flow_state_checks_each_graveyard_type_and_orders_the_remaining_cards() {
    for (instant, sorcery) in [(false, false), (true, false), (false, true), (true, true)] {
        let mut e = issue_228_engine("flow_state");
        if instant {
            inject_graveyard_card(&mut e, 0, "lightning_bolt");
        }
        if sorcery {
            inject_graveyard_card(&mut e, 0, "divination");
        }
        // The opponent's types never satisfy "your graveyard".
        inject_graveyard_card(&mut e, 1, "lightning_bolt");
        inject_graveyard_card(&mut e, 1, "divination");
        let top = seat_on_top(&mut e, 0, &["forest", "island", "mountain", "forest"]);
        let batch = issue_228_resolve(&mut e, "flow_state");
        let choice = find_resolution_choice(&batch).unwrap();
        let count = if instant && sorcery { 2 } else { 1 };
        assert_eq!((choice.min, choice.max), (count, count));
        assert_eq!(choice.candidate_object_ids, top[..3]);
        assert_eq!(choice.candidate_selectable, [true; 3]);
        assert_no_public_library_names(&batch);
        let selected = top[..count as usize].to_vec();
        let batch = e
            .apply_command(0, &submit_resolution_choice(selected.clone()))
            .unwrap();
        assert_no_public_library_names(&batch);
        let remainder: Vec<_> = top[count as usize..3].iter().rev().copied().collect();
        if remainder.len() > 1 {
            let order = find_resolution_choice(&batch).unwrap();
            assert!(order.ordered);
            assert_eq!((order.min, order.max), (2, 2));
            let batch = e
                .apply_command(0, &submit_resolution_choice(remainder.clone()))
                .unwrap();
            assert_no_public_library_names(&batch);
        }
        assert!(e.state.pending_resolution.is_none());
        assert!(e.state.stack.is_empty());
        for oid in selected {
            assert!(e.state.players[0].hand.contains(&oid));
        }
        let library: Vec<_> = e.state.players[0].library.iter().copied().collect();
        assert_eq!(library[0], top[3]);
        assert_eq!(&library[library.len() - remainder.len()..], remainder);
    }
}

#[test]
fn issue_228_short_libraries_do_as_much_as_possible_without_drawing() {
    for card in ["sleight_of_hand", "flow_state"] {
        for size in 0..=3 {
            let mut e = issue_228_engine(card);
            inject_graveyard_card(&mut e, 0, "lightning_bolt");
            inject_graveyard_card(&mut e, 0, "divination");
            let top = seat_on_top(&mut e, 0, &["forest", "island", "mountain"][..size]);
            // Remove surplus library objects consistently from this focused fixture.
            let surplus: Vec<_> = e.state.players[0]
                .library
                .iter()
                .copied()
                .filter(|oid| !top.contains(oid))
                .collect();
            for oid in surplus {
                e.state.objects.remove(&oid);
            }
            e.state.players[0].library.retain(|oid| top.contains(oid));
            let drawn_before = e.state.turn_history.current.player(0).cards_drawn;
            let batch = issue_228_resolve(&mut e, card);
            assert_no_public_library_names(&batch);
            let desired = if card == "flow_state" { 2 } else { 1 };
            if size == 0 {
                assert!(find_resolution_choice(&batch).is_none());
            } else {
                let choice = find_resolution_choice(&batch).unwrap();
                let required = desired.min(size) as u32;
                assert_eq!((choice.min, choice.max), (required, required));
                let batch = e
                    .apply_command(
                        0,
                        &submit_resolution_choice(top[..required as usize].to_vec()),
                    )
                    .unwrap();
                assert_no_public_library_names(&batch);
            }
            assert!(e.state.pending_resolution.is_none());
            assert!(e.state.stack.is_empty());
            assert_eq!(
                e.state.turn_history.current.player(0).cards_drawn,
                drawn_before
            );
            assert!(
                e.state.players.iter().all(|player| !player.has_lost),
                "empty library selection does not cause a loss"
            );
        }
    }
}

#[test]
fn issue_228_flow_state_condition_is_evaluated_at_resolution_and_then_locked() {
    let mut e = issue_228_engine("flow_state");
    let top = seat_on_top(&mut e, 0, &["forest", "island", "mountain"]);
    give_mana(
        &mut e,
        0,
        ManaGift {
            u: 1,
            c: 1,
            ..Default::default()
        },
    );
    let index = hand_index_for_card(&e, 0, "flow_state");
    e.apply_command(0, &cast_spell(index, vec![])).unwrap();
    inject_graveyard_card(&mut e, 0, "lightning_bolt");
    inject_graveyard_card(&mut e, 0, "divination");
    e.apply_command(0, &pass()).unwrap();
    let batch = e.apply_command(1, &pass()).unwrap();
    assert_eq!(find_resolution_choice(&batch).unwrap().min, 2);
    // Simulate changing information after the resolution instruction was already applied.
    e.state.players[0].graveyard.clear();
    e.apply_command(0, &submit_resolution_choice(top[..2].to_vec()))
        .unwrap();
    assert!(e.state.pending_resolution.is_none());
    assert!(top[..2]
        .iter()
        .all(|oid| e.state.players[0].hand.contains(oid)));
}

#[test]
fn issue_228_illegal_and_stale_choices_are_atomic_in_both_stages() {
    let mut e = issue_228_engine("flow_state");
    let top = seat_on_top(&mut e, 0, &["forest", "island", "mountain"]);
    issue_228_resolve(&mut e, "flow_state");
    let mut decline = submit_resolution_choice(vec![]);
    let Some(Cmd::SubmitResolutionChoice(answer)) = &mut decline.cmd else {
        unreachable!()
    };
    answer.decision = tricerules_proto::ruled::v1::ResolutionChoiceDecision::Decline as i32;
    let before = serde_json::to_string(&e.state).unwrap();
    assert!(e.apply_command(0, &decline).is_err());
    assert_eq!(serde_json::to_string(&e.state).unwrap(), before);
    for chosen in [vec![], vec![top[0], top[1]], vec![u32::MAX]] {
        let before = serde_json::to_string(&e.state).unwrap();
        assert!(e
            .apply_command(0, &submit_resolution_choice(chosen))
            .is_err());
        assert_eq!(serde_json::to_string(&e.state).unwrap(), before);
    }
    // Even an unselected card cannot change generation before the cohort is committed.
    *e.state.zone_change_generation.entry(top[2]).or_default() += 1;
    let before = serde_json::to_string(&e.state).unwrap();
    assert!(e
        .apply_command(0, &submit_resolution_choice(vec![top[0]]))
        .is_err());
    assert_eq!(serde_json::to_string(&e.state).unwrap(), before);
    *e.state.zone_change_generation.get_mut(&top[2]).unwrap() -= 1;
    e.apply_command(0, &submit_resolution_choice(vec![top[0]]))
        .unwrap();
    for chosen in [vec![top[1]], vec![top[1], top[1]], vec![top[0], top[2]]] {
        let before = serde_json::to_string(&e.state).unwrap();
        assert!(e
            .apply_command(0, &submit_resolution_choice(chosen))
            .is_err());
        assert_eq!(serde_json::to_string(&e.state).unwrap(), before);
    }
    *e.state.zone_change_generation.entry(top[2]).or_default() += 1;
    let before = serde_json::to_string(&e.state).unwrap();
    assert!(e
        .apply_command(0, &submit_resolution_choice(vec![top[1], top[2]]))
        .is_err());
    assert_eq!(serde_json::to_string(&e.state).unwrap(), before);
}

#[test]
fn issue_228_two_card_selection_rejects_duplicates_and_replays_deterministically() {
    fn play() -> serde_json::Value {
        let mut e = issue_228_engine("flow_state");
        inject_graveyard_card(&mut e, 0, "lightning_bolt");
        inject_graveyard_card(&mut e, 0, "divination");
        let top = seat_on_top(&mut e, 0, &["forest", "forest", "forest"]);
        issue_228_resolve(&mut e, "flow_state");
        let before = serde_json::to_value(&e.state).unwrap();
        assert!(e
            .apply_command(0, &submit_resolution_choice(vec![top[0], top[0]]))
            .is_err());
        assert_eq!(serde_json::to_value(&e.state).unwrap(), before);
        e.apply_command(0, &submit_resolution_choice(vec![top[2], top[0]]))
            .unwrap();
        assert!(e.state.players[0].hand.contains(&top[2]));
        assert!(e.state.players[0].hand.contains(&top[0]));
        assert_eq!(e.state.players[0].library.back(), Some(&top[1]));
        serde_json::to_value(&e.state).unwrap()
    }
    assert_eq!(play(), play());
}

#[test]
fn commune_uses_images_for_all_looked_cards_then_orders_the_remainder() {
    let mut e = GameEngine::new(
        9201,
        &[0, 1],
        20,
        green_deck_with("commune_with_nature"),
        true,
    )
    .expect("new");
    advance_to_main1_from_game_start(&mut e);
    ensure_in_hand(&mut e, 0, "commune_with_nature");
    let top = seat_on_top(
        &mut e,
        0,
        &[
            "grizzly_bears",
            "forest",
            "hill_giant",
            "island",
            "mountain",
            "storm_crow",
        ],
    );

    let batch = cast_instant_and_resolve(&mut e, 0, "commune_with_nature", green_mana());
    let choose = find_resolution_choice(&batch).expect("look choice");
    assert!(
        choose.public_reveal.is_none(),
        "looking does not reveal the candidate set"
    );
    assert_eq!(choose.choice_kind(), ChoiceKind::LibraryLook);
    assert_eq!(choose.candidate_object_ids, top[..5]);
    assert_eq!(
        choose.candidate_selectable,
        [true, false, true, false, false],
        "all five images are visible but only creature images are clickable"
    );
    assert_eq!((choose.min, choose.max), (0, 1));
    assert!(!choose.ordered, "the first step chooses at most one card");
    assert!(matches!(
        &e.state
            .pending_resolution
            .as_ref()
            .expect("library-look continuation")
            .continuation,
        ResolutionContinuation::LibraryLook {
            stage: PendingLibraryLookStage::ChooseToHand { .. },
            ..
        }
    ));

    let order_batch = e
        .apply_command(0, &submit_resolution_choice(vec![top[0]]))
        .expect("choose Grizzly Bears");
    let reveal = order_batch
        .events
        .iter()
        .find_map(|event| match &event.ev {
            Some(tricerules_proto::ruled::v1::ruled_event::Ev::CardsRevealed(reveal)) => {
                Some(reveal)
            }
            _ => None,
        })
        .expect("the chosen card is publicly revealed before moving to hand");
    assert_eq!(reveal.cards.len(), 1);
    assert_eq!(reveal.cards[0].object_id, top[0]);
    assert_eq!(reveal.cards[0].card_name, "Grizzly Bears");
    assert!(!reveal.reveal_id.is_empty());
    assert_eq!(e.state.objects[&top[0]].zone, Zone::Hand);
    assert!(e.state.players[0].hand.contains(&top[0]));

    let order = find_resolution_choice(&order_batch).expect("bottom ordering choice");
    assert!(
        order.public_reveal.is_none(),
        "unselected cards remain private"
    );
    assert_eq!(order.choice_kind(), ChoiceKind::LibraryLook);
    assert_eq!(order.candidate_object_ids, top[1..5]);
    assert_eq!(order.candidate_selectable, [true; 4]);
    assert_eq!((order.min, order.max), (4, 4));
    assert!(order.ordered);
    assert!(matches!(
        &e.state
            .pending_resolution
            .as_ref()
            .expect("bottom-order continuation")
            .continuation,
        ResolutionContinuation::LibraryLook {
            stage: PendingLibraryLookStage::OrderBottom,
            ..
        }
    ));

    let submitted = vec![top[4], top[3], top[2], top[1]];
    e.apply_command(0, &submit_resolution_choice(submitted.clone()))
        .expect("submit bottom order");
    assert!(e.state.pending_resolution.is_none());
    let library: Vec<u32> = e.state.players[0].library.iter().copied().collect();
    assert_eq!(&library[library.len() - submitted.len()..], submitted);
    assert_eq!(
        e.state.players[0].library.front().copied(),
        Some(top[5]),
        "the first unlooked card becomes the top card"
    );
}

#[test]
fn brightwood_rejects_noncreatures_and_random_order_is_replay_deterministic() {
    fn play() -> Vec<u32> {
        let mut e = GameEngine::new(
            9202,
            &[0, 1],
            20,
            green_deck_with("brightwood_tracker"),
            true,
        )
        .expect("new");
        advance_to_main1_from_game_start(&mut e);
        let tracker = relocate_to_battlefield(&mut e, 0, "brightwood_tracker", false);
        let top = seat_on_top(
            &mut e,
            0,
            &[
                "forest",
                "grizzly_bears",
                "island",
                "hill_giant",
                "storm_crow",
            ],
        );
        give_mana(
            &mut e,
            0,
            ManaGift {
                c: 5,
                g: 1,
                ..Default::default()
            },
        );

        e.apply_command(0, &activate_ability(tracker, 0, vec![]))
            .expect("activate Tracker");
        assert!(e.state.objects[&tracker].tapped, "the tap cost is paid");
        e.apply_command(0, &pass()).expect("controller pass");
        let batch = e.apply_command(1, &pass()).expect("opponent pass");
        let choose = find_resolution_choice(&batch).expect("look choice");
        assert_eq!(choose.candidate_object_ids, top[..4]);
        assert_eq!(choose.candidate_selectable, [false, true, false, true]);

        let before: Vec<u32> = e.state.players[0].library.iter().copied().collect();
        assert!(
            e.apply_command(0, &submit_resolution_choice(vec![top[0]]))
                .is_err(),
            "a noncreature image is not a legal submission"
        );
        assert!(e.state.pending_resolution.is_some());
        assert_eq!(
            e.state.players[0]
                .library
                .iter()
                .copied()
                .collect::<Vec<_>>(),
            before,
            "rejection preserves the pending choice and library"
        );

        e.apply_command(0, &submit_resolution_choice(vec![top[1]]))
            .expect("choose creature");
        assert!(e.state.pending_resolution.is_none());
        assert!(e.state.players[0].hand.contains(&top[1]));
        let library: Vec<u32> = e.state.players[0].library.iter().copied().collect();
        assert_eq!(library.first().copied(), Some(top[4]));
        assert_eq!(
            library[library.len() - 3..]
                .iter()
                .copied()
                .collect::<std::collections::BTreeSet<_>>(),
            [top[0], top[2], top[3]].into_iter().collect(),
            "only the unchosen looked-at cohort is randomized onto the bottom"
        );
        library
    }

    assert_eq!(
        play(),
        play(),
        "same seed and command log reproduce the random order"
    );
}
