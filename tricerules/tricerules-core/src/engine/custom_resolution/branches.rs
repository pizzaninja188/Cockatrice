use super::super::payment::PaidCardCost;
use super::*;

impl GameEngine {
    pub(in crate::engine) fn resolution_payment_choice_event(&self) -> Option<rv1::RuledEvent> {
        if let Some(pending) = self.state.pending_resolution.as_ref() {
            if let ResolutionContinuation::SpecialCast { exiled, .. } = &pending.continuation {
                return Some(rv1::RuledEvent {
                    ev: Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(
                        rv1::ResolutionChoiceRequired {
                            deciding_player_id: pending.deciding_player,
                            source_object_id: pending.presentation.source_object_id,
                            prompt_text: pending.presentation.prompt.clone(),
                            choice_kind: rv1::ChoiceKind::SpecialCast as i32,
                            candidate_object_ids: vec![exiled.object_id],
                            min: 0,
                            max: 1,
                            candidate_names: vec![object_display_name(
                                &self.state,
                                self.registry,
                                exiled.object_id,
                            )],
                            candidate_selectable: vec![true],
                            ..Default::default()
                        },
                    )),
                });
            }
        }

        let pending = self.state.pending_resolution.as_ref()?;
        let payment = pending.continuation.mana_payment()?;
        Some(rv1::RuledEvent {
            ev: Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(
                rv1::ResolutionChoiceRequired {
                    deciding_player_id: pending.deciding_player,
                    source_object_id: pending.presentation.source_object_id,
                    prompt_text: pending.presentation.prompt.clone(),
                    choice_kind: rv1::ChoiceKind::ManaPayment as i32,
                    candidate_object_ids: Vec::new(),
                    candidate_card_ids: Vec::new(),
                    min: 0,
                    max: 0,
                    ordered: false,
                    candidate_names: Vec::new(),
                    candidate_server_card_ids: Vec::new(),
                    candidate_selectable: Vec::new(),
                    resolution_branches: Vec::new(),
                    unique_names: false,
                    generic_mana_cost: payment.generic_mana_cost,
                    payment_currently_legal: if payment.mana_cost.pips.is_empty() {
                        self.can_pay_generic_mana(
                            pending.deciding_player,
                            payment.generic_mana_cost,
                        )
                    } else {
                        self.can_pay_resolution_mana(pending.deciding_player, &payment.mana_cost)
                    },
                    mana_cost: payment.mana_cost.to_string(),
                    public_reveal: None,
                    candidate_source_zones: Vec::new(),
                    combat_defender_options: Vec::new(),
                    waterbend: payment.waterbend,
                    selection_slots: Vec::new(),
                    replacement_options: Vec::new(),
                    selection_alternatives: Vec::new(),
                },
            )),
        })
    }

    pub(super) fn finish_resolution_mana_payment(
        &mut self,
        pending: PendingResolution,
        payment: PendingManaPayment,
        answer: &rv1::SubmitResolutionChoice,
        decision: rv1::ResolutionChoiceDecision,
        player: PlayerId,
    ) -> Result<RuledEventBatch, EngineError> {
        if !answer.chosen_object_ids.is_empty() {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal(
                "mana payment choice cannot include object ids",
            ));
        }
        let mut events = Vec::new();
        let soft_counter_paid = matches!(
            pending.continuation,
            ResolutionContinuation::ManaPayment { .. }
        )
        .then_some(decision == rv1::ResolutionChoiceDecision::PayMana);
        match decision {
            rv1::ResolutionChoiceDecision::PayMana => {
                let result = (|| {
                    let costs = self.prepare_resolution_payment_costs(
                        player,
                        &payment,
                        &answer.restricted_mana,
                    )?;
                    let selection = answer
                        .payment
                        .as_ref()
                        .ok_or(EngineError::Illegal("resolution payment was not previewed"))?;
                    let life = self.validate_explicit_payment(
                        player,
                        pending.presentation.source_object_id,
                        false,
                        &costs,
                        selection,
                    )?;
                    let plan = costs.finish_explicit(&self.state, selection, life)?;
                    self.commit_cost_transaction(plan)
                })();
                let receipt = match result {
                    Ok(receipt) => receipt,
                    Err(error) => {
                        self.state.pending_resolution = Some(pending);
                        return Err(error);
                    }
                };
                self.fire_resolution_cost_triggers(receipt.trigger_events, receipt.sacrificed);
                events.extend(receipt.move_events);
                let cost_label = if payment.mana_cost.pips.is_empty() {
                    format!("{{{}}}", payment.generic_mana_cost)
                } else {
                    payment.mana_cost.to_string()
                };
                events.push(ev_log(format!(
                    "P{player} pays {cost_label} during resolution."
                )));
                self.state.undoable_mana_abilities.clear();
            }
            rv1::ResolutionChoiceDecision::Decline => {
                if answer.payment.is_some() || !answer.restricted_mana.is_empty() {
                    self.state.pending_resolution = Some(pending);
                    return Err(EngineError::Illegal("decline cannot include a payment"));
                }
                while self.state.undoable_mana_abilities.len() > payment.undo_history_start {
                    events.push(
                        self.rewind_last_undoable_mana_ability(player, payment.undo_history_start)?,
                    );
                }
                // Resolution is consequential even though the payment-time entries were
                // rewound. Older float stays in the pool but is no longer eligible for Undo.
                self.state.undoable_mana_abilities.clear();
                if matches!(
                    pending.continuation,
                    ResolutionContinuation::AuthoredBranch { .. }
                ) {
                    let stack = pending
                        .continuation
                        .stack()
                        .expect("authored branch has a stack continuation");
                    let effect_index = stack
                        .resume_effect_index
                        .and_then(|next| next.checked_sub(1))
                        .ok_or(EngineError::Illegal(
                            "resolution branch continuation missing",
                        ))?;
                    let mut item = stack.item.clone();
                    if let ResolutionContinuation::AuthoredBranch { branch, .. } =
                        &pending.continuation
                    {
                        if branch.optional {
                            item.resolution_branch_choices.insert(effect_index, None);
                        } else {
                            // Cancelling a payment is not permission to omit a mandatory choice.
                            item.resolution_branch_choices.remove(&effect_index);
                        }
                    }
                    return self.complete_parked_resolution_with_previous(
                        item,
                        Some(effect_index),
                        stack.previous_result.clone(),
                        events,
                    );
                } else if let ResolutionContinuation::WardPayment { stack, ward } =
                    &pending.continuation
                {
                    let label = stack
                        .item
                        .ability_text
                        .as_deref()
                        .unwrap_or("Ward")
                        .to_string();
                    counter_stack_object_ref(self, ward.target, &label, &mut events)?;
                } else {
                    let stack = pending
                        .continuation
                        .stack()
                        .expect("mana payment has a stack continuation");
                    let counter_label = self
                        .registry
                        .get(&stack.item.card_id)
                        .map(|definition| definition.name.clone())
                        .unwrap_or_else(|| stack.item.card_id.clone());
                    counter_stack_object(
                        self,
                        payment.target_spell_id,
                        &counter_label,
                        &mut events,
                    )?;
                }
            }
            rv1::ResolutionChoiceDecision::Unspecified => {
                self.state.pending_resolution = Some(pending);
                return Err(EngineError::Illegal(
                    "mana payment choice requires pay or decline",
                ));
            }
            rv1::ResolutionChoiceDecision::SelectBranch => {
                self.state.pending_resolution = Some(pending);
                return Err(EngineError::Illegal(
                    "mana payment choice cannot select a branch",
                ));
            }
            rv1::ResolutionChoiceDecision::CastSpell => {
                self.state.pending_resolution = Some(pending);
                return Err(EngineError::Illegal(
                    "mana payment choice cannot cast a transformed card",
                ));
            }
        }
        let stack = pending
            .continuation
            .stack()
            .expect("mana payment has a stack continuation")
            .clone();
        let resume = if matches!(
            pending.continuation,
            ResolutionContinuation::AuthoredBranch { .. }
        ) {
            stack
                .resume_effect_index
                .and_then(|next| next.checked_sub(1))
        } else {
            stack.resume_effect_index
        };
        let mut previous_result = stack.previous_result;
        if let Some(paid) = soft_counter_paid {
            previous_result.receipt = Some(ResolutionReceipt::CounterUnlessPaid { paid });
        }
        self.complete_parked_resolution_with_previous(stack.item, resume, previous_result, events)
    }

    pub(in crate::engine) fn resolution_cost_candidates(
        &self,
        player: PlayerId,
        source_object_id: ObjectId,
        source_zone_change: u64,
        cost: &ResolutionCost,
    ) -> Vec<ObjectId> {
        super::super::payment::components::ObjectPaymentComponent::resolution(
            rv1::CostObjectRef {
                object_id: source_object_id,
                zone_change_generation: source_zone_change,
            },
            cost,
        )
        .map_or_else(Vec::new, |component| component.candidates(self, player))
    }

    pub(super) fn select_resolution_branch(
        &mut self,
        mut pending: PendingResolution,
        answer: &rv1::SubmitResolutionChoice,
        decision: rv1::ResolutionChoiceDecision,
    ) -> Result<RuledEventBatch, EngineError> {
        if !answer.chosen_object_ids.is_empty() {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal(
                "resolution branch selection cannot include object ids",
            ));
        }
        let (stack, branch_state) = match &pending.continuation {
            ResolutionContinuation::AuthoredBranch { stack, branch }
                if matches!(branch.stage, PendingResolutionBranchStage::Selecting) =>
            {
                (stack.clone(), branch.clone())
            }
            _ => {
                self.state.pending_resolution = Some(pending);
                return Err(EngineError::Illegal(
                    "resolution branch continuation missing",
                ));
            }
        };
        let effect_index = stack
            .resume_effect_index
            .and_then(|next| next.checked_sub(1))
            .ok_or(EngineError::Illegal(
                "resolution branch effect index missing",
            ))?;

        if decision == rv1::ResolutionChoiceDecision::Decline {
            if !branch_state.optional {
                self.state.pending_resolution = Some(pending);
                return Err(EngineError::Illegal("resolution branch is not optional"));
            }
            let mut item = stack.item;
            item.resolution_branch_choices.insert(effect_index, None);
            return self.complete_parked_resolution_with_previous(
                item,
                Some(effect_index),
                stack.previous_result,
                vec![ev_log(format!(
                    "P{} declines the optional resolution choice.",
                    pending.deciding_player
                ))],
            );
        }
        if decision != rv1::ResolutionChoiceDecision::SelectBranch {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal(
                "resolution branch requires a branch selection or decline",
            ));
        }
        let branch_index = answer.selected_branch_index as usize;
        let Some(branch) = branch_state.branches.get(branch_index).cloned() else {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal("bad resolution branch index"));
        };
        if !resolution::resolution_branch_is_live(
            self,
            &stack.item,
            &stack.previous_result,
            pending.deciding_player,
            &branch,
        ) {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal(
                "that resolution branch is no longer legal",
            ));
        }
        let candidates = self.resolution_cost_candidates(
            pending.deciding_player,
            stack.item.source_permanent_id.unwrap_or(stack.item.id),
            stack.item.source_zone_change,
            &branch.cost,
        );
        let required_candidates = match branch.cost {
            ResolutionCost::TapPermanents { count, .. } => count as usize,
            ResolutionCost::None | ResolutionCost::Mana(_) | ResolutionCost::Waterbend(_) => 0,
            _ => 1,
        };
        if candidates.len() < required_candidates {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal(
                "that resolution branch no longer has a legal payment",
            ));
        }
        pending
            .continuation
            .stack_mut()
            .expect("authored branch has a stack continuation")
            .item
            .resolution_branch_choices
            .insert(effect_index, Some(branch_index));
        let mut ev = vec![ev_log(format!(
            "P{} chooses: {}.",
            pending.deciding_player,
            branch.fallback_label()
        ))];
        let is_waterbend = matches!(branch.cost, ResolutionCost::Waterbend(_));
        match branch.cost {
            ResolutionCost::None => {
                let stack = pending
                    .continuation
                    .stack()
                    .expect("authored branch has a stack continuation")
                    .clone();
                return self.complete_parked_resolution_with_previous(
                    stack.item,
                    Some(effect_index),
                    stack.previous_result,
                    ev,
                );
            }
            ResolutionCost::Mana(mana_cost) | ResolutionCost::Waterbend(mana_cost) => {
                pending.presentation.choice_kind = rv1::ChoiceKind::ManaPayment;
                pending.presentation.prompt = format!("Pay {}?", mana_cost);
                let mut payment = PendingManaPayment::from_cost(
                    0,
                    mana_cost,
                    self.state.undoable_mana_abilities.len(),
                );
                payment.waterbend = is_waterbend;
                let ResolutionContinuation::AuthoredBranch { branch, .. } =
                    &mut pending.continuation
                else {
                    unreachable!("validated authored branch continuation")
                };
                branch.stage = PendingResolutionBranchStage::PayingMana {
                    selected_branch: branch_index,
                    payment,
                };
                self.state.pending_resolution = Some(pending);
                ev.push(
                    self.resolution_payment_choice_event()
                        .expect("resolution branch payment remains parked"),
                );
            }
            ResolutionCost::DiscardCard { .. }
            | ResolutionCost::SacrificePermanent { .. }
            | ResolutionCost::Blight { .. }
            | ResolutionCost::TapPermanents { .. } => {
                let is_discard = matches!(branch.cost, ResolutionCost::DiscardCard { .. });
                let is_tap = matches!(branch.cost, ResolutionCost::TapPermanents { .. });
                let count = match branch.cost {
                    ResolutionCost::TapPermanents { count, .. } => count,
                    _ => 1,
                };
                pending.presentation.choice_kind = if is_discard {
                    rv1::ChoiceKind::HandCards
                } else if is_tap || matches!(branch.cost, ResolutionCost::Blight { .. }) {
                    rv1::ChoiceKind::CostObjects
                } else {
                    rv1::ChoiceKind::TargetObjects
                };
                pending.presentation.candidates = candidates.clone();
                pending.presentation.min = if branch_state.optional { 0 } else { count };
                pending.presentation.max = count;
                pending.presentation.prompt = if let ResolutionCost::Blight { count } = branch.cost
                {
                    format!(
                        "Blight {count}: choose one creature you control{}.",
                        if branch_state.optional {
                            ", or decline"
                        } else {
                            ""
                        }
                    )
                } else if is_discard {
                    "Choose a card to discard, or decline.".into()
                } else if is_tap {
                    let plural = if count == 1 { "" } else { "s" };
                    let decline = if branch_state.optional {
                        ", or decline"
                    } else {
                        ""
                    };
                    format!("Choose {count} untapped permanent{plural} to tap{decline}.")
                } else {
                    "Choose a permanent to sacrifice, or decline.".into()
                };
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
                    .collect();
                let ResolutionContinuation::AuthoredBranch { branch, .. } =
                    &mut pending.continuation
                else {
                    unreachable!("validated authored branch continuation")
                };
                branch.stage = PendingResolutionBranchStage::PayingObjects {
                    selected_branch: branch_index,
                    candidate_generations,
                };
                let candidate_card_ids = candidates
                    .iter()
                    .map(|oid| {
                        self.state
                            .objects
                            .get(oid)
                            .map(|object| object.card_id.clone())
                            .unwrap_or_default()
                    })
                    .collect::<Vec<_>>();
                ev.push(rv1::RuledEvent {
                    ev: Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(
                        rv1::ResolutionChoiceRequired {
                            deciding_player_id: pending.deciding_player,
                            source_object_id: pending.presentation.source_object_id,
                            prompt_text: pending.presentation.prompt.clone(),
                            choice_kind: pending.presentation.choice_kind as i32,
                            candidate_object_ids: candidates.clone(),
                            candidate_card_ids,
                            min: pending.presentation.min,
                            max: count,
                            ordered: false,
                            candidate_names: self.object_names(&candidates),
                            candidate_server_card_ids: Vec::new(),
                            candidate_selectable: Vec::new(),
                            unique_names: false,
                            generic_mana_cost: 0,
                            payment_currently_legal: false,
                            resolution_branches: Vec::new(),
                            mana_cost: String::new(),
                            public_reveal: None,
                            candidate_source_zones: Vec::new(),
                            combat_defender_options: Vec::new(),
                            waterbend: false,
                            selection_slots: Vec::new(),
                            replacement_options: Vec::new(),
                            selection_alternatives: Vec::new(),
                        },
                    )),
                });
                self.state.pending_resolution = Some(pending);
            }
        }
        Ok(finish_with_events(self, ev))
    }

    pub(super) fn finish_resolution_branch_object(
        &mut self,
        pending: PendingResolution,
        chosen: &[ObjectId],
    ) -> Result<RuledEventBatch, EngineError> {
        let (mut stack, branch_state, branch_index, candidate_generations) =
            match &pending.continuation {
                ResolutionContinuation::AuthoredBranch { stack, branch } => match branch.stage {
                    PendingResolutionBranchStage::PayingObjects {
                        selected_branch,
                        ref candidate_generations,
                    } => (
                        stack.clone(),
                        branch.clone(),
                        selected_branch,
                        candidate_generations.clone(),
                    ),
                    _ => {
                        self.state.pending_resolution = Some(pending);
                        return Err(EngineError::Illegal("resolution branch was not selected"));
                    }
                },
                _ => {
                    self.state.pending_resolution = Some(pending);
                    return Err(EngineError::Illegal(
                        "resolution branch continuation missing",
                    ));
                }
            };
        let branch = branch_state
            .branches
            .get(branch_index)
            .ok_or(EngineError::Illegal("resolution branch became stale"))?;
        let effect_index = stack
            .resume_effect_index
            .and_then(|next| next.checked_sub(1))
            .ok_or(EngineError::Illegal(
                "resolution branch effect index missing",
            ))?;
        if chosen.is_empty() {
            if !branch_state.optional {
                self.state.pending_resolution = Some(pending);
                return Err(EngineError::Illegal(
                    "resolution branch payment is required",
                ));
            }
            let mut item = stack.item;
            item.resolution_branch_choices.insert(effect_index, None);
            return self.complete_parked_resolution_with_previous(
                item,
                Some(effect_index),
                stack.previous_result,
                vec![ev_log(format!(
                    "P{} declines the optional resolution choice.",
                    pending.deciding_player
                ))],
            );
        }
        let result = (|| {
            let selected = chosen
                .iter()
                .map(|oid| {
                    candidate_generations
                        .iter()
                        .find_map(|(candidate, generation)| {
                            (*candidate == *oid).then_some(rv1::CostObjectRef {
                                object_id: *oid,
                                zone_change_generation: *generation,
                            })
                        })
                        .ok_or(EngineError::Illegal("resolution payment choice is stale"))
                })
                .collect::<Result<Vec<_>, _>>()?;
            let plan = self.plan_resolution_object_costs(
                pending.deciding_player,
                rv1::CostObjectRef {
                    object_id: stack.item.source_permanent_id.unwrap_or(stack.item.id),
                    zone_change_generation: stack.item.source_zone_change,
                },
                &branch.cost,
                &selected,
            )?;
            self.commit_cost_transaction(plan)
        })();
        let receipt = match result {
            Ok(receipt) => receipt,
            Err(error) => {
                self.state.pending_resolution = Some(pending);
                return Err(error);
            }
        };
        self.fire_resolution_cost_triggers(receipt.trigger_events, receipt.sacrificed);
        let mut ev = receipt.move_events;
        if let ResolutionCost::Blight { count } = branch.cost {
            // Blight does not move its creature; the public name remains available here.
            let name = object_display_name(&self.state, self.registry, chosen[0]);
            ev.push(ev_log(format!(
                "P{} blights {count} using {name}.",
                pending.deciding_player
            )));
        } else {
            for cost in receipt.paid_card_costs {
                let (verb, name) = match cost {
                    PaidCardCost::Discard { card_name, .. } => ("discards", card_name),
                    PaidCardCost::Sacrifice { card_name, .. } => ("sacrifices", card_name),
                    PaidCardCost::Tap { card_name, .. } => ("taps", card_name),
                    PaidCardCost::Exile { .. } => {
                        unreachable!("resolution branch cannot pay exile")
                    }
                };
                ev.push(ev_log(format!(
                    "P{} {verb} {name}.",
                    pending.deciding_player
                )));
            }
        }
        stack.item.blight_receipts.extend(receipt.blight_receipts);
        self.complete_parked_resolution_with_previous(
            stack.item,
            Some(effect_index),
            stack.previous_result,
            ev,
        )
    }
}
