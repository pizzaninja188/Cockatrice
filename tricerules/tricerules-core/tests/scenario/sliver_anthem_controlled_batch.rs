//! Actual-card scenarios for the reviewed controller-scoped Sliver anthem cohort.
//!
//! Expectations are printed Oracle values. CR 604.1/611.3a make each anthem a
//! continuous static ability, CR 613.4c applies the bonus in layer 7c, and
//! CR 109.2 gives "Sliver creatures you control" its controller scope. The
//! pinned rulings confirm that the boosts are cumulative and that a creature
//! which stops being a Sliver stops being affected.
use super::helpers::*;

struct Case {
    id: &'static str,
    power: u32,
    toughness: u32,
    pump_power: u32,
    pump_toughness: u32,
}

const CASES: [Case; 5] = [
    Case {
        id: "battle_sliver",
        power: 3,
        toughness: 3,
        pump_power: 2,
        pump_toughness: 0,
    },
    Case {
        id: "cleaving_sliver",
        power: 2,
        toughness: 2,
        pump_power: 2,
        pump_toughness: 0,
    },
    Case {
        id: "megantic_sliver",
        power: 3,
        toughness: 3,
        pump_power: 3,
        pump_toughness: 3,
    },
    Case {
        id: "predatory_sliver",
        power: 1,
        toughness: 1,
        pump_power: 1,
        pump_toughness: 1,
    },
    Case {
        id: "steelform_sliver",
        power: 2,
        toughness: 2,
        pump_power: 0,
        pump_toughness: 1,
    },
];

fn engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![
        deck_with("forest", &[card, card, "metallic_sliver", "grizzly_bears"]),
        deck_with("island", &["metallic_sliver"]),
    ]);
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

fn body(engine: &GameEngine, id: u32) -> (Option<u32>, Option<u32>) {
    (engine.effective_power(id), engine.effective_toughness(id))
}

#[test]
fn sliver_anthem_batch_cards_pump_only_your_slivers_including_self() {
    for (index, case) in CASES.iter().enumerate() {
        let mut engine = engine(25_080_000 + index as u64, case.id);
        let source = move_ready_to_battlefield(&mut engine, 0, case.id);
        let own_other = move_ready_to_battlefield(&mut engine, 0, "metallic_sliver");
        let non_sliver = move_ready_to_battlefield(&mut engine, 0, "grizzly_bears");
        let opposing_sliver = move_ready_to_battlefield(&mut engine, 1, "metallic_sliver");

        assert_eq!(
            body(&engine, source),
            (
                Some(case.power + case.pump_power),
                Some(case.toughness + case.pump_toughness)
            ),
            "{} pumps itself",
            case.id
        );
        assert_eq!(
            body(&engine, own_other),
            (Some(1 + case.pump_power), Some(1 + case.pump_toughness)),
            "{} pumps another Sliver you control",
            case.id
        );
        assert_eq!(
            body(&engine, non_sliver),
            (Some(2), Some(2)),
            "{} leaves a non-Sliver unchanged",
            case.id
        );
        assert_eq!(
            body(&engine, opposing_sliver),
            (Some(1), Some(1)),
            "{} does not pump an opponent's Sliver",
            case.id
        );
    }
}

#[test]
fn sliver_anthem_batch_cards_stack_with_a_second_copy() {
    for (index, case) in CASES.iter().enumerate() {
        let mut engine = engine(25_081_000 + index as u64, case.id);
        let first = move_ready_to_battlefield(&mut engine, 0, case.id);
        let second = move_ready_to_battlefield(&mut engine, 0, case.id);

        for (slot, other) in [(first, second), (second, first)] {
            assert_eq!(
                body(&engine, slot),
                (
                    Some(case.power + 2 * case.pump_power),
                    Some(case.toughness + 2 * case.pump_toughness)
                ),
                "{} copy {slot} with copy {other}",
                case.id
            );
        }
    }
}
