//! Actual-card scenarios for the reviewed colorless two-mana family.
//!
//! Expectations are printed Oracle values. CR 605.1a/605.3b make the targetless tap ability a
//! mana ability that adds exactly two colorless mana without using the stack; CR 302.6 stops a
//! summoning-sick creature from paying the tap cost.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;

struct Case {
    id: &'static str,
    creature: bool,
}

fn cases() -> [Case; 3] {
    [
        Case {
            id: "kozileks_channeler",
            creature: true,
        },
        Case {
            id: "palladium_myr",
            creature: true,
        },
        Case {
            id: "sisays_ring",
            creature: false,
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
    advance_to_main1_from_game_start(&mut e);
    e
}

#[test]
fn colorless_two_mana_batch_cards_add_exactly_two_colorless_without_the_stack() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(930_000 + index as u64, case.id);
        let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
        e.apply_scenario_command(0, &activate_ability_for(&e, source, 0, vec![]))
            .expect("activate the mana ability");
        assert_eq!(
            e.state.players[0].mana_pool.colorless, 2,
            "{}: exactly two colorless",
            case.id
        );
        assert!(e.state.objects[&source].tapped, "{}", case.id);
        assert!(e.state.stack.is_empty(), "{}: no stack use", case.id);
        assert_eq!(
            e.state.objects[&source].zone,
            Zone::Battlefield,
            "{}",
            case.id
        );
        assert!(
            e.apply_scenario_command(0, &activate_ability_for(&e, source, 0, vec![]))
                .is_err(),
            "{}: a tapped source cannot activate again",
            case.id
        );
        assert_eq!(
            e.state.players[0].mana_pool.colorless, 2,
            "{}: the rejected activation adds nothing",
            case.id
        );
    }
}

#[test]
fn colorless_two_mana_batch_creatures_respect_summoning_sickness() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(931_000 + index as u64, case.id);
        let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
        e.state
            .objects
            .get_mut(&source)
            .expect("source")
            .summoning_sick = case.creature;
        if case.creature {
            assert!(
                e.apply_scenario_command(0, &activate_ability_for(&e, source, 0, vec![]))
                    .is_err(),
                "{}: a summoning-sick creature cannot pay a tap cost",
                case.id
            );
            assert!(!e.state.objects[&source].tapped, "{}", case.id);
            e.state
                .objects
                .get_mut(&source)
                .expect("source")
                .summoning_sick = false;
        }
        e.apply_scenario_command(0, &activate_ability_for(&e, source, 0, vec![]))
            .expect("the source taps once legal");
        assert_eq!(e.state.players[0].mana_pool.colorless, 2, "{}", case.id);
    }
}
