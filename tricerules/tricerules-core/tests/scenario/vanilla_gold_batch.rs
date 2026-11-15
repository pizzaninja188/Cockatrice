//! Actual-card scenarios for the reviewed multicolor legendary vanilla creatures.
//!
//! Expectations are printed Oracle values. CR 202.1 and 205.4a: these cards have
//! no rules text; their printed mana cost, legendary supertype, type line, power
//! and toughness are the whole card. The scenarios cast and use each actual card
//! in combat.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;

struct Case {
    id: &'static str,
    mana: ManaGift,
    body: (u32, u32),
}

fn cases() -> [Case; 9] {
    [
        Case {
            id: "barktooth_warbeard",
            mana: ManaGift {
                b: 1,
                r: 2,
                c: 4,
                ..Default::default()
            },
            body: (6, 5),
        },
        Case {
            id: "lady_orca",
            mana: ManaGift {
                b: 1,
                r: 1,
                c: 5,
                ..Default::default()
            },
            body: (7, 4),
        },
        Case {
            id: "sivitri_scarzam",
            mana: ManaGift {
                u: 1,
                b: 1,
                c: 5,
                ..Default::default()
            },
            body: (6, 4),
        },
        Case {
            id: "jerrard_of_the_closed_fist",
            mana: ManaGift {
                r: 1,
                g: 2,
                c: 3,
                ..Default::default()
            },
            body: (6, 5),
        },
        Case {
            id: "the_lady_of_the_mountain",
            mana: ManaGift {
                r: 1,
                g: 1,
                c: 4,
                ..Default::default()
            },
            body: (5, 5),
        },
        Case {
            id: "sir_shandlar_of_eberyn",
            mana: ManaGift {
                g: 1,
                w: 1,
                c: 4,
                ..Default::default()
            },
            body: (4, 7),
        },
        Case {
            id: "torsten_von_ursus",
            mana: ManaGift {
                g: 2,
                w: 1,
                c: 3,
                ..Default::default()
            },
            body: (5, 5),
        },
        Case {
            id: "jedit_ojanen",
            mana: ManaGift {
                w: 2,
                u: 1,
                c: 4,
                ..Default::default()
            },
            body: (5, 5),
        },
        Case {
            id: "tobias_andrion",
            mana: ManaGift {
                w: 1,
                u: 1,
                c: 3,
                ..Default::default()
            },
            body: (4, 4),
        },
    ]
}

fn engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![deck_with("forest", &[card]), deck_with("forest", &[])]);
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

fn attack_engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![deck_with("forest", &[card]), deck_with("forest", &[])]);
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("engine");
    advance_to_declare_attackers(&mut e);
    e
}

fn block_engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![deck_with("forest", &[]), deck_with("forest", &[card])]);
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("engine");
    advance_to_declare_attackers(&mut e);
    e
}

fn cast_creature(e: &mut GameEngine, case: &Case) -> u32 {
    let slot = inject_card_into_hand(e, 0, case.id);
    give_mana(
        e,
        0,
        ManaGift {
            w: case.mana.w,
            u: case.mana.u,
            b: case.mana.b,
            r: case.mana.r,
            g: case.mana.g,
            c: case.mana.c,
        },
    );
    let hand_slot = e.state.players[0]
        .hand
        .iter()
        .position(|oid| *oid == slot)
        .expect("the injected card is in hand");
    e.apply_scenario_command(0, &cast_spell(hand_slot, vec![]))
        .expect("cast the vanilla creature");
    resolve_entire_stack_two_player(e);
    battlefield_object_for_card(e, 0, case.id)
}

#[test]
fn vanilla_gold_batch_cards_cast_with_printed_characteristics() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_280_000 + index as u64, case.id);
        let creature = cast_creature(&mut e, &case);
        assert_eq!(
            e.state.objects[&creature].zone,
            Zone::Battlefield,
            "{}",
            case.id
        );
        assert_eq!(
            e.effective_power(creature),
            Some(case.body.0),
            "{}",
            case.id
        );
        assert_eq!(
            e.effective_toughness(creature),
            Some(case.body.1),
            "{}",
            case.id
        );
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}

#[test]
fn vanilla_gold_batch_cards_attack_and_block_with_printed_stats() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = attack_engine(9_281_000 + index as u64, case.id);
        let attacker = move_ready_to_battlefield(&mut e, 0, case.id);
        e.apply_scenario_command(0, &declare_attackers(vec![attacker]))
            .expect("declare the vanilla attacker");
        assert!(
            e.state
                .combat
                .as_ref()
                .expect("combat")
                .attacking
                .contains(&attacker),
            "{}: attacks",
            case.id
        );
        pass_both_players(&mut e);
        e.apply_scenario_command(1, &declare_blockers(vec![]))
            .expect("declare no blockers");
        pass_both_players(&mut e);
        assert_eq!(
            e.state.players[1].life,
            20 - i32::try_from(case.body.0).expect("power fits"),
            "{}: printed power is used in combat",
            case.id
        );

        let mut block = block_engine(9_282_000 + index as u64, case.id);
        let blocker = move_ready_to_battlefield(&mut block, 1, case.id);
        let attacker = inject_creature_on_battlefield(&mut block, 0, "grizzly_bears");
        block
            .apply_scenario_command(0, &declare_attackers(vec![attacker]))
            .expect("declare the generic attacker");
        block
            .apply_scenario_command(0, &pass())
            .expect("active pass declare attackers");
        block
            .apply_scenario_command(1, &pass())
            .expect("defender reaches blockers");
        assert_eq!(
            block.state.turn_step,
            tricerules_core::TurnStep::DeclareBlockers
        );
        block
            .apply_scenario_command(
                1,
                &declare_blockers(vec![BlockPair {
                    attacker_id: attacker,
                    blocker_id: blocker,
                }]),
            )
            .expect("the vanilla creature blocks");
        assert!(
            block
                .state
                .combat
                .as_ref()
                .expect("combat")
                .blockers_declared,
            "{}: blocks",
            case.id
        );
    }
}
