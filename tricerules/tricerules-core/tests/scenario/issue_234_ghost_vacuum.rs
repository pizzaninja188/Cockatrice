//! Ghost Vacuum: generation-bound linked exile and modified simultaneous reanimation.
use super::helpers::*;
use prost::Message;
use tricerules_cards::{CardRegistry, CounterKind, Keyword};
use tricerules_core::state::CopiableValues;
use tricerules_core::{TurnStep, Zone};
use tricerules_proto::ruled::v1::dev_command::Dev;
use tricerules_proto::ruled::v1::{
    BattlefieldObject, DevAddMana, DevCommand, DevMoveCard, DevPutCardInZone, DevZone, RuledCommand,
};

fn setup() -> GameEngine {
    let mut engine = GameEngine::new(
        234_001,
        &[0, 1],
        20,
        Some(vec![forest_only_deck(), forest_only_deck()]),
        true,
    )
    .expect("Ghost Vacuum is registered");
    pass_both_players(&mut engine);
    pass_both_players(&mut engine);
    assert_eq!(engine.state.turn_step, TurnStep::Main1);
    grant_pool(&mut engine, 0);
    engine.enable_dev_commands();
    engine
}

fn resolve_top(engine: &mut GameEngine) {
    pass_both_players(engine);
}

fn pass_all_players(engine: &mut GameEngine) {
    for _ in 0..engine.state.players.len() {
        let actor = engine.state.priority_player_id();
        engine.apply_command(actor, &pass()).unwrap();
    }
}

fn exile_with(engine: &mut GameEngine, vacuum: u32, target: u32) {
    engine.state.objects.get_mut(&vacuum).unwrap().tapped = false;
    let command = activate_ability_for(engine, vacuum, 0, target_object(target));
    engine
        .apply_command(0, &command)
        .expect("activate linked graveyard exile");
    resolve_top(engine);
}

fn release_with(engine: &mut GameEngine, vacuum: u32) {
    engine.state.objects.get_mut(&vacuum).unwrap().tapped = false;
    let command = activate_ability_for(engine, vacuum, 1, vec![]);
    engine
        .apply_command(0, &command)
        .expect("activate linked return");
    assert_ne!(engine.state.objects[&vacuum].zone, Zone::Battlefield);
    resolve_top(engine);
}

fn dev_move(target_player_id: i32, card_name: &str, zone: DevZone) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::DevCommand(DevCommand {
            target_player_id,
            dev: Some(Dev::MoveCard(DevMoveCard {
                card_name: card_name.into(),
                zone: zone as i32,
                ready: false,
            })),
        })),
    }
}

fn dev_put(target_player_id: i32, card_name: &str, zone: DevZone, ready: bool) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::DevCommand(DevCommand {
            target_player_id,
            dev: Some(Dev::PutCardInZone(DevPutCardInZone {
                card_name: card_name.into(),
                zone: zone as i32,
                ready,
            })),
        })),
    }
}

fn dev_add_colorless(target_player_id: i32, amount: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::DevCommand(DevCommand {
            target_player_id,
            dev: Some(Dev::AddMana(DevAddMana {
                w: 0,
                u: 0,
                b: 0,
                r: 0,
                g: 0,
                c: amount,
            })),
        })),
    }
}

fn apply_serialized_and_record(
    engine: &mut GameEngine,
    commands: &mut Vec<(i32, RuledCommand)>,
    batches: &mut Vec<tricerules_proto::ruled::v1::RuledEventBatch>,
    actor: i32,
    command: RuledCommand,
) {
    let command = RuledCommand::decode(command.encode_to_vec().as_slice()).unwrap();
    batches.push(engine.apply_command(actor, &command).unwrap());
    commands.push((actor, command));
}

fn published_object(engine: &mut GameEngine, player: usize, oid: u32) -> BattlefieldObject {
    engine
        .initial_response_batch()
        .events
        .iter()
        .find_map(|event| match &event.ev {
            Some(Ev::ZoneView(view)) => view
                .per_player
                .get(player)
                .and_then(|view| {
                    view.battlefield_objects
                        .iter()
                        .find(|object| object.object_id == oid)
                })
                .cloned(),
            _ => None,
        })
        .expect("published battlefield object")
}

