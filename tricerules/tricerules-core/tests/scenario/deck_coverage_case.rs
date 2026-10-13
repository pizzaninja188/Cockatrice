//! The Case of the Locked Hothouse coverage and engine capability scenarios.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_cards::registry;
use tricerules_core::{state::CopiableValues, TurnStep, Zone};
use tricerules_proto::ruled::v1 as rv1;

const CASE: &str = "case_of_the_locked_hothouse";

fn last_zone_view(
    batch: &tricerules_proto::ruled::v1::RuledEventBatch,
) -> &tricerules_proto::ruled::v1::ZoneViewSync {
    batch
        .events
        .iter()
        .filter_map(|event| match event.ev.as_ref() {
            Some(Ev::ZoneView(view)) => Some(view),
            _ => None,
        })
        .next_back()
        .expect("zone view")
}

fn begin_case_end_step_with_lands(land_count: usize) -> (GameEngine, u32) {
    let deck = deck_with("forest", &[]);
    let mut engine = GameEngine::new(
        registry::global(),
        4,
        &[0, 1],
        20,
        Some(vec![deck.clone(), deck]),
        true,
    )
    .expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    inject_card_into_hand(&mut engine, 0, CASE);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            g: 1,
            c: 3,
            ..Default::default()
        },
    );
    let case_slot = hand_index_for_card(&engine, 0, CASE);
    engine
        .apply_scenario_command(0, &cast_spell(case_slot, vec![]))
        .expect("cast Case");
    resolve_entire_stack_two_player(&mut engine);
    let case = engine.state.players[0]
        .battlefield
        .iter()
        .copied()
        .find(|object_id| engine.state.objects[object_id].card_id == CASE)
        .expect("Case entered the battlefield");
    for _ in 0..land_count {
        inject_permanent_on_battlefield(&mut engine, 0, "forest");
    }
    engine.state.active_player_idx = 0;
    engine.state.priority_idx = 0;
    engine.state.turn_step = TurnStep::Main2;
    engine.state.passes_since_stack_change = 0;
    pass_both_players(&mut engine);
    (engine, case)
}

fn solved_case_with_library_top(card_id: &str) -> (GameEngine, u32, u32) {
    let (mut engine, case) = begin_case_end_step_with_lands(7);
    pass_both_players(&mut engine);
    engine.state.active_player_idx = 0;
    engine.state.priority_idx = 0;
    engine.state.turn_step = TurnStep::Main1;
    engine.state.passes_since_stack_change = 0;
    let top = inject_library_card(&mut engine, 0, card_id);
    engine.state.players[0].library.push_front(top);
    (engine, case, top)
}

fn begin_library_spell_cast(top: u32, generation: u64, face_index: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::BeginSpellCast(rv1::BeginSpellCast {
            announcement: Some(rv1::SpellCastAnnouncement {
                source: Some(rv1::CastSource {
                    location: Some(rv1::cast_source::Location::LibraryObjectId(top)),
                    expected_zone_change_generation: Some(generation),
                }),
                face_index,
                cast_method: rv1::CastMethod::Normal as i32,
                ..Default::default()
            }),
        })),
    }
}

#[test]
fn case_of_locked_hothouse_is_registered_and_admitted_in_a_ruled_deck() {
    let deck = deck_with("forest", &[CASE]);
    GameEngine::new(
        registry::global(),
        1,
        &[0, 1],
        20,
        Some(vec![deck.clone(), deck]),
        true,
    )
    .expect("the exact campaign card must be implemented before a ruled deck can start");
}

#[test]
fn case_of_locked_hothouse_grants_its_additional_land_play() {
    let deck = deck_with("forest", &[]);
    let mut engine = GameEngine::new(
        registry::global(),
        2,
        &[0, 1],
        20,
        Some(vec![deck.clone(), deck]),
        true,
    )
    .expect("new game");
    advance_to_main1_from_game_start(&mut engine);

    inject_card_into_hand(&mut engine, 0, CASE);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            g: 1,
            c: 3,
            ..Default::default()
        },
    );
    let case_slot = hand_index_for_card(&engine, 0, CASE);
    engine
        .apply_scenario_command(0, &cast_spell(case_slot, vec![]))
        .expect("cast Case of the Locked Hothouse");
    resolve_entire_stack_two_player(&mut engine);

    inject_card_into_hand(&mut engine, 0, "forest");
    engine.state.lands_played_this_turn = 1;

    let land_slot = hand_index_for_card(&engine, 0, "forest");
    engine
        .apply_scenario_command(0, &play_land(land_slot))
        .expect("Case's extra land play is available alongside the ordinary play");
    assert_eq!(engine.state.lands_played_this_turn, 2);
}

