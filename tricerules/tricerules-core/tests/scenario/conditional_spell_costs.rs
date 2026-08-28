use super::helpers::*;
use tricerules_core::{GameEngine, Zone};
use tricerules_proto::ruled::v1 as rv1;

#[test]
fn issue_176_no_one_left_behind_publishes_graveyard_reduction_and_pays_it() {
    let mut engine = GameEngine::new(
        176_101,
        &[0, 1],
        20,
        Some(vec![
            deck_with(
                "swamp",
                &["no_one_left_behind", "grizzly_bears", "serra_angel"],
            ),
            deck_with("forest", &[]),
        ]),
        true,
    )
    .unwrap();
    advance_to_main1_from_game_start(&mut engine);
    ensure_in_hand(&mut engine, 0, "no_one_left_behind");
    let small = relocate_to_hand(&mut engine, 0, "grizzly_bears");
    let large = relocate_to_hand(&mut engine, 0, "serra_angel");
    for oid in [small, large] {
        engine.state.players[0].hand.retain(|id| *id != oid);
        engine.state.players[0].graveyard.push(oid);
        engine.state.objects.get_mut(&oid).unwrap().zone = Zone::Graveyard;
    }
    give_mana(
        &mut engine,
        0,
        ManaGift {
            b: 1,
            c: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "no_one_left_behind");
    let batch = engine.initial_response_batch();
    let targets = &batch.legal_by_player[&0].valid_targets_by_hand_slot[&((slot as u32) << 8)];
    assert_eq!(targets.targeted_cost_reduction_applications.len(), 1);
    let qualifying = &targets.targeted_cost_reduction_applications[0].qualifying_targets;
    assert_eq!(qualifying.len(), 1);
    assert_eq!(qualifying[0].object_id, small);
    assert_eq!(qualifying[0].kind, rv1::TargetRefKind::Graveyard as i32);
    let target = |oid| {
        vec![TargetRef {
            kind: rv1::TargetRefKind::Graveyard as i32,
            object_id: oid,
            ..Default::default()
        }]
    };
    assert!(engine
        .apply_command(0, &cast_spell(slot, target(large)))
        .is_err());
    engine
        .apply_command(0, &cast_spell(slot, target(small)))
        .unwrap();
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&small].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&large].zone, Zone::Graveyard);
    assert_eq!(engine.state.players[0].mana_pool.black, 0);
    assert_eq!(engine.state.players[0].mana_pool.colorless, 0);
}

fn winged_words_action_cost(engine: &mut GameEngine) -> (String, u32) {
    let hand_index = hand_index_for_card(engine, 0, "winged_words") as u32;
    let batch = engine.initial_response_batch();
    let action = batch.legal_by_player[&0]
        .hand_actions
        .iter()
        .find(|action| action.hand_index == hand_index)
        .expect("Winged Words cast action");
    (action.cost.clone(), action.generic_cost_reduction)
}

#[test]
fn winged_words_uses_the_flying_reduction_for_preview_and_payment() {
    let decks = Some(vec![
        deck_with("island", &["winged_words", "cloudkin_seer"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(56_001, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    ensure_in_hand(&mut engine, 0, "winged_words");
    relocate_to_battlefield(&mut engine, 0, "cloudkin_seer", false);

    assert_eq!(winged_words_action_cost(&mut engine), ("{2}{U}".into(), 1));

    engine.state.players[0].mana_pool.blue = 1;
    engine.state.players[0].mana_pool.colorless = 1;
    let hand_index = hand_index_for_card(&engine, 0, "winged_words");
    engine
        .apply_command(0, &cast_spell(hand_index, vec![]))
        .expect("reduced Winged Words cast");

    assert_eq!(engine.state.players[0].mana_pool.blue, 0);
    assert_eq!(engine.state.players[0].mana_pool.colorless, 0);
    let hand_size_after_cast = engine.state.players[0].hand.len();
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.players[0].hand.len(), hand_size_after_cast + 2);
}

#[test]
fn winged_words_counts_only_its_controllers_flyers_and_reduces_only_once() {
    let decks = Some(vec![
        deck_with(
            "island",
            &["winged_words", "cloudkin_seer", "cloudkin_seer"],
        ),
        deck_with("forest", &["cloudkin_seer"]),
    ]);
    let mut engine = GameEngine::new(56_002, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    ensure_in_hand(&mut engine, 0, "winged_words");

    assert_eq!(winged_words_action_cost(&mut engine), ("{2}{U}".into(), 0));
    relocate_to_battlefield(&mut engine, 1, "cloudkin_seer", false);
    assert_eq!(winged_words_action_cost(&mut engine), ("{2}{U}".into(), 0));
    relocate_to_battlefield(&mut engine, 0, "cloudkin_seer", false);
    assert_eq!(winged_words_action_cost(&mut engine), ("{2}{U}".into(), 1));
    relocate_to_battlefield(&mut engine, 0, "cloudkin_seer", false);
    assert_eq!(winged_words_action_cost(&mut engine), ("{2}{U}".into(), 1));
}

#[test]
fn winged_words_tracks_gained_and_lost_flying_and_revalidates_payment_atomically() {
    let decks = Some(vec![
        deck_with("island", &["winged_words", "grizzly_bears", "flight"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(56_003, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    ensure_in_hand(&mut engine, 0, "winged_words");
    ensure_in_hand(&mut engine, 0, "flight");
    let bear = relocate_to_battlefield(&mut engine, 0, "grizzly_bears", false);

    assert_eq!(winged_words_action_cost(&mut engine), ("{2}{U}".into(), 0));
    grant_pool(&mut engine, 0);
    let flight_index = hand_index_for_card(&engine, 0, "flight");
    engine
        .apply_command(0, &cast_spell(flight_index, target_object(bear)))
        .expect("cast Flight");
    resolve_entire_stack_two_player(&mut engine);
    engine.state.players[0].mana_pool.clear();
    assert_eq!(winged_words_action_cost(&mut engine), ("{2}{U}".into(), 1));

    let battlefield_pos = engine.state.players[0]
        .battlefield
        .iter()
        .position(|oid| *oid == bear)
        .expect("bear remains on battlefield");
    engine.state.players[0].battlefield.remove(battlefield_pos);
    engine.state.players[0].graveyard.push(bear);
    engine
        .state
        .objects
        .get_mut(&bear)
        .expect("bear object")
        .zone = Zone::Graveyard;
    assert_eq!(winged_words_action_cost(&mut engine), ("{2}{U}".into(), 0));

    engine.state.players[0].mana_pool.blue = 1;
    engine.state.players[0].mana_pool.colorless = 1;
    let hand_before = engine.state.players[0].hand.clone();
    let command_index_before = engine.state.command_index;
    let hand_index = hand_index_for_card(&engine, 0, "winged_words");
    assert!(engine
        .apply_command(0, &cast_spell(hand_index, vec![]))
        .is_err());
    assert_eq!(engine.state.players[0].hand, hand_before);
    assert_eq!(engine.state.command_index, command_index_before);
    assert_eq!(engine.state.players[0].mana_pool.blue, 1);
    assert_eq!(engine.state.players[0].mana_pool.colorless, 1);
}
