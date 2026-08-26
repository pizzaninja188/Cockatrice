use super::helpers::*;
use tricerules_cards::CounterKind;
use tricerules_core::{GameEngine, Zone};
use tricerules_proto::ruled::v1::{
    ruled_command::Cmd, AbilitySourceZone, ActivateAbility, RuledCommand,
};

fn zone_ability(engine: &GameEngine, source: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ActivateAbility(ActivateAbility {
            source_object_id: source,
            source_zone: AbilitySourceZone::Hand as i32,
            expected_zone_change_generation: engine
                .state
                .zone_change_generation
                .get(&source)
                .copied()
                .unwrap_or(0),
            ability_index: 0,
            ..Default::default()
        })),
    }
}

fn hand_action_reduction(engine: &mut GameEngine, card_id: &str) -> u32 {
    let hand_index = hand_index_for_card(engine, 0, card_id) as u32;
    engine.initial_response_batch().legal_by_player[&0]
        .hand_actions
        .iter()
        .find(|action| action.hand_index == hand_index)
        .expect("published cast action")
        .generic_cost_reduction
}

#[test]
fn issue_163_conditional_static_and_cost_reduction_track_controller_state() {
    let decks = Some(vec![
        deck_with(
            "forest",
            &[
                "cloudsculpt_technician",
                "mistmeadow_council",
                "bonesplitter",
                "kithkin_billyrider",
            ],
        ),
        deck_with("forest", &["kithkin_billyrider"]),
    ]);
    let mut engine = GameEngine::new(163_001, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    ensure_in_hand(&mut engine, 0, "mistmeadow_council");
    ensure_in_hand(&mut engine, 0, "cloudsculpt_technician");
    ensure_in_hand(&mut engine, 0, "bonesplitter");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            c: 3,
            ..Default::default()
        },
    );
    let cloudsculpt = hand_index_for_card(&engine, 0, "cloudsculpt_technician");
    engine
        .apply_command(0, &cast_spell(cloudsculpt, vec![]))
        .expect("cast Technician");
    resolve_entire_stack_two_player(&mut engine);
    let technician = battlefield_object_for_card(&engine, 0, "cloudsculpt_technician");
    assert_eq!(engine.characteristics(technician).unwrap().power, Some(1));
    assert_eq!(hand_action_reduction(&mut engine, "mistmeadow_council"), 0);

    relocate_to_battlefield(&mut engine, 1, "kithkin_billyrider", false);
    assert_eq!(
        hand_action_reduction(&mut engine, "mistmeadow_council"),
        0,
        "opponent's Kithkin does not qualify"
    );
    let bonesplitter = hand_index_for_card(&engine, 0, "bonesplitter");
    engine
        .apply_command(0, &cast_spell(bonesplitter, vec![]))
        .expect("cast artifact");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.characteristics(technician).unwrap().power, Some(2));
    relocate_to_battlefield(&mut engine, 0, "kithkin_billyrider", false);
    assert_eq!(hand_action_reduction(&mut engine, "mistmeadow_council"), 1);
}

#[test]
fn issue_163_galactic_wayfarer_creates_the_registry_lander_token() {
    let decks = Some(vec![
        deck_with("forest", &["galactic_wayfarer"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(163_002, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    ensure_in_hand(&mut engine, 0, "galactic_wayfarer");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            g: 1,
            c: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "galactic_wayfarer");
    engine
        .apply_command(0, &cast_spell(slot, vec![]))
        .expect("cast Wayfarer");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(battlefield_token_oids(&engine, 0, "lander").len(), 1);
}

#[test]
fn issue_163_mongoose_lizard_mountaincycling_is_private_and_generation_bound() {
    let decks = Some(vec![
        deck_with("mountain", &["mongoose_lizard"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(163_003, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    ensure_in_hand(&mut engine, 0, "mongoose_lizard");
    let mongoose = engine.state.players[0].hand[hand_index_for_card(&engine, 0, "mongoose_lizard")];
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );
    let command = zone_ability(&engine, mongoose);
    engine
        .apply_command(0, &command)
        .expect("activate Mountaincycling");
    assert_eq!(engine.state.objects[&mongoose].zone, Zone::Graveyard);
    engine.apply_command(0, &pass()).expect("controller passes");
    let batch = engine
        .apply_command(1, &pass())
        .expect("Mountaincycling resolves");
    let choice = find_resolution_choice(&batch).expect("private library search");
    assert!(!choice.candidate_object_ids.is_empty());
    assert!(choice
        .candidate_object_ids
        .iter()
        .all(|oid| engine.state.objects[oid].card_id == "mountain"));
    let mountain = choice.candidate_object_ids[0];
    engine
        .apply_command(0, &submit_resolution_choice(vec![mountain]))
        .expect("choose Mountain");
    assert_eq!(engine.state.objects[&mountain].zone, Zone::Hand);
    engine
        .apply_command(0, &command)
        .expect_err("stale hand action cannot be replayed");
}

#[test]
fn issue_163_azula_one_or_both_applies_each_modes_own_target() {
    let decks = Some(vec![
        deck_with("swamp", &["azula_always_lies"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(163_004, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    ensure_in_hand(&mut engine, 0, "azula_always_lies");
    let shrink = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let grow = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            b: 1,
            c: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "azula_always_lies");
    engine
        .apply_command(
            0,
            &cast_modal_spell(
                slot,
                vec![(0, target_object(shrink)), (1, target_object(grow))],
            ),
        )
        .expect("choose both modes");
    resolve_entire_stack_two_player(&mut engine);
    let shrunk = engine.characteristics(shrink).unwrap();
    assert_eq!((shrunk.power, shrunk.toughness), (Some(1), Some(1)));
    assert_eq!(
        engine.state.objects[&grow].counter_count(CounterKind::PlusOnePlusOne),
        1
    );
}

#[test]
fn issue_163_otter_penguin_triggers_only_on_the_second_draw() {
    let decks = Some(vec![
        deck_with("island", &["otter-penguin", "divination"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(163_005, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    let otter = relocate_to_battlefield(&mut engine, 0, "otter-penguin", false);
    ensure_in_hand(&mut engine, 0, "divination");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            c: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "divination");
    engine
        .apply_command(0, &cast_spell(slot, vec![]))
        .expect("cast Divination");
    resolve_entire_stack_two_player(&mut engine);
    let characteristics = engine.characteristics(otter).unwrap();
    assert_eq!(
        (characteristics.power, characteristics.toughness),
        (Some(3), Some(3))
    );
    assert!(zone_view_rules_annotation_labels(&mut engine, 0, otter)
        .contains(&"Can't be blocked".to_string()));
}
