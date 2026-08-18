use super::helpers::*;
use tricerules_core::{GameEngine, Zone};
use tricerules_proto::ruled::v1::permanent_moved::Destination;
use tricerules_proto::ruled::v1::ruled_event::Ev;

#[test]
fn evolving_wilds_finds_only_a_basic_land_and_puts_it_onto_the_battlefield_tapped() {
    let decks = Some(vec![
        deck_with("forest", &["evolving_wilds"]),
        deck_with("island", &[]),
    ]);
    let mut engine = GameEngine::new(9001, &[0, 1], 20, decks, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);

    let wilds = relocate_to_battlefield(&mut engine, 0, "evolving_wilds", false);
    let forest = inject_library_card(&mut engine, 0, "forest");
    let taiga = inject_library_card(&mut engine, 0, "taiga");
    let generation_before = engine
        .state
        .zone_change_generation
        .get(&forest)
        .copied()
        .unwrap_or(0);

    engine
        .apply_command(0, &activate_ability(wilds, 0, vec![]))
        .expect("tap and sacrifice Evolving Wilds");
    assert_eq!(engine.state.objects[&wilds].zone, Zone::Graveyard);
    assert_eq!(engine.state.stack.len(), 1);

    engine.apply_command(0, &pass()).expect("controller passes");
    let search_batch = engine
        .apply_command(1, &pass())
        .expect("opponent passes and the ability resolves");
    let choice = find_resolution_choice(&search_batch).expect("basic-land search choice");
    assert_eq!(choice.choice_kind(), ChoiceKind::LibrarySearch);
    assert_eq!((choice.min, choice.max), (0, 1));
    assert!(choice.candidate_object_ids.contains(&forest));
    assert!(
        !choice.candidate_object_ids.contains(&taiga),
        "Taiga is a land but not a basic land"
    );

    let completion = engine
        .apply_command(0, &submit_resolution_choice(vec![forest]))
        .expect("choose Forest");

    let object = engine.state.objects.get(&forest).expect("Forest object");
    assert_eq!(object.zone, Zone::Battlefield);
    assert!(object.tapped, "the searched-for Forest enters tapped");
    assert_eq!(object.owner, 0);
    assert_eq!(object.controller, 0);
    assert_eq!(
        engine.state.zone_change_generation.get(&forest).copied(),
        Some(generation_before + 1)
    );
    assert!(engine.state.players[0].battlefield.contains(&forest));
    assert!(!engine.state.players[0].library.contains(&forest));
    assert!(engine.state.players[0].library.contains(&taiga));
    assert!(permanents_moved_in(&completion).iter().any(|moved| {
        moved.object_id == forest
            && moved.owner_player_id == 0
            && moved.destination == Destination::Battlefield as i32
    }));
    assert!(completion.events.iter().any(|event| {
        matches!(
            &event.ev,
            Some(Ev::Log(log)) if log.text == "P0 shuffles their library."
        )
    }));
    assert!(engine.state.pending_resolution.is_none());
}

#[test]
fn evolving_wilds_rejects_a_forged_nonbasic_choice_but_allows_fail_to_find() {
    let decks = Some(vec![
        deck_with("forest", &["evolving_wilds"]),
        deck_with("island", &[]),
    ]);
    let mut engine = GameEngine::new(9002, &[0, 1], 20, decks, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);

    let wilds = relocate_to_battlefield(&mut engine, 0, "evolving_wilds", false);
    let forest = inject_library_card(&mut engine, 0, "forest");
    let taiga = inject_library_card(&mut engine, 0, "taiga");
    engine
        .apply_command(0, &activate_ability(wilds, 0, vec![]))
        .expect("activate");
    engine.apply_command(0, &pass()).expect("controller passes");
    let search_batch = engine.apply_command(1, &pass()).expect("resolve ability");
    let choice = find_resolution_choice(&search_batch).expect("search choice");
    assert_eq!(choice.min, 0);

    engine
        .apply_command(0, &submit_resolution_choice(vec![taiga]))
        .expect_err("a nonbasic land is not an engine-published candidate");
    assert!(engine.state.pending_resolution.is_some());
    assert!(engine.state.players[0].library.contains(&forest));
    assert!(engine.state.players[0].library.contains(&taiga));

    let completion = engine
        .apply_command(0, &submit_resolution_choice(vec![]))
        .expect("a filtered hidden-zone search may fail to find");
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.players[0].library.contains(&forest));
    assert!(engine.state.players[0].library.contains(&taiga));
    assert!(completion.events.iter().any(|event| {
        matches!(
            &event.ev,
            Some(Ev::Log(log)) if log.text == "P0 finds no card."
        )
    }));
    assert!(completion.events.iter().any(|event| {
        matches!(
            &event.ev,
            Some(Ev::Log(log)) if log.text == "P0 shuffles their library."
        )
    }));
}

#[test]
fn evolving_wilds_waits_for_entry_replacements_before_shuffling_and_resuming() {
    let decks = Some(vec![
        deck_with(
            "forest",
            &["evolving_wilds", "orb_of_dreams", "orb_of_dreams"],
        ),
        deck_with("island", &[]),
    ]);
    let mut engine = GameEngine::new(9003, &[0, 1], 20, decks, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);

    relocate_to_battlefield(&mut engine, 0, "orb_of_dreams", false);
    relocate_to_battlefield(&mut engine, 0, "orb_of_dreams", false);
    let wilds = relocate_to_battlefield(&mut engine, 0, "evolving_wilds", false);
    let forest = inject_library_card(&mut engine, 0, "forest");

    engine
        .apply_command(0, &activate_ability(wilds, 0, vec![]))
        .expect("activate");
    engine.apply_command(0, &pass()).expect("controller passes");
    engine.apply_command(1, &pass()).expect("resolve ability");
    // Exercise the generic untapped battlefield destination rather than Evolving Wilds' own
    // tapped instruction. Two Orbs then force a CR 616 ordering choice, proving the search uses
    // the shared battlefield-entry pipeline and keeps its shuffle/tail completion parked.
    engine
        .state
        .pending_resolution
        .as_mut()
        .expect("library search pending")
        .search_destination =
        tricerules_cards::primitives::SearchDestination::Battlefield { tapped: false };
    let replacement_batch = engine
        .apply_command(0, &submit_resolution_choice(vec![forest]))
        .expect("choose Forest");

    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("two applicable entry replacements require an ordering choice");
    assert_eq!(pending.choice_kind, ChoiceKind::ReplacementEffect);
    assert_eq!(pending.candidates.len(), 2);
    assert_eq!(engine.state.objects[&forest].zone, Zone::Library);
    assert!(
        !replacement_batch.events.iter().any(|event| {
            matches!(
                &event.ev,
                Some(Ev::Log(log)) if log.text == "P0 shuffles their library."
            )
        }),
        "the search must not shuffle until the selected card finishes entering"
    );

    let application = pending.candidates[0];
    let completion = engine
        .apply_command(0, &submit_resolution_choice(vec![application]))
        .expect("order the entry replacements");
    assert_eq!(engine.state.objects[&forest].zone, Zone::Battlefield);
    assert!(engine.state.objects[&forest].tapped);
    assert!(engine.state.pending_resolution.is_none());
    assert!(completion.events.iter().any(|event| {
        matches!(
            &event.ev,
            Some(Ev::Log(log)) if log.text == "P0 shuffles their library."
        )
    }));
    assert!(permanents_moved_in(&completion)
        .iter()
        .any(|moved| moved.object_id == forest));
}
