//! Actual Teferi, Temporal Archmage +1, using the global registered card definition.
//! These scenarios assert engine offers and events; they do not test Servatrice recipient projection.
use super::helpers::*;
use crate::helpers::ScenarioCommandExt;
use prost::Message;
use tricerules_cards::primitives::CounterKind;
use tricerules_cards::{ContinuousEffectKind, ControllerReference, EffectDuration};
use tricerules_core::{AffectedScope, ContinuousEffect, Zone};
use tricerules_proto::ruled::v1::{dev_command, DevCommand, DevMoveCard, DevZone};

const TEFERI: &str = "teferi,_temporal_archmage";
const CONTROLLER: i32 = 10;
const OPPONENT: i32 = 20;
const THIRD_PLAYER: i32 = 30;
const FOURTH_PLAYER: i32 = 40;

fn setup(seed: u64) -> GameEngine {
    let registry = tricerules_cards::registry::global();
    registry
        .get(TEFERI)
        .expect("Teferi, Temporal Archmage must be registered for its +1 scenarios");
    let deck = deck_with("forest", &[]);
    let mut engine = GameEngine::new(
        registry,
        seed,
        &[CONTROLLER, OPPONENT],
        20,
        Some(vec![deck.clone(), deck]),
        true,
    )
    .expect("two-player Teferi scenario game");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn setup_four_player(seed: u64) -> GameEngine {
    let registry = tricerules_cards::registry::global();
    registry
        .get(TEFERI)
        .expect("Teferi, Temporal Archmage must be registered for its +1 scenarios");
    let deck = deck_with("forest", &[]);
    let mut engine = GameEngine::new(
        registry,
        seed,
        &[CONTROLLER, OPPONENT, THIRD_PLAYER, FOURTH_PLAYER],
        20,
        Some(vec![deck.clone(), deck.clone(), deck.clone(), deck]),
        true,
    )
    .expect("four-player Teferi +1 source-owner departure scenario game");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn stage_synthetic_layer2_control(engine: &mut GameEngine, object_id: u32, controller: i32) {
    let owner = engine.state.objects[&object_id].owner;
    let base_controller = engine.state.objects[&object_id].base_controller;
    let previous_controller = engine.state.objects[&object_id].controller;
    engine.state.continuous_effects.push(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: None,
        affected: AffectedScope::Single(object_id),
        kind: ContinuousEffectKind::Layer2Control {
            controller: ControllerReference::Fixed(controller),
        },
        condition: None,
        duration: EffectDuration::Indefinite,
        timestamp: engine.state.command_index + 1,
    });

    let previous_index = engine
        .state
        .player_idx(previous_controller)
        .expect("previous controller seat");
    let controller_index = engine
        .state
        .player_idx(controller)
        .expect("new controller seat");
    if previous_index != controller_index {
        engine.state.players[previous_index]
            .battlefield
            .retain(|candidate| *candidate != object_id);
    }
    if !engine.state.players[controller_index]
        .battlefield
        .contains(&object_id)
    {
        engine.state.players[controller_index]
            .battlefield
            .push(object_id);
    }
    engine
        .state
        .objects
        .get_mut(&object_id)
        .expect("controlled Teferi")
        .controller = controller;
    assert_eq!(engine.state.objects[&object_id].owner, owner);
    assert_eq!(
        engine.state.objects[&object_id].base_controller,
        base_controller
    );
    assert_eq!(engine.state.objects[&object_id].controller, controller);
    assert!(engine.state.players[controller_index]
        .battlefield
        .contains(&object_id));
    engine.initial_response_batch();
}

fn cast_teferi(engine: &mut GameEngine) -> u32 {
    let source = inject_card_into_hand(engine, 0, TEFERI);
    give_mana(
        engine,
        CONTROLLER,
        ManaGift {
            u: 2,
            c: 4,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(engine, 0, TEFERI);
    engine
        .apply_scenario_command(CONTROLLER, &cast_spell(slot, Vec::new()))
        .expect("cast Teferi for {4}{U}{U}");
    assert_eq!(engine.state.players[0].mana_pool, Default::default());
    pass_to_resolution(engine);
    assert!(engine.state.stack.is_empty());
    assert_eq!(engine.state.objects[&source].zone, Zone::Battlefield);
    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::Loyalty),
        5,
        "Teferi enters with the registered starting loyalty"
    );
    source
}

fn replace_library(engine: &mut GameEngine, cards: &[&str]) -> Vec<u32> {
    let old = std::mem::take(&mut engine.state.players[0].library);
    for object_id in old {
        engine.state.objects.get_mut(&object_id).unwrap().zone = Zone::Graveyard;
        engine.state.players[0].graveyard.push(object_id);
    }
    cards
        .iter()
        .map(|card| inject_library_card(engine, 0, card))
        .collect()
}

fn generation(engine: &GameEngine, object_id: u32) -> u64 {
    engine
        .state
        .zone_change_generation
        .get(&object_id)
        .copied()
        .unwrap_or(0)
}

fn activate_plus_one(engine: &mut GameEngine, source: u32) -> RuledEventBatch {
    let batch = engine
        .apply_scenario_command(
            CONTROLLER,
            &activate_ability_for(engine, source, 0, Vec::new()),
        )
        .expect("pay Teferi's +1 loyalty cost");
    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::Loyalty),
        6,
        "the +1 loyalty cost is paid when the ability is activated"
    );
    assert!(!engine.state.stack.is_empty());
    batch
}

