//! Actual-card scenarios for the queue-v39 draw-then-discard cohort.
//!
//! Expectations are printed Oracle values. CR 121 draws first; CR 608.2c preserves printed order;
//! CR 701.9 makes the discard mandatory and private; CR 601.2 requires the printed mana.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;

struct Case {
    id: &'static str,
    mana: ManaGift,
    draw: usize,
    discard: usize,
}

fn cases() -> [Case; 3] {
    [
        Case {
            id: "prying_eyes",
            mana: ManaGift {
                u: 2,
                c: 4,
                ..Default::default()
            },
            draw: 4,
            discard: 2,
        },
        Case {
            id: "thoughtflare",
            mana: ManaGift {
                u: 1,
                r: 1,
                c: 3,
                ..Default::default()
            },
            draw: 4,
            discard: 2,
        },
        Case {
            id: "catalog",
            mana: ManaGift {
                u: 1,
                c: 2,
                ..Default::default()
            },
            draw: 2,
            discard: 1,
        },
    ]
}

fn engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![
        deck_with("island", &[card, "grizzly_bears"]),
        deck_with("forest", &[]),
    ]);
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("engine");
    advance_to_main1_from_game_start(&mut e);
    e
}

fn seat_on_top(e: &mut GameEngine, card: &str) -> u32 {
    let oid = inject_library_card(e, 0, card);
    e.state.players[0]
        .library
        .retain(|candidate| *candidate != oid);
    e.state.players[0].library.push_front(oid);
    oid
}

fn cast_named(e: &mut GameEngine, card: &Case) {
    ensure_in_hand(e, 0, card.id);
    give_mana(
        e,
        0,
        ManaGift {
            w: card.mana.w,
            u: card.mana.u,
            b: card.mana.b,
            r: card.mana.r,
            g: card.mana.g,
            c: card.mana.c,
        },
    );
    let slot = hand_index_for_card(e, 0, card.id);
    e.apply_scenario_command(0, &cast_spell(slot, vec![]))
        .unwrap_or_else(|error| panic!("cast {}: {error:?}", card.id));
    resolve_entire_stack_two_player(e);
}

#[test]
fn q39_draw_discard_canary_simplest() {
    let case = &cases()[2];
    let mut e = engine(39_040_001, case.id);
    ensure_in_hand(&mut e, 0, case.id);
    let hand_before = e.state.players[0].hand.len();
    let first = seat_on_top(&mut e, "forest");
    let second = seat_on_top(&mut e, "island");
    cast_named(&mut e, case);
    assert_eq!(e.state.players[0].hand.len(), hand_before + 1);
    let pending = e
        .state
        .pending_resolution
        .as_ref()
        .expect("discard is pending");
    assert!(pending.presentation.candidates.contains(&first));
    assert!(pending.presentation.candidates.contains(&second));
    e.apply_scenario_command(0, &submit_resolution_choice(vec![first]))
        .expect("discard a drawn card");
    assert_eq!(e.state.objects[&first].zone, Zone::Graveyard);
    assert_eq!(e.state.objects[&second].zone, Zone::Hand);
}

#[test]
fn q39_draw_discard_canary_hardest() {
    let case = &cases()[1];
    let mut e = engine(39_040_002, case.id);
    ensure_in_hand(&mut e, 0, case.id);
    let hand_before = e.state.players[0].hand.len();
    let drawn = [
        seat_on_top(&mut e, "forest"),
        seat_on_top(&mut e, "island"),
        seat_on_top(&mut e, "swamp"),
        seat_on_top(&mut e, "mountain"),
    ];
    cast_named(&mut e, case);
    assert_eq!(e.state.players[0].hand.len(), hand_before + 3);
    let pending = e
        .state
        .pending_resolution
        .as_ref()
        .expect("discard is pending");
    for oid in drawn {
        assert!(pending.presentation.candidates.contains(&oid));
    }
    e.apply_scenario_command(0, &submit_resolution_choice(vec![drawn[0], drawn[1]]))
        .expect("discard two drawn cards");
    assert_eq!(e.state.objects[&drawn[0]].zone, Zone::Graveyard);
    assert_eq!(e.state.objects[&drawn[1]].zone, Zone::Graveyard);
    assert_eq!(e.state.objects[&drawn[2]].zone, Zone::Hand);
    assert_eq!(e.state.objects[&drawn[3]].zone, Zone::Hand);
}

#[test]
fn q39_draw_discard_cards_draw_then_discard_their_printed_counts() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(39_041_000 + index as u64, case.id);
        ensure_in_hand(&mut e, 0, case.id);
        let hand_before = e.state.players[0].hand.len();
        let opponent_hand = e.state.players[1].hand.len();
        let opponent_library = e.state.players[1].library.len();
        let mut drawn = Vec::new();
        for _ in 0..case.draw {
            drawn.push(seat_on_top(&mut e, "forest"));
        }
        cast_named(&mut e, case);
        assert_eq!(
            e.state.players[0].hand.len(),
            hand_before - 1 + case.draw,
            "{}: draw count",
            case.id
        );
        assert_eq!(
            e.state.players[1].hand.len(),
            opponent_hand,
            "{}: opponent hand is unchanged",
            case.id
        );
        assert_eq!(
            e.state.players[1].library.len(),
            opponent_library,
            "{}: opponent library is unchanged",
            case.id
        );
        let pending = e.state.pending_resolution.as_ref().expect("discard choice");
        for oid in &drawn {
            assert!(
                pending.presentation.candidates.contains(oid),
                "{}: candidate",
                case.id
            );
        }
        e.apply_scenario_command(0, &submit_resolution_choice(drawn[..case.discard].to_vec()))
            .expect("discard the printed count");
        assert_eq!(
            e.state.players[0].hand.len(),
            hand_before - 1 + case.draw - case.discard,
            "{}: final hand count",
            case.id
        );
        for oid in &drawn[..case.discard] {
            assert_eq!(e.state.objects[oid].zone, Zone::Graveyard, "{}", case.id);
        }
        for oid in &drawn[case.discard..] {
            assert_eq!(e.state.objects[oid].zone, Zone::Hand, "{}", case.id);
        }
    }
}

#[test]
fn q39_draw_discard_cards_require_their_printed_mana() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(39_042_000 + index as u64, case.id);
        ensure_in_hand(&mut e, 0, case.id);
        let hand_before = e.state.players[0].hand.len();
        let library_before = e.state.players[0].library.len();
        let slot = hand_index_for_card(&e, 0, case.id);
        assert!(
            e.apply_scenario_command(0, &cast_spell(slot, vec![]))
                .is_err(),
            "{}: cast without mana is rejected",
            case.id
        );
        assert_eq!(e.state.players[0].hand.len(), hand_before);
        assert_eq!(e.state.players[0].library.len(), library_before);
        assert!(e.state.stack.is_empty());
    }
}
