//! Engine-cursor coverage for the narrow resultless Battlefield BasicLand search leaf.
//! The test-only entrant forces the existing as-enters creature-type replacement to suspend.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_cards::CardRegistry;
use tricerules_core::{EngineDeck, Zone};
use tricerules_proto::ruled::v1::{
    ruled_command::Cmd, ChoiceKind, ResolutionChoiceDecision, RuledCommand, SubmitResolutionChoice,
};

const DIRECT_BATTLEFIELD_SEARCH_FIXTURE: &str = r#"(
  id: "direct_battlefield_search_fixture",
  name: "Direct Battlefield Search Fixture",
  face_id: "direct_battlefield_search_fixture",
  mana_cost: "{0}",
  types: ["Instant"],
  spell_program: Some((body: (steps: [
    (id: "choose_search", bind: [], input: None, kind: Choose(
      choice: "search", chooser: Controller, optional: true,
      branches: [(branch_id: "search_for_a_basic_land", presentation: Fallback,
        cost: None, requirement: Always, body: (steps: [
          (id: "search_battlefield", bind: [], input: None, kind: Perform(SearchLibrary(
            who: Controller, optional: false, count: 1, count_by_cast_cost: None,
            filter: Some((card_type: Some(BasicLand))),
            selection_constraint: None, slots: [], zones: Fixed([Library]),
            destination: Battlefield(tapped: true), conditional_destination: None,
            shuffle: true, reveal: false, result_id: None,
            selection_bounds: Some((min: 0, max: 1)),
          )))
        ], yields: []))],
      otherwise: (steps: [], yields: []),
    )),
    (id: "tail_treasure", bind: [], input: None, kind: Perform(CreateTokens(
      token: "treasure", count: 1, who: Controller, tapped: false,
      sacrifice_timing: None,
    ))),
  ], yields: []), target_bindings: [])),
)"#;

const TEST_BASIC_LAND_CREATURE_WITH_ENTRY_CHOICE: &str = r#"(
  id: "test_basic_land_creature_with_entry_choice",
  name: "Test Basic Land Creature with Entry Choice",
  face_id: "test_basic_land_creature_with_entry_choice",
  mana_cost: "{0}",
  supertypes: ["Basic"],
  types: ["Land", "Creature", "Forest", "Dryad"],
  power: 0,
  toughness: 1,
  static_abilities: [(
    ability_id: "choose_type",
    presentation: OracleLines([1]),
    definition: AsEntersChooseCreatureType(link_id: "chosen_type"),
  )],
  triggered_abilities: [(
    ability_id: "observe_chosen_type",
    presentation: OracleLines([2]),
    trigger: WheneverPermanentEntersBattlefield(
      controller: Controller,
      filter: (
        permanent_type: Some(Creature),
        token: Some(false),
        source_chosen_creature_type: Some("chosen_type"),
      ),
    ),
    effect: [PutCounters(counter: PlusOnePlusOne, count: 1, subject: Source)],
  )],
)"#;

const DRAW_STEP_TRIGGERED_BATTLEFIELD_SEARCH_FIXTURE: &str = r#"(
  id: "draw_step_triggered_battlefield_search_fixture",
  name: "Draw Step Triggered Battlefield Search Fixture",
  face_id: "draw_step_triggered_battlefield_search_fixture",
  mana_cost: "{0}",
  types: ["Creature", "Wolf"],
  power: 1,
  toughness: 1,
  triggered_abilities: [(
    ability_id: "search_on_draw_step",
    presentation: Fallback,
    trigger: AtBeginningOfDrawStep(player: AnyPlayer),
    effect: [],
    program: Some((body: (steps: [
      (id: "choose_search", bind: [], input: None, kind: Choose(
        choice: "search", chooser: Controller, optional: true,
        branches: [(branch_id: "search_for_a_basic_land", presentation: Fallback,
          cost: None, requirement: Always, body: (steps: [
            (id: "search_battlefield", bind: [], input: None, kind: Perform(SearchLibrary(
              who: Controller, optional: false, count: 1, count_by_cast_cost: None,
              filter: Some((card_type: Some(BasicLand))),
              selection_constraint: None, slots: [], zones: Fixed([Library]),
              destination: Battlefield(tapped: true), conditional_destination: None,
              shuffle: true, reveal: false, result_id: None,
              selection_bounds: Some((min: 0, max: 1)),
            )))
          ], yields: []))],
        otherwise: (steps: [], yields: []),
      )),
      (id: "tail_draw", bind: [], input: None, kind: Perform(Draw(who: AffectedPlayer, count: 1))),
    ], yields: []), target_bindings: [])),
    modal: None,
    targeting: None,
    may: false,
    intervening_if: None,
    triggers_only_once: false,
    max_triggers_per_turn: None,
  )],
)"#;

