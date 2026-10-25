//! Typed, engine-owned operations for retained library cohorts and their later consumers.

use super::*;
use crate::state::ProgramRevealedLibraryCohort;

fn program_cohort_reveal_id(cx: &EffectCx<'_>) -> String {
    let occurrence = cx.program_state.as_ref().map_or_else(
        || format!("uncompiled:{}", cx.effect_index),
        |program| {
            let frames = program
                .frames
                .iter()
                .map(|frame| {
                    let path = frame
                        .path
                        .iter()
                        .map(|step| format!("{}:{:?}", step.instruction_index, step.arm))
                        .collect::<Vec<_>>()
                        .join(".");
                    format!("{}[{path}]", frame.next_instruction)
                })
                .collect::<Vec<_>>()
                .join("/");
            format!(
                "{:?}:{}:{frames}",
                program.owner,
                program.pending_owners.len()
            )
        },
    );
    format!(
        "program:{}:{}:{}:{}:{occurrence}",
        cx.engine.state.command_index, cx.top.id, cx.top.source_zone_change, cx.effect_index
    )
}

pub(super) fn reveal_cohort(cx: &mut EffectCx<'_>) -> Result<EffectOutcome, EngineError> {
    let Some(ProgramValue::FoundLibraryCohort(cohort)) =
        cx.previous_effect_result.program_value.as_ref()
    else {
        return Err(EngineError::ProgramInvariant(
            "RevealCohort requires a retained library-search result",
        ));
    };
    // The search result slot is immutable. A legal earlier instruction may have moved some of
    // its exact members, so publish only the still-current incarnations without rebinding them.
    let live = cohort
        .iter()
        .copied()
        .filter(|card| program_library_ref_is_current(cx.engine, card))
        .collect::<Vec<_>>();
    let object_ids = live.iter().map(|card| card.object_id).collect::<Vec<_>>();
    let reveal_id = program_cohort_reveal_id(cx);
    let mut reveal_events = crate::engine::reveals::reveal_cards(
        &cx.engine.state,
        cx.engine.registry,
        &object_ids,
        cx.top.id,
        cx.spell_label,
    );
    let public_reveal = reveal_events.iter_mut().find_map(|event| {
        let Some(rv1::ruled_event::Ev::CardsRevealed(reveal)) = event.ev.as_mut() else {
            return None;
        };
        reveal.reveal_id = reveal_id.clone();
        Some(reveal.clone())
    });
    cx.events.extend(reveal_events);
    cx.effect_result.program_value = Some(ProgramValue::RevealedLibraryCohort(
        ProgramRevealedLibraryCohort {
            cards: live,
            public_reveal,
        },
    ));
    Ok(EffectOutcome::Continue)
}

