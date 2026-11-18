//! Actual-card scenarios for the queue-v34 power-threshold evasion cohort.
//!
//! Expectations are printed Oracle values. CR 509.1b requires each declared
//! blocker to satisfy the attacker's restrictions using current power; the
//! pinned rulings confirm that a later power change does not undo a legal block.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_proto::ruled::v1::BlockPair;

struct Case {
    id: &'static str,
    threshold: u32,
}

fn cases() -> [Case; 4] {
    [
        Case {
            id: "amrou_kithkin",
            threshold: 3,
        },
        Case {
            id: "duskmantle_operative",
            threshold: 4,
        },
        Case {
            id: "fleet-footed_monk",
            threshold: 2,
        },
        Case {
            id: "goldmeadow_dodger",
            threshold: 4,
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
    advance_to_declare_attackers(&mut e);
    e
}

fn setup_attack(seed: u64, case: &Case, blocker_powers: &[u32]) -> (GameEngine, u32, Vec<u32>) {
    let mut e = engine(seed, case.id);
    let attacker = move_ready_to_battlefield(&mut e, 0, case.id);
    let blockers = blocker_powers
        .iter()
        .map(|power| inject_creature_with_stats(&mut e, 1, "grizzly_bears", *power, 4))
        .collect::<Vec<_>>();
    e.apply_scenario_command(0, &declare_attackers(vec![attacker]))
        .expect("declare the attacker");
    pass_both_players(&mut e);
    assert_eq!(
        e.state.turn_step,
        tricerules_core::TurnStep::DeclareBlockers
    );
    (e, attacker, blockers)
}

#[test]
fn q34_power_evasion_canary_simplest() {
    let case = &cases()[2];
    let (mut e, attacker, blockers) = setup_attack(34_080_001, case, &[1, 2]);
    let pairs = &e.initial_response_batch().legal_by_player[&1].legal_block_pairs;
    assert!(
        pairs
            .iter()
            .any(|pair| pair.attacker_id == attacker && pair.blocker_id == blockers[0]),
        "a 1-power creature may block Fleet-Footed Monk"
    );
    assert!(
        !pairs
            .iter()
            .any(|pair| pair.attacker_id == attacker && pair.blocker_id == blockers[1]),
        "a 2-power creature may not block Fleet-Footed Monk"
    );
}

#[test]
fn q34_power_evasion_canary_hardest() {
    let case = &cases()[3];
    let (mut e, attacker, blockers) = setup_attack(34_080_002, case, &[3, 4]);
    let pairs = &e.initial_response_batch().legal_by_player[&1].legal_block_pairs;
    assert!(
        pairs
            .iter()
            .any(|pair| pair.attacker_id == attacker && pair.blocker_id == blockers[0]),
        "a 3-power creature may block Goldmeadow Dodger"
    );
    assert!(
        !pairs
            .iter()
            .any(|pair| pair.attacker_id == attacker && pair.blocker_id == blockers[1]),
        "a 4-power creature may not block Goldmeadow Dodger"
    );
}

#[test]
fn q34_power_evasion_cards_allow_only_blockers_below_the_printed_threshold() {
    for (index, case) in cases().iter().enumerate() {
        let powers = [case.threshold - 1, case.threshold, case.threshold + 1];
        let (mut e, attacker, blockers) = setup_attack(34_081_000 + index as u64, case, &powers);
        let pairs = &e.initial_response_batch().legal_by_player[&1].legal_block_pairs;
        assert!(
            pairs
                .iter()
                .any(|pair| pair.attacker_id == attacker && pair.blocker_id == blockers[0]),
            "{}: power {} is below the threshold",
            case.id,
            powers[0]
        );
        for (blocker, power) in [(blockers[1], powers[1]), (blockers[2], powers[2])] {
            assert!(
                !pairs
                    .iter()
                    .any(|pair| pair.attacker_id == attacker && pair.blocker_id == blocker),
                "{}: power {} meets or exceeds the threshold",
                case.id,
                power
            );
        }
    }
}

#[test]
fn q34_power_evasion_legal_block_persists_when_the_blocker_grows() {
    let case = &cases()[0];
    let (mut e, attacker, blockers) = setup_attack(34_082_000, case, &[2]);
    let blocker = blockers[0];
    e.apply_scenario_command(
        1,
        &declare_blockers(vec![BlockPair {
            attacker_id: attacker,
            blocker_id: blocker,
        }]),
    )
    .expect("the 2-power creature legally blocks Amrou Kithkin");
    assert!(
        e.state.combat.as_ref().unwrap().blockers[&attacker].contains(&blocker),
        "the block is recorded"
    );
    inject_card_into_hand(&mut e, 0, "giant_growth");
    give_mana(
        &mut e,
        0,
        ManaGift {
            g: 1,
            ..Default::default()
        },
    );
    let growth = hand_index_for_card(&e, 0, "giant_growth");
    e.apply_scenario_command(0, &cast_spell(growth, target_object(blocker)))
        .expect("pump the blocker above the threshold");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.effective_power(blocker), Some(5), "the blocker grew");
    assert!(
        e.state.combat.as_ref().unwrap().blockers[&attacker].contains(&blocker),
        "a power change after the block does not undo it"
    );
}
