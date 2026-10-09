//! Actual-card coverage for Lavabrink Floodgates' upkeep choice and reflexive damage.
use super::helpers::*;
use prost::Message;
use tricerules_cards::primitives::*;
use tricerules_cards::{AbilityPresentation, ControllerReference, Layout};
use tricerules_core::{AffectedScope, AttachmentRecipient, ContinuousEffect, TurnStep, Zone};
use tricerules_proto::ruled::v1::{dev_command, ruled_event::Ev, DevCommand, DevMoveCard, DevZone};

const FLOODGATES: &str = "lavabrink_floodgates";

fn setup(players: &[i32]) -> (GameEngine, u32) {
    assert!(
        tricerules_cards::registry::global()
            .get(FLOODGATES)
            .is_some(),
        "exact Lavabrink Floodgates is missing"
    );
    let decks = players.iter().map(|_| deck_with("island", &[])).collect();
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        603_120,
        players,
        20,
        Some(decks),
        true,
    )
    .unwrap();
    let source = inject_permanent_on_battlefield(&mut engine, 0, FLOODGATES);
    (engine, source)
}

fn set_counters(engine: &mut GameEngine, object: u32, counter: CounterKind, count: u32) {
    let counters = &mut engine.state.objects.get_mut(&object).unwrap().counters;
    if count == 0 {
        counters.remove(&counter);
    } else {
        counters.insert(counter, count);
    }
}

fn start_upkeep(engine: &mut GameEngine, player: i32) {
    let player_index = engine
        .state
        .players
        .iter()
        .position(|candidate| candidate.id == player)
        .expect("upkeep player exists");
    let previous_index =
        (player_index + engine.state.players.len() - 1) % engine.state.players.len();
    engine.state.active_player_idx = previous_index;
    engine.state.priority_idx = previous_index;
    engine.state.turn_step = TurnStep::EndStep;
    engine.state.passes_since_stack_change = 0;
    let active_players = engine.state.players.iter().filter(|p| !p.has_lost).count();
    for _ in 0..active_players {
        let actor = engine.state.priority_player_id();
        engine.apply_command(actor, &pass()).unwrap();
    }
    assert_eq!(engine.state.active_player_id(), player, "next turn player");
    assert_eq!(engine.state.turn_step, TurnStep::Upkeep);
    assert_eq!(engine.state.priority_player_id(), player);
}

fn select_branch(branch: u32) -> RuledCommand {
    let mut command = submit_resolution_decision(ResolutionChoiceDecision::SelectBranch);
    let Some(Cmd::SubmitResolutionChoice(answer)) = command.cmd.as_mut() else {
        unreachable!()
    };
    answer.selected_branch_index = branch;
    command
}

fn reject(engine: &mut GameEngine, actor: i32, command: &RuledCommand) {
    let before = engine.diagnostic_snapshot().unwrap();
    assert!(engine.apply_command(actor, command).is_err());
    assert_eq!(engine.diagnostic_snapshot().unwrap(), before);
}

fn add_damage_test_creature(engine: &mut GameEngine, player: usize) -> u32 {
    let creature = inject_creature_on_battlefield(engine, player, "grizzly_bears");
    engine.state.continuous_effects.push(ContinuousEffect {
        source_id: None,
        trigger_grant_origin: None,
        affected: AffectedScope::Single(creature),
        kind: ContinuousEffectKind::Layer7bSetPt {
            power: 9,
            toughness: 9,
        },
        condition: None,
        duration: EffectDuration::Indefinite,
        timestamp: engine.state.command_index,
    });
    creature
}

fn resolve_all_players_pass(engine: &mut GameEngine) {
    loop {
        answer_simultaneous_entry_order_in_engine_order(engine);
        answer_trigger_order_in_engine_order(engine);
        if engine.state.stack.is_empty() {
            break;
        }
        pass_priority_round(engine);
    }
}

fn dev_move_source(engine: &mut GameEngine, zone: DevZone) {
    engine.enable_dev_commands();
    let actor = engine.state.priority_player_id();
    engine
        .apply_command(
            actor,
            &RuledCommand {
                cmd: Some(Cmd::DevCommand(DevCommand {
                    target_player_id: 0,
                    dev: Some(dev_command::Dev::MoveCard(DevMoveCard {
                        card_name: "Lavabrink Floodgates".into(),
                        zone: zone as i32,
                        ready: true,
                    })),
                })),
            },
        )
        .unwrap();
}

