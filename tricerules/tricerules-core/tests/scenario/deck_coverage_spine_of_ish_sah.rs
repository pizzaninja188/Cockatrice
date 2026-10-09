//! Spine of Ish Sah: mandatory ETB destruction and incarnation-bound owner-hand return.
use crate::helpers::*;
use tricerules_cards::primitives::{ContinuousEffectKind, EffectDuration};
use tricerules_core::state::{ActiveDeathReplacement, CopiableValues};
use tricerules_core::{AffectedScope, ContinuousEffect, Zone};
use tricerules_proto::ruled::v1::{dev_command, DevCommand, DevMoveCard, DevZone};

const SPINE: &str = "spine_of_ish_sah";

fn spine_engine(seed: u64) -> GameEngine {
    assert!(
        tricerules_cards::registry::global().get(SPINE).is_some(),
        "missing exact Spine of Ish Sah"
    );
    let deck = deck_with("forest", &["sol_ring"]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1],
        20,
        Some(vec![deck; 2]),
        true,
    )
    .unwrap();
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn cast_spine(engine: &mut GameEngine) -> u32 {
    inject_card_into_hand(engine, 0, SPINE);
    let slot = hand_index_for_card(engine, 0, SPINE);
    let source = engine.state.players[0].hand[slot];
    give_mana(
        engine,
        0,
        ManaGift {
            c: 7,
            ..Default::default()
        },
    );
    engine.apply_command(0, &cast_spell(slot, vec![])).unwrap();
    pass_both_players(engine);
    source
}

fn enter_spine(engine: &mut GameEngine) -> u32 {
    let ring = move_ready_to_battlefield(engine, 0, "sol_ring");
    let source = cast_spine(engine);
    engine.apply_command(0, &choose_target(ring)).unwrap();
    resolve_entire_stack_two_player(engine);
    assert_eq!(engine.state.objects[&source].zone, Zone::Battlefield);
    source
}

fn move_named(engine: &mut GameEngine, name: &str, zone: DevZone) {
    engine.enable_dev_commands();
    engine
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::DevCommand(DevCommand {
                    target_player_id: 0,
                    dev: Some(dev_command::Dev::MoveCard(DevMoveCard {
                        card_name: name.into(),
                        zone: zone as i32,
                        ready: false,
                    })),
                })),
            },
        )
        .unwrap();
}

fn remove_source_abilities(engine: &mut GameEngine, source: u32) {
    engine.state.continuous_effects.push(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: None,
        affected: AffectedScope::Single(source),
        kind: ContinuousEffectKind::Layer6RemoveAllAbilities,
        condition: None,
        duration: EffectDuration::UntilEndOfTurn,
        timestamp: 1000,
    });
}

fn sacrifice_to_ironworks(engine: &GameEngine, ironworks: u32, source: u32) -> RuledCommand {
    let mut command = activate_ability_with_costs(
        ironworks,
        0,
        vec![],
        vec![permanent_cost_selection(0, source)],
    );
    let Some(Cmd::ActivateAbility(activation)) = &mut command.cmd else {
        unreachable!()
    };
    activation.expected_zone_change_generation = engine.state.zone_change_generation[&ironworks];
    command
}

fn choose_target(object: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
            targets: target_object(object),
            ..Default::default()
        })),
    }
}

