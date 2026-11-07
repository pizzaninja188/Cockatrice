use crate::helpers::ScenarioCommandExt;
use crate::helpers::*;
use prost::Message;

// Wire-level regression: older engines decode the proposed command as absent and reject it.
// This proves the missing behavior before adding the automation protocol implementation.
fn automation_command(operation: u8, enabled: bool) -> RuledCommand {
    RuledCommand::decode([0xc2, 0x02, 0x02, operation << 3, u8::from(enabled)].as_slice())
        .expect("well-formed automation command")
}

fn engine() -> GameEngine {
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        9911,
        &[4, 9, 27],
        20,
        Some(vec![vec!["forest".into(); 20]; 3]),
        true,
    )
    .expect("engine");
    advance_to_main1_from_game_start(&mut e);
    e
}

#[test]
fn clear_all_automation_is_accepted_without_priority_and_does_not_pass() {
    let mut e = engine();
    let priority = e.state.priority_player_id();
    let step = e.state.turn_step;
    e.apply_scenario_command(9, &automation_command(1, true))
        .expect("any seated player can stop their own automation");
    assert_eq!(e.state.priority_player_id(), priority);
    assert_eq!(e.state.turn_step, step);
}

#[test]
fn full_control_overrides_the_existing_phase_auto_pass_policy() {
    let mut e = engine();
    e.apply_scenario_command(9, &automation_command(3, true))
        .expect("enable full control");
    let policies = [4, 9, 27]
        .map(|player_id| tricerules_proto::ruled::v1::AutoPassPolicy {
            player_id,
            stop_on_own_turn: vec![],
            stop_on_opponent_turn: vec![],
        })
        .to_vec();
    let command = RuledCommand {
        cmd: Some(Cmd::CanonicalGameplay(
            tricerules_proto::ruled::v1::CanonicalGameplayCommand {
                command: pass().encode_to_vec(),
                auto_pass_policies: policies,
            },
        )),
    };
    e.apply_scenario_command(4, &command).expect("manual pass");
    assert_eq!(e.state.priority_player_id(), 9);
}
use rv1::update_automation::Action;
use rv1::AutomationYieldMode as Mode;
use tricerules_proto::ruled::v1 as rv1;

fn update(action: Action) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::UpdateAutomation(rv1::UpdateAutomation {
            action: Some(action),
        })),
    }
}

fn yield_to(mode: Mode) -> RuledCommand {
    update(Action::YieldTo(rv1::AutomationYield {
        mode: mode as i32,
        ..Default::default()
    }))
}

fn snapshot(batch: &RuledEventBatch, player: i32) -> &rv1::AutomationState {
    batch.legal_by_player[&player].automation.as_ref().unwrap()
}

fn next(e: &mut GameEngine, batch: &RuledEventBatch) -> RuledEventBatch {
    let token = batch
        .automation_continuation
        .as_ref()
        .expect("authorized continuation");
    e.apply_scenario_command(
        token.player_id,
        &RuledCommand {
            cmd: Some(Cmd::ContinueAutomation(rv1::ContinueAutomation {
                expected_command_index: token.expected_command_index,
                expected_revision: token.expected_revision,
            })),
        },
    )
    .unwrap()
}

fn spell(e: &mut GameEngine, index: usize, id: &str) -> RuledEventBatch {
    inject_card_into_hand(e, index, id);
    grant_pool(e, index);
    e.apply_scenario_command(
        e.state.players[index].id,
        &cast_spell(hand_index_for_card(e, index, id), vec![]),
    )
    .unwrap()
}

#[test]
fn continuation_passes_one_seat_and_clear_invalidates_the_token() {
    let mut e = engine();
    let armed = e.apply_scenario_command(4, &yield_to(Mode::Turn)).unwrap();
    assert_eq!(e.state.priority_player_id(), 4, "arming does not pass");
    let token = *armed.automation_continuation.as_ref().unwrap();
    next(&mut e, &armed);
    assert_eq!(e.state.priority_player_id(), 9, "never pass another seat");
    let cleared = e
        .apply_scenario_command(4, &update(Action::ClearAll(true)))
        .unwrap();
    assert!(snapshot(&cleared, 4).full_control);
    assert!(cleared.automation_continuation.is_none());
    assert!(e
        .apply_scenario_command(
            4,
            &RuledCommand {
                cmd: Some(Cmd::ContinueAutomation(rv1::ContinueAutomation {
                    expected_command_index: token.expected_command_index,
                    expected_revision: token.expected_revision,
                }))
            }
        )
        .is_err());
}