#[test]
fn issue_234_returns_only_linked_creatures_with_spirit_characteristics_and_flying_counter() {
    let mut engine = setup();
    let vacuum = inject_permanent_on_battlefield(&mut engine, 0, "ghost_vacuum");
    let creature = inject_graveyard_card(&mut engine, 1, "hill_giant");
    let land = inject_graveyard_card(&mut engine, 1, "forest");
    let counter_prohibited = inject_graveyard_card(&mut engine, 1, "tatterkite");

    exile_with(&mut engine, vacuum, creature);
    assert_eq!(engine.state.objects[&creature].zone, Zone::Exile);

    exile_with(&mut engine, vacuum, land);
    assert_eq!(engine.state.objects[&land].zone, Zone::Exile);
    exile_with(&mut engine, vacuum, counter_prohibited);
    let snapshot = engine.initial_response_batch();
    let view = snapshot
        .events
        .iter()
        .find_map(|event| match &event.ev {
            Some(Ev::ZoneView(view)) => Some(view),
            _ => None,
        })
        .unwrap();
    let owner_view = view
        .per_player
        .iter()
        .find(|player| player.player_id == 1)
        .unwrap();
    assert!(owner_view.exile_object_ids.contains(&creature));
    assert!(owner_view.exile_object_ids.contains(&land));
    assert!(owner_view.exile_object_ids.contains(&counter_prohibited));

    release_with(&mut engine, vacuum);

    assert_eq!(engine.state.objects[&creature].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&creature].controller, 0);
    assert_eq!(engine.state.objects[&land].zone, Zone::Exile);
    assert_eq!(
        engine.state.objects[&counter_prohibited].zone,
        Zone::Battlefield
    );
    assert_eq!(
        engine.state.objects[&counter_prohibited]
            .counter_count(CounterKind::Keyword(Keyword::Flying)),
        0,
        "counter prohibition applies to the entry counter"
    );
    let prohibited = engine.characteristics(counter_prohibited).unwrap();
    assert!(prohibited.has_type("Spirit"));
    assert_eq!((prohibited.power, prohibited.toughness), (Some(1), Some(1)));
    assert_eq!(
        engine.state.objects[&creature].counter_count(CounterKind::Keyword(Keyword::Flying)),
        1
    );
    let characteristics = engine.characteristics(creature).unwrap();
    assert!(characteristics.has_type("Creature"));
    assert!(characteristics.has_type("Giant"));
    assert!(characteristics.has_type("Spirit"));
    assert_eq!(
        (characteristics.power, characteristics.toughness),
        (Some(1), Some(1))
    );
    assert!(characteristics.keywords.contains(&Keyword::Flying));
    let published = published_object(&mut engine, 0, creature);
    assert!(published.is_creature);
    assert_eq!((published.power, published.toughness), (1, 1));
    assert!(published.keywords.contains(&"Flying".into()));
    assert!(published.counters_annotation.contains("flying"));

    inject_card_into_hand(&mut engine, 0, "unsummon");
    let slot = hand_index_for_card(&engine, 0, "unsummon");
    engine
        .apply_command(0, &cast_spell(slot, target_object(creature)))
        .unwrap();
    resolve_top(&mut engine);
    let new_object = engine.characteristics(creature).unwrap();
    assert!(!new_object.has_type("Spirit"));
    assert_eq!((new_object.power, new_object.toughness), (Some(3), Some(3)));
    assert_eq!(
        engine.state.objects[&creature].counter_count(CounterKind::Keyword(Keyword::Flying)),
        0
    );
}

#[test]
fn issue_234_source_and_exile_generations_keep_links_separate() {
    let mut engine = setup();
    let first = inject_permanent_on_battlefield(&mut engine, 0, "ghost_vacuum");
    let second = inject_permanent_on_battlefield(&mut engine, 0, "ghost_vacuum");
    let first_card = inject_graveyard_card(&mut engine, 1, "grizzly_bears");
    let second_card = inject_graveyard_card(&mut engine, 1, "hill_giant");
    exile_with(&mut engine, first, first_card);
    exile_with(&mut engine, second, second_card);

    release_with(&mut engine, first);
    assert_eq!(engine.state.objects[&first_card].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&second_card].zone, Zone::Exile);
    release_with(&mut engine, second);
    assert_eq!(engine.state.objects[&second_card].zone, Zone::Battlefield);
}

