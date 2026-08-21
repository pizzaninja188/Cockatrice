//! Issue #99: CR 709.5 Room casting, public door state, and special-action unlocks.

use crate::helpers::*;
use tricerules_cards::Keyword;
use tricerules_core::{TurnStep, Zone};
use tricerules_proto::ruled::v1::dev_command::Dev;
use tricerules_proto::ruled::v1::ruled_event::Ev;
use tricerules_proto::ruled::v1::{DevCommand, DevMoveCard, DevPutCardInZone, DevZone};

fn conjure_room(engine: &mut GameEngine, name: &str, card_id: &str) -> u32 {
    engine
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::DevCommand(DevCommand {
                    target_player_id: 0,
                    dev: Some(Dev::PutCardInZone(DevPutCardInZone {
                        card_name: name.into(),
                        zone: DevZone::Battlefield as i32,
                        ready: true,
                    })),
                })),
            },
        )
        .expect("conjure Room");
    battlefield_object_for_card(engine, 0, card_id)
}

fn execute_unlock(object_id: u32, generation: u64, face_index: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ExecutePermanentAction(ExecutePermanentAction {
            kind: PermanentActionKind::UnlockRoomDoor as i32,
            object_id,
            expected_zone_change_generation: generation,
            face_index: Some(face_index),
            flex_payments: Vec::new(),
            restricted_mana: Vec::new(),
        })),
    }
}

#[test]
fn either_room_door_casts_and_only_that_door_unlocks() {
    let decks = Some(vec![
        deck_with("mountain", &["glassworks_shattered_yard"]),
        vec!["forest".into(); 20],
    ]);
    let mut engine = GameEngine::new(99_001, &[0, 1], 20, decks, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    let room = relocate_to_hand(&mut engine, 0, "glassworks_shattered_yard");
    let hand_index = engine.state.players[0]
        .hand
        .iter()
        .position(|candidate| *candidate == room)
        .expect("Room in hand");
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
        .apply_command(0, &cast_spell_face(hand_index, Vec::new(), 1))
        .expect("cast Shattered Yard");
    pass_both_players(&mut engine);

    assert_eq!(engine.state.objects[&room].zone, Zone::Battlefield);
    assert_eq!(engine.state.room_states[&room].unlocked, [false, true]);
    let characteristics = engine.characteristics(room).expect("Room characteristics");
    assert_eq!(characteristics.types, vec!["Enchantment", "Room"]);

    let legal = engine.initial_response_batch();
    let actions: Vec<_> = legal.legal_by_player[&0]
        .permanent_actions
        .iter()
        .filter(|action| action.object_id == room)
        .collect();
    assert_eq!(actions.len(), 1);
    assert_eq!(actions[0].face_index, Some(0));
    assert_eq!(actions[0].label, "Unlock Glassworks — {2}{R}");
    assert!(legal.legal_by_player[&1].permanent_actions.is_empty());
}

#[test]
fn unlocking_is_atomic_retains_priority_and_refreshes_public_room_state() {
    let decks = Some(vec![
        deck_with("mountain", &["glassworks_shattered_yard"]),
        vec!["forest".into(); 20],
    ]);
    let mut engine = GameEngine::new(99_002, &[0, 1], 20, decks, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    engine.enable_dev_commands();
    engine
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::DevCommand(DevCommand {
                    target_player_id: 0,
                    dev: Some(Dev::PutCardInZone(DevPutCardInZone {
                        card_name: "Glassworks // Shattered Yard".into(),
                        zone: DevZone::Battlefield as i32,
                        ready: true,
                    })),
                })),
            },
        )
        .expect("conjure locked Room");
    let room = battlefield_object_for_card(&engine, 0, "glassworks_shattered_yard");
    assert_eq!(engine.state.room_states[&room].unlocked, [false, false]);
    let generation = engine.state.zone_change_generation[&room];

    let pool_before = engine.state.players[0].mana_pool;
    let error = engine
        .apply_command(0, &execute_unlock(room, generation + 1, 0))
        .expect_err("stale generation rejected");
    assert!(error.to_string().contains("stale"));
    let pool_after = engine.state.players[0].mana_pool;
    assert_eq!(
        (
            pool_after.white,
            pool_after.blue,
            pool_after.black,
            pool_after.red,
            pool_after.green,
            pool_after.colorless,
        ),
        (
            pool_before.white,
            pool_before.blue,
            pool_before.black,
            pool_before.red,
            pool_before.green,
            pool_before.colorless,
        )
    );
    assert_eq!(engine.state.room_states[&room].unlocked, [false, false]);

    give_mana(
        &mut engine,
        0,
        ManaGift {
            r: 1,
            c: 2,
            ..Default::default()
        },
    );
    let batch = engine
        .apply_command(0, &execute_unlock(room, generation, 0))
        .expect("unlock Glassworks");
    assert_eq!(engine.state.room_states[&room].unlocked, [true, false]);
    assert_eq!(engine.state.priority_player_id(), 0);
    assert!(
        engine.state.stack.is_empty(),
        "unlock itself does not use the stack"
    );

    let zone_view = batch
        .events
        .iter()
        .find_map(|event| match event.ev.as_ref() {
            Some(Ev::ZoneView(view)) => Some(view),
            _ => None,
        })
        .expect("unlock publishes a battlefield refresh");
    assert!(!zone_view.battlefields_unchanged);
    let published = &zone_view.per_player[0].battlefield_objects[0].room_doors;
    assert_eq!(published.len(), 2);
    assert!(published[0].unlocked);
    assert!(!published[1].unlocked);
}