pub(super) fn choose_cohort_partition(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::ChooseCohortPartition { chooser, selection } = effect else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    let Some(ProgramValue::RevealedLibraryCohort(revealed)) =
        cx.previous_effect_result.program_value.as_ref()
    else {
        return Err(EngineError::ProgramInvariant(
            "ChooseCohortPartition requires a revealed library cohort",
        ));
    };
    let live = revealed
        .cards
        .iter()
        .copied()
        .filter(|card| program_library_ref_is_current(cx.engine, card))
        .collect::<Vec<_>>();
    let tricerules_card_model::primitives::CohortPartitionSelection::ExactlyUpTo(limit) = selection;
    let chosen_count = (limit as usize).min(live.len());
    let PlayerRecipient::TargetedPlayer { group_index, .. } = chooser else {
        return Err(EngineError::ProgramInvariant(
            "ChooseCohortPartition lost its targeted player binding",
        ));
    };
    let target_player =
        super::target_player_from_group(cx.targets, cx.target_group_indices, group_index).ok_or(
            EngineError::ProgramInvariant(
                "ChooseCohortPartition is missing its legal target player",
            ),
        )?;

    // CR 701.23 still gives the targeted opponent the choice when the found set contains only
    // one or two cards; choosing the full set is a forced choice, but it is not an omitted choice.
    // Only an actually empty found set auto-completes without a target prompt.
    if live.is_empty() {
        cx.effect_result.program_value = Some(ProgramValue::CardPartition(ProgramCardPartition {
            selected: live,
            remainder: Vec::new(),
            public_reveal: revealed.public_reveal.clone(),
        }));
        return Ok(EffectOutcome::Continue);
    }

    let candidates = live.iter().map(|card| card.object_id).collect::<Vec<_>>();
    let (candidate_card_ids, candidate_names) = super::candidate_identities(cx.engine, &candidates);
    let public_reveal = revealed.public_reveal.clone();
    if public_reveal.is_none() {
        return Err(EngineError::ProgramInvariant(
            "revealed cohort lost its immutable public snapshot",
        ));
    }
    let target_is_live = cx
        .engine
        .state
        .player_idx(target_player)
        .is_some_and(|index| !cx.engine.state.players[index].has_lost);
    if !target_is_live {
        let delegates = super::resolution_choice_delegate_candidates(
            &cx.engine.state,
            cx.controller,
            target_player,
        );
        if delegates.is_empty() {
            return Err(EngineError::ProgramInvariant(
                "departed cohort target has no live CR 800.4g chooser",
            ));
        }
        let chosen_count = chosen_count as u32;
        let (deciding_player, choice_kind, min, max, prompt, stage) = if let [delegate] =
            delegates.as_slice()
        {
            (
                *delegate,
                rv1::ChoiceKind::Revealed,
                chosen_count,
                chosen_count,
                format!(
                    "P{delegate}: choose {chosen_count} of the revealed cards to put into the graveyard."
                ),
                crate::state::PendingProgramCohortPartitionStage::ChoosingPartition,
            )
        } else {
            (
                cx.controller,
                rv1::ChoiceKind::ResolutionBranch,
                1,
                1,
                format!(
                    "P{}: choose a player to make P{target_player}'s library cohort choice.",
                    cx.controller
                ),
                crate::state::PendingProgramCohortPartitionStage::ChoosingDelegate {
                    candidates: delegates.iter().copied().map(Some).collect(),
                },
            )
        };
        let stack = cx.parked_stack();
        let pending = PendingResolution {
            resolution_prompt_id: crate::state::ResolutionPromptId::UNASSIGNED,
            resolution_prompt_semantics: None,
            deciding_player,
            presentation: PendingResolutionPresentation {
                source_object_id: cx.top.id,
                candidates: if matches!(
                    stage,
                    crate::state::PendingProgramCohortPartitionStage::ChoosingDelegate { .. }
                ) {
                    Vec::new()
                } else {
                    candidates.clone()
                },
                min,
                max,
                ordered: false,
                unique_names: false,
                prompt,
                choice_kind,
            },
            continuation: ResolutionContinuation::ProgramCohortPartition {
                stack,
                target_player,
                offered_cohort: live,
                limit,
                public_reveal,
                stage,
            },
        };
        cx.events.push(
            super::super::custom_resolution::library_cohorts::program_cohort_partition_event(
                cx.engine, &pending,
            ),
        );
        cx.events.push(ev_log(pending.presentation.prompt.clone()));
        cx.engine.state.pending_resolution = Some(pending);
        return Ok(EffectOutcome::Suspended);
    }
    let prompt = format!(
        "P{target_player}: choose {chosen_count} of the revealed cards to put into the graveyard."
    );
    let stack = cx.parked_stack();
    let event = rv1::RuledEvent {
        ev: Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(
            rv1::ResolutionChoiceRequired {
                resolution_prompt_id: 0,
                deciding_player_id: target_player,
                source_object_id: cx.top.id,
                prompt_text: prompt.clone(),
                choice_kind: custom::ChoiceKind::Revealed as i32,
                candidate_object_ids: candidates.clone(),
                candidate_card_ids,
                candidate_names,
                min: chosen_count as u32,
                max: chosen_count as u32,
                unique_names: false,
                public_reveal: public_reveal.clone(),
                ..Default::default()
            },
        )),
    };
    cx.events.push(event);
    cx.events.push(ev_log(prompt.clone()));
    cx.engine.state.pending_resolution = Some(PendingResolution {
        resolution_prompt_id: crate::state::ResolutionPromptId::UNASSIGNED,
        resolution_prompt_semantics: None,
        deciding_player: target_player,
        presentation: PendingResolutionPresentation {
            source_object_id: cx.top.id,
            candidates,
            min: chosen_count as u32,
            max: chosen_count as u32,
            ordered: false,
            unique_names: false,
            prompt,
            choice_kind: custom::ChoiceKind::Revealed,
        },
        continuation: ResolutionContinuation::ProgramCohortPartition {
            stack,
            target_player,
            offered_cohort: live,
            limit,
            public_reveal,
            stage: PendingProgramCohortPartitionStage::ChoosingPartition,
        },
    });
    Ok(EffectOutcome::Suspended)
}

