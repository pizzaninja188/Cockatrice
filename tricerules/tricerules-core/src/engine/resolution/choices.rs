use super::{EffectCx, EffectOutcome};
use crate::engine::events::{ev_log, object_display_name};
use crate::engine::presentation::{
    ability_presentation, stack_child_presentation_ref, PresentationPath, StackPresentationSource,
};
use crate::engine::targeting::TargetSourceIdentity;
use crate::engine::{rv1, ConditionContext, EngineError};
use crate::state::{
    ParkedStackResolution, PendingResolution, PendingResolutionBranch,
    PendingResolutionBranchStage, PendingResolutionPresentation, PendingTargetedPlayerChoiceStage,
    ResolutionContinuation, StackItem, StagedTrigger, StagedTriggerGroup, TriggerContext,
    TriggerObjectRef,
};
use crate::Zone;
use crate::{GameEngine, ObjectId, PlayerId};
use tricerules_cards::primitives::{
    Amount, PermanentChoiceConstraint, PlayerRecipient, ResolutionBranchDef,
    ResolutionBranchRequirement, ResolutionBranchSelection, ResolutionCost, SpellEffectKind,
    TargetController, TargetFilter, TargetKind, TriggerCondition, TriggeredAbilityDef,
};

/// Capture the current control set for a live targeted player. Teferi uses this as the player
/// information to preserve if that player later leaves during its parked resolution choice.
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
                    .condition_object_identity(*recipient, condition_context)
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

fn permanent_choice_prompt(filter: &TargetFilter, min: u32, max: u32) -> String {
    let (singular, plural) = if filter.any_of.is_none() && filter.kind == TargetKind::Creature {
        ("creature", "creatures")
    } else if filter.any_of.is_none()
        && filter.kind == TargetKind::AnyPermanent
        && filter.permanent_types == [tricerules_cards::primitives::PermanentTypeFilter::Land]
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
    } else {
        format!("Choose {min}–{max} {plural}{controller}.")
    }
}

