//! CR 701.22 scry — the `Scry` primitive and the two authored cards built on it, Opt and Preordain.
//!
//! Both cards are `[Scry, Draw]`: the scry suspends resolution for the player's decision and the
//! draw runs when it resumes, so these also exercise the effect-tail resume that `docs/issues.md`
//! #36 tracked (its own regression test lives in `spell_effects.rs`).
use crate::helpers::ScenarioCommandExt;

use crate::helpers::*;
use tricerules_cards::CardRegistry;
use tricerules_core::EngineDeck;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::{
    ruled_command::Cmd, ChoiceKind, ResolutionChoiceDecision, RuledCommand, SubmitResolutionChoice,
};

const PROGRAM_SCRY2_DRAW_FIXTURE: &str = r#"(
  id: "program_scry2_draw_fixture",
  name: "Program Scry 2 Draw Fixture",
  face_id: "program_scry2_draw_fixture",
  mana_cost: "{0}",
  types: ["Instant"],
  spell_program: Some((body: (steps: [
    (id: "scry", bind: [], input: None, kind: Perform(Scry(count: 2))),
    (id: "draw", bind: [], input: None, kind: Perform(Draw(who: Controller, count: 1))),
  ], yields: []), target_bindings: [])),
)"#;

const TEST_SCRY_CREATURE_A: &str = r#"(
  id: "test_scry_creature_a",
  name: "Test Scry Creature A",
  face_id: "test_scry_creature_a",
  mana_cost: "{0}",
  types: ["Creature"],
  power: 1,
  toughness: 1,
)"#;

const TEST_SCRY_CREATURE_B: &str = r#"(
  id: "test_scry_creature_b",
  name: "Test Scry Creature B",
  face_id: "test_scry_creature_b",
  mana_cost: "{0}",
  types: ["Creature"],
  power: 1,
  toughness: 1,
)"#;

const TEST_SCRY_CREATURE_C: &str = r#"(
  id: "test_scry_creature_c",
  name: "Test Scry Creature C",
  face_id: "test_scry_creature_c",
  mana_cost: "{0}",
  types: ["Creature"],
  power: 1,
  toughness: 1,
)"#;

const PROGRAM_DRAW_SCRY_FIXTURE: &str = r#"(
  id: "program_draw_scry_fixture",
  name: "Program Draw Scry Fixture",
  face_id: "program_draw_scry_fixture",
  mana_cost: "{0}",
  types: ["Instant"],
  spell_program: Some((body: (steps: [
    (id: "draw", bind: [], input: None, kind: Perform(Draw(who: Controller, count: 1))),
    (id: "scry", bind: [], input: None, kind: Perform(Scry(count: 1))),
  ], yields: []), target_bindings: [])),
)"#;

const PROGRAM_NESTED_CHOOSE_IF_SCRY_TAIL_FIXTURE: &str = r#"(
  id: "program_nested_choose_if_scry_tail_fixture",
  name: "Program Nested Choose If Scry Tail Fixture",
  face_id: "program_nested_choose_if_scry_tail_fixture",
  mana_cost: "{0}",
  types: ["Instant"],
  spell_program: Some((body: (steps: [
    (id: "choose_scry", bind: [], input: None, kind: Choose(
      choice: "scry", chooser: Controller, optional: true,
      branches: [(branch_id: "scry", presentation: Fallback, cost: None,
        requirement: Always, body: (steps: [
          (id: "if_cards_in_hand", bind: [], input: None, kind: If(
            condition: CardsInHand(players: Relative(Controller), min: Some(1), max: None),
            yes: (steps: [
              (id: "scry", bind: [], input: None, kind: Perform(Scry(count: 1))),
            ], yields: []),
            no: (steps: [], yields: []),
          )),
        ], yields: []))],
      otherwise: (steps: [], yields: []),
    )),
    (id: "outer_gain_life", bind: [], input: None, kind: Perform(GainLife(amount: 1))),
  ], yields: []), target_bindings: [])),
)"#;

const PROGRAM_MILL_SCRY_CHOOSE_CARD_FIXTURE: &str = r#"(
  id: "program_mill_scry_choose_card_fixture",
  name: "Program Mill Scry Choose Card Fixture",
  face_id: "program_mill_scry_choose_card_fixture",
  mana_cost: "{0}",
  types: ["Instant"],
  spell_program: Some((body: (steps: [
    (id: "mill", bind: ["milled"], input: None,
      kind: Perform(Mill(who: Controller, count: 1))),
    (id: "scry", bind: [], input: None, kind: Perform(Scry(count: 1))),
    (id: "choose_milled", bind: [], input: Some("milled"),
      kind: Perform(ChooseCard(
        source: CardResultCohort(players: Controller),
        filter: (card_type: Some(Creature)), destination: Hand, optional: false,
      ))),
  ], yields: []), target_bindings: [])),
)"#;

/// Put `card_ids` on top of `player`'s library, first entry on top, and return their object ids.
/// `inject_library_card` appends to the bottom, so the cards are re-seated at the front.
fn seat_on_top(e: &mut GameEngine, player: usize, card_ids: &[&str]) -> Vec<u32> {
    let oids: Vec<u32> = card_ids
        .iter()
        .map(|cid| inject_library_card(e, player, cid))
        .collect();
    e.state.players[player]
        .library
        .retain(|o| !oids.contains(o));
    for &oid in oids.iter().rev() {
        e.state.players[player].library.push_front(oid);
    }
    oids
}

fn move_library_to_graveyard(e: &mut GameEngine, player: usize, object_id: u32) {
    e.state.players[player]
        .library
        .retain(|oid| *oid != object_id);
    e.state.players[player].graveyard.push(object_id);
    e.state.objects.get_mut(&object_id).unwrap().zone = Zone::Graveyard;
    *e.state.zone_change_generation.entry(object_id).or_default() += 1;
}

fn move_graveyard_to_library(e: &mut GameEngine, player: usize, object_id: u32) {
    e.state.players[player]
        .graveyard
        .retain(|oid| *oid != object_id);
    e.state.players[player].library.push_front(object_id);
    e.state.objects.get_mut(&object_id).unwrap().zone = Zone::Library;
    *e.state.zone_change_generation.entry(object_id).or_default() += 1;
}