fn change_control(engine: &mut GameEngine, object: u32, controller: i32) {
    engine.state.continuous_effects.push(ContinuousEffect {
        source_id: None,
        trigger_grant_origin: None,
        affected: AffectedScope::Single(object),
        kind: ContinuousEffectKind::Layer2Control {
            controller: ControllerReference::Fixed(controller),
        },
        condition: None,
        duration: EffectDuration::Indefinite,
        timestamp: engine.state.command_index,
    });
}

#[test]
fn lavabrink_floodgates_has_its_complete_single_face() {
    let card = tricerules_cards::registry::global()
        .get(FLOODGATES)
        .expect("exact Lavabrink Floodgates card is registered");

    assert_eq!(card.name, "Lavabrink Floodgates");
    assert_eq!(card.layout, Layout::Normal);
    assert_eq!(card.face_count(), 1);
    let face = card.primary_face();
    assert_eq!(face.mana_cost.to_string(), "{3}{R}");
    assert_eq!(face.types, ["Artifact"]);
    assert_eq!(face.power, None);
    assert_eq!(face.toughness, None);
    assert_eq!(face.activated_abilities.len(), 1);
    assert_eq!(face.triggered_abilities.len(), 1);
    assert_eq!(
        face.activated_abilities[0].presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert_eq!(
        face.triggered_abilities[0].presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    assert_eq!(
        face.triggered_abilities[0].trigger,
        TriggerCondition::AtBeginningOfUpkeep {
            player: CastTriggerPlayer::AnyPlayer
        }
    );
    assert_eq!(CounterKind::Doom.label(), "doom");
}

#[test]
fn lavabrink_floodgates_uses_the_upkeep_players_choice_and_rejects_invalid_answers() {
    let (mut engine, source) = setup(&[0, 1, 2]);
    set_counters(&mut engine, source, CounterKind::Doom, 3);
    assert_eq!(engine.state.objects[&source].zone, Zone::Battlefield);
    assert!(
        engine.state.stack.is_empty(),
        "external counters are not a state action"
    );

    start_upkeep(&mut engine, 2);
    assert_eq!(engine.state.stack.len(), 1);
    let trigger = engine.state.stack.last().unwrap();
    assert_eq!(
        trigger.controller, 0,
        "the artifact controller controls its trigger"
    );
    assert_eq!(trigger.trigger_context.affected_player, Some(2));

    pass_priority_round(&mut engine);
    let pending = engine.state.pending_resolution.as_ref().unwrap();
    assert_eq!(
        pending.presentation.choice_kind,
        ChoiceKind::ResolutionBranch
    );
    assert_eq!(pending.deciding_player, 2, "the upkeep player chooses");
    reject(&mut engine, 1, &select_branch(1));
    reject(&mut engine, 2, &select_branch(9));

    let batch = engine.apply_command(2, &select_branch(1)).unwrap();
    assert_eq!(engine.state.objects[&source].zone, Zone::Battlefield);
    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::Doom),
        2
    );
    assert!(
        engine.state.stack.is_empty(),
        "removal below three does not sacrifice"
    );
    let zone_view = batch
        .events
        .iter()
        .rev()
        .find_map(|event| match &event.ev {
            Some(Ev::ZoneView(view)) => Some(view),
            _ => None,
        })
        .expect("counter updates publish a battlefield view");
    let floodgates = zone_view.per_player[0]
        .battlefield_objects
        .iter()
        .find(|object| object.object_id == source)
        .expect("Floodgates remains visible on the battlefield");
    assert_eq!(floodgates.counters_annotation, "2 doom counter(s)");
}

#[test]
fn lavabrink_floodgates_add_to_threshold_sacrifices_and_deals_six_to_all_creatures() {
    let (mut engine, source) = setup(&[0, 1, 2]);
    set_counters(&mut engine, source, CounterKind::Doom, 2);
    set_counters(&mut engine, source, CounterKind::Finality, 1);
    let creatures = [
        add_damage_test_creature(&mut engine, 0),
        add_damage_test_creature(&mut engine, 1),
        add_damage_test_creature(&mut engine, 2),
    ];
    let life_totals = engine
        .state
        .players
        .iter()
        .map(|p| p.life)
        .collect::<Vec<_>>();

    start_upkeep(&mut engine, 2);
    pass_priority_round(&mut engine);
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .deciding_player,
        2
    );
    engine.apply_command(2, &select_branch(0)).unwrap();

    assert_eq!(engine.state.objects[&source].zone, Zone::Exile);
    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::Doom),
        0
    );
    assert_eq!(
        engine.state.stack.len(),
        1,
        "the sacrifice creates its reflexive trigger"
    );
    resolve_all_players_pass(&mut engine);

    for creature in creatures {
        let object = &engine.state.objects[&creature];
        assert_eq!(object.zone, Zone::Battlefield);
        assert_eq!(
            object.damage, 6,
            "each current creature receives six damage"
        );
    }
    assert_eq!(
        engine
            .state
            .players
            .iter()
            .map(|p| p.life)
            .collect::<Vec<_>>(),
        life_totals,
        "the reflexive ability damages creatures, not players"
    );
}

