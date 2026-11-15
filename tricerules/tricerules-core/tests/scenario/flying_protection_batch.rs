//! Actual-card scenarios for the reviewed Flying / protection cohort.
//!
//! Expectations are printed Oracle values. CR 702.9a-b define Flying; CR 702.16a-f
//! define protection as a static ability with five separate effects, including
//! preventing damage from the protected color and prohibiting blockers of that
//! color.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_cards::primitives::ProtectionQuality;
use tricerules_cards::{Color, Keyword};
use tricerules_core::{TurnStep, Zone};

struct Case {
    id: &'static str,
    mana: ManaGift,
    body: (u32, u32),
    protected: Color,
}

fn cases() -> [Case; 8] {
    [
        Case {
            id: "coast_watcher",
            mana: ManaGift {
                u: 1,
                c: 1,
                ..Default::default()
            },
            body: (1, 1),
            protected: Color::Green,
        },
        Case {
            id: "treetop_sentinel",
            mana: ManaGift {
                u: 2,
                c: 2,
                ..Default::default()
            },
            body: (2, 3),
            protected: Color::Green,
        },
        Case {
            id: "cerulean_wyvern",
            mana: ManaGift {
                u: 1,
                c: 4,
                ..Default::default()
            },
            body: (3, 3),
            protected: Color::Green,
        },
        Case {
            id: "voice_of_duty",
            mana: ManaGift {
                w: 1,
                c: 3,
                ..Default::default()
            },
            body: (2, 2),
            protected: Color::Green,
        },
        Case {
            id: "melesse_spirit",
            mana: ManaGift {
                w: 2,
                c: 3,
                ..Default::default()
            },
            body: (3, 3),
            protected: Color::Black,
        },
        Case {
            id: "voice_of_grace",
            mana: ManaGift {
                w: 1,
                c: 3,
                ..Default::default()
            },
            body: (2, 2),
            protected: Color::Black,
        },
        Case {
            id: "nightwind_glider",
            mana: ManaGift {
                w: 1,
                c: 2,
                ..Default::default()
            },
            body: (2, 1),
            protected: Color::Black,
        },
        Case {
            id: "duskrider_falcon",
            mana: ManaGift {
                w: 1,
                c: 1,
                ..Default::default()
            },
            body: (1, 1),
            protected: Color::Black,
        },
    ]
}

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("plains", &[]), deck_with("forest", &[])]);
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

fn prevention_engine(seed: u64, case: &Case, attacker: &str) -> GameEngine {
    let decks = Some(vec![
        deck_with("plains", &[case.id]),
        deck_with("forest", &[attacker]),
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
        .expect("cast the protection creature");
    resolve_entire_stack_two_player(e);
    battlefield_object_for_card(e, 0, case.id)
}

fn attack_to_blocks(e: &mut GameEngine, creature: u32) {
    e.apply_scenario_command(0, &primitive_yield())
        .expect("main1 to begin combat");
    e.apply_scenario_command(0, &pass())
        .expect("active pass begin combat");
    e.apply_scenario_command(1, &pass())
        .expect("defender pass begin combat");
    assert_eq!(e.state.turn_step, TurnStep::DeclareAttackers);
    e.apply_scenario_command(0, &declare_attackers(vec![creature]))
        .expect("declare the flyer");
    e.apply_scenario_command(0, &pass())
        .expect("active pass declare attackers");
    e.apply_scenario_command(1, &pass())
        .expect("defender pass reaches manual blockers");
    assert_eq!(e.state.turn_step, TurnStep::DeclareBlockers);
}

fn ground_blocker(case: &Case) -> &'static str {
    match case.protected {
        Color::Green => "scathe_zombies",
        Color::Black => "grizzly_bears",
        _ => unreachable!(),
    }
}

fn reach_blocker(case: &Case) -> &'static str {
    match case.protected {
        Color::Green => "kavaron_skywarden",
        Color::Black => "giant_spider",
        _ => unreachable!(),
    }
}

fn protected_blocker(case: &Case) -> &'static str {
    match case.protected {
        Color::Green => "giant_spider",
        Color::Black => "bloodthirsty_aerialist",
        _ => unreachable!(),
    }
}

fn protected_attacker(case: &Case) -> &'static str {
    match case.protected {
        Color::Green => "grizzly_bears",
        Color::Black => "scathe_zombies",
        _ => unreachable!(),
    }
}

#[test]
fn flying_protection_batch_cards_cast_with_exact_printed_characteristics() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_250_000 + index as u64);
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
        assert!(
            e.effective_has_keyword(creature, Keyword::Flying),
            "{}: has Flying",
            case.id
        );
        assert_eq!(
            e.characteristics(creature)
                .expect("characteristics")
                .protections,
            [ProtectionQuality::Color(case.protected)],
            "{}: protection quality",
            case.id
        );
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}