#[test]
fn response_interrupts_soft_yield_but_not_pass_turn() {
    for (mode, interrupted) in [(Mode::UntilResponse, true), (Mode::Turn, false)] {
        let mut e = engine();
        let b = e.apply_scenario_command(4, &yield_to(mode)).unwrap();
        next(&mut e, &b);
        let b = spell(&mut e, 1, "opt");
        assert_eq!(snapshot(&b, 4).paused, interrupted);
        assert_eq!(snapshot(&b, 4).yield_mode == Mode::None as i32, interrupted);
    }
}

#[test]
fn current_stack_interrupts_new_objects_but_entire_stack_accepts_them() {
    for (mode, interrupted) in [(Mode::CurrentStack, true), (Mode::EntireStack, false)] {
        let mut e = engine();
        spell(&mut e, 0, "grizzly_bears");
        let b = e.apply_scenario_command(4, &yield_to(mode)).unwrap();
        next(&mut e, &b);
        let b = spell(&mut e, 1, "opt");
        assert_eq!(snapshot(&b, 4).paused, interrupted);
    }
}

#[test]
fn entire_stack_finishes_at_empty_without_advancing_the_phase() {
    let mut e = engine();
    spell(&mut e, 0, "grizzly_bears");
    let step = e.state.turn_step;
    let b = e
        .apply_scenario_command(4, &yield_to(Mode::EntireStack))
        .unwrap();
    next(&mut e, &b);
    e.apply_scenario_command(9, &pass()).unwrap();
    let b = e.apply_scenario_command(27, &pass()).unwrap();
    assert!(e.state.stack.is_empty());
    assert_eq!(e.state.turn_step, step);
    assert_eq!(e.state.priority_player_id(), 4);
    assert!(snapshot(&b, 4).paused);
    assert!(b.automation_continuation.is_none());
}

#[test]
fn exact_item_yield_does_not_skip_an_unrelated_top_item() {
    let mut e = engine();
    let b = spell(&mut e, 0, "grizzly_bears");
    let reference = snapshot(&b, 4).stack_offers[0].stack;
    e.apply_scenario_command(4, &pass()).unwrap();
    spell(&mut e, 1, "opt");
    e.apply_scenario_command(9, &pass()).unwrap();
    e.apply_scenario_command(27, &pass()).unwrap();
    let b = e
        .apply_scenario_command(
            4,
            &update(Action::YieldTo(rv1::AutomationYield {
                mode: Mode::StackItem as i32,
                stack: reference,
                ..Default::default()
            })),
        )
        .unwrap();
    assert!(b.automation_continuation.is_none());
    assert_eq!(e.state.stack.len(), 2);
}

#[test]
fn mandatory_resolution_choice_pauses_and_answer_does_not_resume_yield() {
    let mut e = engine();
    spell(&mut e, 0, "opt");
    let b = e.apply_scenario_command(4, &yield_to(Mode::Turn)).unwrap();
    next(&mut e, &b);
    e.apply_scenario_command(9, &pass()).unwrap();
    let b = e.apply_scenario_command(27, &pass()).unwrap();
    assert!(e.state.pending_resolution.is_some());
    assert!(snapshot(&b, 4).paused);
    let prompt = e
        .state
        .pending_resolution
        .as_ref()
        .unwrap()
        .resolution_prompt_id
        .get();
    let b = e
        .apply_scenario_command(
            4,
            &RuledCommand {
                cmd: Some(Cmd::SubmitResolutionChoice(rv1::SubmitResolutionChoice {
                    resolution_prompt_id: prompt,
                    decision: rv1::ResolutionChoiceDecision::Unspecified as i32,
                    ..Default::default()
                })),
            },
        )
        .unwrap();
    assert!(snapshot(&b, 4).paused);
    assert!(b.automation_continuation.is_none());
}

