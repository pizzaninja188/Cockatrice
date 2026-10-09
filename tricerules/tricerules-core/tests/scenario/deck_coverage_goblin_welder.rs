//! Exact Goblin Welder's two-target simultaneous sacrifice and return.
use super::helpers::*;
use tricerules_cards::CounterKind;
use tricerules_core::Zone;

fn game(seed: u64) -> GameEngine {
    let deck = deck_with("mountain", &[]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1, 2],
        20,
        Some(vec![deck; 3]),
        true,
    )
    .unwrap();
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn resolve_one(engine: &mut GameEngine) -> RuledEventBatch {
    let count = engine
        .state
        .players
        .iter()
        .filter(|player| !player.has_lost)
        .count()
        - engine.state.passes_since_stack_change as usize;
    let mut batch = RuledEventBatch::default();
    for _ in 0..count {
        batch = engine
            .apply_command(engine.state.priority_player_id(), &pass())
            .unwrap();
    }
    batch
}

fn exchange(engine: &GameEngine, source: u32, departure: u32, incoming: u32) -> RuledCommand {
    activate_ability_for(
        engine,
        source,
        0,
        vec![
            TargetRef {
                kind: TargetRefKind::Permanent as i32,
                object_id: departure,
                group_index: 0,
                ..Default::default()
            },
            TargetRef {
                kind: TargetRefKind::Graveyard as i32,
                object_id: incoming,
                group_index: 1,
                ..Default::default()
            },
        ],
    )
}

fn rejected_unchanged(engine: &mut GameEngine, actor: i32, command: &RuledCommand) {
    let before = format!("{:?}", engine.state);
    assert!(engine.apply_command(actor, command).is_err());
    assert_eq!(format!("{:?}", engine.state), before);
}

#[test]
fn welder_actual_paid_cast_and_tap_cost_preserve_both_targets_until_resolution() {
    assert!(tricerules_cards::registry::global()
        .get("goblin_welder")
        .is_some());
    let mut engine = game(2026100521);
    let source = inject_card_into_hand(&mut engine, 0, "goblin_welder");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "goblin_welder");
    engine.apply_command(0, &cast_spell(slot, vec![])).unwrap();
    resolve_one(&mut engine);
    let values = engine.characteristics(source).unwrap();
    assert_eq!((values.power, values.toughness), (Some(1), Some(1)));
    assert!(values.is_creature() && !values.is_artifact());
    assert_eq!(engine.state.players[0].mana_pool.red, 0);
    let departure = inject_permanent_on_battlefield(&mut engine, 0, "sol_ring");
    let incoming = inject_graveyard_card(&mut engine, 0, "chromatic_lantern");
    let command = exchange(&engine, source, departure, incoming);
    rejected_unchanged(&mut engine, 0, &command); // Freshly cast: summoning sickness.
    engine
        .state
        .objects
        .get_mut(&source)
        .unwrap()
        .summoning_sick = false;
    rejected_unchanged(&mut engine, 1, &command);
    engine.apply_command(0, &command).unwrap();
    assert!(engine.state.objects[&source].tapped);
    assert_eq!(engine.state.objects[&departure].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&incoming].zone, Zone::Graveyard);
    resolve_one(&mut engine);
    assert_eq!(engine.state.objects[&departure].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&incoming].zone, Zone::Battlefield);
    assert!(!engine.state.objects[&incoming].tapped);
    assert!(engine.state.stack.is_empty() && engine.state.pending_resolution.is_none());
}

