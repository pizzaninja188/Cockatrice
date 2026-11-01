use super::*;
use crate::engine::continuous::CounterPlacementOrigin;

impl GameEngine {
    pub(super) fn finish_proliferate_choice(
        &mut self,
        pending: PendingResolution,
        answer: &rv1::SubmitResolutionChoice,
        decision: rv1::ResolutionChoiceDecision,
    ) -> Result<RuledEventBatch, EngineError> {
        if matches!(
            &pending.continuation,
            ResolutionContinuation::ProgramProliferate {
                stage: crate::state::PendingProliferateStage::ChoosingDelegate { .. },
                ..
            } | ResolutionContinuation::Proliferate {
                stage: crate::state::PendingProliferateStage::ChoosingDelegate { .. },
                ..
            }
        ) {
            return self.finish_proliferate_delegate(pending, answer, decision);
        }
        let (stack, candidate_generations, candidate_player_ids, transaction) =
            match &pending.continuation {
                ResolutionContinuation::Proliferate {
                    stack,
                    candidate_generations,
                    candidate_player_ids,
                    stage: crate::state::PendingProliferateStage::SelectingRecipients,
                    transaction,
                } => (
                    stack,
                    candidate_generations,
                    candidate_player_ids,
                    transaction,
                ),
                ResolutionContinuation::ProgramProliferate {
                    stack,
                    candidate_generations,
                    candidate_player_ids,
                    stage: crate::state::PendingProliferateStage::SelectingRecipients,
                    transaction,
                } => (
                    stack,
                    candidate_generations,
                    candidate_player_ids,
                    transaction,
                ),
                _ => {
                    self.state.pending_resolution = Some(pending);
                    return Err(EngineError::Illegal("not a Proliferate choice"));
                }
            };

        if decision != rv1::ResolutionChoiceDecision::Unspecified
            || answer.selected_branch_index != 0
            || answer.cast_spell.is_some()
            || answer.spell_cast_announcement.is_some()
            || answer.chosen_combat_defender.is_some()
            || answer.payment.is_some()
            || !answer.restricted_mana.is_empty()
        {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal("invalid Proliferate choice payload"));
        }

