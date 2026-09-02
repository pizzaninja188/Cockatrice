use super::helpers::*;
use tricerules_cards::primitives::{ActivatedCostModifier, PermanentEventFilter};
use tricerules_cards::{AbilityId, CounterKind, GameCondition, Keyword, RelativePlayerSet};
use tricerules_core::state::CopiableValues;
use tricerules_core::GameEngine;

fn published_ability_mana_cost(
    engine: &mut GameEngine,
    player: usize,
    object_id: u32,
    ability_index: usize,
) -> String {
    engine
        .initial_response_batch()
        .events
        .iter()
        .find_map(|event| match &event.ev {
            Some(Ev::ZoneView(view)) => Some(view),
            _ => None,
        })
        .and_then(|view| view.per_player.get(player))
        .into_iter()
        .flat_map(|view| view.battlefield_objects.iter())
        .find(|object| object.object_id == object_id)
        .and_then(|object| object.activated_abilities.get(ability_index))
        .map(|ability| ability.mana_cost.clone())
        .unwrap_or_else(|| panic!("missing published ability {ability_index} for {object_id}"))
}

fn power_up_condition() -> GameCondition {
    GameCondition::PermanentsEnteredThisTurn {
        controllers: RelativePlayerSet::All,
        filter: PermanentEventFilter {
            source_only: true,
            ..Default::default()
        },
        min: Some(1),
        max: None,
    }
}

