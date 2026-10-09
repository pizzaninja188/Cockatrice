//! Exact Replicating Ring: source-backed resolution sequencing and named Snow tokens.
use super::helpers::*;
use tricerules_cards::primitives::*;
use tricerules_cards::{AbilityPresentation, ControllerReference, Layout};
use tricerules_core::{AffectedScope, ContinuousEffect, TurnStep, Zone};
use tricerules_proto::ruled::v1::{dev_command, DevCommand, DevMoveCard, DevZone};

const RING: &str = "replicating_ring";
const TOKEN: &str = "replicated_ring";

fn setup() -> (GameEngine, u32) {
    assert!(
        tricerules_cards::registry::global().get(RING).is_some(),
        "exact Replicating Ring is missing"
    );
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        507_100,
        &[0, 1],
        20,
        Some(vec![deck_with("island", &[]), deck_with("forest", &[])]),
        true,
    )
    .unwrap();
    advance_to_main1_from_game_start(&mut engine);
    let source = inject_card_into_hand(&mut engine, 0, RING);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 3,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, RING);
    engine.apply_command(0, &cast_spell(slot, vec![])).unwrap();
    assert_eq!(engine.state.players[0].mana_pool.colorless, 0);
    assert_eq!(engine.state.objects[&source].zone, Zone::Stack);
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&source].zone, Zone::Battlefield);
    (engine, source)
}

fn set_counters(engine: &mut GameEngine, source: u32, kind: CounterKind, count: u32) {
    engine
        .state
        .objects
        .get_mut(&source)
        .unwrap()
        .counters
        .insert(kind, count);
}

fn start_upkeep(engine: &mut GameEngine, player: i32) {
    // Turn-location fixture; the boundary and trigger collection use accepted commands.
    let previous = if player == 0 { 1 } else { 0 };
    engine.state.active_player_idx = previous;
    engine.state.priority_idx = previous;
    engine.state.turn_step = TurnStep::EndStep;
    engine.state.passes_since_stack_change = 0;
    engine.apply_command(previous as i32, &pass()).unwrap();
    engine.apply_command(player, &pass()).unwrap();
    assert_eq!(engine.state.active_player_id(), player);
    assert_eq!(engine.state.turn_step, TurnStep::Upkeep);
}

fn token_ids(engine: &GameEngine) -> Vec<u32> {
    engine
        .state
        .objects
        .values()
        .filter(|o| o.zone == Zone::Battlefield && o.card_id == TOKEN)
        .map(|o| o.id)
        .collect()
}

fn assert_tokens(engine: &GameEngine, count: usize, controller: i32) {
    let ids = token_ids(engine);
    assert_eq!(ids.len(), count);
    assert_eq!(
        ids.iter()
            .copied()
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        count
    );
    for id in ids {
        let object = &engine.state.objects[&id];
        assert_eq!((object.owner, object.controller), (controller, controller));
        assert!(object.token_origin.is_some());
        let face = engine.characteristics(id).unwrap();
        assert_eq!(face.names, ["Replicated Ring"]);
        assert!(face.has_type("Artifact") && face.supertypes.contains(&"Snow".to_string()));
        assert!(!face.has_type("Creature") && face.colors.is_empty());
        assert_eq!((face.power, face.toughness), (None, None));
    }
}

fn reject(engine: &mut GameEngine, actor: i32, command: &RuledCommand) {
    let before = engine.diagnostic_snapshot().unwrap();
    assert!(engine.apply_command(actor, command).is_err());
    assert_eq!(engine.diagnostic_snapshot().unwrap(), before);
}

#[test]
fn ring_paid_cast_and_exact_card_token_characteristics() {
    let (mut engine, source) = setup();
    for (id, name, triggered) in [(RING, "Replicating Ring", 1), (TOKEN, "Replicated Ring", 0)] {
        let card = tricerules_cards::registry::global().get(id).unwrap();
        assert_eq!(card.name, name);
        assert_eq!(card.layout, Layout::Normal);
        assert_eq!(card.face_count(), 1);
        let face = card.primary_face();
        assert_eq!(face.types, ["Artifact"]);
        assert_eq!(face.supertypes, ["Snow"]);
        assert!(face.colors().is_empty());
        assert_eq!((face.power, face.toughness), (None, None));
        assert_eq!(face.triggered_abilities.len(), triggered);
        assert_eq!(face.activated_abilities.len(), 1);
        assert_eq!(face.activated_abilities[0].costs, [AbilityCost::Tap]);
    }
    let face = tricerules_cards::registry::global()
        .get(RING)
        .unwrap()
        .primary_face();
    assert_eq!(face.mana_cost.to_string(), "{3}");
    assert_eq!(
        face.activated_abilities[0].presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    let trigger = &face.triggered_abilities[0];
    assert_eq!(
        trigger.presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    assert_eq!(
        trigger.trigger,
        TriggerCondition::AtBeginningOfUpkeep {
            player: CastTriggerPlayer::Controller
        }
    );
    assert!(trigger.intervening_if.is_none());
    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::Night),
        0
    );
    // Full cast admission rejects insufficient payment and another seat without state mutation.
    let second = inject_card_into_hand(&mut engine, 0, RING);
    let slot = hand_index_for_card(&engine, 0, RING);
    reject(&mut engine, 0, &cast_spell(slot, vec![]));
    reject(&mut engine, 1, &cast_spell(slot, vec![]));
    assert_eq!(engine.state.objects[&second].zone, Zone::Hand);
}