fn island_deck_with(card: &str) -> Option<Vec<Vec<String>>> {
    Some(vec![
        deck_with("island", &[card]),
        vec!["forest".into(); 20],
    ])
}

fn program_scry_count_fixture(count: u32) -> String {
    format!(
        r#"(
  id: "program_scry_count_{count}_fixture",
  name: "Program Scry Count {count} Fixture",
  face_id: "program_scry_count_{count}_fixture",
  mana_cost: "",
  types: ["Instant"],
  spell_program: Some((body: (steps: [
    (id: "scry", bind: [], input: None, kind: Perform(Scry(count: {count}))),
    (id: "tail_gain_life", bind: [], input: None, kind: Perform(GainLife(amount: 1))),
  ], yields: []), target_bindings: [])),
)"#
    )
}

fn program_scry_registry() -> &'static CardRegistry {
    let count_fixtures = (0..=3).map(program_scry_count_fixture).collect::<Vec<_>>();
    let mut cards = count_fixtures
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    cards.extend([
        PROGRAM_SCRY2_DRAW_FIXTURE,
        PROGRAM_DRAW_SCRY_FIXTURE,
        PROGRAM_NESTED_CHOOSE_IF_SCRY_TAIL_FIXTURE,
        PROGRAM_MILL_SCRY_CHOOSE_CARD_FIXTURE,
        TEST_SCRY_CREATURE_A,
        TEST_SCRY_CREATURE_B,
        TEST_SCRY_CREATURE_C,
        include_str!("../../../tricerules-cards/data/forest.ron"),
    ]);
    Box::leak(Box::new(
        CardRegistry::from_chunks_and_tokens(&cards, &[])
            .expect("test-only Program Scry registry is valid"),
    ))
}

fn program_scry2_engine(seed: u64) -> GameEngine {
    let registry = program_scry_registry();
    let decks = Some(vec![
        EngineDeck {
            mainboard: [
                vec!["program_scry2_draw_fixture".into()],
                vec!["forest".into(); 8],
            ]
            .concat(),
            commanders: vec![],
        },
        EngineDeck {
            mainboard: vec!["forest".into(); 9],
            commanders: vec![],
        },
    ]);
    let mut engine = GameEngine::new_with_commander_decks(registry, seed, &[0, 1], 20, decks, true)
        .expect("two-player Program Scry engine");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn program_scry_three_player_engine(seed: u64) -> GameEngine {
    let registry = program_scry_registry();
    let decks = Some(vec![
        EngineDeck {
            mainboard: [
                vec!["program_scry2_draw_fixture".into()],
                vec!["forest".into(); 8],
            ]
            .concat(),
            commanders: vec![],
        },
        EngineDeck {
            mainboard: vec!["forest".into(); 9],
            commanders: vec![],
        },
        EngineDeck {
            mainboard: vec!["forest".into(); 9],
            commanders: vec![],
        },
    ]);
    let mut engine =
        GameEngine::new_with_commander_decks(registry, seed, &[0, 1, 2], 20, decks, true)
            .expect("three-player Program Scry engine");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn select_branch(index: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
            decision: ResolutionChoiceDecision::SelectBranch as i32,
            selected_branch_index: index,
            ..Default::default()
        })),
    }
}

fn resolution_choice_with_prompt(prompt_id: u64, chosen: Vec<u32>) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
            resolution_prompt_id: prompt_id,
            chosen_object_ids: chosen,
            ..Default::default()
        })),
    }
}

fn cast_three_player_scry2_until_choice(engine: &mut GameEngine) -> RuledEventBatch {
    ensure_in_hand(engine, 0, "program_scry2_draw_fixture");
    let slot = hand_index_for_card(engine, 0, "program_scry2_draw_fixture");
    engine
        .apply_scenario_command(0, &cast_spell(slot, vec![]))
        .expect("cast Program Scry 2 fixture");
    let mut batch = engine
        .apply_scenario_command(0, &pass())
        .expect("caster passes priority");
    for _ in 0..30 {
        if find_resolution_choice(&batch).is_some()
            || engine.state.stack.is_empty()
            || engine.state.blocking_choice().is_some()
        {
            break;
        }
        let priority = engine.state.priority_player_id();
        batch = engine
            .apply_scenario_command(priority, &pass())
            .expect("pass priority until Program Scry suspends");
    }
    batch
}

fn blue_mana() -> ManaGift {
    ManaGift {
        u: 1,
        ..Default::default()
    }
}

// ---------------------------------------------------------------------------
// Opt — Scry 1, then draw a card.

#[test]
fn opt_bottoming_the_scried_card_draws_the_next_one() {
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        7001,
        &[0, 1],
        20,
        island_deck_with("opt"),
        true,
    )
    .expect("new");
    advance_to_main1_from_game_start(&mut e);
    ensure_in_hand(&mut e, 0, "opt");
    let top = seat_on_top(&mut e, 0, &["grizzly_bears", "storm_crow"]);
    let (scried, next) = (top[0], top[1]);
    let lib_before = e.state.players[0].library.len();
    let hand_before_cast = e.state.players[0].hand.len();

    let batch = cast_instant_and_resolve(&mut e, 0, "opt", blue_mana());

    let req = find_resolution_choice(&batch).expect("scry parks for a choice");
    assert_eq!(
        e.state.players[0].hand.len(),
        hand_before_cast - 1,
        "Opt has not drawn while its Scry choice is pending"
    );
    assert_eq!(req.choice_kind(), ChoiceKind::LibraryTop);
    assert_eq!(req.deciding_player_id, 0);
    assert_eq!(
        (req.min, req.max),
        (0, 1),
        "scry 1 bottoms zero or one card"
    );
    // CR 701.22a bottoms "in any order", and the engine seats them in submitted order.
    assert!(req.ordered, "the bottom pile is ordered");
    assert_eq!(req.candidate_object_ids, vec![scried]);
    assert!(matches!(
        &e.state
            .pending_resolution
            .as_ref()
            .expect("scry continuation")
            .continuation,
        ResolutionContinuation::LibraryPartition {
            stage: PendingLibraryPartitionStage::ChooseDestination,
            kind: PendingLibraryPartitionKind::Scry,
            ..
        }
    ));
    assert!(
        !e.state.players[0].hand.contains(&scried),
        "scrying does not move the card out of the library"
    );

    e.apply_scenario_command(0, &submit_resolution_choice(vec![scried]))
        .expect("bottom the scried card");

    assert!(e.state.pending_resolution.is_none(), "scry 1 has no step 2");
    assert_eq!(
        *e.state.players[0].library.back().expect("library"),
        scried,
        "the bottomed card is now last"
    );
    assert!(
        e.state.players[0].hand.contains(&next),
        "the draw takes the card that was second from the top"
    );
    assert_eq!(
        e.state.players[0].library.len(),
        lib_before - 1,
        "one card left the library, via the draw"
    );
    assert_eq!(
        e.state.players[0].hand.len(),
        hand_before_cast,
        "Opt draws exactly one card after Scry"
    );
}

