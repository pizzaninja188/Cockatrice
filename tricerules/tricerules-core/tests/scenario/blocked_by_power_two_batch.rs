//! Actual-card scenarios for the queue-v27 "can't be blocked by creatures with
//! power 2 or less" cohort.
//!
//! Expectations are printed Oracle values. CR 509.1b requires every declared
//! blocker to satisfy the attacker's restrictions using current characteristics;
//! the pinned rulings confirm that changing a blocker's power after a legal block
//! does not undo that block.
use super::helpers::*;
use crate::helpers::ScenarioCommandExt;
use tricerules_cards::{ContinuousEffectKind, EffectDuration};
use tricerules_core::{AffectedScope, ContinuousEffect};

struct Case {
    id: &'static str,
    body: (u32, u32),
}

fn cases() -> [Case; 5] {
    [
        Case {
            id: "arlinns_wolf",
            body: (3, 2),
        },
        Case {
            id: "enraged_ceratok",
            body: (4, 4),
        },
        Case {
            id: "giltgrove_stalker",
            body: (2, 1),
        },
        Case {
            id: "outland_boar",
            body: (4, 4),
        },
        Case {
            id: "steel_leaf_champion",
            body: (5, 4),
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
    let decks = Some(vec![
        deck_with("forest", &[card]),
        deck_with("forest", &["hill_giant"]),
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

fn cast_case(e: &mut GameEngine, case: &Case) -> u32 {
    move_ready_to_battlefield(e, 0, case.id);
    battlefield_object_for_card(e, 0, case.id)
}

#[test]
fn blocked_by_power_two_batch_cards_reject_weak_blockers_and_allow_power_three() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(27_010_000 + index as u64, case.id);
        let attacker = cast_case(&mut e, &case);
        assert_eq!(
            (e.effective_power(attacker), e.effective_toughness(attacker)),
            (Some(case.body.0), Some(case.body.1)),
            "{}: printed body",
            case.id
        );
        let weak = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        let strong = move_ready_to_battlefield(&mut e, 1, "hill_giant");
        assert_eq!(
            e.effective_power(weak),
            Some(2),
            "{}: weak blocker",
            case.id
        );
        assert_eq!(
            e.effective_power(strong),
            Some(3),
            "{}: strong blocker",
            case.id
        );
        advance_main1_to_declare_attackers(&mut e);
        e.apply_scenario_command(0, &declare_attackers(vec![attacker]))
            .expect("declare attacker");
        e.apply_scenario_command(0, &pass())
            .expect("active player passes");
        e.apply_scenario_command(1, &pass())
            .expect("defender passes");

        assert_eq!(
            e.state.turn_step,
            tricerules_core::TurnStep::DeclareBlockers,
            "{}: block step",
            case.id
        );
        {
            let combat = e.state.combat.as_ref().expect("combat");
            assert!(!combat.blockers_declared, "{}: not auto-declared", case.id);
            assert!(
                combat.attacking.contains(&attacker),
                "{}: attacker present",
                case.id
            );
        }
        assert!(
            !e.state.objects[&strong].tapped,
            "{}: strong untapped",
            case.id
        );
        let pairs = &e.initial_response_batch().legal_by_player[&1].legal_block_pairs;
        assert!(!pairs.is_empty(), "{}: legal pairs exist", case.id);
        assert!(
            !pairs
                .iter()
                .any(|pair| pair.attacker_id == attacker && pair.blocker_id == weak),
            "{}: a 2-power creature cannot block",
            case.id
        );
        assert!(
            pairs
                .iter()
                .any(|pair| pair.attacker_id == attacker && pair.blocker_id == strong),
            "{}: a 3-power creature can block",
            case.id
        );
    }
}

#[test]
fn blocked_by_power_two_batch_cards_do_not_undo_a_legal_block_after_a_power_change() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(27_011_000 + index as u64, case.id);
        let attacker = cast_case(&mut e, &case);
        let blocker = move_ready_to_battlefield(&mut e, 1, "hill_giant");
        advance_main1_to_declare_attackers(&mut e);
        e.apply_scenario_command(0, &declare_attackers(vec![attacker]))
            .expect("declare attacker");
        e.apply_scenario_command(0, &pass())
            .expect("active player passes");
        e.apply_scenario_command(1, &pass())
            .expect("defender passes");
        e.apply_scenario_command(
            1,
            &declare_blockers(vec![BlockPair {
                attacker_id: attacker,
                blocker_id: blocker,
            }]),
        )
        .expect("a 3-power creature may block");
        assert_eq!(
            e.state.combat.as_ref().expect("combat").blockers[&attacker],
            vec![blocker],
            "{}: the block is recorded",
            case.id
        );
        e.state.continuous_effects.push(ContinuousEffect {
            trigger_grant_origin: None,
            source_id: None,
            affected: AffectedScope::Single(blocker),
            kind: ContinuousEffectKind::Layer7bSetPt {
                power: 2,
                toughness: 3,
            },
            condition: None,
            duration: EffectDuration::UntilEndOfTurn,
            timestamp: e.state.command_index,
        });
        assert_eq!(e.effective_power(blocker), Some(2), "{}", case.id);
        assert_eq!(
            e.state.combat.as_ref().expect("combat").blockers[&attacker],
            vec![blocker],
            "{}: the completed block is not undone by the later power change",
            case.id
        );
    }
}
