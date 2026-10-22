//! Brainstorm's ordered hand-to-library Program transaction.
//!
//! The private selection stays generation-bound while each selected declared commander receives
//! its own CR 903.9b destination choice. The complete selected cohort commits only after the last
//! answer, so a parked choice never changes the private hand or library order.

use super::*;
use crate::state::{
    PendingProgramHandLibraryReturnStage, ProgramHandLibraryReturnPlan, ResolutionPromptId,
    ResolutionPromptSemanticSnapshot,
};
use std::collections::HashSet;

pub(super) fn begin_program_hand_library_return(
    cx: &mut EffectCx<'_>,
    selected: &[TriggerObjectRef],
) -> Result<EffectOutcome, EngineError> {
    if selected.is_empty() {
        return Ok(EffectOutcome::Continue);
    }

    let owner = cx.top.controller;
    let selected_objects = selected
        .iter()
        .map(|reference| reference.object_id)
        .collect::<Vec<_>>();
    if !selected_hand_is_current(cx.engine, owner, selected)
        || selected_objects.len()
            != selected_objects
                .iter()
                .copied()
                .collect::<HashSet<_>>()
                .len()
    {
        return Err(EngineError::ProgramInvariant(
            "Brainstorm hand selection changed before its library return",
        ));
    }

    let owner_index = cx
        .engine
        .state
        .player_idx(owner)
        .ok_or(EngineError::ProgramInvariant(
            "Brainstorm controller disappeared before its library return",
        ))?;
    let commander_choice_indices = selected
        .iter()
        .enumerate()
        .filter_map(|(index, reference)| {
            cx.engine.state.players[owner_index]
                .declared_commander_object_ids
                .contains(&reference.object_id)
                .then_some(index)
        })
        .collect::<Vec<_>>();
    let plan = ProgramHandLibraryReturnPlan {
        owner,
        selected: selected.to_vec(),
        commander_choice_indices,
        commander_choice_cursor: 0,
        commander_destinations: Vec::new(),
        stage: PendingProgramHandLibraryReturnStage::ReadyToCommit,
    };

    if plan.commander_choice_indices.is_empty() {
        commit_program_hand_library_return(cx.engine, &cx.parked_stack(), &plan, cx.events)?;
        return Ok(EffectOutcome::Continue);
    }

    publish_commander_choice(cx.engine, cx.parked_stack(), plan, cx.events)?;
    Ok(EffectOutcome::Suspended)
}

pub(in crate::engine) fn selected_hand_is_current(
    engine: &GameEngine,
    owner: PlayerId,
    selected: &[TriggerObjectRef],
) -> bool {
    let Some(owner_index) = engine.state.player_idx(owner) else {
        return false;
    };
    !engine.state.players[owner_index].has_lost
        && selected.iter().all(|reference| {
            engine
                .state
                .zone_change_generation
                .get(&reference.object_id)
                .copied()
                .unwrap_or(0)
                == reference.zone_change_generation
                && engine
                    .state
                    .objects
                    .get(&reference.object_id)
                    .is_some_and(|object| object.owner == owner && object.zone == Zone::Hand)
                && engine.state.players[owner_index]
                    .hand
                    .contains(&reference.object_id)
        })
}

