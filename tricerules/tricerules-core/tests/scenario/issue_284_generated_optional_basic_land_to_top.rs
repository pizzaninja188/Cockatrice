//! Issue #284 — Campus Guide and Spider-Bot use an optional typed ETB branch to search the
//! controller's library for exactly one basic land, reveal it, shuffle, and put it on top.
//!
//! CR 603.6a governs the ETB trigger; CR 701.20 governs revealing, CR 701.23 and 701.23b govern
//! searching and fail-to-find, CR 701.24 and 701.24b govern shuffling and excluding found cards
//! from that shuffle, and CR 401 governs the library and top-of-library placement. The scenarios
//! also exercise the engine's private candidate publication and current-generation validation.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_cards::CardRegistry;
use tricerules_core::{EngineDeck, EngineError, Zone};
use tricerules_proto::ruled::v1::{
    ruled_command::Cmd, ChoiceKind, ResolutionChoiceDecision, RuledCommand, SubmitResolutionChoice,
};

fn direct_hand_search_fixture(filter: &str, draw_tail: bool) -> String {
    let draw_step = if draw_tail {
        r#"
            (id: "tail_draw", bind: [], input: None, kind: Perform(Draw(
              who: Controller, count: 1,
            ))),"#
    } else {
        ""
    };
    format!(
        r#"(
          id: "direct_hand_search_fixture",
          name: "Direct Hand Search Fixture",
          face_id: "direct_hand_search_fixture",
          mana_cost: "{{0}}",
          types: ["Instant"],
          spell_program: Some((body: (steps: [
            (id: "choose_search", bind: [], input: None, kind: Choose(
              choice: "search", chooser: Controller, optional: true,
              branches: [(branch_id: "search_for_card", presentation: Fallback,
                cost: None, requirement: Always, body: (steps: [
                  (id: "search_hand", bind: [], input: None, kind: Perform(SearchLibrary(
                    who: Controller, optional: false, count: 1, count_by_cast_cost: None,
                    filter: Some({filter}),
                    selection_constraint: None, slots: [], zones: Fixed([Library]),
                    destination: Hand, conditional_destination: None, shuffle: true, reveal: true,
                    selection_bounds: Some((min: 0, max: 1)),
                  )))
                ], yields: []))],
              otherwise: (steps: [], yields: []),
            )),
            {draw_step}
            (id: "tail_token", bind: [], input: None, kind: Perform(CreateTokens(
              token: "treasure", count: 1, who: Controller, tapped: false,
              sacrifice_timing: None,
            ))),
          ], yields: []), target_bindings: [])),
        )"#
    )
}

const LEGAL_TEST_COMMANDER: &str = r#"(
  id: "legal_test_commander", name: "Legal Test Commander", face_id: "legal_test_commander",
  mana_cost: "{0}", types: ["Legendary", "Creature", "Human"], power: 1, toughness: 1,
)"#;

const DIRECT_HAND_SEARCH_DEPARTURE_FIXTURE: &str = r#"(
  id: "direct_hand_search_departure_fixture",
  name: "Direct Hand Search Departure Fixture",
  face_id: "direct_hand_search_departure_fixture",
  mana_cost: "{0}",
  types: ["Instant"],
  spell_program: Some((body: (steps: [
    (id: "choose_search", bind: [], input: None, kind: Choose(
      choice: "search", chooser: Controller, optional: true,
      branches: [(branch_id: "search_for_card", presentation: Fallback,
        cost: None, requirement: Always, body: (steps: [
          (id: "search_hand", bind: [], input: None, kind: Perform(SearchLibrary(
            who: Controller, optional: false, count: 1, count_by_cast_cost: None,
            filter: Some((exact_name: Some("Legal Test Commander"))),
            selection_constraint: None, slots: [], zones: Fixed([Library]),
            destination: Hand, conditional_destination: None, shuffle: true, reveal: true,
            selection_bounds: Some((min: 0, max: 1)),
          )))
        ], yields: []))],
      otherwise: (steps: [], yields: []),
    )),
    (id: "tail_damage", bind: [], input: None, kind: Perform(DamageTarget(
      amount: 1, target: (kind: OpponentPlayer),
    ))),
  ], yields: []), target_bindings: [
    (instruction_id: "tail_damage", role: Target, target_group_index: 0),
  ])),
  targeting: Some((groups: [(min: 1, max: 1,
    prompt: "Choose a surviving opponent", effect_indices: [])])),
)"#;

