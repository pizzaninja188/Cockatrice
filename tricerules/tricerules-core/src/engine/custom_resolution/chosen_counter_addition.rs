use super::*;

impl GameEngine {
    fn complete_chosen_counter_addition_without_placement(
        &mut self,
        stack: ParkedStackResolution,
        events: Vec<rv1::RuledEvent>,
    ) -> Result<RuledEventBatch, EngineError> {
        self.complete_parked_resolution_with_previous(
            stack.item,
            stack.resume_effect_index,
            stack.previous_result,
            events,
        )
    }

    fn selection_is_valid_for_departed_controller(
        &self,
        controller: PlayerId,
        chooser: PlayerId,
    ) -> bool {
        if self
            .state
            .player_idx(controller)
            .is_some_and(|index| !self.state.players[index].has_lost)
        {
            chooser == controller
        } else {
            super::resolution::resolution_choice_delegate_candidates(
                &self.state,
                controller,
                controller,
            )
            .contains(&chooser)
        }
    }

    fn select_chosen_counter_addition_delegate(
        &mut self,
        mut pending: PendingResolution,
        answer: &rv1::SubmitResolutionChoice,
        decision: rv1::ResolutionChoiceDecision,
    ) -> Result<RuledEventBatch, EngineError> {
        if decision != rv1::ResolutionChoiceDecision::SelectBranch
            || !answer.chosen_object_ids.is_empty()
            || !answer.chosen_player_ids.is_empty()
            || answer.cast_spell.is_some()
            || answer.spell_cast_announcement.is_some()
            || answer.chosen_combat_defender.is_some()
            || answer.payment.is_some()
            || !answer.restricted_mana.is_empty()
        {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal(
                "chosen-counter delegate choice requires one offered branch index",
            ));
        }
        let (stack, candidates) = match &pending.continuation {
            ResolutionContinuation::ChosenCounterAddition {
                stack,
                counter_addition:
                    crate::state::PendingChosenCounterAddition {
                        stage:
                            crate::state::PendingChosenCounterAdditionStage::ChoosingDelegate {
                                candidates,
                            },
                        ..
                    },
            } => (stack.clone(), candidates.clone()),
            _ => {
                self.state.pending_resolution = Some(pending);
                return Err(EngineError::Illegal(
                    "chosen-counter delegate continuation is missing",
                ));
            }
        };
        let Some(delegate) = super::resolution::resolution_choice_delegate_at_index(
            &self.state,
            stack.item.controller,
            stack.item.controller,
            &candidates,
            answer.selected_branch_index,
        ) else {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal(
                "that replacement chooser is no longer available",
            ));
        };
        let selector = pending.deciding_player;
        let Some(current_kinds) = super::resolution::chosen_counter_addition_target_counter_kinds(
            self,
            match &pending.continuation {
                ResolutionContinuation::ChosenCounterAddition {
                    counter_addition, ..
                } => &counter_addition.target,
                _ => unreachable!("validated chosen-counter continuation"),
            },
        ) else {
            return self.complete_chosen_counter_addition_without_placement(stack, Vec::new());
        };
        if current_kinds.is_empty() {
            return self.complete_chosen_counter_addition_without_placement(stack, Vec::new());
        }
        if let ResolutionContinuation::ChosenCounterAddition {
            counter_addition, ..
        } = &mut pending.continuation
        {
            counter_addition.offered_counter_kinds = current_kinds;
            counter_addition.stage = crate::state::PendingChosenCounterAdditionStage::Selecting;
        }
        pending.deciding_player = delegate;
        pending.presentation.candidates.clear();
        pending.presentation.min = 1;
        pending.presentation.max = 1;
        pending.presentation.choice_kind = rv1::ChoiceKind::ResolutionBranch;
        pending.presentation.prompt = "Choose a kind of counter on the target permanent.".into();
        pending.resolution_prompt_semantics = None;
        self.state.pending_resolution = Some(pending);
        let mut events = vec![ev_log(format!(
            "P{selector} selects P{delegate} to make P{}'s counter-kind choice.",
            stack.item.controller
        ))];
        events.push(
            self.chosen_counter_addition_choice_event()
                .expect("delegated chosen-counter choice remains parked"),
        );
        Ok(finish_with_events(self, events))
    }

    pub(super) fn finish_chosen_counter_addition_choice(
        &mut self,
        mut pending: PendingResolution,
        answer: &rv1::SubmitResolutionChoice,
        decision: rv1::ResolutionChoiceDecision,
    ) -> Result<RuledEventBatch, EngineError> {
        let stage = match &pending.continuation {
            ResolutionContinuation::ChosenCounterAddition {
                counter_addition, ..
            } => counter_addition.stage.clone(),
            _ => {
                self.state.pending_resolution = Some(pending);
                return Err(EngineError::Illegal(
                    "chosen-counter continuation is missing",
                ));
            }
        };
        if matches!(
            stage,
            crate::state::PendingChosenCounterAdditionStage::ChoosingDelegate { .. }
        ) {
            return self.select_chosen_counter_addition_delegate(pending, answer, decision);
        }
        if decision != rv1::ResolutionChoiceDecision::SelectBranch
            || !answer.chosen_object_ids.is_empty()
            || !answer.chosen_player_ids.is_empty()
            || answer.cast_spell.is_some()
            || answer.spell_cast_announcement.is_some()
            || answer.chosen_combat_defender.is_some()
            || answer.payment.is_some()
            || !answer.restricted_mana.is_empty()
        {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal(
                "chosen-counter choice requires one offered branch index",
            ));
        }
        let (stack, target, offered_kinds) = match &pending.continuation {
            ResolutionContinuation::ChosenCounterAddition {
                stack,
                counter_addition,
            } => (
                stack.clone(),
                counter_addition.target,
                counter_addition.offered_counter_kinds.clone(),
            ),
            _ => unreachable!("validated chosen-counter continuation"),
        };
        let Some(kind) = offered_kinds
            .get(answer.selected_branch_index as usize)
            .copied()
        else {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal("unoffered counter-kind choice"));
        };
        if !self.selection_is_valid_for_departed_controller(
            stack.item.controller,
            pending.deciding_player,
        ) {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal(
                "chosen-counter answerer is not the controller or a rules delegate",
            ));
        }
        let Some(current_kinds) =
            super::resolution::chosen_counter_addition_target_counter_kinds(self, &target)
        else {
            return self.complete_chosen_counter_addition_without_placement(stack, Vec::new());
        };
        if current_kinds.is_empty() {
            return self.complete_chosen_counter_addition_without_placement(stack, Vec::new());
        }
        if current_kinds != offered_kinds {
            if let ResolutionContinuation::ChosenCounterAddition {
                counter_addition, ..
            } = &mut pending.continuation
            {
                counter_addition.offered_counter_kinds = current_kinds;
            }
            pending.resolution_prompt_semantics = None;
            self.state.pending_resolution = Some(pending);
            let mut events = vec![ev_log(
                "The target's counter-kind choices have changed; choose from the refreshed offer."
                    .into(),
            )];
            events.push(
                self.chosen_counter_addition_choice_event()
                    .expect("refreshed chosen-counter choice remains parked"),
            );
            return Ok(finish_with_events(self, events));
        }
        let Some(proposal) = self.proposed_object_counter_placement(
            stack.item.controller,
            target.object_id,
            [(kind, 1)],
            super::super::continuous::CounterPlacementOrigin::Effect,
        ) else {
            return self.complete_chosen_counter_addition_without_placement(stack, Vec::new());
        };
        let (_, spell_label) = self.build_resolution_effects(&stack.item);
        let mut events = Vec::new();
        match self.begin_counter_placement_batch(
            vec![proposal],
            stack.clone(),
            crate::state::CounterPlacementCompletion::StackEffect { spell_label },
            &mut events,
        ) {
            Ok(Some(receipts)) => {
                let mut previous_result = stack.previous_result.clone();
                previous_result.counter_placements.extend(receipts);
                self.complete_parked_resolution_with_previous(
                    stack.item,
                    stack.resume_effect_index,
                    previous_result,
                    events,
                )
            }
            Ok(None) => Ok(finish_with_events(self, events)),
            Err(error) => {
                self.state.pending_resolution = Some(pending);
                Err(error)
            }
        }
    }

    /// Reconcile the exact unanswered counter-kind offer after a departure. This updates only
    /// counter options and CR 800.4g–h chooser delegation; it never repeats targetability.
    pub(in crate::engine) fn refresh_chosen_counter_addition_departure(
        &mut self,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<(), EngineError> {
        let Some(mut pending) = self.state.pending_resolution.take() else {
            return Ok(());
        };
        let (stack, target, stage) = match &pending.continuation {
            ResolutionContinuation::ChosenCounterAddition {
                stack,
                counter_addition,
            } => (
                stack.clone(),
                counter_addition.target,
                counter_addition.stage.clone(),
            ),
            _ => {
                self.state.pending_resolution = Some(pending);
                return Ok(());
            }
        };
        let Some(current_kinds) =
            super::resolution::chosen_counter_addition_target_counter_kinds(self, &target)
        else {
            let completed =
                self.complete_chosen_counter_addition_without_placement(stack, Vec::new())?;
            events.extend(completed.events);
            return Ok(());
        };
        if current_kinds.is_empty() {
            let completed =
                self.complete_chosen_counter_addition_without_placement(stack, Vec::new())?;
            events.extend(completed.events);
            return Ok(());
        }
        let offered_kinds_changed = match &mut pending.continuation {
            ResolutionContinuation::ChosenCounterAddition {
                counter_addition, ..
            } => {
                let changed = counter_addition.offered_counter_kinds != current_kinds;
                if changed {
                    counter_addition.offered_counter_kinds = current_kinds;
                    pending.resolution_prompt_semantics = None;
                }
                changed
            }
            _ => unreachable!("validated chosen-counter continuation"),
        };
        let controller_live = self
            .state
            .player_idx(stack.item.controller)
            .is_some_and(|index| !self.state.players[index].has_lost);
        if controller_live {
            self.state.pending_resolution = Some(pending);
            if offered_kinds_changed {
                events.push(ev_log(
                    "The target's counter-kind choices have changed.".into(),
                ));
                events.push(
                    self.chosen_counter_addition_choice_event()
                        .expect("refreshed chosen-counter choice remains parked"),
                );
            }
            return Ok(());
        }

        let eligible = super::resolution::resolution_choice_delegate_candidates(
            &self.state,
            stack.item.controller,
            stack.item.controller,
        );
        if eligible.is_empty() {
            self.state.pending_resolution = Some(pending);
            return Ok(());
        }
        let current_decider_live = self
            .state
            .player_idx(pending.deciding_player)
            .is_some_and(|index| !self.state.players[index].has_lost);
        if matches!(
            stage,
            crate::state::PendingChosenCounterAdditionStage::Selecting
        ) && current_decider_live
            && eligible.contains(&pending.deciding_player)
        {
            self.state.pending_resolution = Some(pending);
            if offered_kinds_changed {
                events.push(ev_log(
                    "The target's counter-kind choices have changed.".into(),
                ));
                events.push(
                    self.chosen_counter_addition_choice_event()
                        .expect("delegated chosen-counter choice remains parked"),
                );
            }
            return Ok(());
        }

        let selector =
            super::resolution::resolution_choice_selector(&self.state, stack.item.controller)
                .ok_or(EngineError::Illegal(
                    "no live player can select a chosen-counter delegate",
                ))?;
        let previous_slots = match stage {
            crate::state::PendingChosenCounterAdditionStage::ChoosingDelegate { candidates } => {
                candidates
            }
            crate::state::PendingChosenCounterAdditionStage::Selecting => Vec::new(),
        };
        let candidate_slots = super::resolution::reconcile_resolution_choice_delegate_slots(
            &previous_slots,
            &eligible,
        );
        let unchanged = matches!(
            &pending.continuation,
            ResolutionContinuation::ChosenCounterAddition {
                counter_addition: crate::state::PendingChosenCounterAddition {
                    stage: crate::state::PendingChosenCounterAdditionStage::ChoosingDelegate {
                        candidates,
                    },
                    ..
                },
                ..
            } if pending.deciding_player == selector
                && current_decider_live
                && *candidates == candidate_slots
                && !offered_kinds_changed
        );
        if unchanged {
            self.state.pending_resolution = Some(pending);
            return Ok(());
        }
        if let ResolutionContinuation::ChosenCounterAddition {
            counter_addition, ..
        } = &mut pending.continuation
        {
            counter_addition.stage =
                crate::state::PendingChosenCounterAdditionStage::ChoosingDelegate {
                    candidates: candidate_slots,
                };
        }
        pending.deciding_player = selector;
        pending.presentation.candidates.clear();
        pending.presentation.min = 1;
        pending.presentation.max = 1;
        pending.presentation.choice_kind = rv1::ChoiceKind::ResolutionBranch;
        pending.presentation.prompt = format!(
            "P{selector}: choose a player to make P{}'s counter-kind choice.",
            stack.item.controller
        );
        pending.resolution_prompt_semantics = None;
        self.state.pending_resolution = Some(pending);
        events.push(ev_log(format!(
            "P{}'s counter-kind choice must be delegated under CR 800.4g–h.",
            stack.item.controller
        )));
        events.push(
            self.chosen_counter_addition_choice_event()
                .expect("chosen-counter delegate prompt remains parked"),
        );
        Ok(())
    }
}
