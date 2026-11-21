//! Actual-card scenarios for the queue-v53 batch A cohorts.
//!
//! Expectations are printed Oracle values. CR 601.2c/115.1 govern targets, CR 611.2d/613.4b
//! govern base power/toughness, CR 120.3 governs damage, CR 121.1 governs drawing, and
//! CR 602.2 governs activated abilities.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::GameEngine;
use tricerules_proto::ruled::v1::TargetRef;

struct BaseCase {
    id: &'static str,
    mana: ManaGift,
    power: u32,
    toughness: u32,
}

fn base_cases() -> [BaseCase; 3] {
    [
        BaseCase {
            id: "diminish",
            mana: ManaGift {
                u: 1,
                ..Default::default()
            },
            power: 1,
            toughness: 1,
        },
        BaseCase {
            id: "square_up",
            mana: ManaGift {
                g: 1,
                c: 1,
                ..Default::default()
            },
            power: 4,
            toughness: 4,
        },
        BaseCase {
            id: "zhalfirin_shapecraft",
            mana: ManaGift {
                u: 1,
                c: 1,
                ..Default::default()
            },
            power: 4,
            toughness: 3,
        },
    ]
}

struct DamageCase {
    id: &'static str,
    mana: ManaGift,
    amount: u32,
}

fn damage_cases() -> [DamageCase; 3] {
    [
        DamageCase {
            id: "lava_spike",
            mana: ManaGift {
                r: 1,
                ..Default::default()
            },
            amount: 3,
        },
        DamageCase {
            id: "lava_axe",
            mana: ManaGift {
                r: 1,
                c: 4,
                ..Default::default()
            },
            amount: 5,
        },
        DamageCase {
            id: "sarkhans_catharsis",
            mana: ManaGift {
                r: 1,
                c: 4,
                ..Default::default()
            },
            amount: 5,
        },
    ]
}

struct ActivatedCase {
    id: &'static str,
    activation: ManaGift,
    amount: u32,
}

