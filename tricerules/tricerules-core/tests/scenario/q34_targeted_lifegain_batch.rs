//! Actual-card scenarios for the queue-v34 "target player gains N life" cohort.
//!
//! Expectations are printed Oracle values. CR 119.3 adds the life; CR 115.1
//! makes the chosen player the only target. The effect is not a creature pump
//! and cannot target a permanent.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;

struct Case {
    id: &'static str,
    mana: ManaGift,
    amount: i32,
    sorcery: bool,
}

fn cases() -> [Case; 3] {
    [
        Case {
            id: "heroes_reunion",
            mana: ManaGift {
                g: 1,
                w: 1,
                ..Default::default()
            },
            amount: 7,
            sorcery: false,
        },
        Case {
            id: "natural_spring",
            mana: ManaGift {
                g: 2,
                c: 3,
                ..Default::default()
            },
            amount: 8,
            sorcery: true,
        },
        Case {
            id: "soothing_balm",
            mana: ManaGift {
                w: 1,
                c: 1,
                ..Default::default()
            },
            amount: 5,
            sorcery: false,
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

fn cast_life(e: &mut GameEngine, case: &Case, player: i32) {
    inject_card_into_hand(e, 0, case.id);
    give_mana(e, 0, gift(&case.mana));
    let slot = hand_index_for_card(e, 0, case.id);
    e.apply_scenario_command(0, &cast_spell(slot, target_player(player)))
        .unwrap_or_else(|error| panic!("cast {}: {error:?}", case.id));
    resolve_entire_stack_two_player(e);
}

#[test]
fn q34_targeted_lifegain_canary_simplest() {
    let case = &cases()[2];
    let mut e = engine(34_030_001);
    cast_life(&mut e, case, 1);
    assert_eq!(
        (e.state.players[0].life, e.state.players[1].life),
        (20, 25),
        "Soothing Balm gives the chosen opponent 5 life"
    );
}

#[test]
fn q34_targeted_lifegain_canary_hardest() {
    let case = &cases()[1];
    let mut e = engine(34_030_002);
    cast_life(&mut e, case, 0);
    assert_eq!(
        (e.state.players[0].life, e.state.players[1].life),
        (28, 20),
        "Natural Spring gives its controller 8 life"
    );
}

#[test]
fn q34_targeted_lifegain_cards_give_exactly_the_printed_life_to_the_chosen_player() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(34_031_000 + index as u64);
        cast_life(&mut e, case, 0);
        assert_eq!(
            (e.state.players[0].life, e.state.players[1].life),
            (20 + case.amount, 20),
            "{}: own life",
            case.id
        );
        cast_life(&mut e, case, 1);
        assert_eq!(
            (e.state.players[0].life, e.state.players[1].life),
            (20 + case.amount, 20 + case.amount),
            "{}: any player is a legal target",
            case.id
        );
        assert!(case.sorcery == case.id.contains("natural"), "{}", case.id);
    }
}

#[test]
fn q34_targeted_lifegain_cards_reject_nonplayer_targets() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(34_032_000 + index as u64);
        let creature = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        inject_card_into_hand(&mut e, 0, case.id);
        give_mana(&mut e, 0, gift(&case.mana));
        let slot = hand_index_for_card(&e, 0, case.id);
        assert!(
            e.apply_scenario_command(0, &cast_spell(slot, target_object(creature)))
                .is_err(),
            "{}: a creature is not a player target",
            case.id
        );
        assert_eq!(
            (e.state.players[0].life, e.state.players[1].life),
            (20, 20),
            "{}: the rejected cast changes no life",
            case.id
        );
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}
