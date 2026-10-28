use super::{EffectCx, EffectOutcome};
use crate::engine::events::{ev_log, object_display_name};
use crate::engine::presentation::{
    ability_presentation, stack_child_presentation_ref, PresentationPath, StackPresentationSource,
};
use crate::engine::targeting::{counter_move_target_pair_is_current, TargetSourceIdentity};
use crate::engine::{rv1, AmountContext, ConditionContext, EngineError};
use crate::state::{
    ParkedStackResolution, PendingChosenCounterAddition, PendingChosenCounterAdditionStage,
    PendingCounterMove, PendingResolution, PendingResolutionBranch, PendingResolutionBranchStage,
    PendingResolutionPresentation, PendingTargetedPlayerChoiceStage, ProgramValue,
    ResolutionContinuation, StackItem, StackTarget, StagedTrigger, StagedTriggerGroup,
    TargetedPlayerControlCohort, TriggerContext, TriggerObjectRef,
};
use crate::Zone;
use crate::{GameEngine, ObjectId, PlayerId};
use tricerules_card_model::primitives::{
    Amount, BattlefieldQuantityCharacteristic, CardResultAction, CounterKind,
    PermanentChoiceConstraint, PlayerRecipient, RelativePlayerSet, ResolutionBranchDef,
    ResolutionBranchRequirement, ResolutionCost, ResultType, SpellEffectKind, TargetController,
    TargetFilter, TargetKind, TriggerCondition, TriggeredAbilityDef,
};

/// Current positive counter kinds on the exact target incarnation that passed the initial
/// resolution target check. Deliberately does not repeat targetability or controller checks: CR
/// 608.2b made those checks when resolution began, before this choice was opened.
pub(in crate::engine) fn chosen_counter_addition_target_counter_kinds(
    engine: &GameEngine,
    target: &StackTarget,
) -> Option<Vec<CounterKind>> {
    let expected_generation = target.zone_change_generation?;
    if target.group_index != 0
        || engine
            .state
            .zone_change_generation
            .get(&target.object_id)
            .copied()
            .unwrap_or(0)
            != expected_generation
    {
        return None;
    }
    let object = engine.state.objects.get(&target.object_id)?;
    if object.zone != Zone::Battlefield {
        return None;
    }
    Some(
        object
            .counters
            .iter()
            .filter_map(|(&kind, &count)| (count > 0).then_some(kind))
            .collect(),
    )
}

pub(super) fn add_one_counter_of_chosen_kind_to_target(
    cx: &mut EffectCx<'_>,
) -> Result<EffectOutcome, EngineError> {
    let targets = cx
        .target_receipts
        .iter()
        .filter(|target| target.group_index == 0)
        .copied()
        .collect::<Vec<_>>();
    let target = match targets.as_slice() {
        [] => return Ok(EffectOutcome::Continue),
        [target] => *target,
        _ => {
            return Err(EngineError::Illegal(
                "chosen-counter addition requires one target receipt",
            ))
        }
    };
    let Some(offered_counter_kinds) =
        chosen_counter_addition_target_counter_kinds(cx.engine, &target)
    else {
        return Ok(EffectOutcome::Continue);
    };
    if offered_counter_kinds.is_empty() {
        return Ok(EffectOutcome::Continue);
    }

    let prompt = "Choose a kind of counter on the target permanent.".to_string();
    cx.engine.state.pending_resolution = Some(PendingResolution {
        resolution_prompt_id: crate::state::ResolutionPromptId::UNASSIGNED,
        resolution_prompt_semantics: None,
        deciding_player: cx.controller,
        presentation: PendingResolutionPresentation {
            source_object_id: cx.top.id,
            candidates: Vec::new(),
            min: 1,
            max: 1,
            ordered: false,
            prompt: prompt.clone(),
            choice_kind: rv1::ChoiceKind::ResolutionBranch,
            unique_names: false,
        },
        continuation: ResolutionContinuation::ChosenCounterAddition {
            stack: cx.parked_stack(),
            counter_addition: PendingChosenCounterAddition {
                target,
                offered_counter_kinds,
                stage: PendingChosenCounterAdditionStage::Selecting,
            },
        },
    });
    cx.events.push(
        cx.engine
            .chosen_counter_addition_choice_event()
            .expect("the chosen-counter addition prompt was parked"),
    );
    cx.events.push(ev_log(prompt));
    Ok(EffectOutcome::Suspended)
}

