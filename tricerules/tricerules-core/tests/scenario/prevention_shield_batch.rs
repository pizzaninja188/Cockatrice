//! Actual-card scenarios for the queue-v23 tap prevention-shield cohort.
//!
//! Expectations are printed Oracle values. CR 614.1a/615 create a prevention
//! shield, CR 115.1 allows any target, CR 602.2 pays the tap cost, and CR 514.2
//! ends the shield during cleanup.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;

struct Case {
    id: &'static str,
    mana: ManaGift,
    body: (u32, u32),
    amount: u32,
}

fn cases() -> [Case; 8] {
    [
        Case {
            id: "sanctum_custodian",
            mana: ManaGift {
                w: 1,
                c: 2,
                ..Default::default()
            },
            body: (1, 2),
            amount: 2,
        },
        Case {
            id: "master_healer",
            mana: ManaGift {
                w: 1,
                c: 4,
                ..Default::default()
            },
            body: (1, 4),
            amount: 4,
        },
        Case {
            id: "samite_healer",
            mana: ManaGift {
                w: 1,
                c: 1,
                ..Default::default()
            },
            body: (1, 1),
            amount: 1,
        },
        Case {
            id: "orim,_samite_healer",
            mana: ManaGift {
                w: 2,
                c: 1,
                ..Default::default()
            },
            body: (1, 3),
            amount: 3,
        },
        Case {
            id: "clergy_en-vec",
            mana: ManaGift {
                w: 1,
                c: 1,
                ..Default::default()
            },
            body: (1, 1),
            amount: 1,
        },
        Case {
            id: "loxodon_anchorite",
            mana: ManaGift {
                w: 2,
                c: 2,
                ..Default::default()
            },
            body: (2, 3),
            amount: 2,
        },
        Case {
            id: "femeref_healer",
            mana: ManaGift {
                w: 1,
                c: 1,
                ..Default::default()
            },
            body: (1, 1),
            amount: 1,
        },
        Case {
            id: "kithkin_healer",
            mana: ManaGift {
                w: 1,
                c: 2,
                ..Default::default()
            },
            body: (2, 2),
            amount: 1,
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
        deck_with("plains", &["lightning_bolt"]),
        deck_with("mountain", &["grizzly_bears"]),
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

fn cast_source(e: &mut GameEngine, case: &Case) -> u32 {
    inject_card_into_hand(e, 0, case.id);
    give_mana(e, 0, gift(&case.mana));
    let slot = hand_index_for_card(e, 0, case.id);
    e.apply_scenario_command(0, &cast_spell(slot, vec![]))
        .unwrap_or_else(|error| panic!("cast {}: {error:?}", case.id));
    assert_eq!(
        pool(e),
        (0, 0, 0, 0, 0, 0),
        "{}: exact printed cost",
        case.id
    );
    resolve_entire_stack_two_player(e);
    battlefield_object_for_card(e, 0, case.id)
}

#[test]
fn prevention_shield_batch_canary_cards_shield_a_player_and_absorb_damage() {
    for case in cases()
        .into_iter()
        .filter(|case| matches!(case.id, "samite_healer" | "master_healer"))
    {
        let mut e = engine(12_900_000);
        let source = inject_creature_with_stats(&mut e, 0, case.id, case.body.0, case.body.1);
        let player = e.state.players[1].id as u32;
        assert!(
            apply_ability(&mut e, 0, source, 0, target_player(1)).is_ok(),
            "{}: activate on a player",
            case.id
        );
        assert!(e.state.objects[&source].tapped, "{}: tap cost", case.id);
        assert!(
            !e.state.stack.is_empty(),
            "{}: a prevention ability uses the stack",
            case.id
        );
        assert_eq!(
            e.state.remaining_damage_prevention(player),
            0,
            "{}: no shield before resolution",
            case.id
        );
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.remaining_damage_prevention(player),
            case.amount,
            "{}: shield survives resolution",
            case.id
        );
    }
}

#[test]
fn prevention_shield_batch_cards_shield_a_creature() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(12_910_000 + index as u64);
        let source = inject_creature_with_stats(&mut e, 0, case.id, case.body.0, case.body.1);
        let bear = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        apply_ability(&mut e, 0, source, 0, target_object(bear)).expect("activate on the bear");
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.remaining_damage_prevention(bear),
            case.amount,
            "{}",
            case.id
        );
        give_mana(
            &mut e,
            0,
            ManaGift {
                r: 1,
                ..Default::default()
            },
        );
        inject_card_into_hand(&mut e, 0, "shock");
        let slot = hand_index_for_card(&e, 0, "shock");
        e.apply_scenario_command(0, &cast_spell(slot, target_object(bear)))
            .expect("cast shock");
        resolve_entire_stack_two_player(&mut e);
        let expected_damage = 2u32.saturating_sub(case.amount);
        assert_eq!(
            e.state.objects[&bear].damage, expected_damage,
            "{}: shield absorbs {}",
            case.id, case.amount
        );
        assert_eq!(
            e.state.remaining_damage_prevention(bear),
            case.amount.saturating_sub(2),
            "{}: residual creature shield",
            case.id
        );
    }
}