#[test]
fn ring_own_upkeep_threshold_removes_all_night_and_preserves_other_counters() {
    for (before, after, tokens) in [(0, 1, 0), (6, 7, 0), (7, 0, 8), (12, 0, 8)] {
        let (mut engine, source) = setup();
        set_counters(&mut engine, source, CounterKind::Night, before);
        set_counters(&mut engine, source, CounterKind::Quest, 3);
        start_upkeep(&mut engine, 1);
        assert!(
            engine.state.stack.is_empty(),
            "other player's upkeep does not trigger"
        );
        assert_eq!(
            engine.state.objects[&source].counter_count(CounterKind::Night),
            before
        );
        start_upkeep(&mut engine, 0);
        assert_eq!(engine.state.stack.len(), 1, "no intervening-if threshold");
        assert_eq!(engine.state.stack[0].source_permanent_id, Some(source));
        assert!(token_ids(&engine).is_empty());
        resolve_entire_stack_two_player(&mut engine);
        assert_eq!(
            engine.state.objects[&source].counter_count(CounterKind::Night),
            after
        );
        assert_eq!(
            engine.state.objects[&source].counter_count(CounterKind::Quest),
            3
        );
        assert_tokens(&engine, tokens, 0);
        let batch = engine.initial_response_batch();
        let annotation = batch
            .events
            .iter()
            .find_map(|event| match &event.ev {
                Some(Ev::ZoneView(view)) => view
                    .per_player
                    .iter()
                    .flat_map(|player| &player.battlefield_objects)
                    .find(|object| object.object_id == source)
                    .map(|object| &object.counters_annotation),
                _ => None,
            })
            .expect("public counter annotation");
        assert_eq!(annotation.contains("night"), after > 0);
    }
}

#[test]
fn ring_threshold_is_resolution_time_and_external_eighth_counter_is_not_an_event() {
    for (before, response, expected) in [(0, 7, 8), (7, 6, 0), (8, 0, 0)] {
        let (mut engine, source) = setup();
        set_counters(&mut engine, source, CounterKind::Night, before);
        assert!(engine.state.stack.is_empty() && token_ids(&engine).is_empty());
        start_upkeep(&mut engine, 0);
        set_counters(&mut engine, source, CounterKind::Night, response);
        resolve_entire_stack_two_player(&mut engine);
        assert_tokens(&engine, expected, 0);
    }
}

#[test]
fn ring_prevented_placement_still_checks_existing_eight_counters() {
    for (count, expected) in [(7, 0), (8, 8)] {
        let (mut engine, source) = setup();
        set_counters(&mut engine, source, CounterKind::Night, count);
        // Blossombind legally enchants a creature. Animate the artifact in layers without
        // replacing either printed Ring ability or its Snow supertype.
        for kind in [
            ContinuousEffectKind::Layer4AddTypes(TypeLineAddition {
                card_types: vec![PermanentTypeFilter::Creature],
                ..Default::default()
            }),
            ContinuousEffectKind::Layer7bSetPt {
                power: 1,
                toughness: 1,
            },
        ] {
            engine.state.continuous_effects.push(ContinuousEffect {
                source_id: None,
                trigger_grant_origin: None,
                affected: AffectedScope::Single(source),
                kind,
                condition: None,
                duration: EffectDuration::Indefinite,
                timestamp: engine.state.command_index,
            });
        }
        let aura = inject_permanent_on_battlefield(&mut engine, 0, "blossombind");
        engine.state.objects.get_mut(&aura).unwrap().attached_to =
            Some(tricerules_core::AttachmentRecipient::Object(source));
        start_upkeep(&mut engine, 0);
        assert_eq!(engine.state.objects[&aura].zone, Zone::Battlefield);
        resolve_entire_stack_two_player(&mut engine);
        assert_tokens(&engine, expected, 0);
        assert_eq!(
            engine.state.objects[&source].counter_count(CounterKind::Night),
            if expected == 0 { count } else { 0 }
        );
    }
}

