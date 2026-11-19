//! Actual-card scenarios for the queue-v42 combat-damage +1/+1 counter cohort.
//!
//! Expectations are printed Oracle values. CR 510.1b/510.2 assign and deal combat damage to
//! the player only when the attacker is unblocked; CR 603.2 triggers on that event and CR 122.1
//! makes the +1/+1 counter a permanent counter.
use super::helpers::*;
use tricerules_cards::primitives::{CounterKind, Keyword};

struct Case {
    id: &'static str,
    body: (u32, u32),
    keyword: Option<Keyword>,
    line: u16,
}

fn cases() -> [Case; 5] {
    [
        Case {
            id: "stromkirk_patrol",
            body: (4, 3),
            keyword: None,
            line: 1,
        },
        Case {
            id: "erdwal_ripper",
            body: (2, 1),
            keyword: Some(Keyword::Haste),
            line: 2,
        },
        Case {
            id: "slith_ascendant",
            body: (1, 1),
            keyword: Some(Keyword::Flying),
            line: 2,
        },
        Case {
            id: "slith_firewalker",
            body: (1, 1),
            keyword: Some(Keyword::Haste),
            line: 2,
        },
        Case {
            id: "slith_predator",
            body: (1, 1),
            keyword: Some(Keyword::Trample),
            line: 2,
        },
    ]
}

fn engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![
        deck_with("mountain", &[card]),
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

fn enter_combat(e: &mut GameEngine) {
    e.apply_scenario_command(0, &primitive_yield())
        .expect("main1 to begin combat");
    e.apply_scenario_command(0, &pass())
        .expect("ap pass begin combat");
    e.apply_scenario_command(1, &pass())
        .expect("nap pass begin combat");
}

#[test]
fn q42_combat_damage_counter_canary_simplest_adds_one_counter() {
    let case = cases()
        .into_iter()
        .find(|case| case.id == "stromkirk_patrol")
        .expect("Stromkirk Patrol case");
    let mut e = engine(42_060_001, case.id);
    let attacker = move_ready_to_battlefield(&mut e, 0, case.id);
    enter_combat(&mut e);
    e.apply_scenario_command(0, &declare_attackers(vec![attacker]))
        .expect("declare the attacker");
    pass_both_players(&mut e);
    pass_both_players(&mut e);
    assert_eq!(e.state.players[1].life, 20 - case.body.0 as i32);
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.state.objects[&attacker].counter_count(CounterKind::PlusOnePlusOne),
        1,
        "one +1/+1 counter"
    );
    assert_eq!(e.effective_power(attacker), Some(case.body.0 + 1));
    assert_eq!(e.effective_toughness(attacker), Some(case.body.1 + 1));
}

#[test]
fn q42_combat_damage_counter_canary_hardest_does_not_trigger_when_blocked() {
    let case = cases()
        .into_iter()
        .find(|case| case.id == "slith_predator")
        .expect("Slith Predator case");
    let mut e = engine(42_060_002, case.id);
    let attacker = move_ready_to_battlefield(&mut e, 0, case.id);
    let blocker = move_ready_to_battlefield(&mut e, 1, "grizzly_bears");
    enter_combat(&mut e);
    e.apply_scenario_command(0, &declare_attackers(vec![attacker]))
        .expect("declare the attacker");
    pass_both_players(&mut e);
    e.apply_scenario_command(
        1,
        &declare_blockers(vec![BlockPair {
            attacker_id: attacker,
            blocker_id: blocker,
        }]),
    )
    .expect("declare the blocker");
    pass_both_players(&mut e);
    assert_eq!(e.state.players[1].life, 20, "no damage to the player");
    assert!(
        e.state.stack.is_empty(),
        "no combat-damage-to-player trigger fires"
    );
    assert_eq!(
        e.state.objects[&attacker].counter_count(CounterKind::PlusOnePlusOne),
        0
    );
}

#[test]
fn q42_combat_damage_counter_cards_add_one_counter_and_publish_their_keyword() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(42_061_000 + index as u64, case.id);
        let attacker = move_ready_to_battlefield(&mut e, 0, case.id);
        assert_eq!(
            (e.effective_power(attacker), e.effective_toughness(attacker)),
            (Some(case.body.0), Some(case.body.1)),
            "{}: printed body",
            case.id
        );
        if let Some(keyword) = case.keyword {
            assert!(
                e.effective_has_keyword(attacker, keyword),
                "{}: printed keyword",
                case.id
            );
        }
        enter_combat(&mut e);
        e.apply_scenario_command(0, &declare_attackers(vec![attacker]))
            .expect("declare the attacker");
        pass_both_players(&mut e);
        pass_both_players(&mut e);
        assert_eq!(
            e.state.players[1].life,
            20 - case.body.0 as i32,
            "{}: the unblocked attacker deals its printed power",
            case.id
        );
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.objects[&attacker].counter_count(CounterKind::PlusOnePlusOne),
            1,
            "{}: exactly one counter",
            case.id
        );
        assert_eq!(
            e.effective_power(attacker),
            Some(case.body.0 + 1),
            "{}: the counter pumps power",
            case.id
        );
        assert_eq!(
            e.effective_toughness(attacker),
            Some(case.body.1 + 1),
            "{}: the counter pumps toughness",
            case.id
        );
    }
}

#[test]
fn q42_combat_damage_counter_cards_use_the_combat_damage_trigger() {
    for case in cases() {
        let definition = tricerules_cards::registry::global()
            .get(case.id)
            .unwrap_or_else(|| panic!("missing {}", case.id));
        let face = definition.primary_face();
        assert_eq!(face.triggered_abilities.len(), 1, "{}", case.id);
        assert_eq!(
            face.triggered_abilities[0].trigger,
            tricerules_cards::primitives::TriggerCondition::WheneverSelfDealsCombatDamageToPlayer,
            "{}",
            case.id
        );
        assert_eq!(
            face.triggered_abilities[0].presentation,
            tricerules_cards::AbilityPresentation::OracleLines(vec![case.line]),
            "{}",
            case.id
        );
    }
}
