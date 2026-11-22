//! Actual-card scenarios for the queue-v36 enters-the-battlefield mill cohort.
//!
//! Expectations are printed Oracle values. CR 603.6a puts the enters ability on the
//! stack; CR 701.17a-b mills the controller's top cards, and a shorter library mills
//! every remaining card.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;

struct Case {
    id: &'static str,
    mill: usize,
}

fn cases() -> [Case; 6] {
    [
        Case {
            id: "armored_skaab",
            mill: 4,
        },
        Case {
            id: "laboratory_brute",
            mill: 4,
        },
        Case {
            id: "necromancers_assistant",
            mill: 3,
        },
        Case {
            id: "screeching_skaab",
            mill: 2,
        },
        Case {
            id: "sultai_skullkeeper",
            mill: 2,
        },
        Case {
            id: "wailing_ghoul",
            mill: 2,
        },
    ]
}

fn engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![
        deck_with("island", &[card, "grizzly_bears"]),
        deck_with("island", &[]),
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

fn seat_on_top(e: &mut GameEngine, player: usize, card_ids: &[&str]) -> Vec<u32> {
    let object_ids = card_ids
        .iter()
        .map(|card_id| inject_library_card(e, player, card_id))
        .collect::<Vec<_>>();
    e.state.players[player]
        .library
        .retain(|object_id| !object_ids.contains(object_id));
    for object_id in object_ids.iter().rev() {
        e.state.players[player].library.push_front(*object_id);
    }
    object_ids
}

fn resolve_top_stack(e: &mut GameEngine) -> RuledEventBatch {
    let first = e.state.priority_player_id();
    let second = if first == e.state.players[0].id {
        e.state.players[1].id
    } else {
        e.state.players[0].id
    };
    e.apply_scenario_command(first, &pass())
        .expect("first pass");
    e.apply_scenario_command(second, &pass())
        .expect("second pass resolves stack item")
}

fn enter_and_mill(e: &mut GameEngine, card: &str) -> u32 {
    let creature = move_ready_to_battlefield(e, 0, card);
    assert_eq!(
        e.state.objects[&creature].zone,
        Zone::Battlefield,
        "{card}: enters the battlefield"
    );
    resolve_top_stack(e);
    creature
}

#[test]
fn q36_etb_mill_canary_simplest() {
    let case = &cases()[3];
    let mut e = engine(36_040_001, case.id);
    let top = seat_on_top(&mut e, 0, &["storm_crow", "hill_giant", "forest"]);
    let creature = enter_and_mill(&mut e, case.id);
    assert_eq!(e.state.objects[&creature].zone, Zone::Battlefield);
    for object in &top[..case.mill] {
        assert_eq!(
            e.state.objects[object].zone,
            Zone::Graveyard,
            "Screeching Skaab mills the top two cards"
        );
    }
    assert_eq!(e.state.objects[&top[2]].zone, Zone::Library);
    assert_eq!(
        e.state.players[0].library.front().copied(),
        Some(top[2]),
        "the card below the milled cohort is now on top"
    );
}

#[test]
fn q36_etb_mill_canary_hardest() {
    let case = &cases()[0];
    let mut e = engine(36_040_002, case.id);
    let top = seat_on_top(
        &mut e,
        0,
        &[
            "storm_crow",
            "hill_giant",
            "forest",
            "savannah_lions",
            "island",
        ],
    );
    let creature = enter_and_mill(&mut e, case.id);
    assert_eq!(e.state.objects[&creature].zone, Zone::Battlefield);
    for object in &top[..case.mill] {
        assert_eq!(
            e.state.objects[object].zone,
            Zone::Graveyard,
            "Armored Skaab mills the top four cards"
        );
    }
    assert_eq!(e.state.objects[&top[4]].zone, Zone::Library);
}

#[test]
fn q36_etb_mill_cards_mill_their_printed_count_on_entry() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(36_041_000 + index as u64, case.id);
        let top = seat_on_top(
            &mut e,
            0,
            &[
                "storm_crow",
                "hill_giant",
                "forest",
                "savannah_lions",
                "island",
                "grizzly_bears",
            ],
        );
        enter_and_mill(&mut e, case.id);
        for (position, object) in top.iter().enumerate() {
            let expected = if position < case.mill {
                Zone::Graveyard
            } else {
                Zone::Library
            };
            assert_eq!(
                e.state.objects[object].zone, expected,
                "{}: card {position} after the printed mill",
                case.id
            );
        }
        assert_eq!(
            e.state.players[0].graveyard.len(),
            case.mill,
            "{}: exactly the printed number reaches the graveyard",
            case.id
        );
    }
}

#[test]
fn q36_etb_mill_cards_mill_every_available_card_from_a_short_library() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(36_042_000 + index as u64, case.id);
        let creature = move_ready_to_battlefield(&mut e, 0, case.id);
        assert_eq!(e.state.objects[&creature].zone, Zone::Battlefield);
        e.state.players[0].library.clear();
        let short = (case.mill - 1).min(3);
        let seated = seat_on_top(&mut e, 0, &["storm_crow", "hill_giant", "forest"][..short]);
        let opponent_card = seat_on_top(&mut e, 1, &["savannah_lions"])[0];
        resolve_top_stack(&mut e);
        for object in &seated {
            assert_eq!(
                e.state.objects[object].zone,
                Zone::Graveyard,
                "{}: every remaining card is milled",
                case.id
            );
        }
        assert!(
            e.state.players[0].library.is_empty(),
            "{}: the short library is exhausted",
            case.id
        );
        assert_eq!(
            e.state.objects[&opponent_card].zone,
            Zone::Library,
            "{}: only the controller's library is milled",
            case.id
        );
    }
}
