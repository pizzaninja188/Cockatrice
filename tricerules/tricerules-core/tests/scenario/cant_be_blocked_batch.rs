//! Actual-card scenarios for the reviewed "This creature can't be blocked" cohort.
//!
//! Expectations are printed Oracle values. CR 509.1b makes a declaration of
//! blockers illegal when a blocking restriction is disobeyed. The restriction
//! applies only to the printed source and never to other creatures.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;

struct Case {
    id: &'static str,
    power: u32,
    toughness: u32,
}

fn cases() -> [Case; 12] {
    [
        Case {
            id: "covert_operative",
            power: 3,
            toughness: 2,
        },
        Case {
            id: "jhessian_infiltrator",
            power: 2,
            toughness: 2,
        },
        Case {
            id: "latch_seeker",
            power: 3,
            toughness: 1,
        },
        Case {
            id: "metathran_soldier",
            power: 1,
            toughness: 1,
        },
        Case {
            id: "mist-cloaked_herald",
            power: 1,
            toughness: 1,
        },
        Case {
            id: "phantom_ninja",
            power: 2,
            toughness: 2,
        },
        Case {
            id: "phantom_warrior",
            power: 2,
            toughness: 2,
        },
        Case {
            id: "plasma_elemental",
            power: 4,
            toughness: 1,
        },
        Case {
            id: "slither_blade",
            power: 1,
            toughness: 2,
        },
        Case {
            id: "talas_warrior",
            power: 2,
            toughness: 2,
        },
        Case {
            id: "tidal_kraken",
            power: 6,
            toughness: 6,
        },
        Case {
            id: "triton_shorestalker",
            power: 1,
            toughness: 1,
        },
    ]
}

fn engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![
        deck_with("island", &[card]),
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

fn attack_and_reach_blockers(e: &mut GameEngine, attackers: &[u32]) {
    e.apply_scenario_command(0, &primitive_yield())
        .expect("main1 to beginning of combat");
    pass_both_players(e);
    e.apply_scenario_command(0, &declare_attackers(attackers.to_vec()))
        .expect("declare attackers");
    pass_both_players(e);
}

#[test]
fn cant_be_blocked_batch_cards_have_the_printed_characteristics_and_cannot_be_blocked() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_110_000 + index as u64, case.id);
        let source = move_ready_to_battlefield(&mut e, 0, case.id);
        assert_eq!(e.effective_power(source), Some(case.power), "{}", case.id);
        assert_eq!(
            e.effective_toughness(source),
            Some(case.toughness),
            "{}",
            case.id
        );
        assert_eq!(
            zone_view_rules_annotation_labels(&mut e, 0, source),
            vec!["Can't be blocked"],
            "{}: the blocking restriction is published",
            case.id
        );
    }
}

#[test]
fn cant_be_blocked_batch_cards_are_never_offered_as_blockable() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_111_000 + index as u64, case.id);
        let attacker = move_ready_to_battlefield(&mut e, 0, case.id);
        inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        attack_and_reach_blockers(&mut e, &[attacker]);
        let legal = &e.initial_response_batch().legal_by_player[&1];
        assert!(
            !legal
                .legal_block_pairs
                .iter()
                .any(|pair| pair.attacker_id == attacker),
            "{}: no legal blocker is offered",
            case.id
        );
    }
}

#[test]
fn cant_be_blocked_batch_cards_reject_declared_blockers() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_112_000 + index as u64, case.id);
        let attacker = move_ready_to_battlefield(&mut e, 0, case.id);
        let blocker = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        attack_and_reach_blockers(&mut e, &[attacker]);
        assert!(
            e.apply_scenario_command(
                1,
                &declare_blockers(vec![BlockPair {
                    attacker_id: attacker,
                    blocker_id: blocker,
                }]),
            )
            .is_err(),
            "{}: the engine rejects the forbidden block",
            case.id
        );

        let mut e = engine(9_113_000 + index as u64, case.id);
        let restricted = move_ready_to_battlefield(&mut e, 0, case.id);
        let ordinary = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        let blocker = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        attack_and_reach_blockers(&mut e, &[ordinary, restricted]);
        let legal = &e.initial_response_batch().legal_by_player[&1];
        assert!(
            legal
                .legal_block_pairs
                .iter()
                .any(|pair| pair.attacker_id == ordinary && pair.blocker_id == blocker),
            "{}: an unrestricted attacker is still blockable",
            case.id
        );
        assert!(
            !legal
                .legal_block_pairs
                .iter()
                .any(|pair| pair.attacker_id == restricted),
            "{}: the restriction is source-only",
            case.id
        );
        e.apply_scenario_command(
            1,
            &declare_blockers(vec![BlockPair {
                attacker_id: ordinary,
                blocker_id: blocker,
            }]),
        )
        .expect("the ordinary attacker is blocked");
    }
}

#[test]
fn cant_be_blocked_batch_cards_reject_a_mixed_block_declaration() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_114_000 + index as u64, case.id);
        let restricted = move_ready_to_battlefield(&mut e, 0, case.id);
        let ordinary = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        let legal_blocker = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        let restricted_blocker = inject_creature_on_battlefield(&mut e, 1, "savannah_lions");
        attack_and_reach_blockers(&mut e, &[ordinary, restricted]);
        assert!(
            e.apply_scenario_command(
                1,
                &declare_blockers(vec![
                    BlockPair {
                        attacker_id: restricted,
                        blocker_id: restricted_blocker,
                    },
                    BlockPair {
                        attacker_id: ordinary,
                        blocker_id: legal_blocker,
                    },
                ]),
            )
            .is_err(),
            "{}: one illegal pair rejects the whole declaration",
            case.id
        );
        assert!(
            !e.state.combat.as_ref().expect("combat").blockers_declared,
            "{}: the rejected declaration is not made",
            case.id
        );
        e.apply_scenario_command(
            1,
            &declare_blockers(vec![BlockPair {
                attacker_id: ordinary,
                blocker_id: legal_blocker,
            }]),
        )
        .expect("the legal pair is accepted on its own");
    }
}
