//! Actual-card scenarios for the reviewed Flying-only creature family.
//!
//! Expectations are printed Oracle values. CR 702.9a-b define Flying as an
//! evasion ability: a flying creature can't be blocked except by creatures
//! with flying and/or reach. CR 509.1b requires every applicable blocking
//! restriction to hold. The cards have no other abilities.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_cards::Keyword;
use tricerules_core::Zone;

struct Case {
    id: &'static str,
    mana: ManaGift,
    body: (u32, u32),
}

fn cases() -> [Case; 7] {
    [
        Case {
            id: "roc_of_kher_ridges",
            mana: ManaGift {
                r: 1,
                c: 3,
                ..Default::default()
            },
            body: (3, 3),
        },
        Case {
            id: "spotted_griffin",
            mana: ManaGift {
                w: 1,
                c: 3,
                ..Default::default()
            },
            body: (2, 3),
        },
        Case {
            id: "scryb_sprites",
            mana: ManaGift {
                g: 1,
                ..Default::default()
            },
            body: (1, 1),
        },
        Case {
            id: "fighting_drake",
            mana: ManaGift {
                u: 2,
                c: 2,
                ..Default::default()
            },
            body: (2, 4),
        },
        Case {
            id: "bird_maiden",
            mana: ManaGift {
                r: 1,
                c: 2,
                ..Default::default()
            },
            body: (1, 2),
        },
        Case {
            id: "armored_pegasus",
            mana: ManaGift {
                w: 1,
                c: 1,
                ..Default::default()
            },
            body: (1, 2),
        },
        Case {
            id: "exocomp",
            mana: ManaGift {
                u: 1,
                c: 2,
                ..Default::default()
            },
            body: (3, 1),
        },
    ]
}

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("island", &[]), deck_with("forest", &[])]);
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
    inject_card_into_hand(e, 0, case.id);
    let mut mana = ManaGift::default();
    mana.w = case.mana.w;
    mana.u = case.mana.u;
    mana.b = case.mana.b;
    mana.r = case.mana.r;
    mana.g = case.mana.g;
    mana.c = case.mana.c;
    give_mana(e, 0, mana);
    let slot = hand_index_for_card(e, 0, case.id);
    e.apply_scenario_command(0, &cast_spell(slot, vec![]))
        .expect("cast the flying creature");
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
    assert_eq!(
        e.state.turn_step,
        tricerules_core::TurnStep::DeclareAttackers
    );
    e.apply_scenario_command(0, &declare_attackers(vec![creature]))
        .expect("declare the flyer");
    e.apply_scenario_command(0, &pass())
        .expect("active pass declare attackers");
    e.apply_scenario_command(1, &pass())
        .expect("defender pass reaches manual blockers");
    assert_eq!(
        e.state.turn_step,
        tricerules_core::TurnStep::DeclareBlockers
    );
}

#[test]
fn flying_creatures_batch_cards_cast_with_exact_printed_characteristics() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(900_000 + index as u64);
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
        assert!(e.state.stack.is_empty(), "{}", case.id);
        assert!(e.state.pending_resolution.is_none(), "{}", case.id);
    }
}

#[test]
fn flying_creatures_batch_cards_cannot_be_blocked_by_ground_creatures() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(901_000 + index as u64);
        let creature = inject_creature_on_battlefield(&mut e, 0, case.id);
        let blocker = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        let _eligible = inject_creature_on_battlefield(&mut e, 1, "storm_crow");
        attack_to_blocks(&mut e, creature);
        let result = e.apply_scenario_command(
            1,
            &declare_blockers(vec![BlockPair {
                attacker_id: creature,
                blocker_id: blocker,
            }]),
        );
        let error = result.expect_err("a ground creature cannot block a flyer");
        assert!(
            error.to_string().contains("flying") || error.to_string().contains("evasion"),
            "{}: unexpected rejection: {error}",
            case.id
        );
    }
}

#[test]
fn flying_creatures_batch_cards_can_be_blocked_by_flying_or_reach() {
    for (index, case) in cases().into_iter().enumerate() {
        for (blocker, label) in [("storm_crow", "flying"), ("giant_spider", "reach")] {
            let mut e = engine(902_000 + index as u64 * 10 + u64::from(label == "reach"));
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
