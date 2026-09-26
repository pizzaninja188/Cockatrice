//! Exact deck-corpus coverage for Nature's Lore, Rampant Growth, and Farseek.
//!
//! Oracle text and rulings were checked 2026-09-25 against pinned Scryfall oracle snapshot
//! 27bf3214-1271-490b-bdfe-c0be6c23d02e. CR 701.23b permits a hidden-zone search to find no card;
//! CR 701.23e governs whether the found card is revealed; CR 701.24a governs shuffling.

use super::helpers::*;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::permanent_moved::Destination;

#[test]
fn natures_lore_finds_a_nonbasic_forest_and_puts_it_onto_the_battlefield_untapped() {
    let decks = Some(vec![
        deck_with("forest", &["natures_lore"]),
        deck_with("island", &[]),
    ]);
    let mut engine = GameEngine::new(202_609_251, &[0, 1], 20, decks, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);

    let taiga = inject_library_card(&mut engine, 0, "taiga");
    let forest = inject_library_card(&mut engine, 0, "forest");
    let generation_before = engine
        .state
        .zone_change_generation
        .get(&taiga)
        .copied()
        .unwrap_or(0);
    ensure_in_hand(&mut engine, 0, "natures_lore");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            g: 1,
            c: 1,
            ..Default::default()
        },
    );
    engine
        .apply_command(
            0,
            &cast_spell(hand_index_for_card(&engine, 0, "natures_lore"), vec![]),
        )
        .expect("cast Nature's Lore");
    engine.apply_command(0, &pass()).expect("caster passes");
    let search_batch = engine
        .apply_command(1, &pass())
        .expect("resolve Nature's Lore");
    let choice = find_resolution_choice(&search_batch).expect("Forest search choice");
    assert_eq!(choice.choice_kind(), ChoiceKind::LibrarySearch);
    assert_eq!((choice.min, choice.max), (0, 1));
    assert!(choice.candidate_object_ids.contains(&taiga));
    assert!(choice.candidate_object_ids.contains(&forest));

    let completion = engine
        .apply_command(0, &submit_resolution_choice(vec![taiga]))
        .expect("choose Taiga, a nonbasic land with the Forest subtype");
    let object = &engine.state.objects[&taiga];
    assert_eq!(object.zone, Zone::Battlefield);
    assert!(
        !object.tapped,
        "Nature's Lore puts the land onto the battlefield untapped"
    );
    assert_eq!(
        engine.state.zone_change_generation.get(&taiga).copied(),
        Some(generation_before + 1)
    );
    assert!(engine.state.players[0].battlefield.contains(&taiga));
    assert!(engine.state.players[0].library.contains(&forest));
    assert!(permanents_moved_in(&completion).iter().any(|moved| {
        moved.object_id == taiga
            && moved.owner_player_id == 0
            && moved.destination == Destination::Battlefield as i32
    }));
    assert!(completion.events.iter().any(|event| {
        matches!(
            &event.ev,
            Some(Ev::Log(log)) if log.text == "P0 shuffles their library."
        )
    }));
}

#[test]
fn rampant_growth_rejects_a_nonbasic_land_and_puts_a_basic_onto_the_battlefield_tapped() {
    let decks = Some(vec![
        deck_with("forest", &["rampant_growth"]),
        deck_with("island", &[]),
    ]);
    let mut engine = GameEngine::new(202_609_252, &[0, 1], 20, decks, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);

    let forest = inject_library_card(&mut engine, 0, "forest");
    let taiga = inject_library_card(&mut engine, 0, "taiga");
    ensure_in_hand(&mut engine, 0, "rampant_growth");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            g: 1,
            c: 1,
            ..Default::default()
        },
    );
    engine
        .apply_command(
            0,
            &cast_spell(hand_index_for_card(&engine, 0, "rampant_growth"), vec![]),
        )
        .expect("cast Rampant Growth");
    engine.apply_command(0, &pass()).expect("caster passes");
    let search_batch = engine
        .apply_command(1, &pass())
        .expect("resolve Rampant Growth");
    let choice = find_resolution_choice(&search_batch).expect("basic-land search choice");
    assert_eq!(choice.choice_kind(), ChoiceKind::LibrarySearch);
    assert_eq!((choice.min, choice.max), (0, 1));
    assert!(choice.candidate_object_ids.contains(&forest));
    assert!(
        !choice.candidate_object_ids.contains(&taiga),
        "Taiga is a land but is not a basic land"
    );

    let completion = engine
        .apply_command(0, &submit_resolution_choice(vec![forest]))
        .expect("choose Forest");
    assert_eq!(engine.state.objects[&forest].zone, Zone::Battlefield);
    assert!(engine.state.objects[&forest].tapped);
    assert!(engine.state.players[0].battlefield.contains(&forest));
    assert!(engine.state.players[0].library.contains(&taiga));
    assert!(completion.events.iter().any(|event| {
        matches!(
            &event.ev,
            Some(Ev::Log(log)) if log.text == "P0 shuffles their library."
        )
    }));
}

#[test]
fn farseek_accepts_each_listed_subtype_including_a_nonbasic_land_and_enters_tapped() {
    let decks = Some(vec![
        deck_with("forest", &["farseek"]),
        deck_with("island", &[]),
    ]);
    let mut engine = GameEngine::new(202_609_253, &[0, 1], 20, decks, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);

    let plains = inject_library_card(&mut engine, 0, "plains");
    let island = inject_library_card(&mut engine, 0, "island");
    let swamp = inject_library_card(&mut engine, 0, "swamp");
    let mountain = inject_library_card(&mut engine, 0, "mountain");
    let taiga = inject_library_card(&mut engine, 0, "taiga");
    let forest = inject_library_card(&mut engine, 0, "forest");
    ensure_in_hand(&mut engine, 0, "farseek");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            g: 1,
            c: 1,
            ..Default::default()
        },
    );
    engine
        .apply_command(
            0,
            &cast_spell(hand_index_for_card(&engine, 0, "farseek"), vec![]),
        )
        .expect("cast Farseek");
    engine.apply_command(0, &pass()).expect("caster passes");
    let search_batch = engine.apply_command(1, &pass()).expect("resolve Farseek");
    let choice = find_resolution_choice(&search_batch).expect("listed-subtype search choice");
    assert_eq!(choice.choice_kind(), ChoiceKind::LibrarySearch);
    assert_eq!((choice.min, choice.max), (0, 1));
    for eligible in [plains, island, swamp, mountain, taiga] {
        assert!(choice.candidate_object_ids.contains(&eligible));
    }
    assert!(
        !choice.candidate_object_ids.contains(&forest),
        "Forest alone is not one of Farseek’s four listed land subtypes"
    );

    let completion = engine
        .apply_command(0, &submit_resolution_choice(vec![taiga]))
        .expect("choose Taiga, which has the Mountain subtype");
    assert_eq!(engine.state.objects[&taiga].zone, Zone::Battlefield);
    assert!(engine.state.objects[&taiga].tapped);
    assert!(engine.state.players[0].battlefield.contains(&taiga));
    assert!(engine.state.players[0].library.contains(&forest));
    assert!(completion.events.iter().any(|event| {
        matches!(
            &event.ev,
            Some(Ev::Log(log)) if log.text == "P0 shuffles their library."
        )
    }));
}
