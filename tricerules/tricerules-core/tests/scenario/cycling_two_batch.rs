//! Actual-card scenarios for the reviewed Cycling {2} creature cohort.
//!
//! Expectations are printed Oracle values. CR 702.29a defines Cycling as a
//! hand-only activated ability paid with {2} and discarding the card, followed
//! by drawing one card.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::{
    ruled_command::Cmd, AbilitySourceZone, ActivateAbility, RuledCommand,
};

struct Case {
    id: &'static str,
}

fn cases() -> [Case; 5] {
    [
        Case {
            id: "barkhide_mauler",
        },
        Case { id: "ridge_rannet" },
        Case {
            id: "sandbar_merfolk",
        },
        Case {
            id: "sandbar_serpent",
        },
        Case {
            id: "yoked_plowbeast",
        },
    ]
}

fn engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![deck_with("forest", &[card]), deck_with("island", &[])]);
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

fn pool(e: &GameEngine, player: usize) -> (u32, u32, u32, u32, u32, u32) {
    let p = &e.state.players[player].mana_pool;
    (p.white, p.blue, p.black, p.red, p.green, p.colorless)
}

fn put_on_top(e: &mut GameEngine, player: usize, card: &str) -> u32 {
    let object = inject_library_card(e, player, card);
    e.state.players[player]
        .library
        .retain(|candidate| *candidate != object);
    e.state.players[player].library.push_front(object);
    object
}

fn hand_ability(e: &GameEngine, source: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ActivateAbility(ActivateAbility {
            source_object_id: source,
            source_zone: AbilitySourceZone::Hand as i32,
            expected_zone_change_generation: e
                .state
                .zone_change_generation
                .get(&source)
                .copied()
                .unwrap_or(0),
            ability_index: 0,
            ..Default::default()
        })),
    }
}

#[test]
fn cycling_two_batch_cards_cycle_for_two_and_draw_one() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_140_000 + index as u64, case.id);
        ensure_in_hand(&mut e, 0, case.id);
        let card = e.state.players[0].hand[hand_index_for_card(&e, 0, case.id)];
        let drawn = put_on_top(&mut e, 0, "grizzly_bears");
        let hand_before = e.state.players[0].hand.len();
        let library_before = e.state.players[0].library.len();
        let opponent_hand_before = e.state.players[1].hand.clone();
        let opponent_library_before = e.state.players[1].library.clone();
        give_mana(
            &mut e,
            0,
            ManaGift {
                c: 2,
                ..Default::default()
            },
        );
        e.apply_scenario_command(0, &hand_ability(&e, card))
            .expect("activate Cycling {2}");
        assert_eq!(
            e.state.objects[&card].zone,
            Zone::Graveyard,
            "{}: cycling discards itself as a cost",
            case.id
        );
        assert_eq!(pool(&e, 0), (0, 0, 0, 0, 0, 0), "{}", case.id);
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.objects[&drawn].zone,
            Zone::Hand,
            "{}: cycling draws exactly one card",
            case.id
        );
        assert!(
            e.state.players[0].hand.contains(&drawn),
            "{}: the drawn card is the only card added",
            case.id
        );
        assert_eq!(
            e.state.players[0].hand.len(),
            hand_before,
            "{}: discarding one and drawing one leaves the hand size unchanged",
            case.id
        );
        assert_eq!(
            e.state.players[0].library.len(),
            library_before - 1,
            "{}: exactly one card leaves the library",
            case.id
        );
        assert!(
            !e.state.players[0].hand.contains(&card),
            "{}: the discarded card is not also in hand",
            case.id
        );
        assert_eq!(
            e.state.players[1].hand, opponent_hand_before,
            "{}: the opponent's hand is unchanged",
            case.id
        );
        assert_eq!(
            e.state.players[1].library, opponent_library_before,
            "{}: the opponent's library is unchanged",
            case.id
        );
    }
}

#[test]
fn cycling_two_batch_cards_reject_cycling_without_the_printed_cost() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_141_000 + index as u64, case.id);
        ensure_in_hand(&mut e, 0, case.id);
        let card = e.state.players[0].hand[hand_index_for_card(&e, 0, case.id)];
        let command = hand_ability(&e, card);
        e.apply_scenario_command(0, &command)
            .expect_err("Cycling {2} needs two mana");
        assert_eq!(e.state.objects[&card].zone, Zone::Hand, "{}", case.id);
        give_mana(
            &mut e,
            0,
            ManaGift {
                c: 1,
                ..Default::default()
            },
        );
        e.apply_scenario_command(0, &command)
            .expect_err("one mana is not enough");
        assert_eq!(e.state.objects[&card].zone, Zone::Hand, "{}", case.id);
        assert_eq!(pool(&e, 0), (0, 0, 0, 0, 0, 1), "{}", case.id);
    }
}

#[test]
fn cycling_two_batch_cards_reject_cycling_outside_the_hand() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_142_000 + index as u64, case.id);
        let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
        give_mana(
            &mut e,
            0,
            ManaGift {
                c: 2,
                ..Default::default()
            },
        );
        e.apply_scenario_command(0, &hand_ability(&e, source))
            .expect_err("cycling functions only from the hand");
        assert_eq!(
            e.state.objects[&source].zone,
            Zone::Battlefield,
            "{}: rejected activation keeps the permanent",
            case.id
        );
        assert_eq!(pool(&e, 0), (0, 0, 0, 0, 0, 2), "{}", case.id);
    }
}
