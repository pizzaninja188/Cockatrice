use super::helpers::*;
use tricerules_cards::Keyword;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::permanent_moved;

fn engine(seed: u64) -> GameEngine {
    let mut engine = GameEngine::new(
        seed,
        &[0, 1],
        20,
        Some(vec![forest_only_deck(), forest_only_deck()]),
        true,
    )
    .expect("new engine");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

#[track_caller]
fn hand_action_reduction(engine: &mut GameEngine, card_id: &str) -> u32 {
    let hand_index = hand_index_for_card(engine, 0, card_id) as u32;
    engine.initial_response_batch().legal_by_player[&0]
        .hand_actions
        .iter()
        .find(|action| action.hand_index == hand_index)
        .unwrap_or_else(|| panic!("missing cast action for {card_id}"))
        .generic_cost_reduction
}

#[test]
fn issue_218_affinity_counts_only_controlled_forests() {
    let mut engine = engine(218_101);
    inject_card_into_hand(&mut engine, 0, "sapling_nursery");
    inject_permanent_on_battlefield(&mut engine, 0, "forest");
    inject_permanent_on_battlefield(&mut engine, 0, "forest");
    inject_permanent_on_battlefield(&mut engine, 0, "plains");
    inject_permanent_on_battlefield(&mut engine, 1, "forest");

    assert_eq!(hand_action_reduction(&mut engine, "sapling_nursery"), 2);
}

#[test]
fn issue_218_landfall_creates_the_reach_treefolk_token() {
    let mut engine = engine(218_102);
    inject_permanent_on_battlefield(&mut engine, 0, "sapling_nursery");
    inject_card_into_hand(&mut engine, 0, "forest");

    let forest = hand_index_for_card(&engine, 0, "forest");
    engine
        .apply_command(0, &play_land(forest))
        .expect("play a Forest");
    pass_both_players(&mut engine);

    let treefolk_tokens = battlefield_token_oids(&engine, 0, "treefolk_g_3_4_reach");
    let [treefolk] = treefolk_tokens.as_slice() else {
        panic!("one Treefolk token");
    };
    assert_eq!(engine.effective_power(*treefolk), Some(3));
    assert_eq!(engine.effective_toughness(*treefolk), Some(4));
    assert!(engine.effective_has_keyword(*treefolk, Keyword::Reach));
}

#[test]
fn issue_218_activation_exiles_the_source_and_snapshots_only_its_cohort() {
    let mut engine = engine(218_103);
    let nursery = inject_permanent_on_battlefield(&mut engine, 0, "sapling_nursery");
    let generation = engine
        .state
        .zone_change_generation
        .get(&nursery)
        .copied()
        .unwrap_or(0);
    let treefolk = inject_creature_on_battlefield(&mut engine, 0, "treefolk_g_3_4_reach");
    let forest = inject_permanent_on_battlefield(&mut engine, 0, "forest");
    let bear = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let opposing_treefolk = inject_creature_on_battlefield(&mut engine, 1, "treefolk_g_3_4_reach");
    let opposing_forest = inject_permanent_on_battlefield(&mut engine, 1, "forest");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            g: 1,
            c: 1,
            ..Default::default()
        },
    );

    let batch =
        apply_ability(&mut engine, 0, nursery, 0, vec![]).expect("activate Sapling Nursery");

    assert_eq!(engine.state.objects[&nursery].zone, Zone::Exile);
    assert!(engine.state.players[0].exile.contains(&nursery));
    assert_eq!(
        engine.state.zone_change_generation[&nursery],
        generation + 1
    );
    assert!(batch.events.iter().any(|event| {
        matches!(
            &event.ev,
            Some(Ev::PermanentMoved(moved))
                if moved.object_id == nursery
                    && moved.destination == permanent_moved::Destination::Exile as i32
        )
    }));
    assert!(batch.events.iter().any(|event| {
        matches!(
            &event.ev,
            Some(Ev::StackPushed(pushed)) if pushed.description == "Sapling Nursery"
        )
    }));

    pass_both_players(&mut engine);

    assert!(engine.effective_has_keyword(treefolk, Keyword::Indestructible));
    assert!(engine.effective_has_keyword(forest, Keyword::Indestructible));
    assert!(!engine.effective_has_keyword(bear, Keyword::Indestructible));
    assert!(!engine.effective_has_keyword(opposing_treefolk, Keyword::Indestructible));
    assert!(!engine.effective_has_keyword(opposing_forest, Keyword::Indestructible));

    let later_forest = inject_permanent_on_battlefield(&mut engine, 0, "forest");
    assert!(
        !engine.effective_has_keyword(later_forest, Keyword::Indestructible),
        "the one-shot effect must not become a dynamic continuous filter"
    );
}
