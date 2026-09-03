use super::helpers::*;
use tricerules_cards::primitives::{ContinuousEffectKind, ControllerReference, EffectDuration};
use tricerules_core::{AffectedScope, ContinuousEffect, GameEngine, TurnStep, Zone};
use tricerules_proto::ruled::v1::dev_command::Dev;
use tricerules_proto::ruled::v1::{
    self as rv1, DevCommand, DevMoveCard, DevPutCardInZone, DevZone,
};

fn dev(player: i32, command: Dev) -> rv1::RuledCommand {
    rv1::RuledCommand {
        cmd: Some(Cmd::DevCommand(DevCommand {
            target_player_id: player,
            dev: Some(command),
        })),
    }
}

fn issue_engine() -> GameEngine {
    let decks = Some(vec![
        vec!["forest".to_string(); 20],
        vec!["forest".to_string(); 20],
    ]);
    let mut engine = GameEngine::new(21401, &[0, 1], 20, decks, true).expect("new game");
    engine.enable_dev_commands();
    advance_to_main1_from_game_start(&mut engine);
    engine
        .apply_command(
            0,
            &dev(
                0,
                Dev::PutCardInZone(DevPutCardInZone {
                    card_name: "Icetill Explorer".to_string(),
                    zone: DevZone::Battlefield as i32,
                    ready: true,
                }),
            ),
        )
        .expect("put Icetill Explorer onto the battlefield");
    engine
}

fn move_forest_to_graveyard(engine: &mut GameEngine, player: i32) -> u32 {
    engine
        .apply_command(
            player,
            &dev(
                player,
                Dev::MoveCard(DevMoveCard {
                    card_name: "Forest".to_string(),
                    zone: DevZone::Graveyard as i32,
                    ready: false,
                }),
            ),
        )
        .expect("move Forest to graveyard");
    *engine.state.players[player as usize]
        .graveyard
        .last()
        .expect("graveyard Forest")
}

fn icetill_object(engine: &GameEngine) -> u32 {
    battlefield_object_for_card(engine, 0, "icetill_explorer")
}

fn play_from_graveyard(object_id: u32, generation: u64) -> rv1::RuledCommand {
    rv1::RuledCommand {
        cmd: Some(Cmd::PlayLand(rv1::PlayLand {
            source: Some(rv1::LandSource {
                location: Some(rv1::land_source::Location::GraveyardObjectId(object_id)),
                expected_zone_change_generation: Some(generation),
            }),
            face_index: 0,
        })),
    }
}

#[test]
fn icetill_publishes_and_accepts_a_generation_bound_graveyard_land_action() {
    let mut engine = issue_engine();
    let forest = move_forest_to_graveyard(&mut engine, 0);
    assert_eq!(engine.state.objects[&forest].zone, Zone::Graveyard);
    let generation = engine.state.zone_change_generation[&forest];
    let library_before = engine.state.players[0].library.len();

    let legal = &engine.initial_response_batch().legal_by_player[&0].zone_land_actions;
    assert!(legal.iter().any(|action| {
        action.source_zone == rv1::CastSourceZone::Graveyard as i32
            && action.object_id == forest
            && action.face_index == 0
            && action.zone_change_generation == generation
    }));

    engine
        .apply_command(0, &play_from_graveyard(forest, generation))
        .expect("play Forest from graveyard");
    assert_eq!(engine.state.objects[&forest].zone, Zone::Battlefield);
    assert_eq!(engine.state.lands_played_this_turn, 1);
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.players[0].library.len(), library_before - 1);
}

#[test]
fn graveyard_land_permission_revalidates_player_timing_and_source() {
    let mut engine = issue_engine();
    let own_forest = move_forest_to_graveyard(&mut engine, 0);
    let opponent_forest = move_forest_to_graveyard(&mut engine, 1);
    let own_generation = engine.state.zone_change_generation[&own_forest];
    let opponent_generation = engine.state.zone_change_generation[&opponent_forest];

    let legal = &engine.initial_response_batch().legal_by_player[&0].zone_land_actions;
    assert!(legal.iter().any(|action| action.object_id == own_forest));
    assert!(!legal
        .iter()
        .any(|action| action.object_id == opponent_forest));
    assert!(engine
        .apply_command(
            0,
            &play_from_graveyard(opponent_forest, opponent_generation),
        )
        .is_err());

    engine.state.turn_step = TurnStep::Upkeep;
    assert!(engine.initial_response_batch().legal_by_player[&0]
        .zone_land_actions
        .is_empty());
    assert!(engine
        .apply_command(0, &play_from_graveyard(own_forest, own_generation))
        .is_err());
    engine.state.turn_step = TurnStep::Main1;

    engine.state.lands_played_this_turn = 2;
    assert!(engine.initial_response_batch().legal_by_player[&0]
        .zone_land_actions
        .is_empty());
    assert!(engine
        .apply_command(0, &play_from_graveyard(own_forest, own_generation))
        .is_err());
    engine.state.lands_played_this_turn = 0;

    engine
        .apply_command(
            0,
            &dev(
                0,
                Dev::MoveCard(DevMoveCard {
                    card_name: "Icetill Explorer".to_string(),
                    zone: DevZone::Hand as i32,
                    ready: false,
                }),
            ),
        )
        .expect("remove permission source");
    assert!(engine.initial_response_batch().legal_by_player[&0]
        .zone_land_actions
        .is_empty());
    assert!(engine
        .apply_command(0, &play_from_graveyard(own_forest, own_generation))
        .is_err());
}