#[test]
fn spine_actual_cast_can_destroy_itself_and_returns_the_same_card_to_owners_hand() {
    let mut engine = spine_engine(2026093021);
    inject_card_into_hand(&mut engine, 0, SPINE);
    let slot = hand_index_for_card(&engine, 0, SPINE);
    let source = engine.state.players[0].hand[slot];
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 6,
            ..Default::default()
        },
    );
    let before = serde_json::to_value(&engine.state).unwrap();
    assert!(engine.apply_command(0, &cast_spell(slot, vec![])).is_err());
    assert_eq!(serde_json::to_value(&engine.state).unwrap(), before);
    engine.state.players[0].mana_pool.colorless = 0;
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 7,
            ..Default::default()
        },
    );
    engine.apply_command(0, &cast_spell(slot, vec![])).unwrap();
    pass_both_players(&mut engine);
    assert_eq!(engine.state.objects[&source].zone, Zone::Battlefield);
    let entered_generation = engine.state.zone_change_generation[&source];
    assert_eq!(engine.state.pending_triggers.len(), 1);
    for (actor, choice) in [
        (1, choose_target(source)),
        (
            0,
            RuledCommand {
                cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget::default())),
            },
        ),
        (
            0,
            RuledCommand {
                cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
                    decline: true,
                    ..Default::default()
                })),
            },
        ),
        (0, choose_target(999999)),
    ] {
        let before = serde_json::to_value(&engine.state).unwrap();
        assert!(engine.apply_command(actor, &choice).is_err());
        assert_eq!(serde_json::to_value(&engine.state).unwrap(), before);
    }
    engine.apply_command(0, &choose_target(source)).unwrap();
    pass_both_players(&mut engine);
    assert_eq!(engine.state.objects[&source].zone, Zone::Graveyard);
    assert_eq!(
        engine.state.zone_change_generation[&source],
        entered_generation + 1
    );
    assert_eq!(engine.state.stack.len(), 1);
    pass_both_players(&mut engine);
    assert_eq!(engine.state.objects[&source].zone, Zone::Hand);
    assert_eq!(
        engine.state.zone_change_generation[&source],
        entered_generation + 2
    );
    assert!(engine.state.players[0].hand.contains(&source));
    assert_eq!(engine.state.objects[&source].card_id, SPINE);
    let slot = engine.state.players[0]
        .hand
        .iter()
        .position(|id| *id == source)
        .unwrap();
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 7,
            ..Default::default()
        },
    );
    engine.apply_command(0, &cast_spell(slot, vec![])).unwrap();
    pass_both_players(&mut engine);
    assert_eq!(engine.state.objects[&source].zone, Zone::Battlefield);
    assert_eq!(
        engine.state.pending_triggers.len(),
        1,
        "recasting the same physical card triggers again"
    );
    assert_eq!(
        engine.state.zone_change_generation[&source],
        entered_generation + 4
    );
}

#[test]
fn spine_etb_can_destroy_a_noncreature_or_land_and_indestructible_is_a_legal_target() {
    for (case, target_card, expected_zone) in [
        (0, "sol_ring", Zone::Graveyard),
        (1, "forest", Zone::Graveyard),
        (2, "darksteel_forge", Zone::Battlefield),
    ] {
        let mut engine = spine_engine(2026093030 + case);
        inject_card_into_hand(&mut engine, 0, target_card);
        let target = move_ready_to_battlefield(&mut engine, 0, target_card);
        let source = cast_spine(&mut engine);
        let key = u64::from(source) << 32;
        assert!(
            engine.initial_response_batch().legal_by_player[&0].valid_targets_by_ability[&key]
                .groups[0]
                .valid_permanent_ids
                .contains(&target)
        );
        engine.apply_command(0, &choose_target(target)).unwrap();
        resolve_entire_stack_two_player(&mut engine);
        assert_eq!(engine.state.objects[&target].zone, expected_zone);
        assert_eq!(engine.state.objects[&source].zone, Zone::Battlefield);
    }
}

#[test]
fn spine_return_uses_captured_controller_but_owner_hand_and_exact_identity() {
    let mut engine = spine_engine(2026093040);
    let mascot = inject_creature_on_battlefield(&mut engine, 0, "spirit_mascot");
    let source = enter_spine(&mut engine);
    let duplicate = inject_card_into_hand(&mut engine, 0, SPINE);
    let generation = engine.state.zone_change_generation[&source];
    engine.state.players[0]
        .battlefield
        .retain(|id| *id != source);
    engine.state.players[1].battlefield.push(source);
    let object = engine.state.objects.get_mut(&source).unwrap();
    object.base_controller = 1;
    object.controller = 1;
    inject_card_into_hand(&mut engine, 1, "krark-clan_ironworks");
    let ironworks = move_ready_to_battlefield(&mut engine, 1, "krark-clan_ironworks");
    if engine.state.priority_player_id() != 1 {
        engine.apply_command(0, &pass()).unwrap();
    }
    let sacrifice = sacrifice_to_ironworks(&engine, ironworks, source);
    engine.apply_command(1, &sacrifice).unwrap();
    assert_eq!(engine.state.objects[&source].zone, Zone::Graveyard);
    assert_eq!(engine.state.stack.len(), 1);
    assert_eq!(engine.state.stack.last().unwrap().controller, 1);
    assert_eq!(engine.state.players[1].mana_pool.colorless, 2);
    engine
        .apply_command(engine.state.priority_player_id(), &pass())
        .unwrap();
    let batch = engine
        .apply_command(engine.state.priority_player_id(), &pass())
        .unwrap();
    assert_eq!(engine.state.objects[&source].zone, Zone::Hand);
    assert_eq!(engine.state.objects[&source].owner, 0);
    assert_eq!(engine.state.objects[&source].controller, 0);
    assert_eq!(engine.state.zone_change_generation[&source], generation + 2);
    assert!(engine.state.players[0].hand.contains(&source));
    assert!(engine.state.players[0].hand.contains(&duplicate));
    assert!(!engine.state.players[1].hand.contains(&source));
    assert!(batch.events.iter().any(|event| matches!(&event.ev,
        Some(Ev::PermanentMoved(moved)) if moved.object_id == source
            && moved.owner_player_id == 0 && moved.controller_player_id == 0
            && moved.card_id == SPINE
            && moved.destination == tricerules_proto::ruled::v1::permanent_moved::Destination::Hand as i32)));
    assert_eq!(
        engine.state.stack.len(),
        1,
        "graveyard departure observer fires"
    );
    assert_eq!(
        engine.state.stack.last().unwrap().source_permanent_id,
        Some(mascot)
    );
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        engine.state.objects[&mascot]
            .counter_count(tricerules_cards::primitives::CounterKind::PlusOnePlusOne),
        1
    );
}