pub(super) fn move_one_counter_between_targets(
    cx: &mut EffectCx<'_>,
) -> Result<EffectOutcome, EngineError> {
    let source_targets = cx
        .top
        .targets
        .iter()
        .filter(|target| target.group_index == 0)
        .copied()
        .collect::<Vec<_>>();
    let destination_targets = cx
        .top
        .targets
        .iter()
        .filter(|target| target.group_index == 1)
        .copied()
        .collect::<Vec<_>>();
    let ([source_target], [destination_target]) =
        (source_targets.as_slice(), destination_targets.as_slice())
    else {
        // CR 608.2b permits the ability to resolve when one target remains legal; the move
        // itself requires both. The resolution snapshot has already removed illegal targets.
        return Ok(EffectOutcome::Continue);
    };
    let source_target = *source_target;
    let destination_target = *destination_target;
    if !counter_move_target_pair_is_current(cx.engine, cx.top, &source_target, &destination_target)
    {
        return Ok(EffectOutcome::Continue);
    }
    let source = source_target.object_id;
    let destination = destination_target.object_id;
    let offered_counter_kinds = cx
        .engine
        .state
        .objects
        .get(&source)
        .filter(|object| object.zone == Zone::Battlefield)
        .map(|object| {
            object
                .counters
                .iter()
                .filter_map(|(&kind, &count)| (count > 0).then_some(kind))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let Some(first_kind) = offered_counter_kinds.first().copied() else {
        return Ok(EffectOutcome::Continue);
    };
    // Permission and the replacement path do not depend on the selected counter kind in the
    // current rules vocabulary. Preflight before prompting so an impossible move is a no-op.
    if !cx
        .engine
        .counter_move_can_complete(source, destination, first_kind)
    {
        return Ok(EffectOutcome::Continue);
    }

    let stack = ParkedStackResolution::new(cx.top.clone())
        .with_previous_result(cx.previous_effect_result.clone());
    let prompt = "Choose a counter to move.".to_string();
    cx.engine.state.pending_resolution = Some(PendingResolution {
        resolution_prompt_id: crate::state::ResolutionPromptId::UNASSIGNED,
        resolution_prompt_semantics: None,

        deciding_player: cx.controller,
        presentation: PendingResolutionPresentation {
            source_object_id: cx.top.id,
            candidates: Vec::new(),
            min: 1,
            max: 1,
            ordered: false,
            prompt: prompt.clone(),
            choice_kind: rv1::ChoiceKind::ResolutionBranch,
            unique_names: false,
        },
        continuation: ResolutionContinuation::CounterMove {
            stack,
            counter_move: PendingCounterMove {
                source_target,
                destination_target,
                offered_counter_kinds: offered_counter_kinds.clone(),
            },
        },
    });
    let options = offered_counter_kinds
        .iter()
        .enumerate()
        .map(|(index, kind)| {
            let label = format!("Move one {} counter", kind.label());
            rv1::ResolutionBranchOption {
                branch_index: index as u32,
                label: label.clone(),
                cost_kind: rv1::ResolutionBranchCostKind::Unspecified as i32,
                cost_text: String::new(),
                selectable: true,
                search_zones: Vec::new(),
                presentation: Some(rv1::PresentationRef {
                    fallback_text: label,
                    ..Default::default()
                }),
            }
        })
        .collect();
    cx.events.push(rv1::RuledEvent {
        ev: Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(
            rv1::ResolutionChoiceRequired {
                resolution_prompt_id: 0,
                deciding_player_id: cx.controller,
                source_object_id: cx.top.id,
                prompt_text: prompt.clone(),
                choice_kind: rv1::ChoiceKind::ResolutionBranch as i32,
                min: 1,
                max: 1,
                resolution_branches: options,
                ..Default::default()
            },
        )),
    });
    cx.events.push(ev_log(prompt));
    Ok(EffectOutcome::Suspended)
}

/// Snapshot a player's current control set without consulting departure receipts. Reconciliation
/// calls this before cleanup even though `has_lost` is already set on the departing player.
pub(in crate::engine) fn current_targeted_player_control_cohort(
    engine: &GameEngine,
    player_id: PlayerId,
) -> crate::state::TargetedPlayerControlCohort {
    let permanents = engine
        .state
        .players
        .iter()
        .flat_map(|player| player.battlefield.iter().copied())
        .filter_map(|object_id| {
            let characteristics = engine.characteristics(object_id)?;
            (characteristics.controller == player_id).then(|| {
                let object = engine.state.objects.get(&object_id)?;
                Some(crate::state::TargetedPlayerPermanent {
                    object_id,
                    zone_change_generation: engine
                        .state
                        .zone_change_generation
                        .get(&object_id)
                        .copied()
                        .unwrap_or(0),
                    owner: object.owner,
                })
            })?
        })
        .collect();
    crate::state::TargetedPlayerControlCohort {
        player_id,
        permanents,
    }
}

/// Enumerate the current legal permanent choices for a living targeted player. A departed
/// target instead uses the last-known control cohort captured by player-departure reconciliation.
pub(in crate::engine) fn current_targeted_player_permanent_candidates(
    engine: &GameEngine,
    item: &StackItem,
    target_player: PlayerId,
    filter: &TargetFilter,
    constraints: &[PermanentChoiceConstraint],
) -> Vec<ObjectId> {
    let source = TargetSourceIdentity::for_stack_item(engine, item);
    let condition_context = ConditionContext::for_stack_item(item);
    engine
        .state
        .players
        .iter()
        .flat_map(|player| player.battlefield.iter().copied())
        .filter(|object_id| {
            crate::engine::targeting::permanent_choice_filter_legal(
                engine,
                filter,
                *object_id,
                target_player,
                source,
                item.trigger_context,
            ) && constraints.iter().all(|constraint| match constraint {
                PermanentChoiceConstraint::EquipmentAttachableTo { recipient } => engine
                    .condition_object_identity(recipient.clone(), condition_context)
                    .is_some_and(|(recipient_id, expected_generation)| {
                        engine
                            .state
                            .zone_change_generation
                            .get(&recipient_id)
                            .copied()
                            .unwrap_or(0)
                            == expected_generation
                            && crate::engine::targeting::equipment_attachment_legal(
                                engine,
                                *object_id,
                                recipient_id,
                            )
                    }),
            })
        })
        .collect()
}

/// Enumerate exact surviving members of a departed targeted player's control cohort that still
/// satisfy the choice's characteristic, owner, exclusion, and auxiliary constraints.
pub(in crate::engine) fn targeted_player_control_cohort_candidates(
    engine: &GameEngine,
    item: &StackItem,
    target_player: PlayerId,
    cohort: &TargetedPlayerControlCohort,
    filter: &TargetFilter,
    constraints: &[PermanentChoiceConstraint],
) -> Vec<(ObjectId, u64)> {
    if cohort.player_id != target_player {
        return Vec::new();
    }
    let source = TargetSourceIdentity::for_stack_item(engine, item);
    let condition_context = ConditionContext::for_stack_item(item);
    cohort
        .permanents
        .iter()
        .filter_map(|member| {
            let current_generation = engine
                .state
                .zone_change_generation
                .get(&member.object_id)
                .copied()
                .unwrap_or(0);
            if current_generation != member.zone_change_generation
                || !engine
                    .state
                    .objects
                    .get(&member.object_id)
                    .is_some_and(|object| object.zone == Zone::Battlefield)
                || !crate::engine::targeting::permanent_choice_filter_legal_for_control_cohort(
                    engine,
                    filter,
                    member.object_id,
                    target_player,
                    source,
                    item.trigger_context,
                )
            {
                return None;
            }
            let constraints_hold = constraints.iter().all(|constraint| match constraint {
                PermanentChoiceConstraint::EquipmentAttachableTo { recipient } => engine
                    .condition_object_identity(recipient.clone(), condition_context)
                    .is_some_and(|(recipient_id, expected_generation)| {
                        engine
                            .state
                            .zone_change_generation
                            .get(&recipient_id)
                            .copied()
                            .unwrap_or(0)
                            == expected_generation
                            && crate::engine::targeting::equipment_attachment_legal(
                                engine,
                                member.object_id,
                                recipient_id,
                            )
                    }),
            });
            constraints_hold.then_some((member.object_id, member.zone_change_generation))
        })
        .collect()
}

pub(in crate::engine) fn permanent_choice_prompt(
    filter: &TargetFilter,
    min: u32,
    max: u32,
) -> String {
    let (singular, plural) = if filter.any_of.is_none() && filter.kind == TargetKind::Creature {
        ("creature", "creatures")
    } else if filter.any_of.is_none()
        && filter.kind == TargetKind::AnyPermanent
        && filter.permanent_types == [tricerules_card_model::primitives::PermanentTypeFilter::Land]
    {
        ("land", "lands")
    } else {
        ("permanent", "permanents")
    };
    let controller = if filter.any_of.is_some() {
        ""
    } else {
        match filter.controller {
            TargetController::Any => "",
            TargetController::You => " you control",
            TargetController::Opponent => " an opponent controls",
            TargetController::NotYou => " you don't control",
            TargetController::DefendingPlayer => " the defending player controls",
        }
    };

    if min == 1 && max == 1 {
        format!("Choose a {singular}{controller}.")
    } else if min == max {
        format!("Choose {min} {plural}{controller}.")
    } else {
        format!("Choose {min}–{max} {plural}{controller}.")
    }
}

pub(super) fn choose_permanents(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::ChoosePermanents {
        chooser,
        filter,
        min,
        max,
        constraints,
    } = effect
    else {
        unreachable!();
    };
    let targeted_player = match chooser {
        PlayerRecipient::TargetedPlayer { group_index, .. } => Some(
            super::target_player_from_group(cx.target_receipts, group_index).ok_or(
                EngineError::Illegal("permanent choice is missing its targeted player"),
            )?,
        ),
        _ => None,
    };
    let recipients = super::player_recipients(cx, chooser);
    let [deciding_player] = recipients.as_slice() else {
        return Err(EngineError::Illegal(
            "permanent choice requires exactly one deciding player",
        ));
    };
    let deciding_player = *deciding_player;
    let targeted_player_live = targeted_player.is_none_or(|player_id| {
        cx.engine
            .state
            .player_idx(player_id)
            .is_some_and(|index| !cx.engine.state.players[index].has_lost)
    });
    let targeted_player_cohort = if let Some(player_id) = targeted_player {
        Some(if targeted_player_live {
            current_targeted_player_control_cohort(cx.engine, player_id)
        } else {
            cx.engine
                .state
                .departed_targeted_player_control_cohorts
                .get(&player_id)
                .filter(|cohort| cohort.player_id == player_id)
                .cloned()
                .ok_or(EngineError::ProgramInvariant(
                    "departed targeted player is missing its control cohort",
                ))?
        })
    } else {
        None
    };
    let candidates = if let Some(target_player) = targeted_player {
        if targeted_player_live {
            current_targeted_player_permanent_candidates(
                cx.engine,
                cx.top,
                target_player,
                &filter,
                &constraints,
            )
        } else {
            targeted_player_control_cohort_candidates(
                cx.engine,
                cx.top,
                target_player,
                targeted_player_cohort
                    .as_ref()
                    .expect("targeted-player cohort was captured"),
                &filter,
                &constraints,
            )
            .into_iter()
            .map(|(object_id, _)| object_id)
            .collect()
        }
    } else {
        let source =
            crate::engine::targeting::TargetSourceIdentity::for_stack_item(cx.engine, cx.top);
        let condition_context = ConditionContext::for_stack_item(cx.top);
        cx.engine
            .state
            .players
            .iter()
            .flat_map(|player| player.battlefield.iter().copied())
            .filter(|oid| {
                crate::engine::targeting::permanent_choice_filter_legal(
                    cx.engine,
                    &filter,
                    *oid,
                    deciding_player,
                    source,
                    cx.top.trigger_context,
                ) && constraints.iter().all(|constraint| match constraint {
                    PermanentChoiceConstraint::EquipmentAttachableTo { recipient } => cx
                        .engine
                        .condition_object_identity(recipient.clone(), condition_context)
                        .is_some_and(|(recipient_id, expected_generation)| {
                            cx.engine
                                .state
                                .zone_change_generation
                                .get(&recipient_id)
                                .copied()
                                .unwrap_or(0)
                                == expected_generation
                                && crate::engine::targeting::equipment_attachment_legal(
                                    cx.engine,
                                    *oid,
                                    recipient_id,
                                )
                        }),
                })
            })
            .collect::<Vec<_>>()
    };
    if candidates.len() < min as usize {
        cx.events.push(ev_log(format!(
            "P{deciding_player} has no legal permanent to choose."
        )));
        return Ok(EffectOutcome::Continue);
    }
    if candidates.is_empty() && min == 0 {
        return Ok(EffectOutcome::Continue);
    }
    let departed_target_choice = targeted_player.is_some() && !targeted_player_live;
    let max = max.min(candidates.len() as u32);
    if min == max && candidates.len() == min as usize && !departed_target_choice {
        cx.effect_result.produced_objects = candidates
            .iter()
            .map(|oid| TriggerObjectRef {
                object_id: *oid,
                zone_change_generation: cx
                    .engine
                    .state
                    .zone_change_generation
                    .get(oid)
                    .copied()
                    .unwrap_or(0),
                controller_at_event: cx
                    .engine
                    .characteristics(*oid)
                    .map(|characteristics| characteristics.controller)
                    .unwrap_or(deciding_player),
            })
            .collect();
        return Ok(EffectOutcome::Continue);
    }

    let candidate_card_ids = candidates
        .iter()
        .map(|oid| {
            cx.engine
                .state
                .objects
                .get(oid)
                .map(|object| object.card_id.clone())
                .unwrap_or_default()
        })
        .collect::<Vec<_>>();
    let candidate_names = candidates
        .iter()
        .map(|oid| {
            cx.engine
                .state
                .objects
                .get(oid)
                .and_then(|object| cx.engine.registry.get(&object.card_id))
                .map(|definition| definition.name.clone())
                .unwrap_or_else(|| "[object]".to_string())
        })
        .collect::<Vec<_>>();
    let prompt = permanent_choice_prompt(&filter, min, max);
    if !departed_target_choice {
        cx.events.push(rv1::RuledEvent {
            ev: Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(
                rv1::ResolutionChoiceRequired {
                    resolution_prompt_id: 0,
                    variable_mana_contribution: false,
                    candidate_player_ids: Vec::new(),
                    deciding_player_id: deciding_player,
                    source_object_id: cx.top.id,
                    prompt_text: prompt.clone(),
                    choice_kind: rv1::ChoiceKind::PermanentObjects as i32,
                    candidate_object_ids: candidates.clone(),
                    candidate_card_ids,
                    candidate_names,
                    min,
                    max,
                    ..Default::default()
                },
            )),
        });
        cx.events.push(ev_log(prompt.clone()));
    }
    let candidate_generations = candidates
        .iter()
        .map(|oid| {
            (
                *oid,
                cx.engine
                    .state
                    .zone_change_generation
                    .get(oid)
                    .copied()
                    .unwrap_or(0),
            )
        })
        .collect();
    cx.engine.state.pending_resolution = Some(PendingResolution {
        resolution_prompt_id: crate::state::ResolutionPromptId::UNASSIGNED,
        resolution_prompt_semantics: None,
        deciding_player,
        presentation: PendingResolutionPresentation {
            source_object_id: cx.top.id,
            candidates,
            min,
            max,
            ordered: false,
            prompt,
            choice_kind: rv1::ChoiceKind::PermanentObjects,
            unique_names: false,
        },
        continuation: if let (Some(target_player), Some(cohort)) =
            (targeted_player, targeted_player_cohort)
        {
            ResolutionContinuation::TargetedPlayerPermanentChoice {
                stack: cx.parked_stack(),
                target_player,
                cohort,
                filter,
                constraints,
                candidate_generations,
                stage: PendingTargetedPlayerChoiceStage::ChoosingPermanent,
            }
        } else {
            ResolutionContinuation::PermanentChoice {
                stack: cx.parked_stack(),
                candidate_generations,
            }
        },
    });
    if departed_target_choice {
        cx.engine
            .refresh_targeted_player_permanent_choice_departure(cx.events)?;
    }
    Ok(EffectOutcome::Suspended)
}

pub(super) fn may_behold(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::MayBehold { who, criterion, .. } = effect else {
        unreachable!();
    };
    let hand_filter = criterion.hand_filter();
    let permanent_filter = criterion.permanent_filter();
    let recipients = super::player_recipients(cx, who);
    let [deciding_player] = recipients.as_slice() else {
        return Err(EngineError::Illegal(
            "Behold requires exactly one deciding player",
        ));
    };
    let deciding_player = *deciding_player;
    let player_index = cx
        .engine
        .state
        .player_idx(deciding_player)
        .ok_or(EngineError::UnknownPlayer(deciding_player))?;
    let hand_candidates = cx.engine.state.players[player_index]
        .hand
        .iter()
        .copied()
        .filter(|oid| {
            super::zone_card_matches_filter(
                &cx.engine.state,
                cx.engine.registry,
                *oid,
                Some(&hand_filter),
            )
        })
        .collect::<Vec<_>>();
    let source = crate::engine::targeting::TargetSourceIdentity::for_stack_item(cx.engine, cx.top);
    let permanent_candidates = cx
        .engine
        .state
        .players
        .iter()
        .flat_map(|player| player.battlefield.iter().copied())
        .filter(|oid| {
            crate::engine::targeting::permanent_choice_filter_legal(
                cx.engine,
                &permanent_filter,
                *oid,
                deciding_player,
                source,
                cx.top.trigger_context,
            )
        })
        .collect::<Vec<_>>();
    let candidates = hand_candidates
        .iter()
        .chain(&permanent_candidates)
        .copied()
        .collect::<Vec<_>>();
    if candidates.is_empty() {
        cx.events.push(ev_log(format!(
            "P{deciding_player} has no matching object to behold."
        )));
        return Ok(EffectOutcome::Continue);
    }
    let candidate_card_ids = candidates
        .iter()
        .map(|oid| cx.engine.state.objects[oid].card_id.clone())
        .collect();
    let candidate_names = candidates
        .iter()
        .map(|oid| {
            crate::engine::events::object_display_name(&cx.engine.state, cx.engine.registry, *oid)
        })
        .collect();
    let candidate_source_zones = hand_candidates
        .iter()
        .map(|_| rv1::ChoiceCandidateSourceZone::Hand as i32)
        .chain(
            permanent_candidates
                .iter()
                .map(|_| rv1::ChoiceCandidateSourceZone::Battlefield as i32),
        )
        .collect();
    let prompt = format!("P{deciding_player}: you may behold a matching object.");
    cx.events.push(rv1::RuledEvent {
        ev: Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(
            rv1::ResolutionChoiceRequired {
                resolution_prompt_id: 0,
                variable_mana_contribution: false,
                candidate_player_ids: Vec::new(),
                deciding_player_id: deciding_player,
                source_object_id: cx.top.id,
                prompt_text: prompt.clone(),
                choice_kind: rv1::ChoiceKind::Behold as i32,
                candidate_object_ids: candidates.clone(),
                candidate_card_ids,
                candidate_names,
                min: 0,
                max: 1,
                candidate_source_zones,
                ..Default::default()
            },
        )),
    });
    cx.events.push(ev_log(prompt.clone()));
    let candidate_generations = candidates
        .iter()
        .map(|oid| {
            (
                *oid,
                cx.engine
                    .state
                    .zone_change_generation
                    .get(oid)
                    .copied()
                    .unwrap_or(0),
            )
        })
        .collect();
    cx.engine.state.pending_resolution = Some(PendingResolution {
        resolution_prompt_id: crate::state::ResolutionPromptId::UNASSIGNED,
        resolution_prompt_semantics: None,
        deciding_player,
        presentation: PendingResolutionPresentation {
            source_object_id: cx.top.id,
            candidates,
            min: 0,
            max: 1,
            ordered: false,
            prompt,
            choice_kind: rv1::ChoiceKind::Behold,
            unique_names: false,
        },
        continuation: ResolutionContinuation::BeholdChoice {
            stack: if cx.program_state.is_some() {
                cx.parked_stack()
            } else {
                ParkedStackResolution::new(cx.top.clone())
                    .with_previous_result(cx.previous_effect_result.clone())
            },
            candidate_generations,
            hand_candidates,
            hand_filter,
            permanent_filter,
        },
    });
    Ok(EffectOutcome::Suspended)
}