fn choose_branch(index: u32) -> RuledCommand {
    choose_branch_for_prompt(0, index)
}

fn choose_branch_for_prompt(prompt_id: u64, index: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
            resolution_prompt_id: prompt_id,
            decision: ResolutionChoiceDecision::SelectBranch as i32,
            selected_branch_index: index,
            ..Default::default()
        })),
    }
}

fn test_engine(seed: u64) -> GameEngine {
    let registry = Box::leak(Box::new(
        CardRegistry::from_chunks_and_tokens(
            &[
                DIRECT_BATTLEFIELD_SEARCH_FIXTURE,
                TEST_BASIC_LAND_CREATURE_WITH_ENTRY_CHOICE,
                include_str!("../../../tricerules-cards/data/forest.ron"),
                include_str!("../../../tricerules-cards/data/island.ron"),
                include_str!("../../../tricerules-cards/data/wan_shi_tong,_librarian.ron"),
            ],
            &[include_str!(
                "../../../tricerules-cards/data/tokens/treasure.ron"
            )],
        )
        .expect("the test-only Battlefield search registry is valid"),
    ));
    let decks = Some(vec![
        EngineDeck {
            mainboard: [
                vec!["direct_battlefield_search_fixture".into()],
                vec!["forest".into(); 6],
                vec!["test_basic_land_creature_with_entry_choice".into()],
            ]
            .concat(),
            commanders: vec![],
        },
        EngineDeck {
            mainboard: [
                vec!["wan_shi_tong,_librarian".into()],
                vec!["island".into(); 8],
            ]
            .concat(),
            commanders: vec![],
        },
    ]);
    let mut engine = GameEngine::new_with_commander_decks(registry, seed, &[0, 1], 20, decks, true)
        .expect("two-player resultless Battlefield-search engine");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn three_player_draw_step_search_engine(seed: u64) -> GameEngine {
    let registry = Box::leak(Box::new(
        CardRegistry::from_chunks_and_tokens(
            &[
                DRAW_STEP_TRIGGERED_BATTLEFIELD_SEARCH_FIXTURE,
                TEST_BASIC_LAND_CREATURE_WITH_ENTRY_CHOICE,
                include_str!("../../../tricerules-cards/data/forest.ron"),
                include_str!("../../../tricerules-cards/data/island.ron"),
                include_str!("../../../tricerules-cards/data/wan_shi_tong,_librarian.ron"),
            ],
            &[],
        )
        .expect("the multiplayer triggered-search registry is valid"),
    ));
    // Seat order makes P2 active/affected, P0 the source controller/searcher, and P1 the source
    // owner. The draw-step trigger supplies P2 to the typed tail after P0 leaves.
    let decks = Some(vec![
        EngineDeck {
            mainboard: [
                vec!["wan_shi_tong,_librarian".into()],
                vec!["island".into(); 12],
            ]
            .concat(),
            commanders: vec![],
        },
        EngineDeck {
            mainboard: vec!["forest".into(); 18],
            commanders: vec![],
        },
        EngineDeck {
            mainboard: [
                vec!["draw_step_triggered_battlefield_search_fixture".into()],
                vec!["forest".into(); 18],
            ]
            .concat(),
            commanders: vec![],
        },
    ]);
    GameEngine::new_with_commander_decks(registry, seed, &[2, 0, 1], 20, decks, true)
        .expect("three-player triggered Battlefield-search engine")
}

fn install_search_observer(engine: &mut GameEngine) -> u32 {
    install_search_observer_for(engine, 1)
}

fn install_search_observer_for(engine: &mut GameEngine, player: usize) -> u32 {
    let player_id = engine.state.players[player].id;
    let wan = engine.state.players[player]
        .library
        .iter()
        .chain(engine.state.players[player].hand.iter())
        .copied()
        .find(|object_id| engine.state.objects[object_id].card_id == "wan_shi_tong,_librarian")
        .expect("Wan Shi Tong in opponent library or hand");
    engine.state.players[player]
        .library
        .retain(|object_id| *object_id != wan);
    engine.state.players[player]
        .hand
        .retain(|object_id| *object_id != wan);
    engine.state.players[player].battlefield.push(wan);
    let observer = engine.state.objects.get_mut(&wan).unwrap();
    observer.zone = Zone::Battlefield;
    observer.controller = player_id;
    observer.base_controller = player_id;
    *engine.state.zone_change_generation.entry(wan).or_default() += 1;
    wan
}

fn open_triggered_search_with_entry_choice(
    engine: &mut GameEngine,
    selected: u32,
) -> tricerules_proto::ruled::v1::ResolutionChoiceRequired {
    pass_priority_round(engine);
    assert_eq!(engine.state.turn_step, tricerules_core::TurnStep::Draw);
    pass_priority_round(engine);
    assert!(engine.state.pending_resolution.is_some());
    let branch_offer = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("the resolving trigger offers its optional search")
        .resolution_prompt_id
        .get();
    engine
        .apply_scenario_command(0, &choose_branch_for_prompt(branch_offer, 0))
        .expect("choose the optional battlefield search");
    let offer = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("the triggered search offers the selected library")
        .resolution_prompt_id
        .get();
    let response = engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![selected]))
        .expect("choose the exact searched entrant");
    let choice = find_resolution_choice(&response).expect("the entrant asks for its creature type");
    assert_eq!(choice.deciding_player_id, 0);
    assert_eq!(choice.choice_kind(), ChoiceKind::ResolutionBranch);
    assert_ne!(offer, choice.resolution_prompt_id);
    choice
}