fn direct_hand_search_controller_departure_engine(seed: u64) -> GameEngine {
    let registry = Box::leak(Box::new(
        CardRegistry::from_chunks_and_tokens(
            &[
                DIRECT_HAND_SEARCH_DEPARTURE_FIXTURE,
                LEGAL_TEST_COMMANDER,
                include_str!("../../../tricerules-cards/data/forest.ron"),
            ],
            &[],
        )
        .expect("controller-departure direct Hand search fixture registry"),
    ));
    let decks = Some(
        (0..3)
            .map(|player| EngineDeck {
                mainboard: vec!["forest".into(); 30],
                commanders: if player == 0 {
                    vec!["legal_test_commander".into()]
                } else {
                    vec![]
                },
            })
            .collect(),
    );
    let mut engine =
        GameEngine::new_with_commander_decks(registry, seed, &[0, 1, 2], 20, decks, true)
            .expect("three-player controller-departure Hand search engine");
    advance_to_main1_from_game_start(&mut engine);
    inject_card_into_hand(&mut engine, 0, "direct_hand_search_departure_fixture");
    let commander = engine.state.players[0].command_zone[0];
    engine.state.players[0]
        .command_zone
        .retain(|object_id| *object_id != commander);
    engine.state.players[0].library.push_back(commander);
    engine.state.objects.get_mut(&commander).unwrap().zone = Zone::Library;
    *engine
        .state
        .zone_change_generation
        .entry(commander)
        .or_default() += 1;
    engine
}

fn direct_hand_search_commander_engine(seed: u64, filter: &str) -> GameEngine {
    direct_hand_search_engine(seed, filter, true)
}

fn direct_hand_search_without_draw_engine(seed: u64, filter: &str) -> GameEngine {
    direct_hand_search_engine(seed, filter, false)
}

fn direct_hand_search_engine(seed: u64, filter: &str, draw_tail: bool) -> GameEngine {
    direct_hand_search_engine_with_players(seed, filter, draw_tail, &[0, 1])
}

fn direct_hand_search_engine_with_players(
    seed: u64,
    filter: &str,
    draw_tail: bool,
    player_ids: &[i32],
) -> GameEngine {
    let search_program = direct_hand_search_fixture(filter, draw_tail);
    let registry = Box::leak(Box::new(
        CardRegistry::from_chunks_and_tokens(
            &[
                &search_program,
                LEGAL_TEST_COMMANDER,
                include_str!("../../../tricerules-cards/data/forest.ron"),
                include_str!("../../../tricerules-cards/data/island.ron"),
                include_str!("../../../tricerules-cards/data/wan_shi_tong,_librarian.ron"),
            ],
            &[include_str!(
                "../../../tricerules-cards/data/tokens/treasure.ron"
            )],
        )
        .expect("direct Hand search commander fixture registry"),
    ));
    let mut decks = vec![
        EngineDeck {
            mainboard: [
                vec!["direct_hand_search_fixture".into()],
                vec!["forest".into(); 7],
            ]
            .concat(),
            commanders: vec!["legal_test_commander".into()],
        },
        EngineDeck {
            mainboard: [
                vec!["wan_shi_tong,_librarian".into()],
                vec!["island".into(); 8],
            ]
            .concat(),
            commanders: vec![],
        },
    ];
    decks.extend((2..player_ids.len()).map(|_| EngineDeck {
        mainboard: vec!["island".into(); 9],
        commanders: vec![],
    }));
    let mut engine =
        GameEngine::new_with_commander_decks(registry, seed, player_ids, 20, Some(decks), true)
            .expect("direct Hand search commander engine");
    advance_to_main1_from_game_start(&mut engine);
    let commander = engine.state.players[0].command_zone[0];
    engine.state.players[0]
        .command_zone
        .retain(|object_id| *object_id != commander);
    engine.state.players[0].library.push_back(commander);
    engine.state.objects.get_mut(&commander).unwrap().zone = Zone::Library;
    *engine
        .state
        .zone_change_generation
        .entry(commander)
        .or_default() += 1;
    engine
}

fn install_search_observer(engine: &mut GameEngine) -> u32 {
    let wan = engine.state.players[1]
        .library
        .iter()
        .chain(engine.state.players[1].hand.iter())
        .copied()
        .find(|object_id| engine.state.objects[object_id].card_id == "wan_shi_tong,_librarian")
        .expect("Wan Shi Tong in opponent library or hand");
    engine.state.players[1]
        .library
        .retain(|object_id| *object_id != wan);
    engine.state.players[1]
        .hand
        .retain(|object_id| *object_id != wan);
    engine.state.players[1].battlefield.push(wan);
    let observer = engine.state.objects.get_mut(&wan).unwrap();
    observer.zone = Zone::Battlefield;
    observer.controller = 1;
    observer.base_controller = 1;
    *engine.state.zone_change_generation.entry(wan).or_default() += 1;
    wan
}