pub(in crate::engine) fn resolution_branch_is_live(
    engine: &crate::engine::GameEngine,
    top: &StackItem,
    deciding_player: i32,
    branch: &ResolutionBranchDef,
) -> bool {
    let requirement_met = match &branch.requirement {
        ResolutionBranchRequirement::Always => true,
        ResolutionBranchRequirement::EffectsApplicable => {
            branch.effects.iter().all(|effect| match effect {
                SpellEffectKind::PutCounters { subject, .. } => {
                    super::pump_counters::can_put_counters(engine, top, &[], subject)
                }
                SpellEffectKind::Mill {
                    count: Amount::Fixed(count),
                    who: PlayerRecipient::Controller,
                } => engine
                    .state
                    .player_idx(top.controller)
                    .is_some_and(|index| {
                        engine.state.players[index].library.len() >= *count as usize
                    }),
                SpellEffectKind::Exile {
                    subject: tricerules_card_model::primitives::EffectSubject::TriggerObject,
                    ..
                } => super::resolve_zone_effect_subject(
                    engine,
                    top,
                    &[],
                    &[],
                    &[],
                    &tricerules_card_model::primitives::EffectSubject::TriggerObject,
                )
                .is_some(),
                _ => true,
            })
        }
        ResolutionBranchRequirement::GameCondition(condition) => engine.condition_holds(
            condition,
            crate::engine::ConditionContext::for_stack_item(top),
        ),
        ResolutionBranchRequirement::CastCostReceipt(condition) => {
            top.cast_cost_condition_matches(condition)
        }
    };
    let required_candidates = match branch.cost {
        ResolutionCost::None | ResolutionCost::Mana(_) | ResolutionCost::Waterbend(_) => 0,
        ResolutionCost::TapPermanents { count, .. } => count as usize,
        _ => 1,
    };
    requirement_met
        && engine
            .resolution_cost_candidates(
                deciding_player,
                top.source_permanent_id.unwrap_or(top.id),
                top.source_zone_change,
                &branch.cost,
            )
            .len()
            >= required_candidates
}

/// A replacement player may make the departed player's non-cost choice, but this engine couples
/// a costed branch and its payment in one choice contract. CR 800.4f means that payment is not
/// transferred to the replacement player, so only cost-free branches remain selectable.
pub(in crate::engine) fn resolution_branch_is_selectable(
    engine: &crate::engine::GameEngine,
    top: &StackItem,
    previous_result: &crate::state::EffectResult,
    deciding_player: i32,
    original_chooser: i32,
    branch: &ResolutionBranchDef,
    check_draw_capacity: bool,
) -> bool {
    let original_chooser_live = engine
        .state
        .player_idx(original_chooser)
        .is_some_and(|index| !engine.state.players[index].has_lost);
    (original_chooser_live || branch.cost == ResolutionCost::None)
        && resolution_branch_is_live(engine, top, deciding_player, branch)
        && (!check_draw_capacity
            || branch.cost != ResolutionCost::None
            || direct_draw_effect_is_selectable(engine, top, previous_result, &branch.effects)
                .unwrap_or(true))
}

/// Whether one direct, cost-free draw instruction in a player-selected branch can be carried out
/// in full under the active draw limits. A multi-effect branch keeps its normal applicability;
/// the individual draw instruction still enforces limits when it resolves.
pub(in crate::engine) fn direct_draw_effect_is_selectable(
    engine: &GameEngine,
    top: &StackItem,
    previous_result: &crate::state::EffectResult,
    effects: &[SpellEffectKind],
) -> Option<bool> {
    let [effect] = effects else {
        return None;
    };
    let (drawers, count) = match effect {
        SpellEffectKind::Draw { who, count } => (
            branch_draw_recipients(engine, top, previous_result, *who),
            count,
        ),
        SpellEffectKind::TargetPlayerDraws { count, .. } => {
            let targets = top
                .targets
                .iter()
                .chain(top.chosen_modes.iter().flat_map(|mode| mode.targets.iter()));
            let player = targets
                .filter_map(|target| engine.state.player_idx(target.object_id as PlayerId))
                .map(|index| engine.state.players[index].id)
                .find(|player| {
                    engine
                        .state
                        .player_idx(*player)
                        .is_some_and(|index| !engine.state.players[index].has_lost)
                });
            (player.into_iter().collect(), count)
        }
        _ => return None,
    };

    if matches!(count, Amount::Fixed(0)) || drawers.is_empty() {
        return Some(true);
    }
    Some(drawers.into_iter().any(|drawer| {
        let requested = engine.resolve_amount(
            count,
            AmountContext::for_stack_item(top, top.controller)
                .with_affected_player(drawer)
                .with_previous_effect_result(previous_result),
        );
        requested == 0
            || engine
                .draw_limit_remaining(drawer)
                .is_none_or(|remaining| requested <= remaining)
    }))
}

fn branch_draw_recipients(
    engine: &GameEngine,
    top: &StackItem,
    previous_result: &crate::state::EffectResult,
    who: PlayerRecipient,
) -> Vec<PlayerId> {
    let targets = top
        .targets
        .iter()
        .chain(top.chosen_modes.iter().flat_map(|mode| mode.targets.iter()))
        .copied()
        .collect::<Vec<_>>();
    match who {
        PlayerRecipient::TargetedPlayer { group_index, .. } => {
            super::target_player_from_group(&targets, group_index)
                .into_iter()
                .collect()
        }
        PlayerRecipient::ControllerOfTargetGroup { group_index } => targets
            .iter()
            .find(|target| target.group_index == group_index)
            .and_then(|target| {
                let generation = target.zone_change_generation?;
                let current = engine
                    .state
                    .zone_change_generation
                    .get(&target.object_id)
                    .copied()
                    .unwrap_or(0)
                    == generation
                    && engine
                        .state
                        .objects
                        .get(&target.object_id)
                        .is_some_and(|object| object.zone == Zone::Battlefield);
                if current {
                    engine.controller_of(target.object_id)
                } else {
                    engine
                        .state
                        .last_known_controller_by_generation
                        .get(&(target.object_id, generation))
                        .copied()
                }
            })
            .into_iter()
            .collect(),
        PlayerRecipient::ProgramInputPlayer => match previous_result.program_value.as_ref() {
            Some(crate::state::ProgramValue::TargetedSpellController(Some(player))) => {
                vec![*player]
            }
            Some(crate::state::ProgramValue::TargetedSpellController(None)) | None => Vec::new(),
            _ => Vec::new(),
        },
        PlayerRecipient::EachOtherPlayerThanAffectedPlayer => {
            let Some(affected) = top.trigger_context.affected_player else {
                return Vec::new();
            };
            let mut players = engine
                .state
                .players
                .iter()
                .filter(|player| !player.has_lost && player.id != affected)
                .map(|player| player.id)
                .collect::<Vec<_>>();
            players.sort_by_key(|player| engine.state.apnap_rank(*player));
            players
        }
        PlayerRecipient::DefendingPlayer => top
            .trigger_context
            .defending_player
            .filter(|player| {
                engine
                    .state
                    .player_idx(*player)
                    .is_some_and(|index| !engine.state.players[index].has_lost)
            })
            .into_iter()
            .collect(),
        PlayerRecipient::TriggeringAttackingPlayer => {
            let Some((attacker, defender)) = top
                .trigger_context
                .attacking_player
                .zip(top.trigger_context.defending_player)
            else {
                return Vec::new();
            };
            (engine
                .state
                .player_idx(attacker)
                .is_some_and(|index| !engine.state.players[index].has_lost)
                && engine.state.are_opponents(attacker, defender))
            .then_some(attacker)
            .into_iter()
            .collect()
        }
        PlayerRecipient::AttackingOpponentsOfDefendingPlayer => {
            let Some(defender) = top.trigger_context.defending_player else {
                return Vec::new();
            };
            let Some(combat) = engine.state.combat.as_ref() else {
                return Vec::new();
            };
            let mut recipients = Vec::new();
            for attacker in &combat.attacking {
                let Some(assignment) = combat.attack_assignments.get(attacker) else {
                    continue;
                };
                if assignment.attacker.object_id != *attacker
                    || assignment.defending_player != defender
                    || assignment.defender != crate::state::CombatDefenderTarget::Player(defender)
                    || engine
                        .state
                        .zone_change_generation
                        .get(attacker)
                        .copied()
                        .unwrap_or(0)
                        != assignment.attacker.zone_change_generation
                    || !engine
                        .state
                        .objects
                        .get(attacker)
                        .is_some_and(|object| object.zone == Zone::Battlefield)
                {
                    continue;
                }
                let Some(player) = engine.controller_of(*attacker) else {
                    continue;
                };
                if engine
                    .state
                    .player_idx(player)
                    .is_some_and(|index| !engine.state.players[index].has_lost)
                    && engine.state.are_opponents(player, defender)
                    && !recipients.contains(&player)
                {
                    recipients.push(player);
                }
            }
            recipients.sort_by_key(|player| engine.state.apnap_rank(*player));
            recipients
        }
        _ => super::simple_player_recipients(
            &engine.state,
            top.controller,
            top.trigger_context
                .affected_player
                .unwrap_or(top.controller),
            super::trigger_object_controller(engine, top),
            super::source_controller(engine, top),
            who,
        ),
    }
}

fn previous_result_receipt_matches(
    engine: &crate::engine::GameEngine,
    top: &StackItem,
    previous_result: &crate::state::EffectResult,
    condition: &tricerules_card_model::primitives::ResolutionReceiptCondition,
) -> bool {
    use tricerules_card_model::primitives::ResolutionReceiptCondition;

    match condition {
        ResolutionReceiptCondition::CountersPlaced { counter, object } => {
            let Some((object_id, zone_change_generation)) = engine
                .condition_object_identity(object.clone(), ConditionContext::for_stack_item(top))
            else {
                return false;
            };
            previous_result.counter_placements.iter().any(|receipt| {
                receipt.counter == *counter
                    && receipt.count > 0
                    && receipt.object.object_id == object_id
                    && receipt.object.zone_change_generation == zone_change_generation
            })
        }
        ResolutionReceiptCondition::Sacrificed { object } => {
            let Some((object_id, zone_change_generation)) = engine
                .condition_object_identity(object.clone(), ConditionContext::for_stack_item(top))
            else {
                return false;
            };
            previous_result.sacrificed_objects.iter().any(|receipt| {
                receipt.object_id == object_id
                    && receipt.zone_change_generation == zone_change_generation
            })
        }
    }
}