#[test]
fn opt_keeping_the_scried_card_on_top_draws_it() {
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        7002,
        &[0, 1],
        20,
        island_deck_with("opt"),
        true,
    )
    .expect("new");
    advance_to_main1_from_game_start(&mut e);
    ensure_in_hand(&mut e, 0, "opt");
    let scried = seat_on_top(&mut e, 0, &["grizzly_bears"])[0];
    let hand_before_cast = e.state.players[0].hand.len();

    let batch = cast_instant_and_resolve(&mut e, 0, "opt", blue_mana());
    assert!(
        find_resolution_choice(&batch).is_some(),
        "Scry parks before drawing"
    );
    assert_eq!(e.state.players[0].hand.len(), hand_before_cast - 1);
    // Keeping everything on top is submitting nothing (min is 0).
    e.apply_scenario_command(0, &submit_resolution_choice(vec![]))
        .expect("keep the scried card on top");

    assert!(e.state.pending_resolution.is_none());
    assert!(
        e.state.players[0].hand.contains(&scried),
        "the card kept on top is the one drawn"
    );
    assert_eq!(
        e.state.players[0].hand.len(),
        hand_before_cast,
        "Opt draws exactly one card after Scry"
    );
}

// ---------------------------------------------------------------------------
// Preordain — Scry 2, then draw a card.

#[test]
fn preordain_bottoming_both_skips_the_ordering_step() {
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        7003,
        &[0, 1],
        20,
        island_deck_with("preordain"),
        true,
    )
    .expect("new");
    advance_to_main1_from_game_start(&mut e);
    ensure_in_hand(&mut e, 0, "preordain");
    let top = seat_on_top(&mut e, 0, &["grizzly_bears", "storm_crow", "hill_giant"]);
    let (a, b, third) = (top[0], top[1], top[2]);
    let hand_before_cast = e.state.players[0].hand.len();

    let batch = cast_instant_and_resolve(&mut e, 0, "preordain", blue_mana());
    let req = find_resolution_choice(&batch).expect("scry parks");
    assert_eq!(
        e.state.players[0].hand.len(),
        hand_before_cast - 1,
        "Preordain has not drawn while its Scry choice is pending"
    );
    assert_eq!((req.min, req.max), (0, 2));
    assert_eq!(req.candidate_object_ids, vec![a, b]);

    e.apply_scenario_command(0, &submit_resolution_choice(vec![b, a]))
        .expect("bottom both");

    assert!(
        e.state.pending_resolution.is_none(),
        "nothing stays on top, so there is no order to choose"
    );
    let library: Vec<u32> = e.state.players[0].library.iter().copied().collect();
    assert_eq!(
        &library[library.len() - 2..],
        &[b, a],
        "cards go down in submitted order, so the last one clicked is bottom-most"
    );
    assert!(
        e.state.players[0].hand.contains(&third),
        "the draw comes from what was the third card"
    );
    assert_eq!(
        e.state.players[0].hand.len(),
        hand_before_cast,
        "Preordain draws exactly one card after Scry"
    );
}

#[test]
fn preordain_keeping_both_asks_for_an_order_then_draws_the_new_top() {
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        7004,
        &[0, 1],
        20,
        island_deck_with("preordain"),
        true,
    )
    .expect("new");
    advance_to_main1_from_game_start(&mut e);
    ensure_in_hand(&mut e, 0, "preordain");
    let top = seat_on_top(&mut e, 0, &["grizzly_bears", "storm_crow"]);
    let (a, b) = (top[0], top[1]);
    let hand_before_cast = e.state.players[0].hand.len();

    let batch = cast_instant_and_resolve(&mut e, 0, "preordain", blue_mana());
    assert!(
        find_resolution_choice(&batch).is_some(),
        "Scry parks before drawing"
    );
    assert_eq!(e.state.players[0].hand.len(), hand_before_cast - 1);
    let order_batch = e
        .apply_scenario_command(0, &submit_resolution_choice(vec![]))
        .expect("keep both on top");

    // Both stayed on top, so CR 701.22a's "in any order" is a real decision: step 2.
    let req = find_resolution_choice(&order_batch).expect("ordering choice required");
    assert_eq!(req.choice_kind(), ChoiceKind::LibraryTop);
    assert_eq!((req.min, req.max), (2, 2), "every kept card must be placed");
    assert!(req.ordered, "the order submitted is significant");
    assert_eq!(req.candidate_object_ids, vec![a, b]);
    assert!(e.state.pending_resolution.is_some(), "still parked");
    assert!(matches!(
        &e.state
            .pending_resolution
            .as_ref()
            .expect("scry ordering continuation")
            .continuation,
        ResolutionContinuation::LibraryPartition {
            stage: PendingLibraryPartitionStage::OrderTop,
            kind: PendingLibraryPartitionKind::Scry,
            ..
        }
    ));
    assert_eq!(
        e.state.players[0].hand.len(),
        hand_before_cast - 1,
        "the draw has not happened while the top-order choice is pending"
    );

    // Swap them. The submitted list is bottom-first, so the *last* entry ends up on top —
    // the same convention as Brainstorm's put-back.
    e.apply_scenario_command(0, &submit_resolution_choice(vec![b, a]))
        .expect("submit the order");

    assert!(e.state.pending_resolution.is_none());
    assert!(
        e.state.players[0].hand.contains(&a),
        "the last card submitted went on top, so it is the one drawn"
    );
    assert_eq!(
        e.state.players[0].library[0], b,
        "the other kept card sits under it, now the top of the library"
    );
    assert_eq!(
        e.state.players[0].hand.len(),
        hand_before_cast,
        "Preordain draws exactly once after top ordering"
    );
}