pub(super) fn move_cohort_partition(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::MoveCohortPartition {
        selected_to,
        remainder_to,
    } = effect
    else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    let valid_destination = |destination| {
        matches!(
            destination,
            SearchDestination::Graveyard | SearchDestination::Hand
        )
    };
    if !valid_destination(selected_to)
        || !valid_destination(remainder_to)
        || selected_to == remainder_to
    {
        return Err(EngineError::ProgramInvariant(
            "typed library partition has unsupported destinations",
        ));
    }
    let Some(ProgramValue::CardPartition(partition)) =
        cx.previous_effect_result.program_value.as_ref()
    else {
        return Err(EngineError::ProgramInvariant(
            "MoveCohortPartition requires a typed card partition",
        ));
    };
    if partition
        .selected
        .iter()
        .chain(&partition.remainder)
        .any(|card| !program_library_ref_is_current(cx.engine, card))
    {
        return Err(EngineError::ProgramInvariant(
            "card partition became stale before its move transaction",
        ));
    }
    let mut plan = ProgramPartitionMovePlan {
        partition: partition.clone(),
        selected_to,
        remainder_to,
        commander_replacements: Vec::new(),
        commander_choice_indices: Vec::new(),
        commander_choice_cursor: 0,
        graveyard_order: Vec::new(),
        stage: PendingProgramPartitionMoveStage::ReadyToCommit,
    };
    let hand_cohort = program_partition_hand_cohort(&plan);
    for (index, card) in hand_cohort.iter().enumerate() {
        if cx
            .engine
            .state
            .player_idx(card.owner)
            .is_some_and(|owner_index| {
                cx.engine.state.players[owner_index]
                    .declared_commander_object_ids
                    .contains(&card.object_id)
            })
        {
            plan.commander_choice_indices.push(index);
        }
    }
    plan.stage = plan
        .commander_choice_indices
        .first()
        .copied()
        .map_or(PendingProgramPartitionMoveStage::ReadyToCommit, |index| {
            PendingProgramPartitionMoveStage::ChoosingCommander { index }
        });
    match advance_program_partition_move(cx.engine, cx.parked_stack(), plan, cx.events)? {
        ProgramPartitionMoveProgress::Committed => Ok(EffectOutcome::Continue),
        ProgramPartitionMoveProgress::Parked => Ok(EffectOutcome::Suspended),
    }
}

/// The exact retained cohort routed to the graveyard by a typed split. Ordering is the order of
/// the published partition side, and is independent of which side has that destination.
pub(in crate::engine) fn program_partition_graveyard_cohort(
    plan: &ProgramPartitionMovePlan,
) -> Vec<ProgramCardRef> {
    if plan.selected_to == SearchDestination::Graveyard {
        plan.partition.selected.clone()
    } else {
        plan.partition.remainder.clone()
    }
}

pub(in crate::engine) fn program_partition_hand_cohort(
    plan: &ProgramPartitionMovePlan,
) -> Vec<ProgramCardRef> {
    if plan.selected_to == SearchDestination::Hand {
        plan.partition.selected.clone()
    } else {
        plan.partition.remainder.clone()
    }
}

pub(super) fn shuffle_library(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::ShuffleLibrary { who } = effect else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    let Some(player) = super::player_recipients(cx, who).into_iter().next() else {
        return Ok(EffectOutcome::Continue);
    };
    let Some(player_index) = cx.engine.state.player_idx(player) else {
        return Ok(EffectOutcome::Continue);
    };
    if cx.engine.state.players[player_index].has_lost {
        return Ok(EffectOutcome::Continue);
    }
    crate::engine::shuffle_player_library_for_current_command(&mut cx.engine.state, player);
    cx.events
        .push(ev_log(format!("P{player} shuffles their library.")));
    Ok(EffectOutcome::Continue)
}

pub(in crate::engine) fn program_library_ref_is_current(
    engine: &GameEngine,
    card: &ProgramCardRef,
) -> bool {
    card.zone == Zone::Library
        && engine
            .state
            .zone_change_generation
            .get(&card.object_id)
            .copied()
            .unwrap_or(0)
            == card.zone_change_generation
        && engine
            .state
            .objects
            .get(&card.object_id)
            .is_some_and(|object| object.zone == Zone::Library && object.owner == card.owner)
        && engine.state.player_idx(card.owner).is_some_and(|index| {
            !engine.state.players[index].has_lost
                && engine.state.players[index]
                    .library
                    .contains(&card.object_id)
        })
}

pub(in crate::engine) enum ProgramPartitionMoveProgress {
    Committed,
    Parked,
}

