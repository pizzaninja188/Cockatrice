//! Exact deck-corpus coverage for Cultivate's split basic-land search.
//!
//! Oracle text and its one-card ruling were checked against the pinned Scryfall source and
//! Wizards' Strixhaven release notes. CR 701.23 governs the library search, CR 701.24a the
//! shuffle, and CR 611.2a / 614.1 govern the tapped battlefield entry and its replacements.

use super::helpers::*;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::permanent_moved::Destination;

fn begin_cultivate(seed: u64, library_cards: &[&str]) -> (GameEngine, Vec<u32>, RuledEventBatch) {
    let decks = Some(vec![
        deck_with("forest", &["cultivate"]),
        deck_with("island", &[]),
    ]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("new game");
    advance_to_main1_from_game_start(&mut engine);

    let searched: Vec<_> = library_cards
        .iter()
        .map(|card| inject_library_card(&mut engine, 0, card))
        .collect();
    ensure_in_hand(&mut engine, 0, "cultivate");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            g: 1,
            c: 2,
            ..Default::default()
        },
    );
    let cultivate = cast_spell(hand_index_for_card(&engine, 0, "cultivate"), vec![]);
    semantic::accepted(&mut engine, 0, &cultivate);
    semantic::accepted(&mut engine, 0, &pass());
    let search = semantic::accepted(&mut engine, 1, &pass());
    (engine, searched, search)
}

fn shuffle_count(batch: &RuledEventBatch) -> usize {
    batch
        .events
        .iter()
        .filter(|event| {
            matches!(&event.ev, Some(Ev::Log(log)) if log.text == "P0 shuffles their library.")
        })
        .count()
}

#[test]
fn cultivate_can_find_zero_basics_and_still_shuffles_once() {
    let (mut engine, cards, search) = begin_cultivate(202_609_280, &["forest", "island"]);
    let choice = find_resolution_choice(&search).expect("basic-land search choice");
    assert_eq!(choice.choice_kind(), ChoiceKind::LibrarySearch);
    assert_eq!((choice.min, choice.max), (0, 2));
    assert!(
        choice.ordered,
        "selection order assigns the two destinations"
    );
    assert!(choice
        .prompt_text
        .contains("the first enters the battlefield tapped"));
    assert!(cards
        .iter()
        .all(|card| choice.candidate_object_ids.contains(card)));

    let completion = semantic::accepted(&mut engine, 0, &submit_resolution_choice(vec![]));
    assert!(cards
        .iter()
        .all(|card| engine.state.objects[card].zone == Zone::Library));
    assert_eq!(shuffle_count(&completion), 1);
    assert!(!completion
        .events
        .iter()
        .any(|event| matches!(&event.ev, Some(Ev::CardsRevealed(_)))));
}

#[test]
fn cultivate_sends_a_single_found_basic_to_the_battlefield_tapped() {
    let (mut engine, cards, search) = begin_cultivate(202_609_281, &["forest", "island"]);
    let choice = find_resolution_choice(&search).expect("basic-land search choice");
    let basic = cards[0];

    let completion = semantic::accepted(&mut engine, 0, &submit_resolution_choice(vec![basic]));
    assert_eq!(engine.state.objects[&basic].zone, Zone::Battlefield);
    assert!(engine.state.objects[&basic].tapped);
    assert!(engine.state.players[0].battlefield.contains(&basic));
    assert!(engine.state.players[0].library.contains(&cards[1]));
    assert!(completion.events.iter().any(|event| {
        matches!(&event.ev, Some(Ev::CardsRevealed(reveal)) if reveal.cards.iter().any(|card| card.object_id == basic))
    }));
    assert_eq!(shuffle_count(&completion), 1);
    assert!(permanents_moved_in(&completion).iter().any(|moved| {
        moved.object_id == basic && moved.destination == Destination::Battlefield as i32
    }));
    assert!(choice.candidate_object_ids.contains(&basic));
}

#[test]
fn cultivate_uses_selection_order_for_the_two_revealed_destinations() {
    let (mut engine, cards, search) = begin_cultivate(202_609_282, &["forest", "island", "taiga"]);
    let choice = find_resolution_choice(&search).expect("basic-land search choice");
    let forest = cards[0];
    let island = cards[1];
    let taiga = cards[2];
    assert!(choice.candidate_object_ids.contains(&forest));
    assert!(choice.candidate_object_ids.contains(&island));
    assert!(
        !choice.candidate_object_ids.contains(&taiga),
        "Taiga is a land but is not a basic land"
    );

    let forest_candidate_index = choice
        .candidate_object_ids
        .iter()
        .position(|candidate| *candidate == forest)
        .expect("forest candidate");
    let island_candidate_index = choice
        .candidate_object_ids
        .iter()
        .position(|candidate| *candidate == island)
        .expect("island candidate");
    assert!(forest_candidate_index < island_candidate_index);

    let rejected = engine.apply_command(0, &submit_resolution_choice(vec![taiga, forest]));
    assert!(rejected.is_err(), "a forged nonbasic choice is rejected");
    assert!(engine.state.pending_resolution.is_some());
    assert_eq!(engine.state.objects[&taiga].zone, Zone::Library);
    assert_eq!(engine.state.objects[&forest].zone, Zone::Library);

    let completion = semantic::accepted(
        &mut engine,
        0,
        &submit_resolution_choice(vec![island, forest]),
    );
    assert_eq!(engine.state.objects[&island].zone, Zone::Battlefield);
    assert!(engine.state.objects[&island].tapped);
    assert!(engine.state.players[0].battlefield.contains(&island));
    assert_eq!(engine.state.objects[&forest].zone, Zone::Hand);
    assert!(engine.state.players[0].hand.contains(&forest));
    assert_eq!(engine.state.objects[&taiga].zone, Zone::Library);
    assert!(completion.events.iter().any(|event| {
        matches!(&event.ev, Some(Ev::CardsRevealed(reveal))
            if reveal.cards.iter().any(|card| card.object_id == forest)
                && reveal.cards.iter().any(|card| card.object_id == island))
    }));
    assert_eq!(shuffle_count(&completion), 1);
    let moved = permanents_moved_in(&completion);
    let battlefield_index = moved
        .iter()
        .position(|event| {
            event.object_id == island && event.destination == Destination::Battlefield as i32
        })
        .expect("first selection enters the battlefield first");
    let hand_index = moved
        .iter()
        .position(|event| {
            event.object_id == forest && event.destination == Destination::Hand as i32
        })
        .expect("second selection moves to hand after the battlefield entry");
    assert!(battlefield_index < hand_index);
    let shuffle_index = completion
        .events
        .iter()
        .position(|event| matches!(&event.ev, Some(Ev::Log(log)) if log.text == "P0 shuffles their library."))
        .expect("library shuffles after both destinations");
    let hand_event_index = completion
        .events
        .iter()
        .position(|event| matches!(&event.ev, Some(Ev::PermanentMoved(moved)) if moved.object_id == forest && moved.destination == Destination::Hand as i32))
        .expect("second selection moves to hand");
    assert!(hand_event_index < shuffle_index);
}