pub(in crate::engine) fn card_result_count(
    engine: &crate::engine::GameEngine,
    top: &StackItem,
    previous_result: &crate::state::EffectResult,
    filter: &tricerules_card_model::primitives::CardResultFilter,
) -> u32 {
    if let Some(receipt) = previous_result.program_discard_result.as_ref() {
        return program_discard_result_count(&engine.state, top.controller, receipt, filter, None);
    }
    card_result_count_from_cohorts(
        &engine.state,
        top.controller,
        &top.payment_result,
        previous_result,
        filter,
    )
}

pub(in crate::engine) fn card_result_count_for_player(
    engine: &crate::engine::GameEngine,
    top: &StackItem,
    previous_result: &crate::state::EffectResult,
    filter: &tricerules_card_model::primitives::CardResultFilter,
    player: crate::state::PlayerId,
) -> u32 {
    if let Some(receipt) = previous_result.program_discard_result.as_ref() {
        return program_discard_result_count(
            &engine.state,
            top.controller,
            receipt,
            filter,
            Some(player),
        );
    }
    matching_card_result_entries(
        &engine.state,
        top.controller,
        &top.payment_result,
        previous_result,
        filter,
    )
    .filter(|entry| entry.affected_player == player)
    .count() as u32
}

pub(in crate::engine) fn card_result_maximum(
    engine: &crate::engine::GameEngine,
    top: &StackItem,
    previous_result: &crate::state::EffectResult,
    filter: &tricerules_card_model::primitives::CardResultFilter,
) -> u32 {
    if let Some(receipt) = previous_result.program_discard_result.as_ref() {
        return program_discard_result_maximum(&engine.state, top.controller, receipt, filter);
    }
    card_result_maximum_from_cohorts(
        &engine.state,
        top.controller,
        &top.payment_result,
        previous_result,
        filter,
    )
}

fn card_result_maximum_from_cohorts(
    state: &crate::state::GameState,
    controller: i32,
    payment_result: &crate::state::CardResultCohort,
    previous_result: &crate::state::EffectResult,
    filter: &tricerules_card_model::primitives::CardResultFilter,
) -> u32 {
    let mut counts = std::collections::BTreeMap::<i32, u32>::new();
    for entry in
        matching_card_result_entries(state, controller, payment_result, previous_result, filter)
    {
        let count = counts.entry(entry.affected_player).or_default();
        *count = count.saturating_add(1);
    }
    counts.into_values().max().unwrap_or(0)
}

fn program_discard_result_count(
    state: &crate::state::GameState,
    controller: i32,
    receipt: &crate::state::ProgramDiscardResult,
    filter: &tricerules_card_model::primitives::CardResultFilter,
    affected_player: Option<crate::state::PlayerId>,
) -> u32 {
    if filter.source != tricerules_card_model::primitives::CardResultSource::PreviousEffect
        || filter.action != tricerules_card_model::primitives::CardResultAction::Discard
    {
        return 0;
    }
    receipt
        .cards
        .iter()
        .filter(|entry| {
            crate::engine::history::relative_player_set_contains(
                state,
                filter.players,
                controller,
                entry.affected_player,
            ) && affected_player.is_none_or(|player| entry.affected_player == player)
        })
        .filter(|entry| match (&entry.type_knowledge, filter.card_type) {
            (_, None) => true,
            (crate::state::DiscardCardTypeKnowledge::Known(types), Some(card_type)) => {
                types.contains(&card_type)
            }
            (crate::state::DiscardCardTypeKnowledge::UnknownHidden, Some(_)) => false,
        })
        .count()
        .min(u32::MAX as usize) as u32
}

fn program_discard_result_maximum(
    state: &crate::state::GameState,
    controller: i32,
    receipt: &crate::state::ProgramDiscardResult,
    filter: &tricerules_card_model::primitives::CardResultFilter,
) -> u32 {
    if filter.source != tricerules_card_model::primitives::CardResultSource::PreviousEffect
        || filter.action != tricerules_card_model::primitives::CardResultAction::Discard
    {
        return 0;
    }
    let mut counts = std::collections::BTreeMap::<i32, u32>::new();
    for entry in &receipt.cards {
        if !crate::engine::history::relative_player_set_contains(
            state,
            filter.players,
            controller,
            entry.affected_player,
        ) || match (&entry.type_knowledge, filter.card_type) {
            (_, None) => false,
            (crate::state::DiscardCardTypeKnowledge::Known(types), Some(card_type)) => {
                !types.contains(&card_type)
            }
            (crate::state::DiscardCardTypeKnowledge::UnknownHidden, Some(_)) => true,
        } {
            continue;
        }
        let count = counts.entry(entry.affected_player).or_default();
        *count = count.saturating_add(1);
    }
    counts.into_values().max().unwrap_or(0)
}