#[test]
fn welder_own_and_opponent_pairs_reject_crossed_targets_without_paying() {
    for player in 0..3 {
        let mut engine = game(2026100522 + player as u64);
        let source = inject_permanent_on_battlefield(&mut engine, 0, "goblin_welder");
        let departure = inject_permanent_on_battlefield(&mut engine, player, "sol_ring");
        let incoming = inject_graveyard_card(&mut engine, player, "chromatic_lantern");
        let crossed = inject_graveyard_card(&mut engine, (player + 1) % 3, "sol_ring");
        let wrong_type = inject_graveyard_card(&mut engine, player, "grizzly_bears");
        for invalid in [crossed, wrong_type, departure] {
            let command = exchange(&engine, source, departure, invalid);
            rejected_unchanged(&mut engine, 0, &command);
        }
        let command = exchange(&engine, source, departure, incoming);
        engine.state.objects.get_mut(&source).unwrap().tapped = true;
        rejected_unchanged(&mut engine, 0, &command);
        engine.state.objects.get_mut(&source).unwrap().tapped = false;
        engine.apply_command(0, &command).unwrap();
        resolve_one(&mut engine);
        assert_eq!(engine.state.objects[&departure].zone, Zone::Graveyard);
        assert_eq!(engine.state.objects[&incoming].zone, Zone::Battlefield);
        assert_eq!(engine.state.objects[&incoming].controller, player as i32);
        assert!(engine.state.players[player].battlefield.contains(&incoming));
        assert!(engine.state.players[player].graveyard.contains(&departure));
        assert!(engine.state.stack.is_empty());
    }
}

#[test]
fn welder_either_illegal_target_or_changed_controller_keeps_both_objects_and_paid_tap() {
    for invalidation in 0..6 {
        let mut engine = game(2026100525 + invalidation);
        let source = inject_permanent_on_battlefield(&mut engine, 0, "goblin_welder");
        let departure = inject_permanent_on_battlefield(&mut engine, 0, "sol_ring");
        let incoming = inject_graveyard_card(&mut engine, 0, "chromatic_lantern");
        engine
            .apply_command(0, &exchange(&engine, source, departure, incoming))
            .unwrap();
        // Synthetic mutations isolate each resolution-time check, including same-zone reincarnation.
        match invalidation {
            0 => {
                *engine
                    .state
                    .zone_change_generation
                    .entry(departure)
                    .or_default() += 2
            }
            1 => {
                *engine
                    .state
                    .zone_change_generation
                    .entry(incoming)
                    .or_default() += 2
            }
            2 => engine.state.objects.get_mut(&departure).unwrap().card_id = "mountain".into(),
            3 => engine.state.objects.get_mut(&incoming).unwrap().card_id = "grizzly_bears".into(),
            4 => {
                let object = engine.state.objects.get_mut(&departure).unwrap();
                object.controller = 1;
                object.base_controller = 1;
                engine.state.players[0]
                    .battlefield
                    .retain(|oid| *oid != departure);
                engine.state.players[1].battlefield.push(departure);
            }
            _ => {
                *engine
                    .state
                    .zone_change_generation
                    .entry(departure)
                    .or_default() += 2;
                *engine
                    .state
                    .zone_change_generation
                    .entry(incoming)
                    .or_default() += 2;
            }
        }
        resolve_one(&mut engine);
        assert_eq!(engine.state.objects[&departure].zone, Zone::Battlefield);
        assert_eq!(engine.state.objects[&incoming].zone, Zone::Graveyard);
        assert!(engine.state.objects[&source].tapped);
        assert!(engine.state.stack.is_empty());
    }
}

#[test]
fn welder_foreign_owned_departure_and_artifact_source_follow_current_controller() {
    for self_target in [false, true] {
        let mut engine = game(2026100532 + u64::from(self_target));
        let source = inject_permanent_on_battlefield(&mut engine, 0, "goblin_welder");
        let incoming = inject_graveyard_card(&mut engine, 0, "chromatic_lantern");
        let departure = if self_target {
            let torque = inject_permanent_on_battlefield(&mut engine, 0, "liquimetal_torque");
            apply_ability(&mut engine, 0, torque, 1, target_object(source)).unwrap();
            resolve_one(&mut engine);
            assert!(engine.characteristics(source).unwrap().is_artifact());
            source
        } else {
            let object = inject_permanent_on_battlefield(&mut engine, 1, "sol_ring");
            engine.state.players[1]
                .battlefield
                .retain(|oid| *oid != object);
            engine.state.players[0].battlefield.push(object);
            engine
                .state
                .objects
                .get_mut(&object)
                .unwrap()
                .base_controller = 0;
            engine.state.objects.get_mut(&object).unwrap().controller = 0;
            object
        };
        engine
            .apply_command(0, &exchange(&engine, source, departure, incoming))
            .unwrap();
        resolve_one(&mut engine);
        let owner = usize::from(!self_target);
        assert!(engine.state.players[owner].graveyard.contains(&departure));
        assert_eq!(engine.state.objects[&incoming].controller, 0);
        assert_eq!(engine.state.objects[&incoming].zone, Zone::Battlefield);
        assert!(engine.state.stack.is_empty());
    }
}

