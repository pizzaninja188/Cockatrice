//! Actual-card scenarios for the reviewed shrink-target spell family.
//!
//! Expectations are printed Oracle values. CR 613.4c applies the negative P/T modification in
//! layer 7c; CR 514.2 ends it during cleanup; CR 608.2b fizzles a spell whose only target left;
//! and CR 107.4f/119.4 govern Dismember's Phyrexian payment. CR 510.1a floors negative power at
//! zero for combat damage (Disorient's saved ruling).
use super::helpers::*;
use tricerules_core::Zone;

struct Case {
    id: &'static str,
    mana: ManaGift,
    power: i32,
    toughness: i32,
}

fn cases() -> Vec<Case> {
    vec![
        Case {
            id: "demons_grasp",
            mana: ManaGift {
                b: 1,
                c: 4,
                ..Default::default()
            },
            power: -5,
            toughness: -5,
        },
        Case {
            id: "disfigure",
            mana: ManaGift {
                b: 1,
                ..Default::default()
            },
            power: -2,
            toughness: -2,
        },
        Case {
            id: "dismember",
            mana: ManaGift {
                b: 2,
                c: 1,
                ..Default::default()
            },
            power: -5,
            toughness: -5,
        },
        Case {
            id: "disorient",
            mana: ManaGift {
                u: 1,
                c: 3,
                ..Default::default()
            },
            power: -7,
            toughness: -0,
        },
        Case {
            id: "eyes_of_the_beholder",
            mana: ManaGift {
                b: 2,
                c: 4,
                ..Default::default()
            },
            power: -11,
            toughness: -11,
        },
        Case {
            id: "fatal_fumes",
            mana: ManaGift {
                b: 1,
                c: 3,
                ..Default::default()
            },
            power: -4,
            toughness: -2,
        },
        Case {
            id: "flatten",
            mana: ManaGift {
                b: 1,
                c: 3,
                ..Default::default()
            },
            power: -4,
            toughness: -4,
        },
        Case {
            id: "grasp_of_darkness",
            mana: ManaGift {
                b: 2,
                ..Default::default()
            },
            power: -4,
            toughness: -4,
        },
        Case {
            id: "hydrosurge",
            mana: ManaGift {
                u: 1,
                ..Default::default()
            },
            power: -5,
            toughness: -0,
        },
        Case {
            id: "lash_of_the_whip",
            mana: ManaGift {
                b: 1,
                c: 4,
                ..Default::default()
            },
            power: -4,
            toughness: -4,
        },
        Case {
            id: "pull_under",
            mana: ManaGift {
                b: 1,
                c: 5,
                ..Default::default()
            },
            power: -5,
            toughness: -5,
        },
        Case {
            id: "shrink",
            mana: ManaGift {
                g: 1,
                ..Default::default()
            },
            power: -5,
            toughness: -0,
        },
        Case {
            id: "strangling_spores",
            mana: ManaGift {
                b: 1,
                c: 3,
                ..Default::default()
            },
            power: -3,
            toughness: -3,
        },
        Case {
            id: "tar_snare",
            mana: ManaGift {
                b: 1,
                c: 2,
                ..Default::default()
            },
            power: -3,
            toughness: -2,
        },
        Case {
            id: "throttle",
            mana: ManaGift {
                b: 1,
                c: 4,
                ..Default::default()
            },
            power: -4,
            toughness: -4,
        },
        Case {
            id: "wring_flesh",
            mana: ManaGift {
                b: 1,
                ..Default::default()
            },
            power: -3,
            toughness: -1,
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

fn pool(e: &GameEngine) -> (u32, u32, u32, u32, u32, u32) {
    let pool = &e.state.players[0].mana_pool;
    (
        pool.white,
        pool.blue,
        pool.black,
        pool.red,
        pool.green,
        pool.colorless,
    )
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

fn begin_cast_at(e: &mut GameEngine, id: &str, target: u32, mana: ManaGift) -> u32 {
    give_mana(e, 0, mana);
    let slot = inject_card_into_hand(e, 0, id);
    let hand_slot = hand_index_for_card(e, 0, id);
    semantic::accepted(e, 0, &cast_spell(hand_slot, target_object(target)));
    assert_eq!(pool(e), (0, 0, 0, 0, 0, 0), "{id}: exact cost consumed");
    slot
}

fn cast_at(e: &mut GameEngine, id: &str, target: u32, mana: ManaGift) -> u32 {
    let slot = begin_cast_at(e, id, target, mana);
    resolve_entire_stack_two_player(e);
    assert_eq!(
        e.state.objects[&slot].zone,
        Zone::Graveyard,
        "{id}: the spell resolves to its owner's graveyard"
    );
    slot
}

#[test]
fn shrink_batch_cards_apply_exact_negative_pump_and_go_to_graveyard() {
    for (case_index, case) in cases().into_iter().enumerate() {
        let mut e = engine(790_000 + case_index as u64);
        let target = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 20, 20);
        let opponent_hand = e.state.players[1].hand.len();
        let opponent_library = e.state.players[1].library.len();
        cast_at(&mut e, case.id, target, case.mana);
        assert_eq!(
            e.effective_power(target),
            Some((20 + case.power) as u32),
            "{}: exact power after the shrink",
            case.id
        );
        assert_eq!(
            e.effective_toughness(target),
            Some((20 + case.toughness) as u32),
            "{}: exact toughness after the shrink",
            case.id
        );
        assert_eq!(
            e.state.players[1].hand.len(),
            opponent_hand,
            "{}: opponent hand is unchanged",
            case.id
        );
        assert_eq!(
            e.state.players[1].library.len(),
            opponent_library,
            "{}: opponent library is unchanged",
            case.id
        );
    }
}

#[test]
fn shrink_batch_cards_expire_at_cleanup() {
    for (case_index, case) in cases().into_iter().enumerate() {
        let mut e = engine(791_000 + case_index as u64);
        let target = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 20, 20);
        cast_at(&mut e, case.id, target, gift(&case.mana));
        assert_eq!(
            e.effective_power(target),
            Some((20 + case.power) as u32),
            "{}: pumped before cleanup",
            case.id
        );
        end_active_turn(&mut e, 0);
        assert_eq!(
            e.effective_power(target),
            Some(20),
            "{}: power returns at cleanup",
            case.id
        );
        assert_eq!(
            e.effective_toughness(target),
            Some(20),
            "{}: toughness returns at cleanup",
            case.id
        );
    }
}

#[test]
fn shrink_batch_cards_resolve_toughness_lethality_and_power_floor() {
    for (case_index, case) in cases().into_iter().enumerate() {
        let mut e = engine(792_000 + case_index as u64);
        let target = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        cast_at(&mut e, case.id, target, gift(&case.mana));
        if 2 + case.toughness <= 0 {
            assert_eq!(
                e.state.objects[&target].zone,
                Zone::Graveyard,
                "{}: zero toughness sends the creature to the graveyard",
                case.id
            );
        } else {
            assert_eq!(
                e.state.objects[&target].zone,
                Zone::Battlefield,
                "{}: a nonlethal shrink leaves the creature alive",
                case.id
            );
            assert_eq!(
                e.effective_power(target),
                Some((2 + case.power).max(0) as u32),
                "{}: power floors at zero",
                case.id
            );
        }
    }
}

#[test]
fn shrink_batch_cards_fizzle_when_the_target_left() {
    for (case_index, case) in cases().into_iter().enumerate() {
        let mut e = engine(793_000 + case_index as u64);
        let target = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 20, 20);
        let slot = begin_cast_at(&mut e, case.id, target, gift(&case.mana));
        e.state.players[1]
            .battlefield
            .retain(|object| *object != target);
        e.state.players[1].graveyard.push(target);
        e.state.objects.get_mut(&target).expect("target").zone = Zone::Graveyard;
        *e.state.zone_change_generation.entry(target).or_default() += 1;
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.objects[&target].zone,
            Zone::Graveyard,
            "{}: the stale target remains in the graveyard",
            case.id
        );
        assert_eq!(
            e.state.objects[&slot].zone,
            Zone::Graveyard,
            "{}: the fizzled spell goes to its owner's graveyard",
            case.id
        );
        assert_eq!(
            e.effective_power(target),
            Some(20),
            "{}: a fizzled spell changes no characteristics",
            case.id
        );
    }
}