#[test]
fn case_extra_land_play_stacks_with_another_extra_land_effect() {
    let (mut engine, _, _) = solved_case_with_library_top("forest");
    inject_card_into_hand(&mut engine, 0, "exploration");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            g: 2,
            ..Default::default()
        },
    );
    let exploration_slot = hand_index_for_card(&engine, 0, "exploration");
    engine
        .apply_scenario_command(0, &cast_spell(exploration_slot, vec![]))
        .expect("cast Exploration");
    resolve_entire_stack_two_player(&mut engine);
    engine.state.lands_played_this_turn = 2;
    inject_card_into_hand(&mut engine, 0, "forest");

    let land_slot = hand_index_for_card(&engine, 0, "forest");
    engine
        .apply_scenario_command(0, &play_land(land_slot))
        .expect("Case and Exploration each grant an extra land play");
    assert_eq!(engine.state.lands_played_this_turn, 3);
}

#[test]
fn case_with_seven_lands_triggers_to_solve_at_its_controllers_end_step() {
    let (engine, case) = begin_case_end_step_with_lands(7);

    assert!(
        engine
            .state
            .stack
            .iter()
            .any(|item| item.source_permanent_id == Some(case)),
        "the end-step solve ability should be on the stack at seven lands"
    );
}

#[test]
fn case_does_not_trigger_to_solve_below_seven_lands() {
    let (engine, case) = begin_case_end_step_with_lands(6);
    assert!(engine
        .state
        .stack
        .iter()
        .all(|item| item.source_permanent_id != Some(case)));
}

#[test]
fn case_becomes_solved_only_when_its_end_step_trigger_resolves() {
    let (mut engine, case) = begin_case_end_step_with_lands(7);
    assert!(!engine.state.is_case_solved(case));

    pass_both_players(&mut engine);

    assert!(engine.state.is_case_solved(case));
}

#[test]
fn solved_case_is_a_public_generation_bound_battlefield_annotation() {
    let (mut engine, case) = begin_case_end_step_with_lands(7);
    pass_both_players(&mut engine);
    assert!(engine.state.is_case_solved(case));

    let solved = engine.initial_response_batch();
    let zone_view = last_zone_view(&solved);
    for player_id in [0, 1] {
        let visible = zone_view
            .per_player
            .iter()
            .flat_map(|player| player.battlefield_objects.iter())
            .find(|object| object.object_id == case)
            .expect("each player's public battlefield snapshot contains Case");
        assert!(
            visible
                .rules_annotation_labels
                .iter()
                .any(|label| label == "Solved"),
            "the Case's solved designation is public to P{player_id}"
        );
    }

    let old_generation = engine
        .state
        .zone_change_generation
        .get(&case)
        .copied()
        .unwrap_or(0);
    engine.state.players[0]
        .battlefield
        .retain(|object_id| *object_id != case);
    engine.state.players[0].graveyard.push(case);
    engine.state.objects.get_mut(&case).unwrap().zone = Zone::Graveyard;
    engine
        .state
        .zone_change_generation
        .insert(case, old_generation + 1);
    engine.state.players[0]
        .graveyard
        .retain(|object_id| *object_id != case);
    engine.state.players[0].battlefield.push(case);
    engine.state.objects.get_mut(&case).unwrap().zone = Zone::Battlefield;
    engine
        .state
        .zone_change_generation
        .insert(case, old_generation + 2);

    assert!(!engine.state.is_case_solved(case));
    let returned = engine.initial_response_batch();
    let visible = last_zone_view(&returned)
        .per_player
        .iter()
        .flat_map(|player| player.battlefield_objects.iter())
        .find(|object| object.object_id == case)
        .expect("returned Case remains on the battlefield");
    assert!(!visible
        .rules_annotation_labels
        .iter()
        .any(|label| label == "Solved"));
}