#[test]
fn preordain_bottoming_one_of_two_skips_the_ordering_step() {
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        7005,
        &[0, 1],
        20,
        island_deck_with("preordain"),
        true,
    )
    .expect("new");
    advance_to_main1_from_game_start(&mut e);
    ensure_in_hand(&mut e, 0, "preordain");
    let top = seat_on_top(&mut e, 0, &["grizzly_bears", "storm_crow"]);
    let (a, b) = (top[0], top[1]);
    let hand_before_cast = e.state.players[0].hand.len();

    let batch = cast_instant_and_resolve(&mut e, 0, "preordain", blue_mana());
    assert!(
        find_resolution_choice(&batch).is_some(),
        "Scry parks before drawing"
    );
    assert_eq!(e.state.players[0].hand.len(), hand_before_cast - 1);
    e.apply_scenario_command(0, &submit_resolution_choice(vec![a]))
        .expect("bottom the first");

    assert!(
        e.state.pending_resolution.is_none(),
        "one card left on top has only one possible order"
    );
    assert_eq!(*e.state.players[0].library.back().expect("library"), a);
    assert!(
        e.state.players[0].hand.contains(&b),
        "the kept card is drawn"
    );
    assert_eq!(
        e.state.players[0].hand.len(),
        hand_before_cast,
        "Preordain draws exactly once after Scry"
    );
}

#[test]
fn cavern_stomper_entry_scry2_program_orders_retained_cards() {
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        7011,
        &[0, 1],
        20,
        island_deck_with("cavern_stomper"),
        true,
    )
    .expect("new");
    advance_to_main1_from_game_start(&mut e);
    ensure_in_hand(&mut e, 0, "cavern_stomper");
    let top = seat_on_top(&mut e, 0, &["grizzly_bears", "storm_crow"]);
    let (a, b) = (top[0], top[1]);

    grant_pool(&mut e, 0);
    let slot = hand_index_for_card(&e, 0, "cavern_stomper");
    e.apply_scenario_command(0, &cast_spell(slot, vec![]))
        .expect("cast Cavern Stomper");
    let mut batch = e
        .apply_scenario_command(0, &pass())
        .expect("active player passes");
    let mut scry_choice = find_resolution_choice(&batch);
    for _ in 0..20 {
        if scry_choice.is_some() || e.state.stack.is_empty() || e.state.blocking_choice().is_some()
        {
            break;
        }
        let priority = e.state.priority_player_id();
        batch = e
            .apply_scenario_command(priority, &pass())
            .expect("pass priority until Cavern Stomper's trigger resolves");
        scry_choice = find_resolution_choice(&batch);
    }
    let req = scry_choice.expect("entry Scry 2 choice");
    assert_eq!(req.choice_kind(), ChoiceKind::LibraryTop);
    assert_eq!((req.min, req.max), (0, 2));
    assert_eq!(req.candidate_object_ids, vec![a, b]);

    let order_batch = e
        .apply_scenario_command(0, &submit_resolution_choice(vec![]))
        .expect("keep both cards on top");
    let req = find_resolution_choice(&order_batch).expect("Scry 2 ordering choice");
    assert_eq!((req.min, req.max), (2, 2));
    assert!(req.ordered);
    assert_eq!(req.candidate_object_ids, vec![a, b]);

    e.apply_scenario_command(0, &submit_resolution_choice(vec![b, a]))
        .expect("order the retained cards");
    assert!(e.state.pending_resolution.is_none());
    assert_eq!(
        e.state.players[0]
            .library
            .iter()
            .copied()
            .take(2)
            .collect::<Vec<_>>(),
        vec![a, b],
        "the submitted last card remains on top"
    );
}

#[test]
fn program_scry2_orders_kept_cards_then_draws_once() {
    let mut e = program_scry2_engine(7012);
    ensure_in_hand(&mut e, 0, "program_scry2_draw_fixture");
    let top = seat_on_top(&mut e, 0, &["test_scry_creature_a", "test_scry_creature_b"]);
    let (a, b) = (top[0], top[1]);

    let batch =
        cast_instant_and_resolve(&mut e, 0, "program_scry2_draw_fixture", ManaGift::default());
    let req = find_resolution_choice(&batch).expect("Program Scry 2 choice");
    assert_eq!(req.choice_kind(), ChoiceKind::LibraryTop);
    assert_eq!(req.candidate_object_ids, vec![a, b]);
    let hand_after_cast = e.state.players[0].hand.len();
    assert!(!e.state.players[0].hand.contains(&a));

    let order_batch = e
        .apply_scenario_command(0, &submit_resolution_choice(vec![]))
        .expect("keep both cards on top");
    let req = find_resolution_choice(&order_batch).expect("Program Scry 2 order choice");
    assert_eq!((req.min, req.max), (2, 2));
    assert_eq!(req.candidate_object_ids, vec![a, b]);
    assert_eq!(
        e.state.players[0].hand.len(),
        hand_after_cast,
        "draw waits for order"
    );

    e.apply_scenario_command(0, &submit_resolution_choice(vec![b, a]))
        .expect("order retained cards");
    assert!(e.state.pending_resolution.is_none());
    assert_eq!(
        e.state.players[0].hand.len(),
        hand_after_cast + 1,
        "tail draws once"
    );
    assert!(
        e.state.players[0].hand.contains(&a),
        "the new top card is drawn"
    );
    assert_eq!(e.state.players[0].library.front(), Some(&b));
}