pub(in crate::engine) fn card_result_characteristic_sum(
    engine: &crate::engine::GameEngine,
    top: &StackItem,
    previous_result: &crate::state::EffectResult,
    filter: &tricerules_card_model::primitives::CardResultFilter,
    characteristic: BattlefieldQuantityCharacteristic,
) -> i64 {
    matching_card_result_entries(
        &engine.state,
        top.controller,
        &top.payment_result,
        previous_result,
        filter,
    )
    .fold(0_i64, |sum, entry| {
        let value = match characteristic {
            BattlefieldQuantityCharacteristic::Power
            | BattlefieldQuantityCharacteristic::Toughness => {
                if filter.action == CardResultAction::Exile
                    && characteristic == BattlefieldQuantityCharacteristic::Power
                {
                    match committed_movement_evidence(engine, entry) {
                        Ok(CommittedMovementEvidence::Public {
                            input_zone: Zone::Battlefield,
                            input_generation,
                            actual_output_zone: Zone::Exile,
                            ..
                        }) => engine
                            .state
                            .last_known_pt_by_generation
                            .get(&(entry.object_id, input_generation))
                            .and_then(|(power, _)| *power)
                            .unwrap_or(0),
                        // The fallible Program input projection rejects unsupported or
                        // incomplete Exile receipts before this amount evaluator runs.
                        _ => 0,
                    }
                } else {
                    let (power, toughness) = engine.object_power_toughness(
                        entry.object_id,
                        engine.card_result_generation(entry),
                    );
                    match characteristic {
                        BattlefieldQuantityCharacteristic::Power => power,
                        BattlefieldQuantityCharacteristic::Toughness => toughness,
                        BattlefieldQuantityCharacteristic::ManaValue => unreachable!(),
                    }
                }
            }
            BattlefieldQuantityCharacteristic::ManaValue => {
                match committed_movement_evidence(engine, entry) {
                    Ok(CommittedMovementEvidence::Public {
                        input_zone: Zone::Library,
                        actual_output_zone: Zone::Graveyard,
                        outside_stack_mana_value: Some(value),
                        ..
                    }) => i64::from(value),
                    // The fallible Program input projection validates qualifying public Mill
                    // receipts before any amount reaches this legacy infallible evaluator.
                    // Unknown-hidden and non-Mill payment receipts are never dereferenced here.
                    _ => 0,
                }
            }
        };
        sum.saturating_add(value)
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CommittedMovementEvidence {
    Missing,
    Hidden,
    Public {
        input_zone: Zone,
        input_generation: u64,
        actual_output_zone: Zone,
        owner: PlayerId,
        outside_stack_mana_value: Option<u32>,
    },
}

/// Validate and project the exact typed Program input before EffectResult drops the raw
/// ProgramValue variant. A present empty CardResultCohort is valid; an absent or differently
/// typed runtime value remains an invariant error instead of becoming an empty sum.
pub(super) fn validate_program_card_result_characteristic_sum_input(
    engine: &GameEngine,
    top: &StackItem,
    effect: &SpellEffectKind,
    result_type: &ResultType,
    value: &ProgramValue,
) -> Result<(), EngineError> {
    let Some((filter, characteristic)) = effect.previous_effect_card_result_characteristic_sum()
    else {
        return Ok(());
    };
    let (action, players, producer_who, target_group_index) = match result_type {
        ResultType::CardResultCohort { action, who, .. } => (
            *action,
            if *who == PlayerRecipient::Controller {
                RelativePlayerSet::Controller
            } else {
                RelativePlayerSet::All
            },
            Some(*who),
            None,
        ),
        ResultType::TargetCardResultCohort {
            action,
            target_group_index,
            ..
        } => (
            *action,
            RelativePlayerSet::All,
            None,
            Some(*target_group_index),
        ),
        _ => {
            return Err(EngineError::ProgramInvariant(
                "card-result characteristic sum input has the wrong compiled result type",
            ));
        }
    };
    if action != filter.action || players != filter.players {
        return Err(EngineError::ProgramInvariant(
            "card-result characteristic sum input type disagrees with its declared filter",
        ));
    }
    if filter.action == CardResultAction::Exile
        && characteristic == BattlefieldQuantityCharacteristic::Power
        && (target_group_index.is_none() || filter.card_type.is_some())
    {
        return Err(EngineError::ProgramInvariant(
            "Exile Power sum requires its unfiltered typed target-group receipt",
        ));
    }
    let ProgramValue::CardResultCohort(cohort) = value else {
        return Err(EngineError::ProgramInvariant(
            "card-result characteristic sum input has the wrong runtime value",
        ));
    };

    for entry in &cohort.cards {
        if entry.action != filter.action
            || !super::super::history::relative_player_set_contains(
                &engine.state,
                filter.players,
                top.controller,
                entry.affected_player,
            )
        {
            return Err(EngineError::ProgramInvariant(
                "card-result characteristic sum cohort disagrees with its compiled action or player scope",
            ));
        }
        if producer_who.is_some_and(|who| {
            !card_result_entry_matches_program_recipient(engine, top, who, entry.affected_player)
        }) {
            return Err(EngineError::ProgramInvariant(
                "card-result characteristic sum cohort member disagrees with its compiled producer recipient",
            ));
        }
        if filter
            .card_type
            .is_some_and(|card_type| !entry.matched_card_types.contains(&card_type))
        {
            continue;
        }
        if characteristic != BattlefieldQuantityCharacteristic::ManaValue {
            if filter.action == CardResultAction::Exile
                && characteristic == BattlefieldQuantityCharacteristic::Power
            {
                match committed_movement_evidence(engine, entry)? {
                    CommittedMovementEvidence::Missing | CommittedMovementEvidence::Hidden => {
                        return Err(EngineError::ProgramInvariant(
                            "Exile Power sum entry has no public committed movement receipt",
                        ));
                    }
                    CommittedMovementEvidence::Public {
                        input_zone: Zone::Battlefield,
                        input_generation,
                        actual_output_zone: Zone::Exile,
                        ..
                    } => {
                        if engine
                            .state
                            .last_known_pt_by_generation
                            .get(&(entry.object_id, input_generation))
                            .is_none_or(|(power, _)| power.is_none())
                        {
                            return Err(EngineError::ProgramInvariant(
                                "Exile Power sum is missing its exact input-generation power snapshot",
                            ));
                        }
                    }
                    CommittedMovementEvidence::Public { .. } => {
                        return Err(EngineError::ProgramInvariant(
                            "Exile Power sum encountered an unsupported committed movement",
                        ));
                    }
                }
            }
            continue;
        }
        match committed_movement_evidence(engine, entry)? {
            CommittedMovementEvidence::Missing => {
                return Err(EngineError::ProgramInvariant(
                    "ManaValue card-result sum entry has no committed movement receipt",
                ));
            }
            CommittedMovementEvidence::Hidden => continue,
            CommittedMovementEvidence::Public {
                input_zone: Zone::Library,
                actual_output_zone: Zone::Graveyard,
                owner,
                outside_stack_mana_value: Some(_),
                ..
            } if filter.action == CardResultAction::Mill && owner != entry.affected_player => {
                return Err(EngineError::ProgramInvariant(
                    "Mill movement owner disagrees with its affected library player",
                ));
            }
            CommittedMovementEvidence::Public {
                input_zone: Zone::Library,
                actual_output_zone: Zone::Graveyard,
                owner,
                outside_stack_mana_value: None,
                ..
            } => {
                if filter.action == CardResultAction::Mill && owner != entry.affected_player {
                    return Err(EngineError::ProgramInvariant(
                        "Mill movement owner disagrees with its affected library player",
                    ));
                }
                return Err(EngineError::ProgramInvariant(
                    "public Library-to-Graveyard result is missing its event-time ManaValue",
                ));
            }
            CommittedMovementEvidence::Public {
                input_zone: Zone::Library,
                actual_output_zone: Zone::Graveyard,
                outside_stack_mana_value: Some(_),
                ..
            } => {}
            CommittedMovementEvidence::Public { .. } => {
                return Err(EngineError::ProgramInvariant(
                    "ManaValue card-result sum encountered an unsupported committed movement",
                ));
            }
        }
    }
    Ok(())
}

fn card_result_entry_matches_program_recipient(
    engine: &GameEngine,
    top: &StackItem,
    who: PlayerRecipient,
    affected_player: PlayerId,
) -> bool {
    let expected_player = match who {
        PlayerRecipient::Controller => Some(top.controller),
        PlayerRecipient::AffectedPlayer => Some(
            top.trigger_context
                .affected_player
                .unwrap_or(top.controller),
        ),
        _ => None,
    };
    expected_player.is_some_and(|player| {
        engine.state.player_idx(player).is_some() && affected_player == player
    })
}

fn committed_movement_evidence(
    engine: &GameEngine,
    entry: &crate::state::CardResultEntry,
) -> Result<CommittedMovementEvidence, EngineError> {
    match entry.provenance {
        crate::state::CardResultProvenance::ActionFacts => Ok(CommittedMovementEvidence::Missing),
        crate::state::CardResultProvenance::CommittedMovement {
            output_visibility: crate::state::CardResultOutputVisibility::UnknownHidden,
            ..
        } => Ok(CommittedMovementEvidence::Hidden),
        crate::state::CardResultProvenance::CommittedMovement {
            input_zone,
            input_generation,
            owner,
            actual_output_zone,
            output_visibility: crate::state::CardResultOutputVisibility::FaceUpPublic,
            outside_stack_mana_value,
        } => {
            if input_generation >= entry.zone_change_generation {
                return Err(EngineError::ProgramInvariant(
                    "committed card-result movement has an invalid generation range",
                ));
            }
            if engine
                .state
                .objects
                .get(&entry.object_id)
                .is_some_and(|object| object.owner != owner)
            {
                return Err(EngineError::ProgramInvariant(
                    "committed card-result movement owner disagrees with its object identity",
                ));
            }
            Ok(CommittedMovementEvidence::Public {
                input_zone,
                input_generation,
                actual_output_zone,
                owner,
                outside_stack_mana_value,
            })
        }
    }
}

fn card_result_count_from_cohorts(
    state: &crate::state::GameState,
    controller: i32,
    payment_result: &crate::state::CardResultCohort,
    previous_result: &crate::state::EffectResult,
    filter: &tricerules_card_model::primitives::CardResultFilter,
) -> u32 {
    matching_card_result_entries(state, controller, payment_result, previous_result, filter)
        .count()
        .min(u32::MAX as usize) as u32
}

fn matching_card_result_entries<'a>(
    state: &'a crate::state::GameState,
    controller: i32,
    payment_result: &'a crate::state::CardResultCohort,
    previous_result: &'a crate::state::EffectResult,
    filter: &'a tricerules_card_model::primitives::CardResultFilter,
) -> impl Iterator<Item = &'a crate::state::CardResultEntry> + 'a {
    let cards = match filter.source {
        tricerules_card_model::primitives::CardResultSource::Payment => &payment_result.cards,
        tricerules_card_model::primitives::CardResultSource::PreviousEffect => {
            &previous_result.cards
        }
    };
    let mut seen = std::collections::BTreeSet::new();
    cards
        .iter()
        .filter(move |entry| seen.insert((entry.object_id, entry.zone_change_generation)))
        .filter(move |entry| entry.action == filter.action)
        .filter(move |entry| {
            crate::engine::history::relative_player_set_contains(
                state,
                filter.players,
                controller,
                entry.affected_player,
            )
        })
        .filter(move |entry| {
            filter
                .card_type
                .is_none_or(|card_type| entry.matched_card_types.contains(&card_type))
        })
}

pub(super) fn park_resolution_branches(
    cx: &mut EffectCx<'_>,
    optional: bool,
    branches: Vec<tricerules_card_model::primitives::ResolutionBranchDef>,
) -> Result<EffectOutcome, EngineError> {
    park_resolution_branches_for(
        cx,
        tricerules_card_model::primitives::PlayerRecipient::Controller,
        optional,
        branches,
    )
}

fn park_resolution_branches_for(
    cx: &mut EffectCx<'_>,
    chooser: PlayerRecipient,
    optional: bool,
    branches: Vec<tricerules_card_model::primitives::ResolutionBranchDef>,
) -> Result<EffectOutcome, EngineError> {
    let recipients = super::player_recipients(cx, chooser);
    let [deciding_player] = recipients.as_slice() else {
        return Err(EngineError::Illegal(
            "resolution choice requires exactly one deciding player",
        ));
    };
    let original_chooser = *deciding_player;
    let controller = cx.top.controller;
    let is_live = |player| {
        cx.engine
            .state
            .player_idx(player)
            .is_some_and(|index| !cx.engine.state.players[index].has_lost)
    };
    let (deciding_player, stage, prompt) = if is_live(original_chooser) {
        let prompt = if optional {
            "Choose a resolution option, or decline.".to_string()
        } else {
            "Choose a resolution option.".to_string()
        };
        (
            original_chooser,
            PendingResolutionBranchStage::Selecting,
            prompt,
        )
    } else {
        let candidates =
            resolution_choice_delegate_candidates(&cx.engine.state, controller, original_chooser);
        match candidates.as_slice() {
            [] => return Ok(EffectOutcome::Continue),
            [delegate] => {
                let prompt = if optional {
                    "Choose a resolution option, or decline.".to_string()
                } else {
                    "Choose a resolution option.".to_string()
                };
                (*delegate, PendingResolutionBranchStage::Selecting, prompt)
            }
            _ => (
                controller,
                PendingResolutionBranchStage::ChoosingDelegate {
                    candidates: candidates.into_iter().map(Some).collect(),
                },
                format!(
                    "P{controller}: choose a player to make P{original_chooser}'s resolution choice."
                ),
            ),
        }
    };
    cx.engine.state.pending_resolution = Some(PendingResolution {
        resolution_prompt_id: crate::state::ResolutionPromptId::UNASSIGNED,
        resolution_prompt_semantics: None,
        deciding_player,
        presentation: PendingResolutionPresentation {
            source_object_id: cx.top.id,
            candidates: Vec::new(),
            min: if matches!(
                &stage,
                PendingResolutionBranchStage::ChoosingDelegate { .. }
            ) {
                1
            } else {
                u32::from(!optional)
            },
            max: 1,
            ordered: false,
            prompt,
            choice_kind: rv1::ChoiceKind::ResolutionBranch,
            unique_names: false,
        },
        continuation: ResolutionContinuation::AuthoredBranch {
            stack: ParkedStackResolution::new(cx.top.clone())
                .with_previous_result(cx.previous_effect_result.clone()),
            branch: PendingResolutionBranch {
                optional,
                chooser,
                original_chooser,
                branches,
                stage,
            },
        },
    });
    let event = authored_resolution_branch_choice_event(cx.engine)
        .expect("parked resolution branch produces a choice event");
    let prompt = match &event.ev {
        Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(choice)) => choice.prompt_text.clone(),
        _ => unreachable!("authored branch choice helper emits a choice event"),
    };
    cx.events.push(event);
    cx.events.push(ev_log(prompt));
    Ok(EffectOutcome::Suspended)
}

/// Surviving players eligible to take over a departed resolution chooser (CR 800.4g).
/// The controller of the object creating the effect must choose another player. If the departed
/// chooser was that controller's opponent, prefer another live opponent whenever one exists.
pub(in crate::engine) fn resolution_choice_delegate_candidates(
    state: &crate::state::GameState,
    controller: i32,
    original_chooser: i32,
) -> Vec<i32> {
    let other_live_players = state
        .players
        .iter()
        .filter(|player| !player.has_lost && player.id != original_chooser)
        .map(|player| player.id)
        .collect::<Vec<_>>();
    if state.are_opponents(controller, original_chooser) {
        let other_opponents = other_live_players
            .iter()
            .copied()
            .filter(|player| state.are_opponents(controller, *player))
            .collect::<Vec<_>>();
        if !other_opponents.is_empty() {
            return other_opponents;
        }
    }
    other_live_players
}

/// Select the live object controller, or the next live seat after a departed controller (CR 800.4h).
pub(in crate::engine) fn resolution_choice_selector(
    state: &crate::state::GameState,
    controller: PlayerId,
) -> Option<PlayerId> {
    if state
        .player_idx(controller)
        .is_some_and(|index| !state.players[index].has_lost)
    {
        return Some(controller);
    }
    state
        .player_idx(controller)
        .and_then(|index| state.next_in_game_player_idx(index))
        .map(|index| state.players[index].id)
        .or_else(|| {
            state
                .players
                .iter()
                .find(|player| !player.has_lost)
                .map(|player| player.id)
        })
}

/// Keep branch indices stable as eligible replacement choosers leave or become eligible.
pub(in crate::engine) fn reconcile_resolution_choice_delegate_slots(
    previous: &[Option<PlayerId>],
    eligible: &[PlayerId],
) -> Vec<Option<PlayerId>> {
    let mut slots = previous.to_vec();
    for slot in &mut slots {
        if slot.is_some_and(|candidate| !eligible.contains(&candidate)) {
            *slot = None;
        }
    }
    for candidate in eligible.iter().copied() {
        if !slots.contains(&Some(candidate)) {
            slots.push(Some(candidate));
        }
    }
    slots
}

/// Resolve a submitted delegate index against the preserved slots and current CR 800.4g menu.
pub(in crate::engine) fn resolution_choice_delegate_at_index(
    state: &crate::state::GameState,
    controller: PlayerId,
    original_chooser: PlayerId,
    candidates: &[Option<PlayerId>],
    index: u32,
) -> Option<PlayerId> {
    let eligible = resolution_choice_delegate_candidates(state, controller, original_chooser);
    candidates
        .get(index as usize)
        .copied()
        .flatten()
        .filter(|delegate| eligible.contains(delegate))
        .filter(|delegate| {
            state
                .player_idx(*delegate)
                .is_some_and(|player| !state.players[player].has_lost)
        })
}

/// Project delegate slots consistently for the real ProgramChoose and optional-trigger menus.
pub(in crate::engine) fn resolution_choice_delegate_options(
    state: &crate::state::GameState,
    controller: PlayerId,
    original_chooser: PlayerId,
    candidates: &[Option<PlayerId>],
) -> Vec<rv1::ResolutionBranchOption> {
    let eligible = resolution_choice_delegate_candidates(state, controller, original_chooser);
    candidates
        .iter()
        .enumerate()
        .map(|(index, candidate)| {
            let selectable = candidate.is_some_and(|candidate| {
                eligible.contains(&candidate)
                    && state
                        .player_idx(candidate)
                        .is_some_and(|player| !state.players[player].has_lost)
            });
            let label = candidate.map_or_else(
                || "Unavailable player".to_string(),
                |candidate| format!("P{candidate} makes the choice"),
            );
            rv1::ResolutionBranchOption {
                branch_index: index as u32,
                label: label.clone(),
                selectable,
                presentation: Some(rv1::PresentationRef {
                    fallback_text: label,
                    ..Default::default()
                }),
                ..Default::default()
            }
        })
        .collect()
}

/// Rebuild the current authored-branch prompt from its serialized continuation. This is also used
/// after a player leaves, so a stale client prompt cannot retain a departed delegate.
pub(in crate::engine) fn authored_resolution_branch_choice_event(
    engine: &crate::engine::GameEngine,
) -> Option<rv1::RuledEvent> {
    let pending = engine.state.pending_resolution.as_ref()?;
    let ResolutionContinuation::AuthoredBranch { stack, branch } = &pending.continuation else {
        return None;
    };
    if matches!(branch.stage, PendingResolutionBranchStage::Selecting)
        && branch.branches.iter().any(|option| {
            matches!(
                &option.cost,
                ResolutionCost::PutHandCardOnLibraryBottom | ResolutionCost::DiscardCard { .. }
            )
        })
    {
        return None;
    }
    let options = match &branch.stage {
        PendingResolutionBranchStage::ChoosingDelegate { candidates } => {
            let eligible = resolution_choice_delegate_candidates(
                &engine.state,
                stack.item.controller,
                branch.original_chooser,
            );
            candidates
                .iter()
                .enumerate()
                .map(|(index, candidate)| {
                    let valid = candidate.is_some_and(|candidate| {
                        eligible.contains(&candidate)
                            && engine
                                .state
                                .player_idx(candidate)
                                .is_some_and(|idx| !engine.state.players[idx].has_lost)
                    });
                    let label = candidate.map_or_else(
                        || "Unavailable player".into(),
                        |candidate| format!("P{candidate} makes the choice"),
                    );
                    rv1::ResolutionBranchOption {
                        branch_index: index as u32,
                        label: label.clone(),
                        cost_kind: rv1::ResolutionBranchCostKind::Unspecified as i32,
                        cost_text: String::new(),
                        selectable: valid,
                        search_zones: Vec::new(),
                        presentation: Some(rv1::PresentationRef {
                            fallback_text: label,
                            ..Default::default()
                        }),
                    }
                })
                .collect()
        }
        PendingResolutionBranchStage::Selecting => branch
            .branches
            .iter()
            .enumerate()
            .filter(|(_, option)| {
                resolution_branch_is_selectable(
                    engine,
                    &stack.item,
                    &stack.previous_result,
                    pending.deciding_player,
                    branch.original_chooser,
                    option,
                    true,
                )
            })
            .map(|(index, option)| {
                let (kind, cost_text) = match &option.cost {
                    ResolutionCost::None => {
                        (rv1::ResolutionBranchCostKind::Unspecified, String::new())
                    }
                    ResolutionCost::Blight { count } => (
                        rv1::ResolutionBranchCostKind::Blight,
                        format!("Blight {count}"),
                    ),
                    ResolutionCost::Waterbend(cost) => (
                        rv1::ResolutionBranchCostKind::Waterbend,
                        format!("Waterbend {cost}"),
                    ),
                    ResolutionCost::Mana(cost) => {
                        (rv1::ResolutionBranchCostKind::Mana, cost.to_string())
                    }
                    ResolutionCost::DiscardCard { .. } => {
                        unreachable!("flat authored branches reject discard costs")
                    }
                    ResolutionCost::PutHandCardOnLibraryBottom => {
                        unreachable!("flat authored branches reject hand-bottom costs")
                    }
                    ResolutionCost::ExileGraveyardCard { .. } => {
                        unreachable!("flat authored branches reject graveyard-exile costs")
                    }
                    ResolutionCost::SacrificePermanent { .. } => (
                        rv1::ResolutionBranchCostKind::SacrificePermanent,
                        "sacrifice a matching permanent".into(),
                    ),
                    ResolutionCost::TapPermanents { count, .. } => (
                        rv1::ResolutionBranchCostKind::TapPermanents,
                        format!("tap {count} matching permanents"),
                    ),
                };
                rv1::ResolutionBranchOption {
                    branch_index: index as u32,
                    label: option.fallback_label(),
                    cost_kind: kind as i32,
                    cost_text,
                    selectable: true,
                    search_zones: Vec::new(),
                    presentation: stack_child_presentation_ref(
                        engine.registry,
                        &stack.item.card_id,
                        stack.item.face_index,
                        StackPresentationSource::for_stack(
                            engine
                                .state
                                .stack_presentations
                                .get(&stack.item.id)
                                .and_then(|presentation| presentation.primary.as_ref()),
                            stack.item.ability_text.is_none(),
                        ),
                        PresentationPath::ResolutionBranch(&option.branch_id),
                        &option.presentation,
                        option.fallback_label(),
                    ),
                }
            })
            .collect(),
        PendingResolutionBranchStage::PayingMana { .. }
        | PendingResolutionBranchStage::PayingObjects { .. } => return None,
    };
    Some(rv1::RuledEvent {
        ev: Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(
            rv1::ResolutionChoiceRequired {
                resolution_prompt_id: 0,
                variable_mana_contribution: false,
                deciding_player_id: pending.deciding_player,
                source_object_id: pending.presentation.source_object_id,
                prompt_text: pending.presentation.prompt.clone(),
                choice_kind: rv1::ChoiceKind::ResolutionBranch as i32,
                min: pending.presentation.min,
                max: pending.presentation.max,
                resolution_branches: options,
                ..Default::default()
            },
        )),
    })
}

