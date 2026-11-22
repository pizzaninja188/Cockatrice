//! Actual-card scenarios for the reviewed "sacrifice a creature" self-pump cohort.
//!
//! Expectations are printed Oracle values. CR 602.1a/602.2 define the activated
//! ability and its cost; CR 701.21a governs sacrifice as a cost; CR 611.2a
//! applies the pump until end of turn and CR 514.2 ends it during cleanup.
//! The pinned rulings confirm that the source may sacrifice itself but is then
//! not on the battlefield to receive the bonus, and that a blocked attacker
//! sacrificed during declare blockers deals no combat damage.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;

struct Case {
    id: &'static str,
    power: u32,
    toughness: u32,
}

const CASES: [Case; 4] = [
    Case {
        id: "bloodthrone_vampire",
        power: 1,
        toughness: 1,
    },
    Case {
        id: "nantuko_husk",
        power: 2,
        toughness: 2,
    },
    Case {
        id: "phyrexian_ghoul",
        power: 2,
        toughness: 2,
    },
    Case {
        id: "vampire_aristocrat",
        power: 2,
        toughness: 2,
    },
];

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("forest", &[]), deck_with("island", &[])]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn source_on_battlefield(engine: &mut GameEngine, id: &str) -> u32 {
    inject_card_into_hand(engine, 0, id);
    relocate_to_battlefield(engine, 0, id, false)
}

fn activate_sacrifice(source: u32, sacrificed: u32) -> RuledCommand {
    activate_ability_with_costs(
        source,
        0,
        vec![],
        vec![permanent_cost_selection(0, sacrificed)],
    )
}

#[test]
fn sac_creature_self_pump_batch_cards_sacrifice_another_creature_and_pump() {
    for (index, case) in CASES.iter().enumerate() {
        let mut engine = engine(25_060_000 + index as u64);
        let source = source_on_battlefield(&mut engine, case.id);
        assert_eq!(
            (
                engine.effective_power(source),
                engine.effective_toughness(source)
            ),
            (Some(case.power), Some(case.toughness)),
            "{} printed body",
            case.id
        );
        let fodder = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");

        engine
            .apply_scenario_command(0, &activate_sacrifice(source, fodder))
            .unwrap_or_else(|error| panic!("{}: {error:?}", case.id));

        assert_eq!(
            engine.state.objects[&fodder].zone,
            Zone::Graveyard,
            "{}",
            case.id
        );
        assert!(
            engine.state.players[0].graveyard.contains(&fodder),
            "{}",
            case.id
        );
        assert_eq!(
            engine.state.objects[&source].zone,
            Zone::Battlefield,
            "{}",
            case.id
        );
        assert!(!engine.state.objects[&source].tapped, "{}", case.id);
        assert_eq!(engine.state.stack.len(), 1, "{}", case.id);

        resolve_entire_stack_two_player(&mut engine);

        assert_eq!(
            (
                engine.effective_power(source),
                engine.effective_toughness(source)
            ),
            (Some(case.power + 2), Some(case.toughness + 2)),
            "{} pump applied",
            case.id
        );

        end_active_turn(&mut engine, 0);
        assert_eq!(
            (
                engine.effective_power(source),
                engine.effective_toughness(source)
            ),
            (Some(case.power), Some(case.toughness)),
            "{} pump expires at cleanup",
            case.id
        );
    }
}

#[test]
fn sac_creature_self_pump_batch_cards_can_sacrifice_themselves_without_a_bonus() {
    for (index, case) in CASES.iter().enumerate() {
        let mut engine = engine(25_061_000 + index as u64);
        let source = source_on_battlefield(&mut engine, case.id);
        let generation_before = engine
            .state
            .zone_change_generation
            .get(&source)
            .copied()
            .unwrap_or_default();

        engine
            .apply_scenario_command(0, &activate_sacrifice(source, source))
            .unwrap_or_else(|error| panic!("{}: {error:?}", case.id));

        assert_eq!(
            engine.state.objects[&source].zone,
            Zone::Graveyard,
            "{}",
            case.id
        );
        assert!(
            engine.state.players[0].graveyard.contains(&source),
            "{}",
            case.id
        );
        assert_eq!(
            engine
                .state
                .zone_change_generation
                .get(&source)
                .copied()
                .unwrap_or_default(),
            generation_before + 1,
            "{}",
            case.id
        );
        assert_eq!(engine.state.stack.len(), 1, "{}", case.id);

        resolve_entire_stack_two_player(&mut engine);
        assert!(engine.state.stack.is_empty(), "{}", case.id);
        assert_eq!(
            (
                engine.effective_power(source),
                engine.effective_toughness(source)
            ),
            (Some(case.power), Some(case.toughness)),
            "{}: the sacrificed source receives no bonus",
            case.id
        );
    }
}

#[test]
fn sac_creature_self_pump_batch_cards_need_no_tap_and_ignore_summoning_sickness() {
    for (index, case) in CASES.iter().enumerate() {
        let mut engine = engine(25_062_000 + index as u64);
        let source = source_on_battlefield(&mut engine, case.id);
        engine
            .state
            .objects
            .get_mut(&source)
            .expect("source")
            .summoning_sick = true;
        let fodder = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");

        engine
            .apply_scenario_command(0, &activate_sacrifice(source, fodder))
            .unwrap_or_else(|error| panic!("{}: {error:?}", case.id));

        assert_eq!(
            engine.state.objects[&source].zone,
            Zone::Battlefield,
            "{}",
            case.id
        );
        assert!(!engine.state.objects[&source].tapped, "{}", case.id);
        assert_eq!(engine.state.stack.len(), 1, "{}", case.id);
    }
}