fn pass_to_resolution(engine: &mut GameEngine) -> RuledEventBatch {
    let mut batch = RuledEventBatch::default();
    for _ in 0..engine.state.players.len() {
        let actor = engine.state.priority_player_id();
        batch = engine
            .apply_scenario_command(actor, &pass())
            .expect("pass priority toward Teferi's +1");
        if engine.state.pending_resolution.is_some() || engine.state.stack.is_empty() {
            break;
        }
    }
    batch
}

fn assert_no_reveal_or_public_library_names(batch: &RuledEventBatch) {
    for event in &batch.events {
        match &event.ev {
            Some(Ev::CardsRevealed(_)) => {
                panic!("Teferi's +1 does not reveal the looked-at cards")
            }
            Some(Ev::Log(log)) if log.visible_to_player_id.is_none() => {
                for name in ["Forest", "Island"] {
                    assert!(
                        !log.text.contains(name),
                        "public library-name leak: {}",
                        log.text
                    );
                }
            }
            _ => {}
        }
    }
}

fn assert_private_hand_choice(choice: &ResolutionChoiceRequired, min: u32, max: u32) {
    assert_eq!(choice.deciding_player_id, CONTROLLER);
    assert_eq!(choice.choice_kind(), ChoiceKind::LibraryLook);
    assert_eq!((choice.min, choice.max), (min, max));
    assert!(choice.public_reveal.is_none());
}

fn reject_without_mutation(engine: &mut GameEngine, actor: i32, chosen: Vec<u32>) {
    let before = engine
        .diagnostic_snapshot()
        .expect("before rejected choice");
    assert!(engine
        .apply_scenario_command(actor, &submit_resolution_choice(chosen))
        .is_err());
    assert_eq!(
        engine.diagnostic_snapshot().expect("after rejected choice"),
        before
    );
}

#[test]
fn paid_plus_one_with_an_empty_library_does_not_draw_or_open_a_choice() {
    let mut engine = setup(26_102_701);
    let source = cast_teferi(&mut engine);
    replace_library(&mut engine, &[]);
    let hand_before = engine.state.players[0].hand.clone();

    activate_plus_one(&mut engine, source);
    let batch = pass_to_resolution(&mut engine);

    assert!(find_resolution_choice(&batch).is_none());
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
    assert!(engine.state.players[0].library.is_empty());
    assert_eq!(engine.state.players[0].hand, hand_before);
    assert_no_reveal_or_public_library_names(&batch);
}

