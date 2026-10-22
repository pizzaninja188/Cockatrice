use super::*;

impl GameEngine {
    pub(super) fn finish_program_branch_choice(
        &mut self,
        mut pending: PendingResolution,
        answer: &rv1::SubmitResolutionChoice,
        decision: rv1::ResolutionChoiceDecision,
    ) -> Result<RuledEventBatch, EngineError> {
        let clean = answer.chosen_object_ids.is_empty()
            && answer.chosen_player_ids.is_empty()
            && answer.cast_spell.is_none()
            && answer.chosen_combat_defender.is_none()
            && answer.payment.is_none()
            && answer.restricted_mana.is_empty()
            && answer.spell_cast_announcement.is_none();
        if !clean || decision != rv1::ResolutionChoiceDecision::SelectBranch {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal(
                "typed library cohort branch choice has unrelated answer fields",
            ));
        }
        if matches!(
            &pending.continuation,
            ResolutionContinuation::ProgramHandLibraryReturn { .. }
        ) {
            return self.finish_program_hand_library_return_choice(pending, answer);
        }
        let mut events = Vec::new();
        match &mut pending.continuation {
            ResolutionContinuation::ProgramCohortPartition {
                stack,
                target_player,
                offered_cohort,
                limit,
                stage,
                ..
            } => {
                let PendingProgramCohortPartitionStage::ChoosingDelegate { candidates } = stage
                else {
                    self.state.pending_resolution = Some(pending);
                    return Err(EngineError::Illegal(
                        "library cohort is not waiting for a delegate choice",
                    ));
                };
                if pending.deciding_player != stack.item.controller {
                    self.state.pending_resolution = Some(pending);
                    return Err(EngineError::Illegal(
                        "only the effect controller chooses a replacement chooser",
                    ));
                }
                let delegate = candidates
                    .get(answer.selected_branch_index as usize)
                    .copied()
                    .flatten();
                let eligible = resolution::resolution_choice_delegate_candidates(
                    &self.state,
                    stack.item.controller,
                    *target_player,
                );
                let Some(delegate) = delegate.filter(|candidate| eligible.contains(candidate))
                else {
                    self.state.pending_resolution = Some(pending);
                    return Err(EngineError::Illegal(
                        "that library cohort chooser is no longer available",
                    ));
                };
                let Some(index) = self.state.player_idx(delegate) else {
                    self.state.pending_resolution = Some(pending);
                    return Err(EngineError::Illegal(
                        "that library cohort chooser is no longer available",
                    ));
                };
                if self.state.players[index].has_lost {
                    self.state.pending_resolution = Some(pending);
                    return Err(EngineError::Illegal(
                        "that library cohort chooser is no longer available",
                    ));
                }
                pending.deciding_player = delegate;
                pending.presentation.candidates =
                    offered_cohort.iter().map(|card| card.object_id).collect();
                pending.presentation.min = (*limit as usize).min(offered_cohort.len()) as u32;
                pending.presentation.max = pending.presentation.min;
                pending.presentation.choice_kind = rv1::ChoiceKind::Revealed;
                pending.presentation.ordered = false;
                pending.presentation.unique_names = false;
                pending.presentation.prompt = format!(
                    "P{delegate}: choose {} of the revealed cards to put into the graveyard.",
                    pending.presentation.min
                );
                *stage = PendingProgramCohortPartitionStage::ChoosingPartition;
                events.push(ev_log(format!(
                    "P{} chooses P{delegate} to make the departed player's library cohort choice.",
                    stack.item.controller
                )));
                let prompt = pending.presentation.prompt.clone();
                let candidate_ids = pending.presentation.candidates.clone();
                let (candidate_card_ids, candidate_names) =
                    resolution::candidate_identities(self, &candidate_ids);
                events.push(rv1::RuledEvent {
                    ev: Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(
                        rv1::ResolutionChoiceRequired {
                            resolution_prompt_id: 0,
                            deciding_player_id: delegate,
                            source_object_id: stack.item.id,
                            prompt_text: prompt,
                            choice_kind: rv1::ChoiceKind::Revealed as i32,
                            candidate_object_ids: candidate_ids,
                            candidate_card_ids,
                            candidate_names,
                            min: pending.presentation.min,
                            max: pending.presentation.max,
                            public_reveal: match &pending.continuation {
                                ResolutionContinuation::ProgramCohortPartition {
                                    public_reveal,
                                    ..
                                } => public_reveal.clone(),
                                _ => None,
                            },
                            ..Default::default()
                        },
                    )),
                });
                self.state.pending_resolution = Some(pending);
            }
            ResolutionContinuation::ProgramPartitionMove { stack, plan } => {
                let PendingProgramPartitionMoveStage::ChoosingCommander { index } = plan.stage
                else {
                    self.state.pending_resolution = Some(pending);
                    return Err(EngineError::Illegal(
                        "partition move is not waiting for a commander choice",
                    ));
                };
                if answer.selected_branch_index > 1 {
                    self.state.pending_resolution = Some(pending);
                    return Err(EngineError::Illegal("unknown commander destination"));
                }
                let hand_cohort =
                    super::super::resolution::library_cohorts::program_partition_hand_cohort(plan);
                let Some(card) = hand_cohort.get(index).copied() else {
                    self.state.pending_resolution = Some(pending);
                    return Err(EngineError::ProgramInvariant(
                        "commander replacement index is outside its hand partition",
                    ));
                };
                if pending.deciding_player != card.owner
                    || !super::super::resolution::library_cohorts::program_library_ref_is_current(
                        self, &card,
                    )
                {
                    self.state.pending_resolution = Some(pending);
                    return Err(EngineError::Illegal(
                        "commander replacement candidate became stale",
                    ));
                }
                let destination = if answer.selected_branch_index == 0 {
                    Zone::Command
                } else {
                    Zone::Hand
                };
                plan.commander_replacements.push((index, destination));
                plan.commander_choice_cursor += 1;
                plan.stage = PendingProgramPartitionMoveStage::ReadyToCommit;
                let stack = stack.clone();
                let plan = plan.clone();
                match super::super::resolution::library_cohorts::advance_program_partition_move(
                    self, stack.clone(), plan, &mut events,
                )? {
                    super::super::resolution::library_cohorts::ProgramPartitionMoveProgress::Parked => {}
                    super::super::resolution::library_cohorts::ProgramPartitionMoveProgress::Committed => {
                        return self.complete_parked_stack_with_previous(
                            stack,
                            EffectResult::default(),
                            events,
                        );
                    }
                }
            }
            _ => {
                self.state.pending_resolution = Some(pending);
                return Err(EngineError::Illegal(
                    "typed library cohort branch continuation missing",
                ));
            }
        }
        Ok(finish_with_events(self, events))
    }

    pub(super) fn finish_program_cohort_partition(
        &mut self,
        pending: PendingResolution,
        chosen: &[ObjectId],
    ) -> Result<RuledEventBatch, EngineError> {
        let (stack, offered, public_reveal, stage) = match &pending.continuation {
            ResolutionContinuation::ProgramCohortPartition {
                stack,
                offered_cohort,
                public_reveal,
                stage,
                ..
            } => (
                stack.clone(),
                offered_cohort.clone(),
                public_reveal.clone(),
                stage.clone(),
            ),
            _ => unreachable!("library cohort partition continuation"),
        };
        if stage != PendingProgramCohortPartitionStage::ChoosingPartition
            || !self.player_is_live(pending.deciding_player)
        {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal(
                "library cohort chooser changed before answering",
            ));
        }
        if offered.iter().any(|card| {
            !super::super::resolution::library_cohorts::program_library_ref_is_current(self, card)
        }) {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal(
                "library cohort contains a stale offered reference",
            ));
        }
        let selected_set = chosen.iter().copied().collect::<HashSet<_>>();
        if selected_set.len() != chosen.len()
            || chosen
                .iter()
                .any(|id| !offered.iter().any(|card| card.object_id == *id))
        {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal("invalid library cohort partition"));
        }
        let selected = offered
            .iter()
            .copied()
            .filter(|card| selected_set.contains(&card.object_id))
            .collect::<Vec<_>>();
        let remainder = offered
            .iter()
            .copied()
            .filter(|card| !selected_set.contains(&card.object_id))
            .collect::<Vec<_>>();
        let names = selected
            .iter()
            .map(|card| object_display_name(&self.state, self.registry, card.object_id))
            .collect::<Vec<_>>();
        let events = vec![ev_log(format!(
            "P{} selects {} card(s) for the graveyard partition.",
            pending.deciding_player,
            selected.len()
        ))];
        self.complete_parked_stack_with_previous(
            stack,
            EffectResult {
                program_value: Some(ProgramValue::CardPartition(ProgramCardPartition {
                    selected,
                    remainder,
                    public_reveal,
                })),
                ..Default::default()
            },
            if names.is_empty() { Vec::new() } else { events },
        )
    }

    fn complete_empty_program_partition(
        &mut self,
        stack: ParkedStackResolution,
        events: Vec<rv1::RuledEvent>,
    ) -> Result<RuledEventBatch, EngineError> {
        self.complete_parked_stack_with_previous(
            stack,
            EffectResult {
                program_value: Some(ProgramValue::CardPartition(ProgramCardPartition {
                    selected: Vec::new(),
                    remainder: Vec::new(),
                    public_reveal: None,
                })),
                ..Default::default()
            },
            events,
        )
    }

    pub(super) fn finish_program_graveyard_order(
        &mut self,
        pending: PendingResolution,
        chosen: &[ObjectId],
    ) -> Result<RuledEventBatch, EngineError> {
        let (stack, mut plan) = match &pending.continuation {
            ResolutionContinuation::ProgramPartitionMove { stack, plan } => {
                (stack.clone(), plan.clone())
            }
            _ => unreachable!("partition move continuation"),
        };
        if plan.stage != PendingProgramPartitionMoveStage::ChoosingGraveyardOrder {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal(
                "graveyard order is not currently requested",
            ));
        }
        let mut order = Vec::with_capacity(chosen.len());
        for object_id in chosen {
            let graveyard_cohort =
                super::super::resolution::library_cohorts::program_partition_graveyard_cohort(
                    &plan,
                );
            let Some(card) = graveyard_cohort
                .iter()
                .copied()
                .find(|card| card.object_id == *object_id)
            else {
                self.state.pending_resolution = Some(pending);
                return Err(EngineError::Illegal("invalid graveyard order candidate"));
            };
            order.push(card);
        }
        plan.graveyard_order = order;
        let mut events = Vec::new();
        match super::super::resolution::library_cohorts::advance_program_partition_move(
            self,
            stack.clone(),
            plan,
            &mut events,
        )? {
            super::super::resolution::library_cohorts::ProgramPartitionMoveProgress::Parked => {
                Ok(finish_with_events(self, events))
            }
            super::super::resolution::library_cohorts::ProgramPartitionMoveProgress::Committed => {
                self.complete_parked_stack_with_previous(stack, EffectResult::default(), events)
            }
        }
    }

    pub(in crate::engine) fn refresh_program_library_cohort_departure(
        &mut self,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<(), EngineError> {
        let Some(mut pending) = self.state.pending_resolution.take() else {
            return Ok(());
        };
        let Some(stack) = pending.continuation.stack().cloned() else {
            self.state.pending_resolution = Some(pending);
            return Ok(());
        };
        if stack.program.is_none() {
            self.state.pending_resolution = Some(pending);
            return Ok(());
        }
        if !self.player_is_live(stack.item.controller) {
            let completed = match &pending.continuation {
                ResolutionContinuation::SearchLibrary { searcher, .. } => {
                    self.complete_departed_program_library_search(stack, *searcher)?
                }
                ResolutionContinuation::ProgramCohortPartition { .. } => {
                    self.complete_empty_program_partition(stack, Vec::new())?
                }
                ResolutionContinuation::ProgramPartitionMove { .. } => self
                    .complete_parked_stack_with_previous(
                        stack,
                        EffectResult::default(),
                        Vec::new(),
                    )?,
                _ => {
                    self.state.pending_resolution = Some(pending);
                    return Ok(());
                }
            };
            events.extend(completed.events);
            return Ok(());
        }
        match &mut pending.continuation {
            ResolutionContinuation::SearchLibrary {
                stack,
                searcher,
                filter,
                candidate_generations,
                selection_slot_candidates,
                commander_choice,
                shuffle,
                ..
            } if stack.program.is_some() => {
                if !self.player_is_live(*searcher) {
                    let stack = stack.clone();
                    let completed =
                        self.complete_departed_program_library_search(stack, *searcher)?;
                    events.extend(completed.events);
                    return Ok(());
                }
                if let Some(choice) = *commander_choice {
                    let target_is_current =
                        self.state.player_idx(choice.owner).is_some_and(|index| {
                            !self.state.players[index].has_lost
                                && self.state.players[index]
                                    .declared_commander_object_ids
                                    .contains(&choice.object_id)
                                && self.state.players[index]
                                    .library
                                    .contains(&choice.object_id)
                                && self
                                    .state
                                    .zone_change_generation
                                    .get(&choice.object_id)
                                    .copied()
                                    .unwrap_or(0)
                                    == choice.zone_change_generation
                                && self
                                    .state
                                    .objects
                                    .get(&choice.object_id)
                                    .is_some_and(|object| {
                                        object.owner == choice.owner && object.zone == Zone::Library
                                    })
                        });
                    if target_is_current {
                        // The owner-destination decision is already the active prompt. A source
                        // departure cannot replace it with the earlier library-search menu.
                        self.state.pending_resolution = Some(pending);
                        return Ok(());
                    }
                    let stack = stack.clone();
                    let searcher = *searcher;
                    let mut completed_events = Vec::new();
                    if *shuffle {
                        crate::engine::shuffle_player_library_for_current_command(
                            &mut self.state,
                            searcher,
                        );
                        completed_events
                            .push(ev_log(format!("P{searcher} shuffles their library.")));
                    }
                    self.fire_triggers(
                        &[GameEvent::LibrarySearched {
                            searcher,
                            library_owner: searcher,
                        }],
                        &mut completed_events,
                    );
                    let completed = self.complete_parked_stack_with_previous(
                        stack.clone(),
                        stack.previous_result.clone(),
                        completed_events,
                    )?;
                    events.extend(completed.events);
                    return Ok(());
                }
                let index = self
                    .state
                    .player_idx(*searcher)
                    .expect("live searcher has an index");
                let current = candidate_generations
                    .iter()
                    .filter_map(|(object_id, generation)| {
                        (self
                            .state
                            .zone_change_generation
                            .get(object_id)
                            .copied()
                            .unwrap_or(0)
                            == *generation
                            && self.state.players[index].library.contains(object_id)
                            && self.state.objects.get(object_id).is_some_and(|object| {
                                object.zone == Zone::Library && object.owner == *searcher
                            })
                            && super::super::zone_card_matches_filter(
                                &self.state,
                                self.registry,
                                *object_id,
                                filter.as_ref(),
                            ))
                        .then_some((*object_id, *generation))
                    })
                    .collect::<Vec<_>>();
                let changed = *candidate_generations != current;
                if changed {
                    *candidate_generations = current.clone();
                    for slot_candidates in selection_slot_candidates.iter_mut() {
                        slot_candidates.retain(|object_id| {
                            current.iter().any(|(candidate, _)| candidate == object_id)
                        });
                    }
                    let candidate_ids = current
                        .iter()
                        .map(|(object_id, _)| *object_id)
                        .collect::<Vec<_>>();
                    pending.presentation.candidates = candidate_ids.clone();
                    pending.resolution_prompt_id = crate::state::ResolutionPromptId::UNASSIGNED;
                    pending.resolution_prompt_semantics = None;
                    let (candidate_card_ids, candidate_names) =
                        resolution::candidate_identities(self, &candidate_ids);
                    events.push(rv1::RuledEvent {
                        ev: Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(
                            rv1::ResolutionChoiceRequired {
                                resolution_prompt_id: 0,
                                deciding_player_id: pending.deciding_player,
                                source_object_id: pending.presentation.source_object_id,
                                prompt_text: pending.presentation.prompt.clone(),
                                choice_kind: pending.presentation.choice_kind as i32,
                                candidate_object_ids: candidate_ids,
                                candidate_card_ids,
                                candidate_names,
                                min: pending.presentation.min,
                                max: pending.presentation.max,
                                unique_names: pending.presentation.unique_names,
                                ..Default::default()
                            },
                        )),
                    });
                }
                self.state.pending_resolution = Some(pending);
            }
            ResolutionContinuation::ProgramCohortPartition {
                stack,
                target_player,
                offered_cohort,
                limit,
                stage,
                ..
            } => {
                let stack = stack.clone();
                let target_player = *target_player;
                let limit = *limit;
                let live = offered_cohort
                    .iter()
                    .copied()
                    .filter(|card| {
                        super::super::resolution::library_cohorts::program_library_ref_is_current(
                            self, card,
                        )
                    })
                    .collect::<Vec<_>>();
                let changed = live != *offered_cohort;
                *offered_cohort = live.clone();
                if live.is_empty() {
                    let completed = self.complete_empty_program_partition(stack, Vec::new())?;
                    events.extend(completed.events);
                    return Ok(());
                }
                let target_live = self.player_is_live(target_player);
                if target_live {
                    let is_delegate = matches!(
                        stage,
                        PendingProgramCohortPartitionStage::ChoosingDelegate { .. }
                    );
                    if is_delegate {
                        *stage = PendingProgramCohortPartitionStage::ChoosingPartition;
                        pending.deciding_player = target_player;
                    }
                    pending.presentation.candidates =
                        live.iter().map(|card| card.object_id).collect();
                    pending.presentation.min = (limit as usize).min(live.len()) as u32;
                    pending.presentation.max = pending.presentation.min;
                    pending.presentation.choice_kind = rv1::ChoiceKind::Revealed;
                    pending.presentation.prompt = format!(
                        "P{target_player}: choose {} of the revealed cards to put into the graveyard.",
                        pending.presentation.min
                    );
                    if changed || is_delegate {
                        pending.resolution_prompt_id = crate::state::ResolutionPromptId::UNASSIGNED;
                        pending.resolution_prompt_semantics = None;
                        events.push(program_cohort_partition_event(self, &pending));
                    }
                    self.state.pending_resolution = Some(pending);
                    return Ok(());
                }
                let eligible = resolution::resolution_choice_delegate_candidates(
                    &self.state,
                    stack.item.controller,
                    target_player,
                );
                if eligible.is_empty() {
                    let completed = self.complete_empty_program_partition(stack, Vec::new())?;
                    events.extend(completed.events);
                    return Ok(());
                }
                let old_delegate_prompt = matches!(
                    stage,
                    PendingProgramCohortPartitionStage::ChoosingDelegate { .. }
                );
                let mut slots = match stage {
                    PendingProgramCohortPartitionStage::ChoosingDelegate { candidates } => {
                        candidates.clone()
                    }
                    PendingProgramCohortPartitionStage::ChoosingPartition => Vec::new(),
                };
                for slot in &mut slots {
                    if slot.is_some_and(|candidate| !eligible.contains(&candidate)) {
                        *slot = None;
                    }
                }
                for candidate in &eligible {
                    if !slots.contains(&Some(*candidate)) {
                        slots.push(Some(*candidate));
                    }
                }
                if eligible.len() == 1 && !old_delegate_prompt {
                    pending.deciding_player = eligible[0];
                    *stage = PendingProgramCohortPartitionStage::ChoosingPartition;
                    pending.presentation.candidates =
                        live.iter().map(|card| card.object_id).collect();
                    pending.presentation.min = (limit as usize).min(live.len()) as u32;
                    pending.presentation.max = pending.presentation.min;
                    pending.presentation.choice_kind = rv1::ChoiceKind::Revealed;
                    pending.presentation.prompt = format!(
                        "P{}: choose {} of the revealed cards to put into the graveyard.",
                        eligible[0], pending.presentation.min
                    );
                    pending.resolution_prompt_id = crate::state::ResolutionPromptId::UNASSIGNED;
                    pending.resolution_prompt_semantics = None;
                    events.push(ev_log(format!(
                        "P{} assigns P{} to make the departed player's library cohort choice.",
                        stack.item.controller, eligible[0]
                    )));
                    events.push(program_cohort_partition_event(self, &pending));
                } else {
                    pending.deciding_player = stack.item.controller;
                    pending.presentation.candidates.clear();
                    pending.presentation.min = 1;
                    pending.presentation.max = 1;
                    pending.presentation.choice_kind = rv1::ChoiceKind::ResolutionBranch;
                    pending.presentation.prompt = format!(
                        "P{}: choose a player to make P{target_player}'s library cohort choice.",
                        stack.item.controller
                    );
                    *stage =
                        PendingProgramCohortPartitionStage::ChoosingDelegate { candidates: slots };
                    pending.resolution_prompt_id = crate::state::ResolutionPromptId::UNASSIGNED;
                    pending.resolution_prompt_semantics = None;
                    events.push(program_cohort_partition_event(self, &pending));
                }
                self.state.pending_resolution = Some(pending);
            }
            ResolutionContinuation::ProgramPartitionMove { stack, plan } => {
                let stack = stack.clone();
                let old_plan = plan.clone();
                let refreshed = refresh_program_partition_plan(self, old_plan);
                if refreshed.partition.selected == plan.partition.selected
                    && refreshed.partition.remainder == plan.partition.remainder
                    && refreshed.graveyard_order == plan.graveyard_order
                    && refreshed.commander_replacements == plan.commander_replacements
                {
                    self.state.pending_resolution = Some(pending);
                    return Ok(());
                }
                *plan = refreshed;
                if plan.partition.selected.is_empty() && plan.partition.remainder.is_empty() {
                    let completed = self.complete_parked_stack_with_previous(
                        stack,
                        EffectResult::default(),
                        Vec::new(),
                    )?;
                    events.extend(completed.events);
                    return Ok(());
                }
                plan.stage = PendingProgramPartitionMoveStage::ReadyToCommit;
                pending.presentation.candidates.clear();
                pending.resolution_prompt_id = crate::state::ResolutionPromptId::UNASSIGNED;
                pending.resolution_prompt_semantics = None;
                match super::super::resolution::library_cohorts::advance_program_partition_move(
                    self,
                    stack.clone(),
                    plan.clone(),
                    events,
                )? {
                    super::super::resolution::library_cohorts::ProgramPartitionMoveProgress::Parked => {}
                    super::super::resolution::library_cohorts::ProgramPartitionMoveProgress::Committed => {
                        let completed = self.complete_parked_stack_with_previous(
                            stack,
                            EffectResult::default(),
                            std::mem::take(events),
                        )?;
                        events.extend(completed.events);
                        return Ok(());
                    }
                }
            }
            _ => self.state.pending_resolution = Some(pending),
        }
        Ok(())
    }

    fn complete_departed_program_library_search(
        &mut self,
        stack: ParkedStackResolution,
        searcher: PlayerId,
    ) -> Result<RuledEventBatch, EngineError> {
        let mut events = Vec::new();
        self.fire_triggers(
            &[GameEvent::LibrarySearched {
                searcher,
                library_owner: searcher,
            }],
            &mut events,
        );
        let completion_result = if stack
            .program
            .as_ref()
            .is_some_and(|program| program.in_flight.as_ref().is_some_and(Vec::is_empty))
        {
            stack.previous_result.clone()
        } else {
            EffectResult {
                program_value: Some(ProgramValue::FoundLibraryCohort(Vec::new())),
                ..Default::default()
            }
        };
        self.complete_parked_stack_with_previous(stack, completion_result, events)
    }
}

