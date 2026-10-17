//! Consumer scenarios for Tend the Sprigs' post-search Land-or-Treefolk count.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_cards::CardRegistry;
use tricerules_core::{EngineDeck, Zone};
use tricerules_proto::ruled::v1::ChoiceKind;

const TEST_LAND_TREEFOLK: &str = r#"(
  id: "test_land_treefolk",
  name: "Test Land Treefolk",
  face_id: "test_land_treefolk",
  mana_cost: "{0}",
  types: ["Land", "Creature", "Treefolk"],
  power: 1,
  toughness: 1,
)"#;

fn tend_engine(seed: u64) -> GameEngine {
    let registry = Box::leak(Box::new(
        CardRegistry::from_chunks_and_tokens(
            &[
                include_str!("../../../tricerules-cards/data/tend_the_sprigs.ron"),
                TEST_LAND_TREEFOLK,
                include_str!("../../../tricerules-cards/data/forest.ron"),
            ],
            &[include_str!(
                "../../../tricerules-cards/data/tokens/treefolk_g_3_4_reach.ron"
            )],
        )
        .expect("the Tend the Sprigs consumer registry is valid"),
    ));
    let decks = Some(vec![
        EngineDeck {
            mainboard: [vec!["tend_the_sprigs".into()], vec!["forest".into(); 18]].concat(),
            commanders: vec![],
        },
        EngineDeck {
            mainboard: vec!["forest".into(); 18],
            commanders: vec![],
        },
    ]);
    let mut engine = GameEngine::new_with_commander_decks(registry, seed, &[0, 1], 20, decks, true)
        .expect("two-player Tend the Sprigs engine");
    advance_to_main1_from_game_start(&mut engine);
    grant_pool(&mut engine, 0);
    grant_pool(&mut engine, 1);
    engine
}

fn cast_tend_and_open_search(engine: &mut GameEngine) -> Vec<u32> {
    inject_card_into_hand(engine, 0, "tend_the_sprigs");
    let hand_index = hand_index_for_card(engine, 0, "tend_the_sprigs");
    engine
        .apply_scenario_command(0, &cast_spell(hand_index, Vec::new()))
        .expect("cast Tend the Sprigs");
    let batch = pass_both_players_with_batch(engine);
    let search = find_resolution_choice(&batch).expect("Tend offers the Basic Land search");
    assert_eq!(search.deciding_player_id, 0);
    assert_eq!(search.choice_kind(), ChoiceKind::LibrarySearch);
    assert_eq!((search.min, search.max), (0, 1));
    search.candidate_object_ids
}

fn qualifying_permanent_count(engine: &GameEngine) -> usize {
    engine.state.players[0]
        .battlefield
        .iter()
        .filter(|object_id| {
            let characteristics = engine
                .characteristics(**object_id)
                .expect("battlefield object characteristics");
            characteristics.has_type("Land") || characteristics.has_type("Treefolk")
        })
        .count()
}

fn assert_no_automatic_branch_resolution_line(
    batch: &tricerules_proto::ruled::v1::RuledEventBatch,
) {
    assert!(
        batch.events.iter().all(|event| !matches!(
            &event.ev,
            Some(Ev::Log(log)) if log.text.starts_with("P0 resolves:")
        )),
        "authored Program If does not emit the legacy automatic branch line"
    );
}

#[test]
fn tend_searching_the_seventh_permanent_creates_one_treefolk() {
    let mut engine = tend_engine(710_101);
    let starting_lands: Vec<_> = (0..6)
        .map(|_| inject_permanent_on_battlefield(&mut engine, 0, "forest"))
        .collect();
    assert_eq!(starting_lands.len(), 6);
    assert_eq!(qualifying_permanent_count(&engine), 6);

    let selected = inject_library_card(&mut engine, 0, "forest");
    let candidates = cast_tend_and_open_search(&mut engine);
    assert!(candidates.contains(&selected));
    let completion = engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![selected]))
        .expect("select the seventh Basic Land");

    assert_eq!(engine.state.objects[&selected].zone, Zone::Battlefield);
    assert!(engine.state.objects[&selected].tapped);
    assert_eq!(qualifying_permanent_count(&engine), 8);
    let tokens = battlefield_token_oids(&engine, 0, "treefolk_g_3_4_reach");
    assert_eq!(
        tokens.len(),
        1,
        "the post-search threshold creates one token"
    );
    let token = engine
        .characteristics(tokens[0])
        .expect("Treefolk characteristics");
    assert_eq!((token.power, token.toughness), (Some(3), Some(4)));
    assert_eq!(token.colors, [tricerules_cards::Color::Green]);
    assert!(token.keywords.contains(&tricerules_cards::Keyword::Reach));
    assert_eq!(
        completion
            .events
            .iter()
            .filter(|event| matches!(&event.ev, Some(Ev::TokenCreated(created)) if created.object_id == tokens[0]))
            .count(),
        1
    );
    assert!(completion
        .events
        .iter()
        .all(|event| !matches!(&event.ev, Some(Ev::CardsRevealed(_)))));
    assert_no_automatic_branch_resolution_line(&completion);
}

#[test]
fn tend_fail_to_find_counts_overlapping_land_treefolk_once() {
    let mut engine = tend_engine(710_102);
    for _ in 0..5 {
        inject_permanent_on_battlefield(&mut engine, 0, "forest");
    }
    let overlap = inject_permanent_on_battlefield(&mut engine, 0, "test_land_treefolk");
    let overlap_characteristics = engine
        .characteristics(overlap)
        .expect("overlapping fixture characteristics");
    assert!(overlap_characteristics.has_type("Land"));
    assert!(overlap_characteristics.is_creature());
    assert!(overlap_characteristics.has_type("Treefolk"));
    assert_eq!(qualifying_permanent_count(&engine), 6);

    let available = inject_library_card(&mut engine, 0, "forest");
    let candidates = cast_tend_and_open_search(&mut engine);
    assert!(
        candidates.contains(&available),
        "a Basic Land remains available"
    );
    let completion = engine
        .apply_scenario_command(0, &submit_resolution_choice(Vec::new()))
        .expect("legally fail to find despite an available Basic Land");

    assert_eq!(engine.state.objects[&available].zone, Zone::Library);
    assert_eq!(qualifying_permanent_count(&engine), 6);
    assert!(
        battlefield_token_oids(&engine, 0, "treefolk_g_3_4_reach").is_empty(),
        "one Land Treefolk matching both branches contributes one permanent"
    );
    assert_no_automatic_branch_resolution_line(&completion);
}