#[test]
fn program_scry_fixed_counts_and_library_cohorts_follow_the_choice_table() {
    // (count, available cards, bottomed indices, optional submitted top order)
    // covers zero, empty and short libraries, no/all/some bottom choices, and exact top order.
    type ScryCase = (u32, usize, &'static [usize], Option<&'static [usize]>);
    let cases: &[ScryCase] = &[
        (0, 2, &[], None),
        (1, 0, &[], None),
        (1, 1, &[], None),
        (2, 2, &[0, 1], None),
        (3, 1, &[0], None),
        (3, 3, &[1], Some(&[2, 0])),
        (3, 3, &[], Some(&[2, 1, 0])),
    ];

    for (case_index, (count, library_size, bottom_indices, top_order_indices)) in
        cases.iter().enumerate()
    {
        let mut e = program_scry2_engine(7_030 + case_index as u64);
        let card_id = format!("program_scry_count_{count}_fixture");
        inject_card_into_hand(&mut e, 0, &card_id);
        e.state.players[0].library.clear();
        let card_ids = [
            "test_scry_creature_a",
            "test_scry_creature_b",
            "test_scry_creature_c",
        ];
        let library = seat_on_top(&mut e, 0, &card_ids[..*library_size]);
        let looked_count = (*count as usize).min(library.len());
        let life_before = e.state.players[0].life;
        assert_eq!(
            life_before, 20,
            "fixture starts at 20 life in case {case_index}"
        );
        let batch = cast_instant_and_resolve(&mut e, 0, &card_id, ManaGift::default());

        if looked_count == 0 {
            assert!(
                find_resolution_choice(&batch).is_none(),
                "case {case_index}"
            );
            assert!(e.state.pending_resolution.is_none(), "case {case_index}");
            assert_eq!(
                e.state.players[0]
                    .library
                    .iter()
                    .copied()
                    .collect::<Vec<_>>(),
                library,
                "zero or empty Scry leaves the library unchanged in case {case_index}"
            );
            assert_eq!(
                e.state.players[0].life,
                life_before + 1,
                "zero or empty Scry still resumes the resultless tail once in case {case_index}"
            );
            continue;
        }

        let req = find_resolution_choice(&batch).expect("nonempty Scry cohort asks");
        assert_eq!(
            e.state.players[0].life, life_before,
            "the Program tail stays parked while Scry awaits its choice in case {case_index}"
        );
        assert_eq!(
            req.candidate_object_ids,
            library[..looked_count],
            "case {case_index}"
        );
        assert_eq!(
            (req.min, req.max),
            (0, looked_count as u32),
            "case {case_index}"
        );
        let bottom = bottom_indices
            .iter()
            .map(|index| library[*index])
            .collect::<Vec<_>>();
        let kept = (0..looked_count)
            .filter(|index| !bottom_indices.contains(index))
            .map(|index| library[index])
            .collect::<Vec<_>>();
        let result = e
            .apply_scenario_command(0, &submit_resolution_choice(bottom.clone()))
            .expect("submit the Scry bottom cohort");

        let top = if let Some(order_indices) = top_order_indices {
            let order = order_indices
                .iter()
                .map(|index| library[*index])
                .collect::<Vec<_>>();
            let req = find_resolution_choice(&result).expect("multiple kept cards need order");
            assert_eq!(req.candidate_object_ids, kept, "case {case_index}");
            assert_eq!((req.min, req.max), (kept.len() as u32, kept.len() as u32));
            e.apply_scenario_command(0, &submit_resolution_choice(order.clone()))
                .expect("submit exact retained-card order");
            order.into_iter().rev().collect::<Vec<_>>()
        } else {
            assert!(
                find_resolution_choice(&result).is_none(),
                "case {case_index}"
            );
            assert!(e.state.pending_resolution.is_none(), "case {case_index}");
            kept
        };
        assert_eq!(
            e.state.players[0].life,
            life_before + 1,
            "the Program tail resumes exactly once after all Scry choices in case {case_index}"
        );
        let mut expected = top;
        expected.extend(bottom);
        assert_eq!(
            e.state.players[0]
                .library
                .iter()
                .copied()
                .collect::<Vec<_>>(),
            expected,
            "Scry uses the submitted order for retained top and bottom cards in case {case_index}"
        );
    }
}

#[test]
fn program_draw_then_scry_runs_the_draw_before_its_choice() {
    let mut e = program_scry2_engine(7_038);
    let spell = inject_card_into_hand(&mut e, 0, "program_draw_scry_fixture");
    let top = seat_on_top(&mut e, 0, &["test_scry_creature_a", "test_scry_creature_b"]);
    let (drawn, scried) = (top[0], top[1]);

    let batch =
        cast_instant_and_resolve(&mut e, 0, "program_draw_scry_fixture", ManaGift::default());
    let req = find_resolution_choice(&batch).expect("Scry follows the Draw operation");
    assert_eq!(req.candidate_object_ids, vec![scried]);
    assert!(e.state.players[0].hand.contains(&drawn));
    assert_eq!(e.state.objects[&spell].zone, Zone::Stack);
    assert!(!e.state.players[0].graveyard.contains(&spell));

    e.apply_scenario_command(0, &submit_resolution_choice(vec![scried]))
        .expect("bottom the card after drawing");
    assert!(e.state.pending_resolution.is_none());
    assert_eq!(e.state.players[0].library.back(), Some(&scried));
    assert_eq!(e.state.objects[&spell].zone, Zone::Graveyard);
    assert_eq!(
        e.state.players[0]
            .graveyard
            .iter()
            .filter(|&&object_id| object_id == spell)
            .count(),
        1,
        "the spell reaches its graveyard after Scry completes exactly once"
    );
}