pub(in crate::engine) fn program_cohort_partition_event(
    engine: &GameEngine,
    pending: &PendingResolution,
) -> rv1::RuledEvent {
    let (source_object_id, candidates, public_reveal) = match &pending.continuation {
        ResolutionContinuation::ProgramCohortPartition {
            stack,
            offered_cohort,
            public_reveal,
            ..
        } => (
            stack.item.id,
            offered_cohort
                .iter()
                .map(|card| card.object_id)
                .collect::<Vec<_>>(),
            public_reveal.clone(),
        ),
        _ => return rv1::RuledEvent::default(),
    };
    let (candidate_card_ids, candidate_names) =
        resolution::candidate_identities(engine, &candidates);
    let mut choice = rv1::ResolutionChoiceRequired {
        resolution_prompt_id: 0,
        deciding_player_id: pending.deciding_player,
        source_object_id,
        prompt_text: pending.presentation.prompt.clone(),
        choice_kind: pending.presentation.choice_kind as i32,
        candidate_object_ids: candidates,
        candidate_card_ids,
        candidate_names,
        min: pending.presentation.min,
        max: pending.presentation.max,
        ordered: pending.presentation.ordered,
        unique_names: pending.presentation.unique_names,
        public_reveal,
        ..Default::default()
    };
    if let ResolutionContinuation::ProgramCohortPartition {
        stage: PendingProgramCohortPartitionStage::ChoosingDelegate { candidates },
        ..
    } = &pending.continuation
    {
        let eligible = resolution::resolution_choice_delegate_candidates(
            &engine.state,
            pending.deciding_player,
            match &pending.continuation {
                ResolutionContinuation::ProgramCohortPartition { target_player, .. } => {
                    *target_player
                }
                _ => unreachable!(),
            },
        );
        choice.candidate_player_ids = candidates
            .iter()
            .map(|candidate| candidate.unwrap_or(0))
            .collect();
        choice.resolution_branches = candidates
            .iter()
            .enumerate()
            .map(|(index, candidate)| rv1::ResolutionBranchOption {
                branch_index: index as u32,
                label: candidate
                    .map(|player| format!("P{player}"))
                    .unwrap_or_else(|| "Unavailable player".into()),
                selectable: candidate.is_some_and(|player| eligible.contains(&player)),
                ..Default::default()
            })
            .collect();
        choice.candidate_object_ids.clear();
        choice.candidate_card_ids.clear();
        choice.candidate_names.clear();
    }
    rv1::RuledEvent {
        ev: Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(choice)),
    }
}