#[test]
fn welder_outgoing_orb_applies_before_departure_and_finality_does_not_prevent_return() {
    for finality in [false, true] {
        let mut engine = game(2026100534 + u64::from(finality));
        let source = inject_permanent_on_battlefield(&mut engine, 0, "goblin_welder");
        let departure = inject_permanent_on_battlefield(&mut engine, 0, "orb_of_dreams");
        let incoming = inject_graveyard_card(&mut engine, 0, "astral_cornucopia");
        if finality {
            engine
                .state
                .objects
                .get_mut(&departure)
                .unwrap()
                .counters
                .insert(CounterKind::Finality, 1);
        }
        engine
            .apply_command(0, &exchange(&engine, source, departure, incoming))
            .unwrap();
        resolve_one(&mut engine);
        for _ in 0..4 {
            let Some(pending) = engine.state.pending_resolution.as_ref() else {
                break;
            };
            assert_eq!(
                pending.presentation.choice_kind,
                ChoiceKind::ReplacementEffect
            );
            let application = pending.presentation.candidates[0];
            assert_eq!(engine.state.objects[&departure].zone, Zone::Battlefield);
            assert_eq!(engine.state.objects[&incoming].zone, Zone::Graveyard);
            engine
                .apply_command(0, &submit_resolution_choice(vec![application]))
                .unwrap();
        }
        assert_eq!(
            engine.state.objects[&departure].zone,
            if finality {
                Zone::Exile
            } else {
                Zone::Graveyard
            }
        );
        assert_eq!(engine.state.objects[&incoming].zone, Zone::Battlefield);
        assert!(engine.state.objects[&incoming].tapped);
        assert_eq!(
            engine.state.objects[&incoming].counter_count(CounterKind::Charge),
            0
        );
        assert!(engine.state.stack.is_empty());
    }
}