fn activated_cases() -> [ActivatedCase; 3] {
    [
        ActivatedCase {
            id: "scalding_devil",
            activation: ManaGift {
                r: 1,
                c: 2,
                ..Default::default()
            },
            amount: 1,
        },
        ActivatedCase {
            id: "volcanic_rambler",
            activation: ManaGift {
                r: 1,
                c: 2,
                ..Default::default()
            },
            amount: 1,
        },
        ActivatedCase {
            id: "flamewave_invoker",
            activation: ManaGift {
                r: 1,
                c: 7,
                ..Default::default()
            },
            amount: 5,
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

fn engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![
        deck_with("island", &[card]),
        deck_with("forest", &["grizzly_bears", "sol_ring"]),
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

fn pool(e: &GameEngine) -> (u32, u32, u32, u32, u32, u32) {
    let p = &e.state.players[0].mana_pool;
    (p.white, p.blue, p.black, p.red, p.green, p.colorless)
}

fn cast_at(e: &mut GameEngine, card: &str, mana: &ManaGift, targets: Vec<TargetRef>) {
    inject_card_into_hand(e, 0, card);
    give_mana(e, 0, gift(mana));
    let slot = hand_index_for_card(e, 0, card);
    e.apply_scenario_command(0, &cast_spell(slot, targets))
        .unwrap_or_else(|error| panic!("cast {card}: {error:?}"));
    resolve_entire_stack_two_player(e);
    assert_eq!(pool(e), (0, 0, 0, 0, 0, 0), "{card}: exact cost");
}

#[test]
fn q53_canary_simplest_diminish_sets_one_one() {
    let case = &base_cases()[0];
    let mut e = engine(53_030_001, case.id);
    let target = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    cast_at(&mut e, case.id, &case.mana, target_object(target));
    assert_eq!(
        (e.effective_power(target), e.effective_toughness(target)),
        (Some(1), Some(1))
    );
}

#[test]
fn q53_canary_hardest_zhalfirin_shapecraft_sets_four_three_and_draws() {
    let case = &base_cases()[2];
    let mut e = engine(53_030_002, case.id);
    let target = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    let hand_before = e.state.players[0].hand.len();
    cast_at(&mut e, case.id, &case.mana, target_object(target));
    assert_eq!(
        (e.effective_power(target), e.effective_toughness(target)),
        (Some(4), Some(3))
    );
    assert_eq!(
        e.state.players[0].hand.len(),
        hand_before + 1,
        "the spell draws one card"
    );
}

#[test]
fn q53_base_power_toughness_cards_apply_their_printed_values() {
    for (index, case) in base_cases().iter().enumerate() {
        let mut e = engine(53_031_000 + index as u64, case.id);
        let target = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        let hand_before = e.state.players[0].hand.len();
        cast_at(&mut e, case.id, &case.mana, target_object(target));
        assert_eq!(
            (e.effective_power(target), e.effective_toughness(target)),
            (Some(case.power), Some(case.toughness)),
            "{}: printed base P/T",
            case.id
        );
        assert_eq!(
            e.state.players[0].hand.len(),
            hand_before
                + if case.id == "zhalfirin_shapecraft" {
                    1
                } else {
                    0
                },
            "{}: only Shapecraft draws",
            case.id
        );
    }
}

#[test]
fn q53_player_damage_cards_deal_their_printed_damage() {
    for (index, case) in damage_cases().iter().enumerate() {
        let mut e = engine(53_032_000 + index as u64, case.id);
        cast_at(&mut e, case.id, &case.mana, target_player(1));
        assert_eq!(
            e.state.players[1].life,
            20 - case.amount as i32,
            "{}: printed damage",
            case.id
        );
        assert_eq!(e.state.players[0].life, 20, "{}: caster unchanged", case.id);
    }
}

#[test]
fn q53_player_damage_cards_reject_creature_targets() {
    for (index, case) in damage_cases().iter().enumerate() {
        let mut e = engine(53_033_000 + index as u64, case.id);
        let creature = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        inject_card_into_hand(&mut e, 0, case.id);
        give_mana(&mut e, 0, gift(&case.mana));
        let mana_before = pool(&e);
        let hand_before = e.state.players[0].hand.len();
        let slot = hand_index_for_card(&e, 0, case.id);
        assert!(
            e.apply_scenario_command(0, &cast_spell(slot, target_object(creature)))
                .is_err(),
            "{}: a creature is not a player or planeswalker target",
            case.id
        );
        assert_eq!(
            pool(&e),
            mana_before,
            "{}: rejected cast keeps mana",
            case.id
        );
        assert_eq!(
            e.state.players[0].hand.len(),
            hand_before,
            "{}: rejected cast keeps the card",
            case.id
        );
        assert_eq!(e.state.players[1].life, 20);
        assert_eq!(e.state.players[0].life, 20);
    }
}

#[test]
fn q53_activated_damage_cards_deal_their_printed_damage() {
    for (index, case) in activated_cases().iter().enumerate() {
        let mut e = engine(53_034_000 + index as u64, case.id);
        let source = inject_creature_on_battlefield(&mut e, 0, case.id);
        give_mana(&mut e, 0, gift(&case.activation));
        apply_ability(&mut e, 0, source, 0, target_player(1))
            .unwrap_or_else(|error| panic!("activate {}: {error:?}", case.id));
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(pool(&e), (0, 0, 0, 0, 0, 0), "{}: exact cost", case.id);
        assert_eq!(
            e.state.players[1].life,
            20 - case.amount as i32,
            "{}: printed damage",
            case.id
        );
        assert_eq!(e.state.players[0].life, 20, "{}: caster unchanged", case.id);
    }
}

#[test]
fn q53_activated_damage_rejects_creature_targets() {
    for (index, case) in activated_cases().iter().enumerate() {
        let mut e = engine(53_035_000 + index as u64, case.id);
        let source = inject_creature_on_battlefield(&mut e, 0, case.id);
        let creature = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        give_mana(&mut e, 0, gift(&case.activation));
        let mana_before = pool(&e);
        assert!(
            apply_ability(&mut e, 0, source, 0, target_object(creature)).is_err(),
            "{}: a creature is not a player or planeswalker target",
            case.id
        );
        assert_eq!(
            pool(&e),
            mana_before,
            "{}: rejected activation keeps mana",
            case.id
        );
        assert_eq!(
            e.state.objects[&source].zone,
            tricerules_core::Zone::Battlefield,
            "{}: source stays on the battlefield",
            case.id
        );
        assert!(!e.state.objects[&source].tapped);
        assert_eq!(e.state.players[1].life, 20);
        assert_eq!(e.state.players[0].life, 20);
    }
}

#[test]
fn q53_base_power_toughness_fizzles_a_departed_target() {
    let mut e = engine(53_036_001, "diminish");
    let target = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    inject_card_into_hand(&mut e, 0, "diminish");
    give_mana(
        &mut e,
        0,
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&e, 0, "diminish");
    e.apply_scenario_command(0, &cast_spell(slot, target_object(target)))
        .expect("cast Diminish");
    assert!(!e.state.stack.is_empty(), "Diminish is still on the stack");
    inject_card_into_hand(&mut e, 0, "doom_blade");
    give_mana(
        &mut e,
        0,
        ManaGift {
            b: 1,
            c: 1,
            ..Default::default()
        },
    );
    let doom = hand_index_for_card(&e, 0, "doom_blade");
    e.apply_scenario_command(0, &cast_spell(doom, target_object(target)))
        .expect("destroy the target in response");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.state.objects[&target].zone,
        tricerules_core::Zone::Graveyard
    );
    assert!(e.state.stack.is_empty());
}
