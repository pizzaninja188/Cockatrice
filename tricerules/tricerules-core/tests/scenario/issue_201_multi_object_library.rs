use super::helpers::*;
use tricerules_cards::CardRegistry;
use tricerules_cards::CounterKind;
use tricerules_core::state::CopiableValues;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::{dev_command, DevCommand, DevMoveCard, DevZone};

fn engine_with_drowner(seed: u64) -> GameEngine {
    let decks = Some(vec![
        deck_with("island", &["floodpits_drowner", "grizzly_bears"]),
        deck_with("mountain", &["hill_giant", "grizzly_bears"]),
    ]);
    let mut engine = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("engine");
    engine.enable_dev_commands();
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn setup_activation(seed: u64, target_player: usize) -> (GameEngine, u32, u32, u32) {
    let mut engine = engine_with_drowner(seed);
    let source = relocate_to_battlefield(&mut engine, 0, "floodpits_drowner", false);
    let target_card = if target_player == 0 {
        "grizzly_bears"
    } else {
        "hill_giant"
    };
    let target = relocate_to_battlefield(&mut engine, target_player, target_card, false);
    let unstunned = relocate_to_battlefield(&mut engine, 1, "grizzly_bears", false);
    engine
        .state
        .objects
        .get_mut(&target)
        .expect("target")
        .set_counter(CounterKind::Stun, 1);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 2,
            ..Default::default()
        },
    );
    (engine, source, target, unstunned)
}

fn ability_targets(engine: &mut GameEngine, source: u32) -> Vec<u32> {
    let key = u64::from(source) << 32;
    engine.initial_response_batch().legal_by_player[&0].valid_targets_by_ability[&key].groups[0]
        .valid_permanent_ids
        .clone()
}

fn dev_move(engine: &mut GameEngine, player: i32, card_name: &str, zone: DevZone) {
    engine
        .apply_command(
            engine.state.priority_player_id(),
            &RuledCommand {
                cmd: Some(Cmd::DevCommand(DevCommand {
                    target_player_id: player,
                    dev: Some(dev_command::Dev::MoveCard(DevMoveCard {
                        card_name: card_name.into(),
                        zone: zone as i32,
                        ready: true,
                    })),
                })),
            },
        )
        .expect("dev move");
}

fn resolve_top(engine: &mut GameEngine) -> RuledEventBatch {
    let first = engine.state.priority_player_id();
    let second = if first == engine.state.players[0].id {
        engine.state.players[1].id
    } else {
        engine.state.players[0].id
    };
    engine.apply_command(first, &pass()).expect("first pass");
    engine
        .apply_command(second, &pass())
        .expect("second pass resolves")
}

fn shuffle_log_count(batch: &RuledEventBatch, player: i32) -> usize {
    let expected = format!("P{player} shuffles their library.");
    batch
        .events
        .iter()
        .filter(|event| matches!(&event.ev, Some(Ev::Log(log)) if log.text == expected))
        .count()
}

#[test]
fn issue_201_counter_filter_publishes_and_resolves_atomic_shuffle() {
    let (mut engine, source, target, unstunned) = setup_activation(201_001, 1);

    let published = ability_targets(&mut engine, source);
    assert!(published.contains(&target));
    assert!(!published.contains(&unstunned));

    let source_generation = engine
        .state
        .zone_change_generation
        .get(&source)
        .copied()
        .unwrap_or(0);
    let target_generation = engine
        .state
        .zone_change_generation
        .get(&target)
        .copied()
        .unwrap_or(0);
    apply_ability(&mut engine, 0, source, 0, target_object(target)).expect("activate");
    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(engine.state.objects[&source].zone, Zone::Library);
    assert_eq!(engine.state.objects[&target].zone, Zone::Library);
    assert!(engine.state.players[0].library.contains(&source));
    assert!(engine.state.players[1].library.contains(&target));
    assert_eq!(
        engine.state.zone_change_generation[&source],
        source_generation + 1
    );
    assert_eq!(
        engine.state.zone_change_generation[&target],
        target_generation + 1
    );
}

#[test]
fn issue_201_same_owner_and_duplicate_subject_shuffle_only_once() {
    let (mut same_owner, source, target, _) = setup_activation(201_002, 0);
    apply_ability(&mut same_owner, 0, source, 0, target_object(target)).expect("activate");
    let batch = resolve_top(&mut same_owner);
    assert_eq!(shuffle_log_count(&batch, 0), 1);

    let (mut duplicate, source, _, _) = setup_activation(201_003, 1);
    duplicate
        .state
        .objects
        .get_mut(&source)
        .unwrap()
        .set_counter(CounterKind::Stun, 1);
    let generation = duplicate
        .state
        .zone_change_generation
        .get(&source)
        .copied()
        .unwrap_or(0);
    apply_ability(&mut duplicate, 0, source, 0, target_object(source))
        .expect("activate targeting itself");
    let batch = resolve_top(&mut duplicate);
    assert_eq!(
        duplicate.state.zone_change_generation[&source],
        generation + 1
    );
    assert_eq!(shuffle_log_count(&batch, 0), 1);
    assert_eq!(
        batch
            .events
            .iter()
            .filter(|event| matches!(&event.ev, Some(Ev::PermanentMoved(moved)) if moved.object_id == source))
            .count(),
        1
    );
}