/// Rebuild the choice visible for Teferi's targeted-player ultimate. The controller's replacement
/// chooser prompt uses stable branch indices; a live target uses current legal permanents while a
/// departed target uses only survivors from its captured LKI cohort.
pub(in crate::engine) fn targeted_player_permanent_choice_event(
    engine: &crate::engine::GameEngine,
) -> Option<rv1::RuledEvent> {
    let pending = engine.state.pending_resolution.as_ref()?;
    let ResolutionContinuation::TargetedPlayerPermanentChoice {
        stack,
        target_player,
        cohort,
        filter,
        constraints,
        candidate_generations,
        stage,
        ..
    } = &pending.continuation
    else {
        return None;
    };
    let (choice_kind, candidate_player_ids, resolution_branches) = match stage {
        PendingTargetedPlayerChoiceStage::ChoosingDelegate { candidates } => {
            let eligible = resolution_choice_delegate_candidates(
                &engine.state,
                stack.item.controller,
                *target_player,
            );
            let options = candidates
                .iter()
                .enumerate()
                .map(|(index, candidate)| {
                    let selectable = candidate.is_some_and(|candidate| {
                        eligible.contains(&candidate)
                            && engine
                                .state
                                .player_idx(candidate)
                                .is_some_and(|idx| !engine.state.players[idx].has_lost)
                    });
                    let label = candidate.map_or_else(
                        || "Unavailable player".into(),
                        |candidate| format!("P{candidate} makes the choice"),
                    );
                    rv1::ResolutionBranchOption {
                        branch_index: index as u32,
                        label: label.clone(),
                        cost_kind: rv1::ResolutionBranchCostKind::Unspecified as i32,
                        cost_text: String::new(),
                        selectable,
                        search_zones: Vec::new(),
                        presentation: Some(rv1::PresentationRef {
                            fallback_text: label,
                            ..Default::default()
                        }),
                    }
                })
                .collect();
            (
                rv1::ChoiceKind::ResolutionBranch,
                Vec::<i32>::new(),
                options,
            )
        }
        PendingTargetedPlayerChoiceStage::ChoosingPermanent => {
            let target_live = engine
                .state
                .player_idx(*target_player)
                .is_some_and(|index| !engine.state.players[index].has_lost);
            let eligible_objects = if target_live {
                current_targeted_player_permanent_candidates(
                    engine,
                    &stack.item,
                    *target_player,
                    filter,
                    constraints,
                )
            } else {
                cohort
                    .permanents
                    .iter()
                    .map(|member| member.object_id)
                    .collect()
            };
            let candidates = pending
                .presentation
                .candidates
                .iter()
                .copied()
                .filter(|object_id| {
                    eligible_objects.contains(object_id)
                        && candidate_generations.iter().any(|(candidate, generation)| {
                            *candidate == *object_id
                                && engine
                                    .state
                                    .zone_change_generation
                                    .get(object_id)
                                    .copied()
                                    .unwrap_or(0)
                                    == *generation
                                && engine
                                    .state
                                    .objects
                                    .get(object_id)
                                    .is_some_and(|object| object.zone == Zone::Battlefield)
                        })
                })
                .collect::<Vec<_>>();
            let card_ids = candidates
                .iter()
                .map(|object_id| {
                    engine
                        .state
                        .objects
                        .get(object_id)
                        .map(|object| object.card_id.clone())
                        .unwrap_or_default()
                })
                .collect::<Vec<_>>();
            let names = candidates
                .iter()
                .map(|object_id| object_display_name(&engine.state, engine.registry, *object_id))
                .collect::<Vec<_>>();
            return Some(rv1::RuledEvent {
                ev: Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(
                    rv1::ResolutionChoiceRequired {
                        resolution_prompt_id: 0,
                        variable_mana_contribution: false,
                        candidate_player_ids: Vec::new(),
                        deciding_player_id: pending.deciding_player,
                        source_object_id: pending.presentation.source_object_id,
                        prompt_text: pending.presentation.prompt.clone(),
                        choice_kind: rv1::ChoiceKind::PermanentObjects as i32,
                        candidate_object_ids: candidates,
                        candidate_card_ids: card_ids,
                        candidate_names: names,
                        min: pending.presentation.min,
                        max: pending.presentation.max,
                        ..Default::default()
                    },
                )),
            });
        }
    };
    Some(rv1::RuledEvent {
        ev: Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(
            rv1::ResolutionChoiceRequired {
                resolution_prompt_id: 0,
                variable_mana_contribution: false,
                candidate_player_ids,
                deciding_player_id: pending.deciding_player,
                source_object_id: stack.item.id,
                prompt_text: pending.presentation.prompt.clone(),
                choice_kind: choice_kind as i32,
                resolution_branches,
                min: pending.presentation.min,
                max: pending.presentation.max,
                ..Default::default()
            },
        )),
    })
}

pub(in crate::engine) fn authored_resolution_branch_has_selectable_option(
    engine: &crate::engine::GameEngine,
) -> bool {
    let Some(pending) = engine.state.pending_resolution.as_ref() else {
        return false;
    };
    let ResolutionContinuation::AuthoredBranch { stack, branch } = &pending.continuation else {
        return false;
    };
    matches!(branch.stage, PendingResolutionBranchStage::Selecting)
        && branch.branches.iter().any(|option| {
            resolution_branch_is_selectable(
                engine,
                &stack.item,
                &stack.previous_result,
                pending.deciding_player,
                branch.original_chooser,
                option,
                true,
            )
        })
}