pub(in crate::engine) fn plan_is_current(
    engine: &GameEngine,
    stack: &ParkedStackResolution,
    plan: &ProgramHandLibraryReturnPlan,
) -> bool {
    if stack.item.controller != plan.owner
        || !selected_hand_is_current(engine, plan.owner, &plan.selected)
    {
        return false;
    }
    let Some(owner_index) = engine.state.player_idx(plan.owner) else {
        return false;
    };
    let current_commander_indices = plan
        .selected
        .iter()
        .enumerate()
        .filter_map(|(index, reference)| {
            engine.state.players[owner_index]
                .declared_commander_object_ids
                .contains(&reference.object_id)
                .then_some(index)
        })
        .collect::<Vec<_>>();
    if current_commander_indices != plan.commander_choice_indices
        || plan.commander_choice_cursor > plan.commander_choice_indices.len()
        || plan.commander_destinations.len() != plan.commander_choice_cursor
    {
        return false;
    }
    for (answered_index, (selected_index, destination)) in
        plan.commander_destinations.iter().enumerate()
    {
        if plan.commander_choice_indices.get(answered_index) != Some(selected_index)
            || !matches!(destination, Zone::Command | Zone::Library)
        {
            return false;
        }
    }
    match plan.stage {
        PendingProgramHandLibraryReturnStage::ReadyToCommit => {
            plan.commander_choice_cursor == plan.commander_choice_indices.len()
        }
        PendingProgramHandLibraryReturnStage::ChoosingCommander { index } => {
            plan.commander_choice_indices
                .get(plan.commander_choice_cursor)
                == Some(&index)
        }
    }
}

pub(in crate::engine) fn publish_commander_choice(
    engine: &mut GameEngine,
    stack: ParkedStackResolution,
    mut plan: ProgramHandLibraryReturnPlan,
    events: &mut Vec<rv1::RuledEvent>,
) -> Result<(), EngineError> {
    if plan.commander_choice_cursor >= plan.commander_choice_indices.len() {
        return Err(EngineError::ProgramInvariant(
            "Brainstorm commander choice cursor is past its cohort",
        ));
    }
    let index = plan.commander_choice_indices[plan.commander_choice_cursor];
    let reference = *plan
        .selected
        .get(index)
        .ok_or(EngineError::ProgramInvariant(
            "Brainstorm commander choice index is outside its cohort",
        ))?;
    plan.stage = PendingProgramHandLibraryReturnStage::ChoosingCommander { index };
    if !plan_is_current(engine, &stack, &plan) {
        return Err(EngineError::ProgramInvariant(
            "Brainstorm commander choice was built from a stale hand cohort",
        ));
    }

    let card_name = object_display_name(&engine.state, engine.registry, reference.object_id);
    let pending = PendingResolution {
        deciding_player: plan.owner,
        presentation: PendingResolutionPresentation {
            source_object_id: stack.item.id,
            candidates: Vec::new(),
            min: 1,
            max: 1,
            ordered: false,
            prompt: "Choose a destination for the selected card.".into(),
            choice_kind: rv1::ChoiceKind::ResolutionBranch,
            unique_names: false,
        },
        continuation: ResolutionContinuation::ProgramHandLibraryReturn { stack, plan },
        resolution_prompt_id: ResolutionPromptId::UNASSIGNED,
        resolution_prompt_semantics: None::<ResolutionPromptSemanticSnapshot>,
    };
    events.push(super::super::events::ev_log_private(
        format!(
            "P{}: choose a destination for {card_name}.",
            pending.deciding_player
        ),
        pending.deciding_player,
    ));
    events.push(program_hand_library_return_choice_event(&pending));
    engine.state.pending_resolution = Some(pending);
    Ok(())
}

pub(in crate::engine) fn program_hand_library_return_choice_event(
    pending: &PendingResolution,
) -> rv1::RuledEvent {
    let branches = ["Command zone", "Library"]
        .into_iter()
        .enumerate()
        .map(|(branch_index, label)| rv1::ResolutionBranchOption {
            branch_index: branch_index as u32,
            label: label.into(),
            selectable: true,
            ..Default::default()
        })
        .collect();
    rv1::RuledEvent {
        ev: Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(
            rv1::ResolutionChoiceRequired {
                resolution_prompt_id: 0,
                deciding_player_id: pending.deciding_player,
                source_object_id: pending.presentation.source_object_id,
                prompt_text: pending.presentation.prompt.clone(),
                choice_kind: rv1::ChoiceKind::ResolutionBranch as i32,
                min: pending.presentation.min,
                max: pending.presentation.max,
                resolution_branches: branches,
                ..Default::default()
            },
        )),
    }
}

