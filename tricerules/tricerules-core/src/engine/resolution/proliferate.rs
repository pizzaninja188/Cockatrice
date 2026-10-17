use super::{EffectCx, EffectOutcome};
use crate::engine::EngineError;
use crate::engine::GameEngine;
use crate::state::{
    PendingProgramProliferateStage, PendingResolution, PendingResolutionPresentation,
    ResolutionContinuation,
};
use crate::{engine::GameEvent, state::Zone};
use tricerules_proto::ruled::v1 as rv1;

/// CR 701.34: choose any number of permanents and/or players that have counters, then add one of
/// each existing kind to each chosen recipient. Object and player identities stay in separate
/// candidate domains throughout the prompt and continuation.
pub(super) fn proliferate(cx: &mut EffectCx<'_>) -> Result<EffectOutcome, EngineError> {
    let mut candidate_objects = cx
        .engine
        .state
        .objects
        .values()
        .filter(|object| {
            object.zone == Zone::Battlefield && object.counters.values().any(|count| *count > 0)
        })
        .map(|object| object.id)
        .collect::<Vec<_>>();
    candidate_objects.sort_unstable();
    let candidate_player_ids = cx
        .engine
        .state
        .players
        .iter()
        .filter(|player| !player.has_lost && player.counters.values().any(|count| *count > 0))
        .map(|player| player.id)
        .collect::<Vec<_>>();

    if candidate_objects.is_empty() && candidate_player_ids.is_empty() {
        cx.engine.fire_triggers(
            &[GameEvent::Proliferated {
                player: cx.controller,
            }],
            cx.events,
        );
        cx.events.push(super::super::events::ev_log(format!(
            "P{} proliferates.",
            cx.controller
        )));
        return Ok(EffectOutcome::Continue);
    }

    let candidate_generations = candidate_objects
        .iter()
        .map(|&object_id| {
            (
                object_id,
                cx.engine
                    .state
                    .zone_change_generation
                    .get(&object_id)
                    .copied()
                    .unwrap_or(0),
            )
        })
        .collect::<Vec<_>>();
    let prompt = "Choose any number of permanents and/or players with counters.".to_string();
    let max = candidate_objects
        .len()
        .saturating_add(candidate_player_ids.len()) as u32;
    let stack = cx.parked_stack();
    let is_program = stack.program.is_some();
    let stage = if is_program
        && cx
            .engine
            .state
            .player_idx(cx.controller)
            .is_none_or(|index| cx.engine.state.players[index].has_lost)
    {
        let candidates = super::resolution_choice_delegate_candidates(
            &cx.engine.state,
            cx.controller,
            cx.controller,
        );
        let selector = prolif_delegate_selector(&cx.engine.state, cx.controller)
            .ok_or(EngineError::Illegal("no live Proliferate choice delegate"))?;
        let slots = candidates.into_iter().map(Some).collect::<Vec<_>>();
        (
            selector,
            PendingProgramProliferateStage::ChoosingDelegate { candidates: slots },
        )
    } else {
        (
            cx.controller,
            PendingProgramProliferateStage::SelectingRecipients,
        )
    };
    let deciding_player = stage.0;
    let pending_prompt = match &stage.1 {
        PendingProgramProliferateStage::ChoosingDelegate { .. } => format!(
            "P{deciding_player}: choose a player to make P{}'s Proliferate choice.",
            cx.controller
        ),
        PendingProgramProliferateStage::SelectingRecipients => prompt.clone(),
    };
    let continuation = if is_program {
        ResolutionContinuation::ProgramProliferate {
            stack,
            candidate_generations,
            candidate_player_ids: candidate_player_ids.clone(),
            stage: stage.1,
        }
    } else {
        ResolutionContinuation::Proliferate {
            stack,
            candidate_generations,
            candidate_player_ids: candidate_player_ids.clone(),
        }
    };
    cx.engine.state.pending_resolution = Some(PendingResolution {
        resolution_prompt_id: crate::state::ResolutionPromptId::UNASSIGNED,
        resolution_prompt_semantics: None,
        deciding_player,
        presentation: PendingResolutionPresentation {
            source_object_id: cx.top.id,
            candidates: if matches!(
                &continuation,
                ResolutionContinuation::ProgramProliferate {
                    stage: PendingProgramProliferateStage::ChoosingDelegate { .. },
                    ..
                }
            ) {
                Vec::new()
            } else {
                candidate_objects.clone()
            },
            min: u32::from(matches!(
                &continuation,
                ResolutionContinuation::ProgramProliferate {
                    stage: PendingProgramProliferateStage::ChoosingDelegate { .. },
                    ..
                }
            )),
            max: if matches!(
                &continuation,
                ResolutionContinuation::ProgramProliferate {
                    stage: PendingProgramProliferateStage::ChoosingDelegate { .. },
                    ..
                }
            ) {
                1
            } else {
                max
            },
            ordered: false,
            prompt: pending_prompt.clone(),
            choice_kind: if matches!(
                &continuation,
                ResolutionContinuation::ProgramProliferate {
                    stage: PendingProgramProliferateStage::ChoosingDelegate { .. },
                    ..
                }
            ) {
                rv1::ChoiceKind::ResolutionBranch
            } else {
                rv1::ChoiceKind::Proliferate
            },
            unique_names: false,
        },
        continuation,
    });
    if is_program {
        let event =
            cx.engine
                .program_proliferate_choice_event()
                .ok_or(EngineError::ProgramInvariant(
                    "typed Proliferate prompt does not match its continuation",
                ))?;
        cx.events.push(event);
    } else {
        let candidate_names = candidate_objects
            .iter()
            .map(|&object_id| {
                super::super::events::object_display_name(
                    &cx.engine.state,
                    cx.engine.registry,
                    object_id,
                )
            })
            .collect();
        cx.events.push(rv1::RuledEvent {
            ev: Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(
                rv1::ResolutionChoiceRequired {
                    resolution_prompt_id: 0,
                    variable_mana_contribution: false,
                    deciding_player_id: cx.controller,
                    source_object_id: cx.top.id,
                    prompt_text: prompt.clone(),
                    choice_kind: rv1::ChoiceKind::Proliferate as i32,
                    candidate_object_ids: candidate_objects,
                    candidate_player_ids,
                    candidate_names,
                    min: 0,
                    max,
                    ..Default::default()
                },
            )),
        });
    }
    cx.events.push(super::super::events::ev_log(pending_prompt));
    Ok(EffectOutcome::Suspended)
}

