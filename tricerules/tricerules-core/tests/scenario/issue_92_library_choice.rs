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
    assert_eq!(e.state.objects[&top[0]].zone, Zone::Hand);
    assert!(e.state.players[0].hand.contains(&top[0]));

    let order = find_resolution_choice(&order_batch).expect("bottom ordering choice");
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