#[test]
fn power_up_reduces_fixed_symbols_then_overflows_to_generic() {
    let decks = Some(vec![
        deck_with("mountain", &["rough_rhino_cavalry"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(183_001, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    let source = move_ready_to_battlefield(&mut engine, 0, "rough_rhino_cavalry");

    let mut face = tricerules_cards::CardRegistry::global()
        .get("rough_rhino_cavalry")
        .expect("Rough Rhino Cavalry")
        .primary_face()
        .clone();
    face.activated_abilities[0].cost_modifiers =
        vec![ActivatedCostModifier::ConditionalSourceManaCostReduction {
            condition: power_up_condition(),
        }];
    engine
        .state
        .objects
        .get_mut(&source)
        .expect("source")
        .token_origin = Some(CopiableValues {
        source_card_id: "rough_rhino_cavalry".into(),
        source_face_index: 0,
        face,
        room_faces: None,
        display_name: "Rough Rhino Cavalry".into(),
    });

    assert_eq!(
        published_ability_mana_cost(&mut engine, 0, source, 0),
        "{3}",
        "{{4}}{{R}} must reduce {{8}} by five after the permanent enters"
    );
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 3,
            ..Default::default()
        },
    );
    apply_ability(&mut engine, 0, source, 0, vec![])
        .expect("activate the reduced power-up ability");
}

#[test]
fn power_up_tracks_entry_generation_across_turns_and_control_changes() {
    let decks = Some(vec![
        deck_with("wastes", &["ultron_drone", "ultron_drone"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(183_002, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);

    let unrecorded = relocate_to_battlefield(&mut engine, 0, "ultron_drone", false);
    assert_eq!(
        published_ability_mana_cost(&mut engine, 0, unrecorded, 0),
        "{6}"
    );

    let entered = move_ready_to_battlefield(&mut engine, 0, "ultron_drone");
    assert_eq!(
        published_ability_mana_cost(&mut engine, 0, entered, 0),
        "{3}"
    );

    engine.state.players[0]
        .battlefield
        .retain(|object_id| *object_id != entered);
    engine.state.players[1].battlefield.push(entered);
    let object = engine
        .state
        .objects
        .get_mut(&entered)
        .expect("Ultron Drone");
    object.base_controller = 1;
    object.controller = 1;
    engine.state.priority_idx = 1;
    assert_eq!(
        published_ability_mana_cost(&mut engine, 1, entered, 0),
        "{3}",
        "the entry belongs to the exact object even after control changes"
    );

    *engine
        .state
        .zone_change_generation
        .entry(entered)
        .or_insert(0) += 1;
    assert_eq!(
        published_ability_mana_cost(&mut engine, 1, entered, 0),
        "{6}",
        "a new generation cannot reuse the old entry fact"
    );

    let mut next_turn = GameEngine::new(
        183_003,
        &[0, 1],
        20,
        Some(vec![
            deck_with("wastes", &["ultron_drone"]),
            deck_with("forest", &[]),
        ]),
        true,
    )
    .expect("engine");
    advance_to_main1_from_game_start(&mut next_turn);
    let source = move_ready_to_battlefield(&mut next_turn, 0, "ultron_drone");
    assert_eq!(
        published_ability_mana_cost(&mut next_turn, 0, source, 0),
        "{3}"
    );
    end_active_turn(&mut next_turn, 0);
    advance_to_main1_from_game_start(&mut next_turn);
    assert_eq!(
        published_ability_mana_cost(&mut next_turn, 0, source, 0),
        "{6}",
        "the previous turn's entry no longer qualifies"
    );
}

#[test]
fn issue_183_cards_publish_reduced_costs_and_resolve_their_power_ups() {
    let card_ids = [
        "ninja_of_the_hand",
        "ultron_drone",
        "hercules,_prince_of_power",
        "white_tiger,_ava_ayala",
        "viv_vision,_teen_synthezoid",
    ];
    let mut engine = GameEngine::new(
        183_004,
        &[0, 1],
        20,
        Some(vec![
            deck_with("wastes", &card_ids),
            deck_with("forest", &[]),
        ]),
        true,
    )
    .expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    let sources = card_ids.map(|card_id| move_ready_to_battlefield(&mut engine, 0, card_id));
    for (source, expected) in sources.into_iter().zip(["{2}", "{3}", "{2}", "{4}", "{4}"]) {
        assert_eq!(
            published_ability_mana_cost(&mut engine, 0, source, 0),
            expected
        );
    }

    let ninja = sources[0];
    grant_pool(&mut engine, 0);
    apply_ability(&mut engine, 0, ninja, 0, vec![]).expect("activate Ninja of the Hand");
    engine.apply_command(0, &pass()).expect("controller passes");
    let batch = engine.apply_command(1, &pass()).expect("opponent passes");
    let choice = find_resolution_choice(&batch).expect("opponent discard choice");
    let discarded = choice.candidate_object_ids[0];
    engine
        .apply_command(1, &submit_resolution_choice(vec![discarded]))
        .expect("opponent discards");
    assert_eq!(
        engine.state.objects[&ninja].counter_count(CounterKind::PlusOnePlusOne),
        1
    );
    let mana_before = engine.state.players[0].mana_pool;
    let uses_before = engine.state.activation_uses_this_turn.clone();
    apply_ability(&mut engine, 0, ninja, 0, vec![])
        .expect_err("each power-up ability is limited to one activation");
    assert_eq!(engine.state.players[0].mana_pool, mana_before);
    assert_eq!(engine.state.activation_uses_this_turn, uses_before);

    let ultron = sources[1];
    grant_pool(&mut engine, 0);
    apply_ability(&mut engine, 0, ultron, 0, vec![]).expect("activate Ultron Drone");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        engine.state.objects[&ultron].counter_count(CounterKind::PlusOnePlusOne),
        2
    );
    assert_eq!(
        battlefield_token_oids(&engine, 0, "robot_villain_c_2_2").len(),
        1
    );

    let hercules = sources[2];
    grant_pool(&mut engine, 0);
    apply_ability(&mut engine, 0, hercules, 0, vec![]).expect("activate Hercules");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        engine.state.objects[&hercules].counter_count(CounterKind::PlusOnePlusOne),
        1
    );
    for keyword in [Keyword::Vigilance, Keyword::Indestructible, Keyword::Haste] {
        assert!(engine.effective_has_keyword(hercules, keyword));
    }

    let white_tiger = sources[3];
    grant_pool(&mut engine, 0);
    apply_ability(&mut engine, 0, white_tiger, 0, vec![]).expect("activate White Tiger");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        engine.state.objects[&white_tiger].counter_count(CounterKind::PlusOnePlusOne),
        1
    );
    assert_eq!(battlefield_token_oids(&engine, 0, "the_tiger_god").len(), 1);

    let viv = sources[4];
    grant_pool(&mut engine, 0);
    apply_ability(&mut engine, 0, viv, 0, vec![]).expect("activate Viv Vision");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        engine.state.objects[&viv].counter_count(CounterKind::PlusOnePlusOne),
        2
    );
}

#[test]
fn failed_power_up_payment_is_atomic() {
    let mut engine = GameEngine::new(
        183_005,
        &[0, 1],
        20,
        Some(vec![
            deck_with("swamp", &["ninja_of_the_hand"]),
            deck_with("forest", &[]),
        ]),
        true,
    )
    .expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    let ninja = move_ready_to_battlefield(&mut engine, 0, "ninja_of_the_hand");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );
    let mana_before = engine.state.players[0].mana_pool;
    let command_before = engine.state.command_index;
    apply_ability(&mut engine, 0, ninja, 0, vec![])
        .expect_err("the reduced two-mana cost is still unaffordable");
    assert_eq!(engine.state.players[0].mana_pool, mana_before);
    assert_eq!(engine.state.command_index, command_before);
    assert!(engine.state.activation_uses_this_turn.is_empty());
}

#[test]
fn copied_power_up_abilities_keep_independent_once_per_object_limits() {
    let mut engine = GameEngine::new(
        183_006,
        &[0, 1],
        20,
        Some(vec![
            deck_with("mountain", &["rough_rhino_cavalry"]),
            deck_with("forest", &[]),
        ]),
        true,
    )
    .expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    let source = move_ready_to_battlefield(&mut engine, 0, "rough_rhino_cavalry");
    let mut face = tricerules_cards::CardRegistry::global()
        .get("rough_rhino_cavalry")
        .expect("Rough Rhino Cavalry")
        .primary_face()
        .clone();
    face.activated_abilities[0].cost_modifiers =
        vec![ActivatedCostModifier::ConditionalSourceManaCostReduction {
            condition: power_up_condition(),
        }];
    let mut second = face.activated_abilities[0].clone();
    second.ability_id = AbilityId::new("activated_02").unwrap();
    face.activated_abilities.push(second);
    engine.state.objects.get_mut(&source).unwrap().token_origin = Some(CopiableValues {
        source_card_id: "rough_rhino_cavalry".into(),
        source_face_index: 0,
        face,
        room_faces: None,
        display_name: "Rough Rhino Cavalry".into(),
    });

    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 6,
            ..Default::default()
        },
    );
    apply_ability(&mut engine, 0, source, 0, vec![]).expect("first copied power-up");
    resolve_entire_stack_two_player(&mut engine);
    apply_ability(&mut engine, 0, source, 1, vec![]).expect("independent copied power-up");
    resolve_entire_stack_two_player(&mut engine);
    apply_ability(&mut engine, 0, source, 0, vec![])
        .expect_err("the first copied ability has used its own allowance");
    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::PlusOnePlusOne),
        4
    );
}

