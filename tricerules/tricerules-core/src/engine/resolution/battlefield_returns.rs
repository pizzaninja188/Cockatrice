//! CR 903.9b replacements for one frozen simultaneous battlefield-to-owner-zone cohort.

use super::*;
use crate::state::{
    BattlefieldCohortMember, PendingBattlefieldCohortMove, PendingBattlefieldCohortMoveStage,
};

enum CohortMoveProgress {
    Committed(Box<ParkedStackResolution>),
    Parked,
}

fn completion_result(stack: &ParkedStackResolution) -> EffectResult {
    if stack.program.is_some() {
        // Cohort moves are resultless Program instructions. The parked run-level context is
        // already stored on the stack and must remain separate from this operation result.
        EffectResult::default()
    } else {
        // Legacy effect lists publish the suspended operation's result on the parked stack when
        // the executor stamps the resume cursor. Preserve that generic result for the next effect.
        stack.previous_result.clone()
    }
}

fn owner_destination_label(destination: Zone) -> Result<&'static str, EngineError> {
    match destination {
        Zone::Hand => Ok("hand"),
        Zone::Library => Ok("library"),
        _ => Err(EngineError::ProgramInvariant(
            "battlefield cohort move has an unsupported owner destination",
        )),
    }
}

pub(super) fn return_to_owners_hand(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let subjects = match effect {
        SpellEffectKind::ReturnAllToOwnersHand { kind } => {
            battlefield_objects_matching(cx.engine, &kind)
        }
        SpellEffectKind::ReturnToOwnersHand { subject } => {
            if matches!(
                subject,
                EffectSubject::Chosen(_) | EffectSubject::PreviousEffectObject
            ) {
                cx.resolve_battlefield_subjects(&subject)
            } else {
                resolve_zone_effect_subject(cx.engine, cx.top, cx.targets, &subject)
                    .into_iter()
                    .collect()
            }
        }
        _ => return Err(EngineError::Illegal("resolution dispatch mismatch")),
    };
    begin_battlefield_cohort_move(cx, subjects, Zone::Hand, Vec::new())
}

/// Begin one simultaneous battlefield-to-owner-zone instruction. Library-owner receipts are
/// independent of the moving cohort so a stale source can still name its owner's library under
/// CR 701.24c; live cohort owners are added automatically for library destinations.
pub(super) fn begin_battlefield_cohort_move(
    cx: &mut EffectCx<'_>,
    object_ids: Vec<ObjectId>,
    destination: Zone,
    mut library_owners: Vec<PlayerId>,
) -> Result<EffectOutcome, EngineError> {
    if !matches!(destination, Zone::Hand | Zone::Library) {
        return Err(EngineError::ProgramInvariant(
            "battlefield cohort move supports only Hand or Library destinations",
        ));
    }
    if destination != Zone::Library && !library_owners.is_empty() {
        return Err(EngineError::ProgramInvariant(
            "only a library cohort move may retain shuffle-owner receipts",
        ));
    }
    if destination == Zone::Library {
        let mut seen_owners = HashSet::new();
        library_owners.retain(|owner| seen_owners.insert(*owner));
        for &owner in &library_owners {
            if cx.engine.state.player_idx(owner).is_none() {
                return Err(EngineError::ProgramInvariant(
                    "library shuffle owner receipt has no player",
                ));
            }
        }
    }

    let mut seen = HashSet::new();
    let cohort = object_ids
        .into_iter()
        .map(|object_id| {
            if !seen.insert(object_id) {
                return Err(EngineError::ProgramInvariant(
                    "battlefield cohort move contains a duplicate physical object",
                ));
            }
            let object = cx
                .engine
                .state
                .objects
                .get(&object_id)
                .ok_or(EngineError::Illegal("no battlefield cohort object"))?;
            if destination == Zone::Library && !library_owners.contains(&object.owner) {
                library_owners.push(object.owner);
            }
            Ok(BattlefieldCohortMember {
                object_id,
                zone_change_generation: cx
                    .engine
                    .state
                    .zone_change_generation
                    .get(&object_id)
                    .copied()
                    .unwrap_or(0),
                owner: object.owner,
                origin: object.zone,
            })
        })
        .collect::<Result<Vec<_>, EngineError>>()?;

    if cohort.is_empty() && library_owners.is_empty() {
        return Ok(EffectOutcome::Continue);
    }

    let mut groups = BTreeMap::<usize, (PlayerId, Vec<ObjectId>)>::new();
    for card in &cohort {
        let Some(owner_index) = cx.engine.state.player_idx(card.owner) else {
            return Err(EngineError::ProgramInvariant(
                "battlefield cohort move has no live owner",
            ));
        };
        let player = &cx.engine.state.players[owner_index];
        if player.has_lost
            || !player
                .declared_commander_object_ids
                .contains(&card.object_id)
        {
            continue;
        }
        groups
            .entry(cx.engine.state.apnap_rank(card.owner))
            .or_insert_with(|| (card.owner, Vec::new()))
            .1
            .push(card.object_id);
    }
    let commander_groups = groups.into_values().collect::<Vec<_>>();
    let mut plan = PendingBattlefieldCohortMove {
        stack: cx.parked_stack(),
        spell_label: cx.spell_label.to_owned(),
        destination,
        library_owners,
        cohort,
        commander_groups,
        next_commander_group: 0,
        ordered_commanders: Vec::new(),
        next_commander_destination: 0,
        commander_destinations: Vec::new(),
        removed_cohort_ids: Vec::new(),
        stage: None,
    };

    match advance_cohort_move(cx.engine, &mut plan, cx.events)? {
        CohortMoveProgress::Committed(_) => Ok(EffectOutcome::Continue),
        CohortMoveProgress::Parked => Ok(EffectOutcome::Suspended),
    }
}

