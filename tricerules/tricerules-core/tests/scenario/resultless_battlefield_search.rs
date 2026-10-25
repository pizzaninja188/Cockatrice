//! Engine-cursor coverage for the narrow resultless Battlefield BasicLand search leaf.
//! The test-only entrant forces the existing as-enters creature-type replacement to suspend.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_cards::CardRegistry;
use tricerules_core::{EngineDeck, Zone};
use tricerules_proto::ruled::v1::{
    ruled_command::Cmd, ChoiceCandidateSourceZone, ChoiceKind, ResolutionChoiceDecision,
    RuledCommand, SubmitResolutionChoice,
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
            shuffle: true, reveal: false,
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

const TYPED_BATTLEFIELD_SEARCH_BEHOLD_FIXTURE: &str = r#"(
  id: "direct_battlefield_search_fixture",
  name: "Direct Battlefield Search Fixture",
  face_id: "direct_battlefield_search_fixture",
  mana_cost: "{0}",
  types: ["Instant"],
  spell_program: Some((body: (steps: [
    (id: "search_battlefield", bind: ["found_land"], input: None, kind: Perform(SearchLibrary(
      who: Controller, optional: false, count: 1, count_by_cast_cost: None,
      filter: Some((card_type: Some(BasicLand))),
      selection_constraint: None, slots: [], zones: Fixed([Library]),
      destination: Battlefield(tapped: true), conditional_destination: None,
      shuffle: true, reveal: false,
      selection_bounds: Some((min: 0, max: 1)),
    ))),
    (id: "scry_after_search", bind: [], input: None, kind: Perform(Scry(count: 0))),
    (id: "behold_elf", bind: [], input: Some("found_land"), kind: Perform(MayBehold(
      who: Controller, choice_id: "behold_elf",
      criterion: (required_subtype: "Elf"),
      if_beheld: [Untap(subject: PreviousEffectObject)],
    ))),
    (id: "tail_treasure", bind: [], input: None, kind: Perform(CreateTokens(
      token: "treasure", count: 1, who: Controller, tapped: false,
      sacrifice_timing: None,
    ))),
  ], yields: []), target_bindings: [])),
)"#;

