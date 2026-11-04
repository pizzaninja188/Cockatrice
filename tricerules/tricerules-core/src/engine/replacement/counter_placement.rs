//! Counter placement replacement planning, choices, and commit.

use super::*;

impl GameEngine {
    /// Identify an already-started exact Exile-to-owner return by its captured operation and
    /// entry receipts. This deliberately does not inspect the current object: its owner may
    /// already have left, or the object ID may now denote a later incarnation.
    pub(in crate::engine) fn begun_exact_owner_exile_return(
        &self,
        pending: &PendingResolution,
        replacement: Option<&PendingReplacementEvent>,
    ) -> Option<TriggerObjectRef> {
        let stack = pending.continuation.stack()?;
        if !stack.item.is_triggered || stack.item.triggered_ability.is_none() {
            return None;
        }
        let observed = stack.item.trigger_context.observed_object?;
        let effect = if let Some(program) = stack.program() {
            if !program.in_flight.as_ref().is_some_and(Vec::is_empty) {
                return None;
            }
            self.current_program_perform(program).ok()?
        } else {
            let index = stack.resume_effect_index()?.checked_sub(1)? as usize;
            self.build_resolution_effects(&stack.item)
                .0
                .get(index)?
                .effect
                .clone()
        };
        if !matches!(effect, SpellEffectKind::ReturnTriggeredCard {
            reference: TriggeredCardReference::ExactTriggerObject,
            ref from,
            destination: tricerules_card_model::primitives::TriggeredCardDestination::Battlefield,
            controller: ReturnController::Owner,
            ..
        } if from.as_slice() == [tricerules_card_model::primitives::EventZone::Exile])
        {
            return None;
        }

        let (event, completion) = match (&pending.continuation, replacement?) {
            (
                ResolutionContinuation::EntryCopySource {
                    entering_zone,
                    entering_generation,
                    ..
                },
                PendingReplacementEvent::BattlefieldEntry(entry),
            ) if *entering_zone == Zone::Exile
                && *entering_generation == observed.zone_change_generation =>
            {
                (&entry.event, &entry.completion)
            }
            (
                ResolutionContinuation::EntryAuraRecipient { .. },
                PendingReplacementEvent::BattlefieldEntry(entry),
            ) if entry
                .event
                .pending_aura_recipient
                .as_ref()
                .is_some_and(|choice| {
                    choice.entering_zone_generation == observed.zone_change_generation
                }) =>
            {
                (&entry.event, &entry.completion)
            }
            (
                ResolutionContinuation::EntryReveal {
                    entering_zone,
                    entering_generation,
                    ..
                }
                | ResolutionContinuation::EntryChooseOpponent {
                    entering_zone,
                    entering_generation,
                    ..
                }
                | ResolutionContinuation::EntryChooseCreatureType {
                    entering_zone,
                    entering_generation,
                    ..
                },
                PendingReplacementEvent::BattlefieldEntry(entry),
            ) if *entering_zone == Zone::Exile
                && *entering_generation == observed.zone_change_generation =>
            {
                (&entry.event, &entry.completion)
            }
            (
                ResolutionContinuation::EntryReplacement { .. }
                | ResolutionContinuation::EntryCost { .. }
                | ResolutionContinuation::EntryBasicLandType { .. }
                | ResolutionContinuation::SagaReadAhead { .. }
                | ResolutionContinuation::BattleProtector { .. },
                PendingReplacementEvent::BattlefieldEntry(entry),
            ) => (&entry.event, &entry.completion),
            (
                ResolutionContinuation::EntryCounterReplacement { resume },
                PendingReplacementEvent::Counters(batch),
            ) => {
                let crate::state::EntryCounterReplacementResume::Single {
                    event, completion, ..
                } = resume.as_ref()
                else {
                    return None;
                };
                if batch.events.is_empty() || batch.event_index >= batch.events.len()
                    || !batch.events.iter().all(|proposal| {
                        matches!(&proposal.recipient, CounterPlacementRecipient::EnteringObject {
                            object_id, zone_change_generation,
                        } if *object_id == observed.object_id && *zone_change_generation == observed.zone_change_generation)
                        && proposal.entry_context.as_ref().is_some_and(|context|
                            context.source_zone == Zone::Exile
                                && context.source_zone_change_generation == observed.zone_change_generation
                                && context.destination_controller == event.destination_controller)
                    }) {
                    return None;
                }
                (event.as_ref(), completion)
            }
            _ => return None,
        };
        let BattlefieldEntryCompletion::ResolutionEffect {
            owner,
            from_zone,
            triggered_return_generation,
            ..
        } = completion
        else {
            return None;
        };
        (event.object_id == observed.object_id
            && *from_zone == Zone::Exile
            && *triggered_return_generation == Some(observed.zone_change_generation)
            && event.destination_controller == *owner
            && event.deciding_player == *owner
            && pending.deciding_player == *owner)
            .then_some(observed)
    }

    pub(super) fn exact_owner_return_entrant_is_current(
        &self,
        observed: TriggerObjectRef,
        owner: PlayerId,
    ) -> bool {
        self.state
            .objects
            .get(&observed.object_id)
            .is_some_and(|object| object.owner == owner && object.zone == Zone::Exile)
            && self
                .state
                .zone_change_generation
                .get(&observed.object_id)
                .copied()
                .unwrap_or(0)
                == observed.zone_change_generation
            && self
                .state
                .player_idx(owner)
                .is_some_and(|index| !self.state.players[index].has_lost)
    }

    fn recognized_departed_exact_return_entry_counter(
        &self,
        pending: &PendingResolution,
        replacement: &PendingReplacementEvent,
    ) -> Option<TriggerObjectRef> {
        if !matches!(replacement, PendingReplacementEvent::Counters(_)) {
            return None;
        }
        let observed = self.begun_exact_owner_exile_return(pending, Some(replacement))?;
        let ResolutionContinuation::EntryCounterReplacement { resume } = &pending.continuation
        else {
            return None;
        };
        let crate::state::EntryCounterReplacementResume::Single { completion, .. } =
            resume.as_ref()
        else {
            return None;
        };
        let BattlefieldEntryCompletion::ResolutionEffect { owner, .. } = completion else {
            return None;
        };
        (!self.exact_owner_return_entrant_is_current(observed, *owner)).then_some(observed)
    }

    /// Validate incoming observer-restoration counters before installing the suspended state.
    /// A recognized exact owner return whose entrant or owner has departed is left to the
    /// existing finish/skip reconciliation path.
    pub(in crate::engine) fn guard_suspended_observer_entry_counter_payload(
        &self,
        pending: &PendingResolution,
        replacement: Option<&PendingReplacementEvent>,
    ) -> Result<(), EngineError> {
        let (
            ResolutionContinuation::EntryCounterReplacement { resume },
            Some(PendingReplacementEvent::Counters(batch)),
        ) = (&pending.continuation, replacement)
        else {
            return Ok(());
        };
        if self
            .recognized_departed_exact_return_entry_counter(pending, replacement.unwrap())
            .is_none()
        {
            self.validate_entry_counter_replacement_payload(pending, resume, batch)?;
        }
        Ok(())
    }