fn ref_is_current(engine: &GameEngine, card: &BattlefieldCohortMember) -> bool {
    engine
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
            .is_some_and(|object| object.owner == card.owner && object.zone == card.origin)
}

fn cohort_is_current(engine: &GameEngine, cohort: &[BattlefieldCohortMember]) -> bool {
    let mut seen = HashSet::new();
    cohort
        .iter()
        .all(|card| seen.insert(card.object_id) && ref_is_current(engine, card))
}

fn publish_order_choice(
    engine: &mut GameEngine,
    mut plan: PendingBattlefieldCohortMove,
    owner: PlayerId,
    candidates: Vec<ObjectId>,
    events: &mut Vec<rv1::RuledEvent>,
) {
    let destination = owner_destination_label(plan.destination)
        .expect("validated battlefield cohort move destination");
    let prompt = format!(
        "P{owner}: order your commanders before their simultaneous move to their owners' {destination}."
    );
    plan.stage = Some(PendingBattlefieldCohortMoveStage::OrderingCommanders {
        owner,
        candidates: candidates.clone(),
    });
    let (candidate_card_ids, candidate_names) = super::candidate_identities(engine, &candidates);
    let count = candidates.len() as u32;
    events.push(rv1::RuledEvent {
        ev: Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(
            rv1::ResolutionChoiceRequired {
                resolution_prompt_id: 0,
                deciding_player_id: owner,
                source_object_id: plan.stack.item.id,
                prompt_text: prompt.clone(),
                choice_kind: rv1::ChoiceKind::SimultaneousEntryOrder as i32,
                candidate_object_ids: candidates,
                candidate_card_ids,
                candidate_names,
                min: count,
                max: count,
                ordered: true,
                ..Default::default()
            },
        )),
    });
    events.push(super::super::events::ev_log_private(prompt.clone(), owner));
    engine.state.pending_resolution = Some(PendingResolution {
        resolution_prompt_id: crate::state::ResolutionPromptId::UNASSIGNED,
        resolution_prompt_semantics: None,
        deciding_player: owner,
        presentation: PendingResolutionPresentation {
            source_object_id: plan.stack.item.id,
            candidates: match &plan.stage {
                Some(PendingBattlefieldCohortMoveStage::OrderingCommanders {
                    candidates, ..
                }) => candidates.clone(),
                _ => unreachable!(),
            },
            min: count,
            max: count,
            ordered: true,
            unique_names: false,
            prompt,
            choice_kind: rv1::ChoiceKind::SimultaneousEntryOrder,
        },
        continuation: ResolutionContinuation::BattlefieldCohortMove {
            pending: Box::new(plan),
        },
    });
}

