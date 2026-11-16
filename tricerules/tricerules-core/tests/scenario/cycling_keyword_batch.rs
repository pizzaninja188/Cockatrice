//! Actual-card scenarios for the queue-v20 cycling keyword cohort.
//!
//! Expectations are printed Oracle values. CR 702.29a defines cycling as a
//! hand-zone activated ability paid with the printed mana and discarding the
//! card, followed by drawing one card.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::{
    ruled_command::Cmd, AbilitySourceZone, ActivateAbility, RuledCommand,
};

struct Case {
    id: &'static str,
    cost: ManaGift,
    partial: ManaGift,
}

fn cases() -> [Case; 3] {
    [
        Case {
            id: "desert_cerodon",
            cost: ManaGift {
                r: 1,
                ..Default::default()
            },
            partial: ManaGift::default(),
        },
        Case {
            id: "hundroog",
            cost: ManaGift {
                c: 3,
                ..Default::default()
            },
            partial: ManaGift {
                c: 2,
                ..Default::default()
            },
        },
        Case {
            id: "lurching_rotbeast",
            cost: ManaGift {
                b: 1,
                ..Default::default()
            },
            partial: ManaGift::default(),
        },
    ]
}

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("forest", &[]), deck_with("island", &[])]);
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

fn gift(m: &ManaGift) -> ManaGift {
    ManaGift {
        w: m.w,
        u: m.u,
        b: m.b,
        r: m.r,
        g: m.g,
        c: m.c,
    }
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
fn cycling_keyword_batch_cards_cycle_for_the_printed_cost_and_draw_one() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(10_200_000 + index as u64);
        let card = inject_card_into_hand(&mut e, 0, case.id);
        let drawn = put_on_top(&mut e, 0, "grizzly_bears");
        let hand_before = e.state.players[0].hand.len();
        let library_before = e.state.players[0].library.len();
        let opponent_hand_before = e.state.players[1].hand.clone();
        let opponent_library_before = e.state.players[1].library.clone();
        give_mana(&mut e, 0, gift(&case.cost));
        e.apply_scenario_command(0, &hand_ability(&e, card))
            .unwrap_or_else(|error| panic!("{}: cycle: {error:?}", case.id));
        assert_eq!(
            e.state.objects[&card].zone,
            Zone::Graveyard,
            "{}: cycling discards itself as a cost",
            case.id
        );
        assert_eq!(pool(&e, 0), (0, 0, 0, 0, 0, 0), "{}: exact cost", case.id);
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.objects[&drawn].zone,
            Zone::Hand,
            "{}: cycling draws exactly one card",
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
fn cycling_keyword_batch_cards_reject_cycling_without_the_printed_cost() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(10_201_000 + index as u64);
        let card = inject_card_into_hand(&mut e, 0, case.id);
        e.apply_scenario_command(0, &hand_ability(&e, card))
            .expect_err("cycling needs its printed mana");
        assert_eq!(e.state.objects[&card].zone, Zone::Hand, "{}", case.id);
        give_mana(&mut e, 0, gift(&case.partial));
        e.apply_scenario_command(0, &hand_ability(&e, card))
            .expect_err("one mana short is not enough");
        assert_eq!(e.state.objects[&card].zone, Zone::Hand, "{}", case.id);
        assert_eq!(
            pool(&e, 0),
            (
                case.partial.w,
                case.partial.u,
                case.partial.b,
                case.partial.r,
                case.partial.g,
                case.partial.c
            ),
            "{}: no mana is spent on a rejected activation",
            case.id
        );
    }
}

#[test]
fn cycling_keyword_batch_cards_reject_cycling_outside_the_hand() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(10_202_000 + index as u64);
        inject_card_into_hand(&mut e, 0, case.id);
        let card = move_ready_to_battlefield(&mut e, 0, case.id);
        let library_before = e.state.players[0].library.len();
        give_mana(&mut e, 0, gift(&case.cost));
        assert!(
            e.apply_scenario_command(0, &hand_ability(&e, card))
                .is_err(),
            "{}: cycling functions only from hand",
            case.id
        );
        assert_eq!(
            e.state.objects[&card].zone,
            Zone::Battlefield,
            "{}: the card stays on the battlefield",
            case.id
        );
        assert_eq!(
            e.state.players[0].library.len(),
            library_before,
            "{}: no library change",
            case.id
        );
    }
}