#[test]
fn paid_plus_one_with_one_library_card_still_requires_its_mandatory_selection() {
    let mut engine = setup(26_102_702);
    let source = cast_teferi(&mut engine);
    let library = replace_library(&mut engine, &["forest"]);
    let [only] = library.as_slice() else {
        unreachable!()
    };
    let only = *only;
    let original_generation = generation(&engine, only);
    let hand_before = engine.state.players[0].hand.len();

    activate_plus_one(&mut engine, source);
    let batch = pass_to_resolution(&mut engine);
    let choice = find_resolution_choice(&batch).expect("the one available card must be chosen");
    assert_private_hand_choice(&choice, 1, 1);
    assert_eq!(choice.candidate_object_ids, vec![only]);
    assert_eq!(choice.candidate_names, vec!["Forest".to_string()]);
    assert_eq!(engine.state.players[0].library.len(), 1);
    assert_eq!(engine.state.objects[&only].zone, Zone::Library);
    assert_eq!(generation(&engine, only), original_generation);
    assert_eq!(engine.state.players[0].hand.len(), hand_before);
    assert_no_reveal_or_public_library_names(&batch);

    reject_without_mutation(&mut engine, OPPONENT, vec![only]);
    reject_without_mutation(&mut engine, CONTROLLER, vec![]);
    let selected = engine
        .apply_scenario_command(CONTROLLER, &submit_resolution_choice(vec![only]))
        .expect("choose the only card");
    assert_eq!(engine.state.objects[&only].zone, Zone::Hand);
    assert_ne!(generation(&engine, only), original_generation);
    assert!(engine.state.players[0].hand.contains(&only));
    assert!(engine.state.players[0].library.is_empty());
    assert_eq!(engine.state.players[0].hand.len(), hand_before + 1);
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
    assert_no_reveal_or_public_library_names(&selected);
}

#[test]
fn paid_plus_one_selects_one_of_two_private_cards_without_draw_search_or_reveal() {
    let mut engine = setup(26_102_703);
    let source = cast_teferi(&mut engine);
    let candidates = replace_library(&mut engine, &["forest", "island"]);
    let generations = candidates
        .iter()
        .map(|object_id| generation(&engine, *object_id))
        .collect::<Vec<_>>();
    let hand_before = engine.state.players[0].hand.len();

    activate_plus_one(&mut engine, source);
    let batch = pass_to_resolution(&mut engine);
    let choice = find_resolution_choice(&batch).expect("choose exactly one looked-at card");
    assert_private_hand_choice(&choice, 1, 1);
    assert_eq!(choice.candidate_object_ids, candidates);
    assert_eq!(
        choice.candidate_names,
        vec!["Forest".to_string(), "Island".to_string()]
    );
    assert_eq!(engine.state.players[0].library.len(), 2);
    for (object_id, original_generation) in candidates.iter().zip(&generations) {
        assert_eq!(engine.state.objects[object_id].zone, Zone::Library);
        assert_eq!(generation(&engine, *object_id), *original_generation);
    }
    assert_eq!(engine.state.players[0].hand.len(), hand_before);
    assert_no_reveal_or_public_library_names(&batch);
    reject_without_mutation(&mut engine, OPPONENT, vec![candidates[0]]);
    reject_without_mutation(&mut engine, CONTROLLER, vec![]);
    reject_without_mutation(&mut engine, CONTROLLER, candidates.clone());
    let selected = engine
        .apply_scenario_command(CONTROLLER, &submit_resolution_choice(vec![candidates[0]]))
        .expect("make the mandatory private choice");

    assert_eq!(engine.state.objects[&candidates[0]].zone, Zone::Hand);
    assert_eq!(engine.state.objects[&candidates[1]].zone, Zone::Library);
    assert_ne!(generation(&engine, candidates[0]), generations[0]);
    assert_eq!(generation(&engine, candidates[1]), generations[1]);
    assert_eq!(engine.state.players[0].hand.len(), hand_before + 1);
    assert_eq!(engine.state.players[0].library.len(), 1);
    assert_eq!(engine.state.players[0].library.back(), Some(&candidates[1]));
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
    assert_no_reveal_or_public_library_names(&selected);
}

fn depart_teferi_command() -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::DevCommand(DevCommand {
            target_player_id: CONTROLLER,
            dev: Some(dev_command::Dev::MoveCard(DevMoveCard {
                card_name: "Teferi, Temporal Archmage".into(),
                zone: DevZone::Graveyard as i32,
                ready: true,
            })),
        })),
    }
}

fn prepared_departure_replay(seed: u64) -> (GameEngine, u32, Vec<u32>) {
    let mut engine = setup(seed);
    let source = cast_teferi(&mut engine);
    let candidates = replace_library(&mut engine, &["forest", "island"]);
    (engine, source, candidates)
}

fn record(
    engine: &mut GameEngine,
    log: &mut Vec<(i32, RuledCommand, RuledEventBatch)>,
    actor: i32,
    command: RuledCommand,
) -> RuledEventBatch {
    let batch = engine
        .apply_scenario_command(actor, &command)
        .expect("accept the recorded command");
    log.push((actor, command, batch.clone()));
    batch
}

