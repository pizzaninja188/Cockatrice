//! Actual-card scenarios for the reviewed Flying / can-block-only-fliers cohort.
//!
//! Expectations are printed Oracle values. CR 702.9a-b define Flying. CR 509.1b
//! makes a declaration of blockers illegal when a blocking restriction is
//! disobeyed; reach alone does not satisfy "creatures with flying".
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_cards::Keyword;

struct Case {
    id: &'static str,
    mana: ManaGift,
    power: u32,
    toughness: u32,
}

fn cases() -> Vec<Case> {
    vec![
        Case {
            id: "cloud_sprite",
            mana: ManaGift {
                u: 1,
                ..Default::default()
            },
            power: 1,
            toughness: 1,
        },
        Case {
            id: "scrapskin_drake",
            mana: ManaGift {
                u: 1,
                c: 2,
                ..Default::default()
            },
            power: 2,
            toughness: 3,
        },
        Case {
            id: "welkin_tern",
            mana: ManaGift {
                u: 1,
                c: 1,
                ..Default::default()
            },
            power: 2,
            toughness: 1,
        },
    ]
}

fn engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![deck_with("island", &[]), deck_with("island", &[card])]);
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
            u: case.mana.u,
            c: case.mana.c,
            ..Default::default()
        },
    );
    let hand_slot = e.state.players[0]
        .hand
        .iter()
        .position(|oid| *oid == slot)
        .expect("the injected card is in hand");
    e.apply_scenario_command(0, &cast_spell(hand_slot, vec![]))
        .expect("cast the creature");
    let pool = &e.state.players[0].mana_pool;
    assert_eq!(
        (pool.blue, pool.colorless),
        (0, 0),
        "{}: exact cast cost",
        case.id
    );
    resolve_entire_stack_two_player(e);
    battlefield_object_for_card(e, 0, case.id)
}

fn attack(e: &mut GameEngine, attacker: u32) {
    e.apply_scenario_command(0, &primitive_yield())
        .expect("main1 to beginning of combat");
    pass_both_players(e);
    e.apply_scenario_command(0, &declare_attackers(vec![attacker]))
        .expect("declare attacker");
    pass_both_players(e);
}

#[test]
fn flier_blocker_batch_cards_cast_with_exact_printed_characteristics() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_620_000 + index as u64, case.id);
        let source = cast_creature(&mut e, &case);
        assert!(e.state.stack.is_empty(), "{}", case.id);
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
            zone_view_rules_annotation_labels(&mut e, 0, source),
            vec!["Can't block creatures without flying"],
            "{}: the block restriction is published",
            case.id
        );
    }
}

#[test]
fn flier_blocker_batch_cards_block_flying_attackers() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_621_000 + index as u64, case.id);
        let attacker = inject_creature_on_battlefield(&mut e, 0, "storm_crow");
        let blocker = move_ready_to_battlefield(&mut e, 1, case.id);
        attack(&mut e, attacker);
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
fn flier_blocker_batch_cards_reject_ground_and_reach_only_attackers() {
    for (index, case) in cases().into_iter().enumerate() {
        for (attacker_id, label) in [("grizzly_bears", "ground"), ("giant_spider", "reach")] {
            let mut e = engine(
                9_622_000 + index as u64 * 10 + u64::from(label == "reach"),
                case.id,
            );
            let attacker = inject_creature_on_battlefield(&mut e, 0, attacker_id);
            let blocker = move_ready_to_battlefield(&mut e, 1, case.id);
            attack(&mut e, attacker);
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