    /// Reconcile a single entry-counter transaction without restarting already-applied
    /// replacements. This also runs when an independent observer restores the saved prompt.
    pub(in crate::engine) fn refresh_single_entry_counters(
        &mut self,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<bool, EngineError> {
        let Some(pending) = self.state.pending_resolution.clone() else {
            return Ok(false);
        };
        let Some(replacement) = self.state.pending_replacement_event.clone() else {
            return Ok(false);
        };
        self.guard_suspended_observer_entry_counter_payload(&pending, Some(&replacement))?;
        let observed = self.begun_exact_owner_exile_return(&pending, Some(&replacement));
        let opening = self.state.opening.is_some() && self.pending_opening_hand_entry().is_some();
        if observed.is_none() && !opening {
            return Ok(false);
        }
        let ResolutionContinuation::EntryCounterReplacement { resume } = &pending.continuation
        else {
            return Ok(false);
        };
        let crate::state::EntryCounterReplacementResume::Single {
            stack,
            event,
            completion,
        } = resume.as_ref()
        else {
            return Ok(false);
        };
        let PendingReplacementEvent::Counters(mut batch) = replacement else {
            return Ok(false);
        };
        if opening {
            self.validate_pending_opening_hand_entry()?;
            self.validate_entry_counter_replacement_payload(&pending, resume, &batch)?;
        }
        let stack = stack.clone();
        let mut event = (**event).clone();
        let completion = completion.clone();
        let owner = event.destination_controller;
        if observed
            .is_some_and(|observed| !self.exact_owner_return_entrant_is_current(observed, owner))
        {
            self.state.pending_resolution = None;
            self.state.pending_replacement_event = None;
            // A newer incarnation must never receive this old entry's provisional copy state.
            event.pending_copy_candidate = None;
            event.pending_aura_recipient = None;
            event.accepted_aura_recipient = None;
            let completed =
                self.finish_entry_copy_without_recipient(stack, event, completion, Vec::new())?;
            events.extend(completed.events);
            return Ok(true);
        }

        let event_index = batch.event_index;
        if batch
            .applications
            .iter()
            .any(|application| application.event_index != event_index)
        {
            return Err(EngineError::ProgramInvariant(
                "entry-counter replacement applications do not match the saved anchor",
            ));
        }
        let candidates = self.counter_placement_candidates_for_batch(&batch.events, event_index);
        let old: Vec<_> = batch
            .applications
            .iter()
            .map(|application| application.replacement.clone())
            .collect();
        let current: Vec<_> = candidates
            .iter()
            .map(|(candidate, _)| candidate.clone())
            .collect();
        if old == current {
            return Ok(false);
        }
        self.state.pending_resolution = None;
        self.state.pending_replacement_event = None;
        batch.applications.clear();
        if let Some(candidates) = self.advance_counter_placement_batch(&mut batch)? {
            self.park_entry_counter_placement_choice(
                *batch,
                candidates,
                crate::state::EntryCounterReplacementResume::Single {
                    stack,
                    event: Box::new(event),
                    completion,
                },
                events,
            );
        } else {
            Self::apply_entry_counter_placement_results(&mut event, &batch.events)?;
            let completed =
                self.complete_pending_battlefield_entry(pending, event, completion, Vec::new())?;
            events.extend(completed.events);
        }
        Ok(true)
    }

    pub(in crate::engine) fn proposed_object_counter_placement(
        &self,
        actor: PlayerId,
        object_id: ObjectId,
        counters: impl IntoIterator<Item = (CounterKind, u32)>,
        origin: super::super::continuous::CounterPlacementOrigin,
    ) -> Option<ProposedCounterPlacement> {
        if !self.can_receive_counters(object_id)
            || !self
                .state
                .objects
                .get(&object_id)
                .is_some_and(|object| object.zone == Zone::Battlefield)
        {
            return None;
        }
        let mut counter_bag: BTreeMap<CounterKind, u32> = BTreeMap::new();
        for (kind, count) in counters {
            if count > 0 {
                let total = counter_bag.entry(kind).or_insert(0);
                *total = (*total).saturating_add(count);
            }
        }
        if counter_bag.is_empty() {
            return None;
        }
        let original_counters = counter_bag.clone();
        Some(ProposedCounterPlacement {
            recipient: CounterPlacementRecipient::Object {
                object_id,
                zone_change_generation: self
                    .state
                    .zone_change_generation
                    .get(&object_id)
                    .copied()
                    .unwrap_or(0),
            },
            actor,
            original_counters,
            counters: counter_bag,
            origin,
            entry_source: None,
            entry_context: None,
            counter_move_context: None,
            cost_debit_index: None,
            effect_created_counters: origin
                == super::super::continuous::CounterPlacementOrigin::Effect,
            applied_replacements: Vec::new(),
        })
    }

    pub(in crate::engine) fn proposed_player_counter_placement(
        &self,
        actor: PlayerId,
        player_id: PlayerId,
        counters: impl IntoIterator<Item = (CounterKind, u32)>,
        origin: super::super::continuous::CounterPlacementOrigin,
    ) -> Option<ProposedCounterPlacement> {
        if self
            .state
            .player_idx(player_id)
            .is_none_or(|index| self.state.players[index].has_lost)
        {
            return None;
        }
        let mut counter_bag: BTreeMap<CounterKind, u32> = BTreeMap::new();
        for (kind, count) in counters {
            if count > 0 {
                let total = counter_bag.entry(kind).or_insert(0);
                *total = (*total).saturating_add(count);
            }
        }
        if counter_bag.is_empty() {
            return None;
        }
        let original_counters = counter_bag.clone();
        Some(ProposedCounterPlacement {
            recipient: CounterPlacementRecipient::Player { player_id },
            actor,
            original_counters,
            counters: counter_bag,
            origin,
            entry_source: None,
            entry_context: None,
            counter_move_context: None,
            cost_debit_index: None,
            effect_created_counters: origin
                == super::super::continuous::CounterPlacementOrigin::Effect,
            applied_replacements: Vec::new(),
        })
    }

    fn entry_counter_recipient_can_receive(
        &self,
        object_id: ObjectId,
        cohort: &[&BattlefieldEntryEvent],
    ) -> bool {
        for (&source_id, object) in &self.state.objects {
            if object.zone != Zone::Battlefield
                || object.phase_status.is_some()
                || !super::super::characteristics::printed_static_source_is_available(
                    &self.state,
                    self.registry,
                    source_id,
                )
            {
                continue;
            }
            for ability in self.active_static_ability_definitions(source_id) {
                match ability {
                    StaticAbilityDef::ProhibitCounters {
                        affected: tricerules_card_model::primitives::CounterPlacementAffected::Self_,
                    } if source_id == object_id => return false,
                    StaticAbilityDef::ProhibitCounters {
                        affected: tricerules_card_model::primitives::CounterPlacementAffected::AttachedPermanent,
                    } if object.attached_to == Some(AttachmentRecipient::Object(object_id)) => {
                        return false;
                    }
                    _ => {}
                }
            }
        }

        for entry in cohort {
            if entry.object_id == object_id {
                if self
                    .state
                    .objects
                    .get(&entry.object_id)
                    .is_some_and(|object| object.face_down)
                {
                    continue;
                }
                if self.battlefield_entry_face(entry).is_some_and(|face| {
                    face.static_abilities.iter().any(|ability| {
                        matches!(
                            &ability.definition,
                            StaticAbilityDef::ProhibitCounters {
                                affected: tricerules_card_model::primitives::CounterPlacementAffected::Self_
                            }
                        )
                    })
                }) {
                    return false;
                }
            } else if entry.attached_to == Some(AttachmentRecipient::Object(object_id))
                && !self
                    .state
                    .objects
                    .get(&entry.object_id)
                    .is_some_and(|object| object.face_down)
                && self.battlefield_entry_face(entry).is_some_and(|face| {
                    face.static_abilities.iter().any(|ability| {
                        matches!(
                            &ability.definition,
                            StaticAbilityDef::ProhibitCounters {
                                affected: tricerules_card_model::primitives::CounterPlacementAffected::AttachedPermanent
                            }
                        )
                    })
                })
            {
                return false;
            }
        }
        true
    }

    pub(super) fn proposed_entry_counter_placements(
        &self,
        entries: &[&BattlefieldEntryEvent],
        excluded_replacement_sources: &[(ObjectId, u64)],
    ) -> Vec<ProposedCounterPlacement> {
        let mut proposals = Vec::new();
        for event in entries {
            if event.entry_counter_replacements_resolved {
                continue;
            }
            let Some(object) = self.state.objects.get(&event.object_id) else {
                continue;
            };
            let generation = self
                .state
                .zone_change_generation
                .get(&event.object_id)
                .copied()
                .unwrap_or(0);
            let compleated = self.entry_compleated_replacement(event);
            let projected = self.battlefield_entry_characteristics_through_layer_5(event);
            for (placement_index, placement) in event.entry_counter_placements.iter().enumerate() {
                let original_counters = placement.counters.clone();
                if original_counters.is_empty() {
                    continue;
                }
                let history =
                    placement
                        .history
                        .clone()
                        .unwrap_or_else(|| EntryCounterPlacementHistory {
                            baseline_counters: original_counters.clone(),
                            baseline_battlefield_receipts: placement.applied_replacements.clone(),
                            receipts: Vec::new(),
                        });
                let effect_created_counters = entry_counter_history_creates_counters(&history);
                proposals.push(ProposedCounterPlacement {
                    recipient: CounterPlacementRecipient::EnteringObject {
                        object_id: event.object_id,
                        zone_change_generation: generation,
                    },
                    actor: placement.actor,
                    original_counters: history.baseline_counters.clone(),
                    counters: original_counters,
                    origin: super::super::continuous::CounterPlacementOrigin::Entry,
                    entry_source: Some(placement.source.clone()),
                    entry_context: Some(EntryCounterReplacementContext {
                        placement_index,
                        destination_controller: event.destination_controller,
                        recipient_is_creature_or_planeswalker: projected.as_ref().is_some_and(
                            |characteristics| {
                                characteristics.is_creature()
                                    || characteristics.has_type("Planeswalker")
                            },
                        ),
                        recipient_can_receive_counters: self
                            .entry_counter_recipient_can_receive(event.object_id, entries),
                        source_zone: object.zone,
                        source_zone_change_generation: generation,
                        excluded_replacement_sources: excluded_replacement_sources.to_vec(),
                        history,
                        compleated: compleated.clone(),
                    }),
                    counter_move_context: None,
                    cost_debit_index: None,
                    effect_created_counters,
                    applied_replacements: placement.applied_replacements.clone(),
                });
            }
        }
        proposals
    }

    fn entry_compleated_replacement(
        &self,
        event: &BattlefieldEntryEvent,
    ) -> Option<EntryCompleatedReplacement> {
        let entrant = self.state.objects.get(&event.object_id)?;
        if entrant.face_down {
            return None;
        }
        let (_, printed) = self.battlefield_entry_early_characteristics(event)?;
        if !printed {
            return None;
        }
        let face = self.battlefield_entry_face(event)?;
        let ability = face
            .static_abilities
            .iter()
            .find(|ability| matches!(&ability.definition, StaticAbilityDef::Compleated))?;
        Some(EntryCompleatedReplacement {
            identity: CompleatedReplacementIdentity {
                recipient_object_id: event.object_id,
                recipient_zone_change_generation: self
                    .state
                    .zone_change_generation
                    .get(&event.object_id)
                    .copied()
                    .unwrap_or(0),
                copy_revision: entrant.copy_revision,
                ability_id: ability.ability_id.as_str().to_owned(),
            },
            phyrexian_symbols_paid_with_life: event.phyrexian_symbols_paid_with_life,
        })
    }

    fn validate_entry_counter_history(
        proposals: &[ProposedCounterPlacement],
    ) -> Result<(), EngineError> {
        type CompleatedHistoryGroup = (
            CompleatedReplacementIdentity,
            EntryCompleatedReplacement,
            Vec<(usize, u32, u32)>,
        );
        let mut compleated_groups: Vec<CompleatedHistoryGroup> = Vec::new();

        for proposal in proposals {
            let Some(context) = proposal.entry_context.as_ref() else {
                continue;
            };
            if !matches!(
                &proposal.origin,
                super::super::continuous::CounterPlacementOrigin::Entry
            ) {
                return Err(EngineError::Illegal(
                    "entry counter history has a non-entry origin",
                ));
            }
            let (recipient_object_id, recipient_generation) = match &proposal.recipient {
                CounterPlacementRecipient::EnteringObject {
                    object_id,
                    zone_change_generation,
                } => (*object_id, *zone_change_generation),
                _ => {
                    return Err(EngineError::Illegal(
                        "entry counter history recipient changed",
                    ));
                }
            };
            let history = &context.history;
            if proposal.original_counters != history.baseline_counters {
                return Err(EngineError::Illegal(
                    "entry counter history baseline changed",
                ));
            }

            let mut replayed = proposal.clone();
            replayed.counters = history.baseline_counters.clone();
            replayed.applied_replacements = history.baseline_battlefield_receipts.clone();
            replayed.effect_created_counters =
                history
                    .baseline_battlefield_receipts
                    .iter()
                    .any(|instance| {
                        matches!(
                            instance.kind,
                            CounterReplacementKind::Laezel
                                | CounterReplacementKind::DoubleCountersYouWouldPut
                                | CounterReplacementKind::HalveCountersOpponentsWouldPut
                        )
                    });
            replayed.entry_context = None;

            let mut seen_battlefield_identities = BTreeSet::new();
            for instance in &history.baseline_battlefield_receipts {
                if instance.ability_id.trim().is_empty()
                    || !seen_battlefield_identities.insert((
                        instance.source_object_id,
                        instance.source_zone_change_generation,
                        instance.ability_id.clone(),
                    ))
                {
                    return Err(EngineError::Illegal(
                        "entry battlefield replacement identity was repeated",
                    ));
                }
            }

            let mut seen_compleated_identities = Vec::new();
            for receipt in &history.receipts {
                match receipt {
                    EntryCounterReplacementReceipt::Battlefield { instance } => {
                        if instance.ability_id.trim().is_empty()
                            || !seen_battlefield_identities.insert((
                                instance.source_object_id,
                                instance.source_zone_change_generation,
                                instance.ability_id.clone(),
                            ))
                        {
                            return Err(EngineError::Illegal(
                                "entry battlefield replacement identity was repeated",
                            ));
                        }
                        Self::apply_counter_replacement(&mut replayed, instance);
                    }
                    EntryCounterReplacementReceipt::Compleated {
                        identity,
                        removed_loyalty,
                    } => {
                        let Some(compleated) = context.compleated.as_ref() else {
                            return Err(EngineError::Illegal(
                                "entry intrinsic replacement history changed",
                            ));
                        };
                        if !proposal
                            .original_counters
                            .contains_key(&CounterKind::Loyalty)
                            || compleated.phyrexian_symbols_paid_with_life == 0
                            || &compleated.identity != identity
                            || identity.recipient_object_id != recipient_object_id
                            || identity.recipient_zone_change_generation != recipient_generation
                            || identity.ability_id.trim().is_empty()
                            || seen_compleated_identities
                                .iter()
                                .any(|seen| seen == identity)
                        {
                            return Err(EngineError::Illegal(
                                "entry intrinsic replacement history changed",
                            ));
                        }
                        seen_compleated_identities.push(identity.clone());

                        let Some(loyalty) = replayed.counters.get_mut(&CounterKind::Loyalty) else {
                            return Err(EngineError::Illegal(
                                "entry intrinsic replacement has no Loyalty instruction",
                            ));
                        };
                        let loyalty_before = *loyalty;
                        if *removed_loyalty > loyalty_before {
                            return Err(EngineError::Illegal(
                                "entry intrinsic replacement removed too many counters",
                            ));
                        }
                        *loyalty -= *removed_loyalty;

                        if let Some(group_index) = compleated_groups
                            .iter()
                            .position(|(group_identity, _, _)| group_identity == identity)
                        {
                            let (_, group_replacement, participants) =
                                &mut compleated_groups[group_index];
                            if &*group_replacement != compleated {
                                return Err(EngineError::Illegal(
                                    "entry intrinsic replacement payment context changed",
                                ));
                            }
                            participants.push((
                                context.placement_index,
                                loyalty_before,
                                *removed_loyalty,
                            ));
                        } else {
                            compleated_groups.push((
                                identity.clone(),
                                compleated.clone(),
                                vec![(context.placement_index, loyalty_before, *removed_loyalty)],
                            ));
                        }
                    }
                }
            }

            if replayed.counters != proposal.counters
                || replayed.applied_replacements != proposal.applied_replacements
                || replayed.effect_created_counters != proposal.effect_created_counters
            {
                return Err(EngineError::Illegal(
                    "entry counter replacement history result changed",
                ));
            }
        }

        for (_, compleated, participants) in &mut compleated_groups {
            participants.sort_unstable_by_key(|(placement_index, _, _)| *placement_index);
            if !participants
                .iter()
                .any(|(_, loyalty_before, _)| *loyalty_before > 0)
                || participants.windows(2).any(|pair| pair[0].0 == pair[1].0)
            {
                return Err(EngineError::Illegal(
                    "entry intrinsic replacement participant history changed",
                ));
            }
            let mut remaining_reduction = compleated
                .phyrexian_symbols_paid_with_life
                .saturating_mul(2);
            for (_, loyalty_before, removed_loyalty) in participants.iter() {
                let expected_removal = (*loyalty_before).min(remaining_reduction);
                if *removed_loyalty != expected_removal {
                    return Err(EngineError::Illegal(
                        "entry intrinsic replacement attribution changed",
                    ));
                }
                remaining_reduction = remaining_reduction.saturating_sub(expected_removal);
            }
        }
        Ok(())
    }

    pub(super) fn apply_entry_counter_placement_results(
        event: &mut BattlefieldEntryEvent,
        proposals: &[ProposedCounterPlacement],
    ) -> Result<(), EngineError> {
        Self::validate_entry_counter_history(proposals)?;
        let mut updated = event.clone();
        let mut written_placements = BTreeSet::new();
        for proposal in proposals {
            let (object_id, generation) = match &proposal.recipient {
                CounterPlacementRecipient::EnteringObject {
                    object_id,
                    zone_change_generation,
                } => (*object_id, *zone_change_generation),
                _ => return Err(EngineError::Illegal("entry counter recipient changed")),
            };
            let Some(context) = proposal.entry_context.as_ref() else {
                return Err(EngineError::Illegal("entry counter context is missing"));
            };
            if object_id != event.object_id {
                continue;
            }
            if context.source_zone_change_generation != generation
                || context.destination_controller != event.destination_controller
                || proposal.original_counters != context.history.baseline_counters
            {
                return Err(EngineError::Illegal("entry counter generation changed"));
            }
            if !written_placements.insert(context.placement_index) {
                return Err(EngineError::Illegal(
                    "entry counter instruction was proposed more than once",
                ));
            }
            let Some(placement) = updated
                .entry_counter_placements
                .get_mut(context.placement_index)
            else {
                return Err(EngineError::Illegal("entry counter instruction is stale"));
            };
            if placement.actor != proposal.actor
                || !entry_counter_sources_match(proposal.entry_source.as_ref(), &placement.source)
            {
                return Err(EngineError::Illegal(
                    "entry counter instruction provenance changed",
                ));
            }
            if let Some(existing_history) = placement.history.as_ref() {
                if existing_history.baseline_counters != context.history.baseline_counters
                    || existing_history.baseline_battlefield_receipts
                        != context.history.baseline_battlefield_receipts
                    || !context
                        .history
                        .receipts
                        .starts_with(&existing_history.receipts)
                {
                    return Err(EngineError::Illegal(
                        "entry counter replacement history changed",
                    ));
                }
            } else if placement.counters != context.history.baseline_counters
                || placement.applied_replacements != context.history.baseline_battlefield_receipts
            {
                return Err(EngineError::Illegal(
                    "entry counter replacement baseline changed",
                ));
            }
            let mut expected_battlefield_receipts =
                context.history.baseline_battlefield_receipts.clone();
            for receipt in &context.history.receipts {
                if let EntryCounterReplacementReceipt::Battlefield { instance } = receipt {
                    expected_battlefield_receipts.push(instance.clone());
                }
            }
            if proposal.applied_replacements != expected_battlefield_receipts {
                return Err(EngineError::Illegal(
                    "entry battlefield replacement receipts changed",
                ));
            }
            placement.counters = if context.recipient_can_receive_counters {
                proposal.counters.clone()
            } else {
                BTreeMap::new()
            };
            placement.applied_replacements = proposal.applied_replacements.clone();
            placement.history = Some(context.history.clone());
        }
        updated.entry_counters.clear();
        for placement in &updated.entry_counter_placements {
            for (&kind, &count) in &placement.counters {
                accumulate_entry_counters(&mut updated.entry_counters, kind, count);
            }
        }
        updated.entry_counter_replacements_resolved = true;
        *event = updated;
        Ok(())
    }

    pub(in crate::engine) fn replacement_object_display_name(
        &self,
        object_id: ObjectId,
        entry_face: Option<usize>,
    ) -> String {
        let Some(object) = self.state.objects.get(&object_id) else {
            return "[object]".to_string();
        };
        if object.face_down {
            return match object.zone {
                Zone::Battlefield => "Face-down creature",
                Zone::Exile => "Face-down exiled card",
                Zone::Stack => "Face-down spell",
                _ => "Face-down object",
            }
            .to_string();
        }
        match object.zone {
            Zone::Hand => return "a card in a hand".to_string(),
            Zone::Library => return "a card in a library".to_string(),
            _ => {}
        }
        let display_name =
            self.effective_card_identity(object_id)
                .and_then(|(card_id, face_index)| {
                    let face_index =
                        if object.copiable_values.is_some() || object.token_origin.is_some() {
                            face_index
                        } else {
                            entry_face.unwrap_or(face_index)
                        };
                    self.registry.get(card_id)?.face_display_name(face_index)
                });
        display_name.map(str::to_string).unwrap_or_else(|| {
            self.effective_face(object_id)
                .map(|face| face.name.clone())
                .unwrap_or_default()
        })
    }

    pub(in crate::engine) fn replacement_source_presentation(
        &self,
        object_id: ObjectId,
    ) -> ReplacementSourcePresentation {
        let Some(object) = self.state.objects.get(&object_id) else {
            return ReplacementSourcePresentation::default();
        };
        if object.face_down || matches!(object.zone, Zone::Hand | Zone::Library) {
            return ReplacementSourcePresentation::default();
        }
        ReplacementSourcePresentation {
            card_name: self.replacement_object_display_name(object_id, None),
            object_id,
            zone_change_generation: self
                .state
                .zone_change_generation
                .get(&object_id)
                .copied()
                .unwrap_or(0),
        }
    }

    fn validate_counter_placement_completion_scope(
        &self,
        events: &[ProposedCounterPlacement],
        completion: &crate::state::CounterPlacementCompletion,
        parked_stack: Option<&ParkedStackResolution>,
    ) -> Result<(), EngineError> {
        self.validate_poison_difference_completion(events, completion, parked_stack)?;
        self.validate_targeted_poison_completion(events, completion, parked_stack)?;
        self.validate_distributed_counter_completion(events, completion, parked_stack)?;
        let crate::state::CounterPlacementCompletion::DoubleYourCounters {
            stack_id,
            mode_id,
            leaf_index,
            controller,
            snapshot,
        } = completion
        else {
            return Ok(());
        };
        let frame = parked_stack.ok_or(EngineError::Illegal(
            "DoubleYourCounters requires its actual parked stack frame",
        ))?;
        let selected_mode_matches = frame
            .item
            .chosen_modes
            .as_slice()
            .first()
            .is_some_and(|chosen| frame.item.chosen_modes.len() == 1 && chosen.mode_id == *mode_id);
        let supported_frame = frame.item.id == *stack_id
            && frame.item.controller == *controller
            && !frame.item.is_triggered
            && frame.program().is_none()
            && frame.resume_effect_index() == leaf_index.checked_add(1)
            && selected_mode_matches
            && frame
                .item
                .activated_ability
                .as_ref()
                .is_some_and(|ability| {
                    ability.program.is_none()
                        && ability.effect.is_empty()
                        && ability.modal.as_ref().is_some_and(|modal| {
                            modal.mode_by_id(mode_id).is_some_and(|mode| {
                                mode.program.is_none()
                                    && mode.effects.get(*leaf_index as usize)
                                        == Some(&SpellEffectKind::DoubleYourCounters)
                            })
                        })
                });
        if !supported_frame {
            return Err(EngineError::Illegal(
                "DoubleYourCounters completion does not match its captured activated-mode frame",
            ));
        }

        let player_index = self
            .state
            .player_idx(*controller)
            .ok_or(EngineError::Illegal(
                "DoubleYourCounters controller is missing",
            ))?;
        let player = &self.state.players[player_index];
        let is_live = !player.has_lost;
        if is_live {
            let current_snapshot = player
                .counters
                .iter()
                .filter_map(|(&kind, &count)| (count > 0).then_some((kind, count)))
                .collect::<BTreeMap<_, _>>();
            if current_snapshot != *snapshot {
                return Err(EngineError::Illegal(
                    "DoubleYourCounters counter snapshot became stale",
                ));
            }
        }
        let expected_event_count = if snapshot.is_empty() { 0 } else { 1 };
        if events.len() > expected_event_count || (is_live && events.len() != expected_event_count)
        {
            return Err(EngineError::Illegal(
                "DoubleYourCounters counter batch does not match its snapshot",
            ));
        }
        for event in events {
            if !matches!(
                &event.recipient,
                CounterPlacementRecipient::Player { player_id } if *player_id == *controller
            ) || event.actor != *controller
                || event.origin != super::super::continuous::CounterPlacementOrigin::Effect
                || event.original_counters != *snapshot
                || event
                    .counters
                    .keys()
                    .any(|kind| !snapshot.contains_key(kind))
                || snapshot
                    .keys()
                    .any(|kind| !event.counters.contains_key(kind))
                || event.entry_source.is_some()
                || event.entry_context.is_some()
                || event.counter_move_context.is_some()
                || event.cost_debit_index.is_some()
            {
                return Err(EngineError::Illegal(
                    "DoubleYourCounters counter batch has an invalid actor, recipient, or origin",
                ));
            }
        }
        Ok(())
    }

    /// Recognize a begun self-counter modal from its captured frame and validated proposal batch.
    /// Departure cleanup uses this before it prunes the departed player's counter recipient.
    pub(in crate::engine) fn begun_self_counter_placement(
        &self,
        pending: &PendingResolution,
        replacement: Option<&PendingReplacementEvent>,
    ) -> bool {
        let ResolutionContinuation::CounterReplacement {
            stack: Some(stack),
            completion:
                completion @ crate::state::CounterPlacementCompletion::DoubleYourCounters { .. },
        } = &pending.continuation
        else {
            return false;
        };
        let Some(PendingReplacementEvent::Counters(batch)) = replacement else {
            return false;
        };
        pending.presentation.source_object_id == stack.item.id
            && batch.event_index < batch.events.len()
            && self
                .validate_counter_placement_completion_scope(&batch.events, completion, Some(stack))
                .is_ok()
    }

    /// Resolve a parent batch of ordinary counter events. The return value is `None` when the
    /// affected player must choose a replacement order; no counter is committed in that case.
    pub(in crate::engine) fn begin_counter_placement_batch(
        &mut self,
        mut events: Vec<ProposedCounterPlacement>,
        stack: ParkedStackResolution,
        completion: crate::state::CounterPlacementCompletion,
        out: &mut Vec<rv1::RuledEvent>,
    ) -> Result<Option<Vec<CounterPlacementReceipt>>, EngineError> {
        self.validate_counter_placement_completion_scope(&events, &completion, Some(&stack))?;
        events.sort_by_key(|event| {
            let affected = match &event.recipient {
                CounterPlacementRecipient::Object { object_id, .. } => {
                    self.controller_of(*object_id)
                }
                CounterPlacementRecipient::EnteringObject { .. } => event
                    .entry_context
                    .as_ref()
                    .map(|context| context.destination_controller),
                CounterPlacementRecipient::Player { player_id } => Some(*player_id),
            };
            (affected
                .map(|player| self.state.apnap_rank(player))
                .unwrap_or(usize::MAX),)
        });
        let (batch, candidates) = self.prepare_counter_placement_batch(events)?;
        match candidates {
            Some(candidates) => {
                self.park_counter_placement_choice(batch, candidates, Some(stack), completion, out);
                Ok(None)
            }
            None => self
                .commit_counter_placement_batch(&batch.events, &completion, Some(&stack), out)
                .map(Some),
        }
    }

    /// Resolve a turn-based parent counter action that has no stack item to resume. The only
    /// current caller is the simultaneous Saga lore action.
    pub(in crate::engine) fn begin_unstacked_counter_placement_batch(
        &mut self,
        mut events: Vec<ProposedCounterPlacement>,
        completion: crate::state::CounterPlacementCompletion,
        out: &mut Vec<rv1::RuledEvent>,
    ) -> Result<bool, EngineError> {
        if !matches!(
            completion,
            crate::state::CounterPlacementCompletion::SagaLore { .. }
        ) {
            return Err(EngineError::Illegal(
                "unstacked counter batches require a turn-based completion",
            ));
        }
        events.sort_by_key(|event| {
            let affected = match &event.recipient {
                CounterPlacementRecipient::Object { object_id, .. } => {
                    self.controller_of(*object_id)
                }
                CounterPlacementRecipient::EnteringObject { .. } => event
                    .entry_context
                    .as_ref()
                    .map(|context| context.destination_controller),
                CounterPlacementRecipient::Player { player_id } => Some(*player_id),
            };
            (affected
                .map(|player| self.state.apnap_rank(player))
                .unwrap_or(usize::MAX),)
        });
        let (batch, candidates) = self.prepare_counter_placement_batch(events)?;
        if let Some(candidates) = candidates {
            self.park_counter_placement_choice(batch, candidates, None, completion, out);
            Ok(true)
        } else {
            self.commit_counter_placement_batch(&batch.events, &completion, None, out)?;
            Ok(false)
        }
    }

    /// Apply deterministic replacements and return a serializable proposal batch plus the first
    /// choice, if one is required. Callers that are paying costs retain the proposals without
    /// committing them until the full transaction is ready.
    pub(in crate::engine) fn prepare_counter_placement_batch(
        &mut self,
        events: Vec<ProposedCounterPlacement>,
    ) -> Result<PreparedCounterPlacementBatch, EngineError> {
        let mut batch = PendingCounterPlacementBatch {
            events,
            event_index: 0,
            applications: Vec::new(),
        };
        let candidates = self.advance_counter_placement_batch(&mut batch)?;
        Ok((batch, candidates))
    }

    pub(super) fn counter_replacement_candidates(
        &mut self,
        event: &ProposedCounterPlacement,
    ) -> Vec<(CounterReplacementInstance, String)> {
        let Some(context) = event.counter_move_context.clone() else {
            return self.counter_replacement_candidates_in_current_state(event);
        };
        let Some((before, timestamp)) = self
            .state
            .objects
            .get(&context.source_object_id)
            .filter(|object| object.zone == Zone::Battlefield && object.phase_status.is_none())
            .map(|object| {
                (
                    object.counter_count(context.kind),
                    object.counter_timestamps.get(&context.kind).copied(),
                )
            })
        else {
            return Vec::new();
        };
        if before == 0
            || self
                .state
                .zone_change_generation
                .get(&context.source_object_id)
                .copied()
                .unwrap_or(0)
                != context.source_zone_change_generation
        {
            return Vec::new();
        }
        self.state
            .objects
            .get_mut(&context.source_object_id)
            .expect("counter-move source was validated")
            .set_counter(context.kind, before - 1);
        let candidates = self.counter_replacement_candidates_in_current_state(event);
        let source = self
            .state
            .objects
            .get_mut(&context.source_object_id)
            .expect("counter-move source remains present during preview");
        source.set_counter(context.kind, before);
        if let Some(timestamp) = timestamp {
            source.counter_timestamps.insert(context.kind, timestamp);
        } else {
            source.counter_timestamps.remove(&context.kind);
        }
        candidates
    }

    fn counter_replacement_candidates_in_current_state(
        &self,
        event: &ProposedCounterPlacement,
    ) -> Vec<(CounterReplacementInstance, String)> {
        if !event.counters.values().any(|count| *count > 0) {
            return Vec::new();
        }
        let (recipient_controller, laezel_recipient) = match &event.recipient {
            CounterPlacementRecipient::Object {
                object_id,
                zone_change_generation,
            } => {
                let current_generation = self
                    .state
                    .zone_change_generation
                    .get(object_id)
                    .copied()
                    .unwrap_or(0);
                if current_generation != *zone_change_generation
                    || !self.can_receive_counters(*object_id)
                {
                    return Vec::new();
                }
                let Some(characteristics) = self.characteristics(*object_id) else {
                    return Vec::new();
                };
                (
                    Some(characteristics.controller),
                    characteristics.is_creature() || characteristics.has_type("Planeswalker"),
                )
            }
            CounterPlacementRecipient::EnteringObject {
                object_id,
                zone_change_generation,
            } => {
                let Some(context) = event.entry_context.as_ref() else {
                    return Vec::new();
                };
                let Some(object) = self.state.objects.get(object_id) else {
                    return Vec::new();
                };
                if self
                    .state
                    .zone_change_generation
                    .get(object_id)
                    .copied()
                    .unwrap_or(0)
                    != *zone_change_generation
                    || object.zone != context.source_zone
                    || context.source_zone_change_generation != *zone_change_generation
                    || !context.recipient_can_receive_counters
                {
                    return Vec::new();
                }
                (
                    Some(context.destination_controller),
                    context.recipient_is_creature_or_planeswalker,
                )
            }
            CounterPlacementRecipient::Player { player_id } => (Some(*player_id), true),
        };
        let mut sources = self
            .state
            .objects
            .values()
            .filter(|object| {
                object.zone == Zone::Battlefield
                    && object.phase_status.is_none()
                    && !object.face_down
            })
            .map(|object| object.id)
            .collect::<Vec<_>>();
        sources.sort_unstable();
        let mut candidates = Vec::new();
        for source_id in sources {
            if !self.state.objects.contains_key(&source_id) {
                continue;
            }
            if !super::super::characteristics::printed_static_source_is_available(
                &self.state,
                self.registry,
                source_id,
            ) {
                continue;
            }
            let Some(source_controller) = self
                .characteristics(source_id)
                .map(|characteristics| characteristics.controller)
            else {
                continue;
            };
            let source_generation = self
                .state
                .zone_change_generation
                .get(&source_id)
                .copied()
                .unwrap_or(0);
            if event.entry_context.as_ref().is_some_and(|context| {
                context
                    .excluded_replacement_sources
                    .contains(&(source_id, source_generation))
            }) {
                continue;
            }
            for ability in self.active_static_abilities(source_id) {
                let kind = match ability.definition {
                    StaticAbilityDef::AddOneCounterOfEachKindToSelfOrControlledCreaturePlaneswalker
                        if source_controller == event.actor
                            && recipient_controller == Some(event.actor)
                            && laezel_recipient =>
                    {
                        CounterReplacementKind::Laezel
                    }
                    StaticAbilityDef::DoubleCountersYouWouldPut
                        if source_controller == event.actor =>
                    {
                        CounterReplacementKind::DoubleCountersYouWouldPut
                    }
                    StaticAbilityDef::HalveCountersOpponentsWouldPut
                        if self.state.are_opponents(event.actor, source_controller) =>
                    {
                        CounterReplacementKind::HalveCountersOpponentsWouldPut
                    }
                        StaticAbilityDef::DoubleEffectCountersPlacedOnPermanentsYouControl
                        if (event.origin == super::super::continuous::CounterPlacementOrigin::Effect
                            || event.origin == super::super::continuous::CounterPlacementOrigin::Entry
                            || event.effect_created_counters)
                            && recipient_controller == Some(source_controller)
                                && matches!(
                                    &event.recipient,
                                    CounterPlacementRecipient::Object { .. }
                                        | CounterPlacementRecipient::EnteringObject { .. }
                                ) =>
                    {
                        CounterReplacementKind::DoublingSeason
                    }
                    _ => continue,
                };
                let instance = CounterReplacementInstance {
                    source_object_id: source_id,
                    source_zone_change_generation: source_generation,
                    ability_id: ability.ability_id.as_str().to_owned(),
                    kind,
                };
                if event.applied_replacements.contains(&instance) {
                    continue;
                }
                let summary = match kind {
                    CounterReplacementKind::Laezel => "adds one of each counter kind",
                    CounterReplacementKind::DoublingSeason => "doubles effect counters",
                    CounterReplacementKind::DoubleCountersYouWouldPut => {
                        "doubles counters this player would put"
                    }
                    CounterReplacementKind::HalveCountersOpponentsWouldPut => {
                        "halves counters opponents would put, rounded down"
                    }
                };
                let label = format!(
                    "{} {summary}",
                    self.replacement_object_display_name(source_id, None)
                );
                candidates.push((instance, label));
            }
        }
        candidates
    }

    fn counter_placement_candidates_for_batch(
        &mut self,
        events: &[ProposedCounterPlacement],
        event_index: usize,
    ) -> CounterPlacementCandidates {
        let Some(event) = events.get(event_index) else {
            return Vec::new();
        };
        let Some(group) = entry_compleated_counter_group(events, event_index) else {
            return single_row_counter_placement_candidates(
                event_index,
                self.counter_replacement_candidates(event),
            );
        };
        if group.member_event_indices.first() != Some(&event_index) {
            return Vec::new();
        }

        let mut candidates: CounterPlacementCandidates = Vec::new();
        for &member_event_index in &group.member_event_indices {
            for (instance, label) in
                self.counter_replacement_candidates(&events[member_event_index])
            {
                if let Some(candidate_index) = candidates.iter().position(|(replacement, _)| {
                    matches!(
                        replacement,
                        CounterPlacementReplacement::Battlefield {
                            instance: existing,
                            ..
                        } if existing == &instance
                    )
                }) {
                    let CounterPlacementReplacement::Battlefield {
                        member_event_indices,
                        ..
                    } = &mut candidates[candidate_index].0
                    else {
                        unreachable!("duplicate entry candidates are battlefield operations");
                    };
                    if !member_event_indices.contains(&member_event_index) {
                        member_event_indices.push(member_event_index);
                    }
                    continue;
                }
                let Some(source_controller) = self
                    .characteristics(instance.source_object_id)
                    .map(|characteristics| characteristics.controller)
                else {
                    continue;
                };
                candidates.push((
                    CounterPlacementReplacement::Battlefield {
                        instance,
                        member_event_indices: vec![member_event_index],
                        source_controller: Some(source_controller),
                    },
                    label,
                ));
            }
        }

        let identity = &group.replacement.identity;
        let already_applied = group.member_event_indices.iter().any(|&index| {
            events[index].entry_context.as_ref().is_some_and(|context| {
                context.history.receipts.iter().any(|receipt| {
                    matches!(
                        receipt,
                        EntryCounterReplacementReceipt::Compleated {
                            identity: receipt_identity,
                            ..
                        } if receipt_identity == identity
                    )
                })
            })
        });
        let has_positive_loyalty = group.member_event_indices.iter().any(|&index| {
            events[index]
                .counters
                .get(&CounterKind::Loyalty)
                .is_some_and(|count| *count > 0)
        });
        let can_receive_counters = group.member_event_indices.iter().all(|&index| {
            events[index]
                .entry_context
                .as_ref()
                .is_some_and(|context| context.recipient_can_receive_counters)
        });
        if group.replacement.phyrexian_symbols_paid_with_life > 0
            && has_positive_loyalty
            && can_receive_counters
            && !already_applied
        {
            let reduction = group
                .replacement
                .phyrexian_symbols_paid_with_life
                .saturating_mul(2);
            candidates.push((
                CounterPlacementReplacement::Compleated {
                    replacement: group.replacement,
                    member_event_indices: group.member_event_indices,
                },
                format!("Compleated removes up to {reduction} Loyalty counters"),
            ));
        }
        candidates
    }

    pub(super) fn apply_counter_placement_replacement(
        batch: &mut PendingCounterPlacementBatch,
        replacement: &CounterPlacementReplacement,
    ) -> Result<(), EngineError> {
        let invalid_operation = || {
            EngineError::ProgramInvariant("counter replacement operation does not match its batch")
        };
        match replacement {
            CounterPlacementReplacement::Battlefield {
                instance,
                member_event_indices,
                source_controller,
            } => {
                if member_event_indices.is_empty()
                    || member_event_indices
                        .windows(2)
                        .any(|pair| pair[0] >= pair[1])
                    || member_event_indices
                        .iter()
                        .any(|index| *index >= batch.events.len())
                {
                    return Err(invalid_operation());
                }
                if source_controller.is_none() {
                    single_row_battlefield_instance(replacement, batch.event_index)?;
                } else {
                    let Some(group) =
                        entry_compleated_counter_group(&batch.events, batch.event_index)
                    else {
                        return Err(invalid_operation());
                    };
                    if group.member_event_indices.first() != Some(&batch.event_index)
                        || member_event_indices
                            .iter()
                            .any(|index| !group.member_event_indices.contains(index))
                    {
                        return Err(invalid_operation());
                    }
                }
                for &event_index in member_event_indices {
                    Self::apply_counter_replacement(&mut batch.events[event_index], instance);
                }
            }
            CounterPlacementReplacement::Compleated {
                replacement,
                member_event_indices,
            } => {
                let Some(group) = entry_compleated_counter_group(&batch.events, batch.event_index)
                else {
                    return Err(invalid_operation());
                };
                if group.member_event_indices.first() != Some(&batch.event_index)
                    || group.member_event_indices != *member_event_indices
                    || group.replacement != *replacement
                    || replacement.phyrexian_symbols_paid_with_life == 0
                    || member_event_indices.iter().any(|&index| {
                        batch.events[index]
                            .entry_context
                            .as_ref()
                            .is_none_or(|context| !context.recipient_can_receive_counters)
                            || !batch.events[index]
                                .counters
                                .contains_key(&CounterKind::Loyalty)
                    })
                    || member_event_indices.iter().any(|&index| {
                        batch.events[index]
                            .entry_context
                            .as_ref()
                            .is_some_and(|context| {
                                context.history.receipts.iter().any(|receipt| {
                                    matches!(
                                        receipt,
                                        EntryCounterReplacementReceipt::Compleated {
                                            identity,
                                            ..
                                        } if identity == &replacement.identity
                                    )
                                })
                            })
                    })
                    || !member_event_indices.iter().any(|&index| {
                        batch.events[index]
                            .counters
                            .get(&CounterKind::Loyalty)
                            .is_some_and(|count| *count > 0)
                    })
                {
                    return Err(invalid_operation());
                }
                let mut remaining = replacement
                    .phyrexian_symbols_paid_with_life
                    .saturating_mul(2);
                for &event_index in member_event_indices {
                    let event = &mut batch.events[event_index];
                    let loyalty = event
                        .counters
                        .get_mut(&CounterKind::Loyalty)
                        .ok_or_else(invalid_operation)?;
                    let removed_loyalty = (*loyalty).min(remaining);
                    *loyalty -= removed_loyalty;
                    remaining -= removed_loyalty;
                    event
                        .entry_context
                        .as_mut()
                        .ok_or_else(invalid_operation)?
                        .history
                        .receipts
                        .push(EntryCounterReplacementReceipt::Compleated {
                            identity: replacement.identity.clone(),
                            removed_loyalty,
                        });
                }
            }
        }
        Ok(())
    }

    /// Apply one replacement and continue until either a player must choose or every event is
    /// fully processed. `Some` contains the current eligible choices; `None` means ready to commit.
    pub(super) fn advance_counter_placement_batch(
        &mut self,
        batch: &mut PendingCounterPlacementBatch,
    ) -> Result<Option<CounterPlacementCandidates>, EngineError> {
        Self::validate_entry_counter_history(&batch.events)?;
        while batch.event_index < batch.events.len() {
            let event_index = batch.event_index;
            let candidates =
                self.counter_placement_candidates_for_batch(&batch.events, event_index);
            match candidates.len() {
                0 => batch.event_index += 1,
                1 => {
                    Self::apply_counter_placement_replacement(batch, &candidates[0].0)?;
                }
                _ => return Ok(Some(candidates)),
            }
        }
        Ok(None)
    }

    pub(in crate::engine) fn validate_resolved_cost_counter_placements(
        &mut self,
        plan: &super::super::payment::transaction::CostTransactionPlan,
        placements: &[ProposedCounterPlacement],
    ) -> Result<(), EngineError> {
        let expected = plan.counter_placement_costs();
        if expected.len() != placements.len() {
            return Err(EngineError::Illegal(
                "counter cost placements do not match the payment plan",
            ));
        }
        for (debit, placement) in expected.iter().zip(placements) {
            let original_counters = BTreeMap::from([(debit.kind, debit.count)]);
            if placement.cost_debit_index != Some(debit.debit_index)
                || placement.actor != plan.player()
                || placement.origin != super::super::continuous::CounterPlacementOrigin::Cost
                || placement.original_counters != original_counters
                || placement.counters.keys().any(|kind| *kind != debit.kind)
                || !matches!(
                    &placement.recipient,
                    CounterPlacementRecipient::Object {
                        object_id,
                        zone_change_generation
                    } if *object_id == debit.object_id && *zone_change_generation == debit.generation
                )
            {
                return Err(EngineError::Illegal(
                    "positive loyalty cost placement does not match its debit",
                ));
            }

            let mut replayed = self
                .proposed_object_counter_placement(
                    plan.player(),
                    debit.object_id,
                    [(debit.kind, debit.count)],
                    super::super::continuous::CounterPlacementOrigin::Cost,
                )
                .ok_or(EngineError::Illegal("counter cost recipient became stale"))?;
            replayed.cost_debit_index = Some(debit.debit_index);
            // Applied replacements are an ordered engine-owned receipt. A source can leave
            // after its replacement was applied, so only unapplied choices use live sources.
            let mut applied = BTreeSet::new();
            for replacement in &placement.applied_replacements {
                if !applied.insert((
                    replacement.source_object_id,
                    replacement.source_zone_change_generation,
                    replacement.ability_id.as_str(),
                )) {
                    return Err(EngineError::Illegal(
                        "counter cost replacement was applied more than once",
                    ));
                }
                if replacement.kind == CounterReplacementKind::DoublingSeason
                    && !replayed.effect_created_counters
                {
                    return Err(EngineError::Illegal(
                        "effect-only counter replacement applied to a cost",
                    ));
                }
                Self::apply_counter_replacement(&mut replayed, replacement);
            }
            let mut remaining = PendingCounterPlacementBatch {
                events: vec![replayed],
                event_index: 0,
                applications: Vec::new(),
            };
            if self
                .advance_counter_placement_batch(&mut remaining)?
                .is_some()
            {
                return Err(EngineError::Illegal(
                    "counter cost replacement order is incomplete",
                ));
            }
            let replayed = &remaining.events[0];
            if replayed.counters != placement.counters
                || replayed.effect_created_counters != placement.effect_created_counters
                || replayed.applied_replacements != placement.applied_replacements
            {
                return Err(EngineError::Illegal(
                    "counter cost replacement result changed",
                ));
            }
        }
        Ok(())
    }

    /// A replacement can reduce a moved counter to zero while the move still removes it from
    /// its source. Admit that result only when replaying the engine-recorded replacement order
    /// from the original one-counter instruction produces the exact committed proposal.
    pub(super) fn counter_move_zero_result_has_authenticated_replacement_history(
        event: &ProposedCounterPlacement,
    ) -> bool {
        let Some((&kind, &original_count)) = event.original_counters.iter().next() else {
            return false;
        };
        if event.original_counters.len() != 1
            || original_count != 1
            || event.applied_replacements.is_empty()
            || event.counters != BTreeMap::from([(kind, 0)])
        {
            return false;
        }

        let mut replayed = event.clone();
        replayed.counters = event.original_counters.clone();
        replayed.effect_created_counters = false;
        replayed.applied_replacements.clear();
        let mut applied = BTreeSet::new();
        for replacement in &event.applied_replacements {
            if !replayed.counters.values().any(|count| *count > 0)
                || replacement.ability_id.trim().is_empty()
                || !applied.insert((
                    replacement.source_object_id,
                    replacement.source_zone_change_generation,
                    replacement.ability_id.as_str(),
                ))
            {
                return false;
            }
            Self::apply_counter_replacement(&mut replayed, replacement);
        }
        replayed.counters == event.counters
            && replayed.effect_created_counters == event.effect_created_counters
            && replayed.applied_replacements == event.applied_replacements
    }

    pub(super) fn apply_counter_replacement(
        event: &mut ProposedCounterPlacement,
        instance: &CounterReplacementInstance,
    ) {
        match instance.kind {
            CounterReplacementKind::Laezel => {
                for count in event.counters.values_mut() {
                    if *count > 0 {
                        *count = count.saturating_add(1);
                    }
                }
                event.effect_created_counters = true;
            }
            CounterReplacementKind::DoublingSeason => {
                for count in event.counters.values_mut() {
                    *count = count.saturating_mul(2);
                }
            }
            CounterReplacementKind::DoubleCountersYouWouldPut => {
                for count in event.counters.values_mut() {
                    *count = count.saturating_mul(2);
                }
                event.effect_created_counters = true;
            }
            CounterReplacementKind::HalveCountersOpponentsWouldPut => {
                for count in event.counters.values_mut() {
                    *count /= 2;
                }
                event.effect_created_counters = true;
            }
        }
        event.applied_replacements.push(instance.clone());
        if let Some(context) = event.entry_context.as_mut() {
            context
                .history
                .receipts
                .push(EntryCounterReplacementReceipt::Battlefield {
                    instance: instance.clone(),
                });
        }
    }

    /// Re-project the exact current typed Proliferate counter-order offer without allocating
    /// replacement handles or advancing its saved batch. The observer return may have reset only
    /// the prompt correlation; the application IDs and source incarnations remain engine-owned.
    pub(in crate::engine) fn current_proliferate_counter_placement_choice_event(
        &self,
    ) -> Result<Option<rv1::RuledEvent>, EngineError> {
        let Some(pending) = self.state.pending_resolution.as_ref() else {
            return Ok(None);
        };
        let ResolutionContinuation::CounterReplacement {
            stack: Some(stack),
            completion: crate::state::CounterPlacementCompletion::Proliferate { .. },
        } = &pending.continuation
        else {
            return Ok(None);
        };
        self.current_single_row_counter_placement_choice_event(pending, stack)
            .map(Some)
    }

    pub(in crate::engine) fn current_targeted_poison_counter_placement_choice_event(
        &self,
    ) -> Result<Option<rv1::RuledEvent>, EngineError> {
        let Some(pending) = self.state.pending_resolution.as_ref() else {
            return Ok(None);
        };
        let ResolutionContinuation::CounterReplacement {
            stack: Some(stack),
            completion: completion @ crate::state::CounterPlacementCompletion::TargetedPoison { .. },
        } = &pending.continuation
        else {
            return Ok(None);
        };
        let Some(PendingReplacementEvent::Counters(batch)) =
            self.state.pending_replacement_event.as_ref()
        else {
            return Err(EngineError::ProgramInvariant(
                "targeted poison counter-order choice has no paired Counters batch",
            ));
        };
        self.validate_targeted_poison_completion(&batch.events, completion, Some(stack))?;
        self.current_single_row_counter_placement_choice_event(pending, stack)
            .map(Some)
    }

    fn current_single_row_counter_placement_choice_event(
        &self,
        pending: &PendingResolution,
        stack: &ParkedStackResolution,
    ) -> Result<rv1::RuledEvent, EngineError> {
        if pending.presentation.choice_kind != rv1::ChoiceKind::ReplacementEffect
            || pending.presentation.source_object_id != stack.item.id
        {
            return Err(EngineError::ProgramInvariant(
                "Proliferate counter-order presentation does not match its stack item",
            ));
        }
        let Some(PendingReplacementEvent::Counters(batch)) =
            self.state.pending_replacement_event.as_ref()
        else {
            return Err(EngineError::ProgramInvariant(
                "Proliferate counter-order choice has no paired Counters batch",
            ));
        };
        let batch = batch.as_ref();
        let Some(event) = batch.events.get(batch.event_index) else {
            return Err(EngineError::ProgramInvariant(
                "Proliferate counter-order batch has no current event",
            ));
        };
        if event.counter_move_context.is_some() {
            return Err(EngineError::ProgramInvariant(
                "Proliferate counter-order event unexpectedly has counter-move context",
            ));
        }
        if batch.applications.len() < 2
            || batch
                .applications
                .iter()
                .any(|application| application.event_index != batch.event_index)
        {
            return Err(EngineError::ProgramInvariant(
                "Proliferate counter-order applications do not match the current event",
            ));
        }
        for application in &batch.applications {
            single_row_battlefield_instance(&application.replacement, application.event_index)?;
        }
        let current_candidates = single_row_counter_placement_candidates(
            batch.event_index,
            self.counter_replacement_candidates_in_current_state(event),
        );
        if current_candidates.len() != batch.applications.len() {
            return Err(EngineError::ProgramInvariant(
                "Proliferate counter-order application set is no longer current",
            ));
        }
        let mut labels = Vec::with_capacity(batch.applications.len());
        for application in &batch.applications {
            let mut matches = current_candidates
                .iter()
                .filter(|(candidate, _)| candidate == &application.replacement);
            let Some((_, label)) = matches.next() else {
                return Err(EngineError::ProgramInvariant(
                    "Proliferate counter-order application source is no longer current",
                ));
            };
            if matches.next().is_some() {
                return Err(EngineError::ProgramInvariant(
                    "Proliferate counter-order application source is ambiguous",
                ));
            }
            labels.push(label.clone());
        }
        if current_candidates.iter().any(|(candidate, _)| {
            !batch
                .applications
                .iter()
                .any(|application| application.replacement == *candidate)
        }) {
            return Err(EngineError::ProgramInvariant(
                "Proliferate counter-order has an unbound current replacement source",
            ));
        }
        let application_ids = batch
            .applications
            .iter()
            .map(|application| application.choice_id)
            .collect::<Vec<_>>();
        if pending.presentation.candidates != application_ids {
            return Err(EngineError::ProgramInvariant(
                "Proliferate counter-order handles do not match their saved applications",
            ));
        }
        let current_decider = match &event.recipient {
            CounterPlacementRecipient::Object { object_id, .. } => self.controller_of(*object_id),
            CounterPlacementRecipient::EnteringObject { .. } => event
                .entry_context
                .as_ref()
                .map(|context| context.destination_controller),
            CounterPlacementRecipient::Player { player_id } => Some(*player_id),
        }
        .ok_or(EngineError::ProgramInvariant(
            "Proliferate counter-order recipient has no current decider",
        ))?;
        if current_decider != pending.deciding_player {
            return Err(EngineError::ProgramInvariant(
                "Proliferate counter-order decider changed without refreshing the batch",
            ));
        }
        let (choice, _) = self.counter_placement_choice_event(
            event,
            pending.deciding_player,
            stack.item.id,
            pending.resolution_prompt_id.get(),
            &batch.applications,
            &labels,
        );
        Ok(choice)
    }

    /// Build the one concrete ReplacementEffect projection shared by initial counter-order
    /// parking and observer restoration of unchanged saved Proliferate work.
    fn counter_placement_choice_event(
        &self,
        event: &ProposedCounterPlacement,
        deciding_player: PlayerId,
        source_object_id: ObjectId,
        resolution_prompt_id: u64,
        applications: &[CounterPlacementApplication],
        labels: &[String],
    ) -> (rv1::RuledEvent, String) {
        let prompt = match &event.recipient {
            CounterPlacementRecipient::Object { object_id, .. } => format!(
                "Choose the next counter replacement effect for {}.",
                self.replacement_object_display_name(*object_id, None)
            ),
            CounterPlacementRecipient::EnteringObject { object_id, .. } => format!(
                "Choose the next counter replacement effect for {} entering the battlefield.",
                self.replacement_object_display_name(*object_id, None)
            ),
            CounterPlacementRecipient::Player { player_id } => {
                format!("Choose the next counter replacement effect for P{player_id}.")
            }
        };
        let application_ids = applications
            .iter()
            .map(|application| application.choice_id)
            .collect::<Vec<_>>();
        let replacement_options = applications
            .iter()
            .zip(labels)
            .map(|(application, label)| {
                self.replacement_source_presentation(
                    counter_placement_replacement_source_object_id(&application.replacement),
                )
                .option(application.choice_id, label.clone())
            })
            .collect();
        (
            rv1::RuledEvent {
                ev: Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(
                    rv1::ResolutionChoiceRequired {
                        resolution_prompt_id,
                        accepted_permanent_selections: Vec::new(),
                        variable_mana_contribution: false,
                        candidate_token_identities: Vec::new(),
                        candidate_player_ids: Vec::new(),
                        deciding_player_id: deciding_player,
                        source_object_id,
                        prompt_text: prompt.clone(),
                        choice_kind: rv1::ChoiceKind::ReplacementEffect as i32,
                        candidate_object_ids: application_ids,
                        candidate_card_ids: vec![String::new(); applications.len()],
                        min: 1,
                        max: 1,
                        ordered: false,
                        candidate_names: labels.to_vec(),
                        candidate_server_card_ids: Vec::new(),
                        candidate_selectable: Vec::new(),
                        resolution_branches: Vec::new(),
                        mana_cost: String::new(),
                        unique_names: false,
                        generic_mana_cost: 0,
                        payment_currently_legal: false,
                        public_reveal: None,
                        candidate_source_zones: Vec::new(),
                        combat_defender_options: Vec::new(),
                        waterbend: false,
                        selection_slots: Vec::new(),
                        replacement_options,
                        selection_alternatives: Vec::new(),
                    },
                )),
            },
            prompt,
        )
    }