fn issue_284_engine(seed: u64, card_id: &str, basic: &str) -> GameEngine {
    let decks = Some(vec![deck_with(basic, &[card_id]), deck_with("island", &[])]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("issue #284 engine");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn select_branch(index: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
            resolution_prompt_id: 0,
            decision: ResolutionChoiceDecision::SelectBranch as i32,
            selected_branch_index: index,
            ..Default::default()
        })),
    }
}

fn decline_branch() -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
            resolution_prompt_id: 0,
            decision: ResolutionChoiceDecision::Decline as i32,
            ..Default::default()
        })),
    }
}

fn open_branch(engine: &mut GameEngine, card_id: &str) -> (u32, ResolutionChoiceRequired) {
    let source = move_ready_to_battlefield(engine, 0, card_id);
    let batch = {
        engine
            .apply_scenario_command(0, &pass())
            .expect("controller passes ETB");
        engine
            .apply_scenario_command(1, &pass())
            .expect("opponent passes ETB")
    };
    let choice = find_resolution_choice(&batch).expect("optional ETB branch");
    (source, choice)
}

fn move_library_to_graveyard(engine: &mut GameEngine, player: usize, object_id: u32) {
    engine.state.players[player]
        .library
        .retain(|id| *id != object_id);
    engine.state.players[player].graveyard.push(object_id);
    engine.state.objects.get_mut(&object_id).unwrap().zone = Zone::Graveyard;
    *engine
        .state
        .zone_change_generation
        .entry(object_id)
        .or_default() += 1;
}

fn move_graveyard_to_library(engine: &mut GameEngine, player: usize, object_id: u32) {
    engine.state.players[player]
        .graveyard
        .retain(|id| *id != object_id);
    engine.state.players[player].library.push_back(object_id);
    engine.state.objects.get_mut(&object_id).unwrap().zone = Zone::Library;
    *engine
        .state
        .zone_change_generation
        .entry(object_id)
        .or_default() += 1;
}

fn has_log(batch: &RuledEventBatch, text: &str) -> bool {
    batch
        .events
        .iter()
        .any(|event| matches!(&event.ev, Some(Ev::Log(log)) if log.text == text))
}

#[test]
fn issue_284_optional_search_offers_readable_search_label_for_both_cards() {
    for (seed, card_id) in [(284_000, "campus_guide"), (284_001, "spider-bot")] {
        let mut engine = issue_284_engine(seed, card_id, "forest");
        let (_source, choice) = open_branch(&mut engine, card_id);

        assert_eq!(choice.resolution_branches.len(), 1, "{card_id}");
        assert_eq!(choice.resolution_branches[0].branch_index, 0, "{card_id}");
        assert_eq!(
            choice.resolution_branches[0].label, "Search",
            "{card_id} should offer the readable Search action"
        );
    }
}

#[test]
fn issue_284_declining_the_etb_branch_does_not_search_or_shuffle() {
    let mut engine = issue_284_engine(284_001, "campus_guide", "forest");
    let source = move_ready_to_battlefield(&mut engine, 0, "campus_guide");
    let library_before: Vec<_> = engine.state.players[0].library.iter().copied().collect();
    engine
        .apply_scenario_command(0, &pass())
        .expect("controller passes ETB");
    let branch_batch = engine
        .apply_scenario_command(1, &pass())
        .expect("opponent passes ETB");
    let branch = find_resolution_choice(&branch_batch).expect("optional ETB branch");
    assert_eq!(branch.deciding_player_id, 0);
    assert_eq!(branch.choice_kind(), ChoiceKind::ResolutionBranch);
    assert_eq!((branch.min, branch.max), (0, 1));
    assert!(branch.candidate_object_ids.is_empty());

    assert!(
        engine.apply_scenario_command(1, &decline_branch()).is_err(),
        "only the controller may decline the branch"
    );
    let completion = engine
        .apply_scenario_command(0, &decline_branch())
        .expect("decline the optional branch");

    assert_eq!(engine.state.objects[&source].zone, Zone::Battlefield);
    assert_eq!(
        engine.state.players[0]
            .library
            .iter()
            .copied()
            .collect::<Vec<_>>(),
        library_before
    );
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
    assert!(!completion
        .events
        .iter()
        .any(|event| matches!(&event.ev, Some(Ev::CardsRevealed(_)))));
    assert!(!completion
        .events
        .iter()
        .any(|event| matches!(&event.ev, Some(Ev::Log(log)) if log.text.contains("shuffles"))));
}

