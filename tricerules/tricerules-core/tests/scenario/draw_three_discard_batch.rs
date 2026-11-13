//! Actual-card scenarios for the reviewed "Draw three cards, then discard a
//! card" family.
//!
//! Expectations are printed Oracle values. CR 121 resolves the draw first and
//! CR 701.9 then applies the mandatory private discard. A cast without the
//! printed mana is rejected and spends no resources.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;

struct Case {
    id: &'static str,
    mana: ManaGift,
}

fn cases() -> [Case; 3] {
    [
        Case {
            id: "enhanced_awareness",
            mana: ManaGift {
                u: 1,
                c: 4,
                ..Default::default()
            },
        },
        Case {
            id: "rain_of_revelation",
            mana: ManaGift {
                u: 1,
                c: 3,
                ..Default::default()
            },
        },
        Case {
            id: "sift",
            mana: ManaGift {
                u: 1,
                c: 3,
                ..Default::default()
            },
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

fn cast_named(e: &mut GameEngine, card_id: &str, mana: ManaGift) {
    ensure_in_hand(e, 0, card_id);
    give_mana(e, 0, mana);
    let slot = hand_index_for_card(e, 0, card_id);
    e.apply_scenario_command(0, &cast_spell(slot, vec![]))
        .unwrap_or_else(|error| panic!("cast {card_id}: {error}"));
    resolve_entire_stack_two_player(e);
}

#[test]
fn draw_three_discard_batch_cards_draw_three_then_discard_one() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_120_000 + index as u64, case.id);
        ensure_in_hand(&mut e, 0, case.id);
        let hand_before = e.state.players[0].hand.len();
        let drawn = [
            seat_on_top(&mut e, "forest"),
            seat_on_top(&mut e, "island"),
            seat_on_top(&mut e, "swamp"),
        ];
        cast_named(&mut e, case.id, case.mana);
        assert_eq!(
            e.state.players[0].hand.len(),
            hand_before + 2,
            "{}: the spell left hand and three cards were drawn",
            case.id
        );
        let pending = e
            .state
            .pending_resolution
            .as_ref()
            .expect("the mandatory discard is pending");
        for oid in drawn {
            assert!(
                pending.presentation.candidates.contains(&oid),
                "{}: each drawn card is a legal discard choice",
                case.id
            );
        }
        let discarded = drawn[0];
        e.apply_scenario_command(0, &submit_resolution_choice(vec![discarded]))
            .expect("discard a drawn card");
        assert_eq!(
            e.state.objects[&discarded].zone,
            Zone::Graveyard,
            "{}: the discarded card goes to its owner's graveyard",
            case.id
        );
        for oid in [drawn[1], drawn[2]] {
            assert_eq!(
                e.state.objects[&oid].zone,
                Zone::Hand,
                "{}: the other drawn cards stay in hand",
                case.id
            );
        }
        assert!(e.state.pending_resolution.is_none(), "{}", case.id);
    }
}

#[test]
fn draw_three_discard_batch_cards_discard_from_hand_to_graveyard() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_121_000 + index as u64, case.id);
        ensure_in_hand(&mut e, 0, case.id);
        let preexisting = inject_card_into_hand(&mut e, 0, "grizzly_bears");
        let hand_before = e.state.players[0].hand.len();
        seat_on_top(&mut e, "forest");
        seat_on_top(&mut e, "island");
        seat_on_top(&mut e, "swamp");
        cast_named(&mut e, case.id, case.mana);
        e.apply_scenario_command(0, &submit_resolution_choice(vec![preexisting]))
            .expect("discard a pre-existing hand card");
        assert_eq!(
            e.state.objects[&preexisting].zone,
            Zone::Graveyard,
            "{}: any card in hand may be discarded",
            case.id
        );
        assert_eq!(
            e.state.players[0].hand.len(),
            hand_before + 1,
            "{}: the spell left hand, three were drawn, and one was discarded",
            case.id
        );
    }
}

#[test]
fn draw_three_discard_batch_cards_reject_cast_without_mana() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_122_000 + index as u64, case.id);
        ensure_in_hand(&mut e, 0, case.id);
        let hand_before = e.state.players[0].hand.len();
        let library_before = e.state.players[0].library.len();
        let slot = hand_index_for_card(&e, 0, case.id);
        assert!(
            e.apply_scenario_command(0, &cast_spell(slot, vec![]))
                .is_err(),
            "{}: the cast is rejected without mana",
            case.id
        );
        assert_eq!(
            e.state.players[0].hand.len(),
            hand_before,
            "{}: the rejected spell stays in hand",
            case.id
        );
        assert_eq!(
            e.state.players[0].library.len(),
            library_before,
            "{}: no draw happened",
            case.id
        );
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}
