//! Actual-card scenarios for the queue-v33 fixed cost-reduction cohort.
//!
//! Expectations are printed Oracle values. CR 601.2f applies each static
//! reduction once; the filters are the printed color or the artifact card type.

use super::helpers::*;

struct Case {
    id: &'static str,
    spell: &'static str,
    nonmatch: &'static str,
    target: Option<usize>,
}

const CASES: [Case; 8] = [
    Case {
        id: "emerald_medallion",
        spell: "tread_upon",
        nonmatch: "firdoch_core",
        target: Some(0),
    },
    Case {
        id: "etherium_sculptor",
        spell: "firdoch_core",
        nonmatch: "strategic_planning",
        target: None,
    },
    Case {
        id: "foundry_inspector",
        spell: "firdoch_core",
        nonmatch: "strategic_planning",
        target: None,
    },
    Case {
        id: "enthusiastic_mechanaut",
        spell: "firdoch_core",
        nonmatch: "strategic_planning",
        target: None,
    },
    Case {
        id: "jet_medallion",
        spell: "instill_infection",
        nonmatch: "firdoch_core",
        target: Some(1),
    },
    Case {
        id: "pearl_medallion",
        spell: "ethereal_guidance",
        nonmatch: "firdoch_core",
        target: None,
    },
    Case {
        id: "ruby_medallion",
        spell: "breath_of_fire",
        nonmatch: "firdoch_core",
        target: Some(1),
    },
    Case {
        id: "sapphire_medallion",
        spell: "strategic_planning",
        nonmatch: "firdoch_core",
        target: None,
    },
];

#[track_caller]
fn hand_action_reduction(engine: &mut GameEngine, player: usize, card_id: &str) -> u32 {
    let hand_index = hand_index_for_card(engine, player, card_id) as u32;
    let batch = engine.initial_response_batch();
    batch.legal_by_player[&(player as i32)]
        .hand_actions
        .iter()
        .find(|action| action.hand_index == hand_index)
        .unwrap_or_else(|| panic!("missing cast action for {card_id}"))
        .generic_cost_reduction
}

fn engine_with_reducer(seed: u64, case: &Case, reducer_player: usize) -> GameEngine {
    let decks = Some(vec![
        deck_with("plains", &[case.id, case.spell, case.nonmatch]),
        deck_with("forest", &["grizzly_bears", case.spell]),
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
    inject_permanent_on_battlefield(&mut e, reducer_player, case.id);
    ensure_in_hand(&mut e, 0, case.spell);
    ensure_in_hand(&mut e, 0, case.nonmatch);
    ensure_in_hand(&mut e, 1, case.spell);
    if let Some(player) = case.target {
        inject_creature_on_battlefield(&mut e, player, "grizzly_bears");
    }
    e
}

fn engine(seed: u64, case: &Case) -> GameEngine {
    engine_with_reducer(seed, case, 0)
}

#[test]
fn q33_cost_reduction_canary_simplest() {
    let case = &CASES[1];
    let mut e = engine(33_060_001, case);
    assert_eq!(hand_action_reduction(&mut e, 0, case.spell), 1);
}

#[test]
fn q33_cost_reduction_canary_hardest() {
    let case = &CASES[0];
    let mut e = engine(33_060_002, case);
    assert_eq!(hand_action_reduction(&mut e, 0, case.spell), 1);
    assert_eq!(hand_action_reduction(&mut e, 0, case.nonmatch), 0);
}

#[test]
fn q33_cost_reduction_cards_reduce_only_their_printed_filter() {
    for (index, case) in CASES.iter().enumerate() {
        let mut e = engine(33_061_000 + index as u64, case);
        assert_eq!(
            hand_action_reduction(&mut e, 0, case.spell),
            1,
            "{}: matching spell",
            case.id
        );
        assert_eq!(
            hand_action_reduction(&mut e, 0, case.nonmatch),
            0,
            "{}: nonmatching spell",
            case.id
        );
    }
}

#[test]
fn q33_cost_reduction_cards_do_not_reduce_an_opponents_spells() {
    for (index, case) in CASES.iter().enumerate() {
        let mut e = engine_with_reducer(33_063_000 + index as u64, case, 1);
        assert_eq!(
            hand_action_reduction(&mut e, 0, case.spell),
            0,
            "{}: a reducer controlled by the opponent does not reduce this player's spell",
            case.id
        );
    }
}