#[test]
fn program_scry_nested_in_choose_and_if_resumes_the_outer_tail_once() {
    let mut e = program_scry2_engine(7_039);
    inject_card_into_hand(&mut e, 0, "program_nested_choose_if_scry_tail_fixture");
    let scried = seat_on_top(&mut e, 0, &["test_scry_creature_a"])[0];
    let life_before = e.state.players[0].life;
    let batch = cast_instant_and_resolve(
        &mut e,
        0,
        "program_nested_choose_if_scry_tail_fixture",
        ManaGift::default(),
    );
    let req = find_resolution_choice(&batch).expect("optional nested branch prompt");
    assert_eq!(req.choice_kind(), ChoiceKind::ResolutionBranch);

    let scry_batch = e
        .apply_scenario_command(0, &select_branch(0))
        .expect("select nested Scry branch");
    let req = find_resolution_choice(&scry_batch).expect("nested If reaches Scry");
    assert_eq!(req.candidate_object_ids, vec![scried]);
    assert_eq!(
        e.state.players[0].life, life_before,
        "outer tail is still parked"
    );

    e.apply_scenario_command(0, &submit_resolution_choice(vec![]))
        .expect("keep the scried card on top");
    assert!(e.state.pending_resolution.is_none());
    assert_eq!(
        e.state.players[0].life,
        life_before + 1,
        "outer tail runs once"
    );
}

#[test]
fn program_scry_preserves_a_prior_mill_cohort_for_choose_card() {
    let mut e = program_scry2_engine(7_040);
    inject_card_into_hand(&mut e, 0, "program_mill_scry_choose_card_fixture");
    let top = seat_on_top(&mut e, 0, &["test_scry_creature_a", "test_scry_creature_b"]);
    let (milled, scried) = (top[0], top[1]);

    let batch = cast_instant_and_resolve(
        &mut e,
        0,
        "program_mill_scry_choose_card_fixture",
        ManaGift::default(),
    );
    let req = find_resolution_choice(&batch).expect("Scry follows Mill");
    assert_eq!(req.candidate_object_ids, vec![scried]);
    assert!(e.state.players[0].graveyard.contains(&milled));

    let choose_batch = e
        .apply_scenario_command(0, &submit_resolution_choice(vec![]))
        .expect("finish Scry and continue to ChooseCard");
    let req = find_resolution_choice(&choose_batch).expect("prior Mill cohort remains available");
    assert_eq!(req.choice_kind(), ChoiceKind::GraveyardCards);
    assert_eq!(req.candidate_object_ids, vec![milled]);

    e.apply_scenario_command(0, &submit_resolution_choice(vec![milled]))
        .expect("choose the card from the prior Mill result");
    assert!(e.state.pending_resolution.is_none());
    assert!(e.state.players[0].hand.contains(&milled));
    assert_eq!(e.state.players[0].library.front(), Some(&scried));
}

#[test]
fn program_scry_rejects_invalid_and_replayed_choices_atomically() {
    let mut e = program_scry2_engine(7_041);
    let top = seat_on_top(
        &mut e,
        0,
        &[
            "test_scry_creature_a",
            "test_scry_creature_b",
            "test_scry_creature_c",
        ],
    );
    let (a, b, unoffered) = (top[0], top[1], top[2]);
    let draws_before = e.state.turn_history.current.player(0).cards_drawn;
    let batch =
        cast_instant_and_resolve(&mut e, 0, "program_scry2_draw_fixture", ManaGift::default());
    assert!(find_resolution_choice(&batch).is_some());
    let first_prompt = e
        .state
        .pending_resolution
        .as_ref()
        .expect("first Scry choice")
        .resolution_prompt_id
        .get();
    let first_pending = format!("{:?}", e.state.pending_resolution);
    let library_before = e.state.players[0]
        .library
        .iter()
        .copied()
        .collect::<Vec<_>>();

    for (label, player, ids) in [
        ("wrong player", 1, vec![a]),
        ("duplicate candidate", 0, vec![a, a]),
        ("unoffered candidate", 0, vec![a, unoffered]),
    ] {
        let state_before = format!("{:?}", e.state);
        assert!(
            e.apply_scenario_command(player, &submit_resolution_choice(ids))
                .is_err(),
            "{label} must be rejected"
        );
        assert_eq!(format!("{:?}", e.state), state_before, "{label} is atomic");
        assert_eq!(format!("{:?}", e.state.pending_resolution), first_pending);
        assert_eq!(
            e.state.players[0]
                .library
                .iter()
                .copied()
                .collect::<Vec<_>>(),
            library_before
        );
    }

    let order_batch = e
        .apply_scenario_command(0, &submit_resolution_choice(vec![]))
        .expect("keep both candidates on top");
    assert!(find_resolution_choice(&order_batch).is_some());
    let order_prompt = e
        .state
        .pending_resolution
        .as_ref()
        .expect("OrderTop choice")
        .resolution_prompt_id
        .get();
    let order_pending = format!("{:?}", e.state.pending_resolution);
    let library_before_order = e.state.players[0]
        .library
        .iter()
        .copied()
        .collect::<Vec<_>>();

    for (label, player, command) in [
        (
            "replayed first prompt",
            0,
            resolution_choice_with_prompt(first_prompt, vec![b, a]),
        ),
        (
            "wrong player at OrderTop",
            1,
            submit_resolution_choice(vec![b, a]),
        ),
        (
            "duplicate at OrderTop",
            0,
            submit_resolution_choice(vec![a, a]),
        ),
        (
            "unoffered card at OrderTop",
            0,
            submit_resolution_choice(vec![b, unoffered]),
        ),
    ] {
        let state_before = format!("{:?}", e.state);
        assert!(
            e.apply_scenario_command(player, &command).is_err(),
            "{label}"
        );
        assert_eq!(format!("{:?}", e.state), state_before, "{label} is atomic");
        assert_eq!(format!("{:?}", e.state.pending_resolution), order_pending);
        assert_eq!(
            e.state.players[0]
                .library
                .iter()
                .copied()
                .collect::<Vec<_>>(),
            library_before_order
        );
    }

    e.apply_scenario_command(0, &submit_resolution_choice(vec![b, a]))
        .expect("valid OrderTop submission after rejected attempts");
    assert!(e.state.pending_resolution.is_none());
    assert_eq!(
        e.state.turn_history.current.player(0).cards_drawn,
        draws_before + 1,
        "the Program Draw tail resumes exactly once"
    );
    let after_success = format!("{:?}", e.state);
    assert!(
        e.apply_scenario_command(0, &resolution_choice_with_prompt(order_prompt, vec![b, a]))
            .is_err(),
        "a consumed OrderTop prompt cannot be replayed"
    );
    assert_eq!(format!("{:?}", e.state), after_success);
}