const TYPED_CREATURE_BATTLEFIELD_SEARCH_FIXTURE: &str = r#"(
  id: "direct_battlefield_search_fixture",
  name: "Direct Battlefield Search Fixture",
  face_id: "direct_battlefield_search_fixture",
  mana_cost: "{0}",
  types: ["Instant"],
  spell_program: Some((body: (steps: [
    (id: "search_creature", bind: ["found_creature"], input: None,
      kind: Perform(SearchLibrary(
        who: Controller, optional: false, count: 2, count_by_cast_cost: None,
        filter: Some((card_type: Some(Creature))),
        selection_constraint: None, slots: [], zones: Fixed([Library]),
        destination: Battlefield(tapped: true), conditional_destination: None,
        shuffle: false, reveal: true,
        selection_bounds: Some((min: 0, max: 1)),
      ))),
    (id: "scry_after_search", bind: [], input: None,
      kind: Perform(Scry(count: 0))),
    (id: "behold_elf", bind: [], input: Some("found_creature"),
      kind: Perform(MayBehold(
        who: Controller, choice_id: "behold_elf",
        criterion: (required_subtype: "Elf"),
        if_beheld: [Untap(subject: PreviousEffectObject)],
      ))),
    (id: "tail_treasure", bind: [], input: None,
      kind: Perform(CreateTokens(
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

const TEST_KINDRED_ARTIFACT_DRAGON: &str = r#"(
  id: "test_kindred_artifact_dragon",
  name: "Test Kindred Artifact Dragon",
  face_id: "test_kindred_artifact_dragon",
  mana_cost: "{0}",
  types: ["Kindred", "Artifact", "Dragon"],
)"#;

const TEST_ELF_PERMANENT: &str = r#"(
  id: "test_elf_permanent",
  name: "Test Elf Permanent",
  face_id: "test_elf_permanent",
  mana_cost: "{G}",
  types: ["Creature", "Elf"],
  power: 1,
  toughness: 1,
)"#;

// Test-only validated composition: the same exact closed Embermouth conditional leaf followed
// by the shared Controller Draw tail. Production Embermouth remains its Oracle-exact search only.
const CONDITIONAL_BATTLEFIELD_SEARCH_TAIL_FIXTURE: &str = r#"(
  id: "conditional_battlefield_search_tail_fixture",
  name: "Conditional Battlefield Search Tail Fixture",
  face_id: "conditional_battlefield_search_tail_fixture",
  mana_cost: "{0}",
  types: ["Creature", "Wolf"],
  power: 1,
  toughness: 1,
  triggered_abilities: [(
    ability_id: "search_on_enter",
    presentation: Fallback,
    trigger: WhenSelfEntersBattlefield,
    effect: [],
    program: Some((body: (steps: [
      (id: "choose_search", bind: [], input: None, kind: Choose(
        choice: "search", chooser: Controller, optional: true,
        branches: [(branch_id: "search_for_a_basic_land", presentation: Fallback,
          cost: None, requirement: Always, body: (steps: [
            (id: "search_battlefield", bind: [], input: None, kind: Perform(SearchLibrary(
              who: Controller, optional: false, count: 1, count_by_cast_cost: None,
              filter: Some((card_type: Some(BasicLand))), selection_constraint: None, slots: [],
              zones: Fixed([Library]), destination: TopOfLibrary,
              conditional_destination: Some((condition: BattlefieldAggregate(
                filter: (controllers: Controller, card_type: None,
                  required_subtypes: ["Dragon"], exclude_source: false),
                aggregate: Count, min: Some(1), max: None, exclude_observed_object: false,
              ), destination: Battlefield(tapped: true))),
              shuffle: true, reveal: true,
              selection_bounds: Some((min: 0, max: 1)),
            )))
          ], yields: []))],
        otherwise: (steps: [], yields: []),
      )),
      (id: "tail_draw", bind: [], input: None, kind: Perform(Draw(who: Controller, count: 1))),
    ], yields: []), target_bindings: [])),
    modal: None,
    targeting: None,
    may: false,
    intervening_if: None,
    triggers_only_once: false,
    max_triggers_per_turn: None,
  )],
)"#;

const RECURSIVE_CONDITION_SEARCH_FIXTURE: &str = r#"(
  id: "recursive_condition_search_fixture",
  name: "Recursive Condition Search Fixture",
  face_id: "recursive_condition_search_fixture",
  mana_cost: "{0}",
  types: ["Creature", "Wolf"],
  power: 1,
  toughness: 1,
  triggered_abilities: [(
    ability_id: "search_on_enter",
    presentation: Fallback,
    trigger: WhenSelfEntersBattlefield,
    effect: [],
    program: Some((body: (steps: [
      (id: "choose_search", bind: [], input: None, kind: Choose(
        choice: "search", chooser: Controller, optional: true,
        branches: [(branch_id: "search_for_a_basic_land", presentation: Fallback,
          cost: None, requirement: Always, body: (steps: [
            (id: "search_battlefield", bind: [], input: None, kind: Perform(SearchLibrary(
              who: Controller, optional: false, count: 1, count_by_cast_cost: None,
              filter: Some((card_type: Some(BasicLand))), selection_constraint: None, slots: [],
              zones: Fixed([Library]), destination: TopOfLibrary,
              conditional_destination: Some((condition: AllOf([
                CardsInHand(players: Relative(Controller), min: Some(1)),
                AnyOf([
                  BattlefieldAggregate(
                    filter: (controllers: Controller, required_subtypes: ["Dragon"]),
                    aggregate: Count, min: Some(1), max: None,
                    exclude_observed_object: false,
                  ),
                  ControllerLibraryEmpty,
                ]),
              ]), destination: Battlefield(tapped: true))),
              shuffle: true, reveal: true,
              selection_bounds: Some((min: 0, max: 1)),
            )))
          ], yields: []))],
        otherwise: (steps: [], yields: []),
      )),
      (id: "tail_draw", bind: [], input: None, kind: Perform(Draw(who: Controller, count: 1))),
    ], yields: []), target_bindings: [])),
    modal: None,
    targeting: None,
    may: false,
    intervening_if: None,
    triggers_only_once: false,
    max_triggers_per_turn: None,
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
              shuffle: true, reveal: false,
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
    test_engine_with_search_fixture(seed, DIRECT_BATTLEFIELD_SEARCH_FIXTURE)
}

fn direct_battlefield_search_fixture_with_bounds(count: u32, max: u32) -> String {
    let fixture = DIRECT_BATTLEFIELD_SEARCH_FIXTURE
        .replace(
            "count: 1, count_by_cast_cost: None",
            &format!("count: {count}, count_by_cast_cost: None"),
        )
        .replace(
            "selection_bounds: Some((min: 0, max: 1))",
            &format!("selection_bounds: Some((min: 0, max: {max}))"),
        );
    assert_eq!(
        (count, max),
        (2, 2),
        "this fixture helper is reserved for the combined two-entry case"
    );
    fixture
}

fn test_engine_with_search_fixture(seed: u64, search_fixture: &str) -> GameEngine {
    let registry = Box::leak(Box::new(
        CardRegistry::from_chunks_and_tokens(
            &[
                search_fixture,
                TEST_BASIC_LAND_CREATURE_WITH_ENTRY_CHOICE,
                TEST_ELF_PERMANENT,
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

fn move_fixture_land_creature_to_graveyard(engine: &mut GameEngine) -> u32 {
    let object_id =
        take_oid_from_library_or_hand(engine, 0, "test_basic_land_creature_with_entry_choice");
    engine.state.players[0].graveyard.push(object_id);
    engine.state.objects.get_mut(&object_id).unwrap().zone = Zone::Graveyard;
    *engine
        .state
        .zone_change_generation
        .entry(object_id)
        .or_default() += 1;
    object_id
}

fn shuffle_log_count(batch: &tricerules_proto::ruled::v1::RuledEventBatch, player: i32) -> usize {
    batch
        .events
        .iter()
        .filter(|event| {
            matches!(
                &event.ev,
                Some(Ev::Log(log)) if log.text == format!("P{player} shuffles their library.")
            )
        })
        .count()
}

fn assert_no_search_reveal(batch: &tricerules_proto::ruled::v1::RuledEventBatch) {
    assert!(batch
        .events
        .iter()
        .all(|event| !matches!(&event.ev, Some(Ev::CardsRevealed(_)))));
}

fn conditional_search_tail_engine(seed: u64) -> GameEngine {
    let registry = Box::leak(Box::new(
        CardRegistry::from_chunks_and_tokens(
            &[
                CONDITIONAL_BATTLEFIELD_SEARCH_TAIL_FIXTURE,
                TEST_BASIC_LAND_CREATURE_WITH_ENTRY_CHOICE,
                TEST_KINDRED_ARTIFACT_DRAGON,
                include_str!("../../../tricerules-cards/data/forest.ron"),
                include_str!("../../../tricerules-cards/data/island.ron"),
                include_str!("../../../tricerules-cards/data/wan_shi_tong,_librarian.ron"),
            ],
            &[],
        )
        .expect("the conditional-search tail registry is valid"),
    ));
    let decks = Some(vec![
        EngineDeck {
            mainboard: [
                vec!["conditional_battlefield_search_tail_fixture".into()],
                vec!["forest".into(); 8],
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
        .expect("two-player conditional-search tail engine");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn recursive_condition_search_engine(seed: u64) -> GameEngine {
    let registry = Box::leak(Box::new(
        CardRegistry::from_chunks_and_tokens(
            &[
                RECURSIVE_CONDITION_SEARCH_FIXTURE,
                TEST_KINDRED_ARTIFACT_DRAGON,
                include_str!("../../../tricerules-cards/data/forest.ron"),
                include_str!("../../../tricerules-cards/data/island.ron"),
                include_str!("../../../tricerules-cards/data/wan_shi_tong,_librarian.ron"),
            ],
            &[],
        )
        .expect("the recursive conditional-search registry is valid"),
    ));
    let decks = Some(vec![
        EngineDeck {
            mainboard: [
                vec!["recursive_condition_search_fixture".into()],
                vec!["forest".into(); 8],
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
        .expect("two-player recursive conditional-search engine");
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

fn embermouth_conditional_search_engine(seed: u64) -> (GameEngine, &'static CardRegistry) {
    let registry = Box::leak(Box::new(
        CardRegistry::from_chunks_and_tokens(
            &[
                include_str!("../../../tricerules-cards/data/embermouth_sentinel.ron"),
                TEST_BASIC_LAND_CREATURE_WITH_ENTRY_CHOICE,
                TEST_KINDRED_ARTIFACT_DRAGON,
                include_str!("../../../tricerules-cards/data/forest.ron"),
                include_str!("../../../tricerules-cards/data/island.ron"),
                include_str!("../../../tricerules-cards/data/wan_shi_tong,_librarian.ron"),
            ],
            &[],
        )
        .expect("the Embermouth conditional-search registry is valid"),
    ));
    let decks = Some(vec![
        EngineDeck {
            mainboard: [vec!["embermouth_sentinel".into()], vec!["forest".into(); 8]].concat(),
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
        .expect("two-player Embermouth conditional-search engine");
    advance_to_main1_from_game_start(&mut engine);
    (engine, registry)
}

#[test]
fn embermouth_accepts_noncreature_kindred_dragon_and_locks_destination_through_entry_choice() {
    let (mut engine, registry) = embermouth_conditional_search_engine(284_207);
    let kindred_face = registry
        .get("test_kindred_artifact_dragon")
        .expect("test Kindred artifact");
    assert!(!kindred_face.primary_face().is_creature);
    assert!(kindred_face
        .primary_face()
        .types
        .iter()
        .any(|card_type| card_type == "Kindred"));
    assert!(kindred_face
        .primary_face()
        .types
        .iter()
        .any(|subtype| subtype == "Dragon"));

    let wan = install_search_observer(&mut engine);
    let dragon = inject_permanent_on_battlefield(&mut engine, 0, "test_kindred_artifact_dragon");
    let selected =
        inject_library_card(&mut engine, 0, "test_basic_land_creature_with_entry_choice");
    let selected_generation = engine
        .state
        .zone_change_generation
        .get(&selected)
        .copied()
        .unwrap_or(0);

    let search_batch = open_embermouth_library_search(&mut engine);
    let search = find_resolution_choice(&search_batch).expect("Embermouth search offer");
    assert_eq!(search.choice_kind(), ChoiceKind::LibrarySearch);
    assert!(search.candidate_object_ids.contains(&selected));
    let entry_batch = engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![selected]))
        .expect("select the exact Basic Land Creature");
    let entry_choice = find_resolution_choice(&entry_batch).expect("as-enters type choice");
    let dragon_type = entry_choice
        .resolution_branches
        .iter()
        .find(|branch| branch.label == "Dragon")
        .expect("Dragon is a legal creature type");

    // The accepted search has fixed its Battlefield destination. Remove the matching permanent
    // before the asynchronous entry replacement resumes to prove the condition is not reevaluated.
    engine.state.players[0]
        .battlefield
        .retain(|object_id| *object_id != dragon);
    engine.state.players[0].graveyard.push(dragon);
    engine.state.objects.get_mut(&dragon).unwrap().zone = Zone::Graveyard;
    *engine
        .state
        .zone_change_generation
        .entry(dragon)
        .or_default() += 1;
    let completion = engine
        .apply_scenario_command(
            0,
            &choose_branch_for_prompt(entry_choice.resolution_prompt_id, dragon_type.branch_index),
        )
        .expect("finish the selected card's entry choice");

    assert_eq!(engine.state.objects[&dragon].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&selected].zone, Zone::Battlefield);
    assert!(engine.state.players[0].battlefield.contains(&selected));
    assert!(engine.state.objects[&selected].tapped);
    assert_eq!(
        engine
            .state
            .zone_change_generation
            .get(&selected)
            .copied()
            .unwrap_or(0),
        selected_generation + 1,
        "the selected physical object enters as one new incarnation"
    );
    assert_eq!(
        entry_batch
            .events
            .iter()
            .filter_map(|event| match &event.ev {
                Some(Ev::CardsRevealed(reveal)) => Some(reveal),
                _ => None,
            })
            .flat_map(|reveal| reveal.cards.iter())
            .filter(|card| card.object_id == selected)
            .count(),
        1,
        "Embermouth reveals the selected card once"
    );
    assert_eq!(
        search_batch
            .events
            .iter()
            .chain(entry_batch.events.iter())
            .chain(completion.events.iter())
            .filter(|event| matches!(
                &event.ev,
                Some(Ev::Log(log)) if log.text == "P0 shuffles their library."
            ))
            .count(),
        1,
        "one completed library search shuffles once across the parked entry choice"
    );
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(engine.state.stack.len(), 1, "one LibrarySearched observer");
    assert_eq!(engine.state.stack[0].source_permanent_id, Some(wan));
}

fn open_embermouth_library_search(
    engine: &mut GameEngine,
) -> tricerules_proto::ruled::v1::RuledEventBatch {
    ensure_in_hand(engine, 0, "embermouth_sentinel");
    give_mana(
        engine,
        0,
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );
    let hand_index = hand_index_for_card(engine, 0, "embermouth_sentinel");
    engine
        .apply_scenario_command(0, &cast_spell(hand_index, Vec::new()))
        .expect("cast Embermouth Sentinel");
    pass_both_players(engine);
    engine
        .apply_scenario_command(0, &pass())
        .expect("controller passes for the ETB trigger");
    let offer = engine
        .apply_scenario_command(1, &pass())
        .expect("the ETB trigger offers its optional search");
    let choice = find_resolution_choice(&offer).expect("optional search choice");
    assert_eq!(choice.deciding_player_id, 0);
    assert_eq!(choice.choice_kind(), ChoiceKind::ResolutionBranch);
    engine
        .apply_scenario_command(0, &choose_branch(0))
        .expect("choose the search branch")
}

fn open_conditional_tail_library_search(
    engine: &mut GameEngine,
) -> tricerules_proto::ruled::v1::RuledEventBatch {
    ensure_in_hand(engine, 0, "conditional_battlefield_search_tail_fixture");
    let hand_index = hand_index_for_card(engine, 0, "conditional_battlefield_search_tail_fixture");
    engine
        .apply_scenario_command(0, &cast_spell(hand_index, Vec::new()))
        .expect("cast the zero-cost conditional search fixture");
    pass_both_players(engine);
    engine
        .apply_scenario_command(0, &pass())
        .expect("controller passes for the fixture ETB trigger");
    let offer = engine
        .apply_scenario_command(1, &pass())
        .expect("fixture ETB offers its optional conditional search");
    let choice = find_resolution_choice(&offer).expect("optional conditional search choice");
    assert_eq!(choice.deciding_player_id, 0);
    assert_eq!(choice.choice_kind(), ChoiceKind::ResolutionBranch);
    engine
        .apply_scenario_command(0, &choose_branch(0))
        .expect("choose the conditional search branch")
}

fn open_recursive_condition_library_search(
    engine: &mut GameEngine,
) -> tricerules_proto::ruled::v1::RuledEventBatch {
    ensure_in_hand(engine, 0, "recursive_condition_search_fixture");
    let hand_index = hand_index_for_card(engine, 0, "recursive_condition_search_fixture");
    engine
        .apply_scenario_command(0, &cast_spell(hand_index, Vec::new()))
        .expect("cast the zero-cost recursive-condition fixture");
    pass_both_players(engine);
    engine
        .apply_scenario_command(0, &pass())
        .expect("controller passes for the fixture ETB trigger");
    let offer = engine
        .apply_scenario_command(1, &pass())
        .expect("fixture ETB offers its optional conditional search");
    let choice = find_resolution_choice(&offer).expect("optional conditional search choice");
    assert_eq!(choice.deciding_player_id, 0);
    assert_eq!(choice.choice_kind(), ChoiceKind::ResolutionBranch);
    engine
        .apply_scenario_command(0, &choose_branch(0))
        .expect("choose the recursive conditional search branch")
}

fn assert_one_conditional_search_completion(
    engine: &GameEngine,
    wan: u32,
    selected: u32,
    batches: &[&tricerules_proto::ruled::v1::RuledEventBatch],
) {
    let events = batches.iter().flat_map(|batch| batch.events.iter());
    assert_eq!(
        events
            .clone()
            .filter(|event| matches!(
                &event.ev,
                Some(Ev::Log(log)) if log.text == "P0 shuffles their library."
            ))
            .count(),
        1,
        "the conditional search shuffles once"
    );
    let revealed = events
        .filter_map(|event| match &event.ev {
            Some(Ev::CardsRevealed(reveal)) => Some(reveal),
            _ => None,
        })
        .flat_map(|reveal| reveal.cards.iter())
        .collect::<Vec<_>>();
    assert_eq!(revealed.len(), 1, "the accepted selection is revealed once");
    assert_eq!(revealed[0].object_id, selected);
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(engine.state.stack.len(), 1, "one LibrarySearched observer");
    assert_eq!(engine.state.stack[0].source_permanent_id, Some(wan));
}

#[test]
fn conditional_search_top_result_is_drawn_by_the_typed_tail_once() {
    let mut engine = conditional_search_tail_engine(284_208);
    let wan = install_search_observer(&mut engine);
    let selected = inject_library_card(&mut engine, 0, "forest");
    let search_batch = open_conditional_tail_library_search(&mut engine);
    let search = find_resolution_choice(&search_batch).expect("conditional top search offer");
    assert_eq!(search.choice_kind(), ChoiceKind::LibrarySearch);
    assert!(search.candidate_object_ids.contains(&selected));
    let hand_before_tail = engine.state.players[0].hand.len();

    let completion = engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![selected]))
        .expect("accept the resultless top search");
    assert_eq!(engine.state.objects[&selected].zone, Zone::Hand);
    assert!(engine.state.players[0].hand.contains(&selected));
    assert_eq!(
        engine.state.players[0].hand.len(),
        hand_before_tail + 1,
        "the typed Controller Draw tail runs exactly once"
    );
    assert_one_conditional_search_completion(&engine, wan, selected, &[&completion]);
}

#[test]
fn recursive_conditional_search_uses_the_live_program_condition_at_choice_time() {
    for (seed, remove_dragon_before_answer, enters_battlefield) in
        [(284_219, false, true), (284_220, true, false)]
    {
        let mut engine = recursive_condition_search_engine(seed);
        inject_card_into_hand(&mut engine, 0, "island");
        let dragon =
            inject_permanent_on_battlefield(&mut engine, 0, "test_kindred_artifact_dragon");
        let selected = inject_library_card(&mut engine, 0, "forest");
        let search_batch = open_recursive_condition_library_search(&mut engine);
        let search = find_resolution_choice(&search_batch).expect("conditional search offer");
        assert_eq!(search.choice_kind(), ChoiceKind::LibrarySearch);
        assert!(search.candidate_object_ids.contains(&selected));
        let hand_before_tail = engine.state.players[0].hand.len();

        if remove_dragon_before_answer {
            engine.state.players[0]
                .battlefield
                .retain(|object_id| *object_id != dragon);
            engine.state.players[0].graveyard.push(dragon);
            engine.state.objects.get_mut(&dragon).unwrap().zone = Zone::Graveyard;
            *engine
                .state
                .zone_change_generation
                .entry(dragon)
                .or_default() += 1;
        }

        let completion = engine
            .apply_scenario_command(0, &submit_resolution_choice(vec![selected]))
            .expect("submit the offered Basic Land");
        assert_eq!(
            engine.state.objects[&selected].zone,
            if enters_battlefield {
                Zone::Battlefield
            } else {
                Zone::Hand
            },
            "the nested live predicate controls the destination before the parent Draw tail"
        );
        if enters_battlefield {
            assert!(!engine.state.players[0].hand.contains(&selected));
        } else {
            assert!(engine.state.players[0].hand.contains(&selected));
        }
        assert_eq!(
            engine.state.players[0].hand.len(),
            hand_before_tail + 1,
            "the Draw instruction after the search runs once"
        );
        assert!(engine.state.pending_resolution.is_none());
        assert_eq!(
            completion
                .events
                .iter()
                .filter(|event| matches!(
                    &event.ev,
                    Some(Ev::Log(log))
                        if log.text == "P0 draws 1 card (Recursive Condition Search Fixture)."
                ))
                .count(),
            1,
            "the parent Draw tail runs exactly once"
        );
    }
}

#[test]
fn conditional_search_synchronous_battlefield_result_runs_tail_once() {
    let mut engine = conditional_search_tail_engine(284_209);
    let wan = install_search_observer(&mut engine);
    inject_permanent_on_battlefield(&mut engine, 0, "test_kindred_artifact_dragon");
    let selected = inject_library_card(&mut engine, 0, "forest");
    let selected_generation = engine
        .state
        .zone_change_generation
        .get(&selected)
        .copied()
        .unwrap_or(0);
    let search_batch = open_conditional_tail_library_search(&mut engine);
    let search =
        find_resolution_choice(&search_batch).expect("conditional Battlefield search offer");
    assert_eq!(search.choice_kind(), ChoiceKind::LibrarySearch);
    assert!(search.candidate_object_ids.contains(&selected));
    let hand_before_tail = engine.state.players[0].hand.len();

    let completion = engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![selected]))
        .expect("accept the conditional Battlefield destination");
    assert_eq!(engine.state.objects[&selected].zone, Zone::Battlefield);
    assert!(engine.state.objects[&selected].tapped);
    assert_eq!(
        engine
            .state
            .zone_change_generation
            .get(&selected)
            .copied()
            .unwrap_or(0),
        selected_generation + 1
    );
    assert_eq!(
        engine.state.players[0].hand.len(),
        hand_before_tail + 1,
        "the typed Controller Draw tail runs exactly once"
    );
    assert_one_conditional_search_completion(&engine, wan, selected, &[&completion]);
}

#[test]
fn conditional_search_async_battlefield_result_locks_destination_and_resumes_tail_once() {
    let mut engine = conditional_search_tail_engine(284_210);
    let wan = install_search_observer(&mut engine);
    let dragon = inject_permanent_on_battlefield(&mut engine, 0, "test_kindred_artifact_dragon");
    let selected =
        inject_library_card(&mut engine, 0, "test_basic_land_creature_with_entry_choice");
    let selected_generation = engine
        .state
        .zone_change_generation
        .get(&selected)
        .copied()
        .unwrap_or(0);
    let search_batch = open_conditional_tail_library_search(&mut engine);
    let search = find_resolution_choice(&search_batch).expect("conditional async search offer");
    assert!(search.candidate_object_ids.contains(&selected));
    let hand_before_tail = engine.state.players[0].hand.len();
    let entry_batch = engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![selected]))
        .expect("accept the conditional Battlefield entrant");
    let entry_choice = find_resolution_choice(&entry_batch).expect("as-enters creature type");
    let dragon_type = entry_choice
        .resolution_branches
        .iter()
        .find(|branch| branch.label == "Dragon")
        .expect("Dragon is a legal creature type");

    engine.state.players[0]
        .battlefield
        .retain(|object_id| *object_id != dragon);
    engine.state.players[0].graveyard.push(dragon);
    engine.state.objects.get_mut(&dragon).unwrap().zone = Zone::Graveyard;
    *engine
        .state
        .zone_change_generation
        .entry(dragon)
        .or_default() += 1;
    let completion = engine
        .apply_scenario_command(
            0,
            &choose_branch_for_prompt(entry_choice.resolution_prompt_id, dragon_type.branch_index),
        )
        .expect("complete the entrant's replacement choice");

    assert_eq!(engine.state.objects[&dragon].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&selected].zone, Zone::Battlefield);
    assert!(engine.state.objects[&selected].tapped);
    assert_eq!(
        engine
            .state
            .zone_change_generation
            .get(&selected)
            .copied()
            .unwrap_or(0),
        selected_generation + 1
    );
    assert_eq!(
        engine.state.players[0].hand.len(),
        hand_before_tail + 1,
        "the typed tail resumes once, after the as-enters choice commits"
    );
    assert_one_conditional_search_completion(&engine, wan, selected, &[&entry_batch, &completion]);
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

struct ParkedTypedBehold {
    engine: GameEngine,
    wan: u32,
    elf: u32,
    selected: u32,
    generation_before: u64,
}

struct TypedSearchAfterEntry {
    engine: GameEngine,
    wan: u32,
    elf: Option<u32>,
    selected: u32,
    generation_before: u64,
    entry_completion: tricerules_proto::ruled::v1::RuledEventBatch,
}

fn complete_typed_search_entry(seed: u64, include_elf: bool) -> TypedSearchAfterEntry {
    let mut engine = test_engine_with_search_fixture(seed, TYPED_BATTLEFIELD_SEARCH_BEHOLD_FIXTURE);
    let wan = install_search_observer(&mut engine);
    let elf =
        include_elf.then(|| inject_permanent_on_battlefield(&mut engine, 0, "test_elf_permanent"));
    let selected =
        inject_library_card(&mut engine, 0, "test_basic_land_creature_with_entry_choice");
    let generation_before = engine
        .state
        .zone_change_generation
        .get(&selected)
        .copied()
        .unwrap_or(0);

    let search_batch = cast_fixture(&mut engine);
    let search = find_resolution_choice(&search_batch).expect("the BasicLand search choice");
    assert_eq!(search.choice_kind(), ChoiceKind::LibrarySearch);
    assert!(search.candidate_object_ids.contains(&selected));
    assert_no_search_reveal(&search_batch);

    let entry_batch = engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![selected]))
        .expect("choose the Basic Land Creature");
    let entry_choice = find_resolution_choice(&entry_batch).expect("as-enters type choice");
    assert_eq!(entry_choice.choice_kind(), ChoiceKind::ResolutionBranch);
    assert_no_search_reveal(&entry_batch);
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("the result-producing search is parked before entry commits");
    let serialized_pending =
        serde_json::to_value(pending).expect("the parked entry continuation is serializable");
    let program =
        &serialized_pending["continuation"]["EntryChooseCreatureType"]["stack"]["program"];
    assert!(program["slots"][0].is_null());
    assert!(program["frames"].is_array());

    let human = tricerules_card_model::CREATURE_TYPES
        .iter()
        .position(|creature_type| *creature_type == "Human")
        .expect("Human is a legal creature type") as u32;
    let entry_completion = engine
        .apply_scenario_command(0, &choose_branch(human))
        .expect("complete the as-enters replacement");
    assert_eq!(engine.state.objects[&selected].zone, Zone::Battlefield);
    assert!(engine.state.objects[&selected].tapped);
    assert_eq!(
        engine.state.zone_change_generation[&selected],
        generation_before + 1
    );
    assert_no_search_reveal(&entry_completion);

    assert_eq!(shuffle_log_count(&search_batch, 0), 0);
    assert_eq!(shuffle_log_count(&entry_batch, 0), 0);
    assert_eq!(
        shuffle_log_count(&entry_completion, 0),
        1,
        "the committed search shuffles exactly once before any Behold prompt"
    );

    TypedSearchAfterEntry {
        engine,
        wan,
        elf,
        selected,
        generation_before,
        entry_completion,
    }
}

fn open_typed_search_behold_choice(seed: u64) -> ParkedTypedBehold {
    let TypedSearchAfterEntry {
        engine,
        wan,
        elf,
        selected,
        generation_before,
        entry_completion,
    } = complete_typed_search_entry(seed, true);
    let elf = elf.expect("the test setup includes an eligible battlefield Elf");

    let behold = find_resolution_choice(&entry_completion).expect("Behold the battlefield Elf");
    assert_eq!(behold.choice_kind(), ChoiceKind::Behold);
    assert_eq!(behold.candidate_object_ids, vec![elf]);
    assert_eq!(
        behold.candidate_source_zones,
        vec![ChoiceCandidateSourceZone::Battlefield as i32]
    );
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("the Behold choice parks the typed Program cursor");
    let serialized_pending =
        serde_json::to_value(pending).expect("the parked Behold continuation is serializable");
    let program = &serialized_pending["continuation"]["BeholdChoice"]["stack"]["program"];
    let slots = &program["slots"];
    let receipt = &slots[0]["PermanentSelection"][0];
    assert_eq!(receipt["object_id"].as_u64(), Some(selected as u64));
    assert_eq!(
        receipt["zone_change_generation"].as_u64(),
        Some(generation_before + 1),
        "the typed search result captures the post-entry incarnation"
    );
    assert_eq!(receipt["controller_at_event"].as_i64(), Some(0));
    let frames = program["frames"].as_array().expect("saved Program frames");
    assert_eq!(frames.len(), 1);
    assert_eq!(
        frames[0]["next_instruction"].as_u64(),
        Some(3),
        "the parked cursor follows SearchLibrary and the intervening Scry(0)"
    );
    ParkedTypedBehold {
        engine,
        wan,
        elf,
        selected,
        generation_before,
    }
}

fn assert_typed_behold_completion(
    engine: &GameEngine,
    wan: u32,
    completion: &tricerules_proto::ruled::v1::RuledEventBatch,
) {
    assert_eq!(
        treasure_count(engine),
        1,
        "the saved Program tail runs once"
    );
    assert_no_search_reveal(completion);
    assert_eq!(
        shuffle_log_count(completion, 0),
        0,
        "answering Behold does not repeat the search shuffle"
    );
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(engine.state.stack.len(), 1, "one LibrarySearched observer");
    assert_eq!(engine.state.stack[0].source_permanent_id, Some(wan));
}

#[test]
fn typed_battlefield_search_untaps_its_exact_post_entry_result_after_scry() {
    let mut parked = open_typed_search_behold_choice(284_207);
    assert!(parked.engine.state.objects[&parked.selected].tapped);

    let completion = parked
        .engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![parked.elf]))
        .expect("behold the offered battlefield Elf");

    assert!(!parked.engine.state.objects[&parked.selected].tapped);
    assert_eq!(
        parked.engine.state.zone_change_generation[&parked.selected],
        parked.generation_before + 1,
        "the accepted Behold uses the exact searched incarnation"
    );
    assert_typed_behold_completion(&parked.engine, parked.wan, &completion);
}

#[test]
fn typed_battlefield_search_stale_result_cannot_untap_a_later_incarnation() {
    let mut parked = open_typed_search_behold_choice(284_208);

    // Simulate a later incarnation with the same physical id; the stale result cannot retarget it.
    parked
        .engine
        .state
        .zone_change_generation
        .insert(parked.selected, parked.generation_before + 2);
    let completion = parked
        .engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![parked.elf]))
        .expect("behold the offered battlefield Elf");
    assert!(parked.engine.state.objects[&parked.selected].tapped);
    assert_typed_behold_completion(&parked.engine, parked.wan, &completion);
}

#[test]
fn typed_battlefield_search_declining_behold_resumes_tail_once_without_research() {
    let mut parked = open_typed_search_behold_choice(284_209);
    assert!(parked.engine.state.objects[&parked.selected].tapped);

    let completion = parked
        .engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![]))
        .expect("decline the Behold choice");

    assert!(parked.engine.state.objects[&parked.selected].tapped);
    assert_typed_behold_completion(&parked.engine, parked.wan, &completion);
}

#[test]
fn typed_battlefield_search_without_eligible_elf_skips_behold_and_resumes_tail_once() {
    let setup = complete_typed_search_entry(284_210, false);
    assert!(find_resolution_choice(&setup.entry_completion).is_none());
    assert!(setup.engine.state.pending_resolution.is_none());
    assert!(setup.engine.state.objects[&setup.selected].tapped);
    assert_eq!(
        setup.engine.state.zone_change_generation[&setup.selected],
        setup.generation_before + 1,
        "the found land remains the same committed incarnation"
    );
    assert_eq!(
        treasure_count(&setup.engine),
        1,
        "the saved Program tail runs once when Behold has no candidates"
    );
    assert_no_search_reveal(&setup.entry_completion);
    assert_eq!(
        shuffle_log_count(&setup.entry_completion, 0),
        1,
        "the search and shuffle complete once without repeating after the skipped prompt"
    );
    assert_eq!(
        setup.engine.state.stack.len(),
        1,
        "one LibrarySearched observer"
    );
    assert_eq!(
        setup.engine.state.stack[0].source_permanent_id,
        Some(setup.wan)
    );
}

#[test]
fn typed_creature_battlefield_search_binds_exact_revealed_result_with_count_two_and_max_one() {
    let mut engine =
        test_engine_with_search_fixture(284_211, TYPED_CREATURE_BATTLEFIELD_SEARCH_FIXTURE);
    move_fixture_land_creature_to_graveyard(&mut engine);
    let selected = inject_library_card(&mut engine, 0, "test_elf_permanent");
    let alternative =
        inject_library_card(&mut engine, 0, "test_basic_land_creature_with_entry_choice");
    let generation_before = engine
        .state
        .zone_change_generation
        .get(&selected)
        .copied()
        .unwrap_or(0);

    let search_offer = cast_fixture(&mut engine);
    let search = find_resolution_choice(&search_offer).expect("Creature library-search offer");
    assert_eq!(search.choice_kind(), ChoiceKind::LibrarySearch);
    assert_eq!((search.min, search.max), (0, 1));
    assert_eq!(search.candidate_object_ids.len(), 2);
    assert!(search.candidate_object_ids.contains(&selected));
    assert!(search.candidate_object_ids.contains(&alternative));
    assert_no_search_reveal(&search_offer);

    let search_completion = engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![selected]))
        .expect("choose the exact nonland Creature result");
    assert_eq!(engine.state.objects[&selected].zone, Zone::Battlefield);
    assert!(engine.state.objects[&selected].tapped);
    assert_eq!(engine.state.objects[&alternative].zone, Zone::Library);
    assert!(engine.state.players[0].library.contains(&alternative));
    assert_eq!(
        engine.state.zone_change_generation[&selected],
        generation_before + 1
    );
    let reveal = search_completion
        .events
        .iter()
        .find_map(|event| match event.ev.as_ref() {
            Some(Ev::CardsRevealed(reveal)) => Some(reveal),
            _ => None,
        })
        .expect("reveal:true publishes the selected search result after the private offer");
    assert_eq!(reveal.cards.len(), 1);
    assert_eq!(reveal.cards[0].object_id, selected);
    assert_eq!(reveal.cards[0].card_id, "test_elf_permanent");

    let behold = find_resolution_choice(&search_completion).expect("Behold choice");
    assert_eq!(behold.choice_kind(), ChoiceKind::Behold);
    assert!(behold.candidate_object_ids.contains(&selected));
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("the search result remains bound while the later Behold choice is pending");
    let serialized_pending = serde_json::to_value(pending).expect("pending choice serializes");
    let program = &serialized_pending["continuation"]["BeholdChoice"]["stack"]["program"];
    let receipt = &program["slots"][0]["PermanentSelection"][0];
    assert_eq!(receipt["object_id"].as_u64(), Some(selected as u64));
    assert_eq!(
        receipt["zone_change_generation"].as_u64(),
        Some(generation_before + 1),
        "the slot records the exact post-search incarnation passed to PreviousEffectObject"
    );

    let completion = engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![selected]))
        .expect("behold the searched Elf and resolve Untap");
    assert!(!engine.state.objects[&selected].tapped);
    assert_eq!(
        engine.state.zone_change_generation[&selected],
        generation_before + 1,
        "using PreviousEffectObject does not change the selected object's incarnation"
    );
    assert_eq!(treasure_count(&engine), 1, "the Program tail runs once");
    assert_eq!(shuffle_log_count(&search_completion, 0), 0);
    assert_eq!(shuffle_log_count(&completion, 0), 0);
    assert!(engine.state.pending_resolution.is_none());
}

#[test]
fn typed_creature_battlefield_search_no_match_keeps_hidden_library_cards_private_and_runs_tail() {
    let mut engine =
        test_engine_with_search_fixture(284_212, TYPED_CREATURE_BATTLEFIELD_SEARCH_FIXTURE);
    move_fixture_land_creature_to_graveyard(&mut engine);
    ensure_in_hand(&mut engine, 0, "direct_battlefield_search_fixture");
    assert!(
        !engine.state.players[0].library.is_empty(),
        "the no-match case uses a nonempty library"
    );
    assert!(engine.state.players[0]
        .library
        .iter()
        .all(|object_id| engine.state.objects[object_id].card_id == "forest"));

    let search_offer = cast_fixture(&mut engine);
    let search = find_resolution_choice(&search_offer).expect("empty Creature search offer");
    assert_eq!(search.choice_kind(), ChoiceKind::LibrarySearch);
    assert_eq!((search.min, search.max), (0, 1));
    assert!(search.candidate_object_ids.is_empty());
    assert_no_search_reveal(&search_offer);

    let completion = engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![]))
        .expect("decline an empty optional selection");
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(
        treasure_count(&engine),
        1,
        "the empty result runs the tail once"
    );
    assert!(completion
        .events
        .iter()
        .all(|event| { !matches!(event.ev, Some(Ev::CardsRevealed(_))) }));
    assert_eq!(shuffle_log_count(&completion, 0), 0);
}