pub(in crate::engine) fn advance_program_partition_move(
    engine: &mut GameEngine,
    stack: ParkedStackResolution,
    mut plan: ProgramPartitionMovePlan,
    events: &mut Vec<rv1::RuledEvent>,
) -> Result<ProgramPartitionMoveProgress, EngineError> {
    while let Some(&selected_index) = plan
        .commander_choice_indices
        .get(plan.commander_choice_cursor)
    {
        let hand_cohort = program_partition_hand_cohort(&plan);
        let Some(card) = hand_cohort.get(selected_index).copied() else {
            return Err(EngineError::ProgramInvariant(
                "commander replacement index is outside its hand partition",
            ));
        };
        if !program_library_ref_is_current(engine, &card) {
            return Err(EngineError::ProgramInvariant(
                "commander replacement candidate is no longer in its library",
            ));
        }
        let owner_index =
            engine
                .state
                .player_idx(card.owner)
                .ok_or(EngineError::ProgramInvariant(
                    "commander replacement owner disappeared",
                ))?;
        if !engine.state.players[owner_index]
            .declared_commander_object_ids
            .contains(&card.object_id)
        {
            plan.commander_choice_cursor += 1;
            continue;
        }
        plan.stage = PendingProgramPartitionMoveStage::ChoosingCommander {
            index: selected_index,
        };
        let card_name = object_display_name(&engine.state, engine.registry, card.object_id);
        let prompt = format!(
            "P{}: put {card_name} into the command zone instead of their hand (CR 903.9b)?",
            card.owner
        );
        let branches = ["Command zone", "Hand"]
            .into_iter()
            .enumerate()
            .map(|(branch_index, label)| rv1::ResolutionBranchOption {
                branch_index: branch_index as u32,
                label: label.into(),
                selectable: true,
                ..Default::default()
            })
            .collect();
        events.push(rv1::RuledEvent {
            ev: Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(
                rv1::ResolutionChoiceRequired {
                    resolution_prompt_id: 0,
                    deciding_player_id: card.owner,
                    source_object_id: stack.item.id,
                    prompt_text: prompt.clone(),
                    choice_kind: custom::ChoiceKind::ResolutionBranch as i32,
                    min: 1,
                    max: 1,
                    resolution_branches: branches,
                    public_reveal: plan.partition.public_reveal.clone(),
                    ..Default::default()
                },
            )),
        });
        events.push(ev_log(prompt.clone()));
        engine.state.pending_resolution = Some(PendingResolution {
            resolution_prompt_id: crate::state::ResolutionPromptId::UNASSIGNED,
            resolution_prompt_semantics: None,
            deciding_player: card.owner,
            presentation: PendingResolutionPresentation {
                source_object_id: stack.item.id,
                candidates: Vec::new(),
                min: 1,
                max: 1,
                ordered: false,
                unique_names: false,
                prompt,
                choice_kind: custom::ChoiceKind::ResolutionBranch,
            },
            continuation: ResolutionContinuation::ProgramPartitionMove { stack, plan },
        });
        return Ok(ProgramPartitionMoveProgress::Parked);
    }

    let graveyard = if plan.graveyard_order.is_empty() {
        program_partition_graveyard_cohort(&plan)
    } else {
        plan.graveyard_order.clone()
    };
    if graveyard.len() > 1
        && !matches!(
            plan.stage,
            PendingProgramPartitionMoveStage::ChoosingGraveyardOrder
        )
    {
        let owner = graveyard[0].owner;
        if graveyard.iter().any(|card| card.owner != owner) {
            return Err(EngineError::ProgramInvariant(
                "one typed partition spans multiple graveyard owners",
            ));
        }
        let candidate_ids = graveyard
            .iter()
            .map(|card| card.object_id)
            .collect::<Vec<_>>();
        let (candidate_card_ids, candidate_names) =
            super::candidate_identities(engine, &candidate_ids);
        let prompt = format!(
            "P{owner}: choose the order for the cards put into your graveyard. The last card you click will be on top."
        );
        plan.stage = PendingProgramPartitionMoveStage::ChoosingGraveyardOrder;
        events.push(rv1::RuledEvent {
            ev: Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(
                rv1::ResolutionChoiceRequired {
                    resolution_prompt_id: 0,
                    deciding_player_id: owner,
                    source_object_id: stack.item.id,
                    prompt_text: prompt.clone(),
                    choice_kind: custom::ChoiceKind::SimultaneousEntryOrder as i32,
                    candidate_object_ids: candidate_ids.clone(),
                    candidate_card_ids,
                    candidate_names,
                    min: candidate_ids.len() as u32,
                    max: candidate_ids.len() as u32,
                    ordered: true,
                    public_reveal: plan.partition.public_reveal.clone(),
                    ..Default::default()
                },
            )),
        });
        events.push(ev_log(prompt.clone()));
        engine.state.pending_resolution = Some(PendingResolution {
            resolution_prompt_id: crate::state::ResolutionPromptId::UNASSIGNED,
            resolution_prompt_semantics: None,
            deciding_player: owner,
            presentation: PendingResolutionPresentation {
                source_object_id: stack.item.id,
                candidates: candidate_ids,
                min: graveyard.len() as u32,
                max: graveyard.len() as u32,
                ordered: true,
                unique_names: false,
                prompt,
                choice_kind: custom::ChoiceKind::SimultaneousEntryOrder,
            },
            continuation: ResolutionContinuation::ProgramPartitionMove { stack, plan },
        });
        return Ok(ProgramPartitionMoveProgress::Parked);
    }

    commit_program_partition_move(engine, &stack, &plan, &graveyard, events)?;
    Ok(ProgramPartitionMoveProgress::Committed)
}