#[test]
fn prevention_shield_batch_cards_absorb_exact_damage_on_a_player() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(12_920_000 + index as u64);
        let source = inject_creature_with_stats(&mut e, 0, case.id, case.body.0, case.body.1);
        apply_ability(&mut e, 0, source, 0, target_player(1)).expect("activate on the player");
        resolve_entire_stack_two_player(&mut e);
        let life = e.state.players[1].life;
        give_mana(
            &mut e,
            0,
            ManaGift {
                r: 1,
                ..Default::default()
            },
        );
        inject_card_into_hand(&mut e, 0, "lightning_bolt");
        let slot = hand_index_for_card(&e, 0, "lightning_bolt");
        e.apply_scenario_command(0, &cast_spell(slot, target_player(1)))
            .expect("cast bolt");
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.players[1].life,
            life - i32::try_from(3u32.saturating_sub(case.amount)).unwrap(),
            "{}: only unprevented damage",
            case.id
        );
        assert_eq!(
            e.state
                .remaining_damage_prevention(e.state.players[1].id as u32),
            case.amount.saturating_sub(3),
            "{}: residual shield",
            case.id
        );
    }
}

#[test]
fn prevention_shield_batch_cards_reject_a_second_activation_while_tapped() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(12_930_000 + index as u64);
        let source = inject_creature_with_stats(&mut e, 0, case.id, case.body.0, case.body.1);
        apply_ability(&mut e, 0, source, 0, target_player(1)).expect("first activation");
        resolve_entire_stack_two_player(&mut e);
        let shield = e
            .state
            .remaining_damage_prevention(e.state.players[1].id as u32);
        assert!(
            apply_ability(&mut e, 0, source, 0, target_player(1)).is_err(),
            "{}: a tapped creature cannot pay {{T}} again",
            case.id
        );
        assert_eq!(
            e.state
                .remaining_damage_prevention(e.state.players[1].id as u32),
            shield,
            "{}: rejected activation adds no shield",
            case.id
        );
        assert_eq!(pool(&e), (0, 0, 0, 0, 0, 0), "{}", case.id);
    }
}

#[test]
fn prevention_shield_batch_cards_require_a_target() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(12_940_000 + index as u64);
        let source = inject_creature_with_stats(&mut e, 0, case.id, case.body.0, case.body.1);
        assert!(
            apply_ability(&mut e, 0, source, 0, vec![]).is_err(),
            "{}: the shield needs a target",
            case.id
        );
        assert!(
            !e.state.objects[&source].tapped,
            "{}: rejected activation leaves it untapped",
            case.id
        );
        assert_eq!(
            e.state
                .remaining_damage_prevention(e.state.players[1].id as u32),
            0,
            "{}",
            case.id
        );
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}

#[test]
fn prevention_shield_batch_cards_expire_at_cleanup() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(12_950_000 + index as u64);
        let source = inject_creature_with_stats(&mut e, 0, case.id, case.body.0, case.body.1);
        apply_ability(&mut e, 0, source, 0, target_player(1)).expect("activate");
        resolve_entire_stack_two_player(&mut e);
        end_active_turn(&mut e, 0);
        assert_eq!(
            e.state
                .remaining_damage_prevention(e.state.players[1].id as u32),
            0,
            "{}: CR 514.2 ends the shield",
            case.id
        );
    }
}

#[test]
fn prevention_shield_batch_cards_apply_summoning_sickness_to_the_tap_cost() {
    for case in cases()
        .into_iter()
        .filter(|case| matches!(case.id, "samite_healer" | "orim,_samite_healer"))
    {
        let mut e = engine(12_960_000);
        let source = cast_source(&mut e, &case);
        assert!(e.state.objects[&source].summoning_sick, "{}", case.id);
        assert!(
            apply_ability(&mut e, 0, source, 0, target_player(1)).is_err(),
            "{}: a summoning-sick creature cannot tap",
            case.id
        );
        assert!(!e.state.objects[&source].tapped, "{}", case.id);
        assert_eq!(
            e.state
                .remaining_damage_prevention(e.state.players[1].id as u32),
            0,
            "{}",
            case.id
        );
    }
}