pub(in crate::engine) fn commit_program_hand_library_return(
    engine: &mut GameEngine,
    stack: &ParkedStackResolution,
    plan: &ProgramHandLibraryReturnPlan,
    events: &mut Vec<rv1::RuledEvent>,
) -> Result<(), EngineError> {
    if !plan_is_current(engine, stack, plan)
        || plan.stage != PendingProgramHandLibraryReturnStage::ReadyToCommit
    {
        return Err(EngineError::ProgramInvariant(
            "Brainstorm hand return changed before its complete commit",
        ));
    }

    let commander_destinations = plan
        .commander_destinations
        .iter()
        .copied()
        .collect::<std::collections::HashMap<_, _>>();
    let moves = plan
        .selected
        .iter()
        .enumerate()
        .map(|(index, reference)| {
            (
                reference.object_id,
                commander_destinations
                    .get(&index)
                    .copied()
                    .unwrap_or(Zone::Library),
                None,
            )
        })
        .collect::<Vec<_>>();
    let command_zone_ids = plan
        .selected
        .iter()
        .enumerate()
        .filter_map(|(index, reference)| {
            (commander_destinations.get(&index) == Some(&Zone::Command))
                .then_some(reference.object_id)
        })
        .collect::<Vec<_>>();
    let library_ids = plan
        .selected
        .iter()
        .enumerate()
        .filter_map(|(index, reference)| {
            (commander_destinations.get(&index) != Some(&Zone::Command))
                .then_some(reference.object_id)
        })
        .collect::<Vec<_>>();
    let label = engine
        .registry
        .get(&stack.item.card_id)
        .and_then(|definition| definition.face(stack.item.face_index))
        .map(|face| face.name.to_string())
        .unwrap_or_else(|| stack.item.card_id.clone());
    // Build the exact hand-origin receipt before movement, but publish it only after the entire
    // accepted cohort has committed. Library-destined cards never enter this public receipt.
    let reveal_events = if command_zone_ids.is_empty() {
        Vec::new()
    } else {
        super::reveals::reveal_cards(
            &engine.state,
            engine.registry,
            &command_zone_ids,
            stack.item.id,
            &label,
        )
    };
    let receipts = engine.commit_observed_zone_move_batch(&moves, events)?;
    if receipts.len() != plan.selected.len()
        || plan.selected.iter().enumerate().any(|(index, reference)| {
            let destination = commander_destinations
                .get(&index)
                .copied()
                .unwrap_or(Zone::Library);
            !receipts.iter().any(|receipt| {
                receipt.object_id == reference.object_id
                    && receipt.owner == plan.owner
                    && receipt.origin == Zone::Hand
                    && receipt.before_zone_change_generation == reference.zone_change_generation
                    && receipt.destination == destination
                    && receipt.destination_generation == reference.zone_change_generation + 1
            })
        })
    {
        return Err(EngineError::ProgramInvariant(
            "Brainstorm hand return did not commit every exact generation-bound move",
        ));
    }

    let owner_index = engine
        .state
        .player_idx(plan.owner)
        .ok_or(EngineError::ProgramInvariant(
            "Brainstorm controller disappeared during its hand return",
        ))?;
    let library = &mut engine.state.players[owner_index].library;
    library.retain(|object_id| !library_ids.contains(object_id));
    // The ordered hand choice is bottom-to-top; later selected library cards go above earlier
    // ones, matching Brainstorm's established top-of-library contract.
    for object_id in &library_ids {
        library.push_front(*object_id);
    }

    events.extend(reveal_events);
    for object_id in &command_zone_ids {
        events.push(permanent_moved_event(
            &engine.state,
            *object_id,
            plan.owner,
            rv1::permanent_moved::Destination::Command,
        ));
        let card_name = object_display_name(&engine.state, engine.registry, *object_id);
        events.push(super::ev_log(format!(
            "P{} puts {card_name} into the command zone instead of the library ({label}).",
            plan.owner
        )));
    }
    if !library_ids.is_empty() {
        events.push(super::ev_log(format!(
            "P{} puts {} card{} on top of their library ({label}).",
            plan.owner,
            library_ids.len(),
            if library_ids.len() == 1 { "" } else { "s" },
        )));
    }
    Ok(())
}

