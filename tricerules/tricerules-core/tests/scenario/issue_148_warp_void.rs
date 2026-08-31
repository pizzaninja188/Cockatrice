use super::helpers::*;
use tricerules_core::{TurnStep, Zone};

fn engine() -> GameEngine {
    let mut e = GameEngine::new(
        148001,
        &[0, 1],
        20,
        Some(vec![vec!["forest".into(); 30]; 2]),
        true,
    )
    .unwrap();
    advance_to_main1_from_game_start(&mut e);
    e
}

fn cast(e: &mut GameEngine, card: &str, method: CastMethod, targets: Vec<TargetRef>) -> u32 {
    let oid = inject_card_into_hand(e, 0, card);
    grant_pool(e, 0);
    e.apply_command(
        0,
        &RuledCommand {
            cmd: Some(Cmd::CastSpell(CastSpell {
                source: Some(hand_cast_source(e.state.players[0].hand.len() - 1)),
                cast_method: method as i32,
                targets,
                ..Default::default()
            })),
        },
    )
    .unwrap();
    oid
}

#[test]
fn issue_148_knight_warp_creates_human_soldier_and_normal_recast_stays() {
    let mut e = engine();
    let knight = cast(&mut e, "knight_luminary", CastMethod::Warp, vec![]);
    resolve_entire_stack_two_player(&mut e);
    let tokens: Vec<_> = e
        .state
        .objects
        .values()
        .filter(|o| o.zone == Zone::Battlefield && o.is_token())
        .collect();
    assert_eq!(tokens.len(), 1);
    let ch = e.characteristics(tokens[0].id).unwrap();
    assert!(ch.types.iter().any(|t| t == "Human"));
    assert!(ch.types.iter().any(|t| t == "Soldier"));
    e.state.turn_step = TurnStep::Main2;
    pass_both_players(&mut e);
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&knight].zone, Zone::Exile);
    assert!(e.initial_response_batch().legal_by_player[&0]
        .zone_cast_actions
        .is_empty());
    e.state.turn_instance += 1;
    e.state.turn_step = TurnStep::Main1;
    e.state.priority_idx = 0;
    grant_pool(&mut e, 0);
    let generation = e.state.zone_change_generation[&knight];
    let permission_id = e
        .state
        .active_exile_play_permissions
        .iter()
        .find(|permission| permission.object_id == knight)
        .expect("Warp recast permission")
        .group_id;
    e.apply_command(
        0,
        &RuledCommand {
            cmd: Some(Cmd::CastSpell(CastSpell {
                source: Some(exile_cast_source(knight, generation)),
                cast_method: CastMethod::Normal as i32,
                casting_permission_id: Some(permission_id),
                ..Default::default()
            })),
        },
    )
    .unwrap();
    resolve_entire_stack_two_player(&mut e);
    assert!(e.state.active_exile_play_permissions.is_empty());
    e.state.turn_step = TurnStep::Main2;
    pass_both_players(&mut e);
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&knight].zone, Zone::Battlefield);
    assert_eq!(
        e.state
            .objects
            .values()
            .filter(|o| o.zone == Zone::Battlefield && o.is_token())
            .count(),
        2
    );
}

fn choose(targets: Vec<TargetRef>) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
            targets,
            ..Default::default()
        })),
    }
}

#[test]
fn issue_148_weftblade_accepts_zero_one_or_two_including_itself_and_opponent() {
    for count in 0..=2 {
        let mut e = engine();
        let other = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        let enhancer = cast(&mut e, "weftblade_enhancer", CastMethod::Warp, vec![]);
        pass_both_players(&mut e);
        let targets: Vec<_> = [enhancer, other]
            .into_iter()
            .take(count)
            .flat_map(target_object)
            .collect();
        assert!(
            e.apply_command(0, &choose(vec![target_object(enhancer)[0]; 2]))
                .is_err(),
            "duplicate targets rejected"
        );
        e.apply_command(0, &choose(targets)).unwrap();
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.characteristics(enhancer).unwrap().power,
            Some(if count > 0 { 4 } else { 3 })
        );
        assert_eq!(
            e.characteristics(other).unwrap().power,
            Some(if count == 2 { 3 } else { 2 })
        );
    }
}