#[test]
fn flying_protection_batch_cards_cannot_be_blocked_by_ground_creatures() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_251_000 + index as u64);
        let creature = inject_creature_on_battlefield(&mut e, 0, case.id);
        let blocker = inject_creature_on_battlefield(&mut e, 1, ground_blocker(&case));
        let _eligible = inject_creature_on_battlefield(&mut e, 1, "storm_crow");
        attack_to_blocks(&mut e, creature);
        let error = e
            .apply_scenario_command(
                1,
                &declare_blockers(vec![BlockPair {
                    attacker_id: creature,
                    blocker_id: blocker,
                }]),
            )
            .expect_err("a ground creature cannot block a flyer");
        assert!(
            error.to_string().contains("flying") || error.to_string().contains("evasion"),
            "{}: unexpected rejection: {error}",
            case.id
        );
    }
}

#[test]
fn flying_protection_batch_cards_can_be_blocked_by_flying_or_reach() {
    for (index, case) in cases().into_iter().enumerate() {
        for (blocker, label) in [("storm_crow", "flying"), (reach_blocker(&case), "reach")] {
            let mut e = engine(9_252_000 + index as u64 * 10 + u64::from(label == "reach"));
            let creature = inject_creature_on_battlefield(&mut e, 0, case.id);
            let blocker_id = inject_creature_on_battlefield(&mut e, 1, blocker);
            attack_to_blocks(&mut e, creature);
            e.apply_scenario_command(
                1,
                &declare_blockers(vec![BlockPair {
                    attacker_id: creature,
                    blocker_id,
                }]),
            )
            .unwrap_or_else(|error| panic!("{}: {label} blocker rejected: {error}", case.id));
            assert!(
                e.state.combat.as_ref().expect("combat").blockers_declared,
                "{}: {label} blocker accepted",
                case.id
            );
        }
    }
}

#[test]
fn flying_protection_batch_cards_prevent_damage_from_the_protected_color() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = prevention_engine(9_253_000 + index as u64, &case, protected_attacker(&case));
        let protected = move_ready_to_battlefield(&mut e, 0, case.id);
        let attacker_card = protected_attacker(&case);
        let attacker = move_ready_to_battlefield(&mut e, 1, attacker_card);
        end_active_turn(&mut e, 0);
        assert_eq!(e.state.turn_step, TurnStep::Upkeep);
        pass_priority_round(&mut e);
        pass_priority_round(&mut e);
        assert_eq!(e.state.turn_step, TurnStep::Main1);
        e.apply_scenario_command(1, &primitive_yield())
            .expect("opponent main1 to begin combat");
        e.apply_scenario_command(1, &pass())
            .expect("opponent pass begin combat");
        e.apply_scenario_command(0, &pass())
            .expect("defender pass begin combat");
        assert_eq!(e.state.turn_step, TurnStep::DeclareAttackers);
        e.apply_scenario_command(1, &declare_attackers(vec![attacker]))
            .expect("declare the protected-color attacker");
        e.apply_scenario_command(1, &pass())
            .expect("active pass declare attackers");
        e.apply_scenario_command(0, &pass())
            .expect("defender pass reaches blockers");
        e.apply_scenario_command(
            0,
            &declare_blockers(vec![BlockPair {
                attacker_id: attacker,
                blocker_id: protected,
            }]),
        )
        .expect("the protected creature blocks the attacker");
        pass_both_players(&mut e);
        assert_eq!(
            e.state.objects[&protected].damage, 0,
            "{}: damage from the protected color is prevented",
            case.id
        );
        assert_eq!(
            e.state.objects[&protected].zone,
            Zone::Battlefield,
            "{}: the protected creature survives",
            case.id
        );
        assert!(
            e.state.objects[&attacker].zone == Zone::Graveyard
                || e.state.objects[&attacker].damage >= case.body.0,
            "{}: the protected creature still deals its combat damage",
            case.id
        );
    }
}

#[test]
fn flying_protection_batch_cards_reject_blockers_of_the_protected_color() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_254_000 + index as u64);
        let creature = inject_creature_on_battlefield(&mut e, 0, case.id);
        let blocker = inject_creature_on_battlefield(&mut e, 1, protected_blocker(&case));
        attack_to_blocks(&mut e, creature);
        let error = e
            .apply_scenario_command(
                1,
                &declare_blockers(vec![BlockPair {
                    attacker_id: creature,
                    blocker_id: blocker,
                }]),
            )
            .expect_err("a protected-color creature cannot block");
        assert!(
            error.to_string().contains("protection") || error.to_string().contains("block"),
            "{}: unexpected rejection: {error}",
            case.id
        );
    }
}
