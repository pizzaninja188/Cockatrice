//! Actual-card scenarios for the queue-v32 activated life-gain cohort.
//!
//! Expectations are printed Oracle values. CR 602.1/602.2 activate the ability;
//! CR 118.3 requires the printed tap and mana costs; CR 119.3/119.4 change only
//! the activating player's life total.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;

struct Case {
    id: &'static str,
    mana_cost: ManaGift,
    activation: ManaGift,
    amount: u32,
}

fn cases() -> [Case; 4] {
    [
        Case {
            id: "fountain_of_youth",
            mana_cost: ManaGift::default(),
            activation: ManaGift {
                c: 2,
                ..Default::default()
            },
            amount: 1,
        },
        Case {
            id: "stone_haven_medic",
            mana_cost: ManaGift {
                w: 1,
                c: 1,
                ..Default::default()
            },
            activation: ManaGift {
                w: 1,
                ..Default::default()
            },
            amount: 1,
        },
        Case {
            id: "tanglebloom",
            mana_cost: ManaGift {
                c: 1,
                ..Default::default()
            },
            activation: ManaGift {
                c: 1,
                ..Default::default()
            },
            amount: 1,
        },
        Case {
            id: "tower_of_eons",
            mana_cost: ManaGift {
                c: 4,
                ..Default::default()
            },
            activation: ManaGift {
                c: 8,
                ..Default::default()
            },
            amount: 10,
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

fn expected(case: &Case) -> (i32, i32) {
    (20 + case.amount as i32, 20)
}

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![
        deck_with(
            "forest",
            &[
                "fountain_of_youth",
                "stone_haven_medic",
                "tanglebloom",
                "tower_of_eons",
            ],
        ),
        deck_with("island", &[]),
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

fn cast_card(e: &mut GameEngine, case: &Case) -> u32 {
    give_mana(e, 0, gift(&case.mana_cost));
    inject_card_into_hand(e, 0, case.id);
    let slot = hand_index_for_card(e, 0, case.id);
    e.apply_scenario_command(0, &cast_spell(slot, vec![]))
        .unwrap_or_else(|error| panic!("cast {}: {error:?}", case.id));
    resolve_entire_stack_two_player(e);
    let source = battlefield_object_for_card(e, 0, case.id);
    e.state.objects.get_mut(&source).unwrap().summoning_sick = false;
    source
}

#[test]
fn q32_activated_lifegain_canary_simplest() {
    let case = &cases()[0];
    let mut e = engine(32_070_001);
    let source = cast_card(&mut e, case);
    give_mana(&mut e, 0, gift(&case.activation));
    apply_ability(&mut e, 0, source, 0, vec![]).expect("activate the life ability");
    assert!(e.state.objects[&source].tapped);
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        (e.state.players[0].life, e.state.players[1].life),
        expected(case)
    );
    assert!(e.state.stack.is_empty());
}

#[test]
fn q32_activated_lifegain_canary_hardest() {
    let case = &cases()[3];
    let mut e = engine(32_070_002);
    let source = cast_card(&mut e, case);
    give_mana(&mut e, 0, gift(&case.activation));
    apply_ability(&mut e, 0, source, 0, vec![]).expect("activate for ten");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        (e.state.players[0].life, e.state.players[1].life),
        expected(case)
    );
}

#[test]
fn q32_activated_lifegain_cards_gain_only_the_controllers_printed_life() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(32_071_000 + index as u64);
        let source = cast_card(&mut e, case);
        give_mana(&mut e, 0, gift(&case.activation));
        apply_ability(&mut e, 0, source, 0, vec![]).expect("activate");
        assert!(
            apply_ability(&mut e, 0, source, 0, vec![]).is_err(),
            "{}: the tap cost prevents a second activation",
            case.id
        );
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            (e.state.players[0].life, e.state.players[1].life),
            expected(case),
            "{}",
            case.id
        );
    }
}

#[test]
fn q32_activated_lifegain_cards_require_the_exact_costs_and_reject_a_foreign_controller() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(32_072_000 + index as u64);
        let source = move_ready_to_battlefield(&mut e, 0, case.id);
        assert!(
            apply_ability(&mut e, 0, source, 0, vec![]).is_err(),
            "{}: the printed mana cost is required",
            case.id
        );
        assert!(!e.state.objects[&source].tapped, "{}", case.id);
        assert_eq!(e.state.players[0].life, 20, "{}", case.id);
        let mut funded = engine(32_073_000 + index as u64);
        let funded_source = move_ready_to_battlefield(&mut funded, 0, case.id);
        give_mana(&mut funded, 0, gift(&case.activation));
        funded
            .apply_scenario_command(0, &pass())
            .expect("controller passes");
        assert!(
            apply_ability(&mut funded, 1, funded_source, 0, vec![]).is_err(),
            "{}: another player cannot activate it",
            case.id
        );
        assert!(!funded.state.objects[&funded_source].tapped, "{}", case.id);
    }
}
