//! Issue #256 — generated simple creature triggers reuse canonical tokens and private
//! library actions.
//!
//! Oracle and rulings checked 2026-09-11. CR 111.10a-b defines Treasure and Food,
//! CR 603.6c/603.10 and 700.4 govern dies/LKI, and CR 701.22/701.25 define Scry/Surveil.

use super::helpers::*;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::{permanent_moved, ruled_command::Cmd, ChoiceKind};

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
    engine.apply_command(first, &pass()).expect("first pass");
    engine
        .apply_command(second, &pass())
        .expect("second pass resolves stack item")
}

#[test]
fn generated_plundering_pirate_creates_a_functional_canonical_treasure() {
    let decks = Some(vec![
        deck_with("mountain", &["plundering_pirate"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(256_001, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);

    move_ready_to_battlefield(&mut engine, 0, "plundering_pirate");
    assert_eq!(
        engine.state.stack.len(),
        1,
        "entry stages exactly one trigger"
    );
    let resolved = resolve_top_stack(&mut engine);
    let treasure = battlefield_token_oids(&engine, 0, "treasure");
    assert_eq!(treasure.len(), 1);
    let created = token_created_events(&resolved);
    assert_eq!(created.len(), 1);
    assert_eq!(created[0].card_id, "treasure");
    let identity = created[0].identity.as_ref().expect("public token identity");
    assert_eq!(identity.name, "Treasure");
    assert!(!identity.is_creature);

    let mut activate = activate_ability_for(&engine, treasure[0], 0, vec![]);
    let Some(Cmd::ActivateAbility(ability)) = activate.cmd.as_mut() else {
        unreachable!()
    };
    ability.mana_option_index = 3;
    engine
        .apply_command(0, &activate)
        .expect("canonical Treasure mana ability");
    assert_eq!(engine.state.players[0].mana_pool.red, 1);
    assert!(
        !engine.state.objects.contains_key(&treasure[0]),
        "sacrificed token ceases to exist after the mana ability resolves"
    );
}

#[test]
fn generated_gleaming_barrier_death_lki_gives_treasure_to_its_last_controller() {
    let mut engine = GameEngine::new(256_002, &[0, 1], 20, None, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    let barrier = inject_creature_under_foreign_control(&mut engine, 0, 1, "gleaming_barrier");
    engine
        .state
        .objects
        .get_mut(&barrier)
        .expect("barrier")
        .damage = 4;

    engine.apply_command(0, &pass()).expect("death SBA");
    assert_eq!(engine.state.objects[&barrier].zone, Zone::Graveyard);
    assert_eq!(
        engine.state.stack.len(),
        1,
        "death stages exactly one trigger"
    );
    resolve_entire_stack_two_player(&mut engine);
    assert!(battlefield_token_oids(&engine, 0, "treasure").is_empty());
    assert_eq!(battlefield_token_oids(&engine, 1, "treasure").len(), 1);
}

#[test]
fn generated_wakandan_drone_flock_scries_two_with_an_atomic_private_choice() {
    let decks = Some(vec![
        deck_with("plains", &["wakandan_drone_flock"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(256_003, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    let top = seat_on_top(
        &mut engine,
        0,
        &["storm_crow", "grizzly_bears", "hill_giant"],
    );

    move_ready_to_battlefield(&mut engine, 0, "wakandan_drone_flock");
    let resolution = resolve_top_stack(&mut engine);
    let choice = find_resolution_choice(&resolution).expect("Scry 2 choice");
    assert_eq!(choice.choice_kind(), ChoiceKind::LibraryTop);
    assert_eq!(choice.deciding_player_id, 0);
    assert_eq!((choice.min, choice.max), (0, 2));
    assert_eq!(choice.candidate_object_ids, top[..2]);

    let library_before = engine.state.players[0].library.clone();
    assert!(engine
        .apply_command(1, &submit_resolution_choice(vec![top[0]]))
        .is_err());
    assert_eq!(engine.state.players[0].library, library_before);
    engine
        .apply_command(0, &submit_resolution_choice(vec![top[0], top[1]]))
        .expect("put both cards on bottom");
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(
        engine.state.players[0]
            .library
            .iter()
            .rev()
            .take(2)
            .copied()
            .collect::<Vec<_>>(),
        vec![top[1], top[0]]
    );
}

#[test]
fn generated_shore_lurker_surveillance_moves_the_exact_top_card() {
    let decks = Some(vec![
        deck_with("plains", &["shore_lurker"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(256_004, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    let top = seat_on_top(&mut engine, 0, &["storm_crow"]);

    move_ready_to_battlefield(&mut engine, 0, "shore_lurker");
    let resolution = resolve_top_stack(&mut engine);
    let choice = find_resolution_choice(&resolution).expect("Surveil 1 choice");
    assert_eq!(choice.choice_kind(), ChoiceKind::LibraryLook);
    assert_eq!(choice.deciding_player_id, 0);
    assert_eq!(choice.candidate_object_ids, top);

    let completion = engine
        .apply_command(0, &submit_resolution_choice(top.clone()))
        .expect("put top card into graveyard");
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(engine.state.objects[&top[0]].zone, Zone::Graveyard);
    let moved = permanents_moved_in(&completion)
        .into_iter()
        .find(|moved| moved.object_id == top[0])
        .expect("exact private candidate move");
    assert_eq!(moved.destination(), permanent_moved::Destination::Graveyard);
    assert_eq!(moved.source_library_position, Some(0));
}
