use super::*;

pub(super) fn blight(cx: &mut EffectCx<'_>, count: u32) -> Result<EffectOutcome, EngineError> {
    let candidates = cx.engine.blight_candidates(cx.controller);
    if candidates.is_empty() {
        let receipt = cx.engine.complete_blight(cx.controller, count, None);
        cx.engine.fire_triggers(&[GameEvent::Blighted(receipt)]);
        cx.events.push(ev_log(format!(
            "P{} blights {count}; no creature can receive counters.",
            cx.controller
        )));
        return Ok(EffectOutcome::Blighted(receipt));
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
    let prompt = format!("Blight {count}: choose one creature you control.");
    cx.events.push(rv1::RuledEvent {
        ev: Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(
            rv1::ResolutionChoiceRequired {
                deciding_player_id: cx.controller,
                source_object_id: cx.top.id,
                prompt_text: prompt.clone(),
                choice_kind: rv1::ChoiceKind::CostObjects as i32,
                candidate_object_ids: candidates.clone(),
                candidate_names: candidates
                    .iter()
                    .map(|oid| object_display_name(&cx.engine.state, cx.engine.registry, *oid))
                    .collect(),
                min: 1,
                max: 1,
                ..Default::default()
            },
        )),
    });
    cx.engine.state.pending_resolution = Some(PendingResolution {
        deciding_player: cx.controller,
        presentation: PendingResolutionPresentation {
            source_object_id: cx.top.id,
            candidates,
            min: 1,
            max: 1,
            ordered: false,
            prompt,
            choice_kind: rv1::ChoiceKind::CostObjects,
            unique_names: false,
        },
        continuation: ResolutionContinuation::Blight {
            stack: ParkedStackResolution::new(cx.top.clone()),
            count,
            candidate_generations,
        },
    });
    Ok(EffectOutcome::Suspended)
}

impl GameEngine {
    pub(in crate::engine) fn finish_blight_choice(
        &mut self,
        pending: PendingResolution,
        chosen: ObjectId,
    ) -> Result<RuledEventBatch, EngineError> {
        let ResolutionContinuation::Blight {
            stack,
            count,
            candidate_generations,
        } = &pending.continuation
        else {
            return Err(EngineError::Illegal("not a Blight choice"));
        };
        let generation = self
            .state
            .zone_change_generation
            .get(&chosen)
            .copied()
            .unwrap_or(0);
        if !candidate_generations.contains(&(chosen, generation))
            || !self.can_blight_creature(pending.deciding_player, chosen)
        {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal("stale Blight creature"));
        }
        let mut stack = stack.clone();
        let receipt = self.complete_blight(pending.deciding_player, *count, Some(chosen));
        self.fire_triggers(&[GameEvent::Blighted(receipt)]);
        stack.item.blight_receipts.push(receipt);
        let name = object_display_name(&self.state, self.registry, chosen);
        let events = vec![ev_log(format!(
            "P{} blights {count} using {name}.",
            pending.deciding_player
        ))];
        self.complete_parked_resolution_with_previous(
            stack.item,
            stack.resume_effect_index,
            stack.previous_result,
            events,
        )
    }
}