fn record_raw(
    engine: &mut GameEngine,
    log: &mut Vec<(i32, Vec<u8>, RuledEventBatch)>,
    actor: i32,
    command: RuledCommand,
) -> RuledEventBatch {
    let wire = command.encode_to_vec();
    let decoded = RuledCommand::decode(wire.as_slice()).expect("decode accepted raw command");
    let batch = engine
        .apply_command(actor, &decoded)
        .expect("accept the recorded raw command");
    log.push((actor, wire, batch.clone()));
    batch
}

fn prepared_controlled_teferi(seed: u64) -> (GameEngine, u32, Vec<u32>) {
    let mut engine = setup_four_player(seed);
    let source = inject_permanent_on_battlefield(&mut engine, 1, TEFERI);
    engine
        .state
        .objects
        .get_mut(&source)
        .expect("synthetically placed registered Teferi")
        .set_counter(CounterKind::Loyalty, 5);
    stage_synthetic_layer2_control(&mut engine, source, CONTROLLER);
    assert_eq!(engine.state.objects[&source].owner, OPPONENT);
    assert_eq!(engine.state.objects[&source].base_controller, OPPONENT);
    assert_eq!(engine.state.objects[&source].controller, CONTROLLER);
    assert_eq!(
        engine.characteristics(source).unwrap().controller,
        CONTROLLER
    );
    let candidates = replace_library(&mut engine, &["forest", "island"]);
    (engine, source, candidates)
}

fn submit_choice_with_prompt(chosen: Vec<u32>, prompt_id: u64) -> RuledCommand {
    let mut command = submit_resolution_choice(chosen);
    let Some(Cmd::SubmitResolutionChoice(answer)) = command.cmd.as_mut() else {
        unreachable!()
    };
    answer.resolution_prompt_id = prompt_id;
    command
}

#[test]
fn paid_plus_one_resolves_after_synthetic_source_departure_and_replays_recorded_commands() {
    let seed = 26_102_704;
    let (mut engine, source, candidates) = prepared_departure_replay(seed);
    let mut log = Vec::new();
    let activation = activate_ability_for(&engine, source, 0, Vec::new());
    record(&mut engine, &mut log, CONTROLLER, activation);
    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::Loyalty),
        6
    );
    engine.enable_dev_commands();
    let actor = engine.state.priority_player_id();
    record(&mut engine, &mut log, actor, depart_teferi_command());
    assert_eq!(engine.state.objects[&source].zone, Zone::Graveyard);
    assert!(
        !engine.state.stack.is_empty(),
        "the activated ability survives its source"
    );

    let mut choice = None;
    for _ in 0..engine.state.players.len() {
        let actor = engine.state.priority_player_id();
        let batch = record(&mut engine, &mut log, actor, pass());
        if let Some(found) = find_resolution_choice(&batch) {
            choice = Some(found);
            break;
        }
    }
    let choice = choice.expect("the departed source's +1 still offers its private choice");
    assert_private_hand_choice(&choice, 1, 1);
    let choice_batch = log
        .last()
        .expect("the final pass produced the choice")
        .2
        .clone();
    assert_no_reveal_or_public_library_names(&choice_batch);
    let selected = record(
        &mut engine,
        &mut log,
        CONTROLLER,
        submit_resolution_choice(vec![candidates[0]]),
    );
    assert_no_reveal_or_public_library_names(&selected);
    assert_eq!(engine.state.objects[&candidates[0]].zone, Zone::Hand);
    assert_eq!(engine.state.objects[&candidates[1]].zone, Zone::Library);
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());

    let (mut replay, replay_source, replay_candidates) = prepared_departure_replay(seed);
    assert_eq!(replay_source, source);
    assert_eq!(replay_candidates, candidates);
    replay.enable_dev_commands();
    for (actor, command, expected_batch) in log {
        assert_eq!(
            replay
                .apply_scenario_command(actor, &command)
                .expect("replay each accepted Teferi command"),
            expected_batch
        );
    }
    assert_eq!(
        replay.diagnostic_snapshot().unwrap(),
        engine.diagnostic_snapshot().unwrap()
    );
}

