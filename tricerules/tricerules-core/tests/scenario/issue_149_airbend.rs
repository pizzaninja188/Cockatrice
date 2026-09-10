use super::helpers::*;
use tricerules_cards::ManaCost;
use tricerules_core::state::{
    ActiveExilePlayPermission, ExilePermissionCastCost, ExilePlayPermissionOrigin,
    ExilePlayPermissionScope,
};
use tricerules_core::Zone;

#[test]
fn airbending_lesson_token_ceases_to_exist_without_cast_permission() {
    let mut engine = GameEngine::new(
        149_003,
        &[0, 1],
        20,
        Some(vec![vec!["forest".into(); 30]; 2]),
        true,
    )
    .expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    let token = inject_creature_on_battlefield(&mut engine, 1, "soldier_w_1_1");
    assert!(engine.state.objects[&token].is_token());
    inject_card_into_hand(&mut engine, 0, "airbending_lesson");
    let slot = hand_index_for_card(&engine, 0, "airbending_lesson");
    let library_before = engine.state.players[0].library.len();
    give_mana(
        &mut engine,
        0,
        ManaGift {
            w: 1,
            c: 2,
            ..Default::default()
        },
    );
    engine
        .apply_command(0, &cast_spell(slot, target_object(token)))
        .expect("airbend token");
    resolve_entire_stack_two_player(&mut engine);

    assert!(!engine.state.objects.contains_key(&token));
    assert_eq!(engine.state.players[0].library.len(), library_before - 1);
    assert!(engine.state.active_exile_play_permissions.is_empty());
    for legal in engine.initial_response_batch().legal_by_player.values() {
        assert!(legal
            .zone_cast_actions
            .iter()
            .all(|action| action.object_id != token));
        assert!(legal.exile_play_permission_groups.is_empty());
    }
}