#[test]
fn disorient_floors_power_at_zero() {
    let mut e = engine(794_001);
    let target = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 1, 1);
    cast_at(
        &mut e,
        "disorient",
        target,
        ManaGift {
            u: 1,
            c: 3,
            ..Default::default()
        },
    );
    assert_eq!(
        e.effective_power(target),
        Some(0),
        "negative power is not exposed as negative"
    );
    assert_eq!(e.effective_toughness(target), Some(1));
}

#[test]
fn dismember_pays_black_mana_or_two_life_per_symbol_and_rejects_at_three_life() {
    let black = ManaGift {
        b: 2,
        c: 1,
        ..Default::default()
    };
    let mut black_paid = engine(795_001);
    let target = inject_creature_with_stats(&mut black_paid, 1, "grizzly_bears", 20, 20);
    let life = black_paid.state.players[0].life;
    cast_at(&mut black_paid, "dismember", target, black);
    assert_eq!(
        black_paid.state.players[0].life, life,
        "black mana does not cost life"
    );
    assert_eq!(
        black_paid.effective_power(target),
        Some(15),
        "-5/-5 from black-mana Dismember"
    );
    assert_eq!(black_paid.effective_toughness(target), Some(15));

    let mut life_paid = engine(795_002);
    let target = inject_creature_with_stats(&mut life_paid, 1, "grizzly_bears", 20, 20);
    let life = life_paid.state.players[0].life;
    give_mana(
        &mut life_paid,
        0,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );
    let _slot = inject_card_into_hand(&mut life_paid, 0, "dismember");
    let hand_slot = hand_index_for_card(&life_paid, 0, "dismember");
    semantic::accepted(
        &mut life_paid,
        0,
        &cast_spell_flex(
            hand_slot,
            target_object(target),
            vec![
                FlexPipPayment {
                    pip_index: 1,
                    pay_life: true,
                    mana_color: 0,
                },
                FlexPipPayment {
                    pip_index: 2,
                    pay_life: true,
                    mana_color: 0,
                },
            ],
        ),
    );
    assert_eq!(
        life_paid.state.players[0].life,
        life - 4,
        "two Phyrexian symbols cost 4 life"
    );
    resolve_entire_stack_two_player(&mut life_paid);
    assert_eq!(life_paid.effective_power(target), Some(15));
    assert_eq!(life_paid.effective_toughness(target), Some(15));

    let mut one_life = engine(795_003);
    one_life.state.players[0].life = 3;
    let target = inject_creature_with_stats(&mut one_life, 1, "grizzly_bears", 20, 20);
    give_mana(
        &mut one_life,
        0,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );
    let slot = inject_card_into_hand(&mut one_life, 0, "dismember");
    let hand_slot = hand_index_for_card(&one_life, 0, "dismember");
    assert!(
        one_life
            .apply_scenario_command(
                0,
                &cast_spell_flex(
                    hand_slot,
                    target_object(target),
                    vec![
                        FlexPipPayment {
                            pip_index: 1,
                            pay_life: true,
                            mana_color: 0,
                        },
                        FlexPipPayment {
                            pip_index: 2,
                            pay_life: true,
                            mana_color: 0,
                        },
                    ],
                ),
            )
            .is_err(),
        "3 life cannot pay two Phyrexian symbols"
    );
    assert_eq!(one_life.state.players[0].life, 3);
    assert!(one_life.state.stack.is_empty());
    assert!(
        one_life.state.players[0].hand.contains(&slot),
        "the rejected cast keeps Dismember in hand"
    );

    give_mana(
        &mut one_life,
        0,
        ManaGift {
            b: 2,
            ..Default::default()
        },
    );
    let hand_slot = hand_index_for_card(&one_life, 0, "dismember");
    semantic::accepted(
        &mut one_life,
        0,
        &cast_spell(hand_slot, target_object(target)),
    );
    assert_eq!(
        one_life.state.players[0].life, 3,
        "the black branch remains available at 3 life"
    );
    resolve_entire_stack_two_player(&mut one_life);
    assert_eq!(one_life.effective_power(target), Some(15));
}
