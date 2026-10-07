use super::{EffectCx, EffectOutcome};
use crate::engine::EngineError;
use crate::state::{
    ParkedStackResolution, PendingResolution, PendingResolutionPresentation, ResolutionContinuation,
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
    let prompt = "Choose any number of permanents and/or players with counters.".to_string();
    let max = candidate_objects
        .len()
        .saturating_add(candidate_player_ids.len()) as u32;
    cx.events.push(rv1::RuledEvent {
        ev: Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(
            rv1::ResolutionChoiceRequired {
                variable_mana_contribution: false,
                deciding_player_id: cx.controller,
                source_object_id: cx.top.id,
                prompt_text: prompt.clone(),
                choice_kind: rv1::ChoiceKind::Proliferate as i32,
                candidate_object_ids: candidate_objects.clone(),
                candidate_player_ids: candidate_player_ids.clone(),
                candidate_names,
                min: 0,
                max,
                ..Default::default()
            },
        )),
    });
    cx.events.push(super::super::events::ev_log(prompt.clone()));
    cx.engine.state.pending_resolution = Some(PendingResolution {
        deciding_player: cx.controller,
        presentation: PendingResolutionPresentation {
            source_object_id: cx.top.id,
            candidates: candidate_objects,
            min: 0,
            max,
            ordered: false,
            prompt,
            choice_kind: rv1::ChoiceKind::Proliferate,
            unique_names: false,
        },
        continuation: ResolutionContinuation::Proliferate {
            stack: ParkedStackResolution::new(cx.top.clone())
                .with_previous_result(cx.previous_effect_result.clone()),
            candidate_generations,
            candidate_player_ids,
        },
    });
    Ok(EffectOutcome::Suspended)
}
