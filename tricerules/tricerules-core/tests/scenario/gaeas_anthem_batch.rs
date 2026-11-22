//! Actual-card scenarios for the reviewed controller-wide anthem family.
//!
//! Expectations are printed Oracle values. CR 604.2/611.3b and 613.4c make each anthem a
//! static layer-7c effect for creatures their controller controls, evaluated dynamically
//! and drained when the source leaves the battlefield.
use super::helpers::*;
use tricerules_core::Zone;

struct Case {
    id: &'static str,
    delta_power: i32,
    delta_toughness: i32,
}

fn cases() -> [Case; 4] {
    [
        Case {
            id: "collective_blessing",
            delta_power: 3,
            delta_toughness: 3,
        },
        Case {
            id: "fire_nations_conquest",
            delta_power: 1,
            delta_toughness: 0,
        },
        Case {
            id: "gaeas_anthem",
            delta_power: 1,
            delta_toughness: 1,
        },
        Case {
            id: "power_boost",
            delta_power: 1,
            delta_toughness: 0,
        },
    ]
}

fn engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![deck_with("forest", &[card]), deck_with("island", &[])]);
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
fn gaeas_anthem_batch_cards_buff_only_their_controllers_creatures() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(890_000 + index as u64, case.id);
        move_ready_to_battlefield(&mut e, 0, case.id);
        let mine = inject_permanent_on_battlefield(&mut e, 0, "savannah_lions");
        let theirs = inject_permanent_on_battlefield(&mut e, 1, "savannah_lions");
        assert_eq!(
            e.effective_power(mine),
            Some((2 + case.delta_power) as u32),
            "{} power",
            case.id
        );
        assert_eq!(
            e.effective_toughness(mine),
            Some((1 + case.delta_toughness) as u32),
            "{} toughness",
            case.id
        );
        assert_eq!(e.effective_power(theirs), Some(2), "{} opponent", case.id);
        assert_eq!(
            e.effective_toughness(theirs),
            Some(1),
            "{} opponent",
            case.id
        );
    }
}

#[test]
fn gaeas_anthem_batch_cards_apply_dynamically_and_drain_with_the_source() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(891_000 + index as u64, case.id);
        let anthem = move_ready_to_battlefield(&mut e, 0, case.id);
        let late = inject_permanent_on_battlefield(&mut e, 0, "savannah_lions");
        assert_eq!(
            e.effective_power(late),
            Some((2 + case.delta_power) as u32),
            "{}: a later creature is buffed",
            case.id
        );
        e.state.players[0].battlefield.retain(|oid| *oid != anthem);
        e.state.objects.get_mut(&anthem).expect("anthem").zone = Zone::Graveyard;
        e.state.players[0].graveyard.push(anthem);
        assert_eq!(
            e.effective_power(late),
            Some(2),
            "{}: the buff drains when the source leaves",
            case.id
        );
        assert_eq!(
            e.effective_toughness(late),
            Some(1),
            "{}: the toughness buff drains",
            case.id
        );
    }
}