#[test]
fn issue_284_search_is_private_controller_only_and_basic_land_only() {
    let mut engine = issue_284_engine(284_002, "spider-bot", "forest");
    let own_basic = inject_library_card(&mut engine, 0, "forest");
    let own_nonbasic = inject_library_card(&mut engine, 0, "taiga");
    let opposing_basic = inject_library_card(&mut engine, 1, "forest");
    let (_source, branch) = open_branch(&mut engine, "spider-bot");
    assert_eq!(branch.deciding_player_id, 0);
    assert_eq!(branch.choice_kind(), ChoiceKind::ResolutionBranch);
    assert!(branch.public_reveal.is_none());

    assert!(
        engine.apply_scenario_command(1, &select_branch(0)).is_err(),
        "only the controller may select the ETB branch"
    );
    let search_batch = engine
        .apply_scenario_command(0, &select_branch(0))
        .expect("select the search branch");
    let search = find_resolution_choice(&search_batch).expect("private library search");
    assert_eq!(search.deciding_player_id, 0);
    assert_eq!(search.choice_kind(), ChoiceKind::LibrarySearch);
    assert_eq!((search.min, search.max), (0, 1));
    assert!(search.public_reveal.is_none());
    assert!(search.candidate_object_ids.contains(&own_basic));
    assert!(!search.candidate_object_ids.contains(&own_nonbasic));
    assert!(!search.candidate_object_ids.contains(&opposing_basic));
    assert!(search
        .candidate_object_ids
        .iter()
        .all(|object_id| engine.state.objects[object_id].card_id == "forest"));

    assert!(matches!(
        engine.apply_scenario_command(1, &submit_resolution_choice(vec![own_basic])),
        Err(EngineError::Illegal(_))
    ));
    assert!(engine.state.pending_resolution.is_some());
}

#[test]
fn issue_284_accepting_search_reveals_shuffles_and_puts_the_basic_land_on_top() {
    let mut engine = issue_284_engine(284_003, "campus_guide", "forest");
    let basic = inject_library_card(&mut engine, 0, "forest");
    let nonbasic = inject_library_card(&mut engine, 0, "taiga");
    let (_source, _) = open_branch(&mut engine, "campus_guide");
    let search_batch = engine
        .apply_scenario_command(0, &select_branch(0))
        .expect("select search branch");
    let search = find_resolution_choice(&search_batch).expect("search choice");
    assert!(search.candidate_object_ids.contains(&basic));
    assert!(!search.candidate_object_ids.contains(&nonbasic));

    let completion = engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![basic]))
        .expect("choose the basic land");
    assert_eq!(engine.state.objects[&basic].zone, Zone::Library);
    assert_eq!(engine.state.players[0].library.front(), Some(&basic));
    assert!(engine.state.players[0].library.contains(&nonbasic));
    assert!(completion
        .events
        .iter()
        .any(|event| matches!(&event.ev, Some(Ev::CardsRevealed(reveal)) if reveal.cards.iter().any(|card| card.object_id == basic))));
    assert!(has_log(&completion, "P0 shuffles their library."));
    assert!(has_log(&completion, "P0 reveals Forest."));
    assert!(has_log(
        &completion,
        "P0 puts Forest on top of their library."
    ));
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
}

#[test]
fn resultless_direct_hand_search_accepts_empty_library_and_runs_tail() {
    let mut engine =
        direct_hand_search_without_draw_engine(284_102, "(card_type: Some(BasicLand))");
    let wan = install_search_observer(&mut engine);
    ensure_in_hand(&mut engine, 0, "direct_hand_search_fixture");
    let commander = engine.state.players[0]
        .library
        .iter()
        .copied()
        .find(|object_id| engine.state.objects[object_id].card_id == "legal_test_commander")
        .expect("declared commander in library");
    engine.state.players[0]
        .library
        .retain(|object_id| *object_id != commander);
    engine.state.players[0].command_zone.push(commander);
    engine.state.objects.get_mut(&commander).unwrap().zone = Zone::Command;
    *engine
        .state
        .zone_change_generation
        .entry(commander)
        .or_default() += 1;
    let remaining: Vec<_> = engine.state.players[0].library.drain(..).collect();
    for object_id in remaining {
        engine.state.players[0].graveyard.push(object_id);
        engine.state.objects.get_mut(&object_id).unwrap().zone = Zone::Graveyard;
        *engine
            .state
            .zone_change_generation
            .entry(object_id)
            .or_default() += 1;
    }
    let slot = hand_index_for_card(&engine, 0, "direct_hand_search_fixture");
    engine
        .apply_scenario_command(0, &cast_spell(slot, Vec::new()))
        .expect("cast the direct Hand search fixture");
    assert!(
        engine.state.pending_resolution.is_none(),
        "a newly cast test spell should be on the stack before priority passes: {:?}",
        engine.state.pending_resolution
    );
    let branch_batch = pass_both_players_with_batch(&mut engine);
    assert!(find_resolution_choice(&branch_batch).is_some());
    let completion = engine
        .apply_scenario_command(0, &select_branch(0))
        .expect("select search despite an empty library");
    assert!(
        find_resolution_choice(&completion).is_none(),
        "an empty library completes without presenting a card or owner choice"
    );
    assert!(completion
        .events
        .iter()
        .any(|event| matches!(&event.ev, Some(Ev::Log(log)) if log.text == "P0 finds no card.")));
    assert_eq!(
        completion
            .events
            .iter()
            .filter(|event| matches!(&event.ev, Some(Ev::Log(log)) if log.text == "P0 shuffles their library."))
            .count(),
        1,
        "accepted search shuffles even though the library is empty"
    );
    assert!(!completion
        .events
        .iter()
        .any(|event| matches!(&event.ev, Some(Ev::CardsRevealed(_)))));
    assert_eq!(
        engine.state.players[0]
            .battlefield
            .iter()
            .filter(|object_id| engine.state.objects[object_id].card_id == "treasure")
            .count(),
        1,
        "the resultless parent tail executes exactly once"
    );
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(
        engine.state.stack.len(),
        1,
        "one LibrarySearched observer (pending: {}, staged groups: {})",
        engine.state.pending_triggers.len(),
        engine.state.staged_trigger_groups.len()
    );
    assert_eq!(engine.state.stack[0].source_permanent_id, Some(wan));
}