fn assert_aborted_copy_restored(aura_copy: bool, chained: bool) {
    let mut engine = game(2026100540 + u64::from(aura_copy));
    let source = inject_permanent_on_battlefield(&mut engine, 0, "goblin_welder");
    let departure = inject_permanent_on_battlefield(&mut engine, 2, "sol_ring");
    engine.state.players[2]
        .battlefield
        .retain(|oid| *oid != departure);
    engine.state.players[0].battlefield.push(departure);
    engine
        .state
        .objects
        .get_mut(&departure)
        .unwrap()
        .base_controller = 0;
    engine.state.objects.get_mut(&departure).unwrap().controller = 0;
    let copy_source = inject_permanent_on_battlefield(
        &mut engine,
        0,
        if aura_copy {
            "confiscate"
        } else {
            "steam_vents"
        },
    );
    if aura_copy {
        engine
            .state
            .objects
            .get_mut(&copy_source)
            .unwrap()
            .attached_to = Some(AttachmentRecipient::Object(source));
    }
    let coating = inject_permanent_on_battlefield(&mut engine, 0, "liquimetal_coating");
    apply_ability(&mut engine, 0, coating, 0, target_object(copy_source)).unwrap();
    resolve_one(&mut engine);
    let intermediate = if chained {
        inject_card_into_hand(&mut engine, 0, "glorious_anthem");
        move_ready_to_battlefield(&mut engine, 0, "glorious_anthem");
        let metamorph = inject_permanent_on_battlefield(&mut engine, 0, "phyrexian_metamorph");
        assert_eq!(engine.effective_toughness(metamorph), Some(1));
        Some(metamorph)
    } else {
        None
    };
    let incoming = inject_graveyard_card(&mut engine, 0, "sculpting_steel");
    let original_revision = engine.state.objects[&incoming].copy_revision;
    let original_generation = engine
        .state
        .zone_change_generation
        .get(&incoming)
        .copied()
        .unwrap_or(0);
    engine
        .apply_command(0, &exchange(&engine, source, departure, incoming))
        .unwrap();
    resolve_one(&mut engine);
    if let Some(intermediate) = intermediate {
        engine
            .apply_command(0, &submit_resolution_choice(vec![intermediate]))
            .unwrap();
        assert!(
            engine.state.pending_resolution.is_some(),
            "copied Metamorph supplies a later copy replacement"
        );
        assert_eq!(
            engine.state.objects[&incoming].copy_revision,
            original_revision + 1
        );
    }
    engine
        .apply_command(0, &submit_resolution_choice(vec![copy_source]))
        .unwrap();
    assert!(engine.state.pending_resolution.is_some());
    assert_eq!(engine.state.objects[&incoming].zone, Zone::Graveyard);
    assert!(engine.state.objects[&incoming].copiable_values.is_some());
    engine.apply_command(2, &concede()).unwrap();
    assert!(!engine.state.objects.contains_key(&departure));
    assert_eq!(engine.state.objects[&incoming].zone, Zone::Graveyard);
    assert!(engine.state.objects[&incoming].copiable_values.is_none());
    assert_eq!(
        engine.state.objects[&incoming].copy_revision,
        original_revision
    );
    assert_eq!(
        engine
            .state
            .zone_change_generation
            .get(&incoming)
            .copied()
            .unwrap_or(0),
        original_generation
    );
    assert!(engine
        .characteristics(incoming)
        .unwrap()
        .names
        .iter()
        .any(|name| name == "Sculpting Steel"));
    assert!(engine.state.pending_resolution.is_none() && engine.state.stack.is_empty());
    // A later real Welder activation must still offer Steel's own entry-copy choice.
    let next_source = inject_permanent_on_battlefield(&mut engine, 0, "goblin_welder");
    let next_departure = inject_permanent_on_battlefield(&mut engine, 0, "sol_ring");
    engine
        .apply_command(0, &exchange(&engine, next_source, next_departure, incoming))
        .unwrap();
    resolve_one(&mut engine);
    let pending = engine.state.pending_resolution.as_ref().unwrap();
    assert!(pending.presentation.candidates.contains(&next_departure));
    engine
        .apply_command(0, &submit_resolution_choice(vec![next_departure]))
        .unwrap();
    assert_eq!(engine.state.objects[&incoming].zone, Zone::Battlefield);
    assert!(engine
        .characteristics(incoming)
        .unwrap()
        .names
        .iter()
        .any(|name| name == "Sol Ring"));
    assert!(engine.state.pending_resolution.is_none() && engine.state.stack.is_empty());
}

#[test]
fn welder_aborted_aura_choice_restores_surviving_provisional_copy() {
    assert_aborted_copy_restored(true, false);
}

#[test]
fn welder_aborted_entry_cost_restores_surviving_provisional_copy() {
    assert_aborted_copy_restored(false, false);
}

#[test]
fn welder_aborted_copy_chain_restores_original_graveyard_identity() {
    assert_aborted_copy_restored(false, true);
}

#[test]
fn welder_copied_battle_rejects_departed_protector_then_commits_with_live_opponent() {
    let mut engine = game(2026100542);
    let source = inject_permanent_on_battlefield(&mut engine, 0, "goblin_welder");
    let departure = inject_permanent_on_battlefield(&mut engine, 0, "sol_ring");
    let battle = inject_permanent_on_battlefield(
        &mut engine,
        0,
        "invasion_of_ulgrotha_grandmother_ravi_sengir",
    );
    engine.state.battle_protectors.insert(battle, 1);
    engine
        .state
        .objects
        .get_mut(&battle)
        .unwrap()
        .counters
        .insert(CounterKind::Defense, 5);
    let torque = inject_permanent_on_battlefield(&mut engine, 0, "liquimetal_torque");
    apply_ability(&mut engine, 0, torque, 1, target_object(battle)).unwrap();
    resolve_one(&mut engine);
    let incoming = inject_graveyard_card(&mut engine, 0, "sculpting_steel");
    engine
        .apply_command(0, &exchange(&engine, source, departure, incoming))
        .unwrap();
    resolve_one(&mut engine);
    engine
        .apply_command(0, &submit_resolution_choice(vec![battle]))
        .unwrap();
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .presentation
            .choice_kind,
        ChoiceKind::BattleProtector
    );
    engine.apply_command(2, &concede()).unwrap();
    assert!(!engine
        .state
        .pending_resolution
        .as_ref()
        .unwrap()
        .presentation
        .candidates
        .contains(&2));
    rejected_unchanged(&mut engine, 0, &submit_resolution_choice(vec![2]));
    assert_eq!(engine.state.objects[&departure].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&incoming].zone, Zone::Graveyard);
    engine
        .apply_command(0, &submit_resolution_choice(vec![1]))
        .unwrap();
    assert_eq!(engine.state.objects[&departure].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&incoming].zone, Zone::Battlefield);
    assert_eq!(engine.state.battle_protectors[&incoming], 1);
    assert_eq!(
        engine.state.objects[&incoming].counter_count(CounterKind::Defense),
        5
    );
    assert!(engine.state.pending_resolution.is_none());
}