#[test]
fn case_trigger_rechecks_the_land_threshold_on_resolution() {
    let (mut engine, case) = begin_case_end_step_with_lands(7);
    let land = engine.state.players[0]
        .battlefield
        .iter()
        .copied()
        .find(|oid| engine.state.objects[oid].card_id == "forest")
        .expect("one of the seven lands");
    engine.state.players[0]
        .battlefield
        .retain(|object_id| *object_id != land);
    engine.state.players[0].graveyard.push(land);
    engine.state.objects.get_mut(&land).unwrap().zone = Zone::Graveyard;
    let old_generation = engine
        .state
        .zone_change_generation
        .get(&land)
        .copied()
        .unwrap_or(0);
    engine
        .state
        .zone_change_generation
        .insert(land, old_generation + 1);

    pass_both_players(&mut engine);

    assert!(!engine.state.is_case_solved(case));
}

#[test]
fn case_solve_trigger_keeps_its_controller_after_the_case_changes_control() {
    let (mut engine, case) = begin_case_end_step_with_lands(7);
    let trigger = engine
        .state
        .stack
        .iter()
        .find(|item| item.source_permanent_id == Some(case))
        .expect("Case solve trigger is on the stack");
    assert_eq!(trigger.controller, 0);

    let land = engine.state.players[0]
        .battlefield
        .iter()
        .copied()
        .find(|oid| engine.state.objects[oid].card_id == "forest")
        .expect("one of player zero's seven lands");
    engine.state.players[0]
        .battlefield
        .retain(|object_id| *object_id != land);
    engine.state.players[0].graveyard.push(land);
    engine.state.objects.get_mut(&land).unwrap().zone = Zone::Graveyard;
    let land_generation = engine
        .state
        .zone_change_generation
        .get(&land)
        .copied()
        .unwrap_or(0);
    engine
        .state
        .zone_change_generation
        .insert(land, land_generation + 1);
    for _ in 0..7 {
        inject_permanent_on_battlefield(&mut engine, 1, "forest");
    }

    engine.state.players[0]
        .battlefield
        .retain(|object_id| *object_id != case);
    engine.state.players[1].battlefield.push(case);
    let case_object = engine.state.objects.get_mut(&case).unwrap();
    case_object.controller = 1;
    case_object.base_controller = 1;

    pass_both_players(&mut engine);

    assert!(!engine.state.is_case_solved(case));
}

#[test]
fn solved_case_designation_survives_copy_layer_changes_but_is_not_copied() {
    let (mut engine, case, _) = solved_case_with_library_top("grizzly_bears");
    assert!(engine.state.is_case_solved(case));
    let generation = engine.state.zone_change_generation[&case];

    let case_definition = registry::global().get(CASE).expect("Case definition");
    let copied_values = CopiableValues {
        source_card_id: CASE.into(),
        source_face_index: 0,
        display_name: case_definition.name.clone(),
        face: case_definition.primary_face().clone(),
        prototyped: false,
        room_faces: None,
    };
    let new_copy = inject_permanent_on_battlefield(&mut engine, 1, "forest");
    engine
        .state
        .objects
        .get_mut(&new_copy)
        .unwrap()
        .copiable_values = Some(copied_values);
    assert!(
        !engine.state.is_case_solved(new_copy),
        "a new copy does not copy the Case designation"
    );

    inject_permanent_on_battlefield(&mut engine, 0, "grizzly_bears");
    let bear_definition = registry::global()
        .get("grizzly_bears")
        .expect("Grizzly Bears definition");
    let changed_values = CopiableValues {
        source_card_id: "grizzly_bears".into(),
        source_face_index: 0,
        display_name: bear_definition.name.clone(),
        face: bear_definition.primary_face().clone(),
        prototyped: false,
        room_faces: None,
    };
    engine.state.objects.get_mut(&case).unwrap().copiable_values = Some(changed_values);
    let changed_characteristics = engine
        .characteristics(case)
        .expect("changed Case characteristics");
    assert!(changed_characteristics.is_creature());
    assert!(!changed_characteristics.has_type("Enchantment"));
    engine.state.objects.get_mut(&case).unwrap().face_down = true;
    assert!(engine.state.is_case_solved(case));
    engine.state.objects.get_mut(&case).unwrap().face_down = false;
    assert!(engine.state.is_case_solved(case));
    assert_eq!(engine.state.zone_change_generation[&case], generation);
}

