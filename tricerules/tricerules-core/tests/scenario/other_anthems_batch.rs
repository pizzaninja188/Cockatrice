//! Actual-card scenarios for the queue-v26 unconditional other-creature anthem cohort.
//!
//! Expectations are printed Oracle values. CR 604.1/611.3a make the anthem a
//! continuous static ability; CR 613.4c applies it in layer 7c; CR 109.2 gives
//! the controller scope and the printed "other" excludes the source.

use super::helpers::*;
use tricerules_core::Zone;

struct Case {
    id: &'static str,
    power: u32,
    toughness: u32,
    delta_power: u32,
    delta_toughness: u32,
}

const CASES: [Case; 3] = [
    Case {
        id: "veteran_armorer",
        power: 2,
        toughness: 2,
        delta_power: 0,
        delta_toughness: 1,
    },
    Case {
        id: "benalish_marshal",
        power: 3,
        toughness: 3,
        delta_power: 1,
        delta_toughness: 1,
    },
    Case {
        id: "blessed_orator",
        power: 1,
        toughness: 4,
        delta_power: 0,
        delta_toughness: 1,
    },
];

fn engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![
        deck_with("plains", &[card, card, "grizzly_bears", "savannah_lions"]),
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

fn move_to_graveyard(e: &mut GameEngine, player: usize, object: u32) {
    e.state.players[player]
        .battlefield
        .retain(|oid| *oid != object);
    e.state.players[player].graveyard.push(object);
    e.state.objects.get_mut(&object).expect("source").zone = Zone::Graveyard;
    *e.state.zone_change_generation.entry(object).or_default() += 1;
}

#[test]
fn other_anthems_batch_cards_pump_only_other_creatures_you_control() {
    for (index, case) in CASES.iter().enumerate() {
        let mut e = engine(26_050_000 + index as u64, case.id);
        let source = move_ready_to_battlefield(&mut e, 0, case.id);
        let own = move_ready_to_battlefield(&mut e, 0, "grizzly_bears");
        let opposing = move_ready_to_battlefield(&mut e, 1, "grizzly_bears");

        assert_eq!(
            (e.effective_power(source), e.effective_toughness(source)),
            (Some(case.power), Some(case.toughness)),
            "{}: excludes itself",
            case.id
        );
        assert_eq!(
            (e.effective_power(own), e.effective_toughness(own)),
            (Some(2 + case.delta_power), Some(2 + case.delta_toughness)),
            "{}: pumps another creature you control",
            case.id
        );
        assert_eq!(
            (e.effective_power(opposing), e.effective_toughness(opposing)),
            (Some(2), Some(2)),
            "{}: does not pump an opponent's creature",
            case.id
        );
    }
}

#[test]
fn other_anthems_batch_cards_apply_to_later_entries_and_stop_when_the_source_leaves() {
    for (index, case) in CASES.iter().enumerate() {
        let mut e = engine(26_051_000 + index as u64, case.id);
        let source = move_ready_to_battlefield(&mut e, 0, case.id);
        let late = move_ready_to_battlefield(&mut e, 0, "savannah_lions");
        assert_eq!(
            (e.effective_power(late), e.effective_toughness(late)),
            (Some(2 + case.delta_power), Some(1 + case.delta_toughness)),
            "{}: a later entry is affected",
            case.id
        );

        move_to_graveyard(&mut e, 0, source);
        assert_eq!(
            (e.effective_power(late), e.effective_toughness(late)),
            (Some(2), Some(1)),
            "{}: the bonus ends with the source",
            case.id
        );
    }
}

#[test]
fn other_anthems_batch_cards_stack_with_a_second_copy() {
    for (index, case) in CASES.iter().enumerate() {
        let mut e = engine(26_052_000 + index as u64, case.id);
        let first = move_ready_to_battlefield(&mut e, 0, case.id);
        let second = move_ready_to_battlefield(&mut e, 0, case.id);
        let own = move_ready_to_battlefield(&mut e, 0, "grizzly_bears");

        assert_eq!(
            (e.effective_power(own), e.effective_toughness(own)),
            (
                Some(2 + 2 * case.delta_power),
                Some(2 + 2 * case.delta_toughness)
            ),
            "{}: two copies accumulate",
            case.id
        );
        for source in [first, second] {
            assert_eq!(
                (e.effective_power(source), e.effective_toughness(source)),
                (
                    Some(case.power + case.delta_power),
                    Some(case.toughness + case.delta_toughness)
                ),
                "{}: each copy is affected by the other",
                case.id
            );
        }
    }
}