#[test]
fn end_step_yield_armed_at_end_step_stops_immediately() {
    let mut e = engine();
    e.state.turn_step = tricerules_core::TurnStep::EndStep;
    let b = e
        .apply_scenario_command(4, &yield_to(Mode::EndStep))
        .unwrap();
    assert!(b.automation_continuation.is_none());
    assert_eq!(snapshot(&b, 4).yield_mode, Mode::None as i32);
}

#[test]
fn smart_auto_pass_keeps_priority_for_uncertain_spell_payment() {
    let mut e = engine();
    inject_card_into_hand(&mut e, 0, "grizzly_bears");
    let command = update(Action::AutoPass(true));
    let b = e
        .apply_scenario_command(
            4,
            &RuledCommand {
                cmd: Some(Cmd::CanonicalGameplay(rv1::CanonicalGameplayCommand {
                    command: command.encode_to_vec(),
                    auto_pass_policies: [4, 9, 27]
                        .map(|player_id| rv1::AutoPassPolicy {
                            player_id,
                            ..Default::default()
                        })
                        .to_vec(),
                })),
            },
        )
        .unwrap();
    assert!(snapshot(&b, 4).auto_pass);
    assert!(
        b.automation_continuation.is_none(),
        "missing affordable hints never prove no action"
    );
}

fn configure_trigger(
    reference: rv1::AutomationStackRef,
    scope: rv1::AutomationTriggerScope,
    yield_priority: bool,
    answer: rv1::AutomationAnswer,
) -> RuledCommand {
    update(Action::TriggerRule(rv1::ConfigureTriggerAutomation {
        stack: Some(reference),
        scope: scope as i32,
        yield_priority,
        answer: answer as i32,
    }))
}

fn optional_draw_trigger(e: &mut GameEngine) -> rv1::AutomationStackRef {
    inject_permanent_on_battlefield(e, 0, "garruks_packleader");
    spell(e, 0, "hill_giant");
    pass_priority_round(e);
    assert_eq!(e.state.stack.len(), 1);
    let b = e.initial_response_batch();
    let offer = &snapshot(&b, 4).stack_offers[0];
    assert!(offer.can_yield_trigger && offer.can_answer_optional);
    offer.stack.unwrap()
}

#[test]
fn optional_trigger_answers_are_separate_from_yielding_and_use_the_current_prompt() {
    for answer in [rv1::AutomationAnswer::Yes, rv1::AutomationAnswer::No] {
        let mut e = engine();
        let reference = optional_draw_trigger(&mut e);
        let hand = e.state.players[0].hand.len();
        let b = e
            .apply_scenario_command(
                4,
                &configure_trigger(
                    reference,
                    rv1::AutomationTriggerScope::Source,
                    false,
                    answer,
                ),
            )
            .unwrap();
        assert!(
            b.automation_continuation.is_none(),
            "remembering an answer does not pass priority"
        );
        e.apply_scenario_command(4, &pass()).unwrap();
        e.apply_scenario_command(9, &pass()).unwrap();
        let b = e.apply_scenario_command(27, &pass()).unwrap();
        assert!(snapshot(&b, 4).can_answer_no);
        assert_eq!(snapshot(&b, 4).yes_branch_index, Some(0));
        assert!(
            !snapshot(&b, 4).paused,
            "configured answer survives publication of its prompt id"
        );
        next(&mut e, &b);
        assert!(e.state.pending_resolution.is_none());
        assert_eq!(
            e.state.players[0].hand.len(),
            hand + usize::from(answer == rv1::AutomationAnswer::Yes)
        );
    }
}