fn dev_move(engine: &mut GameEngine, zone: DevZone) {
    engine.enable_dev_commands();
    engine
        .apply_command(
            engine.state.priority_player_id(),
            &RuledCommand {
                cmd: Some(Cmd::DevCommand(DevCommand {
                    target_player_id: 0,
                    dev: Some(dev_command::Dev::MoveCard(DevMoveCard {
                        card_name: "Replicating Ring".into(),
                        zone: zone as i32,
                        ready: true,
                    })),
                })),
            },
        )
        .unwrap();
}

#[test]
fn ring_departed_generation_uses_seven_or_eight_lki_and_cannot_mutate_returned_ring() {
    for count in [7, 8] {
        for returns in [false, true] {
            let (mut engine, source) = setup();
            set_counters(&mut engine, source, CounterKind::Night, count);
            start_upkeep(&mut engine, 0);
            let generation = engine.state.zone_change_generation[&source];
            dev_move(&mut engine, DevZone::Graveyard);
            if returns {
                dev_move(&mut engine, DevZone::Battlefield);
                set_counters(&mut engine, source, CounterKind::Night, 11);
            }
            assert_ne!(engine.state.zone_change_generation[&source], generation);
            resolve_entire_stack_two_player(&mut engine);
            assert_tokens(&engine, if count == 8 { 8 } else { 0 }, 0);
            assert_eq!(
                engine.state.objects[&source].counter_count(CounterKind::Night),
                if returns { 11 } else { 0 }
            );
        }
    }
}

fn change_control(engine: &mut GameEngine, source: u32, player: i32) {
    engine.state.continuous_effects.push(ContinuousEffect {
        source_id: None,
        trigger_grant_origin: None,
        affected: AffectedScope::Single(source),
        kind: ContinuousEffectKind::Layer2Control {
            controller: ControllerReference::Fixed(player),
        },
        condition: None,
        duration: EffectDuration::Indefinite,
        timestamp: engine.state.command_index,
    });
}

#[test]
fn ring_control_changes_preserve_trigger_controller_and_new_upkeep_scope() {
    for after_trigger in [false, true] {
        let (mut engine, source) = setup();
        set_counters(&mut engine, source, CounterKind::Night, 7);
        if after_trigger {
            start_upkeep(&mut engine, 0);
        }
        change_control(&mut engine, source, 1);
        if !after_trigger {
            start_upkeep(&mut engine, 0);
            assert!(engine.state.stack.is_empty());
            start_upkeep(&mut engine, 1);
        }
        assert_eq!(
            engine.state.stack[0].controller,
            if after_trigger { 0 } else { 1 }
        );
        resolve_entire_stack_two_player(&mut engine);
        assert_tokens(&engine, 8, if after_trigger { 0 } else { 1 });
    }
}

#[test]
fn ring_token_entry_parks_after_removal_rejects_bad_answers_and_commits_once() {
    let (mut engine, source) = setup();
    set_counters(&mut engine, source, CounterKind::Night, 7);
    start_upkeep(&mut engine, 0);
    // Avoid helpers that automatically answer the actual ordering choice being tested.
    for _ in 0..2 {
        engine
            .apply_command(engine.state.priority_player_id(), &pass())
            .unwrap();
    }
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("eight simultaneous token timestamps");
    assert_eq!(
        pending.presentation.choice_kind,
        ChoiceKind::SimultaneousEntryOrder
    );
    assert_eq!(pending.deciding_player, 0);
    let candidates = pending.presentation.candidates.clone();
    assert_eq!(candidates.len(), 8);
    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::Night),
        0
    );
    assert!(
        token_ids(&engine).is_empty(),
        "no public tokens before entry commits"
    );
    let valid = submit_resolution_choice(candidates.clone());
    reject(&mut engine, 1, &valid);
    reject(
        &mut engine,
        0,
        &submit_resolution_choice(vec![candidates[0]; 8]),
    );
    reject(
        &mut engine,
        0,
        &submit_resolution_choice(candidates[..7].to_vec()),
    );
    engine.apply_command(0, &valid).unwrap();
    assert_tokens(&engine, 8, 0);
    assert!(engine.state.pending_resolution.is_none() && engine.state.stack.is_empty());
    reject(&mut engine, 0, &valid);
    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::Night),
        0
    );
}