    pub(in crate::engine) fn park_counter_placement_choice(
        &mut self,
        mut batch: PendingCounterPlacementBatch,
        candidates: CounterPlacementCandidates,
        stack: Option<ParkedStackResolution>,
        completion: crate::state::CounterPlacementCompletion,
        out: &mut Vec<rv1::RuledEvent>,
    ) {
        let event = &batch.events[batch.event_index];
        let deciding_player = match &event.recipient {
            CounterPlacementRecipient::Object { object_id, .. } => {
                self.controller_of(*object_id).unwrap_or(event.actor)
            }
            CounterPlacementRecipient::EnteringObject { .. } => event
                .entry_context
                .as_ref()
                .map(|context| context.destination_controller)
                .unwrap_or(event.actor),
            CounterPlacementRecipient::Player { player_id } => *player_id,
        };
        batch.applications.clear();
        let mut labels = Vec::with_capacity(candidates.len());
        for (replacement, label) in candidates {
            let choice_id = self.state.next_replacement_application_id;
            self.state.next_replacement_application_id = choice_id.saturating_add(1);
            labels.push(label);
            batch.applications.push(CounterPlacementApplication {
                choice_id,
                event_index: batch.event_index,
                replacement,
            });
        }
        let source_object_id = stack
            .as_ref()
            .map(|parked| parked.item.id)
            .unwrap_or_else(|| match &completion {
                crate::state::CounterPlacementCompletion::ActivationCost { replay } => {
                    replay.source_object_id
                }
                crate::state::CounterPlacementCompletion::DamageBatch { completed, .. } => {
                    completed
                        .first()
                        .map(|damage| damage.spec.event.source.object_id)
                        .unwrap_or_default()
                }
                crate::state::CounterPlacementCompletion::DamageReplacement {
                    damage_batch,
                    ..
                } => damage_batch
                    .damage
                    .first()
                    .map(|damage| damage.spec.event.source.object_id)
                    .unwrap_or_default(),
                crate::state::CounterPlacementCompletion::SagaLore { sagas, .. } => {
                    sagas.first().map(|saga| saga.object_id).unwrap_or_default()
                }
                _ => 0,
            });
        let application_ids = batch
            .applications
            .iter()
            .map(|application| application.choice_id)
            .collect::<Vec<_>>();
        let (choice_event, prompt) = self.counter_placement_choice_event(
            event,
            deciding_player,
            source_object_id,
            0,
            &batch.applications,
            &labels,
        );
        out.push(choice_event);
        out.push(ev_log(prompt.clone()));
        self.state.pending_replacement_event =
            Some(PendingReplacementEvent::Counters(Box::new(batch)));
        self.state.pending_resolution = Some(PendingResolution::new_choice(
            deciding_player,
            PendingResolutionPresentation {
                source_object_id,
                candidates: application_ids,
                min: 1,
                max: 1,
                ordered: false,
                prompt,
                choice_kind: rv1::ChoiceKind::ReplacementEffect,
                unique_names: false,
            },
            ResolutionContinuation::CounterReplacement { stack, completion },
        ));
    }