#[test]
fn trigger_rules_use_origin_controller_and_source_incarnation_not_text() {
    let mut e = engine();
    let reference = optional_draw_trigger(&mut e);
    e.apply_scenario_command(
        4,
        &configure_trigger(
            reference,
            rv1::AutomationTriggerScope::Source,
            true,
            rv1::AutomationAnswer::Ask,
        ),
    )
    .unwrap();
    e.apply_scenario_command(
        4,
        &configure_trigger(
            reference,
            rv1::AutomationTriggerScope::MatchingAbility,
            true,
            rv1::AutomationAnswer::Ask,
        ),
    )
    .unwrap();
    let original = e.state.stack[0].clone();
    let key = original.trigger_use_key.clone().unwrap();
    // A second source of the same definition matches only the explicitly broader rule.
    e.state.stack[0].trigger_use_key.as_mut().unwrap().object_id += 10_000;
    let b = e.initial_response_batch();
    assert_eq!(snapshot(&b, 4).stack_offers[0].source_rule_id, 0);
    assert_ne!(snapshot(&b, 4).stack_offers[0].matching_rule_id, 0);
    e.state.stack[0].controller = 9;
    let b = e.initial_response_batch();
    assert_eq!(snapshot(&b, 4).stack_offers[0].matching_rule_id, 0);
    e.state.stack[0] = original.clone();
    e.state.stack[0]
        .trigger_use_key
        .as_mut()
        .unwrap()
        .ability_origin = tricerules_core::state::TriggerAbilityOrigin::ResolvingGrant {
        occurrence: 55_000,
        definition: None,
    };
    let b = e.initial_response_batch();
    assert_eq!(
        snapshot(&b, 4).stack_offers[0].matching_rule_id,
        0,
        "identical display text cannot match a different ability"
    );
    e.state.stack[0] = original;
    e.state
        .zone_change_generation
        .insert(key.object_id, key.zone_change_generation + 1);
    let b = e
        .apply_scenario_command(4, &update(Action::FullControl(true)))
        .unwrap();
    assert_eq!(
        snapshot(&b, 4).rules.len(),
        1,
        "a zone change expires only the source-specific rule"
    );
    assert!(snapshot(&b, 4).full_control);
    assert!(b.automation_continuation.is_none());
    let b = e
        .apply_scenario_command(4, &update(Action::FullControl(false)))
        .unwrap();
    assert_eq!(
        snapshot(&b, 4).rules.len(),
        1,
        "full control preserves saved rules"
    );
    assert!(b.automation_continuation.is_some());
}

#[test]
fn matching_trigger_below_unrelated_spell_never_passes_over_that_spell() {
    let mut e = engine();
    let reference = optional_draw_trigger(&mut e);
    e.apply_scenario_command(
        4,
        &configure_trigger(
            reference,
            rv1::AutomationTriggerScope::Source,
            true,
            rv1::AutomationAnswer::Ask,
        ),
    )
    .unwrap();
    // A deliberate response keeps the persistent rule, but it only applies at the top.
    let b = spell(&mut e, 0, "opt");
    assert_eq!(e.state.stack.len(), 2);
    assert!(b.automation_continuation.is_none());
}

#[test]
fn one_time_stop_cancels_yield_and_is_consumed() {
    let mut e = engine();
    e.apply_scenario_command(
        4,
        &update(Action::StopOnce(rv1::AutomationPhaseStop {
            phase: rv1::PhaseId::BeginCombat as i32,
            on_own_turn: true,
        })),
    )
    .unwrap();
    let b = e.apply_scenario_command(4, &yield_to(Mode::Turn)).unwrap();
    next(&mut e, &b);
    e.apply_scenario_command(9, &pass()).unwrap();
    let b = e.apply_scenario_command(27, &pass()).unwrap();
    assert_eq!(e.state.turn_step, tricerules_core::TurnStep::BeginCombat);
    assert!(snapshot(&b, 4).paused);
    assert!(snapshot(&b, 4).stop_once.is_none());
    assert!(b.automation_continuation.is_none());
}

#[test]
fn turn_yield_expires_on_actual_turn_boundary_and_disconnect_pause_retains_preferences() {
    let mut e = engine();
    e.state.turn_step = tricerules_core::TurnStep::EndStep;
    for player in [4, 9, 27] {
        e.state
            .players
            .iter_mut()
            .find(|p| p.id == player)
            .unwrap()
            .hand
            .clear();
    }
    let b = e.apply_scenario_command(4, &yield_to(Mode::Turn)).unwrap();
    next(&mut e, &b);
    e.apply_scenario_command(9, &pass()).unwrap();
    let b = e.apply_scenario_command(27, &pass()).unwrap();
    assert!(snapshot(&b, 4).paused);
    assert_eq!(snapshot(&b, 4).yield_mode, Mode::None as i32);
    e.apply_scenario_command(4, &update(Action::AutoPass(true)))
        .unwrap();
    let b = e
        .apply_scenario_command(4, &update(Action::Pause(true)))
        .unwrap();
    assert!(snapshot(&b, 4).auto_pass);
    assert!(snapshot(&b, 4).paused);
    assert!(b.automation_continuation.is_none());
}