#[test]
fn private_plus_one_choice_survives_source_owner_concession_and_wire_replay() {
    let seed = 26_102_705;
    let (mut engine, source, candidates) = prepared_controlled_teferi(seed);
    let [first, second] = candidates.as_slice() else {
        unreachable!()
    };
    let first = *first;
    let second = *second;
    let original_generations = [generation(&engine, first), generation(&engine, second)];
    let hand_before = engine.state.players[0].hand.len();
    let mut log = Vec::new();

    let activation = activate_ability_for(&engine, source, 0, Vec::new());
    record_raw(&mut engine, &mut log, CONTROLLER, activation);
    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::Loyalty),
        6,
        "P10 pays the actual +1 ability while controlling P20's Teferi"
    );
    assert_eq!(engine.state.stack.len(), 1);

    let mut offered_choice = None;
    let mut pass_actors = Vec::new();
    for _ in 0..engine.state.players.len() {
        let actor = engine.state.priority_player_id();
        pass_actors.push(actor);
        let batch = record_raw(&mut engine, &mut log, actor, pass());
        if let Some(choice) = find_resolution_choice(&batch) {
            offered_choice = Some(choice);
            break;
        }
    }
    let choice =
        offered_choice.expect("the four-player pass cycle reaches Teferi's mandatory look choice");
    assert_eq!(
        pass_actors,
        vec![CONTROLLER, OPPONENT, THIRD_PLAYER, FOURTH_PLAYER],
        "all four actual players pass before the ability parks its choice"
    );
    assert_private_hand_choice(&choice, 1, 1);
    assert_ne!(choice.resolution_prompt_id, 0);
    assert_eq!(choice.candidate_object_ids, vec![first, second]);
    assert_eq!(
        choice.candidate_names,
        vec!["Forest".to_string(), "Island".to_string()]
    );
    let choice_batch = log
        .last()
        .expect("the last pass offered the choice")
        .2
        .clone();
    assert_no_reveal_or_public_library_names(&choice_batch);

    let concede_batch = record_raw(&mut engine, &mut log, OPPONENT, concede());
    assert_no_reveal_or_public_library_names(&concede_batch);
    assert!(engine.state.players[1].has_lost);
    assert_eq!(
        engine
            .state
            .players
            .iter()
            .filter(|player| !player.has_lost)
            .count(),
        3,
        "P20's real concession leaves three surviving players"
    );
    assert!(engine.state.winner().is_none());
    assert!(
        !engine.state.objects.contains_key(&source),
        "P20's owned Teferi leaves the game even while P10 controls it"
    );
    assert!(engine.state.pending_resolution.is_some());

    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("P10's parked choice survives P20's departure");
    assert_eq!(pending.deciding_player, CONTROLLER);
    assert_eq!(pending.presentation.choice_kind, ChoiceKind::LibraryLook);
    assert_eq!((pending.presentation.min, pending.presentation.max), (1, 1));
    assert_eq!(pending.presentation.candidates, vec![first, second]);
    let prompt_id = pending.resolution_prompt_id.get();
    assert_ne!(prompt_id, 0);
    assert_eq!(prompt_id, choice.resolution_prompt_id);
    let selected = record_raw(
        &mut engine,
        &mut log,
        CONTROLLER,
        submit_choice_with_prompt(vec![first], prompt_id),
    );
    assert_no_reveal_or_public_library_names(&selected);
    assert_eq!(engine.state.objects[&first].zone, Zone::Hand);
    assert_eq!(engine.state.objects[&second].zone, Zone::Library);
    assert_eq!(engine.state.players[0].hand.len(), hand_before + 1);
    assert!(engine.state.players[0].hand.contains(&first));
    assert_eq!(engine.state.players[0].library.back(), Some(&second));
    assert_ne!(generation(&engine, first), original_generations[0]);
    assert_eq!(generation(&engine, second), original_generations[1]);
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());

    assert_eq!(engine.state.priority_player_id(), CONTROLLER);
    let next_legal_pass = engine.state.priority_player_id();
    record_raw(&mut engine, &mut log, next_legal_pass, pass());

    let (mut replay, replay_source, replay_candidates) = prepared_controlled_teferi(seed);
    assert_eq!(replay_source, source);
    assert_eq!(replay_candidates, candidates);
    for (actor, wire, expected_batch) in &log {
        let command = RuledCommand::decode(wire.as_slice()).expect("decode recorded wire command");
        assert_eq!(
            replay
                .apply_command(*actor, &command)
                .expect("replay each accepted raw command"),
            *expected_batch,
            "replay publishes the identical event batch"
        );
    }
    assert_eq!(
        replay.diagnostic_snapshot().unwrap(),
        engine.diagnostic_snapshot().unwrap(),
        "the raw wire replay reaches an identical diagnostic state"
    );
    assert_eq!(
        replay.state.priority_player_id(),
        engine.state.priority_player_id()
    );
}