#[test]
fn spine_return_rejects_exile_and_a_later_graveyard_incarnation() {
    for return_to_graveyard in [false, true] {
        let mut engine = spine_engine(2026093050 + u64::from(return_to_graveyard));
        let source = enter_spine(&mut engine);
        move_named(&mut engine, "Spine of Ish Sah", DevZone::Graveyard);
        let captured_generation = engine.state.zone_change_generation[&source];
        assert_eq!(engine.state.stack.len(), 1);
        move_named(&mut engine, "Spine of Ish Sah", DevZone::Exile);
        if return_to_graveyard {
            move_named(&mut engine, "Spine of Ish Sah", DevZone::Graveyard);
        }
        resolve_entire_stack_two_player(&mut engine);
        assert_eq!(
            engine.state.objects[&source].zone,
            if return_to_graveyard {
                Zone::Graveyard
            } else {
                Zone::Exile
            }
        );
        assert!(engine.state.zone_change_generation[&source] > captured_generation);
        assert!(!engine.state.players[0].hand.contains(&source));
    }
}

#[test]
fn spine_only_triggers_for_battlefield_to_graveyard_and_has_no_invented_sacrifice_ability() {
    for destination in [DevZone::Hand, DevZone::Exile] {
        let mut engine = spine_engine(2026093060 + destination as u64);
        let source = enter_spine(&mut engine);
        let before = format!("{:?}", engine.state);
        assert!(engine
            .apply_command(0, &activate_ability(source, 0, vec![]))
            .is_err());
        assert_eq!(format!("{:?}", engine.state), before);
        move_named(&mut engine, "Spine of Ish Sah", destination);
        assert!(engine.state.stack.is_empty());
        move_named(&mut engine, "Spine of Ish Sah", DevZone::Graveyard);
        assert!(
            engine.state.stack.is_empty(),
            "a nonbattlefield move cannot trigger the return"
        );
        assert_eq!(engine.state.objects[&source].zone, Zone::Graveyard);
    }
}

#[test]
fn spine_suppression_before_departure_prevents_capture_but_captured_return_is_independent() {
    for suppressed_before_departure in [false, true] {
        let mut engine = spine_engine(2026093070 + u64::from(suppressed_before_departure));
        let source = enter_spine(&mut engine);
        if suppressed_before_departure {
            remove_source_abilities(&mut engine, source);
        }
        move_named(&mut engine, "Spine of Ish Sah", DevZone::Graveyard);
        assert_eq!(
            engine.state.stack.len(),
            usize::from(!suppressed_before_departure)
        );
        if !suppressed_before_departure {
            remove_source_abilities(&mut engine, source);
        }
        resolve_entire_stack_two_player(&mut engine);
        assert_eq!(
            engine.state.objects[&source].zone,
            if suppressed_before_departure {
                Zone::Graveyard
            } else {
                Zone::Hand
            }
        );
    }
}

#[test]
fn spine_sacrifice_replaced_by_exile_does_not_capture_a_graveyard_return() {
    let mut engine = spine_engine(2026093080);
    let source = enter_spine(&mut engine);
    inject_card_into_hand(&mut engine, 0, "krark-clan_ironworks");
    let ironworks = move_ready_to_battlefield(&mut engine, 0, "krark-clan_ironworks");
    engine
        .state
        .death_replacement_effects
        .push(ActiveDeathReplacement {
            object_id: source,
            zone_change_generation: engine.state.zone_change_generation[&source],
        });
    let sacrifice = sacrifice_to_ironworks(&engine, ironworks, source);
    engine.apply_command(0, &sacrifice).unwrap();
    assert_eq!(engine.state.objects[&source].zone, Zone::Exile);
    assert_eq!(engine.state.players[0].mana_pool.colorless, 2);
    assert!(engine.state.stack.is_empty());
}

