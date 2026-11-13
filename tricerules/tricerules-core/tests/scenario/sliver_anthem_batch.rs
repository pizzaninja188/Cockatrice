//! Actual-card scenarios for the reviewed all-Sliver anthem family.
//!
//! Expectations are printed Oracle values. CR 604.1/611.3a make the anthem continuous; CR 613.4c applies the layer-7c bonus; the affected set is every Sliver creature, including the source, regardless of controller.
use super::helpers::*;

struct Case {
    id: &'static str,
    power: u32,
    toughness: u32,
    pump_power: i32,
    pump_toughness: i32,
}

fn cases() -> [Case; 7] {
    [
        Case {
            id: "blade_sliver",
            power: 2,
            toughness: 2,
            pump_power: 1,
            pump_toughness: 0,
        },
        Case {
            id: "bonesplitter_sliver",
            power: 2,
            toughness: 2,
            pump_power: 2,
            pump_toughness: 0,
        },
        Case {
            id: "might_sliver",
            power: 2,
            toughness: 2,
            pump_power: 2,
            pump_toughness: 2,
        },
        Case {
            id: "muscle_sliver",
            power: 1,
            toughness: 1,
            pump_power: 1,
            pump_toughness: 1,
        },
        Case {
            id: "plated_sliver",
            power: 1,
            toughness: 1,
            pump_power: 0,
            pump_toughness: 1,
        },
        Case {
            id: "sinew_sliver",
            power: 1,
            toughness: 1,
            pump_power: 1,
            pump_toughness: 1,
        },
        Case {
            id: "watcher_sliver",
            power: 2,
            toughness: 2,
            pump_power: 0,
            pump_toughness: 2,
        },
    ]
}

fn engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![
        deck_with("forest", &[card, "sliver_construct"]),
        deck_with("island", &["sliver_construct"]),
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

#[test]
fn sliver_anthem_batch_cards_pump_every_sliver_including_the_source() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(11_700_000 + index as u64, case.id);
        let source = move_ready_to_battlefield(&mut e, 0, case.id);
        let friend = move_ready_to_battlefield(&mut e, 0, "sliver_construct");
        assert_eq!(
            (e.effective_power(source), e.effective_toughness(source)),
            (
                Some(case.power + case.pump_power as u32),
                Some(case.toughness + case.pump_toughness as u32)
            ),
            "{}: the source pumps itself",
            case.id
        );
        assert_eq!(
            (e.effective_power(friend), e.effective_toughness(friend)),
            (
                Some(2 + case.pump_power as u32),
                Some(2 + case.pump_toughness as u32)
            ),
            "{}: each other Sliver is pumped",
            case.id
        );
    }
}

#[test]
fn sliver_anthem_batch_cards_do_not_pump_non_slivers() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(11_710_000 + index as u64, case.id);
        move_ready_to_battlefield(&mut e, 0, case.id);
        let bear = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        assert_eq!(
            (e.effective_power(bear), e.effective_toughness(bear)),
            (Some(2), Some(2)),
            "{}: non-Sliver unchanged",
            case.id
        );
    }
}

#[test]
fn sliver_anthem_batch_cards_apply_symmetrically_to_each_players_slivers() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(11_720_000 + index as u64, case.id);
        move_ready_to_battlefield(&mut e, 0, case.id);
        let opposing = move_ready_to_battlefield(&mut e, 1, "sliver_construct");
        assert_eq!(
            (e.effective_power(opposing), e.effective_toughness(opposing)),
            (
                Some(2 + case.pump_power as u32),
                Some(2 + case.pump_toughness as u32)
            ),
            "{}: every player's Slivers are affected",
            case.id
        );
    }
}