fn refresh_program_partition_plan(
    engine: &GameEngine,
    mut plan: ProgramPartitionMovePlan,
) -> ProgramPartitionMovePlan {
    let old_hand = super::super::resolution::library_cohorts::program_partition_hand_cohort(&plan);
    let old_order = plan.graveyard_order.clone();
    let old_order_len = old_order.len();
    plan.partition.selected.retain(|card| {
        super::super::resolution::library_cohorts::program_library_ref_is_current(engine, card)
    });
    plan.partition.remainder.retain(|card| {
        super::super::resolution::library_cohorts::program_library_ref_is_current(engine, card)
    });
    let new_graveyard =
        super::super::resolution::library_cohorts::program_partition_graveyard_cohort(&plan);
    let new_hand = super::super::resolution::library_cohorts::program_partition_hand_cohort(&plan);
    let new_hand_indices = new_hand
        .iter()
        .enumerate()
        .map(|(index, card)| (card.object_id, index))
        .collect::<HashMap<_, _>>();
    let old_hand_index_to_object = old_hand
        .iter()
        .enumerate()
        .map(|(index, card)| (index, card.object_id))
        .collect::<HashMap<_, _>>();
    plan.commander_replacements = plan
        .commander_replacements
        .iter()
        .filter_map(|(old_index, destination)| {
            new_hand_indices
                .get(old_hand_index_to_object.get(old_index)?)
                .map(|new_index| (*new_index, *destination))
        })
        .collect();
    let old_choice_indices = plan.commander_choice_indices.clone();
    let mut next_indices = Vec::new();
    for (new_index, card) in new_hand.iter().enumerate() {
        if engine
            .state
            .player_idx(card.owner)
            .is_some_and(|owner_index| {
                engine.state.players[owner_index]
                    .declared_commander_object_ids
                    .contains(&card.object_id)
            })
        {
            next_indices.push(new_index);
        }
    }
    let old_cursor = plan.commander_choice_cursor.min(old_choice_indices.len());
    let answered_objects = old_choice_indices
        .iter()
        .take(old_cursor)
        .filter_map(|old_index| old_hand_index_to_object.get(old_index).copied())
        .collect::<HashSet<_>>();
    plan.commander_choice_indices = next_indices;
    plan.commander_choice_cursor = plan
        .commander_choice_indices
        .iter()
        .take_while(|new_index| {
            new_hand
                .get(**new_index)
                .is_some_and(|card| answered_objects.contains(&card.object_id))
        })
        .count();
    plan.graveyard_order = old_order
        .iter()
        .copied()
        .filter(|card| new_graveyard.contains(card))
        .collect();
    if plan.graveyard_order.len() != old_order_len {
        plan.graveyard_order.clear();
    }
    if plan.commander_choice_cursor < plan.commander_choice_indices.len() {
        plan.stage = PendingProgramPartitionMoveStage::ReadyToCommit;
    } else if plan.graveyard_order.len() <= 1 {
        plan.graveyard_order.clear();
        plan.stage = PendingProgramPartitionMoveStage::ReadyToCommit;
    } else {
        plan.stage = PendingProgramPartitionMoveStage::ChoosingGraveyardOrder;
    }
    plan
}

impl GameEngine {
    fn player_is_live(&self, player: PlayerId) -> bool {
        self.state
            .player_idx(player)
            .is_some_and(|index| !self.state.players[index].has_lost)
    }
}