fn publish_destination_choice(
    engine: &mut GameEngine,
    mut plan: PendingBattlefieldCohortMove,
    owner: PlayerId,
    object_id: ObjectId,
    events: &mut Vec<rv1::RuledEvent>,
) {
    let label = object_display_name(&engine.state, engine.registry, object_id);
    let destination = owner_destination_label(plan.destination)
        .expect("validated battlefield cohort move destination");
    let prompt = format!(
        "P{owner}: put {label} into the command zone instead of their {destination} (CR 903.9b)?"
    );
    plan.stage =
        Some(PendingBattlefieldCohortMoveStage::ChoosingCommanderDestination { owner, object_id });
    let branches = [
        "Command zone",
        if plan.destination == Zone::Library {
            "Library"
        } else {
            "Hand"
        },
    ]
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
                deciding_player_id: owner,
                source_object_id: plan.stack.item.id,
                prompt_text: prompt.clone(),
                choice_kind: rv1::ChoiceKind::ResolutionBranch as i32,
                resolution_branches: branches,
                min: 1,
                max: 1,
                ..Default::default()
            },
        )),
    });
    events.push(super::super::events::ev_log(prompt.clone()));
    engine.state.pending_resolution = Some(PendingResolution {
        resolution_prompt_id: crate::state::ResolutionPromptId::UNASSIGNED,
        resolution_prompt_semantics: None,
        deciding_player: owner,
        presentation: PendingResolutionPresentation {
            source_object_id: plan.stack.item.id,
            candidates: Vec::new(),
            min: 1,
            max: 1,
            ordered: false,
            unique_names: false,
            prompt,
            choice_kind: rv1::ChoiceKind::ResolutionBranch,
        },
        continuation: ResolutionContinuation::BattlefieldCohortMove {
            pending: Box::new(plan),
        },
    });
}

