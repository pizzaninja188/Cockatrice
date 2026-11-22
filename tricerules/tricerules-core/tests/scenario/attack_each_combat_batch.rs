//! Actual-card scenarios for the reviewed `attacks each combat if able` family.
//!
//! Expectations are printed Oracle values. CR 508.1d requires an able
//! must-attack creature to be declared as an attacker; the requirement does not
//! apply while the creature is summoning sick (CR 302.6) or otherwise unable.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;

struct Case {
    id: &'static str,
    power: u32,
    toughness: u32,
}

fn cases() -> [Case; 5] {
    [
        Case {
            id: "tattermunge_maniac",
            power: 2,
            toughness: 1,
        },
        Case {
            id: "bloodrock_cyclops",
            power: 3,
            toughness: 3,
        },
        Case {
            id: "frontline_rebel",
            power: 3,
            toughness: 3,
        },
        Case {
            id: "berserkers_of_blood_ridge",
            power: 4,
            toughness: 4,
        },
        Case {
            id: "rubblebelt_recluse",
            power: 6,
            toughness: 5,
        },
    ]
}

fn engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![
        deck_with("mountain", &[card]),
        deck_with("forest", &[]),
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
    advance_to_declare_attackers(&mut e);
    e
}

#[test]
fn attack_each_combat_batch_cards_apply_the_printed_effect() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(8_030_000 + index as u64, case.id);
        let source = move_ready_to_battlefield(&mut e, 0, case.id);
        assert_eq!(e.effective_power(source), Some(case.power), "{}", case.id);
        assert_eq!(
            e.effective_toughness(source),
            Some(case.toughness),
            "{}",
            case.id
        );
        assert!(
            e.state.objects[&source].must_attack_if_able,
            "{}: the printed must-attack characteristic is loaded",
            case.id
        );
        e.apply_scenario_command(0, &declare_attackers(vec![source]))
            .expect("declare the must-attack creature");
        assert!(
            e.state
                .combat
                .as_ref()
                .expect("combat")
                .attacking
                .contains(&source),
            "{}: the creature is attacking",
            case.id
        );
    }
}

#[test]
fn attack_each_combat_batch_cards_reject_omitting_an_able_attacker() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(8_031_000 + index as u64, case.id);
        let source = move_ready_to_battlefield(&mut e, 0, case.id);
        assert!(e.state.objects[&source].must_attack_if_able, "{}", case.id);
        assert!(
            e.apply_scenario_command(0, &declare_attackers(vec![]))
                .is_err(),
            "{}: omitting an able must-attack creature is illegal",
            case.id
        );
        assert!(
            e.state
                .combat
                .as_ref()
                .is_none_or(|combat| !combat.attacking.contains(&source)),
            "{}: the rejected declaration leaves no attacker",
            case.id
        );
        e.apply_scenario_command(0, &declare_attackers(vec![source]))
            .expect("the legal declaration is accepted");
    }
}

#[test]
fn attack_each_combat_batch_cards_allow_skip_when_unable() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(8_032_000 + index as u64, case.id);
        let source = move_ready_to_battlefield(&mut e, 0, case.id);
        e.state
            .objects
            .get_mut(&source)
            .expect("source")
            .summoning_sick = true;
        e.apply_scenario_command(0, &declare_attackers(vec![]))
            .expect("a summoning-sick must-attack creature may be skipped");
        assert_eq!(
            e.state.turn_step,
            tricerules_core::TurnStep::EndCombat,
            "{}",
            case.id
        );
    }
}