#[test]
fn issue_234_blinked_source_cannot_use_its_old_link() {
    let mut engine = setup();
    engine.enable_dev_commands();
    let vacuum = inject_permanent_on_battlefield(&mut engine, 0, "ghost_vacuum");
    let creature = inject_graveyard_card(&mut engine, 1, "grizzly_bears");
    exile_with(&mut engine, vacuum, creature);
    engine
        .apply_command(0, &dev_move(0, "Ghost Vacuum", DevZone::Hand))
        .unwrap();
    engine
        .apply_command(0, &dev_move(0, "Ghost Vacuum", DevZone::Battlefield))
        .unwrap();
    release_with(&mut engine, vacuum);
    assert_eq!(engine.state.objects[&creature].zone, Zone::Exile);
}

#[test]
fn issue_234_card_that_leaves_and_reenters_exile_is_not_linked() {
    let mut engine = setup();
    engine.enable_dev_commands();
    let vacuum = inject_permanent_on_battlefield(&mut engine, 0, "ghost_vacuum");
    let creature = inject_graveyard_card(&mut engine, 1, "hill_giant");
    exile_with(&mut engine, vacuum, creature);
    let linked_generation = engine.state.zone_change_generation[&creature];
    engine
        .apply_command(0, &dev_move(1, "Hill Giant", DevZone::Hand))
        .unwrap();
    engine
        .apply_command(0, &dev_move(1, "Hill Giant", DevZone::Exile))
        .unwrap();
    assert!(engine.state.zone_change_generation[&creature] > linked_generation);
    release_with(&mut engine, vacuum);
    assert_eq!(engine.state.objects[&creature].zone, Zone::Exile);
}

#[test]
fn issue_234_simultaneous_return_waits_for_copy_choice_then_applies_modifiers() {
    let mut engine = setup();
    let vacuum = inject_permanent_on_battlefield(&mut engine, 0, "ghost_vacuum");
    let clone = inject_graveyard_card(&mut engine, 0, "clone");
    let bear = inject_graveyard_card(&mut engine, 1, "grizzly_bears");
    let model = inject_creature_on_battlefield(&mut engine, 0, "hill_giant");
    exile_with(&mut engine, vacuum, clone);
    exile_with(&mut engine, vacuum, bear);

    engine.state.objects.get_mut(&vacuum).unwrap().tapped = false;
    engine
        .apply_command(0, &activate_ability(vacuum, 1, vec![]))
        .unwrap();
    resolve_top(&mut engine);
    assert!(engine.state.pending_resolution.is_some());
    assert_eq!(engine.state.objects[&clone].zone, Zone::Exile);
    assert_eq!(engine.state.objects[&bear].zone, Zone::Exile);

    engine
        .apply_command(0, &submit_resolution_choice(vec![model]))
        .unwrap();
    for oid in [clone, bear] {
        assert_eq!(engine.state.objects[&oid].zone, Zone::Battlefield);
        assert_eq!(
            engine.state.objects[&oid].counter_count(CounterKind::Keyword(Keyword::Flying)),
            1
        );
        let characteristics = engine.characteristics(oid).unwrap();
        assert!(characteristics.has_type("Spirit"));
        assert_eq!(
            (characteristics.power, characteristics.toughness),
            (Some(1), Some(1))
        );
    }
    let copied = engine.characteristics(clone).unwrap();
    assert!(copied.has_type("Giant"));
}

