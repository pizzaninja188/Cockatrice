//! Actual-card scenarios for the queue-v31 Treasure loot cohort.
//!
//! Expectations are printed Oracle values. CR 601.2b/601.2f make the discard an
//! additional cost paid with the mana cost; CR 701.16 discards; CR 121 draws;
//! CR 111/701.6 create two Treasure tokens; CR 608.2c follows printed order.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::{
    cast_cost_group_selection::SelectedObject, CastCostGroupSelection,
};

struct Case {
    id: &'static str,
    mana: ManaGift,
}

fn cases() -> [Case; 3] {
    [
        Case {
            id: "big_score",
            mana: ManaGift {
                r: 1,
                c: 3,
                ..Default::default()
            },
        },
        Case {
            id: "pirates_pillage",
            mana: ManaGift {
                r: 1,
                c: 3,
                ..Default::default()
            },
        },
        Case {
            id: "unexpected_windfall",
            mana: ManaGift {
                r: 2,
                c: 2,
                ..Default::default()
            },
        },
    ]
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

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![
        deck_with(
            "mountain",
            &["big_score", "pirates_pillage", "unexpected_windfall"],
        ),
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

fn discard_option(hand_index: usize) -> CastCostGroupSelection {
    CastCostGroupSelection {
        group_index: 0,
        option_index: 0,
        selected_object: Some(SelectedObject::HandIndex(hand_index as u32)),
        ..Default::default()
    }
}

fn cast_with_discard(e: &mut GameEngine, case: &Case) -> usize {
    ensure_card_in_hand(e, 0, case.id);
    inject_card_into_hand(e, 0, "grizzly_bears");
    give_mana(e, 0, gift(&case.mana));
    let slot = hand_index_for_card(e, 0, case.id);
    let fodder_slot = hand_index_for_card(e, 0, "grizzly_bears");
    let fodder = e.state.players[0].hand[fodder_slot];
    let hand_before = e.state.players[0].hand.len();
    e.apply_scenario_command(
        0,
        &cast_spell_with_cast_cost_groups(slot, vec![], vec![discard_option(fodder_slot)]),
    )
    .unwrap_or_else(|error| panic!("{}: cast: {error:?}", case.id));
    assert_eq!(
        e.state.objects[&fodder].zone,
        Zone::Graveyard,
        "{}: discard paid",
        case.id
    );
    resolve_entire_stack_two_player(e);
    assert!(e.state.stack.is_empty(), "{}", case.id);
    assert_eq!(
        e.state.players[0].hand.len(),
        hand_before - 2 + 2,
        "{}: discard one and draw two",
        case.id
    );
    assert_eq!(
        battlefield_token_oids(e, 0, "treasure").len(),
        2,
        "{}: two Treasures",
        case.id
    );
    0
}

#[test]
fn q31_treasure_loot_canary_simplest() {
    cast_with_discard(&mut engine(31_080_001), &cases()[0]);
}

#[test]
fn q31_treasure_loot_canary_hardest() {
    cast_with_discard(&mut engine(31_080_002), &cases()[2]);
}

#[test]
fn q31_treasure_loot_cards_draw_two_and_create_two_treasures() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(31_081_000 + index as u64);
        cast_with_discard(&mut e, case);
        let treasures = battlefield_token_oids(&e, 0, "treasure");
        assert_eq!(treasures.len(), 2, "{}", case.id);
        for treasure in treasures {
            let object = &e.state.objects[&treasure];
            assert_eq!(object.zone, Zone::Battlefield, "{}", case.id);
            assert!(object.is_token(), "{}", case.id);
        }
        assert_eq!(
            e.state.players[0].graveyard.len(),
            2,
            "{}: the spell and the discarded card",
            case.id
        );
    }
}

#[test]
fn q31_treasure_loot_cards_require_the_discard_cost() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(31_082_000 + index as u64);
        ensure_card_in_hand(&mut e, 0, case.id);
        give_mana(&mut e, 0, gift(&case.mana));
        let slot = hand_index_for_card(&e, 0, case.id);
        assert!(
            e.apply_scenario_command(0, &cast_spell(slot, vec![]))
                .is_err(),
            "{}: the additional discard cannot be skipped",
            case.id
        );
        assert_eq!(e.state.players[0].graveyard.len(), 0, "{}", case.id);
    }
}

#[test]
fn q31_treasure_loot_cards_consume_their_exact_mana() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(31_083_000 + index as u64);
        cast_with_discard(&mut e, case);
        let pool = &e.state.players[0].mana_pool;
        assert_eq!(
            (
                pool.white,
                pool.blue,
                pool.black,
                pool.red,
                pool.green,
                pool.colorless
            ),
            (0, 0, 0, 0, 0, 0),
            "{}",
            case.id
        );
    }
}