#[test]
fn welder_outgoing_copy_source_remains_unmoved_through_logged_choice() {
    let mut engine = game(2026100536);
    let source = inject_permanent_on_battlefield(&mut engine, 0, "goblin_welder");
    let departure = inject_permanent_on_battlefield(&mut engine, 1, "orb_of_dreams");
    let incoming = inject_graveyard_card(&mut engine, 1, "sculpting_steel");
    engine
        .apply_command(0, &exchange(&engine, source, departure, incoming))
        .unwrap();
    resolve_one(&mut engine);
    let pending = engine.state.pending_resolution.as_ref().unwrap();
    assert_eq!(pending.deciding_player, 1);
    assert!(pending.presentation.candidates.contains(&departure));
    assert_eq!(engine.state.objects[&departure].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&incoming].zone, Zone::Graveyard);
    rejected_unchanged(&mut engine, 0, &submit_resolution_choice(vec![departure]));
    rejected_unchanged(&mut engine, 1, &submit_resolution_choice(vec![source]));
    engine
        .apply_command(1, &submit_resolution_choice(vec![departure]))
        .unwrap();
    assert_eq!(engine.state.objects[&departure].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&incoming].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&incoming].controller, 1);
    assert!(engine
        .characteristics(incoming)
        .unwrap()
        .names
        .iter()
        .any(|name| name == "Orb of Dreams"));
    assert!(engine.state.objects[&incoming].tapped);
    assert!(engine.state.pending_resolution.is_none() && engine.state.stack.is_empty());
}

#[test]
fn welder_copy_choice_stale_member_and_player_concession_never_sacrifice_a_survivor() {
    for concession in [false, true] {
        let mut engine = game(2026100537 + u64::from(concession));
        let source = inject_permanent_on_battlefield(&mut engine, 0, "goblin_welder");
        let departure = inject_permanent_on_battlefield(&mut engine, 1, "sol_ring");
        let incoming = inject_graveyard_card(&mut engine, 1, "sculpting_steel");
        engine
            .apply_command(0, &exchange(&engine, source, departure, incoming))
            .unwrap();
        resolve_one(&mut engine);
        assert!(engine.state.pending_resolution.is_some());
        if concession {
            engine.apply_command(1, &concede()).unwrap();
            assert!(!engine.state.objects.contains_key(&departure));
            assert!(!engine.state.objects.contains_key(&incoming));
            assert_eq!(engine.state.objects[&source].zone, Zone::Battlefield);
            assert!(engine.state.pending_resolution.is_none() && engine.state.stack.is_empty());
        } else {
            *engine
                .state
                .zone_change_generation
                .entry(departure)
                .or_default() += 2;
            rejected_unchanged(&mut engine, 1, &submit_resolution_choice(vec![departure]));
            assert_eq!(engine.state.objects[&departure].zone, Zone::Battlefield);
            assert_eq!(engine.state.objects[&incoming].zone, Zone::Graveyard);
        }
    }
}