#[test]
fn resultless_direct_hand_search_can_fail_to_find_with_nonempty_library() {
    let mut engine = direct_hand_search_commander_engine(284_104, "(card_type: Some(BasicLand))");
    let wan = install_search_observer(&mut engine);
    let forest = engine.state.players[0]
        .library
        .iter()
        .copied()
        .find(|object_id| engine.state.objects[object_id].card_id == "forest")
        .expect("a matching basic land stays in the nonempty library");
    let slot = hand_index_for_card(&engine, 0, "direct_hand_search_fixture");
    let hand_before_cast = engine.state.players[0].hand.len();
    engine
        .apply_scenario_command(0, &cast_spell(slot, Vec::new()))
        .expect("cast the direct Hand search fixture");
    let branch_batch = pass_both_players_with_batch(&mut engine);
    assert!(find_resolution_choice(&branch_batch).is_some());
    engine
        .apply_scenario_command(0, &select_branch(0))
        .expect("select the search branch");
    let completion = engine
        .apply_scenario_command(0, &submit_resolution_choice(Vec::new()))
        .expect("accept the search and fail to find");

    assert_eq!(engine.state.objects[&forest].zone, Zone::Library);
    assert!(engine.state.players[0].library.contains(&forest));
    assert!(completion
        .events
        .iter()
        .any(|event| matches!(&event.ev, Some(Ev::Log(log)) if log.text == "P0 finds no card.")));
    assert_eq!(
        completion
            .events
            .iter()
            .filter(|event| matches!(&event.ev, Some(Ev::Log(log)) if log.text == "P0 shuffles their library."))
            .count(),
        1
    );
    assert!(!completion
        .events
        .iter()
        .any(|event| matches!(&event.ev, Some(Ev::CardsRevealed(_)))));
    assert_eq!(engine.state.players[0].hand.len(), hand_before_cast);
    assert_eq!(
        engine.state.players[0]
            .battlefield
            .iter()
            .filter(|object_id| engine.state.objects[object_id].card_id == "treasure")
            .count(),
        1
    );
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(
        engine.state.stack.len(),
        1,
        "one LibrarySearched observer (pending: {}, staged groups: {})",
        engine.state.pending_triggers.len(),
        engine.state.staged_trigger_groups.len()
    );
    assert_eq!(engine.state.stack[0].source_permanent_id, Some(wan));
}

#[test]
fn issue_284_rejects_stale_noncandidate_and_unauthorized_library_choices() {
    let mut engine = issue_284_engine(284_004, "spider-bot", "forest");
    let stale = inject_library_card(&mut engine, 0, "forest");
    let noncandidate = inject_library_card(&mut engine, 0, "taiga");
    let unauthorized = inject_library_card(&mut engine, 1, "forest");
    let (_source, _) = open_branch(&mut engine, "spider-bot");
    engine
        .apply_scenario_command(0, &select_branch(0))
        .expect("select search branch");
    let search = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("search continuation")
        .presentation
        .clone();
    assert!(search.candidates.contains(&stale));
    assert!(!search.candidates.contains(&noncandidate));
    assert!(!search.candidates.contains(&unauthorized));

    move_library_to_graveyard(&mut engine, 0, stale);
    move_graveyard_to_library(&mut engine, 0, stale);
    for (player, chosen, label) in [
        (0, noncandidate, "noncandidate"),
        (0, stale, "stale"),
        (1, unauthorized, "unauthorized"),
    ] {
        assert!(
            matches!(
                engine.apply_scenario_command(player, &submit_resolution_choice(vec![chosen])),
                Err(EngineError::Illegal(_))
            ),
            "{label} choice must be rejected"
        );
        assert!(engine.state.pending_resolution.is_some());
    }
    engine
        .apply_scenario_command(0, &submit_resolution_choice(Vec::new()))
        .expect("fail to find after rejected stale choices");
    assert!(engine.state.pending_resolution.is_none());
}