#[test]
fn issue_201_removed_counter_fizzles_the_entire_ability() {
    let (mut engine, source, target, _) = setup_activation(201_004, 1);
    apply_ability(&mut engine, 0, source, 0, target_object(target)).expect("activate");
    engine
        .state
        .objects
        .get_mut(&target)
        .unwrap()
        .set_counter(CounterKind::Stun, 0);

    let batch = resolve_top(&mut engine);
    assert_eq!(engine.state.objects[&source].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&target].zone, Zone::Battlefield);
    assert_eq!(shuffle_log_count(&batch, 0), 0);
    assert_eq!(shuffle_log_count(&batch, 1), 0);
}

#[test]
fn issue_201_departed_or_returned_source_uses_captured_owner_without_moving_new_object() {
    for (seed, returns) in [(201_005, false), (201_006, true)] {
        let (mut engine, source, target, _) = setup_activation(seed, 1);
        apply_ability(&mut engine, 0, source, 0, target_object(target)).expect("activate");
        dev_move(&mut engine, 0, "Floodpits Drowner", DevZone::Graveyard);
        if returns {
            dev_move(&mut engine, 0, "Floodpits Drowner", DevZone::Battlefield);
            engine
                .apply_command(
                    0,
                    &RuledCommand {
                        cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
                            targets: target_object(target),
                            ..Default::default()
                        })),
                    },
                )
                .expect("choose returned Drowner ETB target");
            resolve_top(&mut engine);
        }
        let returned_generation = engine
            .state
            .zone_change_generation
            .get(&source)
            .copied()
            .unwrap_or(0);

        let batch = resolve_top(&mut engine);
        assert_eq!(engine.state.objects[&target].zone, Zone::Library);
        assert_eq!(shuffle_log_count(&batch, 0), 1);
        assert_eq!(shuffle_log_count(&batch, 1), 1);
        if returns {
            assert_eq!(engine.state.objects[&source].zone, Zone::Battlefield);
            assert_eq!(
                engine.state.zone_change_generation[&source], returned_generation,
                "the new incarnation must not move"
            );
        } else {
            assert_eq!(engine.state.objects[&source].zone, Zone::Graveyard);
        }
    }
}

#[test]
fn issue_201_departed_token_source_still_shuffles_its_owners_library() {
    let (mut engine, source, target, _) = setup_activation(201_007, 1);
    let face = CardRegistry::global()
        .get("floodpits_drowner")
        .unwrap()
        .primary_face()
        .clone();
    engine.state.objects.get_mut(&source).unwrap().token_origin = Some(CopiableValues {
        source_card_id: "floodpits_drowner".into(),
        source_face_index: 0,
        face,
        room_faces: None,
        display_name: "Floodpits Drowner".into(),
    });
    apply_ability(&mut engine, 0, source, 0, target_object(target)).expect("activate");
    dev_move(&mut engine, 0, "Floodpits Drowner", DevZone::Graveyard);
    assert!(!engine.state.objects.contains_key(&source));

    let batch = resolve_top(&mut engine);
    assert_eq!(engine.state.objects[&target].zone, Zone::Library);
    assert_eq!(shuffle_log_count(&batch, 0), 1);
    assert_eq!(shuffle_log_count(&batch, 1), 1);
}

#[test]
fn issue_201_departures_share_one_trigger_batch_and_replay_is_deterministic() {
    let mut final_libraries = Vec::new();
    for seed in [201_008, 201_008] {
        let (mut engine, source, target, _) = setup_activation(seed, 1);
        apply_ability(&mut engine, 0, source, 0, target_object(target)).expect("activate");
        for object_id in [source, target] {
            engine.state.objects.get_mut(&object_id).unwrap().card_id =
                "featherbrained_filcher".into();
        }
        resolve_top(&mut engine);
        answer_trigger_order_in_engine_order(&mut engine);
        assert_eq!(
            engine.state.stack.len(),
            2,
            "both LTB triggers must be staged from the same departure snapshot"
        );
        final_libraries.push(
            engine
                .state
                .players
                .iter()
                .map(|player| player.library.iter().copied().collect::<Vec<_>>())
                .collect::<Vec<_>>(),
        );
    }
    assert_eq!(final_libraries[0], final_libraries[1]);
}
