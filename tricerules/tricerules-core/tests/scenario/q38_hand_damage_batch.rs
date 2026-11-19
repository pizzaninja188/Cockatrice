//! Actual-card scenarios for the queue-v38 hand-size damage cohort.
//!
//! Expectations are printed Oracle values. CR 115.1 targets a player; CR 402.1/Z.7 define the
//! hand; CR 608.2h reads the targeted player's current hand count when the spell resolves;
//! CR 120.3 deals that much damage.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;

struct Case {
    id: &'static str,
    mana: ManaGift,
}

fn cases() -> [Case; 3] {
    [
        Case {
            id: "gaze_of_adamaro",
            mana: ManaGift {
                r: 2,
                c: 2,
                ..Default::default()
            },
        },
        Case {
            id: "storm_seeker",
            mana: ManaGift {
                g: 1,
                c: 3,
                ..Default::default()
            },
        },
        Case {
            id: "sudden_impact",
            mana: ManaGift {
                r: 1,
                c: 3,
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
    let decks = Some(vec![deck_with("mountain", &[]), deck_with("forest", &[])]);
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

fn set_hand(e: &mut GameEngine, player: usize, count: usize) {
    e.state.players[player].hand.clear();
    for _ in 0..count {
        inject_card_into_hand(e, player, "grizzly_bears");
    }
}

fn cast_at(e: &mut GameEngine, case: &Case, target: i32) {
    inject_card_into_hand(e, 0, case.id);
    give_mana(e, 0, gift(&case.mana));
    let slot = hand_index_for_card(e, 0, case.id);
    e.apply_scenario_command(0, &cast_spell(slot, target_player(target)))
        .unwrap_or_else(|error| panic!("cast {}: {error:?}", case.id));
    resolve_entire_stack_two_player(e);
}

#[test]
fn q38_hand_damage_canary_simplest() {
    let case = &cases()[0];
    let mut e = engine(38_020_101);
    set_hand(&mut e, 1, 3);
    cast_at(&mut e, case, 1);
    assert_eq!(
        e.state.players[1].life, 17,
        "three cards in the targeted hand"
    );
    assert_eq!(e.state.players[0].life, 20, "the caster is not damaged");
}

#[test]
fn q38_hand_damage_canary_hardest() {
    let case = &cases()[1];
    let mut e = engine(38_020_102);
    set_hand(&mut e, 1, 0);
    cast_at(&mut e, case, 1);
    assert_eq!(
        e.state.players[1].life, 20,
        "an empty hand deals zero damage"
    );
    assert!(e.state.stack.is_empty(), "the spell still resolves");
}

#[test]
fn q38_hand_damage_cards_use_only_the_targeted_players_hand_count() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(38_021_100 + index as u64);
        set_hand(&mut e, 0, 2);
        set_hand(&mut e, 1, 4);
        cast_at(&mut e, case, 1);
        assert_eq!(
            e.state.players[1].life, 16,
            "{}: exactly the four cards in the target's hand",
            case.id
        );
        assert_eq!(
            e.state.players[0].life, 20,
            "{}: the caster's hand does not count",
            case.id
        );
    }
}

#[test]
fn q38_hand_damage_cards_may_target_their_controller_and_reject_no_target() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(38_022_100 + index as u64);
        set_hand(&mut e, 0, 2);
        set_hand(&mut e, 1, 5);
        cast_at(&mut e, case, 0);
        assert_eq!(
            e.state.players[0].life, 18,
            "{}: the controller is a legal player target",
            case.id
        );
        assert_eq!(e.state.players[1].life, 20, "{}", case.id);

        let mut rejected = engine(38_023_100 + index as u64);
        inject_card_into_hand(&mut rejected, 0, case.id);
        give_mana(&mut rejected, 0, gift(&case.mana));
        let slot = hand_index_for_card(&rejected, 0, case.id);
        assert!(
            rejected
                .apply_scenario_command(0, &cast_spell(slot, vec![]))
                .is_err(),
            "{}: the printed player target is mandatory",
            case.id
        );
    }
}