#[test]
fn ring_and_tokens_produce_all_five_colors_immediately_and_reject_invalid_activations() {
    let (mut engine, source) = setup();
    set_counters(&mut engine, source, CounterKind::Night, 7);
    start_upkeep(&mut engine, 0);
    resolve_entire_stack_two_player(&mut engine);
    let ids = token_ids(&engine);
    for id in [source, ids[0]] {
        for option in 0..5 {
            engine.state.objects.get_mut(&id).unwrap().tapped = false;
            engine.state.players[0].mana_pool = Default::default();
            let mut command = activate_ability_for(&engine, id, 0, vec![]);
            let Some(Cmd::ActivateAbility(ability)) = command.cmd.as_mut() else {
                unreachable!()
            };
            ability.mana_option_index = option;
            let mut invalid = command.clone();
            let Some(Cmd::ActivateAbility(ability)) = invalid.cmd.as_mut() else {
                unreachable!()
            };
            ability.mana_option_index = 5;
            reject(&mut engine, 0, &invalid);
            reject(&mut engine, 1, &command);
            engine.apply_command(0, &command).unwrap();
            assert!(engine.state.stack.is_empty());
            assert!(engine.state.objects[&id].tapped);
            let pool = engine.state.players[0].mana_pool;
            let values = [pool.white, pool.blue, pool.black, pool.red, pool.green];
            assert_eq!(values[option as usize], 1);
            assert_eq!(values.into_iter().sum::<u32>(), 1);
            assert_eq!(pool.colorless, 0);
            reject(&mut engine, 0, &command);
        }
    }
    start_upkeep(&mut engine, 0);
    assert_eq!(
        engine.state.stack.len(),
        1,
        "tokens do not inherit the upkeep ability"
    );
    assert_eq!(engine.state.stack[0].source_permanent_id, Some(source));
}

#[test]
fn ring_replays_paid_cast_eight_real_upkeeps_and_parked_token_entry_commands() {
    use tricerules_proto::ruled::v1::{dev_command::Dev, DevAddMana, DevPutCardInZone};
    fn game() -> GameEngine {
        let mut engine = GameEngine::new(
            tricerules_cards::registry::global(),
            507_108,
            &[0, 1],
            20,
            Some(vec![deck_with("island", &[]), deck_with("forest", &[])]),
            true,
        )
        .unwrap();
        engine.enable_dev_commands();
        engine
    }
    fn dev(payload: Dev) -> RuledCommand {
        RuledCommand {
            cmd: Some(Cmd::DevCommand(DevCommand {
                target_player_id: 0,
                dev: Some(payload),
            })),
        }
    }
    let mut engine = game();
    let mut log = Vec::new();
    fn apply(
        engine: &mut GameEngine,
        log: &mut Vec<(i32, RuledCommand, RuledEventBatch, serde_json::Value)>,
        actor: i32,
        command: RuledCommand,
    ) {
        let batch = engine.apply_command(actor, &command).unwrap();
        log.push((actor, command, batch, engine.diagnostic_snapshot().unwrap()));
    }
    while engine.state.turn_step != TurnStep::Main1 {
        let actor = engine.state.priority_player_id();
        apply(&mut engine, &mut log, actor, pass());
    }
    apply(
        &mut engine,
        &mut log,
        0,
        dev(Dev::PutCardInZone(DevPutCardInZone {
            card_name: "Replicating Ring".into(),
            zone: DevZone::Hand as i32,
            ready: false,
        })),
    );
    apply(
        &mut engine,
        &mut log,
        0,
        dev(Dev::AddMana(DevAddMana {
            c: 3,
            ..Default::default()
        })),
    );
    let slot = hand_index_for_card(&engine, 0, RING);
    apply(&mut engine, &mut log, 0, cast_spell(slot, vec![]));
    let mut ordering_answers = 0;
    for _ in 0..1800 {
        if token_ids(&engine).len() == 8 {
            break;
        }
        let (actor, command) = if let Some(pending) = &engine.state.pending_resolution {
            assert_eq!(
                pending.presentation.choice_kind,
                ChoiceKind::SimultaneousEntryOrder
            );
            ordering_answers += 1;
            (
                pending.deciding_player,
                submit_resolution_choice(pending.presentation.candidates.clone()),
            )
        } else if let Some(actor) = engine.state.cleanup_discard_player {
            let count = engine.state.players[engine.state.player_idx(actor).unwrap()]
                .hand
                .len()
                - 7;
            (actor, discard_cleanup_batch((0..count as u32).collect()))
        } else {
            (engine.state.priority_player_id(), pass())
        };
        apply(&mut engine, &mut log, actor, command);
    }
    assert_eq!(ordering_answers, 1);
    assert_tokens(&engine, 8, 0);
    let source = battlefield_object_for_card(&engine, 0, RING);
    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::Night),
        0
    );
    let mut replay = game();
    for (actor, command, batch, snapshot) in log {
        assert_eq!(replay.apply_command(actor, &command).unwrap(), batch);
        assert_eq!(replay.diagnostic_snapshot().unwrap(), snapshot);
    }
}