#[test]
fn case_trigger_does_not_solve_a_new_incarnation_after_leave_and_return() {
    let (mut engine, case) = begin_case_end_step_with_lands(7);
    let old_generation = engine
        .state
        .zone_change_generation
        .get(&case)
        .copied()
        .unwrap_or(0);
    engine.state.players[0]
        .battlefield
        .retain(|object_id| *object_id != case);
    engine.state.players[0].graveyard.push(case);
    engine.state.objects.get_mut(&case).unwrap().zone = Zone::Graveyard;
    engine
        .state
        .zone_change_generation
        .insert(case, old_generation + 1);
    engine.state.objects.get_mut(&case).unwrap().zone = Zone::Battlefield;
    engine.state.players[0]
        .graveyard
        .retain(|object_id| *object_id != case);
    engine.state.players[0].battlefield.push(case);
    engine
        .state
        .zone_change_generation
        .insert(case, old_generation + 2);

    pass_both_players(&mut engine);

    assert!(!engine.state.is_case_solved(case));
}

#[test]
fn solved_case_offers_the_exact_library_top_creature_for_casting() {
    let (mut engine, case) = begin_case_end_step_with_lands(7);
    pass_both_players(&mut engine);
    assert!(engine.state.is_case_solved(case));

    let top = inject_library_card(&mut engine, 0, "grizzly_bears");
    engine.state.players[0].library.push_front(top);
    engine.state.active_player_idx = 0;
    engine.state.priority_idx = 0;
    engine.state.turn_step = TurnStep::Main1;
    engine.state.passes_since_stack_change = 0;

    let batch = engine.initial_response_batch();
    let legal = batch.legal_by_player[&0].clone();
    let zone_view = last_zone_view(&batch);
    let owner_view = zone_view
        .per_player
        .iter()
        .find(|view| view.player_id == 0)
        .expect("owner's private view");
    let opponent_view = zone_view
        .per_player
        .iter()
        .find(|view| view.player_id == 1)
        .expect("opponent's view");
    assert_eq!(
        owner_view
            .private_library_top
            .as_ref()
            .map(|top| top.object_id),
        Some(top),
        "the permission holder should receive the exact top identity"
    );
    assert!(
        opponent_view.private_library_top.is_none(),
        "an opponent must not receive the library-top identity"
    );
    assert!(
        legal
            .zone_cast_actions
            .iter()
            .any(|action| action.object_id == top && action.source_zone == 2),
        "a solved Case should offer the exact current library top as a cast source"
    );
}

#[test]
fn solved_case_offers_an_enchantment_spell_from_the_library_top() {
    let (mut engine, _, top) = solved_case_with_library_top(CASE);
    let batch = engine.initial_response_batch();
    assert!(batch.legal_by_player[&0]
        .zone_cast_actions
        .iter()
        .any(|action| action.object_id == top && action.source_zone == 2));
}

#[test]
fn solved_case_with_an_empty_library_has_no_private_top_or_library_actions() {
    let (mut engine, _, _) = solved_case_with_library_top("grizzly_bears");
    engine.state.players[0].library.clear();

    let batch = engine.initial_response_batch();
    let owner = last_zone_view(&batch)
        .per_player
        .iter()
        .find(|view| view.player_id == 0)
        .expect("owner's private view");
    assert!(owner.private_library_top.is_none());
    assert!(batch.legal_by_player[&0]
        .zone_cast_actions
        .iter()
        .all(|action| action.source_zone != 2));
    assert!(batch.legal_by_player[&0]
        .zone_land_actions
        .iter()
        .all(|action| action.source_zone != 2));
}