fn advance_cohort_move(
    engine: &mut GameEngine,
    plan: &mut PendingBattlefieldCohortMove,
    events: &mut Vec<rv1::RuledEvent>,
) -> Result<CohortMoveProgress, EngineError> {
    if !cohort_is_current(engine, &plan.cohort) {
        return Err(EngineError::ProgramInvariant(
            "battlefield cohort move changed before its choice or move batch",
        ));
    }
    plan.stage = None;
    loop {
        while plan
            .ordered_commanders
            .get(plan.next_commander_destination)
            .is_some_and(|(_, object_id)| plan.removed_cohort_ids.contains(object_id))
        {
            plan.next_commander_destination += 1;
        }

        let Some((owner, candidates)) = plan.commander_groups.get(plan.next_commander_group) else {
            break;
        };
        if candidates.is_empty() {
            plan.next_commander_group += 1;
            continue;
        }

        let ordered_count = plan
            .ordered_commanders
            .iter()
            .filter(|(ordered_owner, _)| ordered_owner == owner)
            .count();
        if ordered_count == 0 {
            if candidates.len() == 1 {
                plan.ordered_commanders.push((*owner, candidates[0]));
            } else {
                publish_order_choice(engine, plan.clone(), *owner, candidates.clone(), events);
                return Ok(CohortMoveProgress::Parked);
            }
        } else if ordered_count != candidates.len() {
            return Err(EngineError::ProgramInvariant(
                "commander return order does not cover the active owner's complete group",
            ));
        }

        if let Some((ordered_owner, object_id)) = plan
            .ordered_commanders
            .get(plan.next_commander_destination)
            .copied()
        {
            if ordered_owner == *owner {
                let Some(card) = plan.cohort.iter().find(|card| card.object_id == object_id) else {
                    if plan.removed_cohort_ids.contains(&object_id) {
                        plan.next_commander_destination += 1;
                        continue;
                    }
                    return Err(EngineError::ProgramInvariant(
                        "commander return choice lost its exact cohort member",
                    ));
                };
                if card.owner != *owner {
                    return Err(EngineError::ProgramInvariant(
                        "commander return order changed its card owner",
                    ));
                }
                let owner_is_live = engine
                    .state
                    .player_idx(*owner)
                    .is_some_and(|index| !engine.state.players[index].has_lost);
                let declared = engine.state.player_idx(*owner).is_some_and(|index| {
                    engine.state.players[index]
                        .declared_commander_object_ids
                        .contains(&object_id)
                });
                if !owner_is_live || !declared {
                    return Err(EngineError::ProgramInvariant(
                        "live owner lost a declared commander during return replacement",
                    ));
                }
                publish_destination_choice(engine, plan.clone(), *owner, object_id, events);
                return Ok(CohortMoveProgress::Parked);
            }
        }

        // Finish this owner's order and all of their individual CR 903.9b choices before
        // showing any choice to the next APNAP owner.
        plan.next_commander_group += 1;
    }

    if let Some((_, object_id)) = plan
        .ordered_commanders
        .get(plan.next_commander_destination)
        .copied()
    {
        if !plan.removed_cohort_ids.contains(&object_id) {
            return Err(EngineError::ProgramInvariant(
                "battlefield cohort move has an unprocessed owner group",
            ));
        }
    }
    let command_destinations = plan
        .commander_destinations
        .iter()
        .filter(|(object_id, _)| !plan.removed_cohort_ids.contains(object_id))
        .map(|(object_id, destination)| (*object_id, *destination))
        .collect::<HashMap<_, _>>();
    let planned = plan
        .cohort
        .iter()
        .map(|card| {
            let destination = command_destinations
                .get(&card.object_id)
                .copied()
                .unwrap_or(plan.destination);
            let label = object_display_name(&engine.state, engine.registry, card.object_id);
            (card, destination, label)
        })
        .collect::<Vec<_>>();
    let moves = planned
        .iter()
        .map(|(card, destination, _)| (card.object_id, *destination, None))
        .collect::<Vec<_>>();
    if !moves.is_empty() {
        let receipts = engine.commit_observed_zone_move_batch(&moves, events)?;
        if receipts.len() != planned.len()
            || planned.iter().any(|(card, destination, _)| {
                !receipts.iter().any(|receipt| {
                    receipt.object_id == card.object_id
                        && receipt.owner == card.owner
                        && receipt.origin == card.origin
                        && receipt.before_zone_change_generation == card.zone_change_generation
                        && receipt.destination == *destination
                        && receipt.destination_generation == card.zone_change_generation + 1
                })
            })
        {
            return Err(EngineError::ProgramInvariant(
                "battlefield cohort move did not produce every exact incarnation receipt",
            ));
        }
        for (card, destination, label) in planned {
            let (proto_destination, message) = match destination {
                Zone::Command => (
                    rv1::permanent_moved::Destination::Command,
                    format!(
                        "P{} puts {label} into the command zone instead of their {}.",
                        card.owner,
                        owner_destination_label(plan.destination)?
                    ),
                ),
                Zone::Hand => (
                    rv1::permanent_moved::Destination::Hand,
                    format!("{} returns {label} to its owner's hand", plan.spell_label),
                ),
                Zone::Library => (
                    rv1::permanent_moved::Destination::Library,
                    format!(
                        "{} puts {label} into its owner's library.",
                        plan.spell_label
                    ),
                ),
                _ => {
                    return Err(EngineError::ProgramInvariant(
                        "battlefield cohort move planned an unsupported destination",
                    ));
                }
            };
            events.push(permanent_moved_event(
                &engine.state,
                card.object_id,
                card.owner,
                proto_destination,
            ));
            events.push(super::super::events::ev_log(message));
        }
    }
    if plan.destination == Zone::Library {
        let shuffle_owners = engine
            .state
            .players
            .iter()
            .filter(|player| !player.has_lost && plan.library_owners.contains(&player.id))
            .map(|player| player.id)
            .collect::<Vec<_>>();
        for owner in shuffle_owners {
            crate::engine::shuffle_player_library_for_current_command(&mut engine.state, owner);
            events.push(super::super::events::ev_log(format!(
                "P{owner} shuffles their library."
            )));
        }
    }
    Ok(CohortMoveProgress::Committed(Box::new(plan.stack.clone())))
}

