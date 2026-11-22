//! Actual-card scenarios for the queue-v28 "sacrifice an artifact: pump this creature"
//! cohort.
//!
//! Expectations are printed Oracle values. CR 118.3/701.21a sacrifice the artifact as an
//! activation cost; CR 602.1/602.2 activate the ability; CR 611.2a/613.4c apply the pump
//! until end of turn; CR 514.2 restores the printed body.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_cards::Keyword;
use tricerules_core::{TurnStep, Zone};

struct Case {
    id: &'static str,
    power: u32,
    toughness: u32,
    pump: (i32, i32),
}

fn cases() -> [Case; 3] {
    [
        Case {
            id: "atog",
            power: 1,
            toughness: 2,
            pump: (2, 2),
        },
        Case {
            id: "ravenous_intruder",
            power: 1,
            toughness: 2,
            pump: (2, 2),
        },
        Case {
            id: "rust_monster",
            power: 2,
            toughness: 1,
            pump: (2, 0),
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
    advance_to_main1_from_game_start(&mut e);
    e
}

fn activate_sacrifice(e: &mut GameEngine, card: &Case, source: u32, artifact: u32) {
    e.apply_scenario_command(
        0,
        &activate_ability_with_costs(
            source,
            0,
            vec![],
            vec![permanent_cost_selection(0, artifact)],
        ),
    )
    .unwrap_or_else(|error| panic!("activate {}: {error:?}", card.id));
    resolve_entire_stack_two_player(e);
}

fn advance_to_next_upkeep(e: &mut GameEngine) {
    let starting_turn = e.state.turn_instance;
    for _ in 0..240 {
        if e.state.turn_instance > starting_turn && e.state.turn_step == TurnStep::Upkeep {
            return;
        }
        resolve_cleanup_discards_if_any(e);
        let priority = e.state.priority_player_id();
        e.apply_scenario_command(priority, &pass())
            .expect("advance to the next upkeep");
    }
    panic!("turn advancement stalled before the next upkeep");
}

#[test]
fn sac_artifact_pump_batch_cards_sacrifice_the_artifact_and_pump_the_source() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(28_090_000 + index as u64, case.id);
        let source = inject_creature_on_battlefield(&mut e, 0, case.id);
        let object = e.state.objects.get_mut(&source).expect("source");
        object.power = None;
        object.toughness = None;
        let artifact = inject_permanent_on_battlefield(&mut e, 0, "swiftfoot_boots");
        assert_eq!(
            (e.effective_power(source), e.effective_toughness(source)),
            (Some(case.power), Some(case.toughness)),
            "{}: printed body",
            case.id
        );
        activate_sacrifice(&mut e, &case, source, artifact);
        assert_eq!(
            e.state.objects[&artifact].zone,
            Zone::Graveyard,
            "{}: the artifact was sacrificed",
            case.id
        );
        assert_eq!(
            (e.effective_power(source), e.effective_toughness(source)),
            (
                Some((case.power as i32 + case.pump.0) as u32),
                Some((case.toughness as i32 + case.pump.1) as u32)
            ),
            "{}: the pump",
            case.id
        );
        if case.id == "rust_monster" {
            assert!(
                e.effective_has_keyword(source, Keyword::FirstStrike),
                "{}: first strike is retained",
                case.id
            );
        }

        advance_to_next_upkeep(&mut e);
        assert_eq!(
            (e.effective_power(source), e.effective_toughness(source)),
            (Some(case.power), Some(case.toughness)),
            "{}: cleanup ends the pump",
            case.id
        );
    }
}

#[test]
fn sac_artifact_pump_batch_cards_require_an_artifact_they_control() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(28_091_000 + index as u64, case.id);
        let source = inject_creature_on_battlefield(&mut e, 0, case.id);
        let object = e.state.objects.get_mut(&source).expect("source");
        object.power = None;
        object.toughness = None;
        assert!(
            e.apply_scenario_command(0, &activate_ability(source, 0, vec![]))
                .is_err(),
            "{}: no artifact is available to sacrifice",
            case.id
        );
        assert_eq!(
            (e.effective_power(source), e.effective_toughness(source)),
            (Some(case.power), Some(case.toughness)),
            "{}: the rejected activation changes nothing",
            case.id
        );
    }
}