#[test]
fn unlock_revalidates_controller_timing_stack_payment_and_door_state() {
    let decks = Some(vec![
        deck_with("forest", &["giant_growth"]),
        vec!["forest".into(); 20],
    ]);
    let mut engine = GameEngine::new(99_006, &[0, 1], 20, decks, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    engine.enable_dev_commands();
    let room = conjure_room(
        &mut engine,
        "Glassworks // Shattered Yard",
        "glassworks_shattered_yard",
    );
    let generation = engine.state.zone_change_generation[&room];

    assert!(engine
        .apply_command(1, &execute_unlock(room, generation, 0))
        .is_err());
    assert!(engine
        .apply_command(0, &execute_unlock(room, generation, 0))
        .is_err());
    assert_eq!(engine.state.room_states[&room].unlocked, [false, false]);

    engine.state.objects.get_mut(&room).unwrap().controller = 1;
    assert!(engine
        .apply_command(0, &execute_unlock(room, generation, 0))
        .expect_err("former controller rejected")
        .to_string()
        .contains("do not control"));
    assert_eq!(engine.state.room_states[&room].unlocked, [false, false]);
    engine.state.objects.get_mut(&room).unwrap().controller = 0;

    engine.state.turn_step = TurnStep::BeginCombat;
    assert!(engine
        .apply_command(0, &execute_unlock(room, generation, 0))
        .expect_err("nonmain phase rejected")
        .to_string()
        .contains("main phase"));
    engine.state.turn_step = TurnStep::Main1;

    let creature = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let growth = relocate_to_hand(&mut engine, 0, "giant_growth");
    let growth_index = engine.state.players[0]
        .hand
        .iter()
        .position(|candidate| *candidate == growth)
        .expect("Giant Growth in hand");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            g: 1,
            ..Default::default()
        },
    );
    engine
        .apply_command(0, &cast_spell(growth_index, target_object(creature)))
        .expect("cast Giant Growth");
    assert!(engine
        .apply_command(0, &execute_unlock(room, generation, 0))
        .expect_err("nonempty stack rejected")
        .to_string()
        .contains("empty stack"));
    pass_both_players(&mut engine);

    give_mana(
        &mut engine,
        0,
        ManaGift {
            r: 1,
            c: 2,
            ..Default::default()
        },
    );
    engine
        .apply_command(0, &execute_unlock(room, generation, 0))
        .expect("unlock Glassworks");
    assert!(engine
        .apply_command(0, &execute_unlock(room, generation, 0))
        .expect_err("repeated door rejected")
        .to_string()
        .contains("already unlocked"));
}

#[test]
fn noncast_entry_and_zone_change_start_room_fully_locked() {
    let mut engine = GameEngine::new_with_default_decks(99_003, &[0, 1], 20).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    engine.enable_dev_commands();
    let command = |dev| RuledCommand {
        cmd: Some(Cmd::DevCommand(DevCommand {
            target_player_id: 0,
            dev: Some(dev),
        })),
    };
    engine
        .apply_command(
            0,
            &command(Dev::PutCardInZone(DevPutCardInZone {
                card_name: "Ticket Booth // Tunnel of Hate".into(),
                zone: DevZone::Battlefield as i32,
                ready: true,
            })),
        )
        .expect("noncast entry");
    let room = battlefield_object_for_card(&engine, 0, "ticket_booth_tunnel_of_hate");
    assert_eq!(engine.state.room_states[&room].unlocked, [false, false]);

    engine
        .apply_command(
            0,
            &command(Dev::MoveCard(DevMoveCard {
                card_name: "Ticket Booth // Tunnel of Hate".into(),
                zone: DevZone::Graveyard as i32,
                ready: false,
            })),
        )
        .expect("leave battlefield");
    assert!(!engine.state.room_states.contains_key(&room));
    engine
        .apply_command(
            0,
            &command(Dev::MoveCard(DevMoveCard {
                card_name: "Ticket Booth // Tunnel of Hate".into(),
                zone: DevZone::Battlefield as i32,
                ready: true,
            })),
        )
        .expect("return to battlefield");
    assert_eq!(engine.state.room_states[&room].unlocked, [false, false]);
}