#[test]
fn smart_auto_pass_proves_tapped_mana_sources_unavailable_but_respects_stops() {
    for (tapped, stop, should_pass) in [
        (false, false, false),
        (true, false, true),
        (true, true, false),
    ] {
        let mut e = engine();
        e.state.players[0].hand.clear();
        let forest = inject_permanent_on_battlefield(&mut e, 0, "forest");
        e.state.objects.get_mut(&forest).unwrap().tapped = tapped;
        let command = update(Action::AutoPass(true));
        let policies = [4, 9, 27]
            .map(|player_id| rv1::AutoPassPolicy {
                player_id,
                stop_on_own_turn: if stop || player_id != 4 {
                    vec![rv1::PhaseId::Main1 as i32]
                } else {
                    vec![]
                },
                stop_on_opponent_turn: vec![rv1::PhaseId::Main1 as i32],
            })
            .to_vec();
        let b = e
            .apply_scenario_command(
                4,
                &RuledCommand {
                    cmd: Some(Cmd::CanonicalGameplay(rv1::CanonicalGameplayCommand {
                        command: command.encode_to_vec(),
                        auto_pass_policies: policies,
                    })),
                },
            )
            .unwrap();
        assert_eq!(
            b.automation_continuation.is_some(),
            should_pass,
            "tapped={tapped} phase stop={stop}"
        );
    }
}

#[test]
fn exact_stack_yield_rejects_an_old_physical_spell_generation() {
    let mut e = engine();
    let b = spell(&mut e, 0, "grizzly_bears");
    let mut reference = snapshot(&b, 4).stack_offers[0].stack.unwrap();
    reference.zone_change_generation = Some(reference.zone_change_generation.unwrap() + 1);
    assert!(e
        .apply_scenario_command(
            4,
            &update(Action::YieldTo(rv1::AutomationYield {
                mode: Mode::StackItem as i32,
                stack: Some(reference),
                ..Default::default()
            }))
        )
        .is_err());
}

#[test]
fn automation_preferences_participate_in_replay_state_and_replay_identically() {
    let mut live = engine();
    let mut replay = engine();
    let commands = [
        update(Action::AutoPass(true)),
        yield_to(Mode::Turn),
        update(Action::FullControl(true)),
        update(Action::ClearAll(true)),
    ];
    for command in commands {
        let live_batch = live.apply_scenario_command(4, &command).unwrap();
        let replay_batch = replay.apply_scenario_command(4, &command).unwrap();
        assert_eq!(snapshot(&live_batch, 4), snapshot(&replay_batch, 4));
        let state = serde_json::to_value(&live.state).unwrap();
        assert!(state["automation"]["4"].is_object());
        assert_eq!(state, serde_json::to_value(&replay.state).unwrap());
    }
}

#[test]
fn resolving_optional_trigger_remains_available_to_configure_its_answer() {
    let mut e = engine();
    let reference = optional_draw_trigger(&mut e);
    pass_priority_round(&mut e);
    let b = e.initial_response_batch();
    let offer = snapshot(&b, 4)
        .stack_offers
        .iter()
        .find(|offer| offer.stack.as_ref() == Some(&reference));
    assert!(
        offer.is_some_and(|offer| offer.can_answer_optional),
        "the visible resolving trigger must retain its menu offer"
    );
    let b = e
        .apply_scenario_command(
            4,
            &configure_trigger(
                reference,
                rv1::AutomationTriggerScope::Source,
                false,
                rv1::AutomationAnswer::No,
            ),
        )
        .unwrap();
    next(&mut e, &b);
    assert!(e.state.pending_resolution.is_none());
}