fn cast_fixture(engine: &mut GameEngine) -> tricerules_proto::ruled::v1::RuledEventBatch {
    ensure_in_hand(engine, 0, "direct_battlefield_search_fixture");
    let hand_index = hand_index_for_card(engine, 0, "direct_battlefield_search_fixture");
    engine
        .apply_scenario_command(0, &cast_spell(hand_index, Vec::new()))
        .expect("cast the zero-cost direct Battlefield search fixture");
    let branch_batch = pass_both_players_with_batch(engine);
    assert!(find_resolution_choice(&branch_batch).is_some());
    branch_batch
}

fn treasure_count(engine: &GameEngine) -> usize {
    engine.state.players[0]
        .battlefield
        .iter()
        .filter(|object_id| engine.state.objects[object_id].card_id == "treasure")
        .count()
}

fn move_library_to_graveyard(engine: &mut GameEngine, player: usize) {
    let library: Vec<_> = engine.state.players[player].library.drain(..).collect();
    for object_id in library {
        engine.state.players[player].graveyard.push(object_id);
        engine.state.objects.get_mut(&object_id).unwrap().zone = Zone::Graveyard;
        *engine
            .state
            .zone_change_generation
            .entry(object_id)
            .or_default() += 1;
    }
}

#[test]
fn resultless_battlefield_search_synchronous_entry_resumes_tail_once() {
    let mut engine = test_engine(284_201);
    let wan = install_search_observer(&mut engine);
    let selected = inject_library_card(&mut engine, 0, "forest");
    let generation_before = engine
        .state
        .zone_change_generation
        .get(&selected)
        .copied()
        .unwrap_or(0);
    cast_fixture(&mut engine);
    let search_batch = engine
        .apply_scenario_command(0, &choose_branch(0))
        .expect("choose the mandatory search branch");
    let search = find_resolution_choice(&search_batch).expect("private BasicLand search");
    assert_eq!(search.choice_kind(), ChoiceKind::LibrarySearch);
    assert!(search.candidate_object_ids.contains(&selected));

    let completion = engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![selected]))
        .expect("choose the exact Forest");
    assert_eq!(engine.state.objects[&selected].zone, Zone::Battlefield);
    assert!(engine.state.objects[&selected].tapped);
    assert_eq!(engine.state.objects[&selected].owner, 0);
    assert_eq!(engine.state.objects[&selected].controller, 0);
    assert_eq!(
        engine.state.zone_change_generation[&selected],
        generation_before + 1,
        "the selected physical object enters as one new incarnation"
    );
    assert_eq!(treasure_count(&engine), 1, "the typed tail runs once");
    assert_eq!(
        completion
            .events
            .iter()
            .filter(|event| matches!(&event.ev, Some(Ev::Log(log)) if log.text == "P0 shuffles their library."))
            .count(),
        1
    );
    assert!(
        completion
            .events
            .iter()
            .all(|event| !matches!(&event.ev, Some(Ev::CardsRevealed(_)))),
        "a no-reveal Battlefield search does not expose the selected card"
    );
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(engine.state.stack.len(), 1, "one LibrarySearched observer");
    assert_eq!(engine.state.stack[0].source_permanent_id, Some(wan));
}

