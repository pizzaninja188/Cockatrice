use super::helpers::*;
use tricerules_core::state::ExilePlayPermissionScope;
use tricerules_core::{GameEngine, TurnStep, Zone};
use tricerules_proto::ruled::v1::dev_command::Dev;
use tricerules_proto::ruled::v1::{self as rv1, DevCommand, DevMoveCard, DevZone};

fn grant_from_percussionist(e: &mut GameEngine, exiled_card: &str) -> u32 {
    let source = inject_creature_on_battlefield(e, 0, "clockwork_percussionist");
    let top = inject_library_card(e, 0, exiled_card);
    e.state.players[0].library.retain(|&oid| oid != top);
    e.state.players[0].library.push_front(top);
    e.state.objects.get_mut(&source).expect("source").damage = 2;
    let priority_player = e.state.priority_player_id();
    e.apply_command(priority_player, &pass())
        .expect("run state-based actions");
    resolve_entire_stack_two_player(e);
    top
}

fn play_land_from_exile(object_id: u32, face_index: u32) -> rv1::RuledCommand {
    rv1::RuledCommand {
        cmd: Some(rv1::ruled_command::Cmd::PlayLand(rv1::PlayLand {
            source: Some(rv1::LandSource {
                location: Some(rv1::land_source::Location::ExileObjectId(object_id)),
            }),
            face_index,
        })),
    }
}

fn advance_to_turn_instance(engine: &mut GameEngine, target: u64) {
    let mut commands = 0;
    while engine.state.turn_instance < target {
        resolve_cleanup_discards_if_any(engine);
        let priority = engine.state.priority_player_id();
        engine
            .apply_command(priority, &pass())
            .expect("advance turn");
        commands += 1;
        assert!(commands < 200, "turn advancement stalled");
    }
}

fn put_in_graveyard(engine: &mut GameEngine, card_id: &str) {
    let object_id = inject_card_into_hand(engine, 0, card_id);
    engine.state.players[0].hand.retain(|&oid| oid != object_id);
    engine.state.players[0].graveyard.push(object_id);
    engine.state.objects.get_mut(&object_id).expect("card").zone = Zone::Graveyard;
}

#[test]
fn percussionist_grants_a_generation_bound_group_and_land_actions() {
    let decks = Some(vec![
        vec!["mountain".to_string(); 20],
        vec!["forest".to_string(); 20],
    ]);
    let mut engine = GameEngine::new(12301, &[0, 1], 20, decks, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);

    let exiled = grant_from_percussionist(&mut engine, "cragcrown_pathway_timbercrown_pathway");
    assert_eq!(engine.state.objects[&exiled].zone, Zone::Exile);
    let permission = engine
        .state
        .active_exile_play_permissions
        .iter()
        .find(|permission| permission.object_id == exiled)
        .expect("play permission");
    assert_eq!(permission.scope, ExilePlayPermissionScope::PlayCard);
    assert_eq!(permission.player_id, 0);
    assert_eq!(permission.source_label, "Clockwork Percussionist");
    assert_eq!(
        permission.expires_at_cleanup_turn_instance,
        Some(engine.state.turn_instance + 2),
        "a grant during P0's turn lasts through P0's next turn"
    );

    let legal = &engine.initial_response_batch().legal_by_player[&0];
    assert_eq!(legal.exile_play_permission_groups.len(), 1);
    assert_eq!(legal.exile_play_permission_groups[0].object_ids, [exiled]);
    assert_eq!(legal.zone_land_actions.len(), 2, "both MDFC land faces");
    assert!(engine.initial_response_batch().legal_by_player[&1]
        .exile_play_permission_groups
        .is_empty());

    engine
        .apply_command(0, &play_land_from_exile(exiled, 1))
        .expect("play Timbercrown Pathway from exile");
    assert_eq!(engine.state.objects[&exiled].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&exiled].face_up_index, 1);
    assert!(engine.state.active_exile_play_permissions.is_empty());
}