#[test]
fn lavabrink_floodgates_add_remove_and_decline_check_the_threshold_afterward() {
    for (before, branch, expected_zone, expected_counters) in [
        (0, Some(0), Zone::Battlefield, 1),
        (2, Some(0), Zone::Graveyard, 0),
        (3, Some(1), Zone::Battlefield, 2),
        (3, None, Zone::Graveyard, 0),
        (4, Some(1), Zone::Graveyard, 0),
    ] {
        let (mut engine, source) = setup(&[0, 1]);
        set_counters(&mut engine, source, CounterKind::Doom, before);
        assert_eq!(engine.state.objects[&source].zone, Zone::Battlefield);

        start_upkeep(&mut engine, 0);
        pass_priority_round(&mut engine);
        if let Some(branch) = branch {
            engine.apply_command(0, &select_branch(branch)).unwrap();
        } else {
            engine
                .apply_command(
                    0,
                    &submit_resolution_decision(ResolutionChoiceDecision::Decline),
                )
                .unwrap();
        }
        resolve_all_players_pass(&mut engine);
        assert_eq!(engine.state.objects[&source].zone, expected_zone);
        assert_eq!(
            engine.state.objects[&source].counter_count(CounterKind::Doom),
            expected_counters,
            "counter update precedes the three-counter threshold check"
        );
    }
}

#[test]
fn lavabrink_floodgates_reflexive_damage_respects_prevention_and_damage_triggers() {
    let (mut engine, source) = setup(&[0, 1, 2, 3]);
    set_counters(&mut engine, source, CounterKind::Doom, 2);
    let creatures = [
        add_damage_test_creature(&mut engine, 0),
        add_damage_test_creature(&mut engine, 1),
        add_damage_test_creature(&mut engine, 2),
    ];
    let observer = inject_permanent_on_battlefield(&mut engine, 2, "cracked_skull");
    engine.state.objects.get_mut(&observer).unwrap().attached_to =
        Some(AttachmentRecipient::Object(creatures[2]));
    let life_totals = engine
        .state
        .players
        .iter()
        .map(|p| p.life)
        .collect::<Vec<_>>();

    start_upkeep(&mut engine, 3);
    pass_priority_round(&mut engine);
    engine.state.add_damage_prevention_shield(creatures[1], 6);
    engine.apply_command(3, &select_branch(0)).unwrap();
    resolve_all_players_pass(&mut engine);

    assert_eq!(engine.state.objects[&creatures[0]].damage, 6);
    assert_eq!(engine.state.objects[&creatures[1]].damage, 0);
    assert_eq!(engine.state.objects[&creatures[2]].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&observer].zone, Zone::Graveyard);
    assert_eq!(
        engine
            .state
            .players
            .iter()
            .map(|p| p.life)
            .collect::<Vec<_>>(),
        life_totals,
        "the reflexive damage reaches creatures, not players"
    );
    assert_eq!(engine.state.objects[&source].zone, Zone::Graveyard);
}

#[test]
fn lavabrink_floodgates_logged_upkeep_branch_replays_to_the_same_state() {
    let (mut engine, source) = setup(&[0, 1]);
    let (mut replay, replay_source) = setup(&[0, 1]);
    assert_eq!(source, replay_source);
    for current in [&mut engine, &mut replay] {
        set_counters(current, source, CounterKind::Doom, 2);
        add_damage_test_creature(current, 0);
        add_damage_test_creature(current, 1);
        start_upkeep(current, 0);
        pass_priority_round(current);
    }
    assert_eq!(
        engine.diagnostic_snapshot().unwrap(),
        replay.diagnostic_snapshot().unwrap()
    );

    let command = select_branch(0);
    let encoded = command.encode_to_vec();
    let decoded = tricerules_proto::ruled::v1::RuledCommand::decode(encoded.as_slice()).unwrap();
    assert_eq!(
        engine.apply_command(0, &decoded).unwrap(),
        replay.apply_command(0, &decoded).unwrap()
    );
    resolve_all_players_pass(&mut engine);
    resolve_all_players_pass(&mut replay);
    assert_eq!(
        engine.diagnostic_snapshot().unwrap(),
        replay.diagnostic_snapshot().unwrap()
    );
    assert_eq!(engine.state.objects[&source].zone, Zone::Graveyard);
}