#[test]
fn fully_unlock_edge_shares_trigger_ordering_and_updates_door_count_power() {
    let mut engine = GameEngine::new_with_default_decks(99_004, &[0, 1], 20).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    engine.enable_dev_commands();
    let room = conjure_room(
        &mut engine,
        "Ticket Booth // Tunnel of Hate",
        "ticket_booth_tunnel_of_hate",
    );
    let apparition = conjure_room(&mut engine, "Erratic Apparition", "erratic_apparition");
    let soulrager = conjure_room(&mut engine, "Rampaging Soulrager", "rampaging_soulrager");
    assert_eq!(engine.effective_power(soulrager), Some(1));
    let generation = engine.state.zone_change_generation[&room];

    give_mana(
        &mut engine,
        0,
        ManaGift {
            r: 3,
            c: 6,
            ..Default::default()
        },
    );
    engine
        .apply_command(0, &execute_unlock(room, generation, 1))
        .expect("unlock Tunnel first");
    assert_eq!(engine.effective_power(soulrager), Some(1));
    assert!(engine.state.stack.is_empty());

    engine
        .apply_command(0, &execute_unlock(room, generation, 0))
        .expect("fully unlock with Ticket Booth");
    assert_eq!(engine.effective_power(soulrager), Some(4));
    let order = engine
        .state
        .pending_trigger_order
        .as_ref()
        .expect("door and eerie triggers share one ordering prompt");
    assert_eq!(order.candidates.len(), 2);
    assert!(order
        .candidates
        .iter()
        .any(|candidate| candidate.source_permanent_id == room));
    assert!(order
        .candidates
        .iter()
        .any(|candidate| candidate.source_permanent_id == apparition));
}

#[test]
fn room_attack_triggers_use_the_declared_attacker_cohort() {
    let mut engine = GameEngine::new_with_default_decks(99_005, &[0, 1], 20).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    engine.enable_dev_commands();
    let room = conjure_room(
        &mut engine,
        "Ticket Booth // Tunnel of Hate",
        "ticket_booth_tunnel_of_hate",
    );
    engine.state.room_states.get_mut(&room).unwrap().unlocked = [false, true];
    let first = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let second = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    engine
        .apply_command(0, &primitive_yield())
        .expect("begin combat");
    pass_both_players(&mut engine);

    let batch = engine
        .apply_command(0, &declare_attackers(vec![first, second]))
        .expect("declare two attackers");
    let target_prompt = batch
        .events
        .iter()
        .find_map(|event| match event.ev.as_ref() {
            Some(Ev::TriggerNeedsTarget(prompt)) => Some(prompt),
            _ => None,
        })
        .expect("Tunnel of Hate asks for an attacking creature");
    let mut candidates = target_prompt
        .targets
        .as_ref()
        .expect("target candidates")
        .groups[0]
        .valid_permanent_ids
        .clone();
    candidates.sort_unstable();
    assert_eq!(candidates, vec![first, second]);
}

#[test]
fn widows_walk_binds_the_only_declared_attacker() {
    let mut engine = GameEngine::new_with_default_decks(99_007, &[0, 1], 20).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    engine.enable_dev_commands();
    let room = conjure_room(
        &mut engine,
        "Derelict Attic // Widow's Walk",
        "derelict_attic_widows_walk",
    );
    engine.state.room_states.get_mut(&room).unwrap().unlocked = [false, true];
    let attacker = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    engine
        .apply_command(0, &primitive_yield())
        .expect("begin combat");
    pass_both_players(&mut engine);

    engine
        .apply_command(0, &declare_attackers(vec![attacker]))
        .expect("attack alone");
    assert_eq!(
        engine
            .state
            .stack
            .last()
            .and_then(|item| item.source_permanent_id),
        Some(room)
    );
    pass_both_players(&mut engine);
    assert_eq!(engine.effective_power(attacker), Some(3));
    assert!(engine
        .characteristics(attacker)
        .expect("attacker characteristics")
        .keywords
        .contains(&Keyword::Deathtouch));
}

#[test]
fn room_trigger_stack_card_uses_the_physical_double_sided_name() {
    let mut engine = GameEngine::new_with_default_decks(99_008, &[0, 1], 20).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    engine.enable_dev_commands();
    let room = conjure_room(
        &mut engine,
        "Glassworks // Shattered Yard",
        "glassworks_shattered_yard",
    );
    engine.state.room_states.get_mut(&room).unwrap().unlocked = [false, true];

    engine
        .apply_command(0, &primitive_yield())
        .expect("main 1 to beginning of combat");
    engine
        .apply_command(0, &primitive_yield())
        .expect("beginning of combat advance");
    if engine.state.turn_step == TurnStep::DeclareAttackers {
        engine
            .apply_command(0, &primitive_yield())
            .expect("declare no attackers");
    }
    engine
        .apply_command(0, &primitive_yield())
        .expect("end combat to main 2");
    let batch = engine
        .apply_command(0, &primitive_yield())
        .expect("main 2 to end step");

    let pushed = batch
        .events
        .iter()
        .find_map(|event| match event.ev.as_ref() {
            Some(Ev::StackPushed(pushed)) => Some(pushed),
            _ => None,
        })
        .expect("Shattered Yard trigger pushed");
    assert_eq!(pushed.description, "Glassworks // Shattered Yard");
    assert_eq!(
        pushed.ability_annotation,
        "At the beginning of your end step, this Room deals 1 damage to each opponent."
    );
}