#[test]
fn resultless_battlefield_search_nonempty_zero_selection_shuffles_and_runs_tail_once() {
    let mut engine = test_engine(284_205);
    let wan = install_search_observer(&mut engine);
    let unselected = inject_library_card(&mut engine, 0, "forest");
    let generation_before = engine
        .state
        .zone_change_generation
        .get(&unselected)
        .copied()
        .unwrap_or(0);
    cast_fixture(&mut engine);
    let search_batch = engine
        .apply_scenario_command(0, &choose_branch(0))
        .expect("choose the optional search branch");
    let search = find_resolution_choice(&search_batch).expect("offer the nonempty library");
    assert_eq!(search.choice_kind(), ChoiceKind::LibrarySearch);
    assert_eq!((search.min, search.max), (0, 1));
    assert!(search.candidate_object_ids.contains(&unselected));

    let completion = engine
        .apply_scenario_command(0, &submit_resolution_choice(Vec::new()))
        .expect("choose zero cards from the nonempty library");
    assert_eq!(engine.state.objects[&unselected].zone, Zone::Library);
    assert!(engine.state.players[0].library.contains(&unselected));
    assert_eq!(
        engine
            .state
            .zone_change_generation
            .get(&unselected)
            .copied()
            .unwrap_or(0),
        generation_before,
        "a zero-selection search keeps every library object in the same incarnation"
    );
    assert_eq!(
        completion
            .events
            .iter()
            .filter(|event| matches!(&event.ev, Some(Ev::Log(log)) if log.text == "P0 shuffles their library."))
            .count(),
        1
    );
    assert_eq!(
        completion
            .events
            .iter()
            .filter(
                |event| matches!(&event.ev, Some(Ev::Log(log)) if log.text == "P0 finds no card.")
            )
            .count(),
        1
    );
    assert!(completion
        .events
        .iter()
        .all(|event| !matches!(&event.ev, Some(Ev::CardsRevealed(_)))));
    assert_eq!(treasure_count(&engine), 1, "the typed tail runs once");
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(engine.state.stack.len(), 1, "one LibrarySearched observer");
    assert_eq!(engine.state.stack[0].source_permanent_id, Some(wan));
}

#[test]
fn resultless_battlefield_search_empty_library_shuffles_and_runs_tail_once() {
    let mut engine = test_engine(284_206);
    let wan = install_search_observer(&mut engine);
    ensure_in_hand(&mut engine, 0, "direct_battlefield_search_fixture");
    move_library_to_graveyard(&mut engine, 0);
    assert!(engine.state.players[0].library.is_empty());
    cast_fixture(&mut engine);
    let graveyard_before_search = engine.state.players[0].graveyard.len();
    let branch_batch = engine
        .apply_scenario_command(0, &choose_branch(0))
        .expect("choose the optional search branch with an empty library");
    assert!(find_resolution_choice(&branch_batch).is_none());
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.players[0].library.is_empty());
    assert_eq!(
        engine.state.players[0].graveyard.len(),
        graveyard_before_search + 1,
        "the resolved instant is the only object added; the empty search finds no card"
    );
    assert_eq!(treasure_count(&engine), 1, "the typed tail runs once");
    assert_eq!(
        branch_batch
            .events
            .iter()
            .filter(|event| matches!(&event.ev, Some(Ev::Log(log)) if log.text == "P0 shuffles their library."))
            .count(),
        1
    );
    assert_eq!(
        branch_batch
            .events
            .iter()
            .filter(
                |event| matches!(&event.ev, Some(Ev::Log(log)) if log.text == "P0 finds no card.")
            )
            .count(),
        1
    );
    assert!(branch_batch
        .events
        .iter()
        .all(|event| !matches!(&event.ev, Some(Ev::CardsRevealed(_)))));
    assert_eq!(engine.state.stack.len(), 1, "one LibrarySearched observer");
    assert_eq!(engine.state.stack[0].source_permanent_id, Some(wan));
}