pub(super) fn create_reflexive_trigger(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::CreateReflexiveTrigger { when, ability } = effect else {
        unreachable!();
    };
    let source_id = cx
        .top
        .source_permanent_id
        .ok_or(EngineError::Illegal("reflexive trigger source missing"))?;
    if when.as_ref().is_some_and(|condition| {
        !previous_result_receipt_matches(cx.engine, cx.top, cx.previous_effect_result, condition)
    }) {
        return Ok(EffectOutcome::Continue);
    }
    if !cx.engine.intervening_if_holds_at_generation(
        source_id,
        cx.controller,
        ability.intervening_if.as_ref(),
        Some(cx.top.source_zone_change),
        Some(&cx.top.trigger_context),
    ) {
        return Ok(EffectOutcome::Continue);
    }
    let object_id = cx.engine.state.next_object_id;
    cx.engine.state.next_object_id += 1;
    let card_name = cx
        .engine
        .registry
        .get(&cx.top.card_id)
        .map(|definition| definition.name.clone())
        .unwrap_or_else(|| cx.top.card_id.clone());
    let ability_text = ability.fallback_text(&card_name);
    let definition = cx.engine.ability_definition(
        source_id,
        cx.top.face_index,
        vec![ability.ability_id.clone()],
    );
    let presentation = Some(ability_presentation(
        cx.engine.registry,
        &definition,
        &ability.presentation,
        ability_text.clone(),
    ));
    cx.engine
        .state
        .staged_trigger_groups
        .push_back(StagedTriggerGroup {
            triggers: vec![StagedTrigger {
                object_id,
                source_permanent_id: source_id,
                source_owner: cx.top.source_owner.unwrap_or(cx.controller),
                source_face_index: cx.top.face_index,
                source_zone_change: cx.top.source_zone_change,
                source_face_change: cx.top.source_face_change,
                card_id: cx.top.card_id.clone(),
                card_name,
                controller: cx.controller,
                ability_index: 0,
                ability_origin: None,
                ability: TriggeredAbilityDef {
                    ability_id: ability.ability_id,
                    presentation: ability.presentation,
                    // This definition is staged directly; the condition is never scanned.
                    trigger: TriggerCondition::WhenSelfEntersBattlefield,
                    effect: ability.effect,
                    program: None,
                    modal: None,
                    targeting: ability.targeting,
                    may: false,
                    intervening_if: ability.intervening_if,
                    max_triggers_per_turn: None,
                    triggers_only_once: false,
                },
                ability_text,
                presentation,
                trigger_context: TriggerContext::default(),
                may: false,
            }],
        });
    Ok(EffectOutcome::Continue)
}

#[cfg(test)]
mod result_count_tests {
    use super::*;
    use crate::state::{CardResultCohort, CardResultEntry};
    use tricerules_card_model::primitives::{
        Amount, CardResultAction, CardResultFilter, CardResultSource, CardTypeFilter,
        CountExpression, PlayerRecipient, RelativePlayerSet, ResultType,
    };

    fn characteristic_sum_effect(players: RelativePlayerSet) -> SpellEffectKind {
        SpellEffectKind::DamagePlayer {
            amount: Amount::Count(CountExpression::CardResultCharacteristicSum {
                filter: CardResultFilter {
                    source: CardResultSource::PreviousEffect,
                    action: CardResultAction::Mill,
                    players,
                    card_type: None,
                },
                characteristic: BattlefieldQuantityCharacteristic::ManaValue,
            }),
            who: PlayerRecipient::Controller,
        }
    }

    fn card_result_sum_type(who: PlayerRecipient) -> ResultType {
        ResultType::CardResultCohort {
            action: CardResultAction::Mill,
            who,
            cardinality: tricerules_card_model::primitives::Cardinality {
                min: 0,
                max: Some(10),
            },
        }
    }

    fn card_result_entry(
        affected_player: PlayerId,
        object_id: ObjectId,
        zone_change_generation: u64,
        provenance: crate::state::CardResultProvenance,
    ) -> CardResultEntry {
        CardResultEntry {
            action: CardResultAction::Mill,
            affected_player,
            object_id,
            zone_change_generation,
            matched_card_types: Vec::new(),
            provenance,
        }
    }

    fn cohort_value(cards: Vec<CardResultEntry>) -> ProgramValue {
        ProgramValue::CardResultCohort(CardResultCohort {
            cards,
            program_discard_result: None,
        })
    }

    #[test]
    fn windfall_grouped_maximum_filters_receipts_and_deduplicates_generations() {
        let engine = crate::engine::GameEngine::new(
            tricerules_cards::registry::global(),
            505_060,
            &[4, 9, 27],
            20,
            None,
            true,
        )
        .unwrap();
        let entry = |player, oid, generation, action, kind| CardResultEntry {
            affected_player: player,
            object_id: oid,
            zone_change_generation: generation,
            action,
            matched_card_types: vec![kind],
            provenance: crate::state::CardResultProvenance::ActionFacts,
        };
        let land = CardTypeFilter::Land;
        let discard = CardResultAction::Discard;
        let first = entry(4, 100, 1, discard, land);
        let previous: crate::state::EffectResult = CardResultCohort {
            cards: vec![
                first.clone(),
                first,
                entry(4, 100, 2, discard, land),
                entry(9, 101, 1, discard, land),
                entry(9, 102, 1, discard, land),
                entry(9, 103, 1, discard, CardTypeFilter::Creature),
                entry(27, 104, 1, CardResultAction::Mill, land),
            ],
            program_discard_result: None,
        }
        .into();
        let payment = CardResultCohort {
            cards: vec![entry(27, 105, 1, discard, land)],
            program_discard_result: None,
        };
        let mut filter = CardResultFilter {
            source: CardResultSource::PreviousEffect,
            action: discard,
            players: RelativePlayerSet::All,
            card_type: None,
        };
        let maximum = |filter: &CardResultFilter| {
            card_result_maximum_from_cohorts(&engine.state, 4, &payment, &previous, filter)
        };
        assert_eq!(maximum(&filter), 3);
        assert_eq!(
            card_result_count_from_cohorts(&engine.state, 4, &payment, &previous, &filter),
            5,
            "flat count is unchanged"
        );
        filter.card_type = Some(land);
        assert_eq!(maximum(&filter), 2);
        filter.players = RelativePlayerSet::Controller;
        assert_eq!(
            maximum(&filter),
            2,
            "new generations count separately; duplicates do not"
        );
        filter.players = RelativePlayerSet::Opponents;
        filter.card_type = None;
        assert_eq!(maximum(&filter), 3);
        filter.source = CardResultSource::Payment;
        assert_eq!(maximum(&filter), 1);
        filter.action = CardResultAction::Mill;
        assert_eq!(maximum(&filter), 0);
        assert_eq!(
            card_result_maximum_from_cohorts(
                &engine.state,
                4,
                &CardResultCohort::default(),
                &crate::state::EffectResult::default(),
                &filter
            ),
            0
        );
    }

    #[test]
    fn opponent_filter_is_player_set_generic() {
        let engine = crate::engine::GameEngine::new(
            tricerules_cards::registry::global(),
            122_009,
            &[0, 1],
            20,
            None,
            true,
        )
        .unwrap();
        let previous_result: crate::state::EffectResult = CardResultCohort {
            cards: vec![
                CardResultEntry {
                    action: CardResultAction::Discard,
                    affected_player: 0,
                    object_id: 1,
                    zone_change_generation: 1,
                    matched_card_types: vec![CardTypeFilter::Land],
                    provenance: crate::state::CardResultProvenance::ActionFacts,
                },
                CardResultEntry {
                    action: CardResultAction::Discard,
                    affected_player: 1,
                    object_id: 2,
                    zone_change_generation: 1,
                    matched_card_types: vec![CardTypeFilter::Land],
                    provenance: crate::state::CardResultProvenance::ActionFacts,
                },
                CardResultEntry {
                    action: CardResultAction::Discard,
                    affected_player: 2,
                    object_id: 3,
                    zone_change_generation: 1,
                    matched_card_types: vec![CardTypeFilter::Land],
                    provenance: crate::state::CardResultProvenance::ActionFacts,
                },
            ],
            program_discard_result: None,
        }
        .into();
        let filter = CardResultFilter {
            source: CardResultSource::PreviousEffect,
            action: CardResultAction::Discard,
            players: RelativePlayerSet::Opponents,
            card_type: Some(CardTypeFilter::Land),
        };

        assert_eq!(
            card_result_count_from_cohorts(
                &engine.state,
                0,
                &CardResultCohort::default(),
                &previous_result,
                &filter,
            ),
            2
        );
    }

    #[test]
    fn mana_value_card_result_input_preserves_runtime_type_and_movement_evidence() {
        let engine = crate::engine::GameEngine::new(
            tricerules_cards::registry::global(),
            122_010,
            &[10, 20],
            20,
            None,
            true,
        )
        .unwrap();
        let source_id = *engine
            .state
            .objects
            .keys()
            .next()
            .expect("default decks contain a source object");
        let mut top = engine.observer_return_item(source_id, 10);
        top.trigger_context.affected_player = Some(20);
        let controller_effect = characteristic_sum_effect(RelativePlayerSet::Controller);
        let controller_result_type = card_result_sum_type(PlayerRecipient::Controller);

        assert!(
            validate_program_card_result_characteristic_sum_input(
                &engine,
                &top,
                &controller_effect,
                &controller_result_type,
                &ProgramValue::Count(0),
            )
            .is_err(),
            "a wrong runtime variant must not be flattened into an empty result"
        );
        assert!(
            validate_program_card_result_characteristic_sum_input(
                &engine,
                &top,
                &controller_effect,
                &controller_result_type,
                &cohort_value(Vec::new()),
            )
            .is_ok(),
            "a present, correctly typed empty cohort sums to zero"
        );

        let affected_player_effect = characteristic_sum_effect(RelativePlayerSet::All);
        let affected_player_result_type = card_result_sum_type(PlayerRecipient::AffectedPlayer);
        let wrong_recipient = cohort_value(vec![card_result_entry(
            10,
            900,
            2,
            crate::state::CardResultProvenance::ActionFacts,
        )]);
        assert!(
            validate_program_card_result_characteristic_sum_input(
                &engine,
                &top,
                &affected_player_effect,
                &affected_player_result_type,
                &wrong_recipient,
            )
            .is_err(),
            "All-player filter membership cannot replace the producer's exact recipient"
        );

        let public_receipt = |owner, outside_stack_mana_value| {
            crate::state::CardResultProvenance::CommittedMovement {
                input_zone: Zone::Library,
                input_generation: 1,
                owner,
                actual_output_zone: Zone::Graveyard,
                output_visibility: crate::state::CardResultOutputVisibility::FaceUpPublic,
                outside_stack_mana_value,
            }
        };
        let missing_snapshot = cohort_value(vec![card_result_entry(
            10,
            901,
            2,
            public_receipt(10, None),
        )]);
        assert!(
            validate_program_card_result_characteristic_sum_input(
                &engine,
                &top,
                &controller_effect,
                &controller_result_type,
                &missing_snapshot,
            )
            .is_err(),
            "public Library-to-Graveyard evidence needs its event-time mana value"
        );

        let wrong_library_owner = cohort_value(vec![card_result_entry(
            10,
            902,
            2,
            public_receipt(20, Some(3)),
        )]);
        assert!(
            validate_program_card_result_characteristic_sum_input(
                &engine,
                &top,
                &controller_effect,
                &controller_result_type,
                &wrong_library_owner,
            )
            .is_err(),
            "a Mill receipt must belong to the affected library player"
        );

        let valid_receipt = card_result_entry(10, 903, 2, public_receipt(10, Some(3)));
        let valid_cohort = cohort_value(vec![valid_receipt.clone()]);
        assert!(validate_program_card_result_characteristic_sum_input(
            &engine,
            &top,
            &controller_effect,
            &controller_result_type,
            &valid_cohort,
        )
        .is_ok());
        let previous_result: crate::state::EffectResult = CardResultCohort {
            cards: vec![valid_receipt],
            program_discard_result: None,
        }
        .into();
        let (filter, _) = controller_effect
            .previous_effect_card_result_characteristic_sum()
            .expect("test effect has a previous-effect characteristic sum");
        assert_eq!(
            card_result_characteristic_sum(
                &engine,
                &top,
                &previous_result,
                filter,
                BattlefieldQuantityCharacteristic::ManaValue,
            ),
            3,
            "the sum reads the receipt snapshot even after its object has left the live registry"
        );
    }