#[test]
fn leaving_exile_invalidates_the_old_generation_permission() {
    let decks = Some(vec![
        vec!["mountain".to_string(); 20],
        vec!["forest".to_string(); 20],
    ]);
    let mut engine = GameEngine::new(12306, &[0, 1], 20, decks, true).expect("new game");
    engine.enable_dev_commands();
    advance_to_main1_from_game_start(&mut engine);
    let exiled = grant_from_percussionist(&mut engine, "grizzly_bears");
    let generation = engine.state.zone_change_generation[&exiled];

    engine
        .apply_command(
            0,
            &rv1::RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::DevCommand(DevCommand {
                    target_player_id: 0,
                    dev: Some(Dev::MoveCard(DevMoveCard {
                        card_name: "Grizzly Bears".to_string(),
                        zone: DevZone::Hand as i32,
                        ready: false,
                    })),
                })),
            },
        )
        .expect("move the card out of exile");

    assert_eq!(engine.state.objects[&exiled].zone, Zone::Hand);
    assert!(engine.state.zone_change_generation[&exiled] > generation);
    assert!(engine.state.active_exile_play_permissions.is_empty());
    assert!(engine.initial_response_batch().legal_by_player[&0]
        .exile_play_permission_groups
        .is_empty());
}

#[test]
fn permission_group_persists_when_timing_suppresses_actions() {
    let decks = Some(vec![
        vec!["mountain".to_string(); 20],
        vec!["forest".to_string(); 20],
    ]);
    let mut engine = GameEngine::new(12302, &[0, 1], 20, decks, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    let exiled = grant_from_percussionist(&mut engine, "grizzly_bears");

    engine.state.turn_step = TurnStep::Upkeep;
    let legal = &engine.initial_response_batch().legal_by_player[&0];
    assert!(legal.zone_cast_actions.is_empty());
    assert_eq!(legal.exile_play_permission_groups[0].object_ids, [exiled]);
}

#[test]
fn exile_land_actions_obey_timing_count_player_and_face_validation() {
    let decks = Some(vec![
        vec!["mountain".to_string(); 20],
        vec!["forest".to_string(); 20],
    ]);
    let mut engine = GameEngine::new(12307, &[0, 1], 20, decks, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    let exiled = grant_from_percussionist(&mut engine, "cragcrown_pathway_timbercrown_pathway");

    engine.state.turn_step = TurnStep::Upkeep;
    assert!(engine.initial_response_batch().legal_by_player[&0]
        .zone_land_actions
        .is_empty());
    engine.state.turn_step = TurnStep::Main1;
    engine.state.lands_played_this_turn = 1;
    assert!(engine.initial_response_batch().legal_by_player[&0]
        .zone_land_actions
        .is_empty());
    assert!(engine
        .apply_command(1, &play_land_from_exile(exiled, 0))
        .is_err());
    engine.state.lands_played_this_turn = 0;
    assert!(engine
        .apply_command(0, &play_land_from_exile(exiled, 2))
        .is_err());
    assert_eq!(engine.state.objects[&exiled].zone, Zone::Exile);
}

#[test]
fn permission_expires_at_cleanup_of_the_grantees_next_turn() {
    let decks = Some(vec![
        vec!["mountain".to_string(); 20],
        vec!["forest".to_string(); 20],
    ]);
    let mut engine = GameEngine::new(12303, &[0, 1], 20, decks, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    let exiled = grant_from_percussionist(&mut engine, "grizzly_bears");
    let expiry = engine.state.active_exile_play_permissions[0]
        .expires_at_cleanup_turn_instance
        .expect("finite permission");

    advance_to_turn_instance(&mut engine, expiry);
    assert!(engine
        .state
        .active_exile_play_permissions
        .iter()
        .any(|permission| permission.object_id == exiled));

    advance_to_turn_instance(&mut engine, expiry + 1);
    assert!(engine.state.active_exile_play_permissions.is_empty());
    assert_eq!(engine.state.objects[&exiled].zone, Zone::Exile);
}

#[test]
fn permission_groups_granted_on_successive_turns_expire_independently() {
    let decks = Some(vec![
        vec!["mountain".to_string(); 20],
        vec!["forest".to_string(); 20],
    ]);
    let mut engine = GameEngine::new(12308, &[0, 1], 20, decks, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    let first = grant_from_percussionist(&mut engine, "grizzly_bears");
    let first_expiry = engine.state.active_exile_play_permissions[0]
        .expires_at_cleanup_turn_instance
        .expect("first expiry");

    advance_to_turn_instance(&mut engine, first_expiry);
    assert_eq!(engine.state.turn_step, TurnStep::Upkeep);
    pass_both_players(&mut engine);
    pass_both_players(&mut engine);
    assert_eq!(engine.state.turn_step, TurnStep::Main1);
    let second = grant_from_percussionist(&mut engine, "forest");
    let second_expiry = engine
        .state
        .active_exile_play_permissions
        .iter()
        .find(|permission| permission.object_id == second)
        .and_then(|permission| permission.expires_at_cleanup_turn_instance)
        .expect("second expiry");
    assert!(second_expiry > first_expiry);

    advance_to_turn_instance(&mut engine, first_expiry + 1);
    assert!(!engine
        .state
        .active_exile_play_permissions
        .iter()
        .any(|permission| permission.object_id == first));
    assert!(engine
        .state
        .active_exile_play_permissions
        .iter()
        .any(|permission| permission.object_id == second));

    advance_to_turn_instance(&mut engine, second_expiry + 1);
    assert!(engine.state.active_exile_play_permissions.is_empty());
}

#[test]
fn impossible_inferno_checks_delirium_during_resolution_and_grants_permission() {
    let decks = Some(vec![
        vec!["mountain".to_string(); 20],
        vec!["forest".to_string(); 20],
    ]);
    let mut engine = GameEngine::new(12304, &[0, 1], 20, decks, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    put_in_graveyard(&mut engine, "mountain");
    put_in_graveyard(&mut engine, "grizzly_bears");
    put_in_graveyard(&mut engine, "lightning_bolt");
    put_in_graveyard(&mut engine, "divination");
    let target = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let top = inject_library_card(&mut engine, 0, "forest");
    engine.state.players[0].library.retain(|&oid| oid != top);
    engine.state.players[0].library.push_front(top);
    let inferno = inject_card_into_hand(&mut engine, 0, "impossible_inferno");
    let slot = engine.state.players[0]
        .hand
        .iter()
        .position(|&oid| oid == inferno)
        .expect("Inferno slot");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            r: 1,
            c: 4,
            ..Default::default()
        },
    );

    engine
        .apply_command(0, &cast_spell(slot, target_object(target)))
        .expect("cast Impossible Inferno");
    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(engine.state.objects[&target].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&top].zone, Zone::Exile);
    assert!(engine
        .state
        .active_exile_play_permissions
        .iter()
        .any(|permission| permission.object_id == top
            && permission.source_label == "Impossible Inferno"));
}

#[test]
fn impossible_inferno_without_delirium_deals_damage_but_does_not_exile() {
    let decks = Some(vec![
        vec!["mountain".to_string(); 20],
        vec!["forest".to_string(); 20],
    ]);
    let mut engine = GameEngine::new(12305, &[0, 1], 20, decks, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    let target = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let top = *engine.state.players[0]
        .library
        .front()
        .expect("library top");
    let inferno = inject_card_into_hand(&mut engine, 0, "impossible_inferno");
    let slot = engine.state.players[0]
        .hand
        .iter()
        .position(|&oid| oid == inferno)
        .expect("Inferno slot");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            r: 1,
            c: 4,
            ..Default::default()
        },
    );

    engine
        .apply_command(0, &cast_spell(slot, target_object(target)))
        .expect("cast Impossible Inferno");
    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(engine.state.objects[&target].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&top].zone, Zone::Library);
    assert!(engine.state.active_exile_play_permissions.is_empty());
}
