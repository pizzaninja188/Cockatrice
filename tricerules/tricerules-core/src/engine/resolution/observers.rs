//! Immediate paired one-shot observer returns and suspended resolution restoration.

use super::*;

impl GameEngine {
    /// Complete CR 610.3 paired one-shot work discovered by a committed zone transition. This is
    /// called between effect instructions and again at the command boundary, so the return is
    /// immediate rather than a trigger that uses the stack. All currently-ready objects enter as
    /// one event cohort, preserving simultaneous returns when several sources leave together.
    pub(in crate::engine) fn drain_immediate_observer_actions(
        &mut self,
        resume_stack: Option<ParkedStackResolution>,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<bool, EngineError> {
        self.reconcile_combat_characteristics(events);
        // CR 404.3 completes the simultaneous departure's owner ordering before a CR 610.3
        // return may park another choice. Never overwrite its frozen event continuation.
        if self
            .state
            .pending_resolution
            .as_ref()
            .is_some_and(|pending| {
                matches!(
                    pending.continuation,
                    ResolutionContinuation::MassSacrificeGraveyardOrder { .. }
                )
            })
        {
            return Ok(false);
        }
        let pending_player_set_sacrifice =
            self.state
                .pending_resolution
                .as_ref()
                .is_some_and(|pending| {
                    matches!(
                        pending.continuation,
                        ResolutionContinuation::PlayerSetSacrifice { .. }
                            | ResolutionContinuation::TypeChoiceSacrifice { .. }
                    )
                });
        if self.state.pending_observer_return_batch.is_some()
            && self.state.pending_resolution.is_some()
            && !pending_player_set_sacrifice
        {
            return Ok(true);
        }
        if self.state.pending_observer_return_batch.is_none()
            && self.state.pending_immediate_observer_actions.is_empty()
        {
            return Ok(false);
        }
        let prior = self.state.pending_observer_return_batch.take();
        let prior_suspended_resolution = prior
            .as_ref()
            .and_then(|batch| batch.suspended_resolution.clone());
        // A paired observer return can interrupt any already-parked resolution choice. Preserve
        // the complete continuation (including a linked-play cohort's current prompt) so the
        // observer's Aura or timestamp choice can finish before the original choice is restored.
        let active_suspended_resolution = self.state.pending_resolution.take();
        debug_assert!(
            prior_suspended_resolution.is_none() || active_suspended_resolution.is_none(),
            "an observer return batch cannot suspend two resolution prompts"
        );
        let suspended_resolution = prior_suspended_resolution.or(active_suspended_resolution);
        let prior_suspended_replacement_event = prior
            .as_ref()
            .and_then(|batch| batch.suspended_replacement_event.clone());
        let active_suspended_replacement_event = suspended_resolution
            .as_ref()
            .and_then(|_| self.state.pending_replacement_event.take());
        let suspended_replacement_event =
            prior_suspended_replacement_event.or(active_suspended_replacement_event);
        let mut actions = prior
            .as_ref()
            .map_or_else(VecDeque::new, |batch| batch.remaining.clone());
        actions.extend(std::mem::take(
            &mut self.state.pending_immediate_observer_actions,
        ));
        let mut entries = prior
            .as_ref()
            .map_or_else(Vec::new, |batch| batch.ready.clone());
        let resume_stack =
            resume_stack.or_else(|| prior.as_ref().and_then(|batch| batch.resume_stack.clone()));
        while let Some(action) = actions.pop_front() {
            let ImmediateObserverAction::ReturnExiledObject { exiled } = action;
            let generation = self
                .state
                .zone_change_generation
                .get(&exiled.object_id)
                .copied()
                .unwrap_or(0);
            let Some(object) = self.state.objects.get(&exiled.object_id) else {
                continue;
            };
            if object.zone != Zone::Exile || generation != exiled.zone_change_generation {
                continue;
            }
            // CR 111.7: a token that left the battlefield has ceased to exist at the preceding
            // SBA check. Do not recreate one if a synthetic test reaches this boundary earlier.
            if object.is_token() {
                continue;
            }
            let owner = object.owner;
            let label = object_display_name(&self.state, self.registry, exiled.object_id);
            let aura_filter = self.effective_face(exiled.object_id).and_then(|face| {
                face.spell_effect.iter().find_map(|effect| match effect {
                    SpellEffectKind::AuraAttach { target } => Some(target.clone()),
                    _ => None,
                })
            });
            if let Some(filter) = aura_filter {
                let (choice_kind, candidates) = if filter.is_player() {
                    (
                        rv1::ChoiceKind::AuraPlayer,
                        self.state
                            .players
                            .iter()
                            .filter(|player| !player.has_lost)
                            .map(|player| player.id as ObjectId)
                            .filter(|player_id| {
                                super::super::targeting::attachment_filter_legal(
                                    self,
                                    &filter,
                                    AttachmentRecipient::Player(*player_id as PlayerId),
                                    exiled.object_id,
                                    owner,
                                )
                            })
                            .collect::<Vec<_>>(),
                    )
                } else {
                    let mut candidates = self
                        .state
                        .objects
                        .keys()
                        .copied()
                        .filter(|object_id| {
                            super::super::targeting::attachment_filter_legal(
                                self,
                                &filter,
                                AttachmentRecipient::Object(*object_id),
                                exiled.object_id,
                                owner,
                            )
                        })
                        .collect::<Vec<_>>();
                    candidates.sort_unstable();
                    (rv1::ChoiceKind::AuraPermanent, candidates)
                };
                if candidates.is_empty() {
                    continue;
                }
                // Preserve the rest of a simultaneous return cohort. The accepted choice drains
                // it before resuming the original stack instruction.
                self.state.pending_observer_return_batch = Some(PendingObserverReturnBatch {
                    ready: entries,
                    remaining: actions,
                    resume_stack: resume_stack.clone(),
                    suspended_resolution: suspended_resolution.clone(),
                    suspended_replacement_event: suspended_replacement_event.clone(),
                });
                let prompt = format!("Choose what {label} will enchant as it returns.");
                events.push(rv1::RuledEvent {
                    ev: Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(
                        rv1::ResolutionChoiceRequired {
                            resolution_prompt_id: 0,
                            accepted_permanent_selections: Vec::new(),
                            candidate_token_identities: Vec::new(),
                            candidate_player_ids: Vec::new(),
                            deciding_player_id: owner,
                            source_object_id: exiled.object_id,
                            prompt_text: prompt.clone(),
                            choice_kind: choice_kind as i32,
                            candidate_object_ids: candidates.clone(),
                            candidate_card_ids: candidates
                                .iter()
                                .map(|candidate| {
                                    self.state
                                        .objects
                                        .get(candidate)
                                        .map(|object| object.card_id.clone())
                                        .unwrap_or_default()
                                })
                                .collect(),
                            min: 1,
                            max: 1,
                            ordered: false,
                            candidate_names: candidates
                                .iter()
                                .map(|candidate| {
                                    self.state
                                        .objects
                                        .get(candidate)
                                        .map(|_| {
                                            object_display_name(
                                                &self.state,
                                                self.registry,
                                                *candidate,
                                            )
                                        })
                                        .unwrap_or_else(|| format!("P{candidate}"))
                                })
                                .collect(),
                            candidate_server_card_ids: Vec::new(),
                            unique_names: false,
                            candidate_selectable: Vec::new(),
                            public_reveal: None,
                            candidate_source_zones: Vec::new(),
                            ..Default::default()
                        },
                    )),
                });
                events.push(ev_log(prompt.clone()));
                self.state.pending_resolution = Some(PendingResolution {
                    resolution_prompt_id: crate::state::ResolutionPromptId::UNASSIGNED,
                    resolution_prompt_semantics: None,

                    deciding_player: owner,
                    presentation: PendingResolutionPresentation {
                        source_object_id: exiled.object_id,
                        candidates,
                        min: 1,
                        max: 1,
                        ordered: false,
                        prompt,
                        choice_kind,
                        unique_names: false,
                    },
                    continuation: ResolutionContinuation::AuraReturn {
                        stack: resume_stack,
                        exiled,
                    },
                });
                return Ok(true);
            }
            let entry =
                BattlefieldEntryEvent::new(exiled.object_id, owner, self.player_life_snapshot());
            let resume_original_stack = resume_stack.is_some();
            let item = resume_stack
                .as_ref()
                .map(|stack| stack.item.clone())
                .unwrap_or_else(|| self.observer_return_item(exiled.object_id, owner));
            match self.begin_battlefield_entry(
                item,
                entry,
                BattlefieldEntryCompletion::ObserverReturn {
                    owner,
                    object_label: label.clone(),
                    attached_to: None,
                    resume_original_stack,
                },
                events,
            )? {
                super::super::replacement::BattlefieldEntryProgress::Parked => {
                    if let (Some(resume), Some(pending)) = (
                        resume_stack.as_ref(),
                        self.state.pending_resolution.as_mut(),
                    ) {
                        if let Some(parked) = pending.continuation.stack_mut() {
                            parked.restore_execution_from(resume);
                        }
                    }
                    self.state.pending_observer_return_batch = Some(PendingObserverReturnBatch {
                        ready: entries,
                        remaining: actions,
                        resume_stack: resume_stack.clone(),
                        suspended_resolution: suspended_resolution.clone(),
                        suspended_replacement_event: suspended_replacement_event.clone(),
                    });
                    return Ok(true);
                }
                super::super::replacement::BattlefieldEntryProgress::Ready(entry) => {
                    let entry = *entry;
                    entries.push(ObserverReturnEntry {
                        event: entry,
                        owner,
                        label,
                        attached_to: None,
                    });
                }
                super::super::replacement::BattlefieldEntryProgress::Skipped(entry) => {
                    self.restore_skipped_battlefield_entry(&entry)?;
                }
            }
        }
        let Some(batch) = self.begin_entry_timestamp_order(
            SimultaneousEntryBatch::Observer(Box::new(PendingObserverReturnBatch {
                ready: entries,
                remaining: VecDeque::new(),
                resume_stack: resume_stack.clone(),
                suspended_resolution: suspended_resolution.clone(),
                suspended_replacement_event: suspended_replacement_event.clone(),
            })),
            resume_stack.clone(),
            events,
        )?
        else {
            return Ok(true);
        };
        let Some(batch) =
            self.begin_entry_counter_placements_for_batch(batch, resume_stack.clone(), events)?
        else {
            return Ok(true);
        };
        let SimultaneousEntryBatch::Observer(batch) = batch else {
            return Err(EngineError::Illegal("observer entry batch changed type"));
        };
        self.commit_observer_return_batch(batch.ready, events)?;
        if self.restore_suspended_observer_resolution(
            suspended_resolution,
            suspended_replacement_event,
            events,
        )? {
            return Ok(true);
        }
        Ok(false)
    }