impl GameEngine {
    pub(in crate::engine) fn finish_battlefield_cohort_move_order(
        &mut self,
        pending: PendingResolution,
        chosen: &[ObjectId],
    ) -> Result<RuledEventBatch, EngineError> {
        let backup = pending.clone();
        let mut plan = match &pending.continuation {
            ResolutionContinuation::BattlefieldCohortMove { pending } => pending.as_ref().clone(),
            _ => unreachable!("battlefield cohort move order continuation"),
        };
        let Some(PendingBattlefieldCohortMoveStage::OrderingCommanders { owner, candidates }) =
            plan.stage.clone()
        else {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal(
                "battlefield cohort move is not waiting for a commander order",
            ));
        };
        if owner != pending.deciding_player
            || candidates != pending.presentation.candidates
            || chosen.len() != candidates.len()
            || chosen
                .iter()
                .any(|object_id| !candidates.contains(object_id))
            || !cohort_is_current(self, &plan.cohort)
        {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal(
                "battlefield cohort move commander order is stale or invalid",
            ));
        }
        let group = plan
            .commander_groups
            .get(plan.next_commander_group)
            .filter(|(group_owner, ids)| *group_owner == owner && ids == &candidates);
        if group.is_none() {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::ProgramInvariant(
                "battlefield cohort move order does not match its APNAP owner group",
            ));
        }
        plan.ordered_commanders
            .extend(chosen.iter().copied().map(|object_id| (owner, object_id)));
        plan.stage = None;
        let ordered_names = chosen
            .iter()
            .map(|object_id| object_display_name(&self.state, self.registry, *object_id))
            .collect::<Vec<_>>();
        let mut events = vec![super::super::events::ev_log(format!(
            "P{owner} orders their simultaneous commander returns: {}.",
            ordered_names.join(", ")
        ))];
        match advance_cohort_move(self, &mut plan, &mut events) {
            Ok(CohortMoveProgress::Parked) => {
                Ok(super::super::events::finish_with_events(self, events))
            }
            Ok(CohortMoveProgress::Committed(stack)) => {
                let result = completion_result(&stack);
                self.complete_parked_stack_with_previous(*stack, result, events)
            }
            Err(error) => {
                self.state.pending_resolution = Some(backup);
                Err(error)
            }
        }
    }

    pub(in crate::engine) fn finish_battlefield_cohort_move_branch(
        &mut self,
        pending: PendingResolution,
        answer: &rv1::SubmitResolutionChoice,
        decision: rv1::ResolutionChoiceDecision,
    ) -> Result<RuledEventBatch, EngineError> {
        let backup = pending.clone();
        let mut plan = match &pending.continuation {
            ResolutionContinuation::BattlefieldCohortMove { pending } => pending.as_ref().clone(),
            _ => unreachable!("battlefield cohort move branch continuation"),
        };
        let Some(PendingBattlefieldCohortMoveStage::ChoosingCommanderDestination {
            owner,
            object_id,
        }) = plan.stage.clone()
        else {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal(
                "battlefield cohort move is not waiting for a Commander destination",
            ));
        };
        let clean = answer.chosen_object_ids.is_empty()
            && answer.chosen_player_ids.is_empty()
            && answer.cast_spell.is_none()
            && answer.chosen_combat_defender.is_none()
            && answer.payment.is_none()
            && answer.restricted_mana.is_empty()
            && answer.spell_cast_announcement.is_none();
        if !clean
            || decision != rv1::ResolutionChoiceDecision::SelectBranch
            || answer.selected_branch_index > 1
            || owner != pending.deciding_player
            || !cohort_is_current(self, &plan.cohort)
            || !plan
                .ordered_commanders
                .get(plan.next_commander_destination)
                .is_some_and(|(current_owner, current_object)| {
                    *current_owner == owner && *current_object == object_id
                })
        {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal(
                "battlefield cohort move destination answer is stale or invalid",
            ));
        }
        let destination = if answer.selected_branch_index == 0 {
            Zone::Command
        } else {
            plan.destination
        };
        plan.commander_destinations.push((object_id, destination));
        plan.next_commander_destination += 1;
        plan.stage = None;
        let mut events = vec![super::super::events::ev_log(format!(
            "P{owner} chooses {} for their commander.",
            if destination == Zone::Command {
                "the command zone"
            } else if plan.destination == Zone::Library {
                "their library"
            } else {
                "their hand"
            }
        ))];
        match advance_cohort_move(self, &mut plan, &mut events) {
            Ok(CohortMoveProgress::Parked) => {
                Ok(super::super::events::finish_with_events(self, events))
            }
            Ok(CohortMoveProgress::Committed(stack)) => {
                let result = completion_result(&stack);
                self.complete_parked_stack_with_previous(*stack, result, events)
            }
            Err(error) => {
                self.state.pending_resolution = Some(backup);
                Err(error)
            }
        }
    }

    /// Remove exact objects that leave under either CR 800.4a cleanup category. A borrowed
    /// permanent whose control reverted remains the same generation and stays in the cohort.
    pub(in crate::engine) fn refresh_battlefield_cohort_move_departure(
        &mut self,
        departed_objects: &HashSet<ObjectId>,
        stranded_objects: &HashSet<ObjectId>,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<(), EngineError> {
        let Some(pending) = self.state.pending_resolution.take() else {
            return Ok(());
        };
        let ResolutionContinuation::BattlefieldCohortMove { pending: boxed } =
            &pending.continuation
        else {
            self.state.pending_resolution = Some(pending);
            return Ok(());
        };
        let mut plan = boxed.as_ref().clone();
        let old_stage = plan.stage.clone().ok_or(EngineError::ProgramInvariant(
            "parked battlefield cohort move has no active choice stage",
        ))?;
        let mut removed_objects = departed_objects.clone();
        removed_objects.extend(stranded_objects.iter().copied());
        let before = plan.cohort.len();
        let removed_refs = plan
            .cohort
            .iter()
            .filter(|card| removed_objects.contains(&card.object_id))
            .map(|card| card.object_id)
            .collect::<Vec<_>>();
        plan.cohort
            .retain(|card| !removed_objects.contains(&card.object_id));
        for object_id in removed_refs {
            if !plan.removed_cohort_ids.contains(&object_id) {
                plan.removed_cohort_ids.push(object_id);
            }
        }
        if plan.cohort.len() != before {
            for (_, candidates) in &mut plan.commander_groups {
                candidates.retain(|object_id| !removed_objects.contains(object_id));
            }
        }
        if !cohort_is_current(self, &plan.cohort) {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::ProgramInvariant(
                "surviving battlefield cohort move changed during departure cleanup",
            ));
        }
        plan.stage = match old_stage {
            PendingBattlefieldCohortMoveStage::OrderingCommanders { owner, candidates } => {
                let updated = candidates
                    .into_iter()
                    .filter(|object_id| plan.cohort.iter().any(|card| card.object_id == *object_id))
                    .collect::<Vec<_>>();
                let group = plan
                    .commander_groups
                    .get(plan.next_commander_group)
                    .map(|(_, candidates)| candidates.clone())
                    .unwrap_or_default();
                // A singleton no longer needs an order prompt. Let advance_cohort_move
                // automatically order it and publish the owner's destination choice.
                if updated == group && updated.len() > 1 {
                    Some(PendingBattlefieldCohortMoveStage::OrderingCommanders {
                        owner,
                        candidates: updated,
                    })
                } else {
                    None
                }
            }
            PendingBattlefieldCohortMoveStage::ChoosingCommanderDestination {
                owner,
                object_id,
            } if plan.cohort.iter().any(|card| card.object_id == object_id) => Some(
                PendingBattlefieldCohortMoveStage::ChoosingCommanderDestination {
                    owner,
                    object_id,
                },
            ),
            PendingBattlefieldCohortMoveStage::ChoosingCommanderDestination { .. } => None,
        };
        let mut refresh_events = Vec::new();
        let progress = if plan.stage.is_some() {
            republish_current_choice(self, plan, &mut refresh_events)?;
            CohortMoveProgress::Parked
        } else {
            advance_cohort_move(self, &mut plan, &mut refresh_events)?
        };
        match progress {
            CohortMoveProgress::Parked => events.extend(refresh_events),
            CohortMoveProgress::Committed(stack) => {
                let result = completion_result(&stack);
                let completed =
                    self.complete_parked_stack_with_previous(*stack, result, refresh_events)?;
                events.extend(completed.events);
            }
        }
        Ok(())
    }
}