#[test]
fn welder_copied_aura_chooses_departing_incarnation_then_enters_before_separate_sba() {
    let mut engine = game(2026100539);
    let source = inject_permanent_on_battlefield(&mut engine, 0, "goblin_welder");
    let departure = inject_permanent_on_battlefield(&mut engine, 0, "sol_ring");
    let aura = inject_permanent_on_battlefield(&mut engine, 0, "confiscate");
    engine.state.objects.get_mut(&aura).unwrap().attached_to =
        Some(AttachmentRecipient::Object(departure));
    let torque = inject_permanent_on_battlefield(&mut engine, 0, "liquimetal_torque");
    apply_ability(&mut engine, 0, torque, 1, target_object(aura)).unwrap();
    resolve_one(&mut engine);
    assert!(engine.characteristics(aura).unwrap().is_artifact());
    let incoming = inject_graveyard_card(&mut engine, 0, "sculpting_steel");
    let generation = engine
        .state
        .zone_change_generation
        .get(&incoming)
        .copied()
        .unwrap_or(0);
    engine
        .apply_command(0, &exchange(&engine, source, departure, incoming))
        .unwrap();
    resolve_one(&mut engine);
    engine
        .apply_command(0, &submit_resolution_choice(vec![aura]))
        .unwrap();
    let pending = engine.state.pending_resolution.as_ref().unwrap();
    assert_eq!(pending.presentation.choice_kind, ChoiceKind::AuraPermanent);
    assert!(pending.presentation.candidates.contains(&departure));
    assert_eq!(engine.state.objects[&departure].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&incoming].zone, Zone::Graveyard);
    let batch = engine
        .apply_command(0, &submit_resolution_choice(vec![departure]))
        .unwrap();
    assert_eq!(engine.state.objects[&departure].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&incoming].zone, Zone::Graveyard);
    assert_eq!(
        engine.state.zone_change_generation[&incoming],
        generation + 2
    );
    let moves: Vec<_> = batch
        .events
        .iter()
        .filter_map(|event| match &event.ev {
            Some(Ev::PermanentMoved(moved)) if moved.object_id == incoming => {
                Some(moved.destination)
            }
            _ => None,
        })
        .collect();
    assert_eq!(
        moves,
        vec![
            tricerules_proto::ruled::v1::permanent_moved::Destination::Battlefield as i32,
            tricerules_proto::ruled::v1::permanent_moved::Destination::Graveyard as i32
        ]
    );
    assert!(engine.state.pending_resolution.is_none() && engine.state.stack.is_empty());
}

#[test]
fn welder_copied_aura_keeps_a_surviving_recipient_attached() {
    let mut engine = game(2026100550);
    let source = inject_permanent_on_battlefield(&mut engine, 0, "goblin_welder");
    let departure = inject_permanent_on_battlefield(&mut engine, 0, "sol_ring");
    let aura = inject_permanent_on_battlefield(&mut engine, 0, "confiscate");
    engine.state.objects.get_mut(&aura).unwrap().attached_to =
        Some(AttachmentRecipient::Object(departure));
    let torque = inject_permanent_on_battlefield(&mut engine, 0, "liquimetal_torque");
    apply_ability(&mut engine, 0, torque, 1, target_object(aura)).unwrap();
    resolve_one(&mut engine);
    let incoming = inject_graveyard_card(&mut engine, 0, "sculpting_steel");
    let generation = engine
        .state
        .zone_change_generation
        .get(&incoming)
        .copied()
        .unwrap_or(0);
    engine
        .apply_command(0, &exchange(&engine, source, departure, incoming))
        .unwrap();
    resolve_one(&mut engine);
    engine
        .apply_command(0, &submit_resolution_choice(vec![aura]))
        .unwrap();
    assert_eq!(engine.state.objects[&departure].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&incoming].zone, Zone::Graveyard);
    engine
        .apply_command(0, &submit_resolution_choice(vec![torque]))
        .unwrap();
    assert_eq!(engine.state.objects[&departure].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&incoming].zone, Zone::Battlefield);
    assert_eq!(
        engine.state.objects[&incoming].attached_to,
        Some(AttachmentRecipient::Object(torque))
    );
    assert_eq!(
        engine.state.zone_change_generation[&incoming],
        generation + 1
    );
    assert!(engine
        .characteristics(incoming)
        .unwrap()
        .names
        .contains(&"Confiscate".to_string()));
    assert!(engine.state.pending_resolution.is_none() && engine.state.stack.is_empty());
}

