use super::{EffectCx, EffectOutcome};
use crate::engine::EngineError;
use crate::engine::GameEngine;
use crate::state::{PendingProliferateStage, ResolutionContinuation, Zone};
use tricerules_proto::ruled::v1 as rv1;

/// CR 701.34: route the action through the shared replacement and recipient driver. Object and
/// player identities stay in separate candidate domains throughout the prompt and continuation.
pub(super) fn proliferate(cx: &mut EffectCx<'_>) -> Result<EffectOutcome, EngineError> {
    let stack = cx.parked_stack();
    let transaction = crate::engine::proliferate::PendingProliferateTransaction::new(cx.controller);
    match cx
        .engine
        .begin_initial_proliferate_action(stack, transaction, cx.events)?
    {
        crate::engine::proliferate::InitialProliferateProgress::ContinuedInline => {
            Ok(EffectOutcome::Continue)
        }
        crate::engine::proliferate::InitialProliferateProgress::Parked => {
            Ok(EffectOutcome::Suspended)
        }
    }
}

pub(in crate::engine) fn prolif_delegate_selector(
    state: &crate::state::GameState,
    controller: i32,
) -> Option<i32> {
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
    /// Rebuild the private actor/delegate Proliferate offer from the parked transaction. This
    /// keeps presentation data a projection of the engine continuation and gives each delegation
    /// or recipient choice a newly assigned actor-scoped prompt id.
    pub(in crate::engine) fn proliferate_choice_event(&self) -> Option<rv1::RuledEvent> {
        let pending = self.state.pending_resolution.as_ref()?;
        let (candidate_generations, candidate_player_ids, stage, actor) =
            match &pending.continuation {
                ResolutionContinuation::Proliferate {
                    candidate_generations,
                    candidate_player_ids,
                    stage,
                    transaction,
                    ..
                } => (
                    candidate_generations,
                    candidate_player_ids,
                    stage,
                    transaction.actor,
                ),
                _ => return None,
            };
        let (choice_kind, minimum, maximum, branches, object_ids, player_ids, names) = match stage {
            PendingProliferateStage::ChoosingDelegate { candidates } => {
                let eligible =
                    super::resolution_choice_delegate_candidates(&self.state, actor, actor);
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
            PendingProliferateStage::SelectingRecipients => {
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
            PendingProliferateStage::ChoosingDelegate { .. } => (
                format!(
                    "P{}: choose a player to make P{}'s Proliferate choice.",
                    pending.deciding_player, actor
                ),
                minimum,
                maximum,
            ),
            PendingProliferateStage::SelectingRecipients => (
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
