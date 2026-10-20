//! Actual-card coverage for Lavabrink Floodgates' upkeep choice and reflexive damage.
use super::helpers::*;
use crate::helpers::ScenarioCommandExt;
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
        engine.apply_scenario_command(actor, &pass()).unwrap();
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

fn select_branch_for_prompt(prompt_id: u64, branch: u32) -> RuledCommand {
    let mut command = select_branch(branch);
    let Some(Cmd::SubmitResolutionChoice(answer)) = command.cmd.as_mut() else {
        unreachable!()
    };
    answer.resolution_prompt_id = prompt_id;
    command
}

fn pass_priority_round_with_batch(
    engine: &mut GameEngine,
) -> tricerules_proto::ruled::v1::RuledEventBatch {
    answer_simultaneous_entry_order_in_engine_order(engine);
    answer_trigger_order_in_engine_order(engine);
    let remaining_passes = engine
        .state
        .players
        .iter()
        .filter(|player| !player.has_lost)
        .count()
        .checked_sub(engine.state.passes_since_stack_change as usize)
        .expect("consistent priority pass count");
    let mut resolution_batch = None;
    for _ in 0..remaining_passes {
        let actor = engine.state.priority_player_id();
        resolution_batch = Some(
            engine
                .apply_scenario_command(actor, &pass())
                .expect("each surviving priority holder passes"),
        );
    }
    answer_simultaneous_entry_order_in_engine_order(engine);
    resolution_batch.expect("the final pass produces the resolution batch")
}

fn prime_upkeep_prompt_and_decline(engine: &mut GameEngine, source: u32, player: i32) -> u64 {
    start_upkeep(engine, player);
    let batch = pass_priority_round_with_batch(engine);
    let choice = find_resolution_choice(&batch)
        .expect("the current upkeep player receives the optional choice");
    assert_eq!(choice.deciding_player_id, player);
    assert_ne!(choice.resolution_prompt_id, 0);
    let prompt_id = choice.resolution_prompt_id;
    let mut decline = submit_resolution_decision(ResolutionChoiceDecision::Decline);
    let Some(Cmd::SubmitResolutionChoice(answer)) = decline.cmd.as_mut() else {
        unreachable!()
    };
    answer.resolution_prompt_id = prompt_id;
    engine
        .apply_scenario_command(player, &decline)
        .expect("the upkeep player declines the optional choice");
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::Doom),
        2
    );
    prompt_id
}

