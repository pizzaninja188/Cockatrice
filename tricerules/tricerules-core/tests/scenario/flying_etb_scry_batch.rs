//! Actual-card scenarios for the reviewed Flying plus enters-scry family.
//!
//! Expectations are printed Oracle values. CR 702.9 defines Flying; CR 603.6a
//! makes the enters ability trigger; CR 701.22 defines Scry. Each card's scry
//! count is the printed count, not a shared assumption.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_cards::Keyword;
use tricerules_proto::ruled::v1::ChoiceKind;

struct Case {
    id: &'static str,
    scry: u32,
}

fn cases() -> [Case; 8] {
    [
        Case {
            id: "horizon_scholar",
            scry: 2,
        },
        Case {
            id: "augury_owl",
            scry: 3,
        },
        Case {
            id: "silver_raven",
            scry: 1,
        },
        Case {
            id: "thaumaturges_familiar",
            scry: 1,
        },
        Case {
            id: "senate_griffin",
            scry: 1,
        },
        Case {
            id: "faerie_seer",
            scry: 2,
        },
        Case {
            id: "cloudreader_sphinx",
            scry: 2,
        },
        Case {
            id: "willow-wind",
            scry: 2,
        },
    ]
}

fn engine(seed: u64, case: &Case) -> GameEngine {
    let decks = Some(vec![
        deck_with("island", &[case.id]),
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

fn enter_and_park_scry(e: &mut GameEngine, case: &Case, top: &[u32]) -> ResolutionChoiceRequired {
    let creature = move_ready_to_battlefield(e, 0, case.id);
    assert!(
        e.effective_has_keyword(creature, Keyword::Flying),
        "{}: has Flying",
        case.id
    );
    let resolution = resolve_top_stack(e);
    let choice =
        find_resolution_choice(&resolution).expect("the enters trigger parks a scry choice");
    assert_eq!(choice.choice_kind(), ChoiceKind::LibraryTop, "{}", case.id);
    assert_eq!(choice.deciding_player_id, 0, "{}", case.id);
    assert_eq!((choice.min, choice.max), (0, case.scry), "{}", case.id);
    assert_eq!(
        choice.candidate_object_ids,
        top[..case.scry as usize],
        "{}: only the printed top count is offered",
        case.id
    );
    choice
}

#[test]
fn flying_etb_scry_batch_cards_have_flying_and_scry_their_printed_count() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(890_000 + index as u64, &case);
        let top = seat_on_top(
            &mut e,
            0,
            &["storm_crow", "grizzly_bears", "hill_giant", "forest"],
        );
        let _ = enter_and_park_scry(&mut e, &case, &top);
        let library_before = e.state.players[0].library.clone();
        assert!(
            e.apply_scenario_command(
                0,
                &submit_resolution_choice(top[..case.scry as usize].to_vec())
            )
            .is_ok(),
            "{}: scry choice is accepted",
            case.id
        );
        assert!(e.state.pending_resolution.is_none(), "{}", case.id);
        assert_eq!(
            e.state.players[0].library.back(),
            Some(&top[case.scry as usize - 1]),
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
fn flying_etb_scry_batch_cards_offer_only_their_printed_top_cards() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(891_000 + index as u64, &case);
        let top = seat_on_top(
            &mut e,
            0,
            &["storm_crow", "grizzly_bears", "hill_giant", "forest"],
        );
        let _ = enter_and_park_scry(&mut e, &case, &top);
        let library_before = e.state.players[0].library.clone();
        let too_many = top[..case.scry as usize + 1].to_vec();
        assert!(
            e.apply_scenario_command(0, &submit_resolution_choice(too_many))
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
        e.apply_scenario_command(
            0,
            &submit_resolution_choice(top[..case.scry as usize].to_vec()),
        )
        .expect("complete the scry choice");
        assert!(e.state.pending_resolution.is_none(), "{}", case.id);
    }
}