#[test]
fn airbending_lesson_four_players_only_owner_can_recast_with_normal_timing() {
    let seats = [11, 23, 47, 89];
    let mut engine = GameEngine::new(
        149_004,
        &seats[..2],
        20,
        Some(vec![vec!["forest".into(); 30]; 2]),
        true,
    )
    .expect("new game");
    // The public constructor still admits two seats; extend the fixture as other
    // multiplayer engine scenarios do to exercise player-set-generic resolution.
    for &player in &seats[2..] {
        engine
            .state
            .players
            .push(tricerules_core::state::PlayerState::new(player, 20));
    }
    for _ in 0..8 {
        let player = engine.state.priority_player_id();
        engine
            .apply_command(player, &pass())
            .expect("advance to main phase");
    }
    assert_eq!(engine.state.turn_step, tricerules_core::TurnStep::Main1);
    // Caster, controller, owner, and uninvolved observer occupy distinct seats.
    let target = inject_creature_on_battlefield(&mut engine, 2, "grizzly_bears");
    engine.state.players[2]
        .battlefield
        .retain(|&id| id != target);
    engine.state.players[1].battlefield.push(target);
    engine.state.objects.get_mut(&target).unwrap().controller = seats[1];
    engine
        .state
        .objects
        .get_mut(&target)
        .unwrap()
        .base_controller = seats[1];
    let old_generation = engine
        .state
        .zone_change_generation
        .get(&target)
        .copied()
        .unwrap_or(0);
    inject_card_into_hand(&mut engine, 0, "airbending_lesson");
    let slot = hand_index_for_card(&engine, 0, "airbending_lesson");
    give_mana(
        &mut engine,
        seats[0],
        ManaGift {
            w: 1,
            c: 2,
            ..Default::default()
        },
    );
    engine
        .apply_command(seats[0], &cast_spell(slot, target_object(target)))
        .expect("airbend stolen creature");
    for _ in 0..4 {
        let player = engine.state.priority_player_id();
        engine
            .apply_command(player, &pass())
            .expect("resolve lesson");
    }
    assert!(engine.state.stack.is_empty());
    assert_eq!(engine.state.objects[&target].zone, Zone::Exile);
    let permission = engine
        .state
        .active_exile_play_permissions
        .iter()
        .find(|p| p.object_id == target)
        .expect("owner permission")
        .clone();
    assert_eq!(permission.player_id, seats[2]);
    assert!(permission.zone_change_generation > old_generation);
    let command = RuledCommand {
        cmd: Some(Cmd::CastSpell(CastSpell {
            source: Some(exile_cast_source(target, permission.zone_change_generation)),
            cast_method: CastMethod::Permission as i32,
            casting_permission_id: Some(permission.group_id),
            ..Default::default()
        })),
    };
    for (index, &player) in seats.iter().enumerate() {
        give_mana(
            &mut engine,
            player,
            ManaGift {
                c: 2,
                ..Default::default()
            },
        );
        engine.state.priority_idx = index;
        let legal = engine.initial_response_batch();
        assert!(legal.legal_by_player[&player]
            .zone_cast_actions
            .iter()
            .all(|a| a.object_id != target));
        for &recipient in &seats {
            assert_eq!(
                legal.legal_by_player[&recipient]
                    .exile_play_permission_groups
                    .len(),
                usize::from(recipient == seats[2])
            );
        }
        let command_index = engine.state.command_index;
        let pool = engine.state.players[index].mana_pool;
        assert!(
            engine.apply_command(player, &command).is_err(),
            "nonowners lack permission; owner must wait for their turn"
        );
        assert_eq!(engine.state.command_index, command_index);
        assert_eq!(engine.state.players[index].mana_pool, pool);
        assert_eq!(engine.state.objects[&target].zone, Zone::Exile);
        assert!(engine.state.stack.is_empty());
    }
    engine.state.active_player_idx = 2;
    engine.state.priority_idx = 2;
    let legal = engine.initial_response_batch();
    let action = legal.legal_by_player[&seats[2]]
        .zone_cast_actions
        .iter()
        .find(|a| a.object_id == target)
        .expect("owner can cast on their turn");
    assert_eq!(action.cost, "{2}");
    assert_eq!(action.casting_permission_id, Some(permission.group_id));
    engine
        .apply_command(seats[2], &command)
        .expect("owner pays two generic mana");
    assert_eq!(engine.state.objects[&target].zone, Zone::Stack);
    assert!(engine.state.active_exile_play_permissions.is_empty());
    for _ in 0..4 {
        let player = engine.state.priority_player_id();
        engine
            .apply_command(player, &pass())
            .expect("resolve creature");
    }
    assert_eq!(engine.state.objects[&target].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&target].owner, seats[2]);
    assert_eq!(engine.state.objects[&target].controller, seats[2]);
    assert!(engine.state.players[2].battlefield.contains(&target));
}

