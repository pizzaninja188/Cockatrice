//! Actual-card scenarios for the queue-v83 03-etb-lose-life cohort.
//!
//! Every card loses printed life when it enters. Expectations are printed Oracle values. CR 603.6a
//! stages the entry trigger, 119.3 adjusts the life total, and 119.4 distinguishes the loss from a
//! cost payment. The pinned 2004-10-04 rulings confirm the loss is not damage and is not prevented.
//! Eviscerator's printed protection from white is CR 702.16.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_cards::primitives::ProtectionQuality;
use tricerules_cards::Color;

struct Case {
    id: &'static str,
    mana: ManaGift,
    body: (u32, u32),
    loss: u32,
    protection: bool,
}

fn cases() -> [Case; 3] {
    [
        Case {
            id: "serpent_warrior",
            mana: ManaGift {
                b: 1,
                c: 2,
                ..Default::default()
            },
            body: (3, 3),
            loss: 3,
            protection: false,
        },
        Case {
            id: "toxic_abomination",
            mana: ManaGift {
                b: 1,
                c: 1,
                ..Default::default()
            },
            body: (3, 2),
            loss: 2,
            protection: false,
        },
        Case {
            id: "eviscerator",
            mana: ManaGift {
                b: 2,
                c: 3,
                ..Default::default()
            },
            body: (5, 5),
            loss: 5,
            protection: true,
        },
    ]
}

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![
        deck_with("swamp", &["grizzly_bears", "doom_blade"]),
        deck_with("forest", &["grizzly_bears"]),
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

fn cast_card(e: &mut GameEngine, case: &Case) -> u32 {
    give_mana(e, 0, gift(&case.mana));
    inject_card_into_hand(e, 0, case.id);
    let slot = hand_index_for_card(e, 0, case.id);
    e.apply_scenario_command(0, &cast_spell(slot, vec![]))
        .unwrap_or_else(|error| panic!("{}: cast: {error:?}", case.id));
    resolve_entire_stack_two_player(e);
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
        "{}: exact cast",
        case.id
    );
    battlefield_object_for_card(e, 0, case.id)
}

#[test]
fn q83_lose_life_canary_simplest_toxic_abomination() {
    let case = &cases()[1];
    let mut e = engine(83_030_001);
    let life = e.state.players[0].life;
    let card = cast_card(&mut e, case);
    assert_eq!(e.state.players[0].life, life - 2, "printed two life");
    assert_eq!(
        (e.effective_power(card), e.effective_toughness(card)),
        (Some(3), Some(2))
    );
}

#[test]
fn q83_lose_life_canary_hardest_eviscerator() {
    let case = &cases()[2];
    let mut e = engine(83_031_001);
    let life = e.state.players[0].life;
    let card = cast_card(&mut e, case);
    assert_eq!(e.state.players[0].life, life - 5, "printed five life");
    assert_eq!(
        e.characteristics(card)
            .expect("characteristics")
            .protections,
        [ProtectionQuality::Color(Color::White)]
    );
}

#[test]
fn q83_lose_life_cards_enter_through_their_exact_cost_and_lose_printed_life() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(83_032_000 + index as u64);
        let life = e.state.players[0].life;
        let card = cast_card(&mut e, &case);
        assert_eq!(
            (e.effective_power(card), e.effective_toughness(card)),
            (Some(case.body.0), Some(case.body.1)),
            "{}: printed body",
            case.id
        );
        assert_eq!(
            e.state.players[0].life,
            life - case.loss as i32,
            "{}: printed life loss",
            case.id
        );
        assert_eq!(
            e.characteristics(card)
                .expect("characteristics")
                .protections,
            if case.protection {
                vec![ProtectionQuality::Color(Color::White)]
            } else {
                Vec::new()
            },
            "{}: printed protection",
            case.id
        );
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}

#[test]
fn q83_lose_life_triggers_on_any_battlefield_entry_not_only_a_cast() {
    // The printed ability is an enters-the-battlefield trigger (CR 603.6a), so a permanent put onto
    // the battlefield without being cast still loses the printed life.
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(83_033_000 + index as u64);
        let life = e.state.players[0].life;
        // The card must exist in a zone before the engine can move it; inject it into hand, then
        // enter the battlefield through the engine's own entry path rather than casting it.
        inject_card_into_hand(&mut e, 0, case.id);
        let card = move_ready_to_battlefield(&mut e, 0, case.id);
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.players[0].life,
            life - case.loss as i32,
            "{}: the entry trigger fires outside a cast",
            case.id
        );
        assert_eq!(
            (e.effective_power(card), e.effective_toughness(card)),
            (Some(case.body.0), Some(case.body.1)),
            "{}",
            case.id
        );
    }
}

#[test]
fn q83_lose_life_can_be_cast_below_the_lost_amount() {
    let case = &cases()[0];
    let mut e = engine(83_034_001);
    e.state.players[0].life = 2;
    give_mana(&mut e, 0, gift(&case.mana));
    inject_card_into_hand(&mut e, 0, case.id);
    let slot = hand_index_for_card(&e, 0, case.id);
    e.apply_scenario_command(0, &cast_spell(slot, vec![]))
        .expect("the spell is cast while its controller has 2 life");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.state.players[0].life, -1,
        "CR 119.3: the total goes below zero before the state-based loss check"
    );
    assert!(
        e.state.players[0].has_lost,
        "CR 704.5a: a player at 0 or less life loses the game"
    );
}
