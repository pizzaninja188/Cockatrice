use super::super::payment::PaidCardCost;
use super::*;
use crate::state::{PendingProgramChoose, PendingProgramChooseStage};

impl GameEngine {
    pub(in crate::engine) fn resolution_payment_choice_event(&self) -> Option<rv1::RuledEvent> {
        if self
            .state
            .pending_spell_cast
            .as_ref()
            .is_some_and(|pending| pending.resolution_time_offer)
        {
            return None;
        }
        if let Some(pending) = self.state.pending_resolution.as_ref() {
            if let ResolutionContinuation::SpecialCast {
                exiled, face_index, ..
            } = &pending.continuation
            {
                let permission_scope = match &pending.continuation {
                    ResolutionContinuation::SpecialCast {
                        permission_scope, ..
                    } => {
                        permission_scope.unwrap_or(ExilePlayPermissionScope::CastFace(*face_index))
                    }
                    _ => unreachable!(),
                };
                let cast_selectable = super::super::legal_actions::legal_zone_cast_actions(
                    self,
                    pending.deciding_player,
                )
                .iter()
                .any(|action| action.object_id == exiled.object_id);
                let land_selectable = permission_scope == ExilePlayPermissionScope::PlayCard
                    && super::super::legal_actions::legal_zone_land_actions(
                        self,
                        pending.deciding_player,
                    )
                    .iter()
                    .any(|action| action.object_id == exiled.object_id);
                let object = self.state.objects.get(&exiled.object_id)?;
                let face = self.registry.get(&object.card_id)?.face(*face_index)?;
                return Some(rv1::RuledEvent {
                    ev: Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(
                        rv1::ResolutionChoiceRequired {
                            resolution_prompt_id: pending.resolution_prompt_id.get(),
                            candidate_player_ids: Vec::new(),
                            deciding_player_id: pending.deciding_player,
                            source_object_id: pending.presentation.source_object_id,
                            prompt_text: pending.presentation.prompt.clone(),
                            choice_kind: rv1::ChoiceKind::SpecialCast as i32,
                            candidate_object_ids: vec![exiled.object_id],
                            min: 0,
                            max: 1,
                            candidate_card_ids: vec![object.card_id.clone()],
                            candidate_names: vec![face.name.clone()],
                            candidate_selectable: vec![cast_selectable || land_selectable],
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
                    resolution_prompt_id: pending.resolution_prompt_id.get(),
                    accepted_permanent_selections: Vec::new(),
                    candidate_token_identities: Vec::new(),
                    candidate_player_ids: Vec::new(),
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
                    payment_currently_legal: payment.variable_mana_contribution
                        || if payment.mana_cost.pips.is_empty() {
                            self.can_pay_generic_mana(
                                pending.deciding_player,
                                payment.generic_mana_cost,
                            )
                        } else {
                            self.can_pay_resolution_mana(
                                pending.deciding_player,
                                &payment.mana_cost,
                            )
                        },
                    mana_cost: payment.mana_cost.to_string(),
                    public_reveal: None,
                    candidate_source_zones: Vec::new(),
                    combat_defender_options: Vec::new(),
                    waterbend: payment.waterbend,
                    selection_slots: Vec::new(),
                    replacement_options: Vec::new(),
                    selection_alternatives: Vec::new(),
                    variable_mana_contribution: payment.variable_mana_contribution,
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
        let program_choose_payment = matches!(
            &pending.continuation,
            ResolutionContinuation::ProgramChoose {
                choose: PendingProgramChoose {
                    stage: PendingProgramChooseStage::PayingMana { .. },
                    ..
                },
                ..
            }
        );
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
                self.fire_resolution_cost_triggers(
                    receipt.trigger_events,
                    receipt.sacrificed,
                    &mut events,
                );
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
                } else if program_choose_payment {
                    events.push(ev_log(format!(
                        "P{player} declines the program Choose payment."
                    )));
                    return self.resume_program_choose_payment(pending, false, events);
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
        let counter_payment_outcome = soft_counter_paid.map(|paid| {
            if paid {
                tricerules_card_model::primitives::CounterPaymentOutcome::Paid
            } else {
                tricerules_card_model::primitives::CounterPaymentOutcome::Declined
            }
        });
        if program_choose_payment {
            return self.resume_program_choose_payment(pending, true, events);
        }
        self.resume_mana_payment_continuation(pending, counter_payment_outcome, events)
    }

    fn resume_program_choose_payment(
        &mut self,
        pending: PendingResolution,
        paid: bool,
        events: Vec<rv1::RuledEvent>,
    ) -> Result<RuledEventBatch, EngineError> {
        let (mut stack, choose) = match &pending.continuation {
            ResolutionContinuation::ProgramChoose { stack, choose } => {
                (stack.clone(), choose.clone())
            }
            _ => {
                self.state.pending_resolution = Some(pending);
                return Err(EngineError::Illegal(
                    "program Choose payment continuation missing",
                ));
            }
        };
        let branch_id = match choose.stage {
            PendingProgramChooseStage::PayingMana { branch_id, .. } if paid => Some(branch_id),
            PendingProgramChooseStage::PayingMana { .. } if !paid => None,
            _ => {
                self.state.pending_resolution = Some(pending);
                return Err(EngineError::Illegal("program Choose payment stage changed"));
            }
        };
        self.enter_program_choose_body(&mut stack, &choose.occurrence, branch_id.as_ref())?;
        let previous_result = stack.previous_result.clone();
        self.complete_parked_stack_with_previous(stack, previous_result, events)
    }

    /// A departed payer cannot answer or delegate a standalone pay-or-decline choice. Once the
    /// departed player's objects are removed, complete the retained source at the exact parked
    /// cursor with an explicit ForcedUnpaid result. This intentionally does not call the ordinary
    /// decline path, which can rewind mana abilities and attempts to counter the payer's target.
    pub(in crate::engine) fn refresh_departed_standalone_mana_payment(
        &mut self,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<(), EngineError> {
        let Some(pending) = self.state.pending_resolution.as_ref() else {
            return Ok(());
        };
        let ResolutionContinuation::ManaPayment { stack, .. } = &pending.continuation else {
            return Ok(());
        };
        let payer_departed = self
            .state
            .player_idx(pending.deciding_player)
            .is_none_or(|index| self.state.players[index].has_lost);
        if !payer_departed {
            return Ok(());
        }

        // `detach_departed_outer_resolution` remains authoritative. It clears a pending choice
        // when the resolving object's controller or owner left. The checks here make that
        // retention boundary explicit before completing a still-live parent's parked operation.
        let source_controller_survives = self
            .state
            .player_idx(stack.item.controller)
            .is_some_and(|index| !self.state.players[index].has_lost);
        let source_owner_survives = self.state.objects.get(&stack.item.id).is_none_or(|object| {
            self.state
                .player_idx(object.owner)
                .is_some_and(|index| !self.state.players[index].has_lost)
        });
        if !source_controller_survives || !source_owner_survives {
            return Ok(());
        }

        let pending = self
            .state
            .pending_resolution
            .take()
            .expect("standalone mana-payment continuation was observed");
        let completed = self.resume_mana_payment_continuation(
            pending,
            Some(tricerules_card_model::primitives::CounterPaymentOutcome::ForcedUnpaid),
            std::mem::take(events),
        )?;
        *events = completed.events;
        Ok(())
    }

    /// Shared final tail for accepted and forced mana-payment completion. The caller owns all
    /// decision-specific work; this resumes the exact parked program frame or effect-list cursor.
    fn resume_mana_payment_continuation(
        &mut self,
        pending: PendingResolution,
        counter_payment_outcome: Option<tricerules_card_model::primitives::CounterPaymentOutcome>,
        events: Vec<rv1::RuledEvent>,
    ) -> Result<RuledEventBatch, EngineError> {
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
        let mut previous_result = stack.previous_result.clone();
        if let Some(outcome) = counter_payment_outcome {
            previous_result.counter_payment_outcome = Some(outcome);
        }
        if stack.program.is_some() {
            self.complete_parked_stack_with_previous(stack, previous_result, events)
        } else {
            self.complete_parked_resolution_with_previous(
                stack.item,
                resume,
                previous_result,
                events,
            )
        }
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

    pub(in crate::engine) fn program_choose_cost_is_selectable(
        &self,
        stack: &StackItem,
        payer: PlayerId,
        cost: &ResolutionCost,
    ) -> bool {
        if !matches!(
            cost,
            ResolutionCost::PutHandCardOnLibraryBottom
                | ResolutionCost::Blight { .. }
                | ResolutionCost::SacrificePermanent { .. }
        ) {
            return true;
        }
        !self
            .resolution_cost_candidates(
                payer,
                stack.source_permanent_id.unwrap_or(stack.id),
                stack.source_zone_change,
                cost,
            )
            .is_empty()
    }

    fn program_choose_blight_cost_choice_event(
        &self,
        pending: &PendingResolution,
        count: u32,
        candidates: &[ObjectId],
    ) -> rv1::RuledEvent {
        let prompt = format!("Blight {count}: choose one creature you control.");
        rv1::RuledEvent {
            ev: Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(
                rv1::ResolutionChoiceRequired {
                    resolution_prompt_id: pending.resolution_prompt_id.get(),
                    deciding_player_id: pending.deciding_player,
                    source_object_id: pending.presentation.source_object_id,
                    prompt_text: prompt,
                    choice_kind: rv1::ChoiceKind::CostObjects as i32,
                    candidate_object_ids: candidates.to_vec(),
                    candidate_names: self.object_names(candidates),
                    min: 1,
                    max: 1,
                    ..Default::default()
                },
            )),
        }
    }

    fn program_choose_sacrifice_cost_choice_event(
        &self,
        pending: &PendingResolution,
        candidates: &[ObjectId],
    ) -> rv1::RuledEvent {
        rv1::RuledEvent {
            ev: Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(
                rv1::ResolutionChoiceRequired {
                    resolution_prompt_id: pending.resolution_prompt_id.get(),
                    deciding_player_id: pending.deciding_player,
                    source_object_id: pending.presentation.source_object_id,
                    prompt_text: pending.presentation.prompt.clone(),
                    choice_kind: rv1::ChoiceKind::CostObjects as i32,
                    candidate_object_ids: candidates.to_vec(),
                    candidate_names: self.object_names(candidates),
                    min: 0,
                    max: 1,
                    ..Default::default()
                },
            )),
        }
    }

    fn hand_bottom_candidate_generations(&self, candidates: &[ObjectId]) -> Vec<(ObjectId, u64)> {
        candidates
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
            .collect()
    }

    fn hand_bottom_cost_choice_event(
        &self,
        pending: &PendingResolution,
        candidates: &[ObjectId],
    ) -> rv1::RuledEvent {
        rv1::RuledEvent {
            ev: Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(
                rv1::ResolutionChoiceRequired {
                    resolution_prompt_id: pending.resolution_prompt_id.get(),
                    deciding_player_id: pending.deciding_player,
                    source_object_id: pending.presentation.source_object_id,
                    prompt_text: pending.presentation.prompt.clone(),
                    choice_kind: rv1::ChoiceKind::HandCards as i32,
                    candidate_object_ids: candidates.to_vec(),
                    candidate_card_ids: candidates
                        .iter()
                        .map(|object_id| {
                            self.state
                                .objects
                                .get(object_id)
                                .map(|object| object.card_id.clone())
                                .unwrap_or_default()
                        })
                        .collect(),
                    candidate_names: self.object_names(candidates),
                    min: pending.presentation.min,
                    max: 1,
                    ..Default::default()
                },
            )),
        }
    }

    fn hand_bottom_commander_choice_event(&self, pending: &PendingResolution) -> rv1::RuledEvent {
        let prompt = format!(
            "P{}: put your commander into the command zone or on the bottom of your library?",
            pending.deciding_player
        );
        let resolution_branches = ["Command zone", "Bottom of library"]
            .into_iter()
            .enumerate()
            .map(|(index, label)| rv1::ResolutionBranchOption {
                branch_index: index as u32,
                label: label.into(),
                selectable: true,
                ..Default::default()
            })
            .collect();
        rv1::RuledEvent {
            ev: Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(
                rv1::ResolutionChoiceRequired {
                    resolution_prompt_id: pending.resolution_prompt_id.get(),
                    deciding_player_id: pending.deciding_player,
                    source_object_id: pending.presentation.source_object_id,
                    prompt_text: prompt,
                    choice_kind: rv1::ChoiceKind::ResolutionBranch as i32,
                    min: 1,
                    max: 1,
                    resolution_branches,
                    ..Default::default()
                },
            )),
        }
    }

    fn hand_bottom_commander_is_current(
        &self,
        owner: PlayerId,
        object_id: ObjectId,
        generation: u64,
    ) -> bool {
        self.state
            .objects
            .get(&object_id)
            .is_some_and(|object| object.zone == Zone::Hand && object.owner == owner)
            && self.state.player_idx(owner).is_some_and(|index| {
                !self.state.players[index].has_lost
                    && self.state.players[index].hand.contains(&object_id)
                    && self.state.players[index]
                        .declared_commander_object_ids
                        .contains(&object_id)
            })
            && self
                .state
                .zone_change_generation
                .get(&object_id)
                .copied()
                .unwrap_or(0)
                == generation
    }

    fn commit_hand_bottom_payment(
        &mut self,
        payer: PlayerId,
        selected: rv1::CostObjectRef,
        destination: Zone,
    ) -> Result<super::super::payment::transaction::CostPaymentReceipt, EngineError> {
        let plan = self.plan_resolution_hand_bottom_cost(payer, selected, destination)?;
        self.commit_cost_transaction(plan)
    }

    fn hand_bottom_payment_log(payer: PlayerId, destination: Zone) -> rv1::RuledEvent {
        let action = match destination {
            Zone::Library => "putting a card on the bottom of their library",
            Zone::Command => "putting their commander into the command zone",
            _ => unreachable!("hand-bottom payment destination was validated"),
        };
        ev_log(format!("P{payer} pays by {action}."))
    }

    fn finish_program_choose_hand_bottom_payment(
        &mut self,
        pending: PendingResolution,
        mut stack: ParkedStackResolution,
        choose: PendingProgramChoose,
        branch_id: tricerules_card_model::ChoiceId,
        selected: rv1::CostObjectRef,
        destination: Zone,
    ) -> Result<RuledEventBatch, EngineError> {
        let receipt =
            match self.commit_hand_bottom_payment(pending.deciding_player, selected, destination) {
                Ok(receipt) => receipt,
                Err(error) => {
                    self.state.pending_resolution = Some(pending);
                    return Err(error);
                }
            };
        let mut events = receipt.move_events;
        self.fire_resolution_cost_triggers(receipt.trigger_events, receipt.sacrificed, &mut events);
        events.push(Self::hand_bottom_payment_log(
            pending.deciding_player,
            destination,
        ));
        self.enter_program_choose_body(&mut stack, &choose.occurrence, Some(&branch_id))?;
        let previous_result = stack.previous_result.clone();
        self.complete_parked_stack_with_previous(stack, previous_result, events)
    }

    pub(super) fn finish_program_choose_blight_object(
        &mut self,
        pending: PendingResolution,
        chosen: &[ObjectId],
    ) -> Result<RuledEventBatch, EngineError> {
        let (stack, choose, selected_branch, branch_id, count, candidate_generations) =
            match &pending.continuation {
                ResolutionContinuation::ProgramChoose { stack, choose } => match &choose.stage {
                    PendingProgramChooseStage::PayingBlight {
                        selected_branch,
                        branch_id,
                        count,
                        candidate_generations,
                    } => (
                        stack.clone(),
                        choose.clone(),
                        *selected_branch,
                        branch_id.clone(),
                        *count,
                        candidate_generations.clone(),
                    ),
                    _ => {
                        self.state.pending_resolution = Some(pending);
                        return Err(EngineError::Illegal(
                            "program Choose is not awaiting a Blight payment",
                        ));
                    }
                },
                _ => {
                    self.state.pending_resolution = Some(pending);
                    return Err(EngineError::Illegal(
                        "program Choose Blight continuation missing",
                    ));
                }
            };
        if pending.deciding_player != choose.original_chooser {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal(
                "program Choose Blight payment cannot be delegated",
            ));
        }
        if chosen.len() != 1 {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal("Blight payment requires one creature"));
        }
        let object_id = chosen[0];
        let Some(zone_change_generation) = candidate_generations
            .iter()
            .find_map(|(candidate, generation)| (*candidate == object_id).then_some(*generation))
        else {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal("Blight payment choice is stale"));
        };
        let current_generation = self
            .state
            .zone_change_generation
            .get(&object_id)
            .copied()
            .unwrap_or(0);
        let cost = ResolutionCost::Blight { count };
        let candidates = self.resolution_cost_candidates(
            choose.original_chooser,
            stack.item.source_permanent_id.unwrap_or(stack.item.id),
            stack.item.source_zone_change,
            &cost,
        );
        if current_generation != zone_change_generation || !candidates.contains(&object_id) {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal("Blight payment choice became stale"));
        }
        let branch = match self.program_choose_selected_branch(
            &stack,
            &choose.occurrence,
            choose.original_chooser,
            selected_branch,
        ) {
            Ok(branch) if branch.branch_id == branch_id && branch.cost == cost => branch,
            Ok(_) => {
                self.state.pending_resolution = Some(pending);
                return Err(EngineError::Illegal("program Choose branch became stale"));
            }
            Err(error) => {
                self.state.pending_resolution = Some(pending);
                return Err(error);
            }
        };
        let _ = branch;
        let source = rv1::CostObjectRef {
            object_id: stack.item.source_permanent_id.unwrap_or(stack.item.id),
            zone_change_generation: stack.item.source_zone_change,
        };
        let selected = rv1::CostObjectRef {
            object_id,
            zone_change_generation,
        };
        let result = (|| {
            let plan = self.plan_resolution_object_costs(
                choose.original_chooser,
                source,
                &cost,
                &[selected],
            )?;
            let plan = self.prepare_cost_transaction_commit(plan)?;
            let counter_costs = plan.counter_placement_costs();
            if counter_costs.len() != 1
                || counter_costs[0].object_id != object_id
                || counter_costs[0].generation != zone_change_generation
                || counter_costs[0].kind != tricerules_card_model::CounterKind::MinusOneMinusOne
                || counter_costs[0].count != count
            {
                return Err(EngineError::ProgramInvariant(
                    "Blight payment plan did not retain its exact selected counter debit",
                ));
            }
            let mut placements = Vec::with_capacity(counter_costs.len());
            for debit in counter_costs {
                let mut placement = self
                    .proposed_object_counter_placement(
                        choose.original_chooser,
                        debit.object_id,
                        [(debit.kind, debit.count)],
                        super::super::continuous::CounterPlacementOrigin::Cost,
                    )
                    .ok_or(EngineError::Illegal("Blight cost recipient became stale"))?;
                placement.cost_debit_index = Some(debit.debit_index);
                placements.push(placement);
            }
            let (batch, candidates) = self.prepare_counter_placement_batch(placements)?;
            if let Some(candidates) = candidates {
                let completion = crate::state::CounterPlacementCompletion::ProgramChooseBlight {
                    choose: choose.clone(),
                    object_id,
                    zone_change_generation,
                };
                let mut events = Vec::new();
                self.park_counter_placement_choice(
                    batch,
                    candidates,
                    Some(stack.clone()),
                    completion,
                    &mut events,
                );
                return Ok(finish_with_events(self, events));
            }
            self.finish_program_choose_blight_payment(
                stack.clone(),
                choose.clone(),
                object_id,
                zone_change_generation,
                batch.events,
            )
        })();
        if result.is_err() {
            self.state.pending_resolution = Some(pending);
        }
        result
    }

