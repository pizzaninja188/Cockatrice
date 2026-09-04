//! Issue #200: controller-relative combat-damage cohorts and generation-bound
//! return with a type-setting continuous effect.

use crate::helpers::*;
use tricerules_core::state::CopiableValues;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::{dev_command, DevCommand, DevMoveCard, DevZone, RuledCommand};

fn issue_200_engine(seed: u64) -> GameEngine {
    let decks = Some(vec![
        deck_with("island", &["enduring_curiosity"]),
        vec!["forest".into(); 20],
    ]);
    let mut engine = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("new engine");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn dev_move_curiosity(zone: DevZone) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::DevCommand(DevCommand {
            target_player_id: 0,
            dev: Some(dev_command::Dev::MoveCard(DevMoveCard {
                card_name: "Enduring Curiosity".into(),
                zone: zone as i32,
                ready: false,
            })),
        })),
    }
}

#[test]
fn controlled_creatures_each_create_a_combat_damage_draw_trigger() {
    let mut engine = issue_200_engine(200_001);
    relocate_to_battlefield(&mut engine, 0, "enduring_curiosity", false);
    let first = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let second = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");

    engine
        .apply_command(0, &primitive_yield())
        .expect("advance to beginning of combat");
    engine
        .apply_command(0, &pass())
        .expect("active player pass");
    engine
        .apply_command(1, &pass())
        .expect("nonactive player pass");
    engine
        .apply_command(0, &declare_attackers(vec![first, second]))
        .expect("declare two attackers");
    engine
        .apply_command(0, &pass())
        .expect("active player pass after attackers");
    engine
        .apply_command(1, &pass())
        .expect("nonactive player pass after attackers");
    engine
        .apply_command(0, &pass())
        .expect("active player pass after blockers");
    engine
        .apply_command(1, &pass())
        .expect("nonactive player pass into combat damage");

    assert_eq!(engine.state.players[1].life, 16);
    assert_eq!(
        engine
            .state
            .pending_trigger_order
            .as_ref()
            .expect("simultaneous triggers require ordering")
            .candidates
            .len(),
        2
    );
    let hand_before = engine.state.players[0].hand.len();
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.players[0].hand.len(), hand_before + 2);
}

#[test]
fn death_return_enters_as_an_enchantment_and_uses_the_next_generation() {
    let mut engine = issue_200_engine(200_002);
    let curiosity = relocate_to_battlefield(&mut engine, 0, "enduring_curiosity", false);
    let creature_watcher = inject_creature_on_battlefield(&mut engine, 0, "beast-kin_ranger");
    let enchantment_watcher = inject_creature_on_battlefield(&mut engine, 0, "cult_healer");
    engine.state.players[0]
        .battlefield
        .retain(|object_id| *object_id != curiosity);
    engine.state.players[1].battlefield.push(curiosity);
    let controlled_by_opponent = engine.state.objects.get_mut(&curiosity).unwrap();
    controlled_by_opponent.base_controller = 1;
    controlled_by_opponent.controller = 1;
    let old_generation = engine
        .state
        .zone_change_generation
        .get(&curiosity)
        .copied()
        .unwrap_or(0);

    engine.state.objects.get_mut(&curiosity).unwrap().damage = 4;
    let priority = engine.state.priority_player_id();
    engine
        .apply_command(priority, &pass())
        .expect("state-based actions move lethal creature to graveyard");
    assert_eq!(engine.state.objects[&curiosity].zone, Zone::Graveyard);
    assert_eq!(engine.state.stack.len(), 1, "return trigger is published");

    pass_both_players(&mut engine);

    assert_eq!(engine.state.objects[&curiosity].zone, Zone::Battlefield);
    assert!(engine.state.players[0].battlefield.contains(&curiosity));
    assert_eq!(
        engine.state.zone_change_generation[&curiosity],
        old_generation + 2
    );
    let returned = engine
        .characteristics(curiosity)
        .expect("returned permanent");
    assert_eq!(returned.types, ["Enchantment"]);
    assert!(!returned.is_creature());
    assert_eq!(
        engine.state.stack.len(),
        1,
        "only the enchantment-entry observer triggers"
    );
    assert_eq!(
        engine.state.stack.last().unwrap().source_permanent_id,
        Some(enchantment_watcher)
    );
    assert_ne!(
        engine.state.stack.last().unwrap().source_permanent_id,
        Some(creature_watcher)
    );

    resolve_entire_stack_two_player(&mut engine);
    engine.enable_dev_commands();
    let priority = engine.state.priority_player_id();
    engine
        .apply_command(priority, &dev_move_curiosity(DevZone::Graveyard))
        .expect("move returned enchantment to graveyard");
    assert_eq!(engine.state.objects[&curiosity].zone, Zone::Graveyard);
    assert!(
        engine.state.stack.is_empty(),
        "the noncreature enchantment does not die"
    );
    engine
        .apply_command(priority, &dev_move_curiosity(DevZone::Hand))
        .expect("move returned card to hand");
    let replayed = move_ready_to_battlefield(&mut engine, 0, "enduring_curiosity");
    assert_eq!(replayed, curiosity);
    let replayed = engine
        .characteristics(replayed)
        .expect("replayed permanent");
    assert!(replayed.is_creature());
    assert!(replayed.types.iter().any(|kind| kind == "Cat"));
    assert!(replayed.types.iter().any(|kind| kind == "Glimmer"));
}

#[test]
fn return_trigger_rejects_tokens_and_stale_graveyard_generations() {
    let mut token_engine = issue_200_engine(200_003);
    let token = inject_creature_on_battlefield(&mut token_engine, 0, "enduring_curiosity");
    let definition = tricerules_cards::CardRegistry::global()
        .get("enduring_curiosity")
        .unwrap();
    token_engine
        .state
        .objects
        .get_mut(&token)
        .unwrap()
        .token_origin = Some(CopiableValues {
        source_card_id: "enduring_curiosity".into(),
        source_face_index: 0,
        face: definition.primary_face().clone(),
        room_faces: None,
        display_name: definition.name.clone(),
    });
    token_engine.state.objects.get_mut(&token).unwrap().damage = 4;
    let priority = token_engine.state.priority_player_id();
    token_engine
        .apply_command(priority, &pass())
        .expect("token dies");
    resolve_entire_stack_two_player(&mut token_engine);
    assert!(!token_engine.state.objects.contains_key(&token));

    let mut stale_engine = issue_200_engine(200_004);
    let stale = relocate_to_battlefield(&mut stale_engine, 0, "enduring_curiosity", false);
    stale_engine.state.objects.get_mut(&stale).unwrap().damage = 4;
    let priority = stale_engine.state.priority_player_id();
    stale_engine
        .apply_command(priority, &pass())
        .expect("card dies");
    stale_engine.state.players[0]
        .graveyard
        .retain(|object_id| *object_id != stale);
    stale_engine.state.players[0].hand.push(stale);
    stale_engine.state.objects.get_mut(&stale).unwrap().zone = Zone::Hand;
    *stale_engine
        .state
        .zone_change_generation
        .entry(stale)
        .or_default() += 1;
    stale_engine.state.players[0]
        .hand
        .retain(|object_id| *object_id != stale);
    stale_engine.state.players[0].graveyard.push(stale);
    stale_engine.state.objects.get_mut(&stale).unwrap().zone = Zone::Graveyard;
    *stale_engine
        .state
        .zone_change_generation
        .entry(stale)
        .or_default() += 1;
    resolve_entire_stack_two_player(&mut stale_engine);
    assert_eq!(stale_engine.state.objects[&stale].zone, Zone::Graveyard);
}