#[test]
fn solved_case_permission_follows_its_controller_to_that_players_own_library() {
    let (mut engine, case, owner_top) = solved_case_with_library_top("grizzly_bears");
    let case_object = engine.state.objects.get_mut(&case).unwrap();
    case_object.base_controller = 1;
    case_object.controller = 1;
    engine.state.players[0]
        .battlefield
        .retain(|object_id| *object_id != case);
    engine.state.players[1].battlefield.push(case);
    let controller_top = inject_library_card(&mut engine, 1, "grizzly_bears");
    engine.state.players[1].library.push_front(controller_top);
    engine.state.active_player_idx = 1;
    engine.state.priority_idx = 1;
    engine.state.turn_step = TurnStep::Main1;
    engine.state.passes_since_stack_change = 0;

    let batch = engine.initial_response_batch();
    let view = last_zone_view(&batch);
    let owner_view = view
        .per_player
        .iter()
        .find(|view| view.player_id == 0)
        .expect("owner's view");
    let controller_view = view
        .per_player
        .iter()
        .find(|view| view.player_id == 1)
        .expect("controller's view");
    assert!(owner_view.private_library_top.is_none());
    assert_eq!(
        controller_view
            .private_library_top
            .as_ref()
            .map(|top| top.object_id),
        Some(controller_top)
    );
    assert!(batch.legal_by_player[&0]
        .zone_cast_actions
        .iter()
        .all(|action| action.object_id != owner_top || action.source_zone != 2));
    assert!(batch.legal_by_player[&1]
        .zone_cast_actions
        .iter()
        .any(|action| action.object_id == controller_top && action.source_zone == 2));
    let public_case = last_zone_view(&batch)
        .per_player
        .iter()
        .flat_map(|view| view.battlefield_objects.iter())
        .find(|object| object.object_id == case)
        .expect("controlled Case remains in the public battlefield view");
    assert!(public_case
        .rules_annotation_labels
        .iter()
        .any(|label| label == "Solved"));
}

#[test]
fn solved_case_offers_and_counts_a_land_play_from_the_library_top() {
    let (mut engine, _) = begin_case_end_step_with_lands(7);
    pass_both_players(&mut engine);
    engine.state.active_player_idx = 0;
    engine.state.priority_idx = 0;
    engine.state.turn_step = TurnStep::Main1;
    engine.state.passes_since_stack_change = 0;
    let top = inject_library_card(&mut engine, 0, "forest");
    engine.state.players[0].library.push_front(top);

    let legal = engine.initial_response_batch().legal_by_player[&0].clone();
    let action = legal
        .zone_land_actions
        .iter()
        .find(|action| action.object_id == top && action.source_zone == 2 && action.face_index == 0)
        .expect("Case permits playing the exact top land");
    let generation = action.zone_change_generation;
    engine
        .apply_scenario_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::PlayLand(PlayLand {
                    resolution_prompt_id: 0,
                    source: Some(LandSource {
                        location: Some(rv1::land_source::Location::LibraryObjectId(top)),
                        expected_zone_change_generation: Some(generation),
                    }),
                    face_index: action.face_index,
                })),
            },
        )
        .expect("play the library top as a land");

    assert_eq!(engine.state.lands_played_this_turn, 1);
    assert_eq!(engine.state.objects[&top].zone, Zone::Battlefield);
}

#[test]
fn solved_case_filters_each_adventure_face_independently() {
    let (mut engine, _, top) = solved_case_with_library_top("bonecrusher_giant_stomp");
    let actions = engine.initial_response_batch().legal_by_player[&0].clone();
    let cast_faces = actions
        .zone_cast_actions
        .iter()
        .filter(|action| action.object_id == top && action.source_zone == 2)
        .map(|action| action.face_index)
        .collect::<Vec<_>>();
    assert_eq!(
        cast_faces,
        vec![0],
        "the creature face qualifies, but Stomp is an instant"
    );
}

