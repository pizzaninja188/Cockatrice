//! The finite direct-ability implementation for Kaya's multi-opponent Scry1 tail.
use super::*;
use crate::state::{PendingEachOpponentScry, PendingEachOpponentScryStage};

pub(super) fn each_opponent_may_scry(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::EachOpponentMayScry { count } = effect else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    if count != 1 {
        return Err(EngineError::Illegal(
            "EachOpponentMayScry only supports Scry 1",
        ));
    }

    let controller = cx.controller;
    let subjects = super::simple_player_recipients(
        &cx.engine.state,
        controller,
        controller,
        None,
        None,
        tricerules_card_model::primitives::PlayerRecipient::EachOpponent,
    );
    let Some(first_subject) = subjects.first().copied() else {
        return Ok(EffectOutcome::Continue);
    };

    let stack = cx.parked_stack();
    let spell_label = cx.spell_label.to_string();
    let source_object_id = cx.top.id;
    cx.engine.state.pending_resolution = Some(PendingResolution {
        resolution_prompt_id: crate::state::ResolutionPromptId::UNASSIGNED,
        resolution_prompt_semantics: None,
        deciding_player: first_subject,
        presentation: PendingResolutionPresentation {
            source_object_id,
            candidates: Vec::new(),
            min: 1,
            max: 1,
            ordered: false,
            prompt: format!("P{first_subject} may scry 1 ({spell_label})."),
            choice_kind: rv1::ChoiceKind::ResolutionBranch,
            unique_names: false,
        },
        continuation: ResolutionContinuation::EachOpponentMayScry {
            stack,
            scry: Box::new(PendingEachOpponentScry {
                controller,
                subjects,
                accepted_subjects: Vec::new(),
                cards: Vec::new(),
                spell_label,
                stage: PendingEachOpponentScryStage::OptIn {
                    subject_index: 0,
                    delegated_chooser: None,
                },
            }),
        },
    });
    cx.events
        .push(cx.engine.each_opponent_scry_choice_event()?.ok_or(
            EngineError::ProgramInvariant("Kaya Scry1 offer could not be built"),
        )?);
    cx.events.push(super::super::events::ev_log(format!(
        "P{first_subject} may scry 1 ({}).",
        cx.spell_label
    )));
    Ok(EffectOutcome::Suspended)
}
