//! Actual-card scenarios for the queue-v40 enters-the-battlefield surveil cohort.
//!
//! Expectations are printed Oracle values. CR 603.6a puts the entry trigger on the stack and
//! CR 701.25 defines surveil: look at the top cards, put any number into the graveyard, and the
//! rest back on top in any order. The looked cards stay private.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::ChoiceKind;

struct Case {
    id: &'static str,
    count: usize,
}

fn cases() -> [Case; 5] {
    [
        Case {
            id: "naga_oracle",
            count: 3,
        },
        Case {
            id: "whisper_agent",
            count: 1,
        },
        Case {
            id: "barrier_of_bones",
            count: 1,
        },
        Case {
            id: "watcher_in_the_mist",
            count: 2,
        },
        Case {
            id: "dimir_informant",
            count: 2,
        },
    ]
}

fn engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![deck_with("island", &[card]), deck_with("forest", &[])]);
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

fn seat_on_top(e: &mut GameEngine, card_ids: &[&str]) -> Vec<u32> {
    let oids: Vec<u32> = card_ids
        .iter()
        .map(|card_id| inject_library_card(e, 0, card_id))
        .collect();
    e.state.players[0].library.retain(|oid| !oids.contains(oid));
    for &oid in oids.iter().rev() {
        e.state.players[0].library.push_front(oid);
    }
    oids
}

fn resolve_entry(e: &mut GameEngine, card: &str) -> tricerules_proto::ruled::v1::RuledEventBatch {
    move_ready_to_battlefield(e, 0, card);
    answer_trigger_order_in_engine_order(e);
    e.apply_scenario_command(0, &pass()).expect("p0 pass");
    e.apply_scenario_command(1, &pass())
        .expect("opponent passes and the entry trigger resolves")
}

fn no_public_reveal(batch: &tricerules_proto::ruled::v1::RuledEventBatch) -> bool {
    !batch.events.iter().any(|event| {
        matches!(
            &event.ev,
            Some(tricerules_proto::ruled::v1::ruled_event::Ev::CardsRevealed(
                _
            ))
        )
    })
}

#[test]
fn q40_etb_surveil_canary_simplest() {
    let case = &cases()[2];
    let mut e = engine(40_030_001, case.id);
    let top = seat_on_top(&mut e, &["grizzly_bears", "forest"]);
    let batch = resolve_entry(&mut e, case.id);
    assert!(no_public_reveal(&batch));
    let choice = find_resolution_choice(&batch).expect("surveil choice");
    assert_eq!(choice.choice_kind(), ChoiceKind::LibraryLook);
    assert_eq!(choice.deciding_player_id, 0);
    assert_eq!(choice.candidate_object_ids, vec![top[0]]);
    assert_eq!((choice.min, choice.max), (0, 1));
    e.apply_scenario_command(0, &submit_resolution_choice(vec![top[0]]))
        .expect("put the top card into the graveyard");
    assert_eq!(e.state.objects[&top[0]].zone, Zone::Graveyard);
    assert_eq!(e.state.objects[&top[1]].zone, Zone::Library);
    assert_eq!(e.state.players[0].library.front(), Some(&top[1]));
}

#[test]
fn q40_etb_surveil_canary_hardest() {
    let case = &cases()[0];
    let mut e = engine(40_030_002, case.id);
    let top = seat_on_top(&mut e, &["grizzly_bears", "forest", "island", "mountain"]);
    let batch = resolve_entry(&mut e, case.id);
    let choice = find_resolution_choice(&batch).expect("surveil choice");
    assert_eq!(choice.candidate_object_ids, top[..3]);
    assert_eq!((choice.min, choice.max), (0, 3));
    let after = e
        .apply_scenario_command(0, &submit_resolution_choice(vec![top[0], top[1]]))
        .expect("put two looked cards into the graveyard");
    assert_eq!(e.state.objects[&top[0]].zone, Zone::Graveyard);
    assert_eq!(e.state.objects[&top[1]].zone, Zone::Graveyard);
    assert!(
        find_resolution_choice(&after).is_none(),
        "one remaining card needs no ordering choice"
    );
    assert_eq!(e.state.objects[&top[2]].zone, Zone::Library);
    assert_eq!(e.state.players[0].library.front(), Some(&top[2]));
}

#[test]
fn q40_etb_surveil_cards_apply_their_printed_count_and_keep_cards_private() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(40_031_000 + index as u64, case.id);
        let top = seat_on_top(&mut e, &["grizzly_bears", "forest", "island", "mountain"]);
        let batch = resolve_entry(&mut e, case.id);
        assert!(no_public_reveal(&batch), "{}: surveil is private", case.id);
        let choice = find_resolution_choice(&batch).expect("surveil choice");
        assert_eq!(
            choice.candidate_object_ids,
            top[..case.count],
            "{}: printed count",
            case.id
        );
        assert_eq!(
            (choice.min, choice.max),
            (0, case.count as u32),
            "{}: any number",
            case.id
        );
        let to_graveyard = top[..case.count].to_vec();
        let after = e
            .apply_scenario_command(0, &submit_resolution_choice(to_graveyard.clone()))
            .expect("put all looked cards into the graveyard");
        for oid in &to_graveyard {
            assert_eq!(e.state.objects[oid].zone, Zone::Graveyard, "{}", case.id);
        }
        assert!(
            find_resolution_choice(&after).is_none(),
            "{}: no top order remains when every card went to the graveyard",
            case.id
        );
        assert_eq!(
            e.state.players[0].library.front(),
            Some(&top[case.count]),
            "{}: the next library card is unchanged",
            case.id
        );
    }
}

#[test]
fn q40_etb_surveil_cards_keep_every_card_on_top() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(40_032_000 + index as u64, case.id);
        let top = seat_on_top(&mut e, &["grizzly_bears", "forest", "island", "mountain"]);
        let batch = resolve_entry(&mut e, case.id);
        find_resolution_choice(&batch).expect("surveil choice");
        let after = e
            .apply_scenario_command(0, &submit_resolution_choice(Vec::new()))
            .expect("keep every looked card on top");
        if let Some(order) = find_resolution_choice(&after) {
            assert_eq!(order.candidate_object_ids, top[..case.count], "{}", case.id);
            assert_eq!(
                (order.min, order.max),
                (case.count as u32, case.count as u32),
                "{}",
                case.id
            );
            let top_to_bottom: Vec<u32> = top[..case.count].iter().rev().copied().collect();
            e.apply_scenario_command(0, &submit_resolution_choice(top_to_bottom))
                .expect("order the top cards");
        }
        for oid in &top[..case.count] {
            assert_eq!(e.state.objects[oid].zone, Zone::Library, "{}", case.id);
        }
        assert_eq!(
            e.state.players[0].library.front(),
            Some(&top[0]),
            "{}",
            case.id
        );
    }
}