#[test]
fn resultless_direct_hand_search_rejects_candidate_missing_from_library_vector_atomically() {
    let mut engine = direct_hand_search_commander_engine(
        284_106,
        r#"(exact_name: Some("Legal Test Commander"))"#,
    );
    let commander = engine.state.players[0]
        .library
        .iter()
        .copied()
        .find(|object_id| engine.state.objects[object_id].card_id == "legal_test_commander")
        .expect("declared commander in the searcher's library");
    let generation = engine.state.zone_change_generation[&commander];
    ensure_in_hand(&mut engine, 0, "direct_hand_search_fixture");
    let slot = hand_index_for_card(&engine, 0, "direct_hand_search_fixture");
    engine
        .apply_scenario_command(0, &cast_spell(slot, Vec::new()))
        .expect("cast the direct Hand search fixture");
    let branch_batch = pass_both_players_with_batch(&mut engine);
    assert!(find_resolution_choice(&branch_batch).is_some());
    let search_batch = engine
        .apply_scenario_command(0, &select_branch(0))
        .expect("select the search branch");
    let search = find_resolution_choice(&search_batch).expect("private exact-name search");
    assert!(search.candidate_object_ids.contains(&commander));

    let prompt_id = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("library search prompt remains open")
        .resolution_prompt_id
        .get();
    engine.state.players[0]
        .library
        .retain(|object_id| *object_id != commander);
    assert_eq!(engine.state.objects[&commander].zone, Zone::Library);
    assert_eq!(engine.state.zone_change_generation[&commander], generation);
    assert!(!engine.state.players[0].library.contains(&commander));
    let state_before_answer = serde_json::to_value(&engine.state)
        .expect("serialize the complete parked state before the adversarial answer");

    let result = engine.apply_scenario_command(0, &submit_resolution_choice(vec![commander]));
    assert!(
        matches!(result, Err(EngineError::Illegal(_))),
        "a selected object absent from the live library vector must be rejected before reveal or owner replacement"
    );
    assert_eq!(
        serde_json::to_value(&engine.state).expect("serialize the complete state after rejection"),
        state_before_answer,
        "stale library-vector membership must leave the entire state unchanged"
    );
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("the original library search prompt remains pending");
    assert_eq!(pending.resolution_prompt_id.get(), prompt_id);
    assert_eq!(engine.state.objects[&commander].zone, Zone::Library);
    assert_eq!(engine.state.zone_change_generation[&commander], generation);
    assert!(!engine.state.players[0].library.contains(&commander));
}