#[test]
fn solved_case_can_play_the_land_back_face_of_a_modal_dfc() {
    let (mut engine, _, top) = solved_case_with_library_top("riverglide_pathway_lavaglide_pathway");
    let actions = engine.initial_response_batch().legal_by_player[&0].clone();
    assert!(actions
        .zone_cast_actions
        .iter()
        .all(|action| !(action.object_id == top && action.source_zone == 2)));
    let land_faces = actions
        .zone_land_actions
        .iter()
        .filter(|action| action.object_id == top && action.source_zone == 2)
        .map(|action| action.face_index)
        .collect::<Vec<_>>();
    assert_eq!(land_faces, vec![0, 1]);
    let land = actions
        .zone_land_actions
        .iter()
        .find(|action| action.object_id == top && action.source_zone == 2 && action.face_index == 1)
        .expect("the land back face is playable from the library top");
    assert_eq!(
        land.face_index, 1,
        "the modal DFC's back face remains a legal land choice"
    );
    engine
        .apply_scenario_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::PlayLand(rv1::PlayLand {
                    resolution_prompt_id: 0,
                    source: Some(rv1::LandSource {
                        location: Some(rv1::land_source::Location::LibraryObjectId(top)),
                        expected_zone_change_generation: Some(land.zone_change_generation),
                    }),
                    face_index: land.face_index,
                })),
            },
        )
        .expect("choose the land face");
    assert_eq!(engine.state.lands_played_this_turn, 1);
    assert_eq!(engine.state.objects[&top].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&top].face_up_index, 1);
}

#[test]
fn stale_non_top_library_identity_cannot_be_cast() {
    let (mut engine, _, old_top) = solved_case_with_library_top("grizzly_bears");
    let generation = engine
        .state
        .zone_change_generation
        .get(&old_top)
        .copied()
        .unwrap_or(0);
    let new_top = inject_library_card(&mut engine, 0, "forest");
    engine.state.players[0].library.push_front(new_top);

    assert!(engine
        .apply_scenario_command(0, &begin_library_spell_cast(old_top, generation, 0))
        .is_err());
    assert_eq!(engine.state.players[0].library.front(), Some(&new_top));
    assert!(engine.state.pending_spell_cast.is_none());
}

#[test]
fn library_top_stays_private_during_begin_and_cancel_restores_the_exact_top() {
    let (mut engine, _, top) = solved_case_with_library_top("grizzly_bears");
    engine.state.players[0].mana_pool.green = 1;
    engine.state.players[0].mana_pool.colorless = 1;
    let original_generation_entry = engine.state.zone_change_generation.get(&top).copied();
    let generation = original_generation_entry.unwrap_or(0);

    engine
        .apply_scenario_command(0, &begin_library_spell_cast(top, generation, 0))
        .expect("begin casting from the current top");
    let transaction_id = engine
        .state
        .pending_spell_cast
        .as_ref()
        .expect("the cast remains in payment")
        .transaction_id;
    let pending = engine.initial_response_batch();
    let owner = last_zone_view(&pending)
        .per_player
        .iter()
        .find(|view| view.player_id == 0)
        .expect("owner view during payment");
    assert!(
        owner.private_library_top.is_none(),
        "CR 401.5 hides the newly exposed top"
    );
    assert!(pending.legal_by_player[&0]
        .zone_cast_actions
        .iter()
        .all(|action| action.source_zone != 2));

    let before_rejected_payment = serde_json::to_value(&engine.state).unwrap();
    assert!(engine
        .apply_scenario_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::CommitSpellCast(rv1::CommitSpellCast {
                    transaction_id,
                    payment: Some(rv1::PaymentSelection::default()),
                    ..Default::default()
                })),
            },
        )
        .is_err());
    assert_eq!(
        serde_json::to_value(&engine.state).unwrap(),
        before_rejected_payment,
        "rejected payment leaves the reserved library card and transaction unchanged"
    );

    engine
        .apply_scenario_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::CancelSpellCast(rv1::CancelSpellCast {
                    transaction_id,
                })),
            },
        )
        .expect("cancel casting");
    assert_eq!(engine.state.players[0].library.front(), Some(&top));
    assert_eq!(
        engine.state.zone_change_generation.get(&top).copied(),
        original_generation_entry
    );
    let restored = engine.initial_response_batch();
    let owner = last_zone_view(&restored)
        .per_player
        .iter()
        .find(|view| view.player_id == 0)
        .expect("owner view after cancellation");
    assert_eq!(
        owner
            .private_library_top
            .as_ref()
            .map(|view| view.object_id),
        Some(top)
    );
    assert!(restored.legal_by_player[&0]
        .zone_cast_actions
        .iter()
        .any(|action| action.object_id == top && action.source_zone == 2));
}