#[test]
fn spine_token_copy_cannot_return_but_real_sculpting_steel_copy_returns_its_underlying_card() {
    let mut token_engine = spine_engine(2026093090);
    let token = inject_creature_on_battlefield(&mut token_engine, 0, SPINE);
    token_engine
        .state
        .objects
        .get_mut(&token)
        .unwrap()
        .token_origin = Some(CopiableValues {
        source_card_id: SPINE.into(),
        source_face_index: 0,
        face: tricerules_cards::registry::global()
            .get(SPINE)
            .unwrap()
            .primary_face()
            .clone(),
        room_faces: None,
        display_name: "Spine of Ish Sah".into(),
    });
    move_named(&mut token_engine, "Spine of Ish Sah", DevZone::Graveyard);
    assert_eq!(
        token_engine.state.stack.len(),
        1,
        "token's departure still captures a trigger"
    );
    resolve_entire_stack_two_player(&mut token_engine);
    assert!(!token_engine.state.objects.contains_key(&token));
    assert!(!token_engine.state.players[0].hand.contains(&token));

    let mut engine = spine_engine(2026093091);
    let source = enter_spine(&mut engine);
    let ring = move_ready_to_battlefield(&mut engine, 0, "sol_ring");
    let copy = inject_card_into_hand(&mut engine, 0, "sculpting_steel");
    let slot = hand_index_for_card(&engine, 0, "sculpting_steel");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 3,
            ..Default::default()
        },
    );
    engine.apply_command(0, &cast_spell(slot, vec![])).unwrap();
    pass_both_players(&mut engine);
    engine
        .apply_command(0, &submit_resolution_choice(vec![source]))
        .unwrap();
    assert_eq!(engine.state.pending_triggers.len(), 1);
    engine.apply_command(0, &choose_target(ring)).unwrap();
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&copy].card_id, "sculpting_steel");
    assert_eq!(
        engine.characteristics(copy).unwrap().names,
        ["Spine of Ish Sah"]
    );
    let copy_generation = engine.state.zone_change_generation[&copy];
    move_named(&mut engine, "Spine of Ish Sah", DevZone::Graveyard);
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&source].zone, Zone::Hand);
    assert_eq!(
        engine.characteristics(copy).unwrap().names,
        ["Spine of Ish Sah"],
        "the copy survives donor departure"
    );
    move_named(&mut engine, "Sculpting Steel", DevZone::Graveyard);
    assert_eq!(engine.state.stack.len(), 1);
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&copy].zone, Zone::Hand);
    assert_eq!(engine.state.objects[&source].zone, Zone::Hand);
    assert_eq!(
        engine.state.zone_change_generation[&copy],
        copy_generation + 2
    );
    assert_eq!(engine.state.objects[&copy].card_id, "sculpting_steel");
    assert_eq!(
        engine.characteristics(copy).unwrap().names,
        ["Sculpting Steel"]
    );
}

#[test]
fn spine_etb_with_no_legal_target_discards_the_trigger_without_an_unusable_prompt() {
    let mut engine = spine_engine(2026093100);
    let source = inject_card_into_hand(&mut engine, 0, SPINE);
    engine.state.continuous_effects.push(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: None,
        affected: AffectedScope::PermanentsMatching {
            reference_player: 0,
            filter: Box::new(tricerules_cards::primitives::TargetFilter {
                kind: tricerules_cards::primitives::TargetKind::AnyPermanent,
                ..Default::default()
            }),
            exclude: None,
        },
        kind: ContinuousEffectKind::Layer6AddKeyword(tricerules_cards::Keyword::Shroud),
        condition: None,
        duration: EffectDuration::UntilEndOfTurn,
        timestamp: 1000,
    });
    let slot = hand_index_for_card(&engine, 0, SPINE);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 7,
            ..Default::default()
        },
    );
    engine.apply_command(0, &cast_spell(slot, vec![])).unwrap();
    pass_both_players(&mut engine);
    assert_eq!(engine.state.objects[&source].zone, Zone::Battlefield);
    assert!(engine.state.pending_triggers.is_empty());
    assert!(engine.state.blocking_choice().is_none());
    assert!(engine.state.stack.is_empty());
}

#[test]
fn spine_etb_rechecks_a_departed_target_without_destroying_another_card() {
    let mut engine = spine_engine(2026093110);
    let ring = move_ready_to_battlefield(&mut engine, 0, "sol_ring");
    let source = cast_spine(&mut engine);
    engine.apply_command(0, &choose_target(ring)).unwrap();
    move_named(&mut engine, "Sol Ring", DevZone::Hand);
    let returned = move_ready_to_battlefield(&mut engine, 0, "sol_ring");
    assert_eq!(
        returned, ring,
        "same physical card has a new battlefield incarnation"
    );
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&ring].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&source].zone, Zone::Battlefield);
}