#[test]
fn issue_149_permission_cost_and_identity_are_engine_authoritative() {
    let mut engine = GameEngine::new(
        149_001,
        &[0, 1],
        20,
        Some(vec![vec!["forest".into(); 30]; 2]),
        true,
    )
    .expect("new game");
    advance_to_main1_from_game_start(&mut engine);

    let object_id = inject_card_into_hand(&mut engine, 0, "grizzly_bears");
    engine.state.players[0].hand.retain(|&id| id != object_id);
    engine.state.players[0].exile.push(object_id);
    engine.state.objects.get_mut(&object_id).expect("card").zone = Zone::Exile;
    let generation = engine
        .state
        .zone_change_generation
        .get(&object_id)
        .copied()
        .unwrap_or(0);
    engine
        .state
        .active_exile_play_permissions
        .push(ActiveExilePlayPermission {
            group_id: 149,
            player_id: 0,
            source_label: "Airbending Lesson".into(),
            object_id,
            zone_change_generation: generation,
            scope: ExilePlayPermissionScope::CastCard,
            cast_cost: ExilePermissionCastCost::AlternativeManaCost(
                ManaCost::parse("{2}").expect("cost"),
            ),
            origin: ExilePlayPermissionOrigin::Effect,
            available_after_turn_instance: None,
            expires_at_cleanup_turn_instance: None,
        });
    engine
        .state
        .active_exile_play_permissions
        .push(ActiveExilePlayPermission {
            group_id: 150,
            player_id: 0,
            source_label: "Second permission".into(),
            object_id,
            zone_change_generation: generation,
            scope: ExilePlayPermissionScope::CastCard,
            cast_cost: ExilePermissionCastCost::AlternativeManaCost(
                ManaCost::parse("{3}").expect("cost"),
            ),
            origin: ExilePlayPermissionOrigin::Effect,
            available_after_turn_instance: None,
            expires_at_cleanup_turn_instance: None,
        });

    let legal = engine.initial_response_batch();
    let actions = legal.legal_by_player[&0]
        .zone_cast_actions
        .iter()
        .filter(|action| action.object_id == object_id)
        .collect::<Vec<_>>();
    assert_eq!(actions.len(), 2, "overlapping permissions stay distinct");
    let action = actions
        .iter()
        .find(|action| action.casting_permission_id == Some(149))
        .expect("selected permission cast action");
    assert_eq!(action.cost, "{2}");
    assert_eq!(action.cast_method, CastMethod::Permission as i32);
    assert_eq!(action.casting_permission_id, Some(149));
    assert!(legal.legal_by_player[&1].zone_cast_actions.is_empty());

    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );
    let before_command_index = engine.state.command_index;
    let before_pool = engine.state.players[0].mana_pool;
    let before_stack_len = engine.state.stack.len();
    let cast = |permission_id| RuledCommand {
        cmd: Some(Cmd::CastSpell(CastSpell {
            source: Some(exile_cast_source(object_id, generation)),
            cast_method: CastMethod::Permission as i32,
            casting_permission_id: permission_id,
            ..Default::default()
        })),
    };
    assert!(engine.apply_command(0, &cast(Some(151))).is_err());
    assert_eq!(engine.state.command_index, before_command_index);
    assert_eq!(engine.state.players[0].mana_pool, before_pool);
    assert_eq!(engine.state.stack.len(), before_stack_len);
    assert_eq!(engine.state.objects[&object_id].zone, Zone::Exile);

    engine
        .apply_command(0, &cast(Some(149)))
        .expect("cast for the permission cost");
    assert_eq!(engine.state.objects[&object_id].zone, Zone::Stack);
    assert!(engine.state.active_exile_play_permissions.is_empty());
}

#[test]
fn airbending_lesson_grants_the_exiled_cards_owner_and_draws() {
    let mut engine = GameEngine::new(
        149_002,
        &[0, 1],
        20,
        Some(vec![vec!["forest".into(); 30]; 2]),
        true,
    )
    .expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    let target = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    engine
        .state
        .objects
        .get_mut(&target)
        .expect("target")
        .controller = 0;
    let lesson = inject_card_into_hand(&mut engine, 0, "airbending_lesson");
    let hand_index = engine.state.players[0]
        .hand
        .iter()
        .position(|&id| id == lesson)
        .expect("lesson hand slot");
    let library_before = engine.state.players[0].library.len();
    give_mana(
        &mut engine,
        0,
        ManaGift {
            w: 1,
            c: 2,
            ..Default::default()
        },
    );
    engine
        .apply_command(0, &cast_spell(hand_index, target_object(target)))
        .expect("cast lesson");
    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(engine.state.objects[&target].zone, Zone::Exile);
    assert_eq!(engine.state.players[0].library.len(), library_before - 1);
    let permission = engine
        .state
        .active_exile_play_permissions
        .iter()
        .find(|permission| permission.object_id == target)
        .expect("airbend permission");
    assert_eq!(permission.player_id, 1, "owner, not controller or caster");
    assert_eq!(permission.source_label, "Airbending Lesson");
    assert_eq!(
        permission.cast_cost,
        ExilePermissionCastCost::AlternativeManaCost(ManaCost::parse("{2}").expect("cost"))
    );
    let permission_id = permission.group_id;
    assert!(engine.initial_response_batch().legal_by_player[&0]
        .zone_cast_actions
        .is_empty());

    engine.state.active_player_idx = 1;
    engine.state.priority_idx = 1;
    engine.state.turn_step = tricerules_core::TurnStep::Main1;
    let legal = engine.initial_response_batch();
    let action = legal.legal_by_player[&1]
        .zone_cast_actions
        .iter()
        .find(|action| action.object_id == target)
        .expect("owner cast action");
    assert_eq!(action.cost, "{2}");
    assert_eq!(action.casting_permission_id, Some(permission_id));
}