#[test]
fn issue_148_perigee_excludes_self_and_returns_other_creature_tapped_once() {
    let mut e = engine();
    let bear = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let other = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    let beckoner = cast(&mut e, "perigee_beckoner", CastMethod::Warp, vec![]);
    pass_both_players(&mut e);
    assert!(e
        .apply_command(0, &choose(target_object(beckoner)))
        .is_err());
    assert!(e.apply_command(0, &choose(target_object(other))).is_err());
    e.apply_command(0, &choose(target_object(bear))).unwrap();
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.characteristics(bear).unwrap().power, Some(4));
    cast(&mut e, "shock", CastMethod::Normal, target_object(bear));
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&bear].zone, Zone::Battlefield);
    assert!(e.state.objects[&bear].tapped);
    assert_eq!(e.characteristics(bear).unwrap().power, Some(2));
    cast(&mut e, "shock", CastMethod::Normal, target_object(bear));
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&bear].zone, Zone::Graveyard);
}

#[test]
fn issue_148_void_consumers_check_their_own_timing() {
    for active in [false, true] {
        let mut e = engine();
        cast(&mut e, "plasma_bolt", CastMethod::Normal, target_player(1));
        // A real departure after casting proves this is a resolution-time check.
        if active {
            let bear = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
            cast(&mut e, "unsummon", CastMethod::Normal, target_object(bear));
        }
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(e.state.players[1].life, if active { 17 } else { 18 });
        let hand = e.state.players[0].hand.len();
        cast(&mut e, "decode_transmissions", CastMethod::Normal, vec![]);
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(e.state.players[0].hand.len(), hand + 2);
        assert_eq!(e.state.players[0].life, if active { 20 } else { 18 });
        assert_eq!(e.state.players[1].life, if active { 15 } else { 18 });
        let maw = inject_creature_on_battlefield(&mut e, 0, "insatiable_skittermaw");
        e.state.turn_step = TurnStep::Main2;
        pass_both_players(&mut e);
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.characteristics(maw).unwrap().power,
            Some(if active { 3 } else { 2 })
        );
    }
}

#[test]
fn issue_148_temporal_cost_and_public_reveal_use_existing_choice_contract() {
    let mut e = engine();
    let temporal = inject_card_into_hand(&mut e, 0, "temporal_intervention");
    let victim = inject_card_into_hand(&mut e, 1, "shock");
    let slot = e.state.players[0].hand.len() - 1;
    let cost = |e: &mut GameEngine| {
        e.initial_response_batch().legal_by_player[&0]
            .hand_actions
            .iter()
            .find(|a| {
                a.hand_index == slot as u32
                    && a.kind
                        == tricerules_proto::ruled::v1::HandActionKind::HandActionCastSpell as i32
            })
            .map(|a| (a.cost.clone(), a.generic_cost_reduction))
            .unwrap()
    };
    assert_eq!(cost(&mut e), ("{2}{B}".into(), 0));
    cast(&mut e, "knight_luminary", CastMethod::Warp, vec![]);
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(cost(&mut e), ("{2}{B}".into(), 2));
    e.state.players[0].mana_pool = Default::default();
    e.state.players[0].mana_pool.black = 1;
    let batch = e
        .apply_command(0, &cast_spell(slot, target_player(1)))
        .unwrap();
    assert!(!batch.events.is_empty());
    pass_both_players(&mut e);
    assert!(e.state.pending_resolution.is_some());
    let before = e.state.command_index;
    let land = e.state.players[1]
        .hand
        .iter()
        .copied()
        .find(|id| e.state.objects[id].card_id == "forest")
        .unwrap();
    assert!(e
        .apply_command(0, &submit_resolution_choice(vec![land]))
        .is_err());
    assert_eq!(e.state.command_index, before);
    e.apply_command(0, &submit_resolution_choice(vec![victim]))
        .unwrap();
    assert_eq!(e.state.objects[&victim].zone, Zone::Graveyard);
    assert_eq!(e.state.objects[&temporal].zone, Zone::Graveyard);
    assert_eq!(
        e.state
            .turn_history
            .current
            .spell_casts
            .last()
            .unwrap()
            .mana_value,
        3
    );
}