fn republish_current_choice(
    engine: &mut GameEngine,
    plan: PendingBattlefieldCohortMove,
    events: &mut Vec<rv1::RuledEvent>,
) -> Result<(), EngineError> {
    match plan.stage.clone() {
        Some(PendingBattlefieldCohortMoveStage::OrderingCommanders { owner, candidates }) => {
            if !cohort_is_current(engine, &plan.cohort)
                || candidates.len() < 2
                || candidates
                    .iter()
                    .any(|object_id| !plan.cohort.iter().any(|card| card.object_id == *object_id))
            {
                return Err(EngineError::ProgramInvariant(
                    "battlefield cohort move order became stale during departure refresh",
                ));
            }
            publish_order_choice(engine, plan, owner, candidates, events);
        }
        Some(PendingBattlefieldCohortMoveStage::ChoosingCommanderDestination {
            owner,
            object_id,
        }) => {
            if !cohort_is_current(engine, &plan.cohort)
                || !plan
                    .ordered_commanders
                    .get(plan.next_commander_destination)
                    .is_some_and(|(expected_owner, expected_id)| {
                        *expected_owner == owner && *expected_id == object_id
                    })
            {
                return Err(EngineError::ProgramInvariant(
                    "battlefield cohort move destination became stale during departure refresh",
                ));
            }
            publish_destination_choice(engine, plan, owner, object_id, events);
        }
        None => {
            return Err(EngineError::ProgramInvariant(
                "battlefield cohort move cannot republish an advancing plan",
            ));
        }
    }
    Ok(())
}