#[test]
fn program_scry_rejects_stale_generations_at_both_choice_stages() {
    for (seed, order_stage) in [(7_042, false), (7_043, true)] {
        let mut e = program_scry2_engine(seed);
        let top = seat_on_top(&mut e, 0, &["test_scry_creature_a", "test_scry_creature_b"]);
        let (a, b) = (top[0], top[1]);
        cast_instant_and_resolve(&mut e, 0, "program_scry2_draw_fixture", ManaGift::default());
        if order_stage {
            e.apply_scenario_command(0, &submit_resolution_choice(vec![]))
                .expect("keep both cards and park at OrderTop");
        }
        let pending_before = format!("{:?}", e.state.pending_resolution);
        move_library_to_graveyard(&mut e, 0, a);
        move_graveyard_to_library(&mut e, 0, a);
        let library_before = e.state.players[0]
            .library
            .iter()
            .copied()
            .collect::<Vec<_>>();
        let stale_answer = if order_stage { vec![b, a] } else { vec![a] };
        let error = e
            .apply_scenario_command(0, &submit_resolution_choice(stale_answer))
            .expect_err("returned candidate has a stale physical generation");
        assert!(error.to_string().contains("stale library-partition cohort"));
        assert_eq!(format!("{:?}", e.state.pending_resolution), pending_before);
        assert_eq!(
            e.state.players[0]
                .library
                .iter()
                .copied()
                .collect::<Vec<_>>(),
            library_before
        );
    }
}

#[test]
fn program_scry_observer_departure_preserves_the_frame_through_both_choices() {
    let mut e = program_scry_three_player_engine(7_044);
    let top = seat_on_top(&mut e, 0, &["test_scry_creature_a", "test_scry_creature_b"]);
    let (a, b) = (top[0], top[1]);
    let draws_before = e.state.turn_history.current.player(0).cards_drawn;
    let batch = cast_three_player_scry2_until_choice(&mut e);
    assert!(find_resolution_choice(&batch).is_some(), "Scry is parked");
    let first_pending = format!("{:?}", e.state.pending_resolution);

    e.apply_scenario_command(1, &concede())
        .expect("a nondeciding observer can concede");
    assert!(e.state.players[1].has_lost);
    assert_eq!(format!("{:?}", e.state.pending_resolution), first_pending);
    let order_batch = e
        .apply_scenario_command(0, &submit_resolution_choice(vec![]))
        .expect("the original chooser retains the Scry decision");
    let order = find_resolution_choice(&order_batch).expect("OrderTop remains available");
    assert_eq!(order.candidate_object_ids, vec![a, b]);
    assert!(e.state.pending_resolution.is_some());

    e.apply_scenario_command(0, &submit_resolution_choice(vec![b, a]))
        .expect("finish the retained-card order");
    assert!(e.state.pending_resolution.is_none());
    assert_eq!(
        e.state.turn_history.current.player(0).cards_drawn,
        draws_before + 1,
        "observer departure preserves the Program frame and its Draw tail"
    );
}

#[test]
fn program_scry_controller_departure_clears_both_choice_stages() {
    for (seed, order_stage) in [(7_045, false), (7_046, true)] {
        let mut e = program_scry_three_player_engine(seed);
        seat_on_top(&mut e, 0, &["test_scry_creature_a", "test_scry_creature_b"]);
        let draws_before = e.state.turn_history.current.player(0).cards_drawn;
        let batch = cast_three_player_scry2_until_choice(&mut e);
        assert!(find_resolution_choice(&batch).is_some());
        if order_stage {
            e.apply_scenario_command(0, &submit_resolution_choice(vec![]))
                .expect("keep both and enter OrderTop");
            assert!(e.state.pending_resolution.is_some());
        }

        e.apply_scenario_command(0, &concede())
            .expect("the Program's original controller concedes while Scry is parked");
        assert!(e.state.players[0].has_lost);
        assert!(
            !e.state.is_terminal(),
            "two players remain in the three-player game"
        );
        assert!(e.state.pending_resolution.is_none());
        assert!(e.state.stack.is_empty());
        assert_eq!(
            e.state.turn_history.current.player(0).cards_drawn,
            draws_before,
            "controller departure drops the Program continuation at stage {order_stage}"
        );
    }
}

#[test]
fn program_scry_controller_concession_clears_the_two_player_terminal_state() {
    let mut e = program_scry2_engine(7_047);
    seat_on_top(&mut e, 0, &["test_scry_creature_a", "test_scry_creature_b"]);
    let draws_before = e.state.turn_history.current.player(0).cards_drawn;
    let batch =
        cast_instant_and_resolve(&mut e, 0, "program_scry2_draw_fixture", ManaGift::default());
    assert!(find_resolution_choice(&batch).is_some());

    e.apply_scenario_command(0, &concede())
        .expect("controller concedes during Program Scry");
    assert!(e.state.is_terminal());
    assert_eq!(e.state.winner(), Some(1));
    assert!(e.state.pending_resolution.is_none());
    assert!(e.state.stack.is_empty());
    assert_eq!(
        e.state.turn_history.current.player(0).cards_drawn,
        draws_before,
        "terminal cleanup does not resume the Draw tail"
    );
}

// ---------------------------------------------------------------------------
// Edge cases

#[test]
fn scry_with_an_empty_library_does_not_park_and_the_draw_still_runs() {
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        7006,
        &[0, 1],
        20,
        island_deck_with("opt"),
        true,
    )
    .expect("new");
    advance_to_main1_from_game_start(&mut e);
    ensure_in_hand(&mut e, 0, "opt");
    e.state.players[0].library.clear();

    give_mana(&mut e, 0, blue_mana());
    let idx = hand_index_for_card(&e, 0, "opt");
    e.apply_scenario_command(0, &cast_spell(idx, vec![]))
        .expect("cast");
    e.apply_scenario_command(0, &pass()).expect("p0 pass");
    let batch = e
        .apply_scenario_command(1, &pass())
        .expect("p1 pass — resolves");

    assert!(
        find_resolution_choice(&batch).is_none(),
        "nothing to look at, so no choice is asked for"
    );
    assert!(e.state.pending_resolution.is_none());
    // The `Draw` after the scry still runs, and drawing from an empty library loses the game
    // (CR 104.3c) — the point being that the second effect was not skipped.
    assert!(
        e.state.players[0].has_lost,
        "the draw ran and decked the caster"
    );
}