        let chosen_objects = answer.chosen_object_ids.as_slice();
        let chosen_players = answer.chosen_player_ids.as_slice();
        let total = chosen_objects.len().saturating_add(chosen_players.len()) as u32;
        if total < pending.presentation.min || total > pending.presentation.max {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal(
                "wrong number of Proliferate recipients",
            ));
        }

        let mut seen_objects = HashSet::new();
        for &object_id in chosen_objects {
            let current_generation = self
                .state
                .zone_change_generation
                .get(&object_id)
                .copied()
                .unwrap_or(0);
            let current_candidate = candidate_generations
                .iter()
                .any(|&(candidate, generation)| {
                    candidate == object_id && generation == current_generation
                });
            let still_has_counters = self.state.objects.get(&object_id).is_some_and(|object| {
                object.zone == Zone::Battlefield
                    && object.phase_status.is_none()
                    && object.counters.values().any(|count| *count > 0)
            });
            if !pending.presentation.candidates.contains(&object_id)
                || !current_candidate
                || !still_has_counters
                || !seen_objects.insert(object_id)
            {
                self.state.pending_resolution = Some(pending);
                return Err(EngineError::Illegal(
                    "invalid or stale Proliferate permanent",
                ));
            }
        }

        let mut seen_players = HashSet::new();
        for &player_id in chosen_players {
            let eligible_now = self.state.player_idx(player_id).is_some_and(|index| {
                let player = &self.state.players[index];
                !player.has_lost && player.counters.values().any(|count| *count > 0)
            });
            if !candidate_player_ids.contains(&player_id)
                || !eligible_now
                || !seen_players.insert(player_id)
            {
                self.state.pending_resolution = Some(pending);
                return Err(EngineError::Illegal("invalid or stale Proliferate player"));
            }
        }

        let mut placements = Vec::new();
        let effect_controller = transaction.actor;
        for &object_id in chosen_objects {
            let counters = self
                .state
                .objects
                .get(&object_id)
                .into_iter()
                .flat_map(|object| object.counters.iter())
                .filter_map(|(&kind, &count)| (count > 0).then_some((kind, 1)))
                .collect::<Vec<_>>();
            if let Some(event) = self.proposed_object_counter_placement(
                effect_controller,
                object_id,
                counters,
                CounterPlacementOrigin::Effect,
            ) {
                placements.push(event);
            }
        }

        for &player_id in chosen_players {
            let Some(index) = self.state.player_idx(player_id) else {
                unreachable!("validated player remains present during the command")
            };
            let counters = self.state.players[index]
                .counters
                .iter()
                .filter_map(|(&kind, &count)| (count > 0).then_some((kind, 1)))
                .collect::<Vec<_>>();
            if let Some(event) = self.proposed_player_counter_placement(
                effect_controller,
                player_id,
                counters,
                CounterPlacementOrigin::Effect,
            ) {
                placements.push(event);
            }
        }

        let stack = stack.clone();
        let mut logs = Vec::new();
        let receipts = self.begin_counter_placement_batch(
            placements,
            stack.clone(),
            crate::state::CounterPlacementCompletion::Proliferate {
                transaction: transaction.clone(),
            },
            &mut logs,
        )?;
        let Some(receipts) = receipts else {
            return Ok(finish_with_events(self, logs));
        };
        self.finish_proliferate_counter_leaf(stack, transaction.clone(), receipts, logs)
    }

    fn finish_proliferate_delegate(
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
                "a departed Proliferate chooser must select a live delegate",
            ));
        }
        let (stack, candidates, transaction) = match &pending.continuation {
            ResolutionContinuation::ProgramProliferate {
                stack,
                stage: crate::state::PendingProliferateStage::ChoosingDelegate { candidates },
                transaction,
                ..
            }
            | ResolutionContinuation::Proliferate {
                stack,
                stage: crate::state::PendingProliferateStage::ChoosingDelegate { candidates },
                transaction,
                ..
            } => (stack.clone(), candidates.clone(), transaction.clone()),
            _ => {
                self.state.pending_resolution = Some(pending);
                return Err(EngineError::Illegal(
                    "Proliferate delegate continuation missing",
                ));
            }
        };
        let delegate = candidates
            .get(answer.selected_branch_index as usize)
            .copied()
            .flatten();
        let actor = transaction.actor;
        let eligible = resolution::resolution_choice_delegate_candidates(&self.state, actor, actor);
        let Some(delegate) = delegate.filter(|delegate| eligible.contains(delegate)) else {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal(
                "that Proliferate delegate is no longer available",
            ));
        };
        if self
            .state
            .player_idx(delegate)
            .is_none_or(|index| self.state.players[index].has_lost)
        {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal(
                "that Proliferate delegate is no longer available",
            ));
        }
        let (candidate_generations, candidate_player_ids) = match &pending.continuation {
            ResolutionContinuation::ProgramProliferate {
                candidate_generations,
                candidate_player_ids,
                ..
            }
            | ResolutionContinuation::Proliferate {
                candidate_generations,
                candidate_player_ids,
                ..
            } => current_proliferate_candidates(
                &self.state,
                candidate_generations,
                candidate_player_ids,
            ),
            _ => unreachable!("validated Proliferate delegate continuation"),
        };
        if candidate_generations.is_empty() && candidate_player_ids.is_empty() {
            return self.complete_proliferate_without_candidates(
                stack,
                transaction,
                format!("No eligible recipients remain for P{actor}'s Proliferate choice."),
            );
        }
        let maximum = candidate_generations
            .len()
            .saturating_add(candidate_player_ids.len()) as u32;
        match &mut pending.continuation {
            ResolutionContinuation::ProgramProliferate {
                candidate_generations: pending_generations,
                candidate_player_ids: pending_players,
                stage,
                ..
            }
            | ResolutionContinuation::Proliferate {
                candidate_generations: pending_generations,
                candidate_player_ids: pending_players,
                stage,
                ..
            } => {
                *pending_generations = candidate_generations;
                *pending_players = candidate_player_ids;
                *stage = crate::state::PendingProliferateStage::SelectingRecipients;
            }
            _ => unreachable!("validated Proliferate delegate continuation"),
        }
        pending.deciding_player = delegate;
        pending.presentation.candidates = match &pending.continuation {
            ResolutionContinuation::ProgramProliferate {
                candidate_generations,
                ..
            }
            | ResolutionContinuation::Proliferate {
                candidate_generations,
                ..
            } => candidate_generations
                .iter()
                .map(|(object_id, _)| *object_id)
                .collect(),
            _ => unreachable!("validated Proliferate recipient continuation"),
        };
        pending.presentation.min = 0;
        pending.presentation.max = maximum;
        pending.presentation.choice_kind = rv1::ChoiceKind::Proliferate;
        pending.presentation.prompt =
            "Choose any number of permanents and/or players with counters.".into();
        pending.resolution_prompt_id = crate::state::ResolutionPromptId::UNASSIGNED;
        pending.resolution_prompt_semantics = None;
        let mut events = vec![ev_log(format!(
            "P{actor} chooses P{delegate} to make the Proliferate choice."
        ))];
        self.state.pending_resolution = Some(pending);
        events.push(
            self.proliferate_choice_event()
                .expect("delegated Proliferate choice remains parked"),
        );
        Ok(finish_with_events(self, events))
    }

    fn complete_proliferate_without_candidates(
        &mut self,
        stack: ParkedStackResolution,
        transaction: crate::engine::proliferate::PendingProliferateTransaction,
        log: String,
    ) -> Result<RuledEventBatch, EngineError> {
        let mut events = vec![ev_log(log)];
        self.fire_triggers(
            &[GameEvent::Proliferated {
                player: transaction.actor,
            }],
            &mut events,
        );
        self.finish_proliferate_counter_leaf(stack, transaction, vec![], events)
    }

    /// Reconcile a parked Proliferate prompt after a concession. Preserve stable delegate indices,
    /// retire departed recipients, and transfer only the choice actor; the transaction's original
    /// actor remains authoritative for counter placements and the `Proliferated` event.
    pub(in crate::engine) fn refresh_proliferate_departure(
        &mut self,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<(), EngineError> {
        let Some(mut pending) = self.state.pending_resolution.take() else {
            return Ok(());
        };
        let (stack, transaction, old_generations, old_players, old_stage) =
            match &pending.continuation {
                ResolutionContinuation::ProgramProliferate {
                    stack,
                    candidate_generations,
                    candidate_player_ids,
                    stage,
                    transaction,
                }
                | ResolutionContinuation::Proliferate {
                    stack,
                    candidate_generations,
                    candidate_player_ids,
                    stage,
                    transaction,
                } => (
                    stack.clone(),
                    transaction.clone(),
                    candidate_generations.clone(),
                    candidate_player_ids.clone(),
                    stage.clone(),
                ),
                _ => {
                    self.state.pending_resolution = Some(pending);
                    return Ok(());
                }
            };
        let (candidate_generations, candidate_player_ids) =
            current_proliferate_candidates(&self.state, &old_generations, &old_players);
        let candidates_changed =
            candidate_generations != old_generations || candidate_player_ids != old_players;
        let controller = transaction.actor;
        let controller_live = self
            .state
            .player_idx(controller)
            .is_some_and(|index| !self.state.players[index].has_lost);
        let current_chooser_live = self
            .state
            .player_idx(pending.deciding_player)
            .is_some_and(|index| !self.state.players[index].has_lost);

        if candidate_generations.is_empty() && candidate_player_ids.is_empty() {
            let batch = self.complete_proliferate_without_candidates(
                stack,
                transaction,
                format!("No eligible recipients remain for P{controller}'s Proliferate choice."),
            )?;
            events.extend(batch.events);
            return Ok(());
        }

        let selector = if controller_live {
            Some(controller)
        } else {
            self.state
                .player_idx(controller)
                .and_then(|index| self.state.next_in_game_player_idx(index))
                .map(|index| self.state.players[index].id)
                .or_else(|| {
                    self.state
                        .players
                        .iter()
                        .find(|player| !player.has_lost)
                        .map(|player| player.id)
                })
        };
        let eligible =
            resolution::resolution_choice_delegate_candidates(&self.state, controller, controller);
        let selector_needed = match &old_stage {
            crate::state::PendingProliferateStage::SelectingRecipients => {
                !controller_live && !current_chooser_live
            }
            crate::state::PendingProliferateStage::ChoosingDelegate { .. } => true,
        };
        if !selector_needed && current_chooser_live && !candidates_changed {
            self.state.pending_resolution = Some(pending);
            return Ok(());
        }
        if selector_needed && eligible.is_empty() {
            let batch = self.complete_proliferate_without_candidates(
                stack,
                transaction,
                format!("No live player can choose for P{controller}'s Proliferate effect."),
            )?;
            events.extend(batch.events);
            return Ok(());
        }
        let Some(selector) = selector else {
            self.state.pending_resolution = Some(pending);
            return Ok(());
        };

        let stage = if selector_needed {
            let mut slots = match &old_stage {
                crate::state::PendingProliferateStage::ChoosingDelegate { candidates } => {
                    candidates.clone()
                }
                crate::state::PendingProliferateStage::SelectingRecipients => Vec::new(),
            };
            for slot in &mut slots {
                if slot.is_some_and(|candidate| !eligible.contains(&candidate)) {
                    *slot = None;
                }
            }
            for candidate in eligible.iter().copied() {
                if !slots.contains(&Some(candidate)) {
                    slots.push(Some(candidate));
                }
            }
            crate::state::PendingProliferateStage::ChoosingDelegate { candidates: slots }
        } else {
            crate::state::PendingProliferateStage::SelectingRecipients
        };
        let delegate_prompt = matches!(
            stage,
            crate::state::PendingProliferateStage::ChoosingDelegate { .. }
        );
        let recipient_chooser = if current_chooser_live
            && matches!(
                &old_stage,
                crate::state::PendingProliferateStage::SelectingRecipients
            ) {
            pending.deciding_player
        } else {
            controller
        };
        let same_prompt = selector_needed
            && current_chooser_live
            && pending.deciding_player == selector
            && !candidates_changed
            && matches!(
                (&old_stage, &stage),
                (
                    crate::state::PendingProliferateStage::ChoosingDelegate {
                        candidates: old
                    },
                    crate::state::PendingProliferateStage::ChoosingDelegate {
                        candidates: new
                    }
                ) if old == new
            );
        if same_prompt {
            self.state.pending_resolution = Some(pending);
            return Ok(());
        }
        let maximum = candidate_generations
            .len()
            .saturating_add(candidate_player_ids.len()) as u32;
        pending.deciding_player = if delegate_prompt {
            selector
        } else {
            recipient_chooser
        };
        pending.presentation.candidates = if delegate_prompt {
            Vec::new()
        } else {
            candidate_generations
                .iter()
                .map(|(object_id, _)| *object_id)
                .collect()
        };
        pending.presentation.min = u32::from(delegate_prompt);
        pending.presentation.max = if delegate_prompt { 1 } else { maximum };
        pending.presentation.choice_kind = if delegate_prompt {
            rv1::ChoiceKind::ResolutionBranch
        } else {
            rv1::ChoiceKind::Proliferate
        };
        pending.presentation.prompt = if delegate_prompt {
            format!("P{selector}: choose a player to make P{controller}'s Proliferate choice.")
        } else {
            "Choose any number of permanents and/or players with counters.".into()
        };
        pending.resolution_prompt_id = crate::state::ResolutionPromptId::UNASSIGNED;
        pending.resolution_prompt_semantics = None;
        match &mut pending.continuation {
            ResolutionContinuation::ProgramProliferate {
                candidate_generations: pending_generations,
                candidate_player_ids: pending_players,
                stage: pending_stage,
                ..
            }
            | ResolutionContinuation::Proliferate {
                candidate_generations: pending_generations,
                candidate_player_ids: pending_players,
                stage: pending_stage,
                ..
            } => {
                *pending_generations = candidate_generations;
                *pending_players = candidate_player_ids;
                *pending_stage = stage;
            }
            _ => unreachable!("validated Proliferate continuation"),
        }
        self.state.pending_resolution = Some(pending);
        events.push(ev_log(if delegate_prompt {
            format!("P{selector} must choose a replacement for P{controller}'s Proliferate choice.")
        } else {
            format!("P{controller}'s Proliferate recipient offer has been refreshed.")
        }));
        events.push(
            self.proliferate_choice_event()
                .expect("refreshed Proliferate choice remains parked"),
        );
        Ok(())
    }
}

fn current_proliferate_candidates(
    state: &crate::state::GameState,
    candidate_generations: &[(u32, u64)],
    candidate_player_ids: &[i32],
) -> (Vec<(u32, u64)>, Vec<i32>) {
    let generations = candidate_generations
        .iter()
        .copied()
        .filter(|(object_id, generation)| {
            state
                .zone_change_generation
                .get(object_id)
                .copied()
                .unwrap_or(0)
                == *generation
                && state.objects.get(object_id).is_some_and(|object| {
                    object.zone == Zone::Battlefield
                        && object.phase_status.is_none()
                        && object.counters.values().any(|count| *count > 0)
                })
        })
        .collect();
    let players = candidate_player_ids
        .iter()
        .copied()
        .filter(|&player_id| {
            state.player_idx(player_id).is_some_and(|index| {
                let player = &state.players[index];
                !player.has_lost && player.counters.values().any(|count| *count > 0)
            })
        })
        .collect();
    (generations, players)
}