#[test]
fn resultless_two_card_battlefield_search_retains_cursor_across_both_entry_choices() {
    let search_fixture = direct_battlefield_search_fixture_with_bounds(2, 2);
    let mut engine = test_engine_with_search_fixture(284_206, &search_fixture);
    let wan = install_search_observer(&mut engine);
    let first = inject_library_card(&mut engine, 0, "test_basic_land_creature_with_entry_choice");
    let second = inject_library_card(&mut engine, 0, "test_basic_land_creature_with_entry_choice");
    let generations_before = [first, second].map(|selected| {
        engine
            .state
            .zone_change_generation
            .get(&selected)
            .copied()
            .unwrap_or(0)
    });
    cast_fixture(&mut engine);
    let search_batch = engine
        .apply_scenario_command(0, &choose_branch(0))
        .expect("choose the mandatory two-card search branch");
    let search = find_resolution_choice(&search_batch).expect("private two-card BasicLand search");
    assert_eq!(search.choice_kind(), ChoiceKind::LibrarySearch);
    assert_eq!(search.deciding_player_id, 0);
    assert_eq!((search.min, search.max), (0, 2));
    assert!(search.candidate_object_ids.contains(&first));
    assert!(search.candidate_object_ids.contains(&second));
    assert_no_search_reveal(&search_batch);

    let entry_batch = engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![first, second]))
        .expect("select both Basic Land Creatures");
    let first_type_choice =
        find_resolution_choice(&entry_batch).expect("first as-enters creature-type choice");
    assert_eq!(
        first_type_choice.choice_kind(),
        ChoiceKind::ResolutionBranch
    );
    assert_eq!(first_type_choice.deciding_player_id, 0);
    let first_pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("the first entry replacement is parked");
    let serialized_first_pending =
        serde_json::to_value(first_pending).expect("first continuation is serializable");
    assert!(
        !serialized_first_pending["continuation"]["EntryChooseCreatureType"]["stack"]["program"]
            .is_null(),
        "the first ZoneEntryBatch choice retains the typed program cursor"
    );
    assert_eq!(
        treasure_count(&engine),
        0,
        "the typed tail waits for both entries"
    );

    let human = tricerules_card_model::CREATURE_TYPES
        .iter()
        .position(|creature_type| *creature_type == "Human")
        .expect("Human is a legal creature type") as u32;
    let first_entry_completion = engine
        .apply_scenario_command(0, &choose_branch(human))
        .expect("choose Human for the first entrant");
    let second_type_choice = find_resolution_choice(&first_entry_completion)
        .expect("the second entrant receives its own as-enters choice");
    assert_eq!(
        second_type_choice.choice_kind(),
        ChoiceKind::ResolutionBranch
    );
    assert_eq!(second_type_choice.deciding_player_id, 0);
    assert!(
        first_entry_completion.events.iter().any(|event| matches!(
            &event.ev,
            Some(Ev::Log(log)) if log.text == "P0 chooses Human for this permanent."
        )),
        "the first replacement applies to its own entrant"
    );
    let second_pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("the second entry replacement is parked");
    let serialized_second_pending =
        serde_json::to_value(second_pending).expect("second continuation is serializable");
    assert!(
        !serialized_second_pending["continuation"]["EntryChooseCreatureType"]["stack"]["program"]
            .is_null(),
        "the second ZoneEntryBatch choice still retains the typed program cursor"
    );
    assert_eq!(
        treasure_count(&engine),
        0,
        "the typed tail has not run while the second replacement is pending"
    );

    let elf = tricerules_card_model::CREATURE_TYPES
        .iter()
        .position(|creature_type| *creature_type == "Elf")
        .expect("Elf is a legal creature type") as u32;
    let second_entry_completion = engine
        .apply_scenario_command(0, &choose_branch(elf))
        .expect("choose Elf for the second entrant");
    assert!(
        second_entry_completion.events.iter().any(|event| matches!(
            &event.ev,
            Some(Ev::Log(log)) if log.text == "P0 chooses Elf for this permanent."
        )),
        "the second replacement applies to its own entrant"
    );
    assert_no_search_reveal(&entry_batch);
    assert_no_search_reveal(&first_entry_completion);
    assert_no_search_reveal(&second_entry_completion);
    let entry_order_answers = answer_simultaneous_entry_order_in_engine_order(&mut engine);
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(engine.state.objects[&first].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&second].zone, Zone::Battlefield);
    assert!(engine.state.objects[&first].tapped);
    assert!(engine.state.objects[&second].tapped);
    assert_eq!(
        engine.state.zone_change_generation[&first],
        generations_before[0] + 1
    );
    assert_eq!(
        engine.state.zone_change_generation[&second],
        generations_before[1] + 1
    );
    assert_eq!(
        treasure_count(&engine),
        1,
        "the typed tail runs once after both entries"
    );

    let shuffle_count = [
        shuffle_log_count(&search_batch, 0),
        shuffle_log_count(&entry_batch, 0),
        shuffle_log_count(&first_entry_completion, 0),
        shuffle_log_count(&second_entry_completion, 0),
    ]
    .into_iter()
    .sum::<usize>()
        + entry_order_answers
            .iter()
            .map(|(_, _, batch)| shuffle_log_count(batch, 0))
            .sum::<usize>();
    assert_eq!(
        shuffle_count, 1,
        "the search shuffles once after both entries commit"
    );
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
