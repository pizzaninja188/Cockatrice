//! Actual-card scenarios for the queue-v30 "creatures without flying can't block" cohort.
//!
//! Expectations are printed Oracle values. CR 509.1b applies the restriction while
//! declaring blockers; CR 611.2a/514.2 limits it to the current turn. The effect is
//! untargeted: every current nonflier is restricted, including creatures that enter later.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;

struct Case {
    id: &'static str,
    mana: ManaGift,
}

fn cases() -> [Case; 3] {
    [
        Case {
            id: "magmatic_chasm",
            mana: ManaGift {
                r: 1,
                c: 1,
                ..Default::default()
            },
        },
        Case {
            id: "seismic_stomp",
            mana: ManaGift {
                r: 1,
                c: 1,
                ..Default::default()
            },
        },
        Case {
            id: "falter",
            mana: ManaGift {
                r: 1,
                c: 1,
                ..Default::default()
            },
        },
    ]
}

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("mountain", &[]), deck_with("forest", &[])]);
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

fn cast(e: &mut GameEngine, case: &Case) {
    inject_card_into_hand(e, 0, case.id);
    give_mana(
        e,
        0,
        ManaGift {
            r: case.mana.r,
            c: case.mana.c,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(e, 0, case.id);
    e.apply_scenario_command(0, &cast_spell(slot, vec![]))
        .unwrap_or_else(|error| panic!("{}: {error:?}", case.id));
    resolve_entire_stack_two_player(e);
}

#[test]
fn q30_no_flying_block_canary_simplest() {
    let case = &cases()[0];
    let mut e = engine(30_060_001);
    cast(&mut e, case);
    let attacker = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let flier = inject_creature_on_battlefield(&mut e, 1, "windreaper_falcon");
    let nonflier = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    e.apply_scenario_command(0, &primitive_yield())
        .expect("to combat");
    pass_both_players(&mut e);
    e.apply_scenario_command(0, &declare_attackers(vec![attacker]))
        .expect("attack");
    pass_both_players(&mut e);
    let legal = &e.initial_response_batch().legal_by_player[&1];
    assert!(legal
        .legal_block_pairs
        .iter()
        .any(|pair| pair.attacker_id == attacker && pair.blocker_id == flier));
    assert!(!legal
        .legal_block_pairs
        .iter()
        .any(|pair| pair.attacker_id == attacker && pair.blocker_id == nonflier));
}

#[test]
fn q30_no_flying_block_canary_hardest() {
    let case = &cases()[2];
    let mut e = engine(30_060_002);
    cast(&mut e, case);
    let attacker = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let nonflier = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    e.apply_scenario_command(0, &primitive_yield())
        .expect("to combat");
    pass_both_players(&mut e);
    e.apply_scenario_command(0, &declare_attackers(vec![attacker]))
        .expect("attack");
    pass_both_players(&mut e);
    let legal = &e.initial_response_batch().legal_by_player[&1];
    assert!(!legal
        .legal_block_pairs
        .iter()
        .any(|pair| pair.attacker_id == attacker && pair.blocker_id == nonflier));
}

#[test]
fn q30_no_flying_block_cards_cast_with_exact_cost_and_body() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(30_061_000 + index as u64);
        cast(&mut e, &case);
        let pool = &e.state.players[0].mana_pool;
        assert_eq!(
            (
                pool.white,
                pool.blue,
                pool.black,
                pool.red,
                pool.green,
                pool.colorless
            ),
            (0, 0, 0, 0, 0, 0),
            "{}: exact printed cost",
            case.id
        );
        assert!(e.state.stack.is_empty(), "{}", case.id);
        assert_eq!(e.state.players[0].graveyard.len(), 1, "{}", case.id);
    }
}

#[test]
fn q30_no_flying_block_cards_restrict_only_nonfliers() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(30_062_000 + index as u64);
        cast(&mut e, &case);
        let attacker = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        let flier = inject_creature_on_battlefield(&mut e, 1, "windreaper_falcon");
        let nonflier = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        e.apply_scenario_command(0, &primitive_yield())
            .expect("to combat");
        pass_both_players(&mut e);
        e.apply_scenario_command(0, &declare_attackers(vec![attacker]))
            .expect("attack");
        pass_both_players(&mut e);
        let legal = &e.initial_response_batch().legal_by_player[&1];
        assert!(
            legal
                .legal_block_pairs
                .iter()
                .any(|pair| pair.attacker_id == attacker && pair.blocker_id == flier),
            "{}: a flier may block",
            case.id
        );
        assert!(
            !legal
                .legal_block_pairs
                .iter()
                .any(|pair| pair.attacker_id == attacker && pair.blocker_id == nonflier),
            "{}: a nonflier may not block",
            case.id
        );
    }
}