#[test]
fn source_rule_cannot_silently_register_after_its_incarnation_has_left() {
    let mut e = engine();
    let reference = optional_draw_trigger(&mut e);
    let key = e.state.stack[0].trigger_use_key.clone().unwrap();
    e.state
        .zone_change_generation
        .insert(key.object_id, key.zone_change_generation + 1);
    assert!(e
        .apply_scenario_command(
            4,
            &configure_trigger(
                reference,
                rv1::AutomationTriggerScope::Source,
                true,
                rv1::AutomationAnswer::Ask
            )
        )
        .is_err());
    e.apply_scenario_command(
        4,
        &configure_trigger(
            reference,
            rv1::AutomationTriggerScope::MatchingAbility,
            true,
            rv1::AutomationAnswer::Ask,
        ),
    )
    .unwrap();
}

#[test]
fn remembered_outer_optional_trigger_answer_does_not_answer_another_nested_binary_gate() {
    use tricerules_card_model::primitives::{PlayerRecipient, ResolutionBranchDef, ResolutionCost};
    use tricerules_card_model::{AbilityPresentation, ChoiceId};
    use tricerules_core::state::{PendingResolutionBranch, PendingResolutionBranchStage};
    let mut e = engine();
    let reference = optional_draw_trigger(&mut e);
    e.apply_scenario_command(
        4,
        &configure_trigger(
            reference,
            rv1::AutomationTriggerScope::Source,
            false,
            rv1::AutomationAnswer::Yes,
        ),
    )
    .unwrap();
    pass_priority_round(&mut e);
    // Defensive prepared-state coverage: a later independent free binary gate on the same
    // captured trigger must not inherit authorization for its outer 'may' decision.
    let pending = e.state.pending_resolution.as_mut().unwrap();
    let ResolutionContinuation::OptionalTriggeredAbility { stack, .. } =
        pending.continuation.clone()
    else {
        panic!("outer optional gate")
    };
    pending.continuation = ResolutionContinuation::AuthoredBranch {
        stack,
        branch: PendingResolutionBranch {
            optional: true,
            chooser: PlayerRecipient::Controller,
            original_chooser: 4,
            branches: vec![ResolutionBranchDef {
                branch_id: ChoiceId::new("nested_gate").unwrap(),
                presentation: AbilityPresentation::Fallback,
                runtime_fallback: Some("Independent option".into()),
                cost: ResolutionCost::None,
                requirement: Default::default(),
                effects: vec![],
            }],
            stage: PendingResolutionBranchStage::Selecting,
        },
    };
    let b = e
        .apply_scenario_command(4, &update(Action::FullControl(false)))
        .unwrap();
    assert!(
        snapshot(&b, 4).can_answer_no,
        "manual binary buttons remain available"
    );
    assert!(
        b.automation_continuation.is_none(),
        "saved outer answer must not answer a different choice"
    );
}

#[test]
fn direct_engine_commands_reconcile_current_phase_stops() {
    let mut e = engine();
    let batch = e
        .apply_command(
            4,
            &update(Action::StopOnce(rv1::AutomationPhaseStop {
                phase: rv1::PhaseId::Main1 as i32,
                on_own_turn: true,
            })),
        )
        .unwrap();
    assert!(snapshot(&batch, 4).paused);
    assert!(snapshot(&batch, 4).stop_once.is_none());
}

#[test]
fn configuring_yield_during_an_unanswered_choice_keeps_automation_paused() {
    let mut e = engine();
    let reference = optional_draw_trigger(&mut e);
    pass_priority_round(&mut e);
    let batch = e
        .apply_scenario_command(
            4,
            &configure_trigger(
                reference,
                rv1::AutomationTriggerScope::Source,
                true,
                rv1::AutomationAnswer::Ask,
            ),
        )
        .unwrap();
    assert!(
        snapshot(&batch, 4).paused,
        "a preference update does not answer or bypass the open choice"
    );
    let batch = e
        .apply_scenario_command(
            4,
            &RuledCommand {
                cmd: Some(Cmd::SubmitResolutionChoice(rv1::SubmitResolutionChoice {
                    decision: rv1::ResolutionChoiceDecision::Decline as i32,
                    ..Default::default()
                })),
            },
        )
        .unwrap();
    assert!(
        snapshot(&batch, 4).paused,
        "answering the required choice does not resume automation"
    );
}