#[test]
fn resultless_battlefield_search_entry_choice_retains_cursor_until_entry_commits() {
    let mut engine = test_engine(284_202);
    let wan = install_search_observer(&mut engine);
    let selected =
        inject_library_card(&mut engine, 0, "test_basic_land_creature_with_entry_choice");
    let generation_before = engine
        .state
        .zone_change_generation
        .get(&selected)
        .copied()
        .unwrap_or(0);
    cast_fixture(&mut engine);
    let search_batch = engine
        .apply_scenario_command(0, &choose_branch(0))
        .expect("choose the mandatory search branch");
    let search = find_resolution_choice(&search_batch).expect("private BasicLand search");
    assert!(search.candidate_object_ids.contains(&selected));

    let entry_batch = engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![selected]))
        .expect("choose the test-only BasicLand Creature");
    let type_choice = find_resolution_choice(&entry_batch).expect("as-enters creature-type choice");
    assert_eq!(type_choice.choice_kind(), ChoiceKind::ResolutionBranch);
    assert_eq!(type_choice.deciding_player_id, 0);
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("the entry replacement is parked");
    let serialized_pending =
        serde_json::to_value(pending).expect("pending continuation is serializable");
    assert!(
        !serialized_pending["continuation"]["EntryChooseCreatureType"]["stack"]["program"]
            .is_null(),
        "the ZoneEntryBatch choice retains the complete typed program cursor"
    );

    let human = tricerules_card_model::CREATURE_TYPES
        .iter()
        .position(|creature_type| *creature_type == "Human")
        .expect("Human is a legal creature type") as u32;
    let completion = engine
        .apply_scenario_command(0, &choose_branch(human))
        .expect("choose Human as the entry type");
    assert_eq!(engine.state.objects[&selected].zone, Zone::Battlefield);
    assert!(engine.state.objects[&selected].tapped);
    assert_eq!(
        engine.state.zone_change_generation[&selected],
        generation_before + 1
    );
    assert_eq!(
        treasure_count(&engine),
        1,
        "the parked typed tail runs once"
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
    assert_eq!(engine.state.stack.len(), 1, "one LibrarySearched observer");
    assert_eq!(engine.state.stack[0].source_permanent_id, Some(wan));
}

#[test]
fn resultless_battlefield_search_source_owner_departure_keeps_entry_prompt_and_id() {
    let mut engine = three_player_draw_step_search_engine(284_203);
    inject_creature_under_foreign_control(
        &mut engine,
        2,
        1,
        "draw_step_triggered_battlefield_search_fixture",
    );
    let selected =
        inject_library_card(&mut engine, 1, "test_basic_land_creature_with_entry_choice");
    let selected_generation = engine
        .state
        .zone_change_generation
        .get(&selected)
        .copied()
        .unwrap_or(0);
    let wan = install_search_observer_for(&mut engine, 0);
    let original_choice = open_triggered_search_with_entry_choice(&mut engine, selected);
    let hand_before = engine.state.players[0].hand.len();

    let departure = engine
        .apply_scenario_command(1, &concede())
        .expect("the resolving trigger survives its source owner's departure");
    assert!(engine.state.players[2].has_lost);
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("the live searcher still owns the same entry choice");
    assert_eq!(
        pending.resolution_prompt_id.get(),
        original_choice.resolution_prompt_id,
        "a source-owner departure that does not alter the offer keeps its prompt id"
    );
    assert_eq!(
        engine
            .state
            .zone_change_generation
            .get(&selected)
            .copied()
            .unwrap_or(0),
        selected_generation
    );
    if let Some(republished) = find_resolution_choice(&departure) {
        assert_eq!(
            republished.resolution_prompt_id,
            original_choice.resolution_prompt_id
        );
    }

    let human = tricerules_card_model::CREATURE_TYPES
        .iter()
        .position(|creature_type| *creature_type == "Human")
        .expect("Human is a legal creature type") as u32;
    let completion = engine
        .apply_scenario_command(
            0,
            &choose_branch_for_prompt(original_choice.resolution_prompt_id, human),
        )
        .expect("the unchanged live entry choice remains answerable");
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(engine.state.objects[&selected].zone, Zone::Battlefield);
    assert!(engine.state.objects[&selected].tapped);
    assert_eq!(
        engine.state.zone_change_generation[&selected],
        selected_generation + 1
    );
    assert_eq!(
        engine.state.players[0].hand.len(),
        hand_before + 1,
        "the triggered typed tail draws for the attacked player exactly once"
    );
    assert_eq!(
        completion
            .events
            .iter()
            .filter(|event| matches!(
                &event.ev,
                Some(Ev::Log(log)) if log.text == "P0 shuffles their library."
            ))
            .count(),
        1
    );
    assert_eq!(engine.state.stack.len(), 1, "one LibrarySearched observer");
    assert_eq!(engine.state.stack[0].source_permanent_id, Some(wan));
}

