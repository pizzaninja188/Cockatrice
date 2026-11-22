//! Actual-card scenarios for the reviewed Flying / blocks-only-flying cohort.
//!
//! Expectations are printed Oracle values. CR 702.9a-b define Flying. CR 509.1b
//! makes a declaration of blockers illegal when a blocking restriction is
//! disobeyed; reach alone does not satisfy "creatures with flying".
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_cards::Keyword;

struct Case {
    id: &'static str,
    power: u32,
    toughness: u32,
}

fn cases() -> [Case; 15] {
    [
        Case {
            id: "belbes_percher",
            power: 2,
            toughness: 2,
        },
        Case {
            id: "cloud_djinn",
            power: 5,
            toughness: 4,
        },
        Case {
            id: "cloud_dragon",
            power: 5,
            toughness: 4,
        },
        Case {
            id: "cloud_elemental",
            power: 2,
            toughness: 3,
        },
        Case {
            id: "cloud_pirates",
            power: 1,
            toughness: 1,
        },
        Case {
            id: "cloud_spirit",
            power: 3,
            toughness: 1,
        },
        Case {
            id: "hoverguard_observer",
            power: 3,
            toughness: 3,
        },
        Case {
            id: "long-finned_skywhale",
            power: 4,
            toughness: 3,
        },
        Case {
            id: "rishadan_airship",
            power: 3,
            toughness: 1,
        },
        Case {
            id: "skywinder_drake",
            power: 3,
            toughness: 1,
        },
        Case {
            id: "stratozeppelid",
            power: 4,
            toughness: 4,
        },
        Case {
            id: "stronghold_zeppelin",
            power: 3,
            toughness: 3,
        },
        Case {
            id: "tattered_haunter",
            power: 2,
            toughness: 1,
        },
        Case {
            id: "vaporkin",
            power: 2,
            toughness: 1,
        },
        Case {
            id: "wanderlight_spirit",
            power: 2,
            toughness: 3,
        },
    ]
}

fn engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![
        deck_with("island", &["storm_crow", "grizzly_bears", "giant_spider"]),
        deck_with("island", &[card]),
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

fn attack_and_reach_blockers(e: &mut GameEngine, attacker: u32) {
    e.apply_scenario_command(0, &primitive_yield())
        .expect("main1 to beginning of combat");
    pass_both_players(e);
    e.apply_scenario_command(0, &declare_attackers(vec![attacker]))
        .expect("declare attacker");
    pass_both_players(e);
}

#[test]
fn flying_blocks_only_flying_batch_cards_have_flying_and_the_restriction() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_130_000 + index as u64, case.id);
        let source = move_ready_to_battlefield(&mut e, 1, case.id);
        assert!(
            e.effective_has_keyword(source, Keyword::Flying),
            "{}: printed Flying is active",
            case.id
        );
        assert_eq!(e.effective_power(source), Some(case.power), "{}", case.id);
        assert_eq!(
            e.effective_toughness(source),
            Some(case.toughness),
            "{}",
            case.id
        );
        assert_eq!(
            zone_view_rules_annotation_labels(&mut e, 1, source),
            vec!["Can't block creatures without flying"],
            "{}: the block restriction is published",
            case.id
        );
    }
}

#[test]
fn flying_blocks_only_flying_batch_cards_block_flying_attackers() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_131_000 + index as u64, case.id);
        let attacker = inject_creature_on_battlefield(&mut e, 0, "storm_crow");
        let blocker = move_ready_to_battlefield(&mut e, 1, case.id);
        attack_and_reach_blockers(&mut e, attacker);
        let legal = &e.initial_response_batch().legal_by_player[&1];
        assert!(
            legal
                .legal_block_pairs
                .iter()
                .any(|pair| pair.attacker_id == attacker && pair.blocker_id == blocker),
            "{}: a flying attacker is a legal choice",
            case.id
        );
        e.apply_scenario_command(
            1,
            &declare_blockers(vec![BlockPair {
                attacker_id: attacker,
                blocker_id: blocker,
            }]),
        )
        .expect("the flying attacker is blocked");
    }
}

#[test]
fn flying_blocks_only_flying_batch_cards_reject_ground_and_reach_only_attackers() {
    for (index, case) in cases().into_iter().enumerate() {
        for (attacker_id, label) in [("grizzly_bears", "ground"), ("giant_spider", "reach")] {
            let mut e = engine(
                9_132_000 + index as u64 * 10 + u64::from(label == "reach"),
                case.id,
            );
            let attacker = inject_creature_on_battlefield(&mut e, 0, attacker_id);
            let blocker = move_ready_to_battlefield(&mut e, 1, case.id);
            attack_and_reach_blockers(&mut e, attacker);
            let legal = &e.initial_response_batch().legal_by_player[&1];
            assert!(
                !legal
                    .legal_block_pairs
                    .iter()
                    .any(|pair| pair.attacker_id == attacker && pair.blocker_id == blocker),
                "{}: a {label} attacker is not offered",
                case.id
            );
            assert!(
                e.apply_scenario_command(
                    1,
                    &declare_blockers(vec![BlockPair {
                        attacker_id: attacker,
                        blocker_id: blocker,
                    }]),
                )
                .is_err(),
                "{}: the {label} attacker cannot be blocked",
                case.id
            );
        }
    }
}