#[test]
fn stale_graveyard_land_action_is_rejected_without_state_change() {
    let mut engine = issue_engine();
    let forest = move_forest_to_graveyard(&mut engine, 0);
    let old_generation = engine.state.zone_change_generation[&forest];
    let stale_command = play_from_graveyard(forest, old_generation);

    engine.state.players[0]
        .graveyard
        .retain(|oid| *oid != forest);
    engine.state.players[0].hand.push(forest);
    engine.state.objects.get_mut(&forest).expect("Forest").zone = Zone::Hand;
    *engine
        .state
        .zone_change_generation
        .entry(forest)
        .or_default() += 1;
    engine.state.players[0].hand.retain(|oid| *oid != forest);
    engine.state.players[0].graveyard.push(forest);
    engine.state.objects.get_mut(&forest).expect("Forest").zone = Zone::Graveyard;
    *engine
        .state
        .zone_change_generation
        .entry(forest)
        .or_default() += 1;
    assert_eq!(engine.state.objects[&forest].zone, Zone::Graveyard);
    assert!(engine.state.zone_change_generation[&forest] > old_generation);
    let lands_before = engine.state.lands_played_this_turn;

    assert!(engine.apply_command(0, &stale_command).is_err());
    assert_eq!(engine.state.objects[&forest].zone, Zone::Graveyard);
    assert_eq!(engine.state.lands_played_this_turn, lands_before);
}

#[test]
fn ability_blanking_suppresses_and_restores_the_permission() {
    let mut engine = issue_engine();
    let forest = move_forest_to_graveyard(&mut engine, 0);
    let icetill = icetill_object(&engine);
    let timestamp = engine.state.command_index + 1;
    engine.state.continuous_effects.push(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: None,
        affected: AffectedScope::Single(icetill),
        kind: ContinuousEffectKind::Layer6RemoveAllAbilities,
        condition: None,
        duration: EffectDuration::UntilEndOfTurn,
        timestamp,
    });
    assert!(engine.initial_response_batch().legal_by_player[&0]
        .zone_land_actions
        .is_empty());

    engine
        .state
        .continuous_effects
        .retain(|effect| effect.timestamp != timestamp);
    assert!(engine.initial_response_batch().legal_by_player[&0]
        .zone_land_actions
        .iter()
        .any(|action| action.object_id == forest));
}

#[test]
fn control_change_transfers_the_graveyard_permission() {
    let mut engine = issue_engine();
    let p0_forest = move_forest_to_graveyard(&mut engine, 0);
    let p1_forest = move_forest_to_graveyard(&mut engine, 1);
    let icetill = icetill_object(&engine);
    engine.state.continuous_effects.push(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: None,
        affected: AffectedScope::Single(icetill),
        kind: ContinuousEffectKind::Layer2Control {
            controller: ControllerReference::Fixed(1),
        },
        condition: None,
        duration: EffectDuration::UntilEndOfTurn,
        timestamp: engine.state.command_index + 1,
    });
    engine.state.active_player_idx = 1;
    engine.state.priority_idx = 1;

    assert!(!engine.initial_response_batch().legal_by_player[&0]
        .zone_land_actions
        .iter()
        .any(|action| action.object_id == p0_forest));
    assert!(engine.initial_response_batch().legal_by_player[&1]
        .zone_land_actions
        .iter()
        .any(|action| action.object_id == p1_forest));
}

#[test]
fn graveyard_permission_offers_both_pathway_land_faces() {
    let mut engine = issue_engine();
    engine
        .apply_command(
            0,
            &dev(
                0,
                Dev::PutCardInZone(DevPutCardInZone {
                    card_name: "Cragcrown Pathway // Timbercrown Pathway".to_string(),
                    zone: DevZone::Hand as i32,
                    ready: false,
                }),
            ),
        )
        .expect("put Pathway into hand");
    engine
        .apply_command(
            0,
            &dev(
                0,
                Dev::MoveCard(DevMoveCard {
                    card_name: "Cragcrown Pathway // Timbercrown Pathway".to_string(),
                    zone: DevZone::Graveyard as i32,
                    ready: false,
                }),
            ),
        )
        .expect("move Pathway into graveyard");
    let pathway = engine.state.players[0]
        .graveyard
        .iter()
        .copied()
        .find(|oid| engine.state.objects[oid].card_id == "cragcrown_pathway_timbercrown_pathway")
        .expect("graveyard Pathway");
    let actions: Vec<_> = engine.initial_response_batch().legal_by_player[&0]
        .zone_land_actions
        .iter()
        .filter(|action| action.object_id == pathway)
        .map(|action| action.face_index)
        .collect();
    assert_eq!(actions, [0, 1]);
}