    pub(super) fn finish_program_choose_sacrifice_object(
        &mut self,
        pending: PendingResolution,
        answer: &rv1::SubmitResolutionChoice,
        decision: rv1::ResolutionChoiceDecision,
    ) -> Result<RuledEventBatch, EngineError> {
        let (stack, choose, selected_branch, branch_id, candidate_generations) =
            match &pending.continuation {
                ResolutionContinuation::ProgramChoose { stack, choose } => match &choose.stage {
                    PendingProgramChooseStage::PayingSacrificePermanent {
                        selected_branch,
                        branch_id,
                        candidate_generations,
                    } => (
                        stack.clone(),
                        choose.clone(),
                        *selected_branch,
                        branch_id.clone(),
                        candidate_generations.clone(),
                    ),
                    _ => {
                        self.state.pending_resolution = Some(pending);
                        return Err(EngineError::Illegal(
                            "program Choose is not awaiting a sacrifice payment",
                        ));
                    }
                },
                _ => {
                    self.state.pending_resolution = Some(pending);
                    return Err(EngineError::Illegal(
                        "program Choose sacrifice continuation missing",
                    ));
                }
            };
        let invalid_fields = !answer.chosen_player_ids.is_empty()
            || answer.selected_branch_index != 0
            || answer.cast_spell.is_some()
            || answer.chosen_combat_defender.is_some()
            || answer.payment.is_some()
            || !answer.restricted_mana.is_empty()
            || answer.spell_cast_announcement.is_some();
        if invalid_fields {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal(
                "sacrifice payment choice contains unrelated fields",
            ));
        }
        if pending.deciding_player != choose.original_chooser {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal(
                "program Choose sacrifice payment cannot be delegated",
            ));
        }
        if decision == rv1::ResolutionChoiceDecision::Decline {
            if !answer.chosen_object_ids.is_empty() {
                self.state.pending_resolution = Some(pending);
                return Err(EngineError::Illegal(
                    "declining a sacrifice payment cannot include an object",
                ));
            }
            return self.complete_program_choose_otherwise_from_parts(
                stack,
                choose,
                format!(
                    "P{} declines the program Choose sacrifice cost; Otherwise resolves.",
                    pending.deciding_player
                ),
            );
        }
        if decision != rv1::ResolutionChoiceDecision::Unspecified {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal(
                "sacrifice payment requires an object choice or decline",
            ));
        }
        if answer.chosen_object_ids.is_empty() {
            return self.complete_program_choose_otherwise_from_parts(
                stack,
                choose,
                format!(
                    "P{} declines the program Choose sacrifice cost; Otherwise resolves.",
                    pending.deciding_player
                ),
            );
        }
        if answer.chosen_object_ids.len() != 1 {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal(
                "sacrifice payment requires one permanent",
            ));
        }

        let branch = match self.program_choose_selected_branch(
            &stack,
            &choose.occurrence,
            choose.original_chooser,
            selected_branch,
        ) {
            Ok(branch)
                if branch.branch_id == branch_id
                    && matches!(&branch.cost, ResolutionCost::SacrificePermanent { .. }) =>
            {
                branch
            }
            Ok(_) => {
                self.state.pending_resolution = Some(pending);
                return Err(EngineError::Illegal("program Choose branch became stale"));
            }
            Err(error) => {
                self.state.pending_resolution = Some(pending);
                return Err(error);
            }
        };
        let object_id = answer.chosen_object_ids[0];
        let Some(zone_change_generation) = candidate_generations
            .iter()
            .find_map(|(candidate, generation)| (*candidate == object_id).then_some(*generation))
        else {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal("sacrifice payment choice is stale"));
        };
        let current_generation = self
            .state
            .zone_change_generation
            .get(&object_id)
            .copied()
            .unwrap_or(0);
        let source = rv1::CostObjectRef {
            object_id: stack.item.source_permanent_id.unwrap_or(stack.item.id),
            zone_change_generation: stack.item.source_zone_change,
        };
        let selected = rv1::CostObjectRef {
            object_id,
            zone_change_generation,
        };
        let legal_candidates = self.resolution_cost_candidates(
            choose.original_chooser,
            source.object_id,
            source.zone_change_generation,
            &branch.cost,
        );
        if current_generation != zone_change_generation
            || !pending.presentation.candidates.contains(&object_id)
            || !legal_candidates.contains(&object_id)
        {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal(
                "sacrifice payment choice became stale",
            ));
        }
        let receipt = match self
            .plan_resolution_object_costs(
                choose.original_chooser,
                source,
                &branch.cost,
                &[selected],
            )
            .and_then(|plan| self.commit_cost_transaction(plan))
        {
            Ok(receipt) => receipt,
            Err(error) => {
                self.state.pending_resolution = Some(pending);
                return Err(error);
            }
        };
        let mut events = receipt.move_events;
        self.fire_resolution_cost_triggers(receipt.trigger_events, receipt.sacrificed, &mut events);
        for cost in receipt.paid_card_costs {
            if let PaidCardCost::Sacrifice { card_name, .. } = cost {
                events.push(ev_log(format!(
                    "P{} sacrifices {card_name}.",
                    choose.original_chooser
                )));
            }
        }
        let mut stack = stack;
        self.enter_program_choose_body(&mut stack, &choose.occurrence, Some(&branch_id))?;
        let previous_result = stack.previous_result.clone();
        self.complete_parked_stack_with_previous(stack, previous_result, events)
    }

    pub(in crate::engine) fn finish_program_choose_blight_payment(
        &mut self,
        mut stack: ParkedStackResolution,
        choose: PendingProgramChoose,
        object_id: ObjectId,
        zone_change_generation: u64,
        placements: Vec<super::super::replacement::ProposedCounterPlacement>,
    ) -> Result<RuledEventBatch, EngineError> {
        let (selected_branch, branch_id, count) = match &choose.stage {
            PendingProgramChooseStage::PayingBlight {
                selected_branch,
                branch_id,
                count,
                ..
            } => (*selected_branch, branch_id.clone(), *count),
            _ => {
                return Err(EngineError::ProgramInvariant(
                    "counter replacement lost the pending Choose Blight occurrence",
                ));
            }
        };
        let cost = ResolutionCost::Blight { count };
        let branch = self.program_choose_selected_branch(
            &stack,
            &choose.occurrence,
            choose.original_chooser,
            selected_branch,
        )?;
        if branch.branch_id != branch_id || branch.cost != cost {
            return Err(EngineError::Illegal("program Choose branch became stale"));
        }
        if self
            .state
            .zone_change_generation
            .get(&object_id)
            .copied()
            .unwrap_or(0)
            != zone_change_generation
            || !self.can_blight_creature(choose.original_chooser, object_id)
        {
            return Err(EngineError::Illegal("Blight payment choice became stale"));
        }
        let source = rv1::CostObjectRef {
            object_id: stack.item.source_permanent_id.unwrap_or(stack.item.id),
            zone_change_generation: stack.item.source_zone_change,
        };
        let selected = rv1::CostObjectRef {
            object_id,
            zone_change_generation,
        };
        // The serialized replacement continuation never stores the engine-local transaction
        // plan. Rebuild and validate it after replacement order has completed.
        let plan =
            self.plan_resolution_object_costs(choose.original_chooser, source, &cost, &[selected])?;
        let payment = self.commit_cost_transaction_with_counter_placements(plan, placements)?;
        let mut events = payment.move_events;
        self.fire_resolution_cost_triggers(payment.trigger_events, payment.sacrificed, &mut events);
        let name = self.replacement_object_display_name(object_id, None);
        events.push(ev_log(format!(
            "P{} blights {count} using {name}.",
            choose.original_chooser
        )));
        stack.item.blight_receipts.extend(payment.blight_receipts);
        self.enter_program_choose_body(&mut stack, &choose.occurrence, Some(&branch_id))?;
        let previous_result = stack.previous_result.clone();
        self.complete_parked_stack_with_previous(stack, previous_result, events)
    }

    pub(super) fn finish_program_choose_hand_bottom_object(
        &mut self,
        mut pending: PendingResolution,
        chosen: &[ObjectId],
    ) -> Result<RuledEventBatch, EngineError> {
        let (stack, choose, selected_branch, branch_id, candidate_generations) =
            match &pending.continuation {
                ResolutionContinuation::ProgramChoose { stack, choose } => match &choose.stage {
                    PendingProgramChooseStage::PayingHandCard {
                        selected_branch,
                        branch_id,
                        candidate_generations,
                    } => (
                        stack.clone(),
                        choose.clone(),
                        *selected_branch,
                        branch_id.clone(),
                        candidate_generations.clone(),
                    ),
                    _ => {
                        self.state.pending_resolution = Some(pending);
                        return Err(EngineError::Illegal(
                            "program Choose is not awaiting a hand-card payment",
                        ));
                    }
                },
                _ => {
                    self.state.pending_resolution = Some(pending);
                    return Err(EngineError::Illegal(
                        "program Choose hand-card continuation missing",
                    ));
                }
            };
        if chosen.is_empty() {
            if !choose.optional {
                self.state.pending_resolution = Some(pending);
                return Err(EngineError::Illegal(
                    "program Choose hand-card payment is required",
                ));
            }
            let payer = pending.deciding_player;
            return self.complete_program_choose_otherwise(
                pending,
                format!("P{payer} declines the optional hand-card payment."),
            );
        }
        if chosen.len() != 1 {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal(
                "hand-bottom payment requires one hand card",
            ));
        }
        let object_id = chosen[0];
        let generation = match candidate_generations
            .iter()
            .find_map(|(candidate, generation)| (*candidate == object_id).then_some(*generation))
        {
            Some(generation) => generation,
            None => {
                self.state.pending_resolution = Some(pending);
                return Err(EngineError::Illegal("resolution payment choice is stale"));
            }
        };
        let branch = match self.program_choose_selected_branch(
            &stack,
            &choose.occurrence,
            choose.original_chooser,
            selected_branch,
        ) {
            Ok(branch) if branch.branch_id == branch_id => branch,
            Ok(_) => {
                self.state.pending_resolution = Some(pending);
                return Err(EngineError::Illegal("program Choose branch became stale"));
            }
            Err(error) => {
                self.state.pending_resolution = Some(pending);
                return Err(error);
            }
        };
        if branch.cost != ResolutionCost::PutHandCardOnLibraryBottom
            || !self
                .resolution_cost_candidates(
                    pending.deciding_player,
                    stack.item.source_permanent_id.unwrap_or(stack.item.id),
                    stack.item.source_zone_change,
                    &ResolutionCost::PutHandCardOnLibraryBottom,
                )
                .contains(&object_id)
            || self
                .state
                .zone_change_generation
                .get(&object_id)
                .copied()
                .unwrap_or(0)
                != generation
        {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal(
                "resolution payment choice became stale",
            ));
        }
        let selected = rv1::CostObjectRef {
            object_id,
            zone_change_generation: generation,
        };
        if self.hand_bottom_commander_is_current(pending.deciding_player, object_id, generation) {
            pending.presentation.choice_kind = rv1::ChoiceKind::ResolutionBranch;
            pending.presentation.candidates = vec![object_id];
            pending.presentation.min = 1;
            pending.presentation.max = 1;
            pending.presentation.prompt = format!(
                "P{}: put this commander into the command zone or on the bottom of your library?",
                pending.deciding_player
            );
            let ResolutionContinuation::ProgramChoose { choose, .. } = &mut pending.continuation
            else {
                unreachable!("validated ProgramChoose continuation")
            };
            choose.stage = PendingProgramChooseStage::ChoosingCommanderDestination {
                selected_branch,
                branch_id,
                object_id,
                zone_change_generation: generation,
            };
            let event = self.hand_bottom_commander_choice_event(&pending);
            self.state.pending_resolution = Some(pending);
            return Ok(finish_with_events(self, vec![event]));
        }
        self.finish_program_choose_hand_bottom_payment(
            pending,
            stack,
            choose,
            branch_id,
            selected,
            Zone::Library,
        )
    }

    pub(super) fn finish_program_choose_hand_bottom_commander(
        &mut self,
        pending: PendingResolution,
        answer: &rv1::SubmitResolutionChoice,
        decision: rv1::ResolutionChoiceDecision,
    ) -> Result<RuledEventBatch, EngineError> {
        let (stack, choose, selected_branch, branch_id, object_id, generation) =
            match &pending.continuation {
                ResolutionContinuation::ProgramChoose { stack, choose } => match &choose.stage {
                    PendingProgramChooseStage::ChoosingCommanderDestination {
                        selected_branch,
                        branch_id,
                        object_id,
                        zone_change_generation,
                    } => (
                        stack.clone(),
                        choose.clone(),
                        *selected_branch,
                        branch_id.clone(),
                        *object_id,
                        *zone_change_generation,
                    ),
                    _ => {
                        self.state.pending_resolution = Some(pending);
                        return Err(EngineError::Illegal(
                            "program Choose is not awaiting a Commander destination",
                        ));
                    }
                },
                _ => {
                    self.state.pending_resolution = Some(pending);
                    return Err(EngineError::Illegal(
                        "program Choose Commander continuation missing",
                    ));
                }
            };
        let correct_shape = decision == rv1::ResolutionChoiceDecision::SelectBranch
            && answer.selected_branch_index <= 1
            && answer.chosen_object_ids.is_empty()
            && answer.chosen_player_ids.is_empty()
            && answer.payment.is_none()
            && answer.restricted_mana.is_empty()
            && answer.cast_spell.is_none()
            && answer.spell_cast_announcement.is_none()
            && answer.chosen_combat_defender.is_none();
        if !correct_shape
            || !self.hand_bottom_commander_is_current(
                pending.deciding_player,
                object_id,
                generation,
            )
        {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal(
                "invalid or stale hand-bottom Commander choice",
            ));
        }
        let branch = match self.program_choose_selected_branch(
            &stack,
            &choose.occurrence,
            choose.original_chooser,
            selected_branch,
        ) {
            Ok(branch)
                if branch.branch_id == branch_id
                    && branch.cost == ResolutionCost::PutHandCardOnLibraryBottom =>
            {
                branch
            }
            Ok(_) => {
                self.state.pending_resolution = Some(pending);
                return Err(EngineError::Illegal("program Choose branch became stale"));
            }
            Err(error) => {
                self.state.pending_resolution = Some(pending);
                return Err(error);
            }
        };
        let _ = branch;
        self.finish_program_choose_hand_bottom_payment(
            pending,
            stack,
            choose,
            branch_id,
            rv1::CostObjectRef {
                object_id,
                zone_change_generation: generation,
            },
            if answer.selected_branch_index == 0 {
                Zone::Command
            } else {
                Zone::Library
            },
        )
    }

    pub(super) fn select_program_choose(
        &mut self,
        mut pending: PendingResolution,
        answer: &rv1::SubmitResolutionChoice,
        decision: rv1::ResolutionChoiceDecision,
    ) -> Result<RuledEventBatch, EngineError> {
        let invalid_shape = !answer.chosen_object_ids.is_empty()
            || !answer.chosen_player_ids.is_empty()
            || answer.cast_spell.is_some()
            || answer.chosen_combat_defender.is_some()
            || answer.payment.is_some()
            || !answer.restricted_mana.is_empty()
            || answer.spell_cast_announcement.is_some();
        if invalid_shape {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal(
                "program Choose selection contains unrelated fields",
            ));
        }
        if matches!(
            pending.continuation,
            ResolutionContinuation::ProgramChoose {
                choose: PendingProgramChoose {
                    stage: PendingProgramChooseStage::ChoosingDelegate { .. },
                    ..
                },
                ..
            }
        ) {
            return self.select_program_choose_delegate(pending, answer, decision);
        }
        if matches!(
            pending.continuation,
            ResolutionContinuation::ProgramChoose {
                choose: PendingProgramChoose {
                    stage: PendingProgramChooseStage::ChoosingCommanderDestination { .. },
                    ..
                },
                ..
            }
        ) {
            return self.finish_program_choose_hand_bottom_commander(pending, answer, decision);
        }
        let (stack, choose_state) = match &pending.continuation {
            ResolutionContinuation::ProgramChoose { stack, choose }
                if matches!(choose.stage, PendingProgramChooseStage::Selecting) =>
            {
                (stack.clone(), choose.clone())
            }
            _ => {
                self.state.pending_resolution = Some(pending);
                return Err(EngineError::Illegal("program Choose continuation missing"));
            }
        };
        if decision == rv1::ResolutionChoiceDecision::Decline {
            if !choose_state.optional {
                self.state.pending_resolution = Some(pending);
                return Err(EngineError::Illegal("program Choose is not optional"));
            }
            let mut stack = stack;
            self.enter_program_choose_body(&mut stack, &choose_state.occurrence, None)?;
            return self.complete_parked_stack_with_previous(
                stack.clone(),
                stack.previous_result.clone(),
                vec![ev_log(format!(
                    "P{} declines the optional resolution choice.",
                    pending.deciding_player
                ))],
            );
        }
        if decision != rv1::ResolutionChoiceDecision::SelectBranch {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal(
                "program Choose requires a branch selection or decline",
            ));
        }
        let branch_index = answer.selected_branch_index as usize;
        let branch = match self.program_choose_selected_branch(
            &stack,
            &choose_state.occurrence,
            choose_state.original_chooser,
            branch_index,
        ) {
            Ok(branch) => branch,
            Err(error) => {
                self.state.pending_resolution = Some(pending);
                return Err(error);
            }
        };
        let label = branch.fallback_label();
        let mut events = vec![ev_log(format!(
            "P{} chooses: {label}.",
            pending.deciding_player
        ))];
        let waterbend = matches!(&branch.cost, ResolutionCost::Waterbend(_));
        match branch.cost {
            ResolutionCost::None => {
                let mut stack = stack;
                self.enter_program_choose_body(
                    &mut stack,
                    &choose_state.occurrence,
                    Some(&branch.branch_id),
                )?;
                self.complete_parked_stack_with_previous(
                    stack.clone(),
                    stack.previous_result.clone(),
                    events,
                )
            }
            ResolutionCost::Mana(cost) | ResolutionCost::Waterbend(cost) => {
                pending.presentation.choice_kind = rv1::ChoiceKind::ManaPayment;
                pending.presentation.min = 0;
                pending.presentation.max = 1;
                pending.presentation.prompt = format!("Pay {cost}?");
                let mut payment = PendingManaPayment::from_cost(
                    0,
                    cost,
                    self.state.undoable_mana_abilities.len(),
                );
                payment.waterbend = waterbend;
                let ResolutionContinuation::ProgramChoose { choose, .. } =
                    &mut pending.continuation
                else {
                    unreachable!("validated program Choose continuation")
                };
                choose.stage = PendingProgramChooseStage::PayingMana {
                    selected_branch: branch_index,
                    branch_id: branch.branch_id,
                    payment,
                };
                self.state.pending_resolution = Some(pending);
                events.push(
                    self.resolution_payment_choice_event()
                        .expect("program Choose payment remains parked"),
                );
                Ok(finish_with_events(self, events))
            }
            ResolutionCost::PutHandCardOnLibraryBottom => {
                let payer = choose_state.original_chooser;
                let candidates = self.resolution_cost_candidates(
                    payer,
                    stack.item.source_permanent_id.unwrap_or(stack.item.id),
                    stack.item.source_zone_change,
                    &branch.cost,
                );
                if candidates.is_empty() {
                    self.state.pending_resolution = Some(pending);
                    return Err(EngineError::Illegal(
                        "that program Choose branch no longer has a hand card to pay with",
                    ));
                }
                pending.deciding_player = payer;
                pending.presentation.choice_kind = rv1::ChoiceKind::HandCards;
                pending.presentation.candidates = candidates.clone();
                pending.presentation.min = 0;
                pending.presentation.max = 1;
                pending.presentation.prompt =
                    "Choose a card to put on the bottom of your library, or decline.".into();
                let candidate_generations = self.hand_bottom_candidate_generations(&candidates);
                let ResolutionContinuation::ProgramChoose { choose, .. } =
                    &mut pending.continuation
                else {
                    unreachable!("validated program Choose continuation")
                };
                choose.stage = PendingProgramChooseStage::PayingHandCard {
                    selected_branch: branch_index,
                    branch_id: branch.branch_id,
                    candidate_generations,
                };
                let event = self.hand_bottom_cost_choice_event(&pending, &candidates);
                self.state.pending_resolution = Some(pending);
                events.push(event);
                Ok(finish_with_events(self, events))
            }
            ResolutionCost::Blight { count } => {
                let payer = choose_state.original_chooser;
                let candidates = self.resolution_cost_candidates(
                    payer,
                    stack.item.source_permanent_id.unwrap_or(stack.item.id),
                    stack.item.source_zone_change,
                    &ResolutionCost::Blight { count },
                );
                if candidates.is_empty() {
                    self.state.pending_resolution = Some(pending);
                    return Err(EngineError::Illegal(
                        "that program Choose branch no longer has a creature to blight",
                    ));
                }
                pending.deciding_player = payer;
                pending.presentation.choice_kind = rv1::ChoiceKind::CostObjects;
                pending.presentation.candidates = candidates.clone();
                pending.presentation.min = 1;
                pending.presentation.max = 1;
                pending.presentation.prompt =
                    format!("Blight {count}: choose one creature you control.");
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
                    .collect();
                let ResolutionContinuation::ProgramChoose { choose, .. } =
                    &mut pending.continuation
                else {
                    unreachable!("validated program Choose continuation")
                };
                choose.stage = PendingProgramChooseStage::PayingBlight {
                    selected_branch: branch_index,
                    branch_id: branch.branch_id,
                    count,
                    candidate_generations,
                };
                let event =
                    self.program_choose_blight_cost_choice_event(&pending, count, &candidates);
                self.state.pending_resolution = Some(pending);
                events.push(event);
                Ok(finish_with_events(self, events))
            }
            cost @ ResolutionCost::SacrificePermanent { .. } => {
                let payer = choose_state.original_chooser;
                let candidates = self.resolution_cost_candidates(
                    payer,
                    stack.item.source_permanent_id.unwrap_or(stack.item.id),
                    stack.item.source_zone_change,
                    &cost,
                );
                if candidates.is_empty() {
                    self.state.pending_resolution = Some(pending);
                    return Err(EngineError::Illegal(
                        "that program Choose branch no longer has a permanent to sacrifice",
                    ));
                }
                pending.deciding_player = payer;
                pending.presentation.choice_kind = rv1::ChoiceKind::CostObjects;
                pending.presentation.candidates = candidates.clone();
                pending.presentation.min = 0;
                pending.presentation.max = 1;
                pending.presentation.prompt = "Choose a permanent to sacrifice, or decline.".into();
                let candidate_generations = self.hand_bottom_candidate_generations(&candidates);
                let ResolutionContinuation::ProgramChoose { choose, .. } =
                    &mut pending.continuation
                else {
                    unreachable!("validated program Choose continuation")
                };
                choose.stage = PendingProgramChooseStage::PayingSacrificePermanent {
                    selected_branch: branch_index,
                    branch_id: branch.branch_id,
                    candidate_generations,
                };
                let event = self.program_choose_sacrifice_cost_choice_event(&pending, &candidates);
                self.state.pending_resolution = Some(pending);
                events.push(event);
                Ok(finish_with_events(self, events))
            }
            _ => {
                self.state.pending_resolution = Some(pending);
                Err(EngineError::ProgramInvariant(
                    "compiled program Choose has an unsupported resolution cost",
                ))
            }
        }
    }

    fn select_program_choose_delegate(
        &mut self,
        mut pending: PendingResolution,
        answer: &rv1::SubmitResolutionChoice,
        decision: rv1::ResolutionChoiceDecision,
    ) -> Result<RuledEventBatch, EngineError> {
        if decision != rv1::ResolutionChoiceDecision::SelectBranch {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal(
                "a departed chooser's replacement must be selected",
            ));
        }
        let (controller, original_chooser, candidates) = match &pending.continuation {
            ResolutionContinuation::ProgramChoose {
                stack,
                choose:
                    PendingProgramChoose {
                        original_chooser,
                        stage: PendingProgramChooseStage::ChoosingDelegate { candidates },
                        ..
                    },
            } => (stack.item.controller, *original_chooser, candidates.clone()),
            _ => {
                self.state.pending_resolution = Some(pending);
                return Err(EngineError::Illegal(
                    "program Choose delegate continuation missing",
                ));
            }
        };
        let delegate = candidates
            .get(answer.selected_branch_index as usize)
            .copied()
            .flatten();
        let eligible = resolution::resolution_choice_delegate_candidates(
            &self.state,
            controller,
            original_chooser,
        );
        let Some(delegate) = delegate.filter(|delegate| eligible.contains(delegate)) else {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal(
                "that replacement chooser is no longer available",
            ));
        };
        if self
            .state
            .player_idx(delegate)
            .is_none_or(|index| self.state.players[index].has_lost)
        {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal(
                "that replacement chooser is no longer available",
            ));
        }
        let ResolutionContinuation::ProgramChoose { choose, .. } = &mut pending.continuation else {
            unreachable!("validated program Choose delegate continuation")
        };
        choose.stage = PendingProgramChooseStage::Selecting;
        choose.delegate_selected = true;
        pending.deciding_player = delegate;
        pending.presentation.choice_kind = rv1::ChoiceKind::ResolutionBranch;
        pending.presentation.min = u32::from(!choose.optional);
        pending.presentation.max = 1;
        pending.presentation.prompt = if choose.optional {
            "Choose a resolution option, or decline.".into()
        } else {
            "Choose a resolution option.".into()
        };
        let mut events = vec![ev_log(format!(
            "P{controller} chooses P{delegate} to make P{original_chooser}'s resolution choice."
        ))];
        if !self.program_choose_choice_has_selectable_option(&pending) {
            let stack = pending
                .continuation
                .stack()
                .cloned()
                .ok_or(EngineError::Illegal(
                    "program Choose stack continuation missing",
                ))?;
            let choose = match &pending.continuation {
                ResolutionContinuation::ProgramChoose { choose, .. } => choose.clone(),
                _ => unreachable!(),
            };
            let mut stack = stack;
            self.enter_program_choose_body(&mut stack, &choose.occurrence, None)?;
            return self.complete_parked_stack_with_previous(
                stack.clone(),
                stack.previous_result.clone(),
                {
                    events.push(ev_log(
                        "No cost-free resolution option remains for the departed chooser.".into(),
                    ));
                    events
                },
            );
        }
        self.state.pending_resolution = Some(pending);
        events.push(
            self.program_choose_choice_event()
                .expect("delegated program Choose remains parked"),
        );
        Ok(finish_with_events(self, events))
    }

    fn program_choose_choice_has_selectable_option(&mut self, pending: &PendingResolution) -> bool {
        self.state.pending_resolution = Some(pending.clone());
        let has_selectable = self
            .program_choose_choice_event()
            .and_then(|event| match event.ev {
                Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(choice)) => Some(
                    choice
                        .resolution_branches
                        .iter()
                        .any(|branch| branch.selectable),
                ),
                _ => None,
            })
            .unwrap_or(false);
        self.state.pending_resolution = None;
        has_selectable
    }

    pub(super) fn select_resolution_branch(
        &mut self,
        mut pending: PendingResolution,
        answer: &rv1::SubmitResolutionChoice,
        decision: rv1::ResolutionChoiceDecision,
    ) -> Result<RuledEventBatch, EngineError> {
        if !answer.chosen_object_ids.is_empty() || !answer.chosen_player_ids.is_empty() {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal(
                "resolution branch selection cannot include object or player ids",
            ));
        }
        if matches!(
            pending.continuation,
            ResolutionContinuation::AuthoredBranch {
                branch: PendingResolutionBranch {
                    stage: PendingResolutionBranchStage::ChoosingDelegate { .. },
                    ..
                },
                ..
            }
        ) {
            return self.select_resolution_branch_delegate(pending, answer, decision);
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
        if !resolution::resolution_branch_is_selectable(
            self,
            &stack.item,
            &stack.previous_result,
            pending.deciding_player,
            branch_state.original_chooser,
            &branch,
            true,
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
            ResolutionCost::PutHandCardOnLibraryBottom => {
                self.state.pending_resolution = Some(pending);
                return Err(EngineError::ProgramInvariant(
                    "hand-bottom costs require a typed Program Choose branch",
                ));
            }
            ResolutionCost::DiscardCard { .. }
            | ResolutionCost::ExileGraveyardCard { .. }
            | ResolutionCost::SacrificePermanent { .. }
            | ResolutionCost::Blight { .. }
            | ResolutionCost::TapPermanents { .. } => {
                let is_hand_card = matches!(branch.cost, ResolutionCost::DiscardCard { .. });
                let is_graveyard_exile =
                    matches!(branch.cost, ResolutionCost::ExileGraveyardCard { .. });
                let is_tap = matches!(branch.cost, ResolutionCost::TapPermanents { .. });
                let count = match branch.cost {
                    ResolutionCost::TapPermanents { count, .. } => count,
                    _ => 1,
                };
                pending.presentation.choice_kind = if is_hand_card {
                    rv1::ChoiceKind::HandCards
                } else if is_graveyard_exile {
                    rv1::ChoiceKind::GraveyardCards
                } else if is_tap
                    || matches!(
                        branch.cost,
                        ResolutionCost::Blight { .. } | ResolutionCost::SacrificePermanent { .. }
                    )
                {
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
                } else if matches!(branch.cost, ResolutionCost::DiscardCard { .. }) {
                    "Choose a card to discard, or decline.".into()
                } else if is_graveyard_exile {
                    "Choose a card from your graveyard to exile, or decline.".into()
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
                            resolution_prompt_id: 0,
                            accepted_permanent_selections: Vec::new(),
                            variable_mana_contribution: false,
                            candidate_token_identities: Vec::new(),
                            candidate_player_ids: Vec::new(),
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
                            candidate_source_zones: if is_graveyard_exile {
                                vec![
                                    rv1::ChoiceCandidateSourceZone::Graveyard as i32;
                                    candidates.len()
                                ]
                            } else {
                                Vec::new()
                            },
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

    fn select_resolution_branch_delegate(
        &mut self,
        mut pending: PendingResolution,
        answer: &rv1::SubmitResolutionChoice,
        decision: rv1::ResolutionChoiceDecision,
    ) -> Result<RuledEventBatch, EngineError> {
        if decision != rv1::ResolutionChoiceDecision::SelectBranch {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal(
                "a departed chooser's replacement must be selected",
            ));
        }
        let (controller, original_chooser, candidates) = match &pending.continuation {
            ResolutionContinuation::AuthoredBranch {
                stack,
                branch:
                    PendingResolutionBranch {
                        original_chooser,
                        stage: PendingResolutionBranchStage::ChoosingDelegate { candidates },
                        ..
                    },
            } => (stack.item.controller, *original_chooser, candidates.clone()),
            _ => {
                self.state.pending_resolution = Some(pending);
                return Err(EngineError::Illegal(
                    "resolution branch delegate continuation missing",
                ));
            }
        };
        let delegate = candidates
            .get(answer.selected_branch_index as usize)
            .copied()
            .flatten();
        let eligible = resolution::resolution_choice_delegate_candidates(
            &self.state,
            controller,
            original_chooser,
        );
        let Some(delegate) = delegate.filter(|delegate| eligible.contains(delegate)) else {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal(
                "that replacement chooser is no longer available",
            ));
        };
        let Some(index) = self.state.player_idx(delegate) else {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal(
                "that replacement chooser is no longer available",
            ));
        };
        if self.state.players[index].has_lost {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal(
                "that replacement chooser is no longer available",
            ));
        }
        let ResolutionContinuation::AuthoredBranch { branch, .. } = &mut pending.continuation
        else {
            unreachable!("validated authored branch continuation")
        };
        branch.stage = PendingResolutionBranchStage::Selecting;
        pending.deciding_player = delegate;
        pending.presentation.min = u32::from(!branch.optional);
        pending.presentation.max = 1;
        pending.presentation.prompt = if branch.optional {
            "Choose a resolution option, or decline.".into()
        } else {
            "Choose a resolution option.".into()
        };
        self.state.pending_resolution = Some(pending);
        let assigned_log = ev_log(format!(
            "P{controller} chooses P{delegate} to make P{original_chooser}'s resolution choice."
        ));
        if !resolution::authored_resolution_branch_has_selectable_option(self) {
            let pending = self
                .state
                .pending_resolution
                .take()
                .expect("delegated branch choice remains parked");
            let mut completed = self.complete_authored_branch_without_selection(
                pending,
                "No cost-free resolution branch remains for the departed chooser.".into(),
            )?;
            completed.events.insert(0, assigned_log);
            return Ok(completed);
        }
        let event = resolution::authored_resolution_branch_choice_event(self)
            .expect("delegated branch choice remains parked");
        Ok(finish_with_events(self, vec![assigned_log, event]))
    }

    fn complete_authored_branch_without_selection(
        &mut self,
        pending: PendingResolution,
        log: String,
    ) -> Result<RuledEventBatch, EngineError> {
        let stack = pending
            .continuation
            .stack()
            .cloned()
            .ok_or(EngineError::Illegal(
                "resolution branch stack continuation missing",
            ))?;
        let effect_index = stack
            .resume_effect_index
            .and_then(|next| next.checked_sub(1))
            .ok_or(EngineError::Illegal(
                "resolution branch effect index missing",
            ))?;
        let mut item = stack.item;
        item.resolution_branch_choices.insert(effect_index, None);
        self.complete_parked_resolution_with_previous(
            item,
            Some(effect_index),
            stack.previous_result,
            vec![ev_log(log)],
        )
    }

    fn complete_program_choose_otherwise(
        &mut self,
        pending: PendingResolution,
        log: String,
    ) -> Result<RuledEventBatch, EngineError> {
        let (stack, choose) = match &pending.continuation {
            ResolutionContinuation::ProgramChoose { stack, choose } => {
                (stack.clone(), choose.clone())
            }
            _ => {
                self.state.pending_resolution = Some(pending);
                return Err(EngineError::Illegal(
                    "program Choose stack continuation missing",
                ));
            }
        };
        self.complete_program_choose_otherwise_from_parts(stack, choose, log)
    }

    pub(in crate::engine) fn complete_program_choose_otherwise_from_parts(
        &mut self,
        mut stack: ParkedStackResolution,
        choose: PendingProgramChoose,
        log: String,
    ) -> Result<RuledEventBatch, EngineError> {
        self.enter_program_choose_body(&mut stack, &choose.occurrence, None)?;
        let previous_result = stack.previous_result.clone();
        self.complete_parked_stack_with_previous(stack, previous_result, vec![ev_log(log)])
    }

    /// Reconcile a program Choose occurrence when its chooser, selected delegate, or source
    /// controller leaves. The parked program frame remains authoritative: costed branches cannot
    /// be transferred, optional departed payers take Otherwise, and a noncost choice receives a
    /// fresh selector prompt before a live delegate sees the original menu.
    pub(in crate::engine) fn refresh_program_choose_departure(
        &mut self,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<(), EngineError> {
        let Some(mut pending) = self.state.pending_resolution.take() else {
            return Ok(());
        };
        if let ResolutionContinuation::CounterReplacement {
            stack: Some(stack),
            completion:
                crate::state::CounterPlacementCompletion::ProgramChooseBlight {
                    choose,
                    object_id,
                    zone_change_generation,
                },
        } = &pending.continuation
        {
            let stack = stack.clone();
            let choose = choose.clone();
            let payer_live = self
                .state
                .player_idx(choose.original_chooser)
                .is_some_and(|index| !self.state.players[index].has_lost);
            let replacement_chooser_live = self
                .state
                .player_idx(pending.deciding_player)
                .is_some_and(|index| !self.state.players[index].has_lost);
            let selected_creature_current = self
                .state
                .zone_change_generation
                .get(object_id)
                .copied()
                .unwrap_or(0)
                == *zone_change_generation
                && self.can_blight_creature(choose.original_chooser, *object_id);
            if payer_live && replacement_chooser_live && selected_creature_current {
                self.state.pending_resolution = Some(pending);
                return Ok(());
            }
            if matches!(
                self.state.pending_replacement_event,
                Some(super::super::replacement::PendingReplacementEvent::Counters(_))
            ) {
                self.state.pending_replacement_event = None;
            }
            let log = if !payer_live {
                "The departed payer cannot complete the program Choose Blight payment; Otherwise resolves."
            } else if !selected_creature_current {
                "The selected creature cannot receive the program Choose Blight payment; Otherwise resolves."
            } else {
                "The replacement chooser departed before the program Choose Blight payment; Otherwise resolves."
            };
            let completed =
                self.complete_program_choose_otherwise_from_parts(stack, choose, log.into())?;
            events.extend(completed.events);
            return Ok(());
        }
        let (stack, choose) = match &pending.continuation {
            ResolutionContinuation::ProgramChoose { stack, choose } => {
                (stack.clone(), choose.clone())
            }
            _ => {
                self.state.pending_resolution = Some(pending);
                return Ok(());
            }
        };
        let controller = stack.item.controller;
        let controller_live = self
            .state
            .player_idx(controller)
            .is_some_and(|index| !self.state.players[index].has_lost);
        let original_chooser_live = self
            .state
            .player_idx(choose.original_chooser)
            .is_some_and(|index| !self.state.players[index].has_lost);
        let current_chooser_live = self
            .state
            .player_idx(pending.deciding_player)
            .is_some_and(|index| !self.state.players[index].has_lost);
        let delegate_prompt = matches!(
            choose.stage,
            PendingProgramChooseStage::ChoosingDelegate { .. }
        );
        let payment_prompt = matches!(
            choose.stage,
            PendingProgramChooseStage::PayingMana { .. }
                | PendingProgramChooseStage::PayingHandCard { .. }
                | PendingProgramChooseStage::PayingBlight { .. }
                | PendingProgramChooseStage::PayingSacrificePermanent { .. }
                | PendingProgramChooseStage::ChoosingCommanderDestination { .. }
        );

        if let PendingProgramChooseStage::PayingSacrificePermanent {
            selected_branch,
            branch_id,
            candidate_generations: previous_candidates,
        } = &choose.stage
        {
            if !original_chooser_live {
                // Controller-only Program sacrifice payment has no delegate. The controlling
                // player left, so the owning resolution has already ended under CR 800.4a; do
                // not enter the paid body, Otherwise, or parent tail.
                return Ok(());
            }
            if !current_chooser_live || pending.deciding_player != choose.original_chooser {
                let completed = self.complete_program_choose_otherwise(
                    pending,
                    "The program Choose sacrifice payment cannot be delegated; Otherwise resolves."
                        .into(),
                )?;
                events.extend(completed.events);
                return Ok(());
            }
            if choose.original_chooser != controller {
                self.state.pending_resolution = Some(pending);
                return Err(EngineError::ProgramInvariant(
                    "Controller sacrifice payment lost its resolving controller",
                ));
            }
            let branch = match self.program_choose_branch_at_occurrence(
                &stack,
                &choose.occurrence,
                *selected_branch,
            ) {
                Ok(branch)
                    if branch.branch_id == *branch_id
                        && matches!(&branch.cost, ResolutionCost::SacrificePermanent { .. }) =>
                {
                    branch
                }
                Ok(_) => {
                    self.state.pending_resolution = Some(pending);
                    return Err(EngineError::ProgramInvariant(
                        "parked Program sacrifice branch changed identity",
                    ));
                }
                Err(error) => {
                    self.state.pending_resolution = Some(pending);
                    return Err(error);
                }
            };
            let candidates = self.resolution_cost_candidates(
                choose.original_chooser,
                stack.item.source_permanent_id.unwrap_or(stack.item.id),
                stack.item.source_zone_change,
                &branch.cost,
            );
            if candidates.is_empty() {
                let completed = self.complete_program_choose_otherwise_from_parts(
                    stack,
                    choose,
                    "No permanent remains to pay the program Choose sacrifice cost; Otherwise resolves."
                        .into(),
                )?;
                events.extend(completed.events);
                return Ok(());
            }
            let candidate_generations = self.hand_bottom_candidate_generations(&candidates);
            let candidates_changed = candidate_generations != *previous_candidates;
            let prompt_was_reset = pending.resolution_prompt_id.get() == 0;
            if candidates_changed {
                pending.presentation.candidates = candidates.clone();
                pending.resolution_prompt_semantics = None;
                let ResolutionContinuation::ProgramChoose { choose, .. } =
                    &mut pending.continuation
                else {
                    unreachable!("validated ProgramChoose continuation")
                };
                let PendingProgramChooseStage::PayingSacrificePermanent {
                    candidate_generations: saved,
                    ..
                } = &mut choose.stage
                else {
                    unreachable!("validated Program sacrifice payment stage")
                };
                *saved = candidate_generations;
            }
            self.state.pending_resolution = Some(pending);
            if candidates_changed || prompt_was_reset {
                let pending = self
                    .state
                    .pending_resolution
                    .as_ref()
                    .expect("refreshed Program sacrifice payment remains parked");
                events.push(self.program_choose_sacrifice_cost_choice_event(pending, &candidates));
            }
            return Ok(());
        }

        if let PendingProgramChooseStage::PayingBlight {
            selected_branch,
            branch_id,
            count,
            candidate_generations: previous_candidates,
        } = &choose.stage
        {
            if !original_chooser_live || !current_chooser_live {
                let completed = self.complete_program_choose_otherwise(
                    pending,
                    "The departed payer cannot complete the program Choose Blight payment; Otherwise resolves."
                        .into(),
                )?;
                events.extend(completed.events);
                return Ok(());
            }
            if pending.deciding_player != choose.original_chooser {
                let completed = self.complete_program_choose_otherwise(
                    pending,
                    "The program Choose Blight payment cannot be delegated; Otherwise resolves."
                        .into(),
                )?;
                events.extend(completed.events);
                return Ok(());
            }
            let candidates = self.resolution_cost_candidates(
                choose.original_chooser,
                stack.item.source_permanent_id.unwrap_or(stack.item.id),
                stack.item.source_zone_change,
                &ResolutionCost::Blight { count: *count },
            );
            if candidates.is_empty() {
                let completed = self.complete_program_choose_otherwise(
                    pending,
                    "No creature remains to pay the program Choose Blight cost; Otherwise resolves."
                        .into(),
                )?;
                events.extend(completed.events);
                return Ok(());
            }
            let candidate_generations = self.hand_bottom_candidate_generations(&candidates);
            let candidates_changed = candidate_generations != *previous_candidates;
            let prompt_was_reset = pending.resolution_prompt_id.get() == 0;
            if candidates_changed {
                pending.presentation.candidates = candidates.clone();
                pending.resolution_prompt_semantics = None;
                let ResolutionContinuation::ProgramChoose { choose, .. } =
                    &mut pending.continuation
                else {
                    unreachable!("validated ProgramChoose continuation")
                };
                let PendingProgramChooseStage::PayingBlight {
                    candidate_generations: saved,
                    ..
                } = &mut choose.stage
                else {
                    unreachable!("validated Blight payment stage")
                };
                *saved = candidate_generations;
            }
            self.state.pending_resolution = Some(pending);
            if candidates_changed || prompt_was_reset {
                let pending = self
                    .state
                    .pending_resolution
                    .as_ref()
                    .expect("refreshed program Blight payment remains parked");
                events.push(self.program_choose_blight_cost_choice_event(
                    pending,
                    *count,
                    &candidates,
                ));
            }
            let _ = (selected_branch, branch_id);
            return Ok(());
        }

        if payment_prompt && original_chooser_live && current_chooser_live {
            self.state.pending_resolution = Some(pending);
            return Ok(());
        }
        if payment_prompt {
            let log = if payment_prompt {
                "The departed payer cannot complete the program Choose payment; Otherwise resolves."
            } else {
                "The departed chooser cannot complete the program Choose payment; Otherwise resolves."
            };
            let completed = self.complete_program_choose_otherwise(pending, log.into())?;
            events.extend(completed.events);
            return Ok(());
        }

        if !delegate_prompt && original_chooser_live && current_chooser_live {
            self.state.pending_resolution = Some(pending);
            return Ok(());
        }
        if !delegate_prompt && choose.delegate_selected && current_chooser_live {
            // The chosen live delegate is still entitled to answer the original menu once.
            self.state.pending_resolution = Some(pending);
            return Ok(());
        }

        let selector = if controller_live {
            Some(controller)
        } else {
            self.state
                .player_idx(controller)
                .and_then(|controller_index| self.state.next_in_game_player_idx(controller_index))
                .map(|index| self.state.players[index].id)
                .or_else(|| {
                    self.state
                        .players
                        .iter()
                        .find(|player| !player.has_lost)
                        .map(|player| player.id)
                })
        };
        let eligible = resolution::resolution_choice_delegate_candidates(
            &self.state,
            controller,
            choose.original_chooser,
        );
        if eligible.is_empty() {
            let completed = self.complete_program_choose_otherwise(
                pending,
                "No live player can make the departed chooser's resolution choice.".into(),
            )?;
            events.extend(completed.events);
            return Ok(());
        }

        // Regenerate the Selecting view without losing candidate indices. If no cost-free branch
        // remains after the chooser departed, the selected alternative is the only legal route.
        // Probe the original menu on a temporary pending value. The live continuation may be
        // an unchanged delegate prompt; mutating it to Selecting before the no-op check would
        // silently lose that stage and make the next answer validate as a branch selection.
        let mut probe = pending.clone();
        let ResolutionContinuation::ProgramChoose {
            choose: probe_choose,
            ..
        } = &mut probe.continuation
        else {
            unreachable!("validated program Choose continuation")
        };
        probe_choose.stage = PendingProgramChooseStage::Selecting;
        probe_choose.delegate_selected = false;
        if !self.program_choose_choice_has_selectable_option(&probe) {
            let completed = self.complete_program_choose_otherwise(
                pending,
                "No cost-free resolution option remains for the departed chooser.".into(),
            )?;
            events.extend(completed.events);
            return Ok(());
        }

        let Some(selector) = selector else {
            self.state.pending_resolution = Some(pending);
            return Ok(());
        };
        let mut candidate_slots = match &choose.stage {
            PendingProgramChooseStage::ChoosingDelegate { candidates } => candidates.clone(),
            _ => Vec::new(),
        };
        for slot in &mut candidate_slots {
            if slot.is_some_and(|candidate| !eligible.contains(&candidate)) {
                *slot = None;
            }
        }
        for candidate in eligible.iter().copied() {
            if !candidate_slots.contains(&Some(candidate)) {
                candidate_slots.push(Some(candidate));
            }
        }

        let same_prompt = delegate_prompt
            && current_chooser_live
            && pending.deciding_player == selector
            && matches!(
                &choose.stage,
                PendingProgramChooseStage::ChoosingDelegate { candidates }
                    if *candidates == candidate_slots
            );
        if same_prompt {
            self.state.pending_resolution = Some(pending);
            return Ok(());
        }

        pending.deciding_player = selector;
        pending.presentation.choice_kind = rv1::ChoiceKind::ResolutionBranch;
        pending.presentation.min = 1;
        pending.presentation.max = 1;
        pending.presentation.prompt = format!(
            "P{selector}: choose a player to make P{}'s resolution choice.",
            choose.original_chooser
        );
        pending.resolution_prompt_semantics = None;
        let ResolutionContinuation::ProgramChoose { choose, .. } = &mut pending.continuation else {
            unreachable!("validated program Choose continuation")
        };
        choose.stage = PendingProgramChooseStage::ChoosingDelegate {
            candidates: candidate_slots,
        };
        self.state.pending_resolution = Some(pending);
        events.push(ev_log(format!(
            "P{selector} must choose a replacement for the departed resolution chooser."
        )));
        events.push(
            self.program_choose_choice_event()
                .expect("program Choose delegate prompt remains parked"),
        );
        Ok(())
    }

    /// Reassign an authored branch choice when its current decider leaves. CR 800.4g directs the
    /// object controller to select another player; the candidate vector preserves old indices so
    /// a stale answer cannot silently select a different delegate.
    pub(in crate::engine) fn refresh_authored_branch_departure(
        &mut self,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<(), EngineError> {
        let Some(mut pending) = self.state.pending_resolution.take() else {
            return Ok(());
        };
        let (stack, branch) = match &pending.continuation {
            ResolutionContinuation::AuthoredBranch { stack, branch } => {
                (stack.clone(), branch.clone())
            }
            _ => {
                self.state.pending_resolution = Some(pending);
                return Ok(());
            }
        };
        let controller = stack.item.controller;
        let controller_live = self
            .state
            .player_idx(controller)
            .is_some_and(|index| !self.state.players[index].has_lost);
        if !controller_live {
            // A departed object controller's outer resolution has already been detached by
            // `detach_departed_outer_resolution`; retain any state that was not detached.
            self.state.pending_resolution = Some(pending);
            return Ok(());
        }
        let chooser_live = self
            .state
            .player_idx(pending.deciding_player)
            .is_some_and(|index| !self.state.players[index].has_lost);
        let original_chooser_live = self
            .state
            .player_idx(branch.original_chooser)
            .is_some_and(|index| !self.state.players[index].has_lost);
        let is_delegate_prompt = matches!(
            branch.stage,
            PendingResolutionBranchStage::ChoosingDelegate { .. }
        );
        let is_payment_prompt = matches!(
            branch.stage,
            PendingResolutionBranchStage::PayingMana { .. }
                | PendingResolutionBranchStage::PayingObjects { .. }
        );
        if is_payment_prompt {
            if chooser_live && original_chooser_live {
                self.state.pending_resolution = Some(pending);
                return Ok(());
            }
            let completed = self.complete_authored_branch_without_selection(
                pending,
                "The departed chooser cannot complete the selected branch payment.".into(),
            )?;
            events.extend(completed.events);
            return Ok(());
        }
        if !is_delegate_prompt && chooser_live && original_chooser_live {
            self.state.pending_resolution = Some(pending);
            return Ok(());
        }

        let eligible = resolution::resolution_choice_delegate_candidates(
            &self.state,
            controller,
            branch.original_chooser,
        );
        if !original_chooser_live {
            let has_selectable_option = eligible.iter().any(|delegate| {
                branch.branches.iter().any(|option| {
                    resolution::resolution_branch_is_selectable(
                        self,
                        &stack.item,
                        &stack.previous_result,
                        *delegate,
                        branch.original_chooser,
                        option,
                        true,
                    )
                })
            });
            if !has_selectable_option {
                let completed = self.complete_authored_branch_without_selection(
                    pending,
                    "No cost-free resolution branch remains for the departed chooser.".into(),
                )?;
                events.extend(completed.events);
                return Ok(());
            }
        }
        let mut candidate_slots = match branch.stage {
            PendingResolutionBranchStage::ChoosingDelegate { candidates } => candidates,
            _ => Vec::new(),
        };
        for slot in &mut candidate_slots {
            if slot.is_some_and(|candidate| !eligible.contains(&candidate)) {
                *slot = None;
            }
        }
        for candidate in eligible.iter().copied() {
            if !candidate_slots.contains(&Some(candidate)) {
                candidate_slots.push(Some(candidate));
            }
        }

        match eligible.as_slice() {
            [] => {
                let completed = self.complete_authored_branch_without_selection(
                    pending,
                    "No other player can make the departed player's resolution choice.".into(),
                )?;
                events.extend(completed.events);
                Ok(())
            }
            [_] if is_delegate_prompt => {
                // Keep the controller's choice parked when a candidate disappears mid-prompt.
                // The surviving option stays at its original index; stale clients cannot turn a
                // removed index into a different player's selection.
                pending.deciding_player = controller;
                pending.presentation.min = 1;
                pending.presentation.max = 1;
                pending.presentation.prompt = format!(
                    "P{controller}: choose a player to make P{}'s resolution choice.",
                    branch.original_chooser
                );
                let ResolutionContinuation::AuthoredBranch { branch, .. } =
                    &mut pending.continuation
                else {
                    unreachable!("validated authored branch continuation")
                };
                branch.stage = PendingResolutionBranchStage::ChoosingDelegate {
                    candidates: candidate_slots,
                };
                self.state.pending_resolution = Some(pending);
                events.push(ev_log(format!(
                    "P{controller}'s replacement chooser options have changed."
                )));
                events.push(
                    resolution::authored_resolution_branch_choice_event(self)
                        .expect("replacement chooser selection remains parked"),
                );
                Ok(())
            }
            [delegate] => {
                pending.deciding_player = *delegate;
                pending.presentation.min = u32::from(!branch.optional);
                pending.presentation.max = 1;
                pending.presentation.prompt = if branch.optional {
                    "Choose a resolution option, or decline.".into()
                } else {
                    "Choose a resolution option.".into()
                };
                let ResolutionContinuation::AuthoredBranch { branch, .. } =
                    &mut pending.continuation
                else {
                    unreachable!("validated authored branch continuation")
                };
                branch.stage = PendingResolutionBranchStage::Selecting;
                self.state.pending_resolution = Some(pending);
                events.push(ev_log(format!(
                    "P{controller} assigns P{delegate} to make the departed player's resolution choice."
                )));
                events.push(
                    resolution::authored_resolution_branch_choice_event(self)
                        .expect("reassigned branch choice remains parked"),
                );
                Ok(())
            }
            _ => {
                pending.deciding_player = controller;
                pending.presentation.min = 1;
                pending.presentation.max = 1;
                pending.presentation.prompt = format!(
                    "P{controller}: choose a player to make P{}'s resolution choice.",
                    branch.original_chooser
                );
                let ResolutionContinuation::AuthoredBranch { branch, .. } =
                    &mut pending.continuation
                else {
                    unreachable!("validated authored branch continuation")
                };
                branch.stage = PendingResolutionBranchStage::ChoosingDelegate {
                    candidates: candidate_slots,
                };
                self.state.pending_resolution = Some(pending);
                events.push(ev_log(format!(
                    "P{controller} must choose a replacement for the departed resolution chooser."
                )));
                events.push(
                    resolution::authored_resolution_branch_choice_event(self)
                        .expect("replacement chooser selection remains parked"),
                );
                Ok(())
            }
        }
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
            .cloned()
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
        let mut ev = receipt.move_events;
        self.fire_resolution_cost_triggers(receipt.trigger_events, receipt.sacrificed, &mut ev);
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
                    PaidCardCost::Exile { card_name, .. } => ("exiles", card_name),
                    PaidCardCost::PutOnLibraryBottom { .. } => {
                        ev.push(ev_log(format!(
                            "P{} puts a card on the bottom of their library.",
                            pending.deciding_player
                        )));
                        continue;
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