fn commit_program_partition_move(
    engine: &mut GameEngine,
    stack: &ParkedStackResolution,
    plan: &ProgramPartitionMovePlan,
    graveyard_order: &[ProgramCardRef],
    events: &mut Vec<rv1::RuledEvent>,
) -> Result<(), EngineError> {
    let all = plan
        .partition
        .selected
        .iter()
        .chain(&plan.partition.remainder)
        .copied()
        .collect::<Vec<_>>();
    let mut seen = HashSet::new();
    if all
        .iter()
        .any(|card| !seen.insert(card.object_id) || !program_library_ref_is_current(engine, card))
    {
        return Err(EngineError::ProgramInvariant(
            "program partition changed before its single move commit",
        ));
    }
    let hand_cohort = program_partition_hand_cohort(plan);
    let command_zone = plan
        .commander_replacements
        .iter()
        .filter_map(|(index, destination)| (*destination == Zone::Command).then_some(*index))
        .collect::<HashSet<_>>();
    let spell_label = engine
        .registry
        .get(&stack.item.card_id)
        .and_then(|definition| definition.face(stack.item.face_index))
        .map(|face| face.name.to_string())
        .unwrap_or_else(|| stack.item.card_id.clone());
    let graveyard_cohort = program_partition_graveyard_cohort(plan);
    let mut unordered_graveyard = graveyard_cohort.clone();
    if graveyard_order.len() != unordered_graveyard.len() {
        return Err(EngineError::ProgramInvariant(
            "graveyard order does not contain the complete graveyard partition",
        ));
    }
    for card in graveyard_order {
        let Some(index) = unordered_graveyard
            .iter()
            .position(|candidate| candidate == card)
        else {
            return Err(EngineError::ProgramInvariant(
                "graveyard order is not an exact permutation of the graveyard partition",
            ));
        };
        unordered_graveyard.remove(index);
    }

    // Build the entire commit plan before any object changes zones. The target's chosen
    // partition, CR 903.9b hand replacements, and CR 404.3 graveyard order are all settled now.
    let mut planned = Vec::with_capacity(all.len());
    for (index, card) in hand_cohort.iter().copied().enumerate() {
        if command_zone.contains(&index) {
            planned.push((
                card,
                Zone::Command,
                object_display_name(&engine.state, engine.registry, card.object_id),
            ));
        }
    }
    for card in graveyard_order.iter().copied() {
        planned.push((
            card,
            Zone::Graveyard,
            object_display_name(&engine.state, engine.registry, card.object_id),
        ));
    }
    for (index, card) in hand_cohort.iter().copied().enumerate() {
        if command_zone.contains(&index) {
            continue;
        }
        planned.push((
            card,
            Zone::Hand,
            object_display_name(&engine.state, engine.registry, card.object_id),
        ));
    }
    if planned.is_empty() {
        return Ok(());
    }
    let move_inputs = planned
        .iter()
        .map(|(card, destination, _)| (card.object_id, *destination, None))
        .collect::<Vec<_>>();
    let receipts = engine.commit_observed_zone_move_batch(&move_inputs, events)?;
    if receipts.len() != planned.len()
        || planned.iter().any(|(card, destination, _)| {
            !receipts.iter().any(|receipt| {
                receipt.object_id == card.object_id
                    && receipt.owner == card.owner
                    && receipt.origin == Zone::Library
                    && receipt.before_zone_change_generation == card.zone_change_generation
                    && receipt.destination == *destination
                    && receipt.destination_generation == card.zone_change_generation + 1
            })
        })
    {
        return Err(EngineError::ProgramInvariant(
            "program partition commit did not produce every exact generation-bound move receipt",
        ));
    }
    for (card, destination, label) in planned {
        let proto_destination = match destination {
            Zone::Command => rv1::permanent_moved::Destination::Command,
            Zone::Graveyard => rv1::permanent_moved::Destination::Graveyard,
            Zone::Hand => rv1::permanent_moved::Destination::Hand,
            _ => {
                return Err(EngineError::ProgramInvariant(
                    "program partition planned an unsupported destination",
                ));
            }
        };
        events.push(permanent_moved_event(
            &engine.state,
            card.object_id,
            card.owner,
            proto_destination,
        ));
        let message = match destination {
            Zone::Command => format!(
                "P{} puts {label} into the command zone instead of their hand.",
                card.owner
            ),
            Zone::Graveyard => format!(
                "{spell_label} puts {label} into P{}'s graveyard.",
                card.owner
            ),
            Zone::Hand => format!("{spell_label} puts {label} into P{}'s hand.", card.owner),
            _ => unreachable!(),
        };
        events.push(ev_log(message));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{ProgramArm, ProgramFrame, ProgramPathStep};
    use tricerules_card_model::primitives::ResultSlotId;
    use tricerules_card_model::{CardFaceId, ProgramOwnerId};

    fn fixture_engine() -> GameEngine {
        GameEngine::new(
            tricerules_cards::registry::global(),
            91_701,
            &[0, 1],
            20,
            Some(vec![
                vec!["forest".to_string(); 20],
                vec!["island".to_string(); 20],
            ]),
            true,
        )
        .expect("new game")
    }

    fn fixture_stack_item() -> StackItem {
        StackItem {
            prototyped: false,
            mana_colors_spent_to_cast: Default::default(),
            id: 91_702,
            controller: 0,
            card_id: "divination".into(),
            targets: Vec::new(),
            ability_text: None,
            source_permanent_id: None,
            source_owner: Some(0),
            source_zone_change: 0,
            trigger_use_key: None,
            source_face_change: 0,
            ability_index: None,
            activated_ability: None,
            activated_ability_occurrence: None,
            triggered_ability: None,
            is_triggered: false,
            is_copy: false,
            face_index: 0,
            cast_method: SpellCastMethod::Normal,
            returned_attacker_assignment: None,
            chosen_x: 0,
            chosen_modes: Vec::new(),
            cast_cost_receipts: Vec::new(),
            cast_condition_results: Vec::new(),
            cast_occurrence: None,
            cast_by: Some(0),
            payment_result: Default::default(),
            exiled_cohorts: Default::default(),
            chaos_warp_owner_instructions: Default::default(),
            resolution_branch_choices: Default::default(),
            blight_receipts: Vec::new(),
            trigger_context: Default::default(),
        }
    }

    fn first_library_refs(
        engine: &GameEngine,
        owner: PlayerId,
        count: usize,
    ) -> Vec<ProgramCardRef> {
        let index = engine.state.player_idx(owner).expect("fixture player");
        engine.state.players[index]
            .library
            .iter()
            .take(count)
            .map(|object_id| ProgramCardRef {
                object_id: *object_id,
                zone_change_generation: engine
                    .state
                    .zone_change_generation
                    .get(object_id)
                    .copied()
                    .unwrap_or(0),
                zone: Zone::Library,
                owner,
            })
            .collect()
    }

    #[test]
    fn reveal_cohort_prunes_stale_search_refs_without_rebinding_generation() {
        let mut engine = fixture_engine();
        let found = first_library_refs(&engine, 0, 2);
        let stale = found[0];
        engine
            .commit_observed_zone_move(stale.object_id, Zone::Hand, None, &mut Vec::new())
            .expect("a prior legal instruction moves one search result");

        let previous = EffectResult {
            program_value: Some(ProgramValue::FoundLibraryCohort(found.clone())),
            ..Default::default()
        };
        let mut result = EffectResult::default();
        let mut events = Vec::new();
        let stack = fixture_stack_item();
        let outcome = reveal_cohort(&mut EffectCx {
            engine: &mut engine,
            events: &mut events,
            targets: &[],
            targets_by_role: &[],
            target_damage: &[],
            target_group_indices: &[],
            top: &stack,
            controller: 0,
            affected_player: 0,
            spell_label: "Fixture",
            previous_effect_result: &previous,
            effect_result: &mut result,
            effect_index: 3,
            resume_previous_result: None,
            program_state: None,
        })
        .expect("reveal the exact live projection of the earlier search result");

        assert_eq!(outcome, EffectOutcome::Continue);
        assert_eq!(
            found[0], stale,
            "the immutable search result remains unchanged"
        );
        let Some(ProgramValue::RevealedLibraryCohort(revealed)) = result.program_value else {
            panic!("reveal publishes a typed cohort");
        };
        assert_eq!(revealed.cards, vec![found[1]]);
        let receipt = revealed
            .public_reveal
            .expect("live cohort has a reveal receipt");
        assert_eq!(
            receipt
                .cards
                .iter()
                .map(|card| card.object_id)
                .collect::<Vec<_>>(),
            vec![found[1].object_id],
            "a moved search result is omitted instead of being rebound at a new generation"
        );
        assert_eq!(
            receipt.cards[0].zone_change_generation,
            found[1].zone_change_generation
        );
        assert!(!receipt.reveal_id.is_empty());
        assert!(events.iter().any(|event| matches!(
            &event.ev,
            Some(rv1::ruled_event::Ev::CardsRevealed(snapshot)) if snapshot == &receipt
        )));
        assert!(matches!(
            previous.program_value.as_ref(),
            Some(ProgramValue::FoundLibraryCohort(original)) if original == &found
        ));
    }

    #[test]
    fn retained_library_search_parking_preserves_the_exact_program_frame_and_cursor() {
        let mut engine = fixture_engine();
        let item = fixture_stack_item();
        let owner = ProgramOwnerId::face(
            "gifts_program_fixture",
            CardFaceId::new("gifts_program_fixture").expect("valid fixture face id"),
        );
        let program = ProgramRunState {
            owner: owner.clone(),
            pending_owners: Vec::new(),
            frames: vec![ProgramFrame {
                path: vec![ProgramPathStep {
                    instruction_index: 2,
                    arm: ProgramArm::Yes,
                }],
                next_instruction: 3,
                yield_to: Vec::new(),
            }],
            slots: vec![Some(ProgramValue::FoundLibraryCohort(Vec::new()))],
            in_flight: Some(vec![ResultSlotId(0)]),
            in_flight_produced_object_provenance: None,
        };
        let mut stack = ParkedStackResolution::new(item.clone());
        stack.program = Some(program);
        let mut events = Vec::new();
        zones::park_zone_search_choice(
            &mut engine,
            &mut events,
            &item,
            0,
            zones::ZoneSearchRequest {
                count: 4,
                selection_bounds: Some(tricerules_card_model::primitives::SearchSelectionBounds {
                    min: 0,
                    max: 4,
                }),
                filter: None,
                selection_constraint: Some(
                    tricerules_card_model::primitives::SearchSelectionConstraint::DifferentNames,
                ),
                slots: Vec::new(),
                zones: vec![CardSearchZone::Library],
                destination: SearchDestination::KeepInLibrary,
                conditional_destination: None,
                shuffle: false,
                reveal: false,
                stack,
            },
        )
        .expect("the retained library search parks");

        let saved = engine
            .state
            .pending_resolution
            .as_ref()
            .and_then(|pending| pending.continuation.stack())
            .and_then(|stack| stack.program.as_ref())
            .expect("the search continuation retains its program frame");
        assert_eq!(saved.owner, owner);
        assert_eq!(saved.frames[0].path[0].instruction_index, 2);
        assert_eq!(saved.frames[0].next_instruction, 3);
        assert!(matches!(
            saved.in_flight.as_deref(),
            Some([ResultSlotId(0)])
        ));
        assert!(matches!(
            saved.slots[0],
            Some(ProgramValue::FoundLibraryCohort(ref cohort)) if cohort.is_empty()
        ));
    }

    #[test]
    fn program_partition_collects_commander_choice_before_moving_any_card() {
        let mut engine = fixture_engine();
        let cards = first_library_refs(&engine, 1, 3);
        engine.state.players[1]
            .declared_commander_object_ids
            .push(cards[2].object_id);
        let plan = ProgramPartitionMovePlan {
            partition: ProgramCardPartition {
                selected: cards[..2].to_vec(),
                remainder: cards[2..].to_vec(),
                public_reveal: None,
            },
            selected_to: SearchDestination::Graveyard,
            remainder_to: SearchDestination::Hand,
            commander_replacements: Vec::new(),
            commander_choice_indices: vec![0],
            commander_choice_cursor: 0,
            graveyard_order: Vec::new(),
            stage: PendingProgramPartitionMoveStage::ReadyToCommit,
        };
        let mut events = Vec::new();

        let progress = advance_program_partition_move(
            &mut engine,
            ParkedStackResolution::new(fixture_stack_item()),
            plan,
            &mut events,
        )
        .expect("commander choice parks");

        assert!(matches!(progress, ProgramPartitionMoveProgress::Parked));
        assert!(matches!(
            engine
                .state
                .pending_resolution
                .as_ref()
                .map(|pending| &pending.continuation),
            Some(ResolutionContinuation::ProgramPartitionMove {
                plan: ProgramPartitionMovePlan {
                    stage: PendingProgramPartitionMoveStage::ChoosingCommander { index: 0 },
                    ..
                },
                ..
            })
        ));
        assert!(events.iter().any(|event| matches!(
            &event.ev,
            Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(choice))
                if choice.prompt_text.contains("903.9b")
                    && choice.deciding_player_id == 1
        )));
        for card in cards {
            assert!(program_library_ref_is_current(&engine, &card));
        }
    }

    #[test]
    fn program_partition_graveyard_order_prompt_explains_last_clicked_card_is_top() {
        let mut engine = fixture_engine();
        let cards = first_library_refs(&engine, 1, 3);
        let plan = ProgramPartitionMovePlan {
            partition: ProgramCardPartition {
                selected: cards[..2].to_vec(),
                remainder: cards[2..].to_vec(),
                public_reveal: None,
            },
            selected_to: SearchDestination::Graveyard,
            remainder_to: SearchDestination::Hand,
            commander_replacements: Vec::new(),
            commander_choice_indices: Vec::new(),
            commander_choice_cursor: 0,
            graveyard_order: Vec::new(),
            stage: PendingProgramPartitionMoveStage::ReadyToCommit,
        };
        let mut events = Vec::new();

        let progress = advance_program_partition_move(
            &mut engine,
            ParkedStackResolution::new(fixture_stack_item()),
            plan,
            &mut events,
        )
        .expect("the graveyard order choice parks");

        assert!(matches!(progress, ProgramPartitionMoveProgress::Parked));
        let choice = events
            .iter()
            .find_map(|event| match &event.ev {
                Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(choice)) => Some(choice),
                _ => None,
            })
            .expect("the graveyard order choice is presented");
        assert_eq!(
            choice.choice_kind,
            custom::ChoiceKind::SimultaneousEntryOrder as i32
        );
        assert_eq!(
            choice.prompt_text,
            "P1: choose the order for the cards put into your graveyard. The last card you click will be on top."
        );
        assert_eq!(
            choice.candidate_object_ids,
            vec![cards[0].object_id, cards[1].object_id]
        );
    }

    #[test]
    fn reverse_program_partition_routes_ordered_remainder_to_graveyard() {
        let mut engine = fixture_engine();
        let cards = first_library_refs(&engine, 1, 3);
        let plan = ProgramPartitionMovePlan {
            partition: ProgramCardPartition {
                selected: cards[..1].to_vec(),
                remainder: cards[1..].to_vec(),
                public_reveal: None,
            },
            selected_to: SearchDestination::Hand,
            remainder_to: SearchDestination::Graveyard,
            commander_replacements: Vec::new(),
            commander_choice_indices: Vec::new(),
            commander_choice_cursor: 0,
            graveyard_order: vec![cards[2], cards[1]],
            stage: PendingProgramPartitionMoveStage::ChoosingGraveyardOrder,
        };
        assert_eq!(program_partition_graveyard_cohort(&plan), cards[1..]);
        let mut events = Vec::new();

        let progress = advance_program_partition_move(
            &mut engine,
            ParkedStackResolution::new(fixture_stack_item()),
            plan,
            &mut events,
        )
        .expect("reverse partition commits after all choices");

        assert!(matches!(progress, ProgramPartitionMoveProgress::Committed));
        assert_eq!(engine.state.objects[&cards[0].object_id].zone, Zone::Hand);
        assert_eq!(
            engine.state.objects[&cards[1].object_id].zone,
            Zone::Graveyard
        );
        assert_eq!(
            engine.state.objects[&cards[2].object_id].zone,
            Zone::Graveyard
        );
        let owner_index = engine
            .state
            .player_idx(cards[0].owner)
            .expect("fixture owner");
        let graveyard = &engine.state.players[owner_index].graveyard;
        assert!(graveyard.ends_with(&[cards[2].object_id, cards[1].object_id]));
        assert_eq!(
            graveyard.last(),
            Some(&cards[1].object_id),
            "the last selected card is the top of the graveyard"
        );
        assert_eq!(
            events
                .iter()
                .filter(|event| matches!(
                    &event.ev,
                    Some(rv1::ruled_event::Ev::PermanentMoved(moved))
                        if cards.iter().any(|card| card.object_id == moved.object_id)
                ))
                .count(),
            3,
            "every cohort object has exactly one committed move event"
        );
    }

    #[test]
    fn stale_program_partition_aborts_before_moving_the_remaining_cards() {
        let mut engine = fixture_engine();
        let cards = first_library_refs(&engine, 1, 3);
        let stale = cards[0];
        let stale_index = engine.state.player_idx(1).expect("fixture player");
        let starting_hand = engine.state.players[stale_index].hand.clone();
        engine.state.players[stale_index]
            .library
            .retain(|object_id| *object_id != stale.object_id);
        engine.state.objects.get_mut(&stale.object_id).unwrap().zone = Zone::Exile;
        engine.state.players[stale_index]
            .exile
            .push(stale.object_id);
        engine
            .state
            .zone_change_generation
            .insert(stale.object_id, stale.zone_change_generation + 1);

        let plan = ProgramPartitionMovePlan {
            partition: ProgramCardPartition {
                selected: cards[..2].to_vec(),
                remainder: cards[2..].to_vec(),
                public_reveal: None,
            },
            selected_to: SearchDestination::Graveyard,
            remainder_to: SearchDestination::Hand,
            commander_replacements: Vec::new(),
            commander_choice_indices: Vec::new(),
            commander_choice_cursor: 0,
            graveyard_order: cards[..2].to_vec(),
            stage: PendingProgramPartitionMoveStage::ChoosingGraveyardOrder,
        };
        let mut events = Vec::new();

        let result = advance_program_partition_move(
            &mut engine,
            ParkedStackResolution::new(fixture_stack_item()),
            plan,
            &mut events,
        );

        assert!(result.is_err());
        for card in cards.iter().skip(1) {
            assert!(program_library_ref_is_current(&engine, card));
        }
        assert!(engine.state.players[stale_index].graveyard.is_empty());
        assert_eq!(engine.state.players[stale_index].hand, starting_hand);
    }
}