#[test]
fn welder_copied_aura_without_a_recipient_still_sacrifices_the_legal_departure() {
    let mut engine = game(2026100551);
    let source = inject_permanent_on_battlefield(&mut engine, 0, "goblin_welder");
    let departure = inject_permanent_on_battlefield(&mut engine, 0, "sol_ring");
    let land = inject_permanent_on_battlefield(&mut engine, 2, "mountain");
    let aura = inject_permanent_on_battlefield(&mut engine, 0, "gift_of_paradise");
    engine.state.objects.get_mut(&aura).unwrap().attached_to =
        Some(AttachmentRecipient::Object(land));
    let coating = inject_permanent_on_battlefield(&mut engine, 0, "liquimetal_coating");
    apply_ability(&mut engine, 0, coating, 0, target_object(aura)).unwrap();
    resolve_one(&mut engine);
    let incoming = inject_graveyard_card(&mut engine, 0, "sculpting_steel");
    let generation = engine
        .state
        .zone_change_generation
        .get(&incoming)
        .copied()
        .unwrap_or(0);
    engine
        .apply_command(0, &exchange(&engine, source, departure, incoming))
        .unwrap();
    resolve_one(&mut engine);
    // The land owner's real concession removes the sole land while this copy choice is parked.
    // The Aura belongs to the surviving player; pending-resolution SBA deferral keeps it as a source.
    engine.apply_command(2, &concede()).unwrap();
    assert!(!engine.state.objects.contains_key(&land));
    assert_eq!(engine.state.objects[&aura].zone, Zone::Battlefield);
    let batch = engine
        .apply_command(0, &submit_resolution_choice(vec![aura]))
        .unwrap();
    assert_eq!(engine.state.objects[&departure].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&incoming].zone, Zone::Graveyard);
    assert_eq!(
        engine
            .state
            .zone_change_generation
            .get(&incoming)
            .copied()
            .unwrap_or(0),
        generation
    );
    assert!(engine.state.objects[&incoming].copiable_values.is_none());
    assert_eq!(engine.state.objects[&incoming].copy_revision, 0);
    assert!(engine.state.objects[&source].tapped);
    assert!(!batch.events.iter().any(|event| matches!(&event.ev,
        Some(Ev::PermanentMoved(moved)) if moved.object_id == incoming)));
    assert!(engine.state.pending_resolution.is_none() && engine.state.stack.is_empty());
}

#[test]
fn welder_return_keeps_intrinsic_entry_counters_and_departing_warden_does_not_see_entry() {
    let mut engine = game(2026100543);
    let source = inject_permanent_on_battlefield(&mut engine, 0, "goblin_welder");
    let outgoing = inject_permanent_on_battlefield(&mut engine, 0, "soul_warden");
    let surviving = inject_permanent_on_battlefield(&mut engine, 0, "soul_warden");
    let torque = inject_permanent_on_battlefield(&mut engine, 0, "liquimetal_torque");
    apply_ability(&mut engine, 0, torque, 1, target_object(outgoing)).unwrap();
    resolve_one(&mut engine);
    let incoming = inject_graveyard_card(&mut engine, 0, "solemn_simulacrum");
    let life = engine.state.players[0].life;
    engine
        .apply_command(0, &exchange(&engine, source, outgoing, incoming))
        .unwrap();
    resolve_one(&mut engine);
    answer_trigger_order_in_engine_order(&mut engine);
    let sources: Vec<_> = engine
        .state
        .stack
        .iter()
        .filter_map(|item| item.source_permanent_id)
        .collect();
    assert_eq!(sources.len(), 2);
    assert_eq!(sources.iter().filter(|&&oid| oid == surviving).count(), 1);
    assert_eq!(sources.iter().filter(|&&oid| oid == incoming).count(), 1);
    assert!(!sources.contains(&outgoing));
    for _ in 0..3 {
        if engine.state.stack.is_empty() {
            break;
        }
        resolve_one(&mut engine);
        if engine.state.pending_resolution.is_some() {
            engine
                .apply_command(
                    0,
                    &submit_resolution_decision(
                        tricerules_proto::ruled::v1::ResolutionChoiceDecision::Decline,
                    ),
                )
                .unwrap();
        }
    }
    assert_eq!(engine.state.players[0].life, life + 1);
    assert!(engine.state.stack.is_empty() && engine.state.pending_resolution.is_none());
    let next_source = inject_permanent_on_battlefield(&mut engine, 0, "goblin_welder");
    let departure = inject_permanent_on_battlefield(&mut engine, 0, "sol_ring");
    let counter_artifact = inject_graveyard_card(&mut engine, 0, "pentavus");
    engine
        .apply_command(
            0,
            &exchange(&engine, next_source, departure, counter_artifact),
        )
        .unwrap();
    resolve_one(&mut engine);
    assert_eq!(
        engine.state.objects[&counter_artifact].zone,
        Zone::Battlefield
    );
    assert_eq!(
        engine.state.objects[&counter_artifact].counter_count(CounterKind::PlusOnePlusOne),
        5
    );
    assert_eq!(
        engine.characteristics(counter_artifact).unwrap().toughness,
        Some(5)
    );
    resolve_one(&mut engine); // Surviving Warden's second entry trigger.
    assert_eq!(engine.state.players[0].life, life + 2);
    assert!(engine.state.stack.is_empty());
}