#[test]
fn resultless_battlefield_searcher_departure_retires_entry_without_shuffle_and_runs_tail_once() {
    let mut engine = three_player_draw_step_search_engine(284_204);
    inject_creature_under_foreign_control(
        &mut engine,
        2,
        1,
        "draw_step_triggered_battlefield_search_fixture",
    );
    let selected =
        inject_library_card(&mut engine, 1, "test_basic_land_creature_with_entry_choice");
    let selected_generation = engine
        .state
        .zone_change_generation
        .get(&selected)
        .copied()
        .unwrap_or(0);
    let wan = install_search_observer_for(&mut engine, 0);
    let retired_choice = open_triggered_search_with_entry_choice(&mut engine, selected);
    let hand_before = [
        engine.state.players[2].hand.len(),
        engine.state.players[0].hand.len(),
    ];

    let departure = engine
        .apply_scenario_command(0, &concede())
        .expect("the program is detached through its entry replacement");
    assert!(engine.state.players[1].has_lost);
    assert!(
        engine.state.pending_resolution.is_none(),
        "the departed searcher's entrant prompt is retired"
    );
    assert!(
        engine.state.players[1]
            .battlefield
            .iter()
            .all(|object_id| *object_id != selected),
        "the departed library card is not resurrected as a permanent"
    );
    assert_eq!(
        engine
            .state
            .zone_change_generation
            .get(&selected)
            .copied()
            .unwrap_or(0),
        selected_generation,
        "the abandoned entrant never changes incarnation"
    );
    assert_eq!(
        departure
            .events
            .iter()
            .filter(|event| matches!(
                &event.ev,
                Some(Ev::Log(log)) if log.text == "P0 shuffles their library."
            ))
            .count(),
        0,
        "a departed searcher does not shuffle"
    );
    assert_eq!(
        engine.state.players[2].hand.len(),
        hand_before[0],
        "the other survivor is not the captured draw-step affected player"
    );
    assert_eq!(
        engine.state.players[0].hand.len(),
        hand_before[1] + 1,
        "the captured affected player receives the typed tail once"
    );
    assert_eq!(engine.state.stack.len(), 1, "one begun-search observer");
    assert_eq!(engine.state.stack[0].source_permanent_id, Some(wan));

    let before_stale_answer = engine
        .diagnostic_snapshot()
        .expect("retired-prompt diagnostic snapshot");
    let human = tricerules_card_model::CREATURE_TYPES
        .iter()
        .position(|creature_type| *creature_type == "Human")
        .expect("Human is a legal creature type") as u32;
    assert!(engine
        .apply_scenario_command(
            1,
            &choose_branch_for_prompt(retired_choice.resolution_prompt_id, human),
        )
        .is_err());
    assert_eq!(
        engine
            .diagnostic_snapshot()
            .expect("retired-prompt diagnostic snapshot after stale answer"),
        before_stale_answer,
        "a live wrong actor cannot revive a retired entry prompt"
    );
}