#[test]
fn resultless_direct_hand_search_offers_commander_owner_replacement_before_tail() {
    let mut engine = direct_hand_search_commander_engine(
        284_100,
        r#"(exact_name: Some("Legal Test Commander"))"#,
    );
    let commander = engine.state.players[0].library.back().copied().unwrap();
    let slot = hand_index_for_card(&engine, 0, "direct_hand_search_fixture");
    engine
        .apply_scenario_command(0, &cast_spell(slot, Vec::new()))
        .expect("cast the direct Hand search fixture");
    let branch_batch = pass_both_players_with_batch(&mut engine);
    let branch = find_resolution_choice(&branch_batch).expect("optional search branch");
    assert_eq!(branch.deciding_player_id, 0);
    assert_eq!(branch.choice_kind(), ChoiceKind::ResolutionBranch);

    let search_batch = engine
        .apply_scenario_command(0, &select_branch(0))
        .expect("select the search branch");
    let search = find_resolution_choice(&search_batch).expect("private exact-name search");
    assert_eq!(search.deciding_player_id, 0);
    assert_eq!(search.choice_kind(), ChoiceKind::LibrarySearch);
    assert!(search.candidate_object_ids.contains(&commander));

    let commander_batch = engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![commander]))
        .expect("select the declared commander");
    let replacement = find_resolution_choice(&commander_batch)
        .expect("commander owner should decide between command zone and hand");
    assert_eq!(replacement.deciding_player_id, 0);
    assert_eq!(replacement.choice_kind(), ChoiceKind::ResolutionBranch);
    assert_eq!((replacement.min, replacement.max), (1, 1));
    assert_eq!(replacement.resolution_branches.len(), 2);
    assert_eq!(replacement.resolution_branches[0].label, "Command zone");
    assert_eq!(replacement.resolution_branches[1].label, "Hand");
    assert!(
        !engine.state.players[0].hand.contains(&commander),
        "the selected commander waits in the library until its owner chooses"
    );
    assert_eq!(
        engine.state.objects[&commander].zone,
        Zone::Library,
        "the exact revealed incarnation stays in the library while its owner decides"
    );
    assert!(commander_batch.events.iter().any(
        |event| matches!(&event.ev, Some(Ev::CardsRevealed(reveal)) if reveal.cards.iter().any(|card| card.object_id == commander))
    ));

    let prompt_id = replacement.resolution_prompt_id;
    let pending_prompt = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("owner replacement stays pending")
        .presentation
        .prompt
        .clone();
    let library_before: Vec<_> = engine.state.players[0].library.iter().copied().collect();
    let hand_before = engine.state.players[0].hand.clone();
    let stack_before: Vec<_> = engine.state.stack.iter().map(|item| item.id).collect();
    assert!(engine.apply_scenario_command(1, &select_branch(0)).is_err());
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .expect("wrong actor leaves owner prompt intact")
            .resolution_prompt_id
            .get(),
        prompt_id
    );
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .presentation
            .prompt,
        pending_prompt
    );
    assert_eq!(
        engine.state.players[0]
            .library
            .iter()
            .copied()
            .collect::<Vec<_>>(),
        library_before
    );
    assert_eq!(engine.state.players[0].hand, hand_before);
    assert_eq!(
        engine
            .state
            .stack
            .iter()
            .map(|item| item.id)
            .collect::<Vec<_>>(),
        stack_before
    );

    let mut stale_owner_answer = select_branch(0);
    let Some(Cmd::SubmitResolutionChoice(answer)) = stale_owner_answer.cmd.as_mut() else {
        unreachable!()
    };
    answer.resolution_prompt_id = prompt_id.saturating_add(1);
    assert!(engine
        .apply_scenario_command(0, &stale_owner_answer)
        .is_err());
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .expect("stale owner prompt leaves replacement intact")
            .resolution_prompt_id
            .get(),
        prompt_id
    );
    assert_eq!(
        engine.state.players[0]
            .library
            .iter()
            .copied()
            .collect::<Vec<_>>(),
        library_before
    );
    assert_eq!(engine.state.players[0].hand, hand_before);
    assert_eq!(
        engine
            .state
            .stack
            .iter()
            .map(|item| item.id)
            .collect::<Vec<_>>(),
        stack_before
    );

    let completion = engine
        .apply_scenario_command(0, &select_branch(0))
        .expect("commander owner chooses Command");
    assert_eq!(engine.state.objects[&commander].zone, Zone::Command);
    assert!(engine.state.players[0].command_zone.contains(&commander));
    assert!(!engine.state.players[0].hand.contains(&commander));
    assert_eq!(engine.state.players[0].hand.len(), hand_before.len() + 1);
    assert_eq!(
        completion
            .events
            .iter()
            .filter(|event| matches!(&event.ev, Some(Ev::Log(log)) if log.text == "P0 shuffles their library."))
            .count(),
        1,
        "the remaining library is shuffled once after the destination choice"
    );
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
    assert_eq!(
        engine.state.players[0]
            .battlefield
            .iter()
            .filter(|object_id| engine.state.objects[object_id].card_id == "treasure")
            .count(),
        1,
        "the parent tail executes exactly once"
    );
}

#[test]
fn resultless_direct_hand_search_commander_owner_can_choose_hand() {
    let mut engine = direct_hand_search_commander_engine(
        284_103,
        r#"(exact_name: Some("Legal Test Commander"))"#,
    );
    let commander = engine.state.players[0].library.back().copied().unwrap();
    let slot = hand_index_for_card(&engine, 0, "direct_hand_search_fixture");
    engine
        .apply_scenario_command(0, &cast_spell(slot, Vec::new()))
        .expect("cast the direct Hand search fixture");
    let branch_batch = pass_both_players_with_batch(&mut engine);
    assert!(find_resolution_choice(&branch_batch).is_some());
    engine
        .apply_scenario_command(0, &select_branch(0))
        .expect("select the search branch");
    engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![commander]))
        .expect("select the declared commander");
    let before_hand = engine.state.players[0].hand.len();
    let completion = engine
        .apply_scenario_command(0, &select_branch(1))
        .expect("commander owner chooses Hand");

    assert_eq!(engine.state.objects[&commander].zone, Zone::Hand);
    assert!(engine.state.players[0].hand.contains(&commander));
    assert!(!engine.state.players[0].command_zone.contains(&commander));
    assert_eq!(
        engine.state.players[0].hand.len(),
        before_hand + 2,
        "the selected commander and the parent Draw tail each add one card"
    );
    assert_eq!(
        completion
            .events
            .iter()
            .filter(|event| matches!(&event.ev, Some(Ev::Log(log)) if log.text == "P0 shuffles their library."))
            .count(),
        1
    );
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
    assert_eq!(
        engine.state.players[0]
            .battlefield
            .iter()
            .filter(|object_id| engine.state.objects[object_id].card_id == "treasure")
            .count(),
        1,
        "the parent tail executes exactly once after the owner puts the commander in Hand"
    );
}

