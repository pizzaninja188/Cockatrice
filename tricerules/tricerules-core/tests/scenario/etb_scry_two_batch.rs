//! Actual-card scenarios for the reviewed enters-scry-two cohort.
//!
//! Expectations are printed Oracle values. CR 603.6a makes the enters ability
//! trigger; CR 701.22 defines Scry. Each card scries exactly two cards.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_proto::ruled::v1::ChoiceKind;

struct Case {
    id: &'static str,
}

fn cases() -> [Case; 5] {
    [
        Case {
            id: "automatic_librarian",
        },
        Case { id: "lost_legion" },
        Case { id: "octoprophet" },
        Case { id: "omenspeaker" },
        Case {
            id: "prophet_of_the_peak",
        },
    ]
}

fn engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![
        deck_with("island", &[card]),
        deck_with("island", &[card]),
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

fn seat_on_top(engine: &mut GameEngine, player: usize, card_ids: &[&str]) -> Vec<u32> {
    let object_ids = card_ids
        .iter()
        .map(|card_id| inject_library_card(engine, player, card_id))
        .collect::<Vec<_>>();
    engine.state.players[player]
        .library
        .retain(|object_id| !object_ids.contains(object_id));
    for object_id in object_ids.iter().rev() {
        engine.state.players[player].library.push_front(*object_id);
    }
    object_ids
}

fn resolve_top_stack(engine: &mut GameEngine) -> RuledEventBatch {
    let first = engine.state.priority_player_id();
    let second = if first == engine.state.players[0].id {
        engine.state.players[1].id
    } else {
        engine.state.players[0].id
    };
    engine
        .apply_scenario_command(first, &pass())
        .expect("first pass");
    engine
        .apply_scenario_command(second, &pass())
        .expect("second pass resolves stack item")
}

fn enter_and_park_scry(
    e: &mut GameEngine,
    player: usize,
    card: &str,
    top: &[u32],
) -> ResolutionChoiceRequired {
    let creature = move_ready_to_battlefield(e, player, card);
    assert!(
        e.state.objects[&creature].zone == tricerules_core::Zone::Battlefield,
        "{card}: enters the battlefield"
    );
    let resolution = resolve_top_stack(e);
    let choice =
        find_resolution_choice(&resolution).expect("the enters trigger parks a scry choice");
    assert_eq!(choice.choice_kind(), ChoiceKind::LibraryTop, "{card}");
    assert_eq!(choice.min, 0, "{card}");
    assert_eq!(choice.max, 2, "{card}");
    assert_eq!(
        choice.candidate_object_ids,
        top[..2],
        "{card}: only the printed top two are offered"
    );
    choice
}

#[test]
fn etb_scry_two_batch_cards_scry_two_on_entry() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_160_000 + index as u64, case.id);
        let top = seat_on_top(
            &mut e,
            0,
            &["storm_crow", "grizzly_bears", "hill_giant", "forest"],
        );
        let _ = enter_and_park_scry(&mut e, 0, case.id, &top);
        let library_before = e.state.players[0].library.clone();
        assert!(
            e.apply_scenario_command(0, &submit_resolution_choice(top[..2].to_vec()))
                .is_ok(),
            "{}: scry choice is accepted",
            case.id
        );
        assert!(e.state.pending_resolution.is_none(), "{}", case.id);
        assert_eq!(
            e.state.players[0].library.back(),
            Some(&top[1]),
            "{}: the last clicked card moves to the bottom",
            case.id
        );
        assert_eq!(
            e.state.players[0].library.len(),
            library_before.len(),
            "{}",
            case.id
        );
    }
}

#[test]
fn etb_scry_two_batch_cards_offer_only_the_printed_top_cards() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_161_000 + index as u64, case.id);
        let top = seat_on_top(
            &mut e,
            0,
            &["storm_crow", "grizzly_bears", "hill_giant", "forest"],
        );
        let _ = enter_and_park_scry(&mut e, 0, case.id, &top);
        let library_before = e.state.players[0].library.clone();
        assert!(
            e.apply_scenario_command(0, &submit_resolution_choice(top[..3].to_vec()))
                .is_err(),
            "{}: more than the printed scry count is rejected",
            case.id
        );
        assert_eq!(
            e.state.players[0].library, library_before,
            "{}: rejected choice leaves the library unchanged",
            case.id
        );
        assert!(e.state.pending_resolution.is_some(), "{}", case.id);
        e.apply_scenario_command(0, &submit_resolution_choice(top[..2].to_vec()))
            .expect("complete the scry choice");
        assert!(e.state.pending_resolution.is_none(), "{}", case.id);
    }
}

#[test]
fn etb_scry_two_batch_cards_scope_the_choice_to_their_controller() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_162_000 + index as u64, case.id);
        let top = seat_on_top(
            &mut e,
            1,
            &["storm_crow", "grizzly_bears", "hill_giant", "forest"],
        );
        let choice = enter_and_park_scry(&mut e, 1, case.id, &top);
        assert_eq!(
            choice.deciding_player_id, e.state.players[1].id,
            "{}: the entering controller scries",
            case.id
        );
        e.apply_scenario_command(1, &submit_resolution_choice(top[..2].to_vec()))
            .expect("the controller completes the choice");
        assert!(e.state.pending_resolution.is_none(), "{}", case.id);
    }
}