impl GameEngine {
    pub(in crate::engine) fn finish_program_hand_library_return_choice(
        &mut self,
        pending: PendingResolution,
        answer: &rv1::SubmitResolutionChoice,
    ) -> Result<RuledEventBatch, EngineError> {
        let (stack, mut plan) = match &pending.continuation {
            ResolutionContinuation::ProgramHandLibraryReturn { stack, plan } => {
                (stack.clone(), plan.clone())
            }
            _ => unreachable!("Brainstorm hand-library continuation"),
        };
        if answer.selected_branch_index > 1 {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal(
                "unknown Brainstorm commander destination",
            ));
        }
        let PendingProgramHandLibraryReturnStage::ChoosingCommander { index } = plan.stage else {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal(
                "Brainstorm is not waiting for a commander destination",
            ));
        };
        if pending.deciding_player != plan.owner
            || stack.item.controller != plan.owner
            || !plan_is_current(self, &stack, &plan)
        {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal(
                "Brainstorm commander destination cohort became stale",
            ));
        }

        let destination = if answer.selected_branch_index == 0 {
            Zone::Command
        } else {
            Zone::Library
        };
        plan.commander_destinations.push((index, destination));
        plan.commander_choice_cursor += 1;
        plan.stage = PendingProgramHandLibraryReturnStage::ReadyToCommit;
        let mut events = Vec::new();
        if plan.commander_choice_cursor < plan.commander_choice_indices.len() {
            if let Err(error) = publish_commander_choice(self, stack, plan, &mut events) {
                self.state.pending_resolution = Some(pending);
                return Err(error);
            }
            return Ok(super::super::events::finish_with_events(self, events));
        }

        if let Err(error) = commit_program_hand_library_return(self, &stack, &plan, &mut events) {
            self.state.pending_resolution = Some(pending);
            return Err(error);
        }
        self.complete_parked_stack_with_previous(stack, EffectResult::default(), events)
    }

    pub(in crate::engine) fn refresh_program_hand_library_return_departure(
        &mut self,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<(), EngineError> {
        let Some(pending) = self.state.pending_resolution.take() else {
            return Ok(());
        };
        let (stack, plan) = match &pending.continuation {
            ResolutionContinuation::ProgramHandLibraryReturn { stack, plan } => (stack, plan),
            _ => {
                self.state.pending_resolution = Some(pending);
                return Ok(());
            }
        };
        if self
            .state
            .player_idx(plan.owner)
            .is_none_or(|index| self.state.players[index].has_lost)
        {
            // A departed caster's uncommitted hand cohort and remaining Program frame end
            // together. The departure path removes the owned source and hand cards.
            return Ok(());
        }
        if stack.item.controller != plan.owner || !plan_is_current(self, stack, plan) {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::ProgramInvariant(
                "Brainstorm hand cohort changed during unrelated player departure",
            ));
        }

        events.push(program_hand_library_return_private_prompt_log(
            self, &pending,
        ));
        events.push(program_hand_library_return_choice_event(&pending));
        self.state.pending_resolution = Some(pending);
        Ok(())
    }
}

fn program_hand_library_return_private_prompt_log(
    engine: &GameEngine,
    pending: &PendingResolution,
) -> rv1::RuledEvent {
    let card_name = match &pending.continuation {
        ResolutionContinuation::ProgramHandLibraryReturn { plan, .. } => plan
            .commander_choice_indices
            .get(plan.commander_choice_cursor)
            .and_then(|index| plan.selected.get(*index))
            .map(|reference| {
                object_display_name(&engine.state, engine.registry, reference.object_id)
            })
            .unwrap_or_else(|| "the selected card".into()),
        _ => "the selected card".into(),
    };
    super::super::events::ev_log_private(
        format!(
            "P{}: choose a destination for {card_name}.",
            pending.deciding_player
        ),
        pending.deciding_player,
    )
}
