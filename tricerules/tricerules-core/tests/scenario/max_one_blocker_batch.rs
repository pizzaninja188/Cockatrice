//! Actual-card scenarios for the queue-v27 "can't be blocked by more than one
//! creature" cohort.
//!
//! Expectations are printed Oracle values. CR 509.1b makes a declaration with two
//! blockers illegal, while a single blocker remains legal; the rejected declaration
//! leaves combat state untouched.
use super::helpers::*;
use crate::helpers::ScenarioCommandExt;

struct Case {
    id: &'static str,
}

fn cases() -> [Case; 5] {
    [
        Case {
            id: "bristling_boar",
        },
        Case {
            id: "charging_rhino",
        },
        Case { id: "ironhoof_ox" },
        Case {
            id: "norwood_riders",
        },
        Case {
            id: "stalking_tiger",
        },
    ]
}

fn advance_main1_to_declare_attackers(e: &mut GameEngine) {
    e.apply_scenario_command(0, &primitive_yield())
        .expect("main phase to beginning of combat");
    e.apply_scenario_command(0, &pass())
        .expect("active player passes in beginning of combat");
    e.apply_scenario_command(1, &pass())
        .expect("defender passes in beginning of combat");
    assert_eq!(
        e.state.turn_step,
        tricerules_core::TurnStep::DeclareAttackers
    );
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
    advance_to_main1_from_game_start(&mut e);
    e
}

fn reach_blockers(e: &mut GameEngine, case: &Case) -> u32 {
    let attacker = move_ready_to_battlefield(e, 0, case.id);
    advance_main1_to_declare_attackers(e);
    e.apply_scenario_command(0, &declare_attackers(vec![attacker]))
        .expect("declare attacker");
    e.apply_scenario_command(0, &pass())
        .expect("active player passes after declaring attackers");
    e.apply_scenario_command(1, &pass())
        .expect("defender passes after attackers are declared");
    assert_eq!(
        e.state.turn_step,
        tricerules_core::TurnStep::DeclareBlockers,
        "{}: block step",
        case.id
    );
    attacker
}

#[test]
fn max_one_blocker_batch_cards_reject_two_blockers_and_accept_one() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(27_020_000 + index as u64, case.id);
        let first = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        let second = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        let attacker = reach_blockers(&mut e, &case);
        let before = e.state.command_index;
        let error = e
            .apply_scenario_command(
                1,
                &declare_blockers(vec![
                    BlockPair {
                        attacker_id: attacker,
                        blocker_id: first,
                    },
                    BlockPair {
                        attacker_id: attacker,
                        blocker_id: second,
                    },
                ]),
            )
            .expect_err("two blockers cannot be declared");
        assert!(matches!(error, tricerules_core::EngineError::Illegal(_)));
        assert_eq!(
            e.state.command_index, before,
            "{}: a rejected declaration changes nothing",
            case.id
        );
        assert!(
            e.state.combat.as_ref().expect("combat").blockers.is_empty(),
            "{}: no partial block is recorded",
            case.id
        );
        let mut accepted = engine(27_020_500 + index as u64, case.id);
        let accepted_blocker = inject_creature_on_battlefield(&mut accepted, 1, "grizzly_bears");
        let accepted_attacker = reach_blockers(&mut accepted, &case);
        assert_eq!(
            accepted.state.turn_step,
            tricerules_core::TurnStep::DeclareBlockers,
            "{}: block step",
            case.id
        );
        assert!(
            accepted.state.is_defending_player(1),
            "{}: defender",
            case.id
        );
        assert_eq!(
            accepted.state.priority_player_id(),
            1,
            "{}: priority",
            case.id
        );
        accepted
            .apply_scenario_command(
                1,
                &declare_blockers(vec![BlockPair {
                    attacker_id: accepted_attacker,
                    blocker_id: accepted_blocker,
                }]),
            )
            .unwrap_or_else(|error| panic!("{}: one blocker is legal: {error:?}", case.id));
        assert_eq!(
            accepted.state.combat.as_ref().expect("combat").blockers[&accepted_attacker],
            vec![accepted_blocker],
            "{}: exactly one blocker",
            case.id
        );
    }
}

#[test]
fn max_one_blocker_batch_cards_are_engine_authoritative_in_public_annotations() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(27_021_000 + index as u64, case.id);
        let attacker = reach_blockers(&mut e, &case);
        assert_eq!(
            zone_view_rules_annotation_labels(&mut e, 0, attacker),
            vec!["Can't be blocked by more than 1 creature"],
            "{}: the restriction is visible before blockers",
            case.id
        );
    }
}
