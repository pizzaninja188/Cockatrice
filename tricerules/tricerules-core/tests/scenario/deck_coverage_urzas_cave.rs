//! Actual-card coverage for Urza's Cave's mana and land-search abilities.
//!
//! Oracle text and the empty Scryfall rulings list were checked on 2026-09-28.
//! CR 205.3i covers its Urza's and Cave land subtypes; CR 602.2 covers activation and costs;
//! CR 605.1a classifies the colorless mana ability; CR 701.23 and 701.24 cover searching and
//! shuffling.

use super::helpers::*;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::ruled_event::Ev;

const URZAS_CAVE: &str = "urzas_cave";

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("island", &[]), deck_with("forest", &[])]);
    let mut engine = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

#[test]
fn urzas_cave_taps_for_colorless_mana_without_using_the_stack() {
    let mut engine = engine(202_609_280);
    let cave = inject_permanent_on_battlefield(&mut engine, 0, URZAS_CAVE);

    apply_ability(&mut engine, 0, cave, 0, vec![])
        .expect("activate Urza's Cave's colorless mana ability");

    assert_eq!(engine.state.players[0].mana_pool.colorless, 1);
    assert!(engine.state.objects[&cave].tapped);
    assert!(
        engine.state.stack.is_empty(),
        "the mana ability resolves immediately"
    );
}

#[test]
fn urzas_cave_pays_three_sacrifices_and_searches_for_any_land_tapped() {
    let mut engine = engine(202_609_281);
    let cave = inject_permanent_on_battlefield(&mut engine, 0, URZAS_CAVE);
    let taiga = inject_library_card(&mut engine, 0, "taiga");
    let sol_ring = inject_library_card(&mut engine, 0, "sol_ring");
    let opponent_taiga = inject_library_card(&mut engine, 1, "taiga");

    engine.state.players[0].mana_pool.colorless = 2;
    apply_ability(&mut engine, 0, cave, 1, vec![])
        .expect_err("the activated ability requires three generic mana");
    assert_eq!(engine.state.players[0].mana_pool.colorless, 2);
    assert_eq!(engine.state.objects[&cave].zone, Zone::Battlefield);
    assert!(!engine.state.objects[&cave].tapped);

    engine.state.players[0].mana_pool.colorless = 3;
    apply_ability(&mut engine, 0, cave, 1, vec![])
        .expect("pay {3}, tap, and sacrifice Urza's Cave");
    assert_eq!(engine.state.players[0].mana_pool.colorless, 0);
    assert_eq!(engine.state.objects[&cave].zone, Zone::Graveyard);
    assert_eq!(
        engine.state.stack.len(),
        1,
        "the search ability uses the stack"
    );

    engine.apply_command(0, &pass()).expect("controller passes");
    let search_batch = engine
        .apply_command(1, &pass())
        .expect("opponent passes and the ability resolves");
    let choice = find_resolution_choice(&search_batch).expect("land search choice");
    assert_eq!(choice.choice_kind(), ChoiceKind::LibrarySearch);
    assert_eq!((choice.min, choice.max), (0, 1));
    assert!(choice.candidate_object_ids.contains(&taiga));
    assert!(
        !choice.candidate_object_ids.contains(&opponent_taiga),
        "Urza's Cave searches its controller's library"
    );
    assert!(
        !choice.candidate_object_ids.contains(&sol_ring),
        "a nonland card is outside the land-search candidate set"
    );

    engine
        .apply_command(0, &submit_resolution_choice(vec![sol_ring]))
        .expect_err("a forged nonland choice must be rejected");
    assert!(engine.state.pending_resolution.is_some());
    assert_eq!(engine.state.objects[&taiga].zone, Zone::Library);

    let completion = engine
        .apply_command(0, &submit_resolution_choice(vec![taiga]))
        .expect("choose Taiga");
    assert_eq!(engine.state.objects[&taiga].zone, Zone::Battlefield);
    assert!(engine.state.objects[&taiga].tapped);
    assert_eq!(engine.state.objects[&taiga].owner, 0);
    assert_eq!(engine.state.objects[&taiga].controller, 0);
    assert!(!engine.state.players[0].library.contains(&taiga));
    assert!(engine.state.players[0].library.contains(&sol_ring));
    assert!(engine.state.players[1].library.contains(&opponent_taiga));
    assert!(completion.events.iter().any(|event| {
        matches!(
            &event.ev,
            Some(Ev::Log(log)) if log.text == "P0 shuffles their library."
        )
    }));
    assert!(engine.state.pending_resolution.is_none());
}