#[test]
fn resultless_direct_hand_search_controller_departure_completes_the_accepted_search() {
    let mut engine = direct_hand_search_controller_departure_engine(284_105);
    let commander = engine.state.players[0].library.back().copied().unwrap();
    let opponent_life_before = engine.state.players[1].life;
    let slot = hand_index_for_card(&engine, 0, "direct_hand_search_departure_fixture");
    engine
        .apply_scenario_command(0, &cast_spell(slot, target_player(1)))
        .expect("cast the direct Hand search fixture");
    let spell = engine.state.stack.last().expect("cast spell on stack").id;
    pass_priority_round(&mut engine);
    let branch = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("optional branch is offered")
        .presentation
        .choice_kind;
    assert_eq!(branch, ChoiceKind::ResolutionBranch);
    engine
        .apply_scenario_command(0, &select_branch(0))
        .expect("select the search branch");
    engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![commander]))
        .expect("select the declared commander");
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .expect("owner destination choice is active")
            .deciding_player,
        0
    );

    let departure = engine
        .apply_scenario_command(0, &concede())
        .expect("searching controller may concede while the owner choice is pending");
    assert!(engine.state.players[0].has_lost);
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
    assert!(!engine.state.objects.contains_key(&spell));
    assert!(engine
        .state
        .players
        .iter()
        .enumerate()
        .filter(|(index, _)| *index != 0)
        .all(|(_, player)| !player.has_lost));
    assert_eq!(
        engine.state.players[1].life,
        opponent_life_before - 1,
        "the resultless typed tail affects its live target exactly once after the controller leaves"
    );
    assert_eq!(
        departure
            .events
            .iter()
            .filter(|event| matches!(&event.ev, Some(Ev::Log(log)) if log.text == "P0 shuffles their library."))
            .count(),
        0,
        "a departed searcher's removed library is not shuffled"
    );
}

#[test]
fn resultless_direct_hand_search_moves_exact_basic_and_resumes_tail_once() {
    let mut engine = direct_hand_search_commander_engine(284_101, "(card_type: Some(BasicLand))");
    let selected = engine.state.players[0]
        .library
        .iter()
        .copied()
        .find(|object_id| engine.state.objects[object_id].card_id == "forest")
        .expect("basic land in library");
    let hand_before_cast = engine.state.players[0].hand.len();
    let slot = hand_index_for_card(&engine, 0, "direct_hand_search_fixture");
    engine
        .apply_scenario_command(0, &cast_spell(slot, Vec::new()))
        .expect("cast the direct Hand search fixture");
    let branch_batch = pass_both_players_with_batch(&mut engine);
    assert!(find_resolution_choice(&branch_batch).is_some());
    let search_batch = engine
        .apply_scenario_command(0, &select_branch(0))
        .expect("select the search branch");
    let search = find_resolution_choice(&search_batch).expect("private basic-land search");
    assert_eq!(search.choice_kind(), ChoiceKind::LibrarySearch);
    assert!(search.candidate_object_ids.contains(&selected));

    let completion = engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![selected]))
        .expect("select the exact basic land");
    assert_eq!(engine.state.objects[&selected].zone, Zone::Hand);
    assert!(engine.state.players[0].hand.contains(&selected));
    assert!(!engine.state.players[0].library.contains(&selected));
    assert_eq!(engine.state.players[0].hand.len(), hand_before_cast + 1);
    assert!(completion.events.iter().any(
        |event| matches!(&event.ev, Some(Ev::CardsRevealed(reveal)) if reveal.cards.iter().any(|card| card.object_id == selected))
    ));
    assert_eq!(
        completion
            .events
            .iter()
            .filter(|event| matches!(&event.ev, Some(Ev::Log(log)) if log.text == "P0 shuffles their library."))
            .count(),
        1
    );
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
}