    pub(super) fn park_entry_counter_placement_choice(
        &mut self,
        mut counter_batch: PendingCounterPlacementBatch,
        candidates: CounterPlacementCandidates,
        resume: crate::state::EntryCounterReplacementResume,
        out: &mut Vec<rv1::RuledEvent>,
    ) {
        let event = &counter_batch.events[counter_batch.event_index];
        let (deciding_player, entering_object_id) = match &event.recipient {
            CounterPlacementRecipient::EnteringObject { object_id, .. } => (
                event
                    .entry_context
                    .as_ref()
                    .map(|context| context.destination_controller)
                    .unwrap_or(event.actor),
                *object_id,
            ),
            _ => (event.actor, 0),
        };
        let (stack, resume) = match resume {
            crate::state::EntryCounterReplacementResume::Single {
                stack,
                event,
                completion,
            } => (
                Some(stack.clone()),
                crate::state::EntryCounterReplacementResume::Single {
                    stack,
                    event,
                    completion,
                },
            ),
            crate::state::EntryCounterReplacementResume::Batch {
                stack,
                batch,
                source_generations,
            } => (
                stack.clone(),
                crate::state::EntryCounterReplacementResume::Batch {
                    stack,
                    batch,
                    source_generations,
                },
            ),
        };
        let mut candidate_ids = Vec::with_capacity(candidates.len());
        let mut candidate_names = Vec::with_capacity(candidates.len());
        let mut replacement_options = Vec::with_capacity(candidates.len());
        counter_batch.applications.clear();
        for (replacement, label) in candidates {
            let choice_id = self.state.next_replacement_application_id;
            self.state.next_replacement_application_id = choice_id.saturating_add(1);
            replacement_options.push(
                self.replacement_source_presentation(
                    counter_placement_replacement_source_object_id(&replacement),
                )
                .option(choice_id, label.clone()),
            );
            candidate_ids.push(choice_id);
            candidate_names.push(label);
            counter_batch
                .applications
                .push(CounterPlacementApplication {
                    choice_id,
                    event_index: counter_batch.event_index,
                    replacement,
                });
        }
        let prompt = match &event.recipient {
            CounterPlacementRecipient::EnteringObject { object_id, .. } => format!(
                "Choose the next counter replacement effect for {} entering the battlefield.",
                self.replacement_object_display_name(*object_id, None)
            ),
            _ => "Choose the next counter replacement effect.".to_string(),
        };
        let source_object_id = stack
            .as_ref()
            .map(|parked| parked.item.id)
            .unwrap_or(entering_object_id);
        out.push(rv1::RuledEvent {
            ev: Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(
                rv1::ResolutionChoiceRequired {
                    resolution_prompt_id: 0,
                    accepted_permanent_selections: Vec::new(),
                    candidate_token_identities: Vec::new(),
                    candidate_player_ids: Vec::new(),
                    deciding_player_id: deciding_player,
                    source_object_id,
                    prompt_text: prompt.clone(),
                    choice_kind: rv1::ChoiceKind::ReplacementEffect as i32,
                    candidate_object_ids: candidate_ids.clone(),
                    candidate_card_ids: vec![String::new(); candidate_ids.len()],
                    min: 1,
                    max: 1,
                    ordered: false,
                    candidate_names,
                    candidate_server_card_ids: Vec::new(),
                    candidate_selectable: Vec::new(),
                    unique_names: false,
                    public_reveal: None,
                    candidate_source_zones: Vec::new(),
                    replacement_options,
                    ..Default::default()
                },
            )),
        });
        out.push(ev_log(prompt.clone()));
        self.state.pending_replacement_event =
            Some(PendingReplacementEvent::Counters(Box::new(counter_batch)));
        self.state.pending_resolution = Some(PendingResolution::new_choice(
            deciding_player,
            PendingResolutionPresentation {
                source_object_id,
                candidates: candidate_ids,
                min: 1,
                max: 1,
                ordered: false,
                prompt,
                choice_kind: rv1::ChoiceKind::ReplacementEffect,
                unique_names: false,
            },
            ResolutionContinuation::EntryCounterReplacement {
                resume: Box::new(resume),
            },
        ));
    }

    fn simultaneous_entry_references(
        batch: &SimultaneousEntryBatch,
    ) -> Vec<&BattlefieldEntryEvent> {
        match batch {
            SimultaneousEntryBatch::Zone(batch) => {
                batch.ready.iter().chain(batch.remaining.iter()).collect()
            }
            SimultaneousEntryBatch::Token(batch) => batch
                .ready
                .iter()
                .chain(batch.remaining.iter())
                .map(|entry| &entry.event)
                .collect(),
            SimultaneousEntryBatch::Observer(batch) => {
                batch.ready.iter().map(|entry| &entry.event).collect()
            }
        }
    }

    fn simultaneous_entry_references_mut(
        batch: &mut SimultaneousEntryBatch,
    ) -> Vec<&mut BattlefieldEntryEvent> {
        match batch {
            SimultaneousEntryBatch::Zone(batch) => batch
                .ready
                .iter_mut()
                .chain(batch.remaining.iter_mut())
                .collect(),
            SimultaneousEntryBatch::Token(batch) => batch
                .ready
                .iter_mut()
                .chain(batch.remaining.iter_mut())
                .map(|entry| &mut entry.event)
                .collect(),
            SimultaneousEntryBatch::Observer(batch) => batch
                .ready
                .iter_mut()
                .map(|entry| &mut entry.event)
                .collect(),
        }
    }

    fn simultaneous_entry_source_generations(
        &self,
        batch: &SimultaneousEntryBatch,
    ) -> Vec<(ObjectId, u64, Zone)> {
        Self::simultaneous_entry_references(batch)
            .into_iter()
            .filter_map(|event| {
                let object = self.state.objects.get(&event.object_id)?;
                Some((
                    event.object_id,
                    self.state
                        .zone_change_generation
                        .get(&event.object_id)
                        .copied()
                        .unwrap_or(0),
                    object.zone,
                ))
            })
            .collect()
    }

    fn entry_counter_excluded_sources(batch: &SimultaneousEntryBatch) -> Vec<(ObjectId, u64)> {
        match batch {
            SimultaneousEntryBatch::Zone(batch) => match &batch.completion {
                Some(crate::state::ZoneEntryCompletion::PairedArtifactExchange {
                    departure,
                    ..
                }) => vec![(departure.object_id, departure.zone_change_generation)],
                _ => Vec::new(),
            },
            SimultaneousEntryBatch::Token(_) | SimultaneousEntryBatch::Observer(_) => Vec::new(),
        }
    }

    fn simultaneous_entry_generations_current(
        &self,
        batch: &SimultaneousEntryBatch,
        source_generations: &[(ObjectId, u64, Zone)],
    ) -> bool {
        source_generations
            .iter()
            .all(|(object_id, generation, zone)| {
                self.state.objects.get(object_id).is_some_and(|object| {
                    object.zone == *zone
                        && self
                            .state
                            .zone_change_generation
                            .get(object_id)
                            .copied()
                            .unwrap_or(0)
                            == *generation
                })
            })
            && Self::simultaneous_entry_references(batch)
                .iter()
                .all(|event| {
                    source_generations
                        .iter()
                        .any(|(object_id, _, _)| *object_id == event.object_id)
                })
    }

    /// Prepares every counter instruction in a simultaneous entry parent before any entrant is
    /// moved. `None` means a replacement-order choice has parked the full parent batch.
    pub(in crate::engine) fn begin_entry_counter_placements_for_batch(
        &mut self,
        mut batch: SimultaneousEntryBatch,
        stack: Option<ParkedStackResolution>,
        out: &mut Vec<rv1::RuledEvent>,
    ) -> Result<Option<SimultaneousEntryBatch>, EngineError> {
        let references = Self::simultaneous_entry_references(&batch);
        let exclusions = Self::entry_counter_excluded_sources(&batch);
        let source_generations = self.simultaneous_entry_source_generations(&batch);
        let proposals = self.proposed_entry_counter_placements(&references, &exclusions);
        if proposals.is_empty() {
            for event in Self::simultaneous_entry_references_mut(&mut batch) {
                event.entry_counter_replacements_resolved = true;
            }
            return Ok(Some(batch));
        }
        let (counter_batch, candidates) = self.prepare_counter_placement_batch(proposals)?;
        if let Some(candidates) = candidates {
            self.park_entry_counter_placement_choice(
                counter_batch,
                candidates,
                crate::state::EntryCounterReplacementResume::Batch {
                    stack,
                    batch,
                    source_generations,
                },
                out,
            );
            return Ok(None);
        }
        for event in Self::simultaneous_entry_references_mut(&mut batch) {
            Self::apply_entry_counter_placement_results(event, &counter_batch.events)?;
        }
        Ok(Some(batch))
    }

    pub(in crate::engine) fn validate_entry_counter_replacement_payload(
        &self,
        pending: &PendingResolution,
        resume: &crate::state::EntryCounterReplacementResume,
        counter_batch: &PendingCounterPlacementBatch,
    ) -> Result<(), EngineError> {
        let Some(anchor_proposal) = counter_batch.events.get(counter_batch.event_index) else {
            return Err(EngineError::Illegal("entry-counter choice is stale"));
        };
        let anchor_object_id = match &anchor_proposal.recipient {
            CounterPlacementRecipient::EnteringObject { object_id, .. } => *object_id,
            _ => return Err(EngineError::Illegal("entry-counter choice is stale")),
        };
        let resume_entries = match resume {
            crate::state::EntryCounterReplacementResume::Single { event, .. } => {
                vec![event.as_ref()]
            }
            crate::state::EntryCounterReplacementResume::Batch { batch, .. } => {
                Self::simultaneous_entry_references(batch)
            }
        };
        if let crate::state::EntryCounterReplacementResume::Batch {
            batch,
            source_generations,
            ..
        } = resume
        {
            if !self.simultaneous_entry_generations_current(batch, source_generations) {
                return Err(EngineError::Illegal(
                    "simultaneous entry cohort became stale",
                ));
            }
        }
        let mut seen_placements: BTreeMap<ObjectId, BTreeSet<usize>> = BTreeMap::new();
        for proposal in &counter_batch.events {
            let (object_id, generation) = match &proposal.recipient {
                CounterPlacementRecipient::EnteringObject {
                    object_id,
                    zone_change_generation,
                } => (*object_id, *zone_change_generation),
                _ => return Err(EngineError::Illegal("entry-counter recipient changed")),
            };
            let mut matching_entries = resume_entries
                .iter()
                .copied()
                .filter(|event| event.object_id == object_id);
            let Some(entry_event) = matching_entries.next() else {
                return Err(EngineError::Illegal("entering permanent became stale"));
            };
            if matching_entries.next().is_some() {
                return Err(EngineError::Illegal(
                    "entry-counter recipient has ambiguous entry context",
                ));
            }
            if object_id == anchor_object_id
                && entry_event.destination_controller != pending.deciding_player
            {
                return Err(EngineError::Illegal(
                    "entry-counter chooser no longer controls the entry",
                ));
            }
            let Some(context) = proposal.entry_context.as_ref() else {
                return Err(EngineError::Illegal("entry-counter context is missing"));
            };
            let Some(entrant) = self.state.objects.get(&object_id) else {
                return Err(EngineError::Illegal("entry-counter recipient became stale"));
            };
            let current_generation = self
                .state
                .zone_change_generation
                .get(&object_id)
                .copied()
                .unwrap_or(0);
            let current_compleated = self.entry_compleated_replacement(entry_event);
            if generation != current_generation
                || context.source_zone_change_generation != generation
                || context.source_zone != entrant.zone
                || context.destination_controller != entry_event.destination_controller
                || context.compleated != current_compleated
                || proposal.original_counters != context.history.baseline_counters
            {
                return Err(EngineError::Illegal(
                    "entry-counter recipient or intrinsic replacement changed",
                ));
            }
            if !seen_placements
                .entry(object_id)
                .or_default()
                .insert(context.placement_index)
            {
                return Err(EngineError::Illegal(
                    "entry-counter instruction was proposed more than once",
                ));
            }
            let Some(placement) = entry_event
                .entry_counter_placements
                .get(context.placement_index)
            else {
                return Err(EngineError::Illegal("entry-counter instruction is stale"));
            };
            if placement.actor != proposal.actor
                || !entry_counter_sources_match(proposal.entry_source.as_ref(), &placement.source)
            {
                return Err(EngineError::Illegal(
                    "entry-counter instruction provenance changed",
                ));
            }
            let mut saved_battlefield_receipts =
                context.history.baseline_battlefield_receipts.clone();
            if let Some(saved_history) = placement.history.as_ref() {
                if saved_history.baseline_counters != context.history.baseline_counters
                    || saved_history.baseline_battlefield_receipts
                        != context.history.baseline_battlefield_receipts
                    || !context
                        .history
                        .receipts
                        .starts_with(&saved_history.receipts)
                {
                    return Err(EngineError::Illegal(
                        "entry-counter replacement history changed",
                    ));
                }
                for receipt in &saved_history.receipts {
                    if let EntryCounterReplacementReceipt::Battlefield { instance } = receipt {
                        saved_battlefield_receipts.push(instance.clone());
                    }
                }
            } else if placement.counters != context.history.baseline_counters
                || placement.applied_replacements != context.history.baseline_battlefield_receipts
            {
                return Err(EngineError::Illegal(
                    "entry-counter replacement baseline changed",
                ));
            }
            if placement.applied_replacements != saved_battlefield_receipts {
                return Err(EngineError::Illegal(
                    "entry battlefield replacement receipts changed",
                ));
            }
            let mut expected_battlefield_receipts =
                context.history.baseline_battlefield_receipts.clone();
            for receipt in &context.history.receipts {
                if let EntryCounterReplacementReceipt::Battlefield { instance } = receipt {
                    expected_battlefield_receipts.push(instance.clone());
                }
            }
            if proposal.applied_replacements != expected_battlefield_receipts {
                return Err(EngineError::Illegal(
                    "entry battlefield replacement receipts changed",
                ));
            }
        }
        Self::validate_entry_counter_history(&counter_batch.events)?;
        Ok(())
    }

    fn validate_entry_counter_replacement_resume(
        &self,
        pending: &PendingResolution,
        resume: &crate::state::EntryCounterReplacementResume,
        counter_batch: &PendingCounterPlacementBatch,
        application: &CounterPlacementApplication,
    ) -> Result<(), EngineError> {
        if application.event_index != counter_batch.event_index
            || pending.presentation.choice_kind != rv1::ChoiceKind::ReplacementEffect
            || !pending
                .presentation
                .candidates
                .contains(&application.choice_id)
        {
            return Err(EngineError::Illegal(
                "entry-counter replacement application is stale",
            ));
        }
        let Some(current_proposal) = counter_batch.events.get(application.event_index) else {
            return Err(EngineError::Illegal("entry-counter choice is stale"));
        };
        if !matches!(
            &current_proposal.recipient,
            CounterPlacementRecipient::EnteringObject { .. }
        ) {
            return Err(EngineError::Illegal("entry-counter choice is stale"));
        }
        self.validate_entry_counter_replacement_payload(pending, resume, counter_batch)
    }

    pub(in crate::engine) fn finish_entry_counter_replacement_choice(
        &mut self,
        pending: PendingResolution,
        chosen_application_id: u32,
    ) -> Result<RuledEventBatch, EngineError> {
        let saved_pending = pending.clone();
        let saved_replacement_event = self.state.pending_replacement_event.clone();
        let prepared = (|| -> Result<_, EngineError> {
            let resume = match &pending.continuation {
                ResolutionContinuation::EntryCounterReplacement { resume } => (**resume).clone(),
                _ => {
                    return Err(EngineError::Illegal(
                        "entry-counter replacement continuation is missing",
                    ));
                }
            };
            let Some(saved_event) = saved_replacement_event.as_ref() else {
                return Err(EngineError::Illegal("entry-counter choice is stale"));
            };
            let counter_batch = match saved_event.clone() {
                PendingReplacementEvent::Counters(batch) => *batch,
                _ => return Err(EngineError::Illegal("entry-counter choice is stale")),
            };
            if counter_batch
                .applications
                .iter()
                .any(|application| application.event_index != counter_batch.event_index)
            {
                return Err(EngineError::Illegal(
                    "entry-counter applications do not match the saved anchor",
                ));
            }
            let application_ids = counter_batch
                .applications
                .iter()
                .map(|application| application.choice_id)
                .collect::<Vec<_>>();
            if application_ids != pending.presentation.candidates {
                return Err(EngineError::Illegal(
                    "entry-counter application identities are stale",
                ));
            }
            let Some(application) = counter_batch
                .applications
                .iter()
                .find(|application| application.choice_id == chosen_application_id)
                .cloned()
            else {
                return Err(EngineError::Illegal(
                    "entry-counter replacement application is stale",
                ));
            };
            self.validate_entry_counter_replacement_resume(
                &pending,
                &resume,
                &counter_batch,
                &application,
            )?;
            let live_operations = self
                .counter_placement_candidates_for_batch(
                    &counter_batch.events,
                    counter_batch.event_index,
                )
                .into_iter()
                .map(|(replacement, _)| replacement)
                .collect::<Vec<_>>();
            let saved_operations = counter_batch
                .applications
                .iter()
                .map(|application| application.replacement.clone())
                .collect::<Vec<_>>();
            if live_operations != saved_operations {
                return Err(EngineError::Illegal(
                    "entry-counter replacement offer is no longer current",
                ));
            }
            Ok((counter_batch, application, resume))
        })();
        let (mut counter_batch, application, mut resume) = match prepared {
            Ok(prepared) => prepared,
            Err(error) => {
                self.state.pending_resolution = Some(saved_pending);
                self.state.pending_replacement_event = saved_replacement_event;
                return Err(error);
            }
        };

        let progressed = (|| -> Result<_, EngineError> {
            Self::apply_counter_placement_replacement(
                &mut counter_batch,
                &application.replacement,
            )?;
            counter_batch.applications.clear();
            let candidates = self.advance_counter_placement_batch(&mut counter_batch)?;
            Ok((counter_batch, candidates))
        })();
        let (counter_batch, candidates) = match progressed {
            Ok(progressed) => progressed,
            Err(error) => {
                self.state.pending_resolution = Some(saved_pending.clone());
                self.state.pending_replacement_event = saved_replacement_event.clone();
                return Err(error);
            }
        };

        if candidates.is_none() {
            let writeback = match &mut resume {
                crate::state::EntryCounterReplacementResume::Single { event, .. } => {
                    Self::apply_entry_counter_placement_results(event, &counter_batch.events)
                }
                crate::state::EntryCounterReplacementResume::Batch { batch, .. } => {
                    let mut result = Ok(());
                    for event in Self::simultaneous_entry_references_mut(batch) {
                        if let Err(error) = Self::apply_entry_counter_placement_results(
                            event,
                            &counter_batch.events,
                        ) {
                            result = Err(error);
                            break;
                        }
                    }
                    result
                }
            };
            if let Err(error) = writeback {
                self.state.pending_resolution = Some(saved_pending.clone());
                self.state.pending_replacement_event = saved_replacement_event.clone();
                return Err(error);
            }
        }

        self.state.pending_resolution = None;
        self.state.pending_replacement_event = None;
        let mut events = Vec::new();
        if let Some(candidates) = candidates {
            self.park_entry_counter_placement_choice(
                counter_batch,
                candidates,
                resume,
                &mut events,
            );
            return Ok(finish_with_events(self, events));
        }

        match resume {
            crate::state::EntryCounterReplacementResume::Single {
                stack: _,
                event,
                completion,
            } => self.complete_pending_battlefield_entry(pending, *event, completion, events),
            crate::state::EntryCounterReplacementResume::Batch { stack, batch, .. } => {
                self.complete_entry_timestamp_order(batch, stack, events)
            }
        }
    }
    fn saga_lore_counter_batch_is_current(
        &self,
        events: &[ProposedCounterPlacement],
        completion: &crate::state::CounterPlacementCompletion,
    ) -> bool {
        let crate::state::CounterPlacementCompletion::SagaLore {
            active_player,
            sagas,
        } = completion
        else {
            return false;
        };
        if events.len() != sagas.len() {
            return false;
        }
        let mut expected = sagas
            .iter()
            .map(|saga| (saga.object_id, saga.zone_change_generation))
            .collect::<Vec<_>>();
        let mut actual = Vec::with_capacity(events.len());
        for event in events {
            let CounterPlacementRecipient::Object {
                object_id,
                zone_change_generation,
            } = &event.recipient
            else {
                return false;
            };
            let Some(characteristics) = self.characteristics(*object_id) else {
                return false;
            };
            if event.actor != *active_player
                || event.origin != super::super::continuous::CounterPlacementOrigin::TurnBased
                || event.original_counters != BTreeMap::from([(CounterKind::Lore, 1)])
                || self
                    .state
                    .zone_change_generation
                    .get(object_id)
                    .copied()
                    .unwrap_or(0)
                    != *zone_change_generation
                || !self
                    .state
                    .objects
                    .get(object_id)
                    .is_some_and(|object| object.zone == Zone::Battlefield)
                || characteristics.controller != *active_player
                || !characteristics.has_type("Enchantment")
                || !characteristics.has_type("Saga")
                || !self.can_receive_counters(*object_id)
            {
                return false;
            }
            actual.push((*object_id, *zone_change_generation));
        }
        expected.sort_unstable();
        actual.sort_unstable();
        actual == expected
    }

    fn finish_unstacked_saga_lore_action(
        &mut self,
        mut out: Vec<rv1::RuledEvent>,
    ) -> Result<RuledEventBatch, EngineError> {
        self.apply_sbas(&mut out)?;
        self.flush_staged_triggers(&mut out);
        if self.state.blocking_choice().is_none() {
            out.push(ev_priority_changed(self));
        }
        Ok(finish_with_events(self, out))
    }

    pub(in crate::engine) fn commit_counter_placement_batch(
        &mut self,
        events: &[ProposedCounterPlacement],
        completion: &crate::state::CounterPlacementCompletion,
        parked_stack: Option<&ParkedStackResolution>,
        out: &mut Vec<rv1::RuledEvent>,
    ) -> Result<Vec<CounterPlacementReceipt>, EngineError> {
        self.validate_counter_placement_completion_scope(events, completion, parked_stack)?;
        if matches!(
            completion,
            crate::state::CounterPlacementCompletion::SagaLore { .. }
        ) && !self.saga_lore_counter_batch_is_current(events, completion)
        {
            return Err(EngineError::Illegal("Saga lore counter batch became stale"));
        }
        if matches!(
            completion,
            crate::state::CounterPlacementCompletion::ActivationCost { .. }
        ) {
            return Err(EngineError::Illegal(
                "activation-cost counters must commit with their payment transaction",
            ));
        }
        if let crate::state::CounterPlacementCompletion::Amass {
            controller,
            target_object_id,
            target_zone_change_generation,
            ..
        } = completion
        {
            if self
                .state
                .zone_change_generation
                .get(target_object_id)
                .copied()
                .unwrap_or(0)
                != *target_zone_change_generation
                || !self.is_amass_candidate(*target_object_id, *controller)
            {
                return Err(EngineError::Illegal("Amass Army became stale"));
            }
        }
        if let crate::state::CounterPlacementCompletion::BlightEffect {
            player,
            count,
            creature,
            ..
        } = completion
        {
            let is_current = events.len() == 1
                && events.first().is_some_and(|event| {
                    matches!(
                        &event.recipient,
                        CounterPlacementRecipient::Object {
                            object_id,
                            zone_change_generation
                        } if *object_id == creature.object_id
                            && *zone_change_generation == creature.zone_change_generation
                    ) && event.actor == *player
                        && event.origin == super::super::continuous::CounterPlacementOrigin::Effect
                        && event.original_counters
                            == BTreeMap::from([(CounterKind::MinusOneMinusOne, *count)])
                })
                && self
                    .state
                    .zone_change_generation
                    .get(&creature.object_id)
                    .copied()
                    .unwrap_or(0)
                    == creature.zone_change_generation
                && self.can_blight_creature(*player, creature.object_id);
            if !is_current {
                return Err(EngineError::Illegal("Blight creature became stale"));
            }
        }
        if let crate::state::CounterPlacementCompletion::Explore {
            controller,
            explorer,
            revealed,
            ..
        } = completion
        {
            let is_current = events.len() == 1
                && events.first().is_some_and(|event| {
                    matches!(
                        &event.recipient,
                        CounterPlacementRecipient::Object {
                            object_id,
                            zone_change_generation
                        } if *object_id == explorer.object_id
                            && *zone_change_generation == explorer.zone_change_generation
                    ) && event.actor == *controller
                        && event.origin == super::super::continuous::CounterPlacementOrigin::Effect
                        && event.original_counters
                            == BTreeMap::from([(CounterKind::PlusOnePlusOne, 1)])
                })
                && self
                    .state
                    .zone_change_generation
                    .get(&explorer.object_id)
                    .copied()
                    .unwrap_or(0)
                    == explorer.zone_change_generation
                && self.can_receive_counters(explorer.object_id)
                && revealed.is_none_or(|revealed| {
                    self.state
                        .zone_change_generation
                        .get(&revealed.object_id)
                        .copied()
                        .unwrap_or(0)
                        == revealed.zone_change_generation
                        && self
                            .state
                            .objects
                            .get(&revealed.object_id)
                            .is_some_and(|object| object.zone == Zone::Library)
                        && self
                            .state
                            .player_idx(*controller)
                            .and_then(|index| self.state.players[index].library.front().copied())
                            == Some(revealed.object_id)
                });
            if !is_current {
                return Err(EngineError::Illegal("Explore event became stale"));
            }
        }
        if let crate::state::CounterPlacementCompletion::CounterMove {
            source_target,
            destination_target,
            kind,
            actor,
            ..
        } = completion
        {
            let Some(source_generation) = source_target.zone_change_generation else {
                return Err(EngineError::Illegal(
                    "counter-move source generation missing",
                ));
            };
            let Some(destination_generation) = destination_target.zone_change_generation else {
                return Err(EngineError::Illegal(
                    "counter-move destination generation missing",
                ));
            };
            let matches_event = events.len() == 1
                && events[0].actor == *actor
                && events[0].origin == super::super::continuous::CounterPlacementOrigin::Effect
                && events[0].original_counters.len() == 1
                && events[0].original_counters.get(kind) == Some(&1)
                && (events[0].counters.values().any(|count| *count > 0)
                    || Self::counter_move_zero_result_has_authenticated_replacement_history(
                        &events[0],
                    ))
                && events[0]
                    .counter_move_context
                    .as_ref()
                    .is_some_and(|context| {
                        context.source_object_id == source_target.object_id
                            && context.source_zone_change_generation == source_generation
                            && context.kind == *kind
                    })
                && matches!(
                    &events[0].recipient,
                    CounterPlacementRecipient::Object {
                        object_id,
                        zone_change_generation,
                    } if *object_id == destination_target.object_id
                        && *zone_change_generation == destination_generation
                );
            if !matches_event
                || source_target.object_id == destination_target.object_id
                || self
                    .state
                    .zone_change_generation
                    .get(&source_target.object_id)
                    .copied()
                    .unwrap_or(0)
                    != source_generation
                || !self
                    .state
                    .objects
                    .get(&source_target.object_id)
                    .is_some_and(|object| {
                        object.zone == Zone::Battlefield && object.counter_count(*kind) > 0
                    })
            {
                return Err(EngineError::Illegal("counter-move completion became stale"));
            }
        }
        for event in events {
            match &event.recipient {
                CounterPlacementRecipient::Object {
                    object_id,
                    zone_change_generation,
                } if self
                    .state
                    .zone_change_generation
                    .get(object_id)
                    .copied()
                    .unwrap_or(0)
                    == *zone_change_generation
                    && (matches!(
                        completion,
                        crate::state::CounterPlacementCompletion::CounterMove { .. }
                    ) || self.can_receive_counters(*object_id)) => {}
                CounterPlacementRecipient::EnteringObject { .. } => {
                    return Err(EngineError::Illegal(
                        "entry counters must commit with their battlefield entry",
                    ));
                }
                CounterPlacementRecipient::Player { player_id }
                    if self
                        .state
                        .player_idx(*player_id)
                        .is_some_and(|index| !self.state.players[index].has_lost) => {}
                _ => {
                    return Err(EngineError::Illegal(
                        "counter-placement recipient became stale",
                    ))
                }
            }
        }
        if let crate::state::CounterPlacementCompletion::CounterMove {
            source_target,
            destination_target,
            kind,
            ..
        } = completion
        {
            if !self.counter_move_can_complete(
                source_target.object_id,
                destination_target.object_id,
                *kind,
            ) {
                return Err(EngineError::Illegal(
                    "counter-move destination cannot receive counters",
                ));
            }
        }
        let counter_move_siege_defeat =
            if let crate::state::CounterPlacementCompletion::CounterMove {
                source_target,
                kind: CounterKind::Defense,
                ..
            } = completion
            {
                self.state.objects[&source_target.object_id].counter_count(CounterKind::Defense)
                    == 1
                    && self.characteristics(source_target.object_id).is_some_and(
                        |characteristics| {
                            characteristics.has_type("Battle") && characteristics.has_type("Siege")
                        },
                    )
            } else {
                false
            };
        if let crate::state::CounterPlacementCompletion::CounterMove {
            source_target,
            kind,
            ..
        } = completion
        {
            let source = self
                .state
                .objects
                .get_mut(&source_target.object_id)
                .expect("counter-move source was validated");
            let before = source.counter_count(*kind);
            source.set_counter(*kind, before - 1);
        }
        let mut game_events = Vec::new();
        let mut receipts = Vec::new();
        let timestamp = self.state.command_index;
        if let crate::state::CounterPlacementCompletion::Proliferate { transaction } = completion {
            out.push(ev_log(format!("P{} proliferates.", transaction.actor)));
        }
        for event in events {
            let mut recipient_received_counters = false;
            for replacement in &event.applied_replacements {
                let source_name =
                    self.replacement_object_display_name(replacement.source_object_id, None);
                out.push(ev_log(format!(
                    "{source_name}'s counter replacement effect is applied."
                )));
            }
            match (&event.recipient, completion) {
                (
                    CounterPlacementRecipient::Object { .. },
                    crate::state::CounterPlacementCompletion::DoubleYourCounters { .. },
                )
                | (
                    CounterPlacementRecipient::Object { .. },
                    crate::state::CounterPlacementCompletion::PoisonTargetUpToNine { .. },
                )
                | (
                    CounterPlacementRecipient::Object { .. },
                    crate::state::CounterPlacementCompletion::TargetedPoison { .. },
                ) => {
                    return Err(EngineError::Illegal(
                        "DoubleYourCounters cannot place counters on a permanent",
                    ));
                }
                (
                    CounterPlacementRecipient::Object { .. },
                    crate::state::CounterPlacementCompletion::StackEffect { .. },
                )
                | (
                    CounterPlacementRecipient::Object { .. },
                    crate::state::CounterPlacementCompletion::DistributedCounters { .. },
                )
                | (
                    CounterPlacementRecipient::Object { .. },
                    crate::state::CounterPlacementCompletion::Amass { .. },
                )
                | (
                    CounterPlacementRecipient::Object { .. },
                    crate::state::CounterPlacementCompletion::BlightEffect { .. },
                )
                | (
                    CounterPlacementRecipient::Object { .. },
                    crate::state::CounterPlacementCompletion::Explore { .. },
                )
                | (
                    CounterPlacementRecipient::Object { .. },
                    crate::state::CounterPlacementCompletion::DamageBatch { .. },
                )
                | (
                    CounterPlacementRecipient::Object { .. },
                    crate::state::CounterPlacementCompletion::DamageReplacement { .. },
                )
                | (
                    CounterPlacementRecipient::Object { .. },
                    crate::state::CounterPlacementCompletion::ActivationCost { .. },
                )
                | (
                    CounterPlacementRecipient::Object { .. },
                    crate::state::CounterPlacementCompletion::ProgramChooseBlight { .. },
                )
                | (
                    CounterPlacementRecipient::Object { .. },
                    crate::state::CounterPlacementCompletion::Proliferate { .. },
                )
                | (
                    CounterPlacementRecipient::Object { .. },
                    crate::state::CounterPlacementCompletion::SagaLore { .. },
                )
                | (
                    CounterPlacementRecipient::Object { .. },
                    crate::state::CounterPlacementCompletion::CounterMove { .. },
                ) => {}
                (CounterPlacementRecipient::EnteringObject { .. }, _) => {
                    return Err(EngineError::Illegal(
                        "entry counters must commit with their battlefield entry",
                    ));
                }
                (
                    CounterPlacementRecipient::Player { .. },
                    crate::state::CounterPlacementCompletion::Proliferate { .. },
                ) => {}
                (
                    CounterPlacementRecipient::Player { .. },
                    crate::state::CounterPlacementCompletion::DoubleYourCounters { .. },
                )
                | (
                    CounterPlacementRecipient::Player { .. },
                    crate::state::CounterPlacementCompletion::PoisonTargetUpToNine { .. },
                )
                | (
                    CounterPlacementRecipient::Player { .. },
                    crate::state::CounterPlacementCompletion::TargetedPoison { .. },
                ) => {}
                (
                    CounterPlacementRecipient::Player { .. },
                    crate::state::CounterPlacementCompletion::StackEffect { .. }
                    | crate::state::CounterPlacementCompletion::DistributedCounters { .. }
                    | crate::state::CounterPlacementCompletion::Amass { .. }
                    | crate::state::CounterPlacementCompletion::ActivationCost { .. }
                    | crate::state::CounterPlacementCompletion::ProgramChooseBlight { .. }
                    | crate::state::CounterPlacementCompletion::SagaLore { .. }
                    | crate::state::CounterPlacementCompletion::BlightEffect { .. }
                    | crate::state::CounterPlacementCompletion::Explore { .. }
                    | crate::state::CounterPlacementCompletion::DamageBatch { .. }
                    | crate::state::CounterPlacementCompletion::DamageReplacement { .. }
                    | crate::state::CounterPlacementCompletion::CounterMove { .. },
                ) => {
                    return Err(EngineError::Illegal(
                        "stack-effect counter recipient must be a permanent",
                    ));
                }
            }
            for (&kind, &count) in &event.counters {
                if count == 0 {
                    continue;
                }
                match &event.recipient {
                    CounterPlacementRecipient::Object { object_id, .. } => {
                        let before = self.state.objects[object_id].counter_count(kind);
                        self.state
                            .objects
                            .get_mut(object_id)
                            .expect("validated counter recipient")
                            .add_counters(kind, count, timestamp);
                        let after = self.state.objects[object_id].counter_count(kind);
                        let placed = after.saturating_sub(before);
                        if placed == 0 {
                            continue;
                        }
                        recipient_received_counters = true;
                        let controller = self.controller_of(*object_id).unwrap_or(event.actor);
                        let generation = self
                            .state
                            .zone_change_generation
                            .get(object_id)
                            .copied()
                            .unwrap_or(0);
                        receipts.push(CounterPlacementReceipt {
                            object: TriggerObjectRef {
                                object_id: *object_id,
                                zone_change_generation: generation,
                                controller_at_event: controller,
                            },
                            counter: kind,
                            count: placed,
                            before,
                            after,
                        });
                        game_events.push(GameEvent::CountersPlaced {
                            object: TriggerObjectRef {
                                object_id: *object_id,
                                zone_change_generation: generation,
                                controller_at_event: controller,
                            },
                            kind,
                            before,
                            after,
                            read_ahead_entry: false,
                        });
                        if let crate::state::CounterPlacementCompletion::StackEffect {
                            spell_label,
                        } = completion
                        {
                            out.push(ev_log(format!(
                                "{spell_label} puts {placed} {} counter(s) on {}",
                                kind.label(),
                                self.replacement_object_display_name(*object_id, None),
                            )));
                        }
                    }
                    CounterPlacementRecipient::EnteringObject { .. } => unreachable!(
                        "entry counter placements are committed with their entry event"
                    ),
                    CounterPlacementRecipient::Player { player_id } => {
                        let index = self
                            .state
                            .player_idx(*player_id)
                            .expect("validated counter recipient");
                        let before = self.state.players[index]
                            .counters
                            .get(&kind)
                            .copied()
                            .unwrap_or(0);
                        let after = before.saturating_add(count);
                        self.state.players[index].counters.insert(kind, after);
                        recipient_received_counters |= after > before;
                    }
                }
            }
            if recipient_received_counters {
                match (&event.recipient, completion) {
                    (
                        CounterPlacementRecipient::Object { object_id, .. },
                        crate::state::CounterPlacementCompletion::Proliferate { .. },
                    ) => out.push(ev_log(format!(
                        "{} gets one of each kind of counter on it.",
                        self.replacement_object_display_name(*object_id, None),
                    ))),
                    (
                        CounterPlacementRecipient::Object { object_id, .. },
                        crate::state::CounterPlacementCompletion::SagaLore {
                            active_player, ..
                        },
                    ) => out.push(ev_log(format!(
                        "P{active_player} puts a lore counter on {}.",
                        self.replacement_object_display_name(*object_id, None),
                    ))),
                    (
                        CounterPlacementRecipient::Player { player_id },
                        crate::state::CounterPlacementCompletion::Proliferate { .. },
                    ) => out.push(ev_log(format!(
                        "P{player_id} gets one of each kind of counter on them."
                    ))),
                    _ => {}
                }
            }
        }
        if let crate::state::CounterPlacementCompletion::Proliferate { transaction } = completion {
            game_events.push(GameEvent::Proliferated {
                player: transaction.actor,
            });
        }
        if let crate::state::CounterPlacementCompletion::Amass {
            controller,
            subtype,
            count,
            source_id,
            target_object_id,
            ..
        } = completion
        {
            if !self
                .characteristics(*target_object_id)
                .is_some_and(|characteristics| characteristics.has_type(subtype.as_str()))
            {
                self.state.continuous_effects.push(ContinuousEffect {
                    trigger_grant_origin: None,
                    source_id: Some(*source_id),
                    affected: AffectedScope::Single(*target_object_id),
                    kind: ContinuousEffectKind::Layer4AddTypes(
                        tricerules_card_model::primitives::TypeLineAddition {
                            land_types: Vec::new(),
                            card_types: Vec::<PermanentTypeFilter>::new(),
                            creature_types: vec![subtype.as_str().to_string()],
                        },
                    ),
                    condition: None,
                    duration: EffectDuration::Indefinite,
                    timestamp: self.state.command_index,
                });
            }
            out.push(ev_log(format!(
                "P{controller} amasses {} {count} on {}.",
                subtype.plural(),
                self.replacement_object_display_name(*target_object_id, None)
            )));
        }
        if let crate::state::CounterPlacementCompletion::CounterMove {
            source_target,
            destination_target,
            kind,
            ability_label,
            ..
        } = completion
        {
            if receipts.iter().any(|receipt| {
                receipt.object.object_id == destination_target.object_id
                    && receipt.counter == *kind
                    && receipt.count > 0
            }) {
                out.push(ev_log(format!(
                    "{ability_label} moves one {} counter from {} to {}.",
                    kind.label(),
                    self.replacement_object_display_name(source_target.object_id, None),
                    self.replacement_object_display_name(destination_target.object_id, None),
                )));
            }
            if counter_move_siege_defeat {
                self.stage_siege_defeat_trigger(source_target.object_id);
            }
        }
        if !game_events.is_empty()
            && !matches!(
                completion,
                crate::state::CounterPlacementCompletion::DamageBatch { .. }
                    | crate::state::CounterPlacementCompletion::DamageReplacement { .. }
            )
        {
            self.fire_triggers(&game_events, out);
        }
        Ok(receipts)
    }

    /// Recognize the already-begun flat combined-counter instruction from its captured ability,
    /// saved cursor, and effect-origin batch, without consulting its possibly departed source.
    pub(in crate::engine) fn begun_combined_counter_placement(
        &self,
        pending: &PendingResolution,
        replacement: Option<&PendingReplacementEvent>,
    ) -> bool {
        let ResolutionContinuation::CounterReplacement {
            stack: Some(stack),
            completion: crate::state::CounterPlacementCompletion::StackEffect { .. },
        } = &pending.continuation
        else {
            return false;
        };
        let Some(PendingReplacementEvent::Counters(batch)) = replacement else {
            return false;
        };
        !stack.item.is_triggered
            && stack.item.ability_text.is_some()
            && stack.program().is_none()
            && stack.resume_effect_index() == Some(1)
            && pending.presentation.source_object_id == stack.item.id
            && stack
                .item
                .activated_ability
                .as_ref()
                .is_some_and(|ability| {
                    ability.effect.as_slice()
                        == [SpellEffectKind::PutCountersOnCreaturesAndOtherPlaneswalkers]
                })
            && batch.event_index < batch.events.len()
            && batch.events.iter().all(|event| {
                matches!(&event.recipient, CounterPlacementRecipient::Object { .. })
                    && event.actor == stack.item.controller
                    && event.origin == super::super::continuous::CounterPlacementOrigin::Effect
            })
    }

    /// Recognize the begun Wolf death instruction from its captured trigger and effect batch.
    /// The token may already have ceased, and recipients may have departed or changed
    /// incarnation; those live facts are reconciled only after the structural match.
    pub(in crate::engine) fn begun_wolf_counter_placement(
        &self,
        pending: &PendingResolution,
        replacement: Option<&PendingReplacementEvent>,
    ) -> bool {
        use tricerules_card_model::primitives::PlaneswalkerCounterSubtype;
        let ResolutionContinuation::CounterReplacement {
            stack: Some(stack),
            completion: crate::state::CounterPlacementCompletion::StackEffect { .. },
        } = &pending.continuation
        else {
            return false;
        };
        let Some(PendingReplacementEvent::Counters(batch)) = replacement else {
            return false;
        };
        stack.item.is_triggered
            && stack.item.ability_text.is_some()
            && stack.item.source_permanent_id.is_some()
            && stack.item.activated_ability.is_none()
            && stack.program().is_none()
            && stack.resume_effect_index() == Some(1)
            && pending.presentation.source_object_id == stack.item.id
            && stack
                .item
                .triggered_ability
                .as_ref()
                .is_some_and(|ability| {
                    ability.trigger == TriggerCondition::WhenSelfDies
                        && ability.program.is_none()
                        && ability.modal.is_none()
                        && ability.targeting.is_none()
                        && !ability.may
                        && ability.intervening_if.is_none()
                        && !ability.triggers_only_once
                        && ability.max_triggers_per_turn.is_none()
                        && matches!(
                            ability.effect.as_slice(),
                            [SpellEffectKind::PutCountersAllPlaneswalkers {
                                counter: CounterKind::Loyalty,
                                count: Amount::Fixed(1),
                                exclude_self: false,
                                subtype: Some(PlaneswalkerCounterSubtype::Garruk),
                            }]
                        )
                })
            && batch.event_index < batch.events.len()
            && batch.events.iter().all(|event| {
                matches!(&event.recipient, CounterPlacementRecipient::Object { .. })
                    && event.actor == stack.item.controller
                    && event.origin == super::super::continuous::CounterPlacementOrigin::Effect
                    && event.original_counters.len() == 1
                    && event.original_counters.get(&CounterKind::Loyalty) == Some(&1)
            })
    }

    /// Recognize one already-chosen counter addition while its ordinary placement replacements
    /// wait. The proposal itself carries the selected kind and target generation; no old counter
    /// of that kind is required to remain on the target.
    pub(in crate::engine) fn begun_chosen_counter_addition(
        &self,
        pending: &PendingResolution,
        replacement: Option<&PendingReplacementEvent>,
    ) -> bool {
        let ResolutionContinuation::CounterReplacement {
            stack: Some(stack),
            completion: crate::state::CounterPlacementCompletion::StackEffect { .. },
        } = &pending.continuation
        else {
            return false;
        };
        let Some(PendingReplacementEvent::Counters(batch)) = replacement else {
            return false;
        };
        if stack.program().is_some()
            || pending.presentation.source_object_id != stack.item.id
            || batch.event_index >= batch.events.len()
        {
            return false;
        }
        let Some(effect_index) = stack
            .resume_effect_index()
            .and_then(|resume| resume.checked_sub(1))
        else {
            return false;
        };
        if !matches!(
            self.build_resolution_effects(&stack.item)
                .0
                .get(effect_index as usize)
                .map(|entry| &entry.effect),
            Some(SpellEffectKind::AddOneCounterOfChosenKindToTarget)
        ) {
            return false;
        }
        let targets = stack
            .item
            .targets
            .iter()
            .filter(|target| target.group_index == 0)
            .collect::<Vec<_>>();
        let [target] = targets.as_slice() else {
            return false;
        };
        let Some(target_generation) = target.zone_change_generation else {
            return false;
        };
        batch.events.len() == 1
            && batch.events[0].actor == stack.item.controller
            && batch.events[0].origin == super::super::continuous::CounterPlacementOrigin::Effect
            && batch.events[0].original_counters.len() == 1
            && batch.events[0].original_counters.values().copied().eq([1])
            && batch.events[0].entry_source.is_none()
            && batch.events[0].entry_context.is_none()
            && batch.events[0].counter_move_context.is_none()
            && batch.events[0].cost_debit_index.is_none()
            && matches!(
                &batch.events[0].recipient,
                CounterPlacementRecipient::Object {
                    object_id,
                    zone_change_generation,
                } if *object_id == target.object_id
                    && *zone_change_generation == target_generation
            )
    }

    fn chosen_counter_addition_batch_is_current(
        &self,
        stack: &ParkedStackResolution,
        events: &[ProposedCounterPlacement],
    ) -> bool {
        let targets = stack
            .item
            .targets
            .iter()
            .filter(|target| target.group_index == 0)
            .collect::<Vec<_>>();
        let [target] = targets.as_slice() else {
            return false;
        };
        let Some(expected_generation) = target.zone_change_generation else {
            return false;
        };
        events.len() == 1
            && matches!(
                &events[0].recipient,
                CounterPlacementRecipient::Object {
                    object_id,
                    zone_change_generation,
                } if *object_id == target.object_id
                    && *zone_change_generation == expected_generation
            )
            && self
                .state
                .zone_change_generation
                .get(&target.object_id)
                .copied()
                == Some(expected_generation)
            && self
                .state
                .objects
                .get(&target.object_id)
                .is_some_and(|object| object.zone == Zone::Battlefield)
            && self.can_receive_counters(target.object_id)
    }

    /// Recompute the supported begun counter-order offers after player departure. Their saved
    /// proposals retain accepted replacements while stale recipients are pruned; a changed
    /// recipient controller receives a fresh CR 616.1 order prompt.
    pub(in crate::engine) fn refresh_counter_placement_departure(
        &mut self,
        out: &mut Vec<rv1::RuledEvent>,
    ) -> Result<(), EngineError> {
        let Some(original_pending) = self.state.pending_resolution.clone() else {
            return Ok(());
        };
        let Some(original_replacement) = self.state.pending_replacement_event.clone() else {
            return Ok(());
        };
        let begun_self_counter =
            self.begun_self_counter_placement(&original_pending, Some(&original_replacement));
        let begun_poison = self
            .begun_poison_difference(&original_pending, Some(&original_replacement))
            || self.begun_targeted_poison_placement(&original_pending, Some(&original_replacement));
        let begun_distribution = self
            .begun_distributed_counter_placement(&original_pending, Some(&original_replacement));
        let begun_flat = begun_self_counter
            || begun_poison
            || begun_distribution
            || self
                .begun_combined_counter_placement(&original_pending, Some(&original_replacement))
            || self.begun_wolf_counter_placement(&original_pending, Some(&original_replacement))
            || self.begun_chosen_counter_addition(&original_pending, Some(&original_replacement));
        let (stack, completion) = match &original_pending.continuation {
            ResolutionContinuation::CounterReplacement {
                stack: Some(stack),
                completion:
                    completion @ crate::state::CounterPlacementCompletion::Proliferate { .. },
            } => (stack.clone(), completion.clone()),
            ResolutionContinuation::CounterReplacement {
                stack: Some(stack),
                completion:
                    completion @ crate::state::CounterPlacementCompletion::StackEffect { .. },
            } if begun_flat => (stack.clone(), completion.clone()),
            ResolutionContinuation::CounterReplacement {
                stack: Some(stack),
                completion:
                    completion @ crate::state::CounterPlacementCompletion::DistributedCounters { .. },
            } if begun_distribution => (stack.clone(), completion.clone()),
            ResolutionContinuation::CounterReplacement {
                stack: Some(stack),
                completion:
                    completion @ crate::state::CounterPlacementCompletion::DoubleYourCounters { .. },
            } => (stack.clone(), completion.clone()),
            ResolutionContinuation::CounterReplacement {
                stack: Some(stack),
                completion:
                    completion @ crate::state::CounterPlacementCompletion::PoisonTargetUpToNine {
                        ..
                    },
            } if begun_poison => (stack.clone(), completion.clone()),
            ResolutionContinuation::CounterReplacement {
                stack: Some(stack),
                completion:
                    completion @ crate::state::CounterPlacementCompletion::TargetedPoison { .. },
            } if begun_poison => (stack.clone(), completion.clone()),
            _ => return Ok(()),
        };
        let PendingReplacementEvent::Counters(batch) = &original_replacement else {
            return Ok(());
        };
        if self.begun_chosen_counter_addition(&original_pending, Some(&original_replacement))
            && !self.chosen_counter_addition_batch_is_current(&stack, &batch.events)
        {
            self.state.pending_resolution = None;
            self.state.pending_replacement_event = None;
            let completed = self.complete_parked_stack_with_previous(
                stack.clone(),
                stack.previous_result().clone(),
                Vec::new(),
            )?;
            out.extend(completed.events);
            return Ok(());
        }
        self.validate_counter_placement_completion_scope(&batch.events, &completion, Some(&stack))?;
        let mut batch = (**batch).clone();
        let old_decider = original_pending.deciding_player;
        let old_event_index = batch.event_index;

        let recipient_is_current =
            |engine: &Self, event: &ProposedCounterPlacement| match &event.recipient {
                CounterPlacementRecipient::Object {
                    object_id,
                    zone_change_generation,
                } => {
                    engine
                        .state
                        .objects
                        .get(object_id)
                        .is_some_and(|object| object.zone == Zone::Battlefield)
                        && engine
                            .state
                            .zone_change_generation
                            .get(object_id)
                            .copied()
                            .unwrap_or(0)
                            == *zone_change_generation
                        && engine.can_receive_counters(*object_id)
                }
                CounterPlacementRecipient::Player { player_id } => engine
                    .state
                    .player_idx(*player_id)
                    .is_some_and(|index| !engine.state.players[index].has_lost),
                CounterPlacementRecipient::EnteringObject { .. } => false,
            };
        let batch_had_stale_recipient = batch
            .events
            .iter()
            .any(|event| !recipient_is_current(self, event));
        let current_event = batch.events.get(batch.event_index);
        let new_decider = current_event.and_then(|event| match &event.recipient {
            CounterPlacementRecipient::Object { object_id, .. } => self.controller_of(*object_id),
            CounterPlacementRecipient::EnteringObject { .. } => event
                .entry_context
                .as_ref()
                .map(|context| context.destination_controller),
            CounterPlacementRecipient::Player { player_id } => Some(*player_id),
        });
        let existing_candidates = if let Some(event) = current_event {
            self.counter_replacement_candidates(event)
        } else {
            Vec::new()
        };
        for application in batch
            .applications
            .iter()
            .filter(|application| application.event_index == old_event_index)
        {
            single_row_battlefield_instance(&application.replacement, application.event_index)?;
        }
        let old_candidates = batch
            .applications
            .iter()
            .filter(|application| application.event_index == old_event_index)
            .map(|application| application.replacement.clone())
            .collect::<Vec<_>>();
        let new_candidates =
            single_row_counter_placement_candidates(old_event_index, existing_candidates)
                .iter()
                .map(|(candidate, _)| candidate.clone())
                .collect::<Vec<_>>();
        let decider_is_live = self
            .state
            .player_idx(old_decider)
            .is_some_and(|index| !self.state.players[index].has_lost);
        if !batch_had_stale_recipient
            && decider_is_live
            && new_decider == Some(old_decider)
            && old_candidates == new_candidates
        {
            // A source/controller departure unrelated to this recipient does not change the
            // answer semantics. Keep the existing actor-scoped prompt ID and menu.
            return Ok(());
        }

        let completed_count = batch
            .events
            .iter()
            .take(old_event_index)
            .filter(|event| recipient_is_current(self, event))
            .count();
        batch
            .events
            .retain(|event| recipient_is_current(self, event));
        batch.event_index = completed_count.min(batch.events.len());
        batch.applications.clear();
        self.validate_counter_placement_completion_scope(&batch.events, &completion, Some(&stack))?;

        if let Some(candidates) = self.advance_counter_placement_batch(&mut batch)? {
            let event = batch
                .events
                .get(batch.event_index)
                .ok_or(EngineError::Illegal(
                    "counter replacement event disappeared while refreshing",
                ))?;
            let event_decider = match &event.recipient {
                CounterPlacementRecipient::Object { object_id, .. } => {
                    self.controller_of(*object_id)
                }
                CounterPlacementRecipient::EnteringObject { .. } => event
                    .entry_context
                    .as_ref()
                    .map(|context| context.destination_controller),
                CounterPlacementRecipient::Player { player_id } => Some(*player_id),
            };
            let live_decider = event_decider
                .filter(|player| {
                    self.state
                        .player_idx(*player)
                        .is_some_and(|index| !self.state.players[index].has_lost)
                })
                .or_else(|| {
                    self.state
                        .player_idx(old_decider)
                        .filter(|&index| !self.state.players[index].has_lost)
                        .map(|index| self.state.players[index].id)
                })
                .or_else(|| {
                    self.state
                        .player_idx(old_decider)
                        .and_then(|index| self.state.next_in_game_player_idx(index))
                        .map(|index| self.state.players[index].id)
                })
                .ok_or(EngineError::Illegal(
                    "no live player can order counter replacements",
                ))?;
            self.park_counter_placement_choice(batch, candidates, Some(stack), completion, out);
            if let Some(pending) = self.state.pending_resolution.as_mut() {
                pending.deciding_player = live_decider;
            }
            if live_decider != event_decider.unwrap_or(live_decider) {
                if let Some(choice) =
                    out.iter_mut()
                        .rev()
                        .find_map(|event| match event.ev.as_mut() {
                            Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(choice)) => {
                                Some(choice)
                            }
                            _ => None,
                        })
                {
                    choice.deciding_player_id = live_decider;
                }
            }
            return Ok(());
        }

        let original_event_count = out.len();
        self.state.pending_resolution = None;
        self.state.pending_replacement_event = None;
        let receipts = match self.commit_counter_placement_batch(
            &batch.events,
            &completion,
            Some(&stack),
            out,
        ) {
            Ok(receipts) => receipts,
            Err(error) => {
                out.truncate(original_event_count);
                self.state.pending_resolution = Some(original_pending);
                self.state.pending_replacement_event = Some(original_replacement);
                return Err(error);
            }
        };
        if let crate::state::CounterPlacementCompletion::Proliferate { transaction } = &completion {
            let completion_events = self.finish_proliferate_counter_leaf(
                stack,
                transaction.clone(),
                receipts,
                Vec::new(),
            )?;
            out.extend(completion_events.events);
            return Ok(());
        }
        let mut previous_result = if begun_flat {
            stack.previous_result().clone()
        } else {
            crate::state::OperationResults::default()
        };
        if begun_flat {
            previous_result.record_counter_placements(receipts);
        }
        let completion_events =
            self.complete_parked_stack_with_previous(stack, previous_result, Vec::new())?;
        out.extend(completion_events.events);
        Ok(())
    }

    pub(in crate::engine) fn finish_counter_replacement_choice(
        &mut self,
        pending: PendingResolution,
        chosen_application_id: u32,
    ) -> Result<RuledEventBatch, EngineError> {
        let (stack, completion) = match &pending.continuation {
            ResolutionContinuation::CounterReplacement { stack, completion } => {
                (stack.clone(), completion.clone())
            }
            _ => {
                self.state.pending_resolution = Some(pending);
                return Err(EngineError::Illegal(
                    "counter-replacement continuation missing",
                ));
            }
        };
        let Some(pending_event) = self.state.pending_replacement_event.take() else {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal("counter-replacement choice is stale"));
        };
        let mut batch = match pending_event {
            PendingReplacementEvent::Counters(batch) => *batch,
            other => {
                self.state.pending_replacement_event = Some(other);
                self.state.pending_resolution = Some(pending);
                return Err(EngineError::Illegal("counter-replacement choice is stale"));
            }
        };
        if let Err(error) = self.validate_counter_placement_completion_scope(
            &batch.events,
            &completion,
            stack.as_ref(),
        ) {
            self.state.pending_replacement_event =
                Some(PendingReplacementEvent::Counters(Box::new(batch)));
            self.state.pending_resolution = Some(pending);
            return Err(error);
        }
        if let crate::state::CounterPlacementCompletion::CounterMove {
            source_target,
            destination_target,
            kind,
            ..
        } = &completion
        {
            let Some(parked) = stack.as_ref() else {
                self.state.pending_replacement_event =
                    Some(PendingReplacementEvent::Counters(Box::new(batch)));
                self.state.pending_resolution = Some(pending);
                return Err(EngineError::Illegal(
                    "counter-move continuation is missing its resolving stack item",
                ));
            };
            let current = super::super::targeting::counter_move_target_pair_is_current(
                self,
                &parked.item,
                source_target,
                destination_target,
            ) && self
                .state
                .objects
                .get(&source_target.object_id)
                .is_some_and(|source| source.counter_count(*kind) > 0)
                && self.counter_move_can_complete(
                    source_target.object_id,
                    destination_target.object_id,
                    *kind,
                );
            if !current {
                return self.complete_parked_resolution_with_previous(
                    parked.item.clone(),
                    parked.resume_effect_index(),
                    parked.previous_result().clone(),
                    Vec::new(),
                );
            }
        }
        let Some(application) = batch
            .applications
            .iter()
            .find(|application| application.choice_id == chosen_application_id)
            .cloned()
        else {
            self.state.pending_replacement_event =
                Some(PendingReplacementEvent::Counters(Box::new(batch)));
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal(
                "counter-replacement application is stale",
            ));
        };
        let instance = match single_row_battlefield_instance(
            &application.replacement,
            application.event_index,
        ) {
            Ok(instance) => instance.clone(),
            Err(error) => {
                self.state.pending_replacement_event =
                    Some(PendingReplacementEvent::Counters(Box::new(batch)));
                self.state.pending_resolution = Some(pending);
                return Err(error);
            }
        };
        let event_index = batch.event_index;
        let candidates = single_row_counter_placement_candidates(
            event_index,
            self.counter_replacement_candidates(&batch.events[event_index]),
        );
        if application.event_index != event_index
            || !candidates
                .iter()
                .any(|(candidate, _)| *candidate == application.replacement)
        {
            self.state.pending_replacement_event =
                Some(PendingReplacementEvent::Counters(Box::new(batch)));
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal(
                "counter-replacement effect is no longer applicable",
            ));
        }
        Self::apply_counter_replacement(&mut batch.events[event_index], &instance);
        batch.applications.clear();
        if let Err(error) = self.validate_counter_placement_completion_scope(
            &batch.events,
            &completion,
            stack.as_ref(),
        ) {
            self.state.pending_replacement_event =
                Some(PendingReplacementEvent::Counters(Box::new(batch)));
            self.state.pending_resolution = Some(pending);
            return Err(error);
        }

        let mut events = Vec::new();
        if let Some(candidates) = self.advance_counter_placement_batch(&mut batch)? {
            self.park_counter_placement_choice(batch, candidates, stack, completion, &mut events);
            return Ok(finish_with_events(self, events));
        }
        if let crate::state::CounterPlacementCompletion::ActivationCost { replay } = &completion {
            return match self.finish_activation_cost_replacements(replay, batch.events) {
                Ok(result) => Ok(result),
                Err(error) => {
                    self.state.pending_replacement_event = None;
                    self.pending_counter_cost_internal = None;
                    if replay.command_kind == crate::state::ActivationCostCommandKind::Staged
                        && self.state.pending_ability_activation.as_ref().is_some_and(
                            |activation| {
                                Some(activation.transaction_id) == replay.transaction_id
                                    && Some(activation.revision) == replay.expected_revision
                            },
                        )
                    {
                        self.state.pending_ability_activation = None;
                        self.pending_ability_activation_internal = None;
                    }
                    Err(error)
                }
            };
        }
        if matches!(
            completion,
            crate::state::CounterPlacementCompletion::SagaLore { .. }
        ) {
            if !self.saga_lore_counter_batch_is_current(&batch.events, &completion) {
                return self.finish_unstacked_saga_lore_action(events);
            }
            match self.commit_counter_placement_batch(&batch.events, &completion, None, &mut events)
            {
                Ok(_) => return self.finish_unstacked_saga_lore_action(events),
                Err(error) => {
                    self.state.pending_replacement_event =
                        Some(PendingReplacementEvent::Counters(Box::new(batch)));
                    self.state.pending_resolution = Some(pending);
                    return Err(error);
                }
            }
        }
        if let crate::state::CounterPlacementCompletion::DamageReplacement {
            damage_batch,
            damage_event_index,
            continuation,
        } = &completion
        {
            let parked_batch = batch.clone();
            return match self.finish_prevention_counter_replacement(
                damage_batch.clone(),
                *damage_event_index,
                continuation.clone(),
                batch.events.clone(),
                &mut events,
            ) {
                Ok(result) => Ok(result),
                Err(error) => {
                    self.state.pending_replacement_event =
                        Some(PendingReplacementEvent::Counters(Box::new(parked_batch)));
                    self.state.pending_resolution = Some(pending);
                    Err(error)
                }
            };
        }
        if let crate::state::CounterPlacementCompletion::DamageBatch {
            completed,
            continuation,
        } = &completion
        {
            return match self.commit_counter_placement_batch(
                &batch.events,
                &completion,
                None,
                &mut events,
            ) {
                Ok(receipts) => self.finish_counter_replaced_damage_batch(
                    completed.clone(),
                    continuation.clone(),
                    receipts,
                    &mut events,
                ),
                Err(error) => {
                    self.state.pending_replacement_event =
                        Some(PendingReplacementEvent::Counters(Box::new(batch)));
                    self.state.pending_resolution = Some(pending);
                    Err(error)
                }
            };
        }
        if let crate::state::CounterPlacementCompletion::ProgramChooseBlight {
            choose,
            object_id,
            zone_change_generation,
        } = &completion
        {
            let Some(stack) = stack else {
                self.state.pending_replacement_event =
                    Some(PendingReplacementEvent::Counters(Box::new(batch)));
                self.state.pending_resolution = Some(pending);
                return Err(EngineError::Illegal(
                    "program Choose Blight payment lost its parked stack",
                ));
            };
            if !matches!(
                &choose.stage,
                crate::state::PendingProgramChooseStage::PayingBlight { .. }
            ) {
                self.state.pending_replacement_event =
                    Some(PendingReplacementEvent::Counters(Box::new(batch)));
                self.state.pending_resolution = Some(pending);
                return Err(EngineError::ProgramInvariant(
                    "counter replacement lost the pending Choose Blight occurrence",
                ));
            }
            let payer_live = self
                .state
                .player_idx(choose.original_chooser)
                .is_some_and(|index| !self.state.players[index].has_lost);
            let selected_creature_current = self
                .state
                .zone_change_generation
                .get(object_id)
                .copied()
                .unwrap_or(0)
                == *zone_change_generation
                && self.can_blight_creature(choose.original_chooser, *object_id);
            if !payer_live || !selected_creature_current {
                let log = if !payer_live {
                    "The departed payer cannot complete the program Choose Blight payment; Otherwise resolves."
                } else {
                    "The selected creature cannot receive the program Choose Blight payment; Otherwise resolves."
                };
                return self.complete_program_choose_otherwise_from_parts(
                    stack,
                    choose.clone(),
                    log.into(),
                );
            }
            return match self.finish_program_choose_blight_payment(
                stack,
                choose.clone(),
                *object_id,
                *zone_change_generation,
                batch.events.clone(),
            ) {
                Ok(result) => Ok(result),
                Err(error) => {
                    self.state.pending_replacement_event =
                        Some(PendingReplacementEvent::Counters(Box::new(batch)));
                    self.state.pending_resolution = Some(pending);
                    Err(error)
                }
            };
        }
        if stack.is_none()
            && matches!(
                completion,
                crate::state::CounterPlacementCompletion::BlightEffect { .. }
                    | crate::state::CounterPlacementCompletion::Explore { .. }
            )
        {
            self.state.pending_replacement_event =
                Some(PendingReplacementEvent::Counters(Box::new(batch)));
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal(
                "counter-replacement continuation is missing its resolving stack item",
            ));
        }
        let Some(stack) = stack else {
            self.state.pending_replacement_event =
                Some(PendingReplacementEvent::Counters(Box::new(batch)));
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal(
                "counter-replacement stack continuation is missing",
            ));
        };
        let receipts = match self.commit_counter_placement_batch(
            &batch.events,
            &completion,
            Some(&stack),
            &mut events,
        ) {
            Ok(receipts) => receipts,
            Err(error) => {
                self.state.pending_replacement_event =
                    Some(PendingReplacementEvent::Counters(Box::new(batch)));
                self.state.pending_resolution = Some(pending);
                return Err(error);
            }
        };
        if let crate::state::CounterPlacementCompletion::Proliferate { transaction } = &completion {
            return self.finish_proliferate_counter_leaf(
                stack,
                transaction.clone(),
                receipts,
                events,
            );
        }
        if let crate::state::CounterPlacementCompletion::BlightEffect {
            player,
            count,
            creature,
            spell_label,
        } = &completion
        {
            let receipt = crate::state::BlightReceipt {
                player: *player,
                count: *count,
                creature: Some(*creature),
            };
            let mut stack = stack;
            stack.item.blight_receipts.push(receipt);
            events.push(ev_log(format!(
                "P{player} blights {count} using {} ({spell_label}).",
                self.replacement_object_display_name(creature.object_id, None),
            )));
            self.fire_triggers(&[GameEvent::Blighted(receipt)], &mut events);
            let previous_result = stack.previous_result().clone();
            return if stack.program().is_some() {
                self.complete_parked_stack_with_previous(stack, previous_result, events)
            } else {
                {
                    let (parked_item, parked_resume_effect_index, _, _) = stack.into_parts();
                    self.complete_parked_resolution_with_previous(
                        parked_item,
                        parked_resume_effect_index,
                        previous_result,
                        events,
                    )
                }
            };
        }
        if let crate::state::CounterPlacementCompletion::Explore {
            controller,
            explorer,
            revealed,
            spell_label,
        } = &completion
        {
            let mut previous_result = stack.previous_result().clone();
            previous_result.record_counter_placements(receipts);
            if let Some(revealed) = revealed {
                self.park_explore_choice(
                    stack,
                    *controller,
                    *explorer,
                    *revealed,
                    spell_label,
                    &mut events,
                )?;
                return Ok(finish_with_events(self, events));
            }
            events.push(ev_log(format!(
                "P{controller} explores with an empty library ({spell_label})."
            )));
            self.fire_triggers(&[GameEvent::Explored { object: *explorer }], &mut events);
            return {
                let (parked_item, parked_resume_effect_index, _, _) = stack.into_parts();
                self.complete_parked_resolution_with_previous(
                    parked_item,
                    parked_resume_effect_index,
                    previous_result,
                    events,
                )
            };
        }
        let mut previous_result = stack.previous_result().clone();
        previous_result.record_counter_placements(receipts);
        if let crate::state::CounterPlacementCompletion::Amass {
            controller,
            target_object_id,
            target_zone_change_generation,
            ..
        } = &completion
        {
            previous_result.append(crate::state::ProgramValue::PermanentSelection(vec![
                TriggerObjectRef {
                    object_id: *target_object_id,
                    zone_change_generation: *target_zone_change_generation,
                    controller_at_event: *controller,
                },
            ]))?;
        }
        self.complete_parked_stack_with_previous(stack, previous_result, events)
    }
}