#[test]
fn lavabrink_floodgates_mana_ability_adds_two_red_without_using_the_stack() {
    let (mut engine, source) = setup(&[0, 1]);
    let activation = activate_ability_for(&engine, source, 0, vec![]);
    engine.apply_command(0, &activation).unwrap();
    assert_eq!(engine.state.players[0].mana_pool.red, 2);
    assert!(engine.state.objects[&source].tapped);
    assert!(engine.state.stack.is_empty());
}

#[test]
fn lavabrink_floodgates_cannot_sacrifice_a_source_it_no_longer_controls() {
    let (mut engine, source) = setup(&[0, 1]);
    set_counters(&mut engine, source, CounterKind::Doom, 3);
    start_upkeep(&mut engine, 0);
    assert_eq!(engine.state.stack.last().unwrap().controller, 0);
    change_control(&mut engine, source, 1);

    pass_priority_round(&mut engine);
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .deciding_player,
        0
    );
    engine
        .apply_command(
            0,
            &submit_resolution_decision(ResolutionChoiceDecision::Decline),
        )
        .unwrap();

    assert_eq!(engine.state.objects[&source].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&source].controller, 1);
    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::Doom),
        3
    );
    assert!(
        engine.state.stack.is_empty(),
        "no sacrifice receipt means no reflexive trigger"
    );
}

#[test]
fn lavabrink_floodgates_old_upkeep_trigger_cannot_affect_a_new_source_generation() {
    for returns_to_battlefield in [false, true] {
        let (mut engine, source) = setup(&[0, 1]);
        set_counters(&mut engine, source, CounterKind::Doom, 3);
        start_upkeep(&mut engine, 0);
        let original_generation = engine
            .state
            .zone_change_generation
            .get(&source)
            .copied()
            .unwrap_or(0);

        dev_move_source(&mut engine, DevZone::Graveyard);
        if returns_to_battlefield {
            dev_move_source(&mut engine, DevZone::Battlefield);
            set_counters(&mut engine, source, CounterKind::Doom, 4);
        }
        assert_ne!(
            engine.state.zone_change_generation[&source],
            original_generation
        );

        resolve_all_players_pass(&mut engine);
        assert!(engine.state.pending_resolution.is_none());
        assert!(
            engine.state.stack.is_empty(),
            "stale receipt cannot create reflexive damage"
        );
        if returns_to_battlefield {
            assert_eq!(engine.state.objects[&source].zone, Zone::Battlefield);
            assert_eq!(
                engine.state.objects[&source].counter_count(CounterKind::Doom),
                4
            );
        } else {
            assert_eq!(engine.state.objects[&source].zone, Zone::Graveyard);
        }
    }
}

#[test]
fn lavabrink_floodgates_delegates_a_parked_upkeep_choice_after_the_upkeep_player_concedes() {
    let (mut engine, source) = setup(&[0, 1, 2, 3]);
    set_counters(&mut engine, source, CounterKind::Doom, 2);
    start_upkeep(&mut engine, 2);
    pass_priority_round(&mut engine);
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .deciding_player,
        2
    );

    engine.apply_command(2, &concede()).unwrap();
    let pending = engine.state.pending_resolution.as_ref().unwrap();
    assert_eq!(
        pending.deciding_player, 0,
        "the ability controller chooses a delegate"
    );
    let ResolutionContinuation::AuthoredBranch { branch, .. } = &pending.continuation else {
        panic!("upkeep branch remains parked while its chooser is replaced");
    };
    let PendingResolutionBranchStage::ChoosingDelegate { candidates } = &branch.stage else {
        panic!("the controller receives the 800.4g delegate selection");
    };
    assert_eq!(candidates, &[Some(1), Some(3)]);

    engine.apply_command(0, &select_branch(0)).unwrap();
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .deciding_player,
        1
    );
    engine.apply_command(1, &select_branch(0)).unwrap();
    assert_eq!(engine.state.objects[&source].zone, Zone::Graveyard);
    resolve_all_players_pass(&mut engine);
    assert!(engine.state.stack.is_empty());
    assert!(engine.state.pending_resolution.is_none());
}