    #[test]
    fn exile_power_sum_requires_and_reads_the_exact_input_generation_snapshot() {
        let mut engine = crate::engine::GameEngine::new(
            tricerules_cards::registry::global(),
            122_011,
            &[10, 20],
            20,
            None,
            true,
        )
        .unwrap();
        let source_id = *engine
            .state
            .objects
            .keys()
            .next()
            .expect("default decks contain a source object");
        let top = engine.observer_return_item(source_id, 10);
        let effect = SpellEffectKind::GainLife {
            amount: Amount::Count(CountExpression::CardResultCharacteristicSum {
                filter: CardResultFilter {
                    source: CardResultSource::PreviousEffect,
                    action: CardResultAction::Exile,
                    players: RelativePlayerSet::All,
                    card_type: None,
                },
                characteristic: BattlefieldQuantityCharacteristic::Power,
            }),
            who: PlayerRecipient::ControllerOfTargetGroup { group_index: 0 },
        };
        let result_type = ResultType::TargetCardResultCohort {
            action: CardResultAction::Exile,
            cardinality: tricerules_card_model::primitives::Cardinality {
                min: 0,
                max: Some(1),
            },
            target_group_index: 0,
        };
        let receipt = CardResultEntry {
            action: CardResultAction::Exile,
            affected_player: 20,
            object_id: 900,
            zone_change_generation: 2,
            matched_card_types: vec![CardTypeFilter::Creature],
            provenance: crate::state::CardResultProvenance::CommittedMovement {
                input_zone: Zone::Battlefield,
                input_generation: 1,
                owner: 20,
                actual_output_zone: Zone::Exile,
                output_visibility: crate::state::CardResultOutputVisibility::FaceUpPublic,
                outside_stack_mana_value: None,
            },
        };
        let value = cohort_value(vec![receipt.clone()]);
        let previous_result: crate::state::EffectResult = CardResultCohort {
            cards: vec![receipt],
            program_discard_result: None,
        }
        .into();
        engine
            .state
            .last_known_pt_by_generation
            .insert((900, 1), (Some(4), Some(5)));
        engine
            .state
            .last_known_pt_by_generation
            .insert((900, 2), (Some(8), Some(9)));
        engine
            .state
            .last_known_pt_by_generation
            .insert((900, 5), (Some(11), Some(12)));
        engine.state.zone_change_generation.insert(900, 5);

        assert!(validate_program_card_result_characteristic_sum_input(
            &engine,
            &top,
            &effect,
            &result_type,
            &value,
        )
        .is_ok());
        let mut filtered_effect = effect.clone();
        let SpellEffectKind::GainLife {
            amount: Amount::Count(CountExpression::CardResultCharacteristicSum { filter, .. }),
            ..
        } = &mut filtered_effect
        else {
            panic!("the tested Swords effect has its Exile Power sum in GainLife");
        };
        filter.card_type = Some(CardTypeFilter::Creature);
        assert!(
            validate_program_card_result_characteristic_sum_input(
                &engine,
                &top,
                &filtered_effect,
                &result_type,
                &value,
            )
            .is_err(),
            "runtime projection rejects a redundant result card-type filter"
        );
        let (filter, characteristic) = effect
            .previous_effect_card_result_characteristic_sum()
            .expect("test effect has an Exile Power sum");
        assert_eq!(
            card_result_characteristic_sum(&engine, &top, &previous_result, filter, characteristic),
            4,
            "the sum uses LKI for the movement input generation"
        );

        engine.state.last_known_pt_by_generation.remove(&(900, 1));
        assert!(matches!(
            validate_program_card_result_characteristic_sum_input(
                &engine,
                &top,
                &effect,
                &result_type,
                &value,
            ),
            Err(EngineError::ProgramInvariant(_))
        ));
    }

    #[test]
    fn exile_power_sum_accepts_typed_empty_for_damage_and_keeps_receipt_invariants() {
        let mut engine = crate::engine::GameEngine::new(
            tricerules_cards::registry::global(),
            122_013,
            &[10, 20],
            20,
            None,
            true,
        )
        .unwrap();
        let source_id = *engine
            .state
            .objects
            .keys()
            .next()
            .expect("default decks contain a source object");
        let top = engine.observer_return_item(source_id, 10);
        let effect = SpellEffectKind::DamagePlayer {
            amount: Amount::Count(CountExpression::CardResultCharacteristicSum {
                filter: CardResultFilter {
                    source: CardResultSource::PreviousEffect,
                    action: CardResultAction::Exile,
                    players: RelativePlayerSet::All,
                    card_type: None,
                },
                characteristic: BattlefieldQuantityCharacteristic::Power,
            }),
            who: PlayerRecipient::EachOpponent,
        };
        let result_type = ResultType::TargetCardResultCohort {
            action: CardResultAction::Exile,
            cardinality: tricerules_card_model::primitives::Cardinality {
                min: 0,
                max: Some(1),
            },
            target_group_index: 0,
        };
        let empty = cohort_value(Vec::new());
        assert!(
            validate_program_card_result_characteristic_sum_input(
                &engine,
                &top,
                &effect,
                &result_type,
                &empty,
            )
            .is_ok(),
            "a present, correctly typed empty Exile cohort is a zero sum for any admitted consumer"
        );
        assert!(validate_program_card_result_characteristic_sum_input(
            &engine,
            &top,
            &effect,
            &result_type,
            &ProgramValue::Count(0),
        )
        .is_err());

        let mut wrong_result_action = result_type.clone();
        let ResultType::TargetCardResultCohort { action, .. } = &mut wrong_result_action else {
            unreachable!();
        };
        *action = CardResultAction::Destroy;
        assert!(validate_program_card_result_characteristic_sum_input(
            &engine,
            &top,
            &effect,
            &wrong_result_action,
            &empty,
        )
        .is_err());

        let mut wrong_scope = effect.clone();
        let SpellEffectKind::DamagePlayer {
            amount: Amount::Count(CountExpression::CardResultCharacteristicSum { filter, .. }),
            ..
        } = &mut wrong_scope
        else {
            unreachable!();
        };
        filter.players = RelativePlayerSet::Controller;
        assert!(validate_program_card_result_characteristic_sum_input(
            &engine,
            &top,
            &wrong_scope,
            &result_type,
            &empty,
        )
        .is_err());

        let mut redundant_card_type = effect.clone();
        let SpellEffectKind::DamagePlayer {
            amount: Amount::Count(CountExpression::CardResultCharacteristicSum { filter, .. }),
            ..
        } = &mut redundant_card_type
        else {
            unreachable!();
        };
        filter.card_type = Some(CardTypeFilter::Creature);
        assert!(validate_program_card_result_characteristic_sum_input(
            &engine,
            &top,
            &redundant_card_type,
            &result_type,
            &empty,
        )
        .is_err());

        let receipt = CardResultEntry {
            action: CardResultAction::Exile,
            affected_player: 20,
            object_id: 902,
            zone_change_generation: 2,
            matched_card_types: vec![CardTypeFilter::Creature],
            provenance: crate::state::CardResultProvenance::CommittedMovement {
                input_zone: Zone::Battlefield,
                input_generation: 1,
                owner: 20,
                actual_output_zone: Zone::Exile,
                output_visibility: crate::state::CardResultOutputVisibility::FaceUpPublic,
                outside_stack_mana_value: None,
            },
        };
        engine
            .state
            .last_known_pt_by_generation
            .insert((902, 1), (Some(4), Some(5)));
        let public = cohort_value(vec![receipt.clone()]);
        assert!(validate_program_card_result_characteristic_sum_input(
            &engine,
            &top,
            &effect,
            &result_type,
            &public,
        )
        .is_ok());

        let mut hidden_receipt = receipt;
        let crate::state::CardResultProvenance::CommittedMovement {
            output_visibility, ..
        } = &mut hidden_receipt.provenance
        else {
            unreachable!();
        };
        *output_visibility = crate::state::CardResultOutputVisibility::UnknownHidden;
        assert!(matches!(
            validate_program_card_result_characteristic_sum_input(
                &engine,
                &top,
                &effect,
                &result_type,
                &cohort_value(vec![hidden_receipt]),
            ),
            Err(EngineError::ProgramInvariant(_))
        ));

        engine.state.last_known_pt_by_generation.remove(&(902, 1));
        assert!(matches!(
            validate_program_card_result_characteristic_sum_input(
                &engine,
                &top,
                &effect,
                &result_type,
                &public,
            ),
            Err(EngineError::ProgramInvariant(_))
        ));
    }

    #[test]
    fn exile_power_sum_keeps_negative_lki_signed_until_amount_clamp() {
        let mut engine = crate::engine::GameEngine::new(
            tricerules_cards::registry::global(),
            122_012,
            &[10, 20],
            20,
            None,
            true,
        )
        .unwrap();
        let source_id = *engine
            .state
            .objects
            .keys()
            .next()
            .expect("default decks contain a source object");
        let top = engine.observer_return_item(source_id, 10);
        let effect = SpellEffectKind::GainLife {
            amount: Amount::Count(CountExpression::CardResultCharacteristicSum {
                filter: CardResultFilter {
                    source: CardResultSource::PreviousEffect,
                    action: CardResultAction::Exile,
                    players: RelativePlayerSet::All,
                    card_type: None,
                },
                characteristic: BattlefieldQuantityCharacteristic::Power,
            }),
            who: PlayerRecipient::ControllerOfTargetGroup { group_index: 0 },
        };
        let result_type = ResultType::TargetCardResultCohort {
            action: CardResultAction::Exile,
            cardinality: tricerules_card_model::primitives::Cardinality {
                min: 0,
                max: Some(1),
            },
            target_group_index: 0,
        };
        let receipt = CardResultEntry {
            action: CardResultAction::Exile,
            affected_player: 20,
            object_id: 901,
            zone_change_generation: 2,
            matched_card_types: vec![CardTypeFilter::Creature],
            provenance: crate::state::CardResultProvenance::CommittedMovement {
                input_zone: Zone::Battlefield,
                input_generation: 1,
                owner: 20,
                actual_output_zone: Zone::Exile,
                output_visibility: crate::state::CardResultOutputVisibility::FaceUpPublic,
                outside_stack_mana_value: None,
            },
        };
        let value = cohort_value(vec![receipt.clone()]);
        let previous_result: crate::state::EffectResult = CardResultCohort {
            cards: vec![receipt],
            program_discard_result: None,
        }
        .into();
        engine
            .state
            .last_known_pt_by_generation
            .insert((901, 1), (Some(-1), Some(4)));

        assert!(validate_program_card_result_characteristic_sum_input(
            &engine,
            &top,
            &effect,
            &result_type,
            &value,
        )
        .is_ok());
        let (filter, characteristic) = effect
            .previous_effect_card_result_characteristic_sum()
            .expect("test effect has an Exile Power sum");
        assert_eq!(
            card_result_characteristic_sum(&engine, &top, &previous_result, filter, characteristic,),
            -1,
            "signed power remains negative in the typed LKI sum"
        );
        let amount = Amount::Count(CountExpression::CardResultCharacteristicSum {
            filter: filter.clone(),
            characteristic,
        });
        assert_eq!(
            engine.resolve_amount(
                &amount,
                AmountContext::for_stack_item(&top, top.controller)
                    .with_previous_effect_result(&previous_result),
            ),
            0,
            "CR 107.1b clamps a negative Amount to zero before life gain"
        );
    }
}