#[test]
fn issue_234_multiplayer_entry_choices_follow_owner_apnap_before_atomic_return() {
    let mut engine = setup();
    engine
        .state
        .players
        .push(tricerules_core::state::PlayerState::new(3, 20));
    let vacuum = inject_permanent_on_battlefield(&mut engine, 0, "ghost_vacuum");
    let first = inject_graveyard_card(&mut engine, 1, "clone");
    let second = inject_graveyard_card(&mut engine, 2, "clone");
    let model = inject_creature_on_battlefield(&mut engine, 0, "hill_giant");
    for target in [first, second] {
        engine.state.objects.get_mut(&vacuum).unwrap().tapped = false;
        engine
            .apply_command(
                0,
                &activate_ability_for(&engine, vacuum, 0, target_object(target)),
            )
            .unwrap();
        pass_all_players(&mut engine);
    }

    engine.state.objects.get_mut(&vacuum).unwrap().tapped = false;
    engine
        .apply_command(0, &activate_ability_for(&engine, vacuum, 1, vec![]))
        .unwrap();
    pass_all_players(&mut engine);
    assert_eq!(engine.state.objects[&first].zone, Zone::Exile);
    assert_eq!(engine.state.objects[&second].zone, Zone::Exile);
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .deciding_player,
        1
    );

    engine
        .apply_command(1, &submit_resolution_choice(vec![model]))
        .unwrap();
    assert_eq!(engine.state.objects[&first].zone, Zone::Exile);
    assert_eq!(engine.state.objects[&second].zone, Zone::Exile);
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .deciding_player,
        3
    );
    engine
        .apply_command(3, &submit_resolution_choice(vec![model]))
        .unwrap();

    for (object_id, owner) in [(first, 1), (second, 3)] {
        let object = &engine.state.objects[&object_id];
        assert_eq!(object.zone, Zone::Battlefield);
        assert_eq!(object.owner, owner);
        assert_eq!(object.controller, 0);
        let characteristics = engine.characteristics(object_id).unwrap();
        assert!(characteristics.has_type("Spirit"));
        assert_eq!(
            (characteristics.power, characteristics.toughness),
            (Some(1), Some(1))
        );
    }
}

#[test]
fn issue_234_copied_release_and_permanent_or_token_copies_keep_source_links() {
    let mut engine = setup();
    let token = inject_permanent_on_battlefield(&mut engine, 0, "treasure");
    let vacuum = CardRegistry::global().get("ghost_vacuum").unwrap();
    engine.state.objects.get_mut(&token).unwrap().token_origin = Some(CopiableValues {
        source_card_id: "ghost_vacuum".into(),
        source_face_index: 0,
        face: vacuum.primary_face().clone(),
        room_faces: None,
        display_name: "Ghost Vacuum".into(),
    });
    let creature = inject_graveyard_card(&mut engine, 1, "grizzly_bears");
    exile_with(&mut engine, token, creature);

    engine.state.objects.get_mut(&token).unwrap().tapped = false;
    engine
        .apply_command(0, &activate_ability(token, 1, vec![]))
        .unwrap();
    let mut copy = engine.state.stack.last().unwrap().clone();
    copy.id = engine.state.next_object_id;
    engine.state.next_object_id += 1;
    copy.is_copy = true;
    engine.state.stack.push(copy);
    resolve_top(&mut engine);
    assert_eq!(engine.state.objects[&creature].zone, Zone::Battlefield);
    resolve_top(&mut engine);
    assert!(engine.state.stack.is_empty());

    let permanent_copy = inject_permanent_on_battlefield(&mut engine, 0, "clone");
    engine
        .state
        .objects
        .get_mut(&permanent_copy)
        .unwrap()
        .copiable_values = Some(CopiableValues {
        source_card_id: "ghost_vacuum".into(),
        source_face_index: 0,
        face: vacuum.primary_face().clone(),
        room_faces: None,
        display_name: "Ghost Vacuum".into(),
    });
    let second_creature = inject_graveyard_card(&mut engine, 1, "hill_giant");
    exile_with(&mut engine, permanent_copy, second_creature);
    release_with(&mut engine, permanent_copy);
    assert_eq!(
        engine.state.objects[&second_creature].zone,
        Zone::Battlefield
    );
}