pub(super) fn choose_resolution_branch(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::ChooseResolutionBranch {
        chooser,
        optional,
        selection,
        branches,
        otherwise: _,
    } = effect
    else {
        unreachable!();
    };
    let recipients = super::player_recipients(cx, chooser);
    let [deciding_player] = recipients.as_slice() else {
        return Err(EngineError::Illegal(
            "resolution choice requires exactly one deciding player",
        ));
    };
    let legal = branches
        .iter()
        .enumerate()
        .filter(|(_, branch)| {
            resolution_branch_is_selectable(
                cx.engine,
                cx.top,
                cx.previous_effect_result,
                *deciding_player,
                *deciding_player,
                branch,
            )
        })
        .collect::<Vec<_>>();
    if selection == ResolutionBranchSelection::FirstApplicable {
        let Some((branch_index, branch)) = legal.first() else {
            let chooser_live = cx
                .engine
                .state
                .player_idx(*deciding_player)
                .is_some_and(|index| !cx.engine.state.players[index].has_lost);
            if !chooser_live {
                cx.events.push(ev_log(
                    "No cost-free resolution branch remains for the departed chooser.".into(),
                ));
                return Ok(EffectOutcome::RestartResolutionBranch(None));
            }
            return Err(EngineError::Illegal(
                "automatic resolution branch has no applicable fallback",
            ));
        };
        cx.events.push(ev_log(format!(
            "P{} resolves: {}.",
            deciding_player,
            branch.fallback_label()
        )));
        return Ok(EffectOutcome::RestartResolutionBranch(Some(*branch_index)));
    }
    match (optional, legal.as_slice()) {
        (false, []) => {
            cx.events.push(ev_log(format!(
                "P{} has no legal resolution branch.",
                deciding_player
            )));
            Ok(EffectOutcome::RestartResolutionBranch(None))
        }
        // A mandatory branch may auto-resolve only when it has nothing left to pay. A costed
        // branch still needs its payment (and, for object costs, the printed permanent choice),
        // so it parks through the shared branch-payment path instead of skipping the cost.
        (false, [(branch_index, branch)]) if branch.cost == ResolutionCost::None => {
            cx.events.push(ev_log(format!(
                "P{} chooses: {}.",
                deciding_player,
                branch.fallback_label()
            )));
            Ok(EffectOutcome::RestartResolutionBranch(Some(*branch_index)))
        }
        (true, []) => {
            cx.events.push(ev_log(format!(
                "P{} declines the optional resolution choice.",
                deciding_player
            )));
            Ok(EffectOutcome::RestartResolutionBranch(None))
        }
        _ => park_resolution_branches_for(cx, chooser, optional, branches),
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
            super::target_player_from_group(cx.targets, cx.target_group_indices, group_index)
                .ok_or(EngineError::Illegal(
                    "permanent choice is missing its targeted player",
                ))?,
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
    let targeted_player_cohort = targeted_player
        .map(|player_id| current_targeted_player_control_cohort(cx.engine, player_id));
    cx.effect_result.targeted_player_control_cohort = targeted_player_cohort.clone();
    let candidates = if targeted_player.is_some() {
        current_targeted_player_permanent_candidates(
            cx.engine,
            cx.top,
            deciding_player,
            &filter,
            &constraints,
        )
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
                        .condition_object_identity(*recipient, condition_context)
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
    let max = max.min(candidates.len() as u32);
    if min == max && candidates.len() == min as usize {
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
                .unwrap_or_else(|| format!("[object {oid}]"))
        })
        .collect::<Vec<_>>();
    let prompt = permanent_choice_prompt(&filter, min, max);
    cx.events.push(rv1::RuledEvent {
        ev: Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(
            rv1::ResolutionChoiceRequired {
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
                stack: ParkedStackResolution::new(cx.top.clone())
                    .with_previous_result(cx.previous_effect_result.clone()),
                target_player,
                cohort,
                filter,
                constraints,
                candidate_generations,
                stage: PendingTargetedPlayerChoiceStage::ChoosingPermanent,
            }
        } else {
            ResolutionContinuation::PermanentChoice {
                stack: ParkedStackResolution::new(cx.top.clone())
                    .with_previous_result(cx.previous_effect_result.clone()),
                candidate_generations,
            }
        },
    });
    Ok(EffectOutcome::Suspended)
}

pub(super) fn may_behold(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::MayBehold {
        who,
        hand_filter,
        permanent_filter,
        ..
    } = effect
    else {
        unreachable!();
    };
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
        return Ok(EffectOutcome::RestartResolutionBranch(None));
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
            stack: ParkedStackResolution::new(cx.top.clone())
                .with_previous_result(cx.previous_effect_result.clone()),
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
    previous_result: &crate::state::EffectResult,
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
        ResolutionBranchRequirement::PreviousResultReceipt(condition) => {
            previous_result_receipt_matches(engine, top, previous_result, condition)
        }
        ResolutionBranchRequirement::CardResultCount { filter, min, max } => {
            let count = card_result_count(engine, top, previous_result, filter);
            min.is_none_or(|minimum| count >= minimum) && max.is_none_or(|maximum| count <= maximum)
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
) -> bool {
    let original_chooser_live = engine
        .state
        .player_idx(original_chooser)
        .is_some_and(|index| !engine.state.players[index].has_lost);
    (original_chooser_live || branch.cost == ResolutionCost::None)
        && resolution_branch_is_live(engine, top, previous_result, deciding_player, branch)
}

fn previous_result_receipt_matches(
    engine: &crate::engine::GameEngine,
    top: &StackItem,
    previous_result: &crate::state::EffectResult,
    condition: &tricerules_cards::primitives::ResolutionReceiptCondition,
) -> bool {
    use tricerules_cards::primitives::ResolutionReceiptCondition;

    match condition {
        ResolutionReceiptCondition::CounterUnlessPaid { paid: expected } => matches!(
            previous_result.receipt,
            Some(crate::state::ResolutionReceipt::CounterUnlessPaid { paid }) if paid == *expected
        ),
        ResolutionReceiptCondition::CountersPlaced { counter, object } => {
            let Some((object_id, zone_change_generation)) =
                engine.condition_object_identity(*object, ConditionContext::for_stack_item(top))
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
    }
}

pub(in crate::engine) fn card_result_count(
    engine: &crate::engine::GameEngine,
    top: &StackItem,
    previous_result: &crate::state::EffectResult,
    filter: &tricerules_cards::primitives::CardResultFilter,
) -> u32 {
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
    filter: &tricerules_cards::primitives::CardResultFilter,
    player: crate::state::PlayerId,
) -> u32 {
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
    filter: &tricerules_cards::primitives::CardResultFilter,
) -> u32 {
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
    filter: &tricerules_cards::primitives::CardResultFilter,
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

pub(in crate::engine) fn card_result_characteristic_sum(
    engine: &crate::engine::GameEngine,
    top: &StackItem,
    previous_result: &crate::state::EffectResult,
    filter: &tricerules_cards::primitives::CardResultFilter,
    characteristic: tricerules_cards::primitives::PowerToughnessCharacteristic,
) -> i64 {
    matching_card_result_entries(
        &engine.state,
        top.controller,
        &top.payment_result,
        previous_result,
        filter,
    )
    .fold(0_i64, |sum, entry| {
        let (power, toughness) =
            engine.object_power_toughness(entry.object_id, engine.card_result_generation(entry));
        let value = match characteristic {
            tricerules_cards::primitives::PowerToughnessCharacteristic::Power => power,
            tricerules_cards::primitives::PowerToughnessCharacteristic::Toughness => toughness,
        };
        sum.saturating_add(value)
    })
}

pub(in crate::engine) fn card_result_mana_value_sum(
    engine: &crate::engine::GameEngine,
    top: &StackItem,
    previous_result: &crate::state::EffectResult,
) -> i64 {
    let mut seen = std::collections::BTreeSet::new();
    previous_result
        .cards
        .iter()
        .filter(|entry| {
            entry.action == tricerules_cards::primitives::CardResultAction::Mill
                && entry.affected_player == top.controller
                && seen.insert((entry.object_id, entry.zone_change_generation))
        })
        .filter_map(|entry| {
            let current_generation = engine
                .state
                .zone_change_generation
                .get(&entry.object_id)
                .copied()
                .unwrap_or(0);
            if current_generation != entry.zone_change_generation {
                return None;
            }
            let object = engine.state.objects.get(&entry.object_id)?;
            let definition = engine.registry.get(&object.card_id)?;
            Some(i64::from(definition.mana_value_outside_stack()))
        })
        .fold(0_i64, i64::saturating_add)
}

fn card_result_count_from_cohorts(
    state: &crate::state::GameState,
    controller: i32,
    payment_result: &crate::state::CardResultCohort,
    previous_result: &crate::state::EffectResult,
    filter: &tricerules_cards::primitives::CardResultFilter,
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
    filter: &'a tricerules_cards::primitives::CardResultFilter,
) -> impl Iterator<Item = &'a crate::state::CardResultEntry> + 'a {
    let cards = match filter.source {
        tricerules_cards::primitives::CardResultSource::Payment => &payment_result.cards,
        tricerules_cards::primitives::CardResultSource::PreviousEffect => &previous_result.cards,
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
    branches: Vec<tricerules_cards::primitives::ResolutionBranchDef>,
) -> Result<EffectOutcome, EngineError> {
    park_resolution_branches_for(
        cx,
        tricerules_cards::primitives::PlayerRecipient::Controller,
        optional,
        branches,
    )
}

fn park_resolution_branches_for(
    cx: &mut EffectCx<'_>,
    chooser: PlayerRecipient,
    optional: bool,
    branches: Vec<tricerules_cards::primitives::ResolutionBranchDef>,
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

/// Rebuild the current authored-branch prompt from its serialized continuation. This is also used
/// after a player leaves, so a stale client prompt cannot retain a departed delegate.
pub(in crate::engine) fn authored_resolution_branch_choice_event(
    engine: &crate::engine::GameEngine,
) -> Option<rv1::RuledEvent> {
    let pending = engine.state.pending_resolution.as_ref()?;
    let ResolutionContinuation::AuthoredBranch { stack, branch } = &pending.continuation else {
        return None;
    };
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
                    ResolutionCost::DiscardCard { .. } => (
                        rv1::ResolutionBranchCostKind::DiscardCard,
                        "discard a matching card".into(),
                    ),
                    ResolutionCost::ExileGraveyardCard { .. } => (
                        rv1::ResolutionBranchCostKind::ExileGraveyardCard,
                        "exile a matching card from your graveyard".into(),
                    ),
                    ResolutionCost::PutHandCardOnLibraryBottom => (
                        rv1::ResolutionBranchCostKind::PutHandCardOnLibraryBottom,
                        "put a card from your hand on the bottom of your library".into(),
                    ),
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
                ability: TriggeredAbilityDef {
                    ability_id: ability.ability_id,
                    presentation: ability.presentation,
                    // This definition is staged directly; the condition is never scanned.
                    trigger: TriggerCondition::WhenSelfEntersBattlefield,
                    effect: ability.effect,
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
    use tricerules_cards::primitives::{
        CardResultAction, CardResultFilter, CardResultSource, CardTypeFilter, RelativePlayerSet,
    };

    #[test]
    fn windfall_grouped_maximum_filters_receipts_and_deduplicates_generations() {
        let engine = crate::engine::GameEngine::new(505_060, &[4, 9, 27], 20, None, true).unwrap();
        let entry = |player, oid, generation, action, kind| CardResultEntry {
            affected_player: player,
            object_id: oid,
            zone_change_generation: generation,
            action,
            matched_card_types: vec![kind],
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
        }
        .into();
        let payment = CardResultCohort {
            cards: vec![entry(27, 105, 1, discard, land)],
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
        let engine = crate::engine::GameEngine::new(122_009, &[0, 1], 20, None, true).unwrap();
        let previous_result: crate::state::EffectResult = CardResultCohort {
            cards: vec![
                CardResultEntry {
                    action: CardResultAction::Discard,
                    affected_player: 0,
                    object_id: 1,
                    zone_change_generation: 1,
                    matched_card_types: vec![CardTypeFilter::Land],
                },
                CardResultEntry {
                    action: CardResultAction::Discard,
                    affected_player: 1,
                    object_id: 2,
                    zone_change_generation: 1,
                    matched_card_types: vec![CardTypeFilter::Land],
                },
                CardResultEntry {
                    action: CardResultAction::Discard,
                    affected_player: 2,
                    object_id: 3,
                    zone_change_generation: 1,
                    matched_card_types: vec![CardTypeFilter::Land],
                },
            ],
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
}