#[test]
fn scry_rejects_illegal_submissions_without_mutating_the_library() {
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        7007,
        &[0, 1],
        20,
        island_deck_with("preordain"),
        true,
    )
    .expect("new");
    advance_to_main1_from_game_start(&mut e);
    ensure_in_hand(&mut e, 0, "preordain");
    let top = seat_on_top(&mut e, 0, &["grizzly_bears", "storm_crow", "hill_giant"]);
    let (a, b, third) = (top[0], top[1], top[2]);

    cast_instant_and_resolve(&mut e, 0, "preordain", blue_mana());
    let library_before: Vec<u32> = e.state.players[0].library.iter().copied().collect();

    for (label, player, choice) in [
        ("wrong player", 1, vec![a]),
        ("card below the scried window", 0, vec![third]),
        ("more cards than were looked at", 0, vec![a, b, third]),
        ("the same card twice", 0, vec![a, a]),
    ] {
        assert!(
            e.apply_scenario_command(player, &submit_resolution_choice(choice))
                .is_err(),
            "{label} must be rejected"
        );
        assert!(
            e.state.pending_resolution.is_some(),
            "{label}: the choice is still outstanding"
        );
        assert_eq!(
            e.state.players[0]
                .library
                .iter()
                .copied()
                .collect::<Vec<u32>>(),
            library_before,
            "{label}: the library is untouched"
        );
    }

    // A legal answer still goes through afterwards.
    e.apply_scenario_command(0, &submit_resolution_choice(vec![a, b]))
        .expect("valid submission after the rejections");
    assert!(e.state.pending_resolution.is_none());
}

#[test]
fn scry_rejects_a_stale_choose_destination_without_consuming_the_choice() {
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        7009,
        &[0, 1],
        20,
        island_deck_with("opt"),
        true,
    )
    .expect("new");
    advance_to_main1_from_game_start(&mut e);
    ensure_in_hand(&mut e, 0, "opt");
    let scried = seat_on_top(&mut e, 0, &["grizzly_bears"])[0];

    cast_instant_and_resolve(&mut e, 0, "opt", blue_mana());
    let pending_before = format!("{:?}", e.state.pending_resolution);
    let library_before: Vec<u32> = e.state.players[0].library.iter().copied().collect();

    // The same object id returns to the library as a new physical incarnation. A published
    // answer for the old look must fail closed before Scry moves it or resumes its draw tail.
    move_library_to_graveyard(&mut e, 0, scried);
    move_graveyard_to_library(&mut e, 0, scried);
    let err = e
        .apply_scenario_command(0, &submit_resolution_choice(vec![scried]))
        .expect_err("a stale Scry candidate must be rejected");

    assert!(err.to_string().contains("stale library-partition cohort"));
    assert_eq!(
        format!("{:?}", e.state.pending_resolution),
        pending_before,
        "the stale answer preserves the parked choice atomically"
    );
    assert_eq!(
        e.state.players[0]
            .library
            .iter()
            .copied()
            .collect::<Vec<_>>(),
        library_before,
        "the stale answer does not move the returned card or run Scry's draw tail"
    );
}

#[test]
fn preordain_rejects_a_stale_order_top_without_consuming_the_choice() {
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        7010,
        &[0, 1],
        20,
        island_deck_with("preordain"),
        true,
    )
    .expect("new");
    advance_to_main1_from_game_start(&mut e);
    ensure_in_hand(&mut e, 0, "preordain");
    let top = seat_on_top(&mut e, 0, &["grizzly_bears", "storm_crow"]);

    cast_instant_and_resolve(&mut e, 0, "preordain", blue_mana());
    e.apply_scenario_command(0, &submit_resolution_choice(vec![]))
        .expect("keep both cards on top");
    assert!(matches!(
        &e.state
            .pending_resolution
            .as_ref()
            .expect("order-top continuation")
            .continuation,
        ResolutionContinuation::LibraryPartition {
            stage: PendingLibraryPartitionStage::OrderTop,
            kind: PendingLibraryPartitionKind::Scry,
            ..
        }
    ));
    let pending_before = format!("{:?}", e.state.pending_resolution);
    let library_before: Vec<u32> = e.state.players[0].library.iter().copied().collect();

    // OrderTop validates the remaining presentation candidates, not the cards already sent to
    // Scry's bottom. The first retained card is returned with a fresh generation here.
    move_library_to_graveyard(&mut e, 0, top[0]);
    move_graveyard_to_library(&mut e, 0, top[0]);
    let err = e
        .apply_scenario_command(0, &submit_resolution_choice(vec![top[1], top[0]]))
        .expect_err("a stale retained Scry candidate must be rejected");

    assert!(err.to_string().contains("stale library-partition cohort"));
    assert_eq!(
        format!("{:?}", e.state.pending_resolution),
        pending_before,
        "the stale order preserves the second Scry choice atomically"
    );
    assert_eq!(
        e.state.players[0]
            .library
            .iter()
            .copied()
            .collect::<Vec<_>>(),
        library_before,
        "the stale order does not rearrange the retained cards or draw"
    );
}

#[test]
fn scry_is_deterministic_for_the_same_seed_and_choices() {
    fn play() -> Vec<u32> {
        let mut e = GameEngine::new(
            tricerules_cards::registry::global(),
            7008,
            &[0, 1],
            20,
            island_deck_with("preordain"),
            true,
        )
        .expect("new");
        advance_to_main1_from_game_start(&mut e);
        ensure_in_hand(&mut e, 0, "preordain");
        let top = seat_on_top(&mut e, 0, &["grizzly_bears", "storm_crow"]);
        cast_instant_and_resolve(&mut e, 0, "preordain", blue_mana());
        e.apply_scenario_command(0, &submit_resolution_choice(vec![top[0]]))
            .expect("bottom one");
        e.state.players[0].library.iter().copied().collect()
    }
    assert_eq!(play(), play(), "same seed + same choices ⇒ same library");
}
