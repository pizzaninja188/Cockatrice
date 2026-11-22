//! Queue-v29 canaries: "Search your library for a card, then shuffle and put that card on top.
//! You lose 2 life."
//!
//! CR 701.23, 701.23d-e/701.24, 119.3 and 608.2. Expectations are printed Oracle values.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::ruled_event::Ev;

struct Case {
    id: &'static str,
    mana: ManaGift,
}

fn case(id: &'static str) -> Case {
    match id {
        "imperial_seal" | "vampiric_tutor" => Case {
            id,
            mana: ManaGift {
                b: 1,
                ..Default::default()
            },
        },
        "cruel_tutor" => Case {
            id,
            mana: ManaGift {
                b: 1,
                c: 2,
                ..Default::default()
            },
        },
        _ => unreachable!(),
    }
}

fn cases() -> [Case; 3] {
    [
        case("imperial_seal"),
        case("cruel_tutor"),
        case("vampiric_tutor"),
    ]
}

fn gift(m: &ManaGift) -> ManaGift {
    ManaGift {
        w: m.w,
        u: m.u,
        b: m.b,
        r: m.r,
        g: m.g,
        c: m.c,
    }
}

fn engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![deck_with("swamp", &[card]), deck_with("forest", &[])]);
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

fn assert_canary(case: Case, seed: u64) {
    let mut e = engine(seed, case.id);
    let chosen = inject_library_card(&mut e, 0, "counterspell");
    give_mana(&mut e, 0, gift(&case.mana));
    inject_card_into_hand(&mut e, 0, case.id);
    let hand = hand_index_for_card(&e, 0, case.id);
    e.apply_scenario_command(0, &cast_spell(hand, vec![]))
        .unwrap_or_else(|error| panic!("cast {}: {error:?}", case.id));
    let batch = pass_both_players_with_batch(&mut e);
    let choice = find_resolution_choice(&batch)
        .unwrap_or_else(|| panic!("{}: library search choice", case.id));
    assert!(
        choice.candidate_object_ids.contains(&chosen),
        "{}: the injected card is a legal search choice",
        case.id
    );
    let resolved = e
        .apply_scenario_command(0, &submit_resolution_choice(vec![chosen]))
        .unwrap_or_else(|error| panic!("{}: submit search: {error:?}", case.id));
    assert_eq!(
        e.state.players[0].library.front().copied(),
        Some(chosen),
        "{}: the chosen card is on top",
        case.id
    );
    assert_eq!(
        e.state.objects[&chosen].zone,
        Zone::Library,
        "{}: the chosen card remains in the library",
        case.id
    );
    assert_eq!(
        e.state.players[0].life, 18,
        "{}: the controller loses 2 life",
        case.id
    );
    assert_eq!(
        e.state.players[1].life, 20,
        "{}: the opponent is unaffected",
        case.id
    );
    assert!(
        resolved.events.iter().any(|event| matches!(
            &event.ev, Some(Ev::Log(log)) if log.text == "P0 shuffles their library."
        )),
        "{}: the library was shuffled",
        case.id
    );
    assert!(
        !resolved
            .events
            .iter()
            .any(|event| matches!(&event.ev, Some(Ev::CardsRevealed(_)))),
        "{}: the search is private",
        case.id
    );
    assert!(
        !resolved.events.iter().any(|event| matches!(
            &event.ev,
            Some(Ev::PermanentMoved(moved))
                if moved.object_id == chosen && moved.object_id_publicly_observable
        )),
        "{}: the placed card is not publicly identified",
        case.id
    );
    assert!(e.state.stack.is_empty(), "{}", case.id);
}

#[test]
fn q29_search_top_lose_two_canary_simplest() {
    assert_canary(case("imperial_seal"), 29_090_001);
}

#[test]
fn q29_search_top_lose_two_canary_hardest() {
    assert_canary(case("vampiric_tutor"), 29_090_002);
}

#[test]
fn q29_search_top_lose_two_cards_search_any_card_and_lose_two() {
    for (index, case) in cases().into_iter().enumerate() {
        assert_canary(case, 29_091_000 + index as u64);
    }
}

#[test]
fn q29_search_top_lose_two_cards_can_choose_a_land_card() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(29_092_000 + index as u64, case.id);
        let chosen = inject_library_card(&mut e, 0, "forest");
        give_mana(&mut e, 0, gift(&case.mana));
        inject_card_into_hand(&mut e, 0, case.id);
        let hand = hand_index_for_card(&e, 0, case.id);
        e.apply_scenario_command(0, &cast_spell(hand, vec![]))
            .expect("cast the tutor");
        let batch = pass_both_players_with_batch(&mut e);
        let choice = find_resolution_choice(&batch).expect("search choice");
        assert!(choice.candidate_object_ids.contains(&chosen));
        e.apply_scenario_command(0, &submit_resolution_choice(vec![chosen]))
            .expect("choose the land");
        assert_eq!(e.state.players[0].library.front().copied(), Some(chosen));
        assert_eq!(e.state.players[0].life, 18);
    }
}

#[test]
fn q29_search_top_lose_two_only_the_instant_can_be_cast_with_a_nonempty_stack() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(29_093_000 + index as u64, case.id);
        inject_card_into_hand(&mut e, 0, "grizzly_bears");
        give_mana(
            &mut e,
            0,
            ManaGift {
                g: 1,
                c: 1,
                ..Default::default()
            },
        );
        let bears = hand_index_for_card(&e, 0, "grizzly_bears");
        e.apply_scenario_command(0, &cast_spell(bears, vec![]))
            .expect("cast a creature spell");
        give_mana(&mut e, 0, gift(&case.mana));
        inject_card_into_hand(&mut e, 0, case.id);
        let hand = hand_index_for_card(&e, 0, case.id);
        let result = e.apply_scenario_command(0, &cast_spell(hand, vec![]));
        if case.id == "vampiric_tutor" {
            assert!(result.is_ok(), "Vampiric Tutor is an instant");
            let batch = pass_both_players_with_batch(&mut e);
            let choice = find_resolution_choice(&batch).expect("search choice");
            let chosen = *choice.candidate_object_ids.first().expect("a candidate");
            e.apply_scenario_command(0, &submit_resolution_choice(vec![chosen]))
                .expect("resolve the instant search");
            assert_eq!(e.state.players[0].life, 18);
        } else {
            assert!(
                result.is_err(),
                "{}: a sorcery cannot be cast with a nonempty stack",
                case.id
            );
        }
    }
}