#[test]
fn viv_vision_draws_on_attack_only_after_reaching_four_power() {
    fn attack_with_viv(power_up: bool, seed: u64) -> (usize, usize) {
        let mut engine = GameEngine::new(
            seed,
            &[0, 1],
            20,
            Some(vec![
                deck_with("wastes", &["viv_vision,_teen_synthezoid"]),
                deck_with("forest", &[]),
            ]),
            true,
        )
        .expect("engine");
        advance_to_main1_from_game_start(&mut engine);
        let viv = move_ready_to_battlefield(&mut engine, 0, "viv_vision,_teen_synthezoid");
        if power_up {
            give_mana(
                &mut engine,
                0,
                ManaGift {
                    c: 4,
                    ..Default::default()
                },
            );
            apply_ability(&mut engine, 0, viv, 0, vec![]).expect("power up Viv");
            resolve_entire_stack_two_player(&mut engine);
        }
        let before = engine.state.players[0].hand.len();
        engine
            .apply_command(0, &primitive_yield())
            .expect("main phase to begin combat");
        engine
            .apply_command(0, &pass())
            .expect("active player passes");
        engine
            .apply_command(1, &pass())
            .expect("nonactive player passes");
        engine
            .apply_command(0, &declare_attackers(vec![viv]))
            .expect("Viv attacks");
        resolve_entire_stack_two_player(&mut engine);
        (before, engine.state.players[0].hand.len())
    }

    let (before, after) = attack_with_viv(true, 183_007);
    assert_eq!(after, before + 1);
    let (before, after) = attack_with_viv(false, 183_008);
    assert_eq!(after, before);
}
