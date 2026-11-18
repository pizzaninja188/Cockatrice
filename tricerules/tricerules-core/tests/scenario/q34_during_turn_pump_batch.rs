//! Actual-card scenarios for the queue-v34 "during your turn" pump cohort.
//!
//! Expectations are printed Oracle values. CR 611.3/613.1f reevaluate the
//! active-player condition continuously, so the pump applies only while its
//! controller is the active player.
use super::helpers::*;

struct Case {
    id: &'static str,
    body: (u32, u32),
    pump: (u32, u32),
}

fn cases() -> [Case; 3] {
    [
        Case {
            id: "hardy_veteran",
            body: (2, 2),
            pump: (0, 2),
        },
        Case {
            id: "sporeback_wolf",
            body: (2, 2),
            pump: (0, 2),
        },
        Case {
            id: "wildwood_geist",
            body: (3, 3),
            pump: (2, 2),
        },
    ]
}

fn engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![
        deck_with("forest", &[card]),
        deck_with("forest", &[card]),
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

fn pt(e: &GameEngine, id: u32) -> (Option<u32>, Option<u32>) {
    (e.effective_power(id), e.effective_toughness(id))
}

#[test]
fn q34_during_turn_pump_canary_simplest() {
    let case = &cases()[0];
    let mut e = engine(34_090_001, case.id);
    let source = move_ready_to_battlefield(&mut e, 0, case.id);
    assert_eq!(
        pt(&e, source),
        (Some(2), Some(4)),
        "Hardy Veteran gets +0/+2 during its controller's turn"
    );
    end_active_turn(&mut e, 0);
    assert_ne!(e.state.active_player_id(), 0, "the turn passed");
    assert_eq!(
        pt(&e, source),
        (Some(2), Some(2)),
        "the pump ends when the turn ends"
    );
}

#[test]
fn q34_during_turn_pump_canary_hardest() {
    let case = &cases()[2];
    let mut e = engine(34_090_002, case.id);
    let source = move_ready_to_battlefield(&mut e, 0, case.id);
    assert_eq!(
        pt(&e, source),
        (Some(5), Some(5)),
        "Wildwood Geist gets +2/+2 during its controller's turn"
    );
}

#[test]
fn q34_during_turn_pump_cards_apply_only_while_their_controller_is_active() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(34_091_000 + index as u64, case.id);
        let own = move_ready_to_battlefield(&mut e, 0, case.id);
        let opposing = move_ready_to_battlefield(&mut e, 1, case.id);
        assert_eq!(
            pt(&e, own),
            (
                Some(case.body.0 + case.pump.0),
                Some(case.body.1 + case.pump.1)
            ),
            "{}: the active player's creature is pumped",
            case.id
        );
        assert_eq!(
            pt(&e, opposing),
            (Some(case.body.0), Some(case.body.1)),
            "{}: the nonactive player's creature is not pumped",
            case.id
        );
        end_active_turn(&mut e, 0);
        assert_eq!(
            pt(&e, own),
            (Some(case.body.0), Some(case.body.1)),
            "{}: the first creature loses the pump",
            case.id
        );
        assert_eq!(
            pt(&e, opposing),
            (
                Some(case.body.0 + case.pump.0),
                Some(case.body.1 + case.pump.1)
            ),
            "{}: the new active player's creature is pumped",
            case.id
        );
    }
}
