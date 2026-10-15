use super::*;
use crate::engine::events::finish_with_events;

pub(super) fn blight(cx: &mut EffectCx<'_>, count: u32) -> Result<EffectOutcome, EngineError> {
    let candidates = cx.engine.blight_candidates(cx.controller);
    if candidates.is_empty() {
        let receipt = crate::state::BlightReceipt {
            player: cx.controller,
            count,
            creature: None,
        };
        cx.engine
            .fire_triggers(&[GameEvent::Blighted(receipt)], cx.events);
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
                resolution_prompt_id: 0,
                variable_mana_contribution: false,
                candidate_player_ids: Vec::new(),
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
    let stack = cx.parked_stack();
    cx.engine.state.pending_resolution = Some(PendingResolution {
        resolution_prompt_id: crate::state::ResolutionPromptId::UNASSIGNED,
        resolution_prompt_semantics: None,

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
            stack,
            count,
            candidate_generations,
        },
    });
    Ok(EffectOutcome::Suspended)
}

impl GameEngine {
    /// Rebuild a parked Blight creature offer after a player departure changes battlefield
    /// control or removes a candidate's owned object. The continuation retains the full parent
    /// program; only its generation-bound candidate projection and prompt are refreshed.
    pub(in crate::engine) fn refresh_blight_departure(
        &mut self,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<(), EngineError> {
        let Some(mut pending) = self.state.pending_resolution.take() else {
            return Ok(());
        };
        let (stack, count, previous_candidates) = match &pending.continuation {
            ResolutionContinuation::Blight {
                stack,
                count,
                candidate_generations,
            } => (stack.clone(), *count, candidate_generations.clone()),
            _ => {
                self.state.pending_resolution = Some(pending);
                return Ok(());
            }
        };

        let candidates = self.blight_candidates(pending.deciding_player);
        if candidates.is_empty() {
            let mut stack = stack;
            let receipt = crate::state::BlightReceipt {
                player: pending.deciding_player,
                count,
                creature: None,
            };
            stack.item.blight_receipts.push(receipt);
            self.fire_triggers(&[GameEvent::Blighted(receipt)], events);
            events.push(ev_log(format!(
                "P{} blights {count}; no creature can receive counters.",
                pending.deciding_player
            )));
            let previous_result = stack.previous_result.clone();
            let completed = if stack.program.is_some() {
                self.complete_parked_stack_with_previous(
                    stack,
                    previous_result,
                    std::mem::take(events),
                )?
            } else {
                self.complete_parked_resolution_with_previous(
                    stack.item,
                    stack.resume_effect_index,
                    previous_result,
                    std::mem::take(events),
                )?
            };
            events.extend(completed.events);
            return Ok(());
        }

        let candidate_generations = candidates
            .iter()
            .map(|oid| {
                (
                    *oid,
                    self.state
                        .zone_change_generation
                        .get(oid)
                        .copied()
                        .unwrap_or(0),
                )
            })
            .collect::<Vec<_>>();
        let candidates_changed = candidate_generations != previous_candidates;
        let prompt_was_reset = pending.resolution_prompt_id.get() == 0;
        if candidates_changed {
            pending.presentation.candidates = candidates.clone();
            pending.resolution_prompt_semantics = None;
            let ResolutionContinuation::Blight {
                candidate_generations: saved,
                ..
            } = &mut pending.continuation
            else {
                unreachable!("validated Blight continuation")
            };
            *saved = candidate_generations;
        }

        self.state.pending_resolution = Some(pending);
        if candidates_changed || prompt_was_reset {
            let pending = self
                .state
                .pending_resolution
                .as_ref()
                .expect("refreshed Blight choice remains parked");
            let candidate_names = pending
                .presentation
                .candidates
                .iter()
                .map(|oid| object_display_name(&self.state, self.registry, *oid))
                .collect();
            events.push(rv1::RuledEvent {
                ev: Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(
                    rv1::ResolutionChoiceRequired {
                        resolution_prompt_id: 0,
                        deciding_player_id: pending.deciding_player,
                        source_object_id: pending.presentation.source_object_id,
                        prompt_text: pending.presentation.prompt.clone(),
                        choice_kind: rv1::ChoiceKind::CostObjects as i32,
                        candidate_object_ids: pending.presentation.candidates.clone(),
                        candidate_names,
                        min: pending.presentation.min,
                        max: pending.presentation.max,
                        ..Default::default()
                    },
                )),
            });
        }
        Ok(())
    }

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
        let creature = TriggerObjectRef {
            object_id: chosen,
            zone_change_generation: generation,
            controller_at_event: pending.deciding_player,
        };
        let spell_label = self
            .registry
            .get(&stack.item.card_id)
            .and_then(|card| card.face_display_name(stack.item.face_index))
            .unwrap_or(&stack.item.card_id)
            .to_owned();
        let proposed = self
            .proposed_object_counter_placement(
                pending.deciding_player,
                chosen,
                [(CounterKind::MinusOneMinusOne, *count)],
                super::super::continuous::CounterPlacementOrigin::Effect,
            )
            .into_iter()
            .collect();
        let completion = crate::state::CounterPlacementCompletion::BlightEffect {
            player: pending.deciding_player,
            count: *count,
            creature,
            spell_label: spell_label.clone(),
        };
        let mut events = Vec::new();
        let Some(_) =
            self.begin_counter_placement_batch(proposed, stack.clone(), completion, &mut events)?
        else {
            return Ok(finish_with_events(self, events));
        };
        let receipt = crate::state::BlightReceipt {
            player: pending.deciding_player,
            count: *count,
            creature: Some(creature),
        };
        stack.item.blight_receipts.push(receipt);
        let name = object_display_name(&self.state, self.registry, chosen);
        events.push(ev_log(format!(
            "P{} blights {count} using {name} ({spell_label}).",
            pending.deciding_player
        )));
        self.fire_triggers(&[GameEvent::Blighted(receipt)], &mut events);
        let previous_result = stack.previous_result.clone();
        if stack.program.is_some() {
            self.complete_parked_stack_with_previous(stack, previous_result, events)
        } else {
            self.complete_parked_resolution_with_previous(
                stack.item,
                stack.resume_effect_index,
                previous_result,
                events,
            )
        }
    }
}