fn prolif_delegate_selector(state: &crate::state::GameState, controller: i32) -> Option<i32> {
    let index = state.player_idx(controller)?;
    if !state.players[index].has_lost {
        return Some(controller);
    }
    state
        .next_in_game_player_idx(index)
        .map(|next| state.players[next].id)
        .or_else(|| {
            state
                .players
                .iter()
                .find(|player| !player.has_lost)
                .map(|player| player.id)
        })
}

impl GameEngine {
    /// Rebuild the private actor/delegate Proliferate offer from the parked program state. This
    /// keeps presentation data a projection of the engine continuation and gives each delegation
    /// or recipient choice a newly assigned actor-scoped prompt id.
    pub(in crate::engine) fn program_proliferate_choice_event(&self) -> Option<rv1::RuledEvent> {
        let pending = self.state.pending_resolution.as_ref()?;
        let ResolutionContinuation::ProgramProliferate {
            stack,
            candidate_generations,
            candidate_player_ids,
            stage,
        } = &pending.continuation
        else {
            return None;
        };
        let (choice_kind, minimum, maximum, branches, object_ids, player_ids, names) = match stage {
            PendingProgramProliferateStage::ChoosingDelegate { candidates } => {
                let eligible = super::resolution_choice_delegate_candidates(
                    &self.state,
                    stack.item.controller,
                    stack.item.controller,
                );
                let branches = candidates
                    .iter()
                    .enumerate()
                    .map(|(index, candidate)| {
                        let selectable = candidate.is_some_and(|candidate| {
                            eligible.contains(&candidate)
                                && self
                                    .state
                                    .player_idx(candidate)
                                    .is_some_and(|idx| !self.state.players[idx].has_lost)
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
                    .collect();
                (
                    rv1::ChoiceKind::ResolutionBranch,
                    1,
                    1,
                    branches,
                    Vec::new(),
                    Vec::new(),
                    Vec::new(),
                )
            }
            PendingProgramProliferateStage::SelectingRecipients => {
                let object_ids = candidate_generations
                    .iter()
                    .filter_map(|&(object_id, generation)| {
                        (self
                            .state
                            .zone_change_generation
                            .get(&object_id)
                            .copied()
                            .unwrap_or(0)
                            == generation
                            && self.state.objects.get(&object_id).is_some_and(|object| {
                                object.zone == Zone::Battlefield
                                    && object.counters.values().any(|count| *count > 0)
                            }))
                        .then_some(object_id)
                    })
                    .collect::<Vec<_>>();
                let player_ids = candidate_player_ids
                    .iter()
                    .copied()
                    .filter(|&player_id| {
                        self.state.player_idx(player_id).is_some_and(|index| {
                            let player = &self.state.players[index];
                            !player.has_lost && player.counters.values().any(|count| *count > 0)
                        })
                    })
                    .collect::<Vec<_>>();
                let names = object_ids
                    .iter()
                    .map(|&object_id| {
                        super::super::events::object_display_name(
                            &self.state,
                            self.registry,
                            object_id,
                        )
                    })
                    .collect();
                (
                    rv1::ChoiceKind::Proliferate,
                    0,
                    object_ids.len().saturating_add(player_ids.len()) as u32,
                    Vec::new(),
                    object_ids,
                    player_ids,
                    names,
                )
            }
        };
        let (prompt, min, max) = match stage {
            PendingProgramProliferateStage::ChoosingDelegate { .. } => (
                format!(
                    "P{}: choose a player to make P{}'s Proliferate choice.",
                    pending.deciding_player, stack.item.controller
                ),
                minimum,
                maximum,
            ),
            PendingProgramProliferateStage::SelectingRecipients => (
                "Choose any number of permanents and/or players with counters.".to_string(),
                minimum,
                maximum,
            ),
        };
        Some(rv1::RuledEvent {
            ev: Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(
                rv1::ResolutionChoiceRequired {
                    resolution_prompt_id: pending.resolution_prompt_id.get(),
                    variable_mana_contribution: false,
                    deciding_player_id: pending.deciding_player,
                    source_object_id: pending.presentation.source_object_id,
                    prompt_text: prompt,
                    choice_kind: choice_kind as i32,
                    resolution_branches: branches,
                    candidate_object_ids: object_ids,
                    candidate_player_ids: player_ids,
                    candidate_names: names,
                    min,
                    max,
                    ..Default::default()
                },
            )),
        })
    }
}