#[test]
fn solved_case_keeps_an_unchanged_library_top_visible_during_a_hand_cast() {
    let (mut engine, _, top) = solved_case_with_library_top("grizzly_bears");
    inject_card_into_hand(&mut engine, 0, "grizzly_bears");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            g: 1,
            c: 1,
            ..Default::default()
        },
    );
    let hand_slot = hand_index_for_card(&engine, 0, "grizzly_bears");
    let Some(Cmd::CastSpell(cast)) = cast_spell(hand_slot, vec![]).cmd else {
        unreachable!("hand cast announcement")
    };
    let announcement = rv1::SpellCastAnnouncement {
        targets: cast.targets,
        x_value: cast.x_value,
        flex_payments: cast.flex_payments,
        face_index: cast.face_index,
        selected_modes: cast.selected_modes,
        source: cast.source,
        prototyped: cast.prototyped,
        cost_selections: cast.cost_selections,
        cast_cost_group_selections: cast.cast_cost_group_selections,
        cast_method: cast.cast_method,
        casting_permission_id: cast.casting_permission_id,
    };

    engine
        .apply_scenario_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::BeginSpellCast(rv1::BeginSpellCast {
                    announcement: Some(announcement),
                })),
            },
        )
        .expect("begin paying for a spell from hand");

    let batch = engine.initial_response_batch();
    let view = last_zone_view(&batch);
    let owner = view
        .per_player
        .iter()
        .find(|view| view.player_id == 0)
        .expect("owner view");
    assert_eq!(
        owner
            .private_library_top
            .as_ref()
            .map(|card| card.object_id),
        Some(top),
        "CR 401.5 hides a newly exposed top; this hand cast did not change the library top"
    );
}

#[test]
fn library_cast_commit_does_not_recheck_a_permission_source_removed_after_begin() {
    let (mut engine, case, top) = solved_case_with_library_top("grizzly_bears");
    engine.state.players[0].mana_pool.green = 1;
    engine.state.players[0].mana_pool.colorless = 1;
    let generation = engine
        .state
        .zone_change_generation
        .get(&top)
        .copied()
        .unwrap_or(0);
    engine
        .apply_scenario_command(0, &begin_library_spell_cast(top, generation, 0))
        .expect("begin the proposed cast while Case permission is active");
    let transaction_id = engine
        .state
        .pending_spell_cast
        .as_ref()
        .unwrap()
        .transaction_id;

    engine.state.players[0]
        .battlefield
        .retain(|object_id| *object_id != case);
    engine.state.players[0].graveyard.push(case);
    let case_object = engine.state.objects.get_mut(&case).unwrap();
    case_object.zone = Zone::Graveyard;
    let old_case_generation = engine
        .state
        .zone_change_generation
        .get(&case)
        .copied()
        .unwrap_or(0);
    engine
        .state
        .zone_change_generation
        .insert(case, old_case_generation + 1);
    let payment_state_revision = engine.state.command_index;
    let payment_source_generation = engine.state.zone_change_generation[&top];

    engine
        .apply_scenario_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::CommitSpellCast(rv1::CommitSpellCast {
                    transaction_id,
                    payment: Some(rv1::PaymentSelection {
                        expected_state_revision: payment_state_revision,
                        source: Some(rv1::CostObjectRef {
                            object_id: top,
                            zone_change_generation: payment_source_generation,
                        }),
                        mana: Some(rv1::PaymentMana {
                            g: 1,
                            c: 1,
                            ..Default::default()
                        }),
                        ..Default::default()
                    }),
                    ..Default::default()
                })),
            },
        )
        .expect("the locked cast remains authorized after its permission source leaves");
    assert!(engine.state.pending_spell_cast.is_none());
    assert_eq!(engine.state.objects[&top].zone, Zone::Stack);
    assert!(engine.state.stack.iter().any(|item| item.id == top));
    let after_permission_loss = engine.initial_response_batch();
    let owner = last_zone_view(&after_permission_loss)
        .per_player
        .iter()
        .find(|view| view.player_id == 0)
        .expect("owner view after Case leaves");
    assert!(owner.private_library_top.is_none());
    assert!(after_permission_loss.legal_by_player[&0]
        .zone_cast_actions
        .iter()
        .all(|action| action.source_zone != 2));
    assert!(after_permission_loss.legal_by_player[&0]
        .zone_land_actions
        .iter()
        .all(|action| action.source_zone != 2));
}