fn reject(engine: &mut GameEngine, actor: i32, command: &RuledCommand) {
    let before = engine.diagnostic_snapshot().unwrap();
    assert!(engine.apply_scenario_command(actor, command).is_err());
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
        .apply_scenario_command(
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
fn lavabrink_floodgates_upkeep_trigger_owns_the_typed_program() {
    let registry = tricerules_cards::registry::global();
    let card = registry
        .get(FLOODGATES)
        .expect("exact Lavabrink Floodgates card is registered");
    let face = card.primary_face();
    let ability = face
        .triggered_abilities
        .first()
        .expect("upkeep trigger has one exact ability owner");
    let owner = tricerules_cards::ProgramOwnerId::ability(tricerules_cards::AbilityDefinitionId {
        card_id: FLOODGATES.into(),
        face_id: face.face_id.clone(),
        ability_path: vec![ability.ability_id.clone()],
    });
    let program = registry
        .compiled_program_for_owner(&owner)
        .expect("the exact registered upkeep ability has a compiled Program");
    assert_eq!(
        program.required_inputs,
        vec![tricerules_cards::primitives::ProgramRequiredInput::TriggerAffectedPlayer]
    );

    let [choice, threshold] = program.body.steps.as_slice() else {
        panic!("the admitted upkeep Program has a choice followed by a threshold");
    };
    let tricerules_cards::primitives::CompiledInstructionKind::Choose {
        chooser,
        optional,
        branches,
        ..
    } = &choice.kind
    else {
        panic!("the upkeep player chooses whether to add, remove, or decline");
    };
    assert_eq!(*chooser, PlayerRecipient::AffectedPlayer);
    assert!(*optional);
    let [add, remove] = branches.as_slice() else {
        panic!("the exact card has separate add and remove branches");
    };
    assert_eq!(
        add.fallback_label(),
        "Put a doom counter on this permanent."
    );
    assert_eq!(
        remove.fallback_label(),
        "Remove a doom counter from this permanent."
    );

    let tricerules_cards::primitives::CompiledInstructionKind::If { yes, no, .. } = &threshold.kind
    else {
        panic!("the threshold check follows the upkeep player's choice");
    };
    assert!(no.steps.is_empty());
    let [sacrifice, reflexive] = yes.steps.as_slice() else {
        panic!("threshold resolution sacrifices the source before creating the reflexive child");
    };
    assert!(matches!(
        program
            .slot(sacrifice.outputs[0])
            .map(|slot| &slot.result_type),
        Some(tricerules_cards::primitives::ResultType::CardResultCohort {
            action: tricerules_cards::primitives::CardResultAction::Sacrifice,
            who: PlayerRecipient::Controller,
            cardinality: tricerules_cards::primitives::Cardinality {
                min: 0,
                max: Some(1),
            },
        })
    ));
    assert_eq!(reflexive.input, Some(sacrifice.outputs[0]));
    let tricerules_cards::primitives::CompiledInstructionKind::Perform(effect) = &reflexive.kind
    else {
        panic!("the immediately bound sacrifice result feeds the reflexive child");
    };
    assert!(matches!(
        effect.as_ref(),
        SpellEffectKind::CreateReflexiveTrigger {
            when: Some(ResolutionReceiptCondition::Sacrificed {
                object: ConditionObjectRef::Source,
            }),
            ability,
        } if matches!(
            ability.effect.as_slice(),
            [SpellEffectKind::DamageAll {
                amount: Amount::Fixed(6),
                players: MassPlayerSet::All,
                kind,
            }] if kind.kind == TargetKind::Creature
        )
    ));
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

    let batch = engine.apply_scenario_command(2, &select_branch(1)).unwrap();
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
fn lavabrink_floodgates_missing_captured_upkeep_player_fails_before_program_instruction_zero() {
    let (mut engine, source) = setup(&[0, 1, 2]);
    set_counters(&mut engine, source, CounterKind::Doom, 3);
    start_upkeep(&mut engine, 2);
    engine
        .state
        .stack
        .last_mut()
        .expect("the current upkeep trigger is on the stack")
        .trigger_context
        .affected_player = None;

    let live_players = engine.state.players.iter().filter(|p| !p.has_lost).count();
    for _ in 0..live_players - 1 {
        let actor = engine.state.priority_player_id();
        engine
            .apply_scenario_command(actor, &pass())
            .expect("priority passes before the missing-input boundary");
    }
    let actor = engine.state.priority_player_id();
    let before = engine.diagnostic_snapshot().unwrap();
    let error = engine
        .apply_scenario_command(actor, &pass())
        .expect_err("the required captured actor is checked before the first Program choice");
    assert!(error
        .to_string()
        .contains("resolution program is missing its captured affected player"));
    assert_eq!(engine.diagnostic_snapshot().unwrap(), before);
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(engine.state.stack.len(), 1);
    assert_eq!(engine.state.objects[&source].zone, Zone::Battlefield);
    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::Doom),
        3
    );
}

#[test]
fn lavabrink_floodgates_delegates_before_resolution_when_upkeep_player_has_conceded() {
    let (mut engine, source) = setup(&[0, 1, 2, 3]);
    set_counters(&mut engine, source, CounterKind::Doom, 2);
    let p0_old_prompt_id = prime_upkeep_prompt_and_decline(&mut engine, source, 0);
    let p1_old_prompt_id = prime_upkeep_prompt_and_decline(&mut engine, source, 1);
    let creatures = [
        add_damage_test_creature(&mut engine, 0),
        add_damage_test_creature(&mut engine, 1),
        add_damage_test_creature(&mut engine, 3),
    ];
    start_upkeep(&mut engine, 2);

    engine.apply_scenario_command(2, &concede()).unwrap();
    let resolution_batch = pass_priority_round_with_batch(&mut engine);
    assert_eq!(
        resolution_batch
            .events
            .iter()
            .filter(|event| matches!(&event.ev, Some(Ev::ResolutionChoiceRequired(_))))
            .count(),
        1,
        "the resolution publishes exactly one initial delegate offer"
    );
    assert!(resolution_batch.events.iter().all(|event| match &event.ev {
        Some(Ev::ResolutionChoiceRequired(choice)) => choice.deciding_player_id != 2,
        _ => true,
    }));
    let selector = find_resolution_choice(&resolution_batch)
        .expect("the upkeep trigger should offer a delegated choice");
    assert_ne!(selector.resolution_prompt_id, 0);
    assert_eq!(
        selector.deciding_player_id, 0,
        "the ability controller delegates when the captured upkeep player left before resolution"
    );
    assert_eq!(
        selector
            .resolution_branches
            .iter()
            .map(|branch| (
                branch.branch_index,
                branch.label.as_str(),
                branch.selectable
            ))
            .collect::<Vec<_>>(),
        vec![
            (0, "P1 makes the choice", true),
            (1, "P3 makes the choice", true)
        ]
    );
    let delegate_prompt_id = selector.resolution_prompt_id;
    assert_ne!(delegate_prompt_id, p0_old_prompt_id);
    let snapshot = engine
        .diagnostic_snapshot()
        .expect("pending choice snapshot");
    let stage =
        &snapshot["state"]["pending_resolution"]["continuation"]["value"]["choose"]["stage"];
    assert_eq!(stage["variant"], "ChoosingDelegate");
    assert_eq!(stage["value"]["candidates"], serde_json::json!([1, 3]));

    reject(
        &mut engine,
        1,
        &select_branch_for_prompt(delegate_prompt_id, 0),
    );
    reject(
        &mut engine,
        0,
        &select_branch_for_prompt(p0_old_prompt_id, 0),
    );
    let delegated_menu_batch = engine
        .apply_scenario_command(0, &select_branch_for_prompt(delegate_prompt_id, 0))
        .expect("the controller assigns the choice to surviving player 1");
    let delegated_menu = find_resolution_choice(&delegated_menu_batch)
        .expect("the selected delegate receives the original resolution menu");
    assert_eq!(delegated_menu.deciding_player_id, 1);
    assert_ne!(delegated_menu.resolution_prompt_id, 0);
    assert_ne!(delegated_menu.resolution_prompt_id, p1_old_prompt_id);
    assert_ne!(
        (selector.deciding_player_id, selector.resolution_prompt_id),
        (
            delegated_menu.deciding_player_id,
            delegated_menu.resolution_prompt_id
        )
    );
    assert_eq!(
        delegated_menu
            .resolution_branches
            .iter()
            .map(|branch| (branch.branch_index, branch.selectable))
            .collect::<Vec<_>>(),
        vec![(0, true), (1, true)]
    );
    let original_menu_prompt_id = delegated_menu.resolution_prompt_id;

    reject(
        &mut engine,
        0,
        &select_branch_for_prompt(original_menu_prompt_id, 0),
    );
    reject(
        &mut engine,
        0,
        &select_branch_for_prompt(delegate_prompt_id, 0),
    );
    reject(
        &mut engine,
        1,
        &select_branch_for_prompt(p1_old_prompt_id, 0),
    );
    let chosen_add = engine
        .apply_scenario_command(1, &select_branch_for_prompt(original_menu_prompt_id, 0))
        .expect("the delegated upkeep player adds a doom counter");
    assert!(!chosen_add
        .events
        .iter()
        .any(|event| matches!(&event.ev, Some(Ev::ResolutionChoiceRequired(_)))));
    assert_eq!(engine.state.objects[&source].zone, Zone::Graveyard);
    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::Doom),
        0
    );
    assert_eq!(engine.state.stack.len(), 1);
    assert_eq!(
        engine.state.stack[0].controller, 0,
        "the reflexive trigger is a separate child controlled by the source controller"
    );
    assert!(engine.state.pending_resolution.is_none());

    resolve_all_players_pass(&mut engine);
    assert!(engine.state.stack.is_empty());
    assert!(engine.state.pending_resolution.is_none());
    for creature in creatures {
        assert_eq!(
            engine.state.objects[&creature].damage, 6,
            "the delegated parent creates and resolves one six-damage child"
        );
    }
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
    engine.apply_scenario_command(2, &select_branch(0)).unwrap();

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
fn lavabrink_floodgates_doubling_seasons_resume_to_threshold_and_one_reflexive_trigger() {
    let (mut engine, source) = setup(&[0, 1, 2]);
    inject_permanent_on_battlefield(&mut engine, 0, "doubling_season");
    inject_permanent_on_battlefield(&mut engine, 0, "doubling_season");
    set_counters(&mut engine, source, CounterKind::Doom, 0);
    let creatures = [
        add_damage_test_creature(&mut engine, 0),
        add_damage_test_creature(&mut engine, 1),
        add_damage_test_creature(&mut engine, 2),
    ];

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
    engine
        .apply_scenario_command(2, &select_branch(0))
        .expect("the upkeep player chooses to add a doom counter");

    let mut replacement_prompts = 0;
    while let Some(pending) = engine.state.pending_resolution.as_ref() {
        if pending.presentation.choice_kind != ChoiceKind::ReplacementEffect {
            break;
        }
        replacement_prompts += 1;
        assert!(replacement_prompts <= 2, "replacement effects do not loop");
        assert_eq!(pending.deciding_player, 0);
        let candidates = pending.presentation.candidates.clone();
        assert!(!candidates.is_empty());
        let valid = submit_resolution_choice(vec![candidates[0]]);
        if replacement_prompts == 1 {
            assert_eq!(candidates.len(), 2);
            let snapshot = engine.diagnostic_snapshot().unwrap();
            assert!(
                snapshot["state"]["pending_resolution"]["continuation"]["variant"]
                    == "CounterReplacement"
            );
            assert!(
                snapshot["state"]["pending_resolution"]["continuation"]["value"]["stack"]
                    ["program"]
                    .is_object()
            );
            reject(&mut engine, 1, &valid);
            let invalid = (0..=candidates.len() as u32)
                .find(|candidate| !candidates.contains(candidate))
                .expect("a third choice id is not a offered replacement");
            reject(&mut engine, 0, &submit_resolution_choice(vec![invalid]));
        }
        engine
            .apply_scenario_command(0, &valid)
            .expect("the source controller chooses one offered counter replacement");
    }
    assert!(replacement_prompts >= 1);
    assert_eq!(engine.state.objects[&source].zone, Zone::Graveyard);
    assert_eq!(
        engine.state.stack.len(),
        1,
        "the Program completes the threshold sacrifice and stages one separate reflexive trigger"
    );
    assert!(engine.state.pending_resolution.is_none());

    resolve_all_players_pass(&mut engine);
    for creature in creatures {
        assert_eq!(engine.state.objects[&creature].damage, 6);
    }
    assert!(engine.state.stack.is_empty());
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
            engine
                .apply_scenario_command(0, &select_branch(branch))
                .unwrap();
        } else {
            engine
                .apply_scenario_command(
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
    engine.apply_scenario_command(3, &select_branch(0)).unwrap();
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
        engine.apply_scenario_command(0, &decoded).unwrap(),
        replay.apply_scenario_command(0, &decoded).unwrap()
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
    engine.apply_scenario_command(0, &activation).unwrap();
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
        .apply_scenario_command(
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
    let p0_old_prompt_id = prime_upkeep_prompt_and_decline(&mut engine, source, 0);
    let p1_old_prompt_id = prime_upkeep_prompt_and_decline(&mut engine, source, 1);
    start_upkeep(&mut engine, 2);
    let original_menu_batch = pass_priority_round_with_batch(&mut engine);
    let original_menu = find_resolution_choice(&original_menu_batch)
        .expect("the live upkeep player receives the original menu");
    assert_eq!(original_menu.deciding_player_id, 2);
    assert_ne!(original_menu.resolution_prompt_id, 0);
    let original_prompt_id = original_menu.resolution_prompt_id;

    let delegate_batch = engine
        .apply_scenario_command(2, &concede())
        .expect("the current chooser can concede while its Program is parked");
    let selector = find_resolution_choice(&delegate_batch)
        .expect("the ability controller receives the delegate selector");
    assert_ne!(selector.resolution_prompt_id, 0);
    assert_ne!(selector.resolution_prompt_id, p0_old_prompt_id);
    assert_ne!(
        (original_menu.deciding_player_id, original_prompt_id),
        (selector.deciding_player_id, selector.resolution_prompt_id)
    );
    assert_eq!(
        selector.deciding_player_id, 0,
        "the ability controller chooses a delegate"
    );
    assert_eq!(
        selector
            .resolution_branches
            .iter()
            .map(|branch| (
                branch.branch_index,
                branch.label.as_str(),
                branch.selectable
            ))
            .collect::<Vec<_>>(),
        vec![
            (0, "P1 makes the choice", true),
            (1, "P3 makes the choice", true)
        ]
    );
    let snapshot = engine
        .diagnostic_snapshot()
        .expect("pending choice snapshot");
    let stage =
        &snapshot["state"]["pending_resolution"]["continuation"]["value"]["choose"]["stage"];
    assert_eq!(stage["variant"], "ChoosingDelegate");
    assert_eq!(stage["value"]["candidates"], serde_json::json!([1, 3]));

    let delegate_prompt_id = selector.resolution_prompt_id;
    reject(
        &mut engine,
        1,
        &select_branch_for_prompt(delegate_prompt_id, 0),
    );
    reject(
        &mut engine,
        0,
        &select_branch_for_prompt(p0_old_prompt_id, 0),
    );
    let delegated_menu_batch = engine
        .apply_scenario_command(0, &select_branch_for_prompt(delegate_prompt_id, 0))
        .expect("the controller assigns the choice to surviving player 1");
    let delegated_menu = find_resolution_choice(&delegated_menu_batch)
        .expect("the selected delegate receives the original resolution menu");
    assert_eq!(delegated_menu.deciding_player_id, 1);
    assert_ne!(delegated_menu.resolution_prompt_id, 0);
    assert_ne!(delegated_menu.resolution_prompt_id, p1_old_prompt_id);
    assert_ne!(
        (selector.deciding_player_id, selector.resolution_prompt_id),
        (
            delegated_menu.deciding_player_id,
            delegated_menu.resolution_prompt_id
        )
    );
    assert_eq!(
        delegated_menu
            .resolution_branches
            .iter()
            .map(|branch| (branch.branch_index, branch.selectable))
            .collect::<Vec<_>>(),
        vec![(0, true), (1, true)]
    );
    let original_menu_prompt_id = delegated_menu.resolution_prompt_id;
    reject(
        &mut engine,
        0,
        &select_branch_for_prompt(original_menu_prompt_id, 0),
    );
    reject(
        &mut engine,
        1,
        &select_branch_for_prompt(p1_old_prompt_id, 0),
    );
    let chosen_add = engine
        .apply_scenario_command(1, &select_branch_for_prompt(original_menu_prompt_id, 0))
        .expect("the delegated upkeep player adds a doom counter");
    assert!(!chosen_add
        .events
        .iter()
        .any(|event| matches!(&event.ev, Some(Ev::ResolutionChoiceRequired(_)))));
    assert_eq!(engine.state.objects[&source].zone, Zone::Graveyard);
    assert_eq!(engine.state.stack.len(), 1);
    assert_eq!(
        engine.state.stack[0].controller, 0,
        "the reflexive child remains controlled by the source controller"
    );
    resolve_all_players_pass(&mut engine);
    assert!(engine.state.stack.is_empty());
    assert!(engine.state.pending_resolution.is_none());
}
