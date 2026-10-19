use super::*;

impl GameEngine {
    /// Reconcile an already-offered Ward payment after its payer leaves. The concession path
    /// calls this only after removing that player's objects, so a captured target that left with
    /// them is not recreated by a payment-time rollback. There is no accepted Decline command and
    /// no chooser delegation for this CR 800.4f outcome.
    pub(in crate::engine) fn refresh_departed_ward_payment(
        &mut self,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<bool, EngineError> {
        let Some(pending) = self.state.pending_resolution.as_ref() else {
            return Ok(false);
        };
        if !matches!(
            &pending.continuation,
            ResolutionContinuation::WardPayment { .. }
        ) {
            return Ok(false);
        }
        let payer = pending.deciding_player;
        if self
            .state
            .player_idx(payer)
            .is_some_and(|index| !self.state.players[index].has_lost)
        {
            return Ok(false);
        }

        let pending = self
            .state
            .pending_resolution
            .take()
            .expect("Ward payment was observed above");
        let completed = self.finish_ward_unpaid(
            pending,
            tricerules_card_model::primitives::CounterPaymentOutcome::ForcedUnpaid,
            std::mem::take(events),
        )?;
        *events = completed.events;
        Ok(true)
    }

    /// Restore a saved Ward offer after its independent observer return. Keep the original parked
    /// stack and mana checkpoint; refresh only the live payment projection and card incarnations.
    pub(in crate::engine) fn refresh_suspended_ward_payment(
        &mut self,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<bool, EngineError> {
        let Some(pending) = self.state.pending_resolution.as_ref() else {
            return Ok(false);
        };
        let ResolutionContinuation::WardPayment { stack, ward } = &pending.continuation else {
            return Ok(false);
        };
        let payer = pending.deciding_player;
        if self
            .state
            .player_idx(payer)
            .is_none_or(|index| self.state.players[index].has_lost)
        {
            return self.refresh_departed_ward_payment(events);
        }

        match &ward.stage {
            PendingWardPaymentStage::Mana(_) => {
                let offer =
                    self.resolution_payment_choice_event()
                        .ok_or(EngineError::ProgramInvariant(
                            "restored Ward mana payment lost its live offer",
                        ))?;
                events.push(offer);
            }
            PendingWardPaymentStage::Discard { .. } => {
                let cost = self.ward_discard_cost(stack)?;
                let candidates = self.resolution_cost_candidates(payer, 0, 0, &cost);
                if candidates.is_empty() {
                    let pending = self
                        .state
                        .pending_resolution
                        .take()
                        .expect("restored Ward discard was observed above");
                    let completed = self.finish_ward_unpaid(
                        pending,
                        tricerules_card_model::primitives::CounterPaymentOutcome::NotOffered,
                        std::mem::take(events),
                    )?;
                    *events = completed.events;
                    return Ok(true);
                }
                let candidate_generations = candidates
                    .iter()
                    .map(|object_id| {
                        (
                            *object_id,
                            self.state
                                .zone_change_generation
                                .get(object_id)
                                .copied()
                                .unwrap_or(0),
                        )
                    })
                    .collect::<Vec<_>>();
                let candidate_card_ids = candidates
                    .iter()
                    .map(|object_id| {
                        self.state
                            .objects
                            .get(object_id)
                            .map(|object| object.card_id.clone())
                            .unwrap_or_default()
                    })
                    .collect();
                let candidate_names = candidates
                    .iter()
                    .map(|object_id| object_display_name(&self.state, self.registry, *object_id))
                    .collect();
                let prompt = self
                    .state
                    .pending_resolution
                    .as_ref()
                    .expect("restored Ward discard remains pending")
                    .presentation
                    .prompt
                    .clone();
                let source_object_id = self
                    .state
                    .pending_resolution
                    .as_ref()
                    .expect("restored Ward discard remains pending")
                    .presentation
                    .source_object_id;
                let pending = self
                    .state
                    .pending_resolution
                    .as_mut()
                    .expect("restored Ward discard remains pending");
                pending.presentation.candidates = candidates.clone();
                pending.presentation.min = 0;
                pending.presentation.max = 1;
                if let ResolutionContinuation::WardPayment {
                    ward:
                        PendingWardPayment {
                            stage:
                                PendingWardPaymentStage::Discard {
                                    candidate_generations: saved_generations,
                                },
                            ..
                        },
                    ..
                } = &mut pending.continuation
                {
                    *saved_generations = candidate_generations;
                } else {
                    return Err(EngineError::ProgramInvariant(
                        "restored Ward discard stage changed during refresh",
                    ));
                }
                events.push(rv1::RuledEvent {
                    ev: Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(
                        rv1::ResolutionChoiceRequired {
                            resolution_prompt_id: 0,
                            deciding_player_id: payer,
                            source_object_id,
                            prompt_text: prompt,
                            choice_kind: rv1::ChoiceKind::HandCards as i32,
                            candidate_object_ids: candidates,
                            candidate_card_ids,
                            candidate_names,
                            min: 0,
                            max: 1,
                            ..Default::default()
                        },
                    )),
                });
            }
        }
        Ok(true)
    }

    fn ward_discard_cost(
        &self,
        stack: &ParkedStackResolution,
    ) -> Result<tricerules_card_model::primitives::ResolutionCost, EngineError> {
        use tricerules_card_model::primitives::{ResolutionCost, SpellEffectKind};

        let program = stack.program.as_ref().ok_or(EngineError::ProgramInvariant(
            "restored Ward discard lost its authored Program state",
        ))?;
        let effect = self.current_program_perform(program)?;
        match effect {
            SpellEffectKind::CounterTriggeringStackObjectUnlessPays {
                cost: cost @ ResolutionCost::DiscardCard { .. },
            } => Ok(cost),
            _ => Err(EngineError::ProgramInvariant(
                "restored Ward discard no longer matches its payment instruction",
            )),
        }
    }

    fn finish_ward_unpaid(
        &mut self,
        pending: PendingResolution,
        outcome: tricerules_card_model::primitives::CounterPaymentOutcome,
        mut events: Vec<rv1::RuledEvent>,
    ) -> Result<RuledEventBatch, EngineError> {
        let ResolutionContinuation::WardPayment { stack, ward } = pending.continuation else {
            return Err(EngineError::ProgramInvariant(
                "forced Ward completion lost its payment continuation",
            ));
        };
        let label = stack
            .item
            .ability_text
            .as_deref()
            .unwrap_or("Ward")
            .to_string();
        counter_stack_object_ref(self, ward.target, &label, &mut events)?;
        let mut previous_result = stack.previous_result.clone();
        previous_result.counter_payment_outcome = Some(outcome);
        self.complete_parked_stack_with_previous(stack, previous_result, events)
    }

    pub(super) fn finish_ward_discard(
        &mut self,
        pending: PendingResolution,
        chosen: &[ObjectId],
    ) -> Result<RuledEventBatch, EngineError> {
        let (stack, ward, candidate_generations) = match &pending.continuation {
            ResolutionContinuation::WardPayment {
                stack,
                ward:
                    ward @ PendingWardPayment {
                        stage:
                            PendingWardPaymentStage::Discard {
                                candidate_generations,
                            },
                        ..
                    },
            } => (stack.clone(), ward.clone(), candidate_generations.clone()),
            _ => return Err(EngineError::Illegal("Ward discard continuation missing")),
        };
        if chosen.len() > 1 {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal(
                "Ward discard requires at most one card",
            ));
        }

        let mut events = Vec::new();
        if let Some(&card) = chosen.first() {
            let expected_generation = candidate_generations
                .iter()
                .find_map(|(candidate, generation)| (*candidate == card).then_some(*generation));
            let current_generation = self
                .state
                .zone_change_generation
                .get(&card)
                .copied()
                .unwrap_or(0);
            let in_payer_hand = self
                .state
                .player_idx(pending.deciding_player)
                .is_some_and(|index| self.state.players[index].hand.contains(&card));
            if expected_generation != Some(current_generation) || !in_payer_hand {
                self.state.pending_resolution = Some(pending);
                return Err(EngineError::Illegal("Ward discard choice became stale"));
            }
            // CR 702.21a: paying Ward is a semantic discard cost, so it shares the same committed
            // event boundary and replacement handling as Cycling and every other discard cost.
            let (card_name, moved) = crate::engine::resolution::perform_discard(
                self,
                pending.deciding_player,
                card,
                crate::state::DiscardCause::Cost,
                &mut events,
            )?;
            events.push(moved);
            events.push(ev_log(format!(
                "P{} discards {card_name} to pay Ward.",
                pending.deciding_player
            )));
        } else {
            let label = stack
                .item
                .ability_text
                .as_deref()
                .unwrap_or("Ward")
                .to_string();
            counter_stack_object_ref(self, ward.target, &label, &mut events)?;
            events.push(ev_log(format!(
                "P{} declines to pay Ward.",
                pending.deciding_player
            )));
        }

        let previous_result = stack.previous_result.clone();
        self.complete_parked_stack_with_previous(stack, previous_result, events)
    }
}
