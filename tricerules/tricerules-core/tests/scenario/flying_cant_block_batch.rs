//! Actual-card scenarios for the reviewed "Flying / can't block" family.
//!
//! Expectations are printed Oracle values. CR 702.9a-b define Flying as an
//! evasion ability; CR 509.1b makes a declaration of blockers illegal when a
//! blocking restriction is disobeyed. The restriction applies only to the
//! printed source and never to other creatures.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_cards::Keyword;

struct Case {
    id: &'static str,
    power: u32,
    toughness: u32,
}

fn cases() -> [Case; 5] {
    [
        Case {
            id: "aesthir_glider",
            power: 2,
            toughness: 1,
        },
        Case {
            id: "daggerclaw_imp",
            power: 3,
            toughness: 1,
        },
        Case {
            id: "goblin_glider",
            power: 1,
            toughness: 1,
        },
        Case {
            id: "kyren_glider",
            power: 1,
            toughness: 1,
        },
        Case {
            id: "nightshade_stinger",
            power: 1,
            toughness: 1,
        },
    ]
}

fn engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![
        deck_with("forest", &["grizzly_bears"]),
        deck_with("forest", &[card]),
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
fn flying_cant_block_batch_cards_have_flying_and_cannot_block() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_040_000 + index as u64, case.id);
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
            vec!["Can't block"],
            "{}: the block restriction is published",
            case.id
        );
    }
}

#[test]
fn flying_cant_block_batch_cards_are_rejected_as_blockers() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_041_000 + index as u64, case.id);
        let attacker = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        let source = move_ready_to_battlefield(&mut e, 1, case.id);
        attack_and_reach_blockers(&mut e, attacker);
        let legal = &e.initial_response_batch().legal_by_player[&1];
        assert!(
            !legal
                .legal_block_pairs
                .iter()
                .any(|pair| pair.blocker_id == source),
            "{}: an illegal block pair is never offered",
            case.id
        );
        assert!(
            e.apply_scenario_command(
                1,
                &declare_blockers(vec![BlockPair {
                    attacker_id: attacker,
                    blocker_id: source,
                }]),
            )
            .is_err(),
            "{}: the engine rejects the forbidden block",
            case.id
        );
    }
}

#[test]
fn flying_cant_block_batch_cards_allow_other_creatures_to_block() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_042_000 + index as u64, case.id);
        let attacker = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        let restricted = move_ready_to_battlefield(&mut e, 1, case.id);
        let ordinary = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        attack_and_reach_blockers(&mut e, attacker);
        let legal = &e.initial_response_batch().legal_by_player[&1];
        assert!(
            legal
                .legal_block_pairs
                .iter()
                .any(|pair| pair.blocker_id == ordinary),
            "{}: an unrestricted creature may still block",
            case.id
        );
        assert!(
            !legal
                .legal_block_pairs
                .iter()
                .any(|pair| pair.blocker_id == restricted),
            "{}: the restriction is source-only",
            case.id
        );
        e.apply_scenario_command(
            1,
            &declare_blockers(vec![BlockPair {
                attacker_id: attacker,
                blocker_id: ordinary,
            }]),
        )
        .expect("the unrestricted creature blocks");
    }
}