    pub(in crate::engine) fn restore_suspended_observer_resolution(
        &mut self,
        suspended_resolution: Option<PendingResolution>,
        suspended_replacement_event: Option<crate::engine::replacement::PendingReplacementEvent>,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<bool, EngineError> {
        let Some(mut suspended_resolution) = suspended_resolution else {
            return Ok(false);
        };
        self.guard_suspended_observer_entry_counter_payload(
            &suspended_resolution,
            suspended_replacement_event.as_ref(),
        )?;
        // Returning from a nested observer choice restores the saved operation, but creates a
        // new externally answerable offer. Keep the exact private projection only as a source for
        // re-publication after its current continuation and generation bindings are revalidated.
        // The fresh zero ID forces the normal publisher to allocate a new correlation ID.
        suspended_resolution.resolution_prompt_id = crate::state::ResolutionPromptId::UNASSIGNED;
        self.state.pending_resolution = Some(suspended_resolution);
        self.state.pending_replacement_event = suspended_replacement_event;
        if self.refresh_parent_copy_targets_after_departure(events)? {
            return Ok(true);
        }
        if self.refresh_suspended_ward_payment(events)? {
            return Ok(true);
        }
        if matches!(
            self.state
                .pending_resolution
                .as_ref()
                .map(|pending| &pending.continuation),
            Some(ResolutionContinuation::PlayerSetSacrifice { .. })
                | Some(ResolutionContinuation::TypeChoiceSacrifice { .. })
        ) {
            self.refresh_player_set_sacrifice_departure(events)?;
            self.refresh_type_choice_sacrifice_departure(events)?;
        }
        let exact_return = self
            .state
            .pending_resolution
            .as_ref()
            .is_some_and(|pending| {
                self.begun_exact_owner_exile_return(
                    pending,
                    self.state.pending_replacement_event.as_ref(),
                )
                .is_some()
            });
        if exact_return {
            if matches!(
                self.state
                    .pending_resolution
                    .as_ref()
                    .map(|pending| &pending.continuation),
                Some(ResolutionContinuation::EntryCounterReplacement { .. })
            ) {
                if self.refresh_exact_return_entry_counters(events)? {
                    return Ok(true);
                }
            } else {
                // Reconcile against the saved receipts before publishing: the entrant or a
                // candidate may have departed or changed incarnation below the observer.
                let event_start = events.len();
                self.refresh_participating_entry_departure(&HashSet::new(), events)?;
                if self.state.pending_resolution.is_some()
                    && !events[event_start..].iter().any(|event| {
                        matches!(
                            &event.ev,
                            Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(_))
                        )
                    })
                {
                    self.republish_cached_resolution_choice(events)?;
                }
                return Ok(true);
            }
        }
        let begun_flat_counters = self
            .state
            .pending_resolution
            .as_ref()
            .is_some_and(|pending| {
                self.begun_combined_counter_placement(
                    pending,
                    self.state.pending_replacement_event.as_ref(),
                ) || self.begun_wolf_counter_placement(
                    pending,
                    self.state.pending_replacement_event.as_ref(),
                )
            });
        if begun_flat_counters {
            // The nested return may have changed recipients and replacement sources while this
            // exact counter transaction waited. Reconcile its saved proposals, never replay it.
            let event_start = events.len();
            self.refresh_counter_placement_departure(events)?;
            if self.state.pending_resolution.is_some()
                && !events[event_start..].iter().any(|event| {
                    matches!(
                        &event.ev,
                        Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(_))
                    )
                })
            {
                self.republish_cached_resolution_choice(events)?;
            }
            return Ok(true);
        }
        #[derive(Clone, Copy)]
        enum SavedProliferateStage {
            OuterReplacement,
            Recipient,
            CounterOrder,
        }
        let saved_proliferate_stage = self.state.pending_resolution.as_ref().and_then(|pending| {
            match &pending.continuation {
                ResolutionContinuation::ProliferateReplacement { .. } => {
                    Some(SavedProliferateStage::OuterReplacement)
                }
                ResolutionContinuation::Proliferate { .. } => {
                    Some(SavedProliferateStage::Recipient)
                }
                ResolutionContinuation::CounterReplacement {
                    stack: Some(_),
                    completion:
                        crate::state::CounterPlacementCompletion::Proliferate { transaction },
                } if matches!(
                    self.state.pending_replacement_event.as_ref(),
                    Some(super::super::replacement::PendingReplacementEvent::Counters(batch))
                        if batch
                            .events
                            .get(batch.event_index)
                            .is_some_and(|event| event.actor == transaction.actor)
                ) =>
                {
                    Some(SavedProliferateStage::CounterOrder)
                }
                _ => None,
            }
        });
        if let Some(saved_stage) = saved_proliferate_stage {
            // Reconcile the restored action at its current typed stage. Outer replacement work
            // can advance into a delegate or recipient offer, a nested counter order can retain
            // its opaque applications, and a saved recipient can advance into the Program tail.
            // Never restart the authored instruction or accept an earlier Aura event as evidence
            // that this current choice was published.
            let event_start = events.len();
            match saved_stage {
                SavedProliferateStage::OuterReplacement => {
                    self.refresh_proliferate_replacement_departure(events)?;
                }
                SavedProliferateStage::Recipient => {
                    self.refresh_proliferate_departure(events)?;
                }
                SavedProliferateStage::CounterOrder => {
                    self.refresh_counter_placement_departure(events)?;
                }
            }

            let current_offer = self.state.pending_resolution.as_ref().map(|pending| {
                (
                    pending.presentation.choice_kind,
                    pending.presentation.source_object_id,
                    pending.deciding_player,
                )
            });
            let current_offer_was_emitted =
                current_offer.is_some_and(|(choice_kind, source_object_id, deciding_player)| {
                    events[event_start..].iter().any(|event| {
                        matches!(
                            event.ev.as_ref(),
                            Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(choice))
                                if choice.choice_kind == choice_kind as i32
                                    && choice.source_object_id == source_object_id
                                    && choice.deciding_player_id == deciding_player
                        )
                    })
                });
            if !current_offer_was_emitted {
                let current_event = match self
                    .state
                    .pending_resolution
                    .as_ref()
                    .map(|pending| &pending.continuation)
                {
                    Some(ResolutionContinuation::StackExitReplacement { .. }) => {
                        self.stack_exit_replacement_choice_event()
                    }
                    Some(ResolutionContinuation::ProliferateReplacement { .. }) => {
                        self.proliferate_replacement_choice_event()
                    }
                    Some(ResolutionContinuation::Proliferate { .. }) => {
                        self.proliferate_choice_event()
                    }
                    Some(ResolutionContinuation::CounterReplacement {
                        stack: Some(_),
                        completion: crate::state::CounterPlacementCompletion::Proliferate { .. },
                    }) => self.current_proliferate_counter_placement_choice_event()?,
                    _ => None,
                };
                if let Some(event) = current_event {
                    events.push(event);
                } else if self.state.pending_resolution.is_some() {
                    self.republish_cached_resolution_choice(events)?;
                }
            }
            return Ok(true);
        }
        if self.reprocess_suspended_battlefield_entry(events)? {
            return Ok(true);
        }
        // A player may have left while the observer's own choice was parked. Rebuild a restored
        // linked-play offer against the surviving cohort now that its original continuation is
        // visible again.
        self.reconcile_pending_linked_exile_after_departure(events)?;
        if self.reconcile_departed_special_cast_resolution(events)? {
            return Ok(true);
        }
        if self.reconcile_pending_player_set_discard_departure(events, true)? {
            return Ok(true);
        }
        if self.reconcile_departed_target_hand_discard_program(events)? {
            return Ok(true);
        }
        if self.reconcile_memory_jar_discard_departure(events)? {
            return Ok(true);
        }
        self.refresh_optional_triggered_ability_departure(events)?;
        if self.refresh_suspended_program_draw_replacement(events)?
            || self.refresh_suspended_program_discard_replacement(events)?
            || self.refresh_suspended_program_choice(events)?
            || self.refresh_suspended_nonprogram_choice(events)?
        {
            return Ok(true);
        }
        if let Some(event) = self.special_cast_commander_choice_event() {
            events.push(event);
        } else if let Some(pending) = self.state.pending_resolution.as_ref() {
            if let ResolutionContinuation::SpecialCast { .. } = &pending.continuation {
                if let Some(event) = self.resolution_payment_choice_event() {
                    events.push(event);
                }
            }
        }
        self.republish_cached_resolution_choice(events)?;
        Ok(true)
    }

    /// Resume the existing draw transaction and rebuild only its currently live replacement
    /// prompt. Its receipts and node cursor may already contain committed work, so do not restart
    /// the authored Draw instruction.
    fn refresh_suspended_program_draw_replacement(
        &mut self,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<bool, EngineError> {
        let Some(pending) = self.state.pending_resolution.as_ref() else {
            return Ok(false);
        };
        let ResolutionContinuation::DrawReplacement { stack: Some(stack) } = &pending.continuation
        else {
            return Ok(false);
        };
        if stack.program().is_none()
            || !matches!(
                self.state.pending_replacement_event,
                Some(crate::engine::replacement::PendingReplacementEvent::Draw(_))
            )
        {
            return Ok(false);
        }
        self.reconcile_draw_departure(events)?;
        Ok(true)
    }

    /// Re-enter an authored chooser leaf that was suspended beneath an observer return.
    /// Choice leaves have not committed a transaction, so rebuilding them is safe and lets their
    /// normal domain handler refresh private candidates, generations, names, and the full prompt.
    /// Player-set discards are reconciled by their dedicated transaction path and are never
    /// restarted here, so an accepted live prefix cannot be lost.
    pub(super) fn refresh_suspended_program_choice(
        &mut self,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<bool, EngineError> {
        let Some(pending) = self.state.pending_resolution.as_ref() else {
            return Ok(false);
        };
        let stack = match &pending.continuation {
            ResolutionContinuation::HandChoice { stack, hand_choice }
                if stack.program().is_some()
                    && (hand_choice.program_selection
                        || self.is_target_hand_discard_program(stack)) =>
            {
                stack.clone()
            }
            ResolutionContinuation::PermanentChoice { stack, .. } if stack.program().is_some() => {
                stack.clone()
            }
            ResolutionContinuation::BeholdChoice { stack, .. } if stack.program().is_some() => {
                stack.clone()
            }
            _ => return Ok(false),
        };
        let Some(program) = stack.program() else {
            return Ok(false);
        };
        let target_hand_discard_target = match &pending.continuation {
            ResolutionContinuation::HandChoice { stack, hand_choice }
                if !hand_choice.program_selection && self.is_target_hand_discard_program(stack) =>
            {
                Some(hand_choice.affected_player)
            }
            _ => None,
        };
        if target_hand_discard_target.is_some_and(|target| {
            self.state
                .player_idx(target)
                .is_none_or(|index| self.state.players[index].has_lost)
        }) {
            self.state.pending_resolution = None;
            self.state.pending_replacement_event = None;
            let resumed = self.complete_parked_stack_with_previous(
                stack,
                OperationResults::default(),
                Vec::new(),
            )?;
            events.extend(resumed.events);
            return Ok(true);
        }
        let effect = self.current_program_perform(program)?;
        let chooser_leaf = match &pending.continuation {
            ResolutionContinuation::HandChoice { hand_choice, .. }
                if hand_choice.program_selection =>
            {
                matches!(effect, SpellEffectKind::SelectHandCards { .. })
            }
            ResolutionContinuation::HandChoice { stack, hand_choice }
                if !hand_choice.program_selection && self.is_target_hand_discard_program(stack) =>
            {
                matches!(effect, SpellEffectKind::ChooseHandCards { .. })
            }
            ResolutionContinuation::PermanentChoice { .. } => matches!(
                effect,
                SpellEffectKind::ChoosePermanents {
                    chooser: tricerules_card_model::primitives::PlayerRecipient::Controller,
                    ..
                } | SpellEffectKind::ChooseProliferatedPermanents
            ),
            ResolutionContinuation::BeholdChoice { .. } => {
                matches!(effect, SpellEffectKind::MayBehold { .. })
            }
            _ => false,
        };
        if !chooser_leaf {
            return Err(EngineError::ProgramInvariant(
                "restored program choice does not match its in-flight chooser leaf",
            ));
        }
        let Some(frame) = program.frames.last() else {
            return Err(EngineError::ProgramInvariant(
                "restored program choice lost its active frame",
            ));
        };
        let next_instruction =
            frame
                .next_instruction
                .checked_sub(1)
                .ok_or(EngineError::ProgramInvariant(
                    "restored program chooser cursor is invalid",
                ))?;
        let mut retry = stack;
        retry
            .retry_program_instruction(next_instruction)
            .ok_or(EngineError::ProgramInvariant(
                "restored program choice lost its active frame",
            ))?;

        self.state.pending_resolution = None;
        self.state.pending_replacement_event = None;
        let resumed = self.complete_parked_stack_with_previous(
            retry,
            OperationResults::default(),
            Vec::new(),
        )?;
        events.extend(resumed.events);
        Ok(true)
    }

    /// Re-evaluate a restored non-program choice leaf whose candidate cohort depends on current
    /// battlefield state. Generation equality alone cannot make a cached offer safe after an
    /// object left or lost eligibility. Rewind exactly the current effect in the parked standard
    /// list and keep its already-completed prefix and previous-effect result.
    fn refresh_suspended_nonprogram_choice(
        &mut self,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<bool, EngineError> {
        if self
            .state
            .pending_resolution
            .as_ref()
            .is_some_and(|pending| {
                matches!(
                    pending.continuation,
                    ResolutionContinuation::ChosenCounterAddition { .. }
                )
            })
        {
            let event_start = events.len();
            self.refresh_chosen_counter_addition_departure(events)?;
            let has_refreshed_choice = events[event_start..].iter().any(|event| {
                matches!(
                    &event.ev,
                    Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(_))
                )
            });
            if !has_refreshed_choice
                && self
                    .state
                    .pending_resolution
                    .as_ref()
                    .is_some_and(|pending| {
                        matches!(
                            pending.continuation,
                            ResolutionContinuation::ChosenCounterAddition { .. }
                        )
                    })
            {
                if let Some(choice) = self.chosen_counter_addition_choice_event() {
                    events.push(choice);
                }
            }
            return Ok(true);
        }
        if self
            .state
            .pending_resolution
            .as_ref()
            .is_some_and(|pending| {
                matches!(
                    &pending.continuation,
                    ResolutionContinuation::Proliferate { .. }
                )
            })
        {
            // Reconcile the saved recipient transaction in place. Re-entering the recipient leaf
            // would rebuild the action root and discard committed receipts or its cursor.
            let refresh_event_start = events.len();
            self.refresh_proliferate_departure(events)?;
            let current_offer = self.state.pending_resolution.as_ref().and_then(|pending| {
                matches!(
                    &pending.continuation,
                    ResolutionContinuation::Proliferate { .. }
                )
                .then_some((
                    pending.presentation.choice_kind,
                    pending.presentation.source_object_id,
                    pending.deciding_player,
                ))
            });
            if let Some((choice_kind, source_object_id, deciding_player)) = current_offer {
                let current_offer_was_emitted = events[refresh_event_start..].iter().any(|event| {
                    matches!(
                        event.ev.as_ref(),
                        Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(choice))
                            if choice.choice_kind == choice_kind as i32
                                && choice.source_object_id == source_object_id
                                && choice.deciding_player_id == deciding_player
                    )
                });
                if !current_offer_was_emitted {
                    if let Some(event) = self.proliferate_choice_event() {
                        events.push(event);
                    }
                }
            }
            return Ok(true);
        }
        #[derive(Clone, Copy)]
        enum ChooserLeaf {
            ChoosePermanents,
            Proliferate,
            Populate,
            Amass,
            Blight,
            TargetPlayerSacrifices,
        }

        let Some(pending) = self.state.pending_resolution.as_ref() else {
            return Ok(false);
        };
        let (stack, expected_leaf) = match &pending.continuation {
            ResolutionContinuation::PermanentChoice { stack, .. } if stack.program().is_none() => {
                (stack.clone(), ChooserLeaf::ChoosePermanents)
            }
            ResolutionContinuation::Proliferate { stack, .. } if stack.program().is_none() => {
                (stack.clone(), ChooserLeaf::Proliferate)
            }
            ResolutionContinuation::Populate { stack, .. } if stack.program().is_none() => {
                (stack.clone(), ChooserLeaf::Populate)
            }
            ResolutionContinuation::AmassChoice { stack, .. } if stack.program().is_none() => {
                (stack.clone(), ChooserLeaf::Amass)
            }
            ResolutionContinuation::Blight { stack, .. } if stack.program().is_none() => {
                (stack.clone(), ChooserLeaf::Blight)
            }
            ResolutionContinuation::Sacrifice { stack } if stack.program().is_none() => {
                (stack.clone(), ChooserLeaf::TargetPlayerSacrifices)
            }
            _ => return Ok(false),
        };
        if !self.resolution_prompt_should_be_active(pending) {
            return Ok(false);
        }
        if self
            .state
            .player_idx(stack.item.controller)
            .is_none_or(|index| self.state.players[index].has_lost)
        {
            return Err(EngineError::ProgramInvariant(
                "restored choice belongs to a departed resolution controller",
            ));
        }
        let leaf_index = stack
            .resume_effect_index()
            .and_then(|next| next.checked_sub(1))
            .ok_or(EngineError::ProgramInvariant(
                "restored choice has no current effect index",
            ))?;
        let (mut effects, spell_label) = self.build_resolution_effects(&stack.item);
        let expected_leaf_matches = |effect: &SpellEffectKind| match expected_leaf {
            ChooserLeaf::ChoosePermanents => {
                matches!(effect, SpellEffectKind::ChoosePermanents { .. })
            }
            ChooserLeaf::Proliferate => matches!(effect, SpellEffectKind::Proliferate),
            ChooserLeaf::Populate => matches!(effect, SpellEffectKind::Populate),
            ChooserLeaf::Amass => matches!(effect, SpellEffectKind::Amass { .. }),
            ChooserLeaf::Blight => matches!(effect, SpellEffectKind::Blight { .. }),
            ChooserLeaf::TargetPlayerSacrifices => {
                matches!(effect, SpellEffectKind::TargetPlayerSacrifices { .. })
            }
        };
        let Some(current_effect) = effects.get(leaf_index as usize) else {
            return Err(EngineError::ProgramInvariant(
                "restored non-program choice does not point to its current chooser leaf",
            ));
        };
        let mut entered_effect = &current_effect.effect;
        while let SpellEffectKind::Conditional { effect, .. }
        | SpellEffectKind::ConditionalCastCost { effect, .. } = entered_effect
        {
            entered_effect = effect;
        }
        if !expected_leaf_matches(entered_effect) {
            return Err(EngineError::ProgramInvariant(
                "restored non-program choice does not point to its current chooser leaf",
            ));
        }
        let entered_effect = entered_effect.clone();
        effects[leaf_index as usize].effect = entered_effect;

        let previous_result = stack.previous_result().clone();
        let mut retry = stack;
        retry.set_flat_tail(Some(leaf_index), retry.previous_result().clone());
        self.state.pending_resolution = None;
        self.state.pending_replacement_event = None;
        let resume_effect_index = retry.resume_effect_index();
        let resumed = self.complete_parked_resolution_with_effects_and_previous(
            retry.item,
            resume_effect_index,
            effects,
            spell_label,
            previous_result,
            Vec::new(),
        )?;
        events.extend(resumed.events);
        Ok(true)
    }

    /// Re-run a suspended as-enters transaction after observer choices. This rebuilds the live
    /// prompt from its saved event and currently applicable replacement effects, instead of
    /// restoring a pending choice whose candidates or entrant may have become stale.
    fn reprocess_suspended_battlefield_entry(
        &mut self,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<bool, EngineError> {
        let Some(pending) = self.state.pending_resolution.clone() else {
            return Ok(false);
        };
        if !matches!(
            pending.continuation,
            ResolutionContinuation::EntryCopySource { .. }
                | ResolutionContinuation::EntryAuraRecipient { .. }
                | ResolutionContinuation::EntryReplacement { .. }
                | ResolutionContinuation::EntryCost { .. }
                | ResolutionContinuation::EntryReveal { .. }
                | ResolutionContinuation::EntryBasicLandType { .. }
                | ResolutionContinuation::EntryChooseOpponent { .. }
                | ResolutionContinuation::EntryChooseCreatureType { .. }
                | ResolutionContinuation::SagaReadAhead { .. }
                | ResolutionContinuation::BattleProtector { .. }
        ) {
            return Ok(false);
        }
        let Some(replacement_event) = self.state.pending_replacement_event.take() else {
            return Ok(false);
        };
        let crate::engine::replacement::PendingReplacementEvent::BattlefieldEntry(entry) =
            replacement_event
        else {
            self.state.pending_replacement_event = Some(replacement_event);
            return Ok(false);
        };
        let mut entry = *entry;
        let Some(stack) = pending.continuation.stack().cloned() else {
            self.state.pending_replacement_event = Some(
                crate::engine::replacement::PendingReplacementEvent::BattlefieldEntry(Box::new(
                    entry,
                )),
            );
            return Ok(false);
        };
        let linked_current = self
            .state
            .pending_linked_exile_play
            .as_ref()
            .filter(|cohort| {
                cohort
                    .in_flight
                    .is_some_and(|current| current.object_id == entry.event.object_id)
            })
            .and_then(|cohort| {
                cohort
                    .in_flight
                    .map(|current| (cohort.stack.item.controller, current))
            });
        let linked_entry_is_current = linked_current.is_none_or(|(chooser, current)| {
            let object_current = self
                .state
                .objects
                .get(&current.object_id)
                .is_some_and(|object| object.zone == Zone::Exile);
            let generation_current = self
                .state
                .zone_change_generation
                .get(&current.object_id)
                .copied()
                == Some(current.zone_change_generation);
            let chooser_live = self
                .state
                .player_idx(chooser)
                .is_some_and(|index| !self.state.players[index].has_lost);
            object_current
                && generation_current
                && chooser_live
                && entry.event.destination_controller == chooser
        });
        if !self.state.objects.contains_key(&entry.event.object_id) || !linked_entry_is_current {
            self.state.pending_resolution = None;
            self.state.pending_replacement_event = None;
            let matching_linked_entry =
                self.state
                    .pending_linked_exile_play
                    .as_ref()
                    .is_some_and(|cohort| {
                        cohort
                            .in_flight
                            .is_some_and(|current| current.object_id == entry.event.object_id)
                    });
            if matching_linked_entry {
                let cohort = self
                    .state
                    .pending_linked_exile_play
                    .take()
                    .expect("matching linked entry checked above");
                let resumed = self.advance_pending_linked_exile_play(*cohort, Vec::new())?;
                events.extend(resumed.events);
                return Ok(true);
            }
            // The stale event can never commit. Avoid rolling its provisional copy onto a newer
            // incarnation that reused the object ID.
            entry.event.pending_copy_candidate = None;
            entry.event.pending_aura_recipient = None;
            entry.event.accepted_aura_recipient = None;
            let resumed = self.finish_entry_copy_without_recipient(
                stack,
                entry.event,
                entry.completion,
                Vec::new(),
            )?;
            events.extend(resumed.events);
            return Ok(true);
        }

        // An Aura recipient prompt stores a prospective copy receipt on the event. Return that
        // exact receipt to the entry pipeline so it can recompute legal recipients and republish
        // the prompt without granting a new zone incarnation.
        if let Some(recipient) = entry.event.pending_aura_recipient.take() {
            if entry.event.pending_copy_candidate.is_none() {
                entry.event.pending_copy_candidate = recipient.copy_candidate;
            }
        }
        let resumed_prompt = match &pending.continuation {
            ResolutionContinuation::EntryBasicLandType { .. } => {
                self.resumed_basic_land_type_choice_event(&pending, &entry)
            }
            ResolutionContinuation::EntryCost { .. } => {
                self.resumed_entry_cost_choice_event(&pending, &entry)
            }
            _ => None,
        };
        if let Some(prompt) = resumed_prompt {
            self.state.pending_resolution = Some(pending);
            self.state.pending_replacement_event = Some(
                crate::engine::replacement::PendingReplacementEvent::BattlefieldEntry(Box::new(
                    entry,
                )),
            );
            events.push(prompt);
            return Ok(true);
        }
        self.state.pending_resolution = None;
        self.state.pending_replacement_event = None;
        let completion = entry.completion;
        let progress = self.advance_or_park_battlefield_entry(
            stack.item.clone(),
            entry.event,
            completion.clone(),
            events,
        );
        match progress {
            super::super::replacement::BattlefieldEntryProgress::Parked => {
                self.transfer_entry_choice_resume(&stack);
            }
            super::super::replacement::BattlefieldEntryProgress::Skipped(event) => {
                let resumed = self.finish_entry_copy_without_recipient(
                    stack,
                    *event,
                    completion,
                    Vec::new(),
                )?;
                events.extend(resumed.events);
            }
            super::super::replacement::BattlefieldEntryProgress::Ready(event) => {
                let resumed = self.complete_pending_battlefield_entry(
                    pending,
                    *event,
                    completion,
                    Vec::new(),
                )?;
                events.extend(resumed.events);
            }
        }
        Ok(true)
    }

    pub(in crate::engine) fn commit_observer_return_batch(
        &mut self,
        mut entries: Vec<ObserverReturnEntry>,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<(), EngineError> {
        self.prune_invalid_observer_entry_auras(&mut entries)?;
        if entries.iter().any(|entry| {
            !entry.event.entry_counter_placements.is_empty()
                && !entry.event.entry_counter_replacements_resolved
        }) {
            return Err(EngineError::Illegal(
                "observer-entry counters were not processed by the replacement planner",
            ));
        }
        let mut trigger_events = Vec::new();
        for entry in entries {
            let object_id = entry.event.object_id;
            let chosen_x = entry.event.chosen_x;
            let entry_cause = entry.event.entry_cause.clone();
            // Observer returns are independent one-shot effects. Entry replacement ordering is
            // added in the Aura/choice increment; the base path still uses the canonical commit
            // reset and static-registration machinery.
            let recipient = entry.event.attached_to.or(entry.attached_to);
            let door_event = self.commit_battlefield_entry_state(entry.event, recipient)?;
            trigger_events.push(GameEvent::EntersBattlefield {
                object_id,
                chosen_x,
                entry_cause,
            });
            trigger_events.extend(door_event);
            events.push(permanent_moved_event(
                &self.state,
                object_id,
                entry.owner,
                rv1::permanent_moved::Destination::Battlefield,
            ));
            if let Some(recipient) = recipient {
                events.push(rv1::RuledEvent {
                    ev: Some(rv1::ruled_event::Ev::AuraAttached(rv1::AuraAttached {
                        aura_object_id: object_id,
                        attachment_recipient: Some(attachment_recipient_proto(recipient)),
                    })),
                });
            }
            events.push(ev_log(format!(
                "{} returns to the battlefield under its owner's control.",
                entry.label
            )));
        }
        self.fire_triggers(&trigger_events, events);
        Ok(())
    }

    pub(in crate::engine) fn refresh_observer_aura_departure(
        &mut self,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<(), EngineError> {
        let Some(pending) = self.state.pending_resolution.as_ref() else {
            return Ok(());
        };
        let ResolutionContinuation::AuraReturn { stack, exiled } = &pending.continuation else {
            return Ok(());
        };
        let decider_live = self
            .state
            .player_idx(pending.deciding_player)
            .is_some_and(|index| !self.state.players[index].has_lost);
        let recipients_live = pending.presentation.candidates.iter().all(|oid| {
            if pending.presentation.choice_kind == rv1::ChoiceKind::AuraPlayer {
                self.state
                    .player_idx(*oid as PlayerId)
                    .is_some_and(|index| !self.state.players[index].has_lost)
            } else {
                self.state
                    .objects
                    .get(oid)
                    .is_some_and(|object| object.zone == Zone::Battlefield)
            }
        });
        if decider_live && recipients_live {
            return Ok(());
        }
        let stack = stack.clone();
        let exiled = *exiled;
        self.state.pending_resolution = None;
        let batch = self
            .state
            .pending_observer_return_batch
            .get_or_insert_with(|| PendingObserverReturnBatch {
                ready: Vec::new(),
                remaining: VecDeque::new(),
                resume_stack: stack.clone(),
                suspended_resolution: None,
                suspended_replacement_event: None,
            });
        if decider_live {
            // The current action was popped before parking. Refresh it through the normal
            // chooser path without losing the ready prefix or later independent returns.
            batch
                .remaining
                .push_front(ImmediateObserverAction::ReturnExiledObject { exiled });
        }
        let mut resumed = Vec::new();
        if !self.drain_immediate_observer_actions(stack.clone(), &mut resumed)? {
            if let Some(stack) = stack {
                let previous_result = stack.previous_result().clone();
                resumed = self
                    .complete_parked_stack_with_previous(stack, previous_result, resumed)?
                    .events;
            }
        }
        events.extend(resumed);
        Ok(())
    }

    pub(in crate::engine) fn observer_return_item(
        &self,
        object_id: ObjectId,
        controller: PlayerId,
    ) -> StackItem {
        let card_id = self
            .state
            .objects
            .get(&object_id)
            .map(|object| object.card_id.clone())
            .unwrap_or_default();
        StackItem {
            phyrexian_symbols_paid_with_life: 0,
            mana_colors_spent_to_cast: Default::default(),
            id: object_id,
            controller,
            card_id,
            prototyped: false,
            targets: Vec::new(),
            ability_text: Some("paired one-shot return".into()),
            source_permanent_id: None,
            source_owner: None,
            source_zone_change: 0,
            trigger_use_key: None,
            source_face_change: 0,
            ability_index: None,
            activated_ability: None,
            activated_ability_occurrence: None,
            triggered_ability: None,
            is_triggered: false,
            is_copy: true,
            face_index: 0,
            cast_method: SpellCastMethod::Normal,
            returned_attacker_assignment: None,
            chosen_x: 0,
            chosen_modes: Vec::new(),
            cast_condition_results: Vec::new(),
            cast_occurrence: None,
            cast_by: None,
            cast_cost_receipts: Vec::new(),
            payment_result: CardResultCohort::default(),
            exiled_cohorts: Default::default(),
            chaos_warp_owner_instructions: Default::default(),
            resolution_branch_choices: BTreeMap::new(),
            blight_receipts: Vec::new(),
            trigger_context: TriggerContext::default(),
        }
    }
}