#[test]
fn issue_234_release_is_sorcery_speed_and_stale_target_records_nothing() {
    let mut engine = setup();
    let vacuum = inject_permanent_on_battlefield(&mut engine, 0, "ghost_vacuum");
    let creature = inject_graveyard_card(&mut engine, 1, "grizzly_bears");
    engine
        .apply_command(0, &activate_ability(vacuum, 0, target_object(creature)))
        .unwrap();
    let owner = engine.state.player_idx(1).unwrap();
    engine.state.players[owner]
        .graveyard
        .retain(|oid| *oid != creature);
    engine.state.players[owner].hand.push(creature);
    engine.state.objects.get_mut(&creature).unwrap().zone = Zone::Hand;
    engine.state.zone_change_generation.insert(creature, 1);
    resolve_top(&mut engine);
    engine.state.objects.get_mut(&vacuum).unwrap().tapped = false;
    release_with(&mut engine, vacuum);
    assert_eq!(engine.state.objects[&creature].zone, Zone::Hand);

    let second_vacuum = inject_permanent_on_battlefield(&mut engine, 0, "ghost_vacuum");
    engine.apply_command(0, &primitive_yield()).unwrap();
    assert_ne!(engine.state.turn_step, TurnStep::Main1);
    let before = engine.diagnostic_snapshot().unwrap();
    assert!(engine
        .apply_command(0, &activate_ability(second_vacuum, 1, vec![]))
        .is_err());
    assert_eq!(engine.diagnostic_snapshot().unwrap(), before);
}

#[test]
fn issue_234_serialized_commands_replay_linked_exile_identically() {
    let mut original = setup();
    let mut commands = Vec::new();
    let mut batches = Vec::new();
    apply_serialized_and_record(
        &mut original,
        &mut commands,
        &mut batches,
        0,
        dev_put(0, "Ghost Vacuum", DevZone::Battlefield, true),
    );
    apply_serialized_and_record(
        &mut original,
        &mut commands,
        &mut batches,
        0,
        dev_put(1, "Grizzly Bears", DevZone::Hand, false),
    );
    apply_serialized_and_record(
        &mut original,
        &mut commands,
        &mut batches,
        0,
        dev_move(1, "Grizzly Bears", DevZone::Graveyard),
    );
    let vacuum = battlefield_object_for_card(&original, 0, "ghost_vacuum");
    let creature = original.state.players[1].graveyard[0];
    let activate_exile = activate_ability_for(&original, vacuum, 0, target_object(creature));
    apply_serialized_and_record(
        &mut original,
        &mut commands,
        &mut batches,
        0,
        activate_exile,
    );
    for _ in 0..2 {
        let actor = original.state.priority_player_id();
        apply_serialized_and_record(&mut original, &mut commands, &mut batches, actor, pass());
    }
    assert_eq!(original.state.objects[&creature].zone, Zone::Exile);

    // Keep both hands below maximum size so the recorded pass sequence can advance
    // through cleanup without introducing an unrelated discard choice.
    for owner in [0, 0, 1, 1] {
        apply_serialized_and_record(
            &mut original,
            &mut commands,
            &mut batches,
            0,
            dev_move(owner, "Forest", DevZone::Graveyard),
        );
    }

    let mut saw_opponent_turn = false;
    for _ in 0..64 {
        saw_opponent_turn |= original.state.active_player_id() == 1;
        if saw_opponent_turn
            && original.state.active_player_id() == 0
            && original.state.turn_step == TurnStep::Main1
        {
            break;
        }
        let actor = original.state.priority_player_id();
        apply_serialized_and_record(&mut original, &mut commands, &mut batches, actor, pass());
    }
    assert!(saw_opponent_turn);
    assert_eq!(original.state.active_player_id(), 0);
    assert_eq!(original.state.turn_step, TurnStep::Main1);
    apply_serialized_and_record(
        &mut original,
        &mut commands,
        &mut batches,
        0,
        dev_add_colorless(0, 6),
    );
    let activate_release = activate_ability_for(&original, vacuum, 1, vec![]);
    apply_serialized_and_record(
        &mut original,
        &mut commands,
        &mut batches,
        0,
        activate_release,
    );
    for _ in 0..2 {
        let actor = original.state.priority_player_id();
        apply_serialized_and_record(&mut original, &mut commands, &mut batches, actor, pass());
    }
    assert_eq!(original.state.objects[&creature].zone, Zone::Battlefield);

    let mut replay = setup();
    for ((actor, command), expected) in commands.iter().zip(&batches) {
        assert_eq!(&replay.apply_command(*actor, command).unwrap(), expected);
    }
    assert_eq!(
        replay.diagnostic_snapshot().unwrap(),
        original.diagnostic_snapshot().unwrap()
    );
}