#[test]
fn welder_real_zone_change_invalidates_target_but_source_departure_preserves_ability() {
    use tricerules_proto::ruled::v1::{dev_command, DevCommand, DevMoveCard, DevZone};
    for source_leaves in [false, true] {
        let mut engine = game(2026100544 + u64::from(source_leaves));
        let source = inject_permanent_on_battlefield(&mut engine, 0, "goblin_welder");
        let departure = inject_permanent_on_battlefield(&mut engine, 0, "sol_ring");
        let incoming = inject_graveyard_card(&mut engine, 0, "chromatic_lantern");
        engine
            .apply_command(0, &exchange(&engine, source, departure, incoming))
            .unwrap();
        engine.enable_dev_commands();
        engine
            .apply_command(
                0,
                &RuledCommand {
                    cmd: Some(Cmd::DevCommand(DevCommand {
                        target_player_id: 0,
                        dev: Some(dev_command::Dev::MoveCard(DevMoveCard {
                            card_name: if source_leaves {
                                "Goblin Welder"
                            } else {
                                "Sol Ring"
                            }
                            .into(),
                            zone: DevZone::Graveyard as i32,
                            ready: false,
                        })),
                    })),
                },
            )
            .unwrap();
        resolve_one(&mut engine);
        assert_eq!(engine.state.objects[&departure].zone, Zone::Graveyard);
        assert_eq!(
            engine.state.objects[&incoming].zone,
            if source_leaves {
                Zone::Battlefield
            } else {
                Zone::Graveyard
            }
        );
        assert!(engine.state.stack.is_empty());
    }
}

#[test]
fn welder_accepted_copy_commands_replay_identical_batches_and_incarnations() {
    use prost::Message;
    // Deterministic explicit fixture, followed by actual accepted protobuf commands.
    // Fixture injection is not claimed as a native session command journal.
    let create = || {
        let mut engine = game(2026100546);
        let source = inject_permanent_on_battlefield(&mut engine, 0, "goblin_welder");
        let departure = inject_permanent_on_battlefield(&mut engine, 1, "sol_ring");
        let incoming = inject_graveyard_card(&mut engine, 1, "sculpting_steel");
        (engine, source, departure, incoming)
    };
    let (mut original, source, departure, incoming) = create();
    let mut commands = vec![(0, exchange(&original, source, departure, incoming))];
    let mut batches = vec![original
        .apply_command(commands[0].0, &commands[0].1)
        .unwrap()];
    for _ in 0..3 {
        let actor = original.state.priority_player_id();
        let command = pass();
        batches.push(original.apply_command(actor, &command).unwrap());
        commands.push((actor, command));
    }
    let command = submit_resolution_choice(vec![departure]);
    batches.push(original.apply_command(1, &command).unwrap());
    commands.push((1, command));
    assert!(original.state.pending_resolution.is_none() && original.state.stack.is_empty());
    let (mut replay, replay_source, replay_departure, replay_incoming) = create();
    assert_eq!(
        (source, departure, incoming),
        (replay_source, replay_departure, replay_incoming)
    );
    for ((actor, command), expected) in commands.iter().zip(batches) {
        let decoded = RuledCommand::decode(command.encode_to_vec().as_slice()).unwrap();
        assert_eq!(replay.apply_command(*actor, &decoded).unwrap(), expected);
    }
    assert_eq!(
        replay.diagnostic_snapshot().unwrap(),
        original.diagnostic_snapshot().unwrap()
    );
}
