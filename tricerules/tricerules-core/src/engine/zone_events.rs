//! CR 603.2c / 603.10a: an explicit rules-event boundary, independent of command and
//! trigger-flush boundaries. Three Tree Scribe counts departures; Mortipede counts batches.
use super::*;
use tricerules_card_model::primitives::{
    EventZone, LibraryLookRemainderPolicy, ZoneEventCardinality, ZoneEventDestination,
};

pub(super) struct CompletedZoneEntryBatch {
    pub(super) stack: ParkedStackResolution,
    /// Captured before the batch consumes its completion metadata. Typed operation results are
    /// resumed through the parked Program cursor; legacy lists retain their flat effect cursor.
    pub(super) program_operation_result: Option<EffectResult>,
}

#[derive(Clone)]
pub(super) struct ZoneEventSnapshot {
    sources: Vec<TriggerSourceSnapshot>,
    objects: Vec<(Zone, TurnObjectFact)>,
    emblems: Vec<StaticEmblemInstance>,
}

impl ZoneEventSnapshot {
    pub(super) fn source(&self, oid: ObjectId) -> Option<TriggerSourceSnapshot> {
        self.sources
            .iter()
            .find(|source| source.object_id == oid)
            .cloned()
    }
}

#[derive(serde::Serialize, Debug, Clone)]
pub(super) struct ZoneChangeReceipt {
    pub origin: Zone,
    pub destination: Zone,
    pub before: TurnObjectFact,
    pub destination_generation: u64,
}

/// Exact committed movement receipt for a prevalidated operation. Unlike the trigger batch this
/// includes private-zone origins (such as Library), which are intentionally absent from public
/// zone-observer snapshots.
#[derive(Debug, Clone)]
pub(super) struct CommittedZoneMoveReceipt {
    pub object_id: ObjectId,
    pub owner: PlayerId,
    pub origin: Zone,
    pub destination: Zone,
    pub before_zone_change_generation: u64,
    pub destination_generation: u64,
}

#[derive(serde::Serialize, Debug, Clone)]
pub(super) struct ZoneEventBatch {
    pub sources: Vec<TriggerSourceSnapshot>,
    pub moves: Vec<ZoneChangeReceipt>,
    pub emblems: Vec<StaticEmblemInstance>,
}

impl GameEngine {
    /// Identify a typed, resultless library-search entry boundary from both the in-flight program
    /// operation and the exact Library-origin completion transaction. This deliberately checks
    /// the transaction rather than reimplementing any card/model admission policy.
    pub(super) fn is_typed_resultless_library_search_entry(
        &self,
        stack: &ParkedStackResolution,
        completion: &BattlefieldEntryCompletion,
    ) -> bool {
        let BattlefieldEntryCompletion::ZoneEntryBatch(batch) = completion else {
            return false;
        };
        self.is_typed_resultless_library_search_completion(
            stack,
            batch.origin,
            batch.completion.as_ref(),
        )
    }

    fn is_typed_resultless_library_search_completion(
        &self,
        stack: &ParkedStackResolution,
        origin: Zone,
        completion: Option<&crate::state::ZoneEntryCompletion>,
    ) -> bool {
        let Some(program) = stack.program.as_ref() else {
            return false;
        };
        if !program.in_flight.as_ref().is_some_and(Vec::is_empty)
            || !self
                .current_program_perform(program)
                .is_ok_and(|effect| matches!(effect, SpellEffectKind::SearchLibrary { .. }))
        {
            return false;
        }
        let Some(crate::state::ZoneEntryCompletion::LibrarySearch(search)) = completion else {
            return false;
        };
        origin == Zone::Library && search.searcher == stack.item.controller
    }

    /// The admitted singleton graveyard return is resultless, but may suspend during entry.
    /// Keep the Program cursor until the whole replacement and entry transaction finishes.
    fn is_typed_resultless_graveyard_return_completion(
        &self,
        stack: &ParkedStackResolution,
        origin: Zone,
        completion: Option<&crate::state::ZoneEntryCompletion>,
    ) -> bool {
        let Some(program) = stack.program.as_ref() else {
            return false;
        };
        origin == Zone::Graveyard
            && completion.is_none()
            && program.in_flight.as_ref().is_some_and(Vec::is_empty)
            && self.current_program_perform(program).is_ok_and(|effect| {
                matches!(
                    effect,
                    SpellEffectKind::MoveGraveyardCards {
                        destination:
                            tricerules_card_model::primitives::GraveyardDestination::Battlefield {
                                tapped: false
                            },
                        ..
                    }
                )
            })
    }

    pub(super) fn is_typed_resultful_library_search(&self, stack: &ParkedStackResolution) -> bool {
        let Some(program) = stack.program.as_ref() else {
            return false;
        };
        let Some(outputs) = program
            .in_flight
            .as_ref()
            .filter(|outputs| outputs.len() == 1)
        else {
            return false;
        };
        let Ok((instruction, effect)) = self.current_compiled_perform_instruction(program) else {
            return false;
        };
        let Some(expected_result_type) = effect.typed_resultful_battlefield_search_result_type()
        else {
            return false;
        };
        let [output] = instruction.outputs.as_slice() else {
            return false;
        };
        if outputs.as_slice() != [*output] {
            return false;
        }
        let Some(compiled) = self.registry.compiled_program_for_owner(&program.owner) else {
            return false;
        };
        let Some(slot) = compiled.slot(*output) else {
            return false;
        };
        slot.result_type == expected_result_type
    }

    pub(super) fn is_typed_resultful_library_search_entry(
        &self,
        stack: &ParkedStackResolution,
        origin: Zone,
        completion: Option<&crate::state::ZoneEntryCompletion>,
    ) -> bool {
        let Some(crate::state::ZoneEntryCompletion::LibrarySearch(search)) = completion else {
            return false;
        };
        origin == Zone::Library
            && search.searcher == stack.item.controller
            && self.is_typed_resultful_library_search(stack)
    }

    /// Complete the exact ZoneEntryBatch boundary, preserving the typed resultless
    /// Battlefield-search cursor when its validated SearchLibrary operation is in flight.
    /// Other entry batches keep their existing legacy fallback semantics.
    pub(super) fn complete_parked_zone_entry_batch(
        &mut self,
        completion: CompletedZoneEntryBatch,
        events: Vec<rv1::RuledEvent>,
    ) -> Result<RuledEventBatch, EngineError> {
        let CompletedZoneEntryBatch {
            stack,
            program_operation_result,
        } = completion;
        if let Some(operation_result) = program_operation_result {
            return self.complete_parked_stack_with_previous(stack, operation_result, events);
        }

        self.complete_parked_resolution_with_previous(
            stack.item,
            stack.resume_effect_index,
            stack.previous_result,
            events,
        )
    }

    pub(super) fn begin_zone_entry_batch(
        &mut self,
        stack: ParkedStackResolution,
        entries: Vec<BattlefieldEntryEvent>,
        origin: Zone,
        spell_label: &str,
        completion: Option<crate::state::ZoneEntryCompletion>,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<Option<ParkedStackResolution>, EngineError> {
        Ok(self
            .begin_zone_entry_batch_with_resume(
                stack,
                entries,
                origin,
                spell_label,
                completion,
                events,
            )?
            .map(|completed| completed.stack))
    }

    pub(super) fn begin_zone_entry_batch_with_resume(
        &mut self,
        stack: ParkedStackResolution,
        mut entries: Vec<BattlefieldEntryEvent>,
        origin: Zone,
        spell_label: &str,
        completion: Option<crate::state::ZoneEntryCompletion>,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<Option<CompletedZoneEntryBatch>, EngineError> {
        entries.sort_by_key(|entry| self.state.apnap_rank(entry.deciding_player));
        let generations: Vec<_> = entries
            .iter()
            .map(|entry| {
                (
                    entry.object_id,
                    self.state
                        .zone_change_generation
                        .get(&entry.object_id)
                        .copied()
                        .unwrap_or(0),
                )
            })
            .collect();
        let origin_mana_values = generations
            .iter()
            .filter_map(|(oid, generation)| {
                characteristics::characteristics_from(&self.state, self.registry, *oid)
                    .map(|characteristics| (*oid, *generation, characteristics.mana_value))
            })
            .collect();
        self.continue_zone_entry_batch_with_resume(
            stack,
            crate::state::PendingZoneEntryBatch {
                ready: vec![],
                remaining: entries,
                generations,
                origin_mana_values,
                origin,
                spell_label: spell_label.into(),
                completion,
            },
            events,
        )
    }

    pub(super) fn zone_entry_batch_current(
        &self,
        batch: &crate::state::PendingZoneEntryBatch,
    ) -> bool {
        if let Some(crate::state::ZoneEntryCompletion::PairedArtifactExchange {
            departure,
            incoming,
            player,
        }) = &batch.completion
        {
            if targeting::artifact_exchange_player(self, departure.object_id, incoming.object_id)
                != Some(*player)
                || [departure, incoming].iter().any(|reference| {
                    self.state
                        .zone_change_generation
                        .get(&reference.object_id)
                        .copied()
                        .unwrap_or(0)
                        != reference.zone_change_generation
                })
            {
                return false;
            }
        }
        if let Some(crate::state::ZoneEntryCompletion::ChaosWarpRevealedTop {
            library_owner,
            object_id,
            generation,
        }) = &batch.completion
        {
            if self
                .state
                .player_idx(*library_owner)
                .is_none_or(|idx| self.state.players[idx].has_lost)
                || self.state.library_card_objects(*library_owner).next() != Some(*object_id)
                || self.state.objects.get(object_id).is_none_or(|object| {
                    object.zone != Zone::Library || object.owner != *library_owner
                })
                || self
                    .state
                    .zone_change_generation
                    .get(object_id)
                    .copied()
                    .unwrap_or(0)
                    != *generation
            {
                return false;
            }
        }
        if let Some(crate::state::ZoneEntryCompletion::LibraryLookRemainder {
            library_owner,
            looked_refs,
            ..
        }) = &batch.completion
        {
            let Some(idx) = self.state.player_idx(*library_owner) else {
                return false;
            };
            if !looked_refs
                .iter()
                .map(|(oid, _)| *oid)
                .eq(self.state.players[idx]
                    .library
                    .iter()
                    .take(looked_refs.len())
                    .copied())
                || looked_refs.iter().any(|(oid, generation)| {
                    self.state.objects.get(oid).is_none_or(|object| {
                        object.zone != Zone::Library || object.owner != *library_owner
                    }) || self
                        .state
                        .zone_change_generation
                        .get(oid)
                        .copied()
                        .unwrap_or(0)
                        != *generation
                })
            {
                return false;
            }
        }
        if let Some(crate::state::ZoneEntryCompletion::RevealUntilArtifact {
            library_owner,
            found_ref,
            revealed_refs,
            ..
        }) = &batch.completion
        {
            let Some(idx) = self.state.player_idx(*library_owner) else {
                return false;
            };
            if self.state.players[idx].has_lost
                || revealed_refs.last() != Some(found_ref)
                || !revealed_refs
                    .iter()
                    .map(|(object_id, _)| *object_id)
                    .eq(self.state.players[idx]
                        .library
                        .iter()
                        .take(revealed_refs.len())
                        .copied())
                || revealed_refs.iter().any(|(object_id, generation)| {
                    self.state.objects.get(object_id).is_none_or(|object| {
                        object.zone != Zone::Library || object.owner != *library_owner
                    }) || self
                        .state
                        .zone_change_generation
                        .get(object_id)
                        .copied()
                        .unwrap_or(0)
                        != *generation
                })
            {
                return false;
            }
        }
        batch.generations.iter().all(|(oid, generation)| {
            self.state
                .objects
                .get(oid)
                .is_some_and(|object| object.zone == batch.origin)
                && self
                    .state
                    .zone_change_generation
                    .get(oid)
                    .copied()
                    .unwrap_or(0)
                    == *generation
        })
    }

    pub(super) fn continue_zone_entry_batch_with_resume(
        &mut self,
        stack: ParkedStackResolution,
        mut batch: crate::state::PendingZoneEntryBatch,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<Option<CompletedZoneEntryBatch>, EngineError> {
        if !self.zone_entry_batch_current(&batch) {
            return Err(EngineError::Illegal("zone entry cohort became stale"));
        }
        while !batch.remaining.is_empty() {
            let entry = batch.remaining.remove(0);
            match self.begin_battlefield_entry(
                stack.item.clone(),
                entry,
                BattlefieldEntryCompletion::ZoneEntryBatch(Box::new(batch.clone())),
                events,
            )? {
                replacement::BattlefieldEntryProgress::Parked => {
                    self.transfer_entry_choice_resume(&stack);
                    return Ok(None);
                }
                replacement::BattlefieldEntryProgress::Ready(entry) => batch.ready.push(*entry),
                replacement::BattlefieldEntryProgress::Skipped(entry) => {
                    self.restore_skipped_battlefield_entry(&entry)?;
                }
            }
        }
        let Some(SimultaneousEntryBatch::Zone(batch)) = self.begin_entry_timestamp_order(
            SimultaneousEntryBatch::Zone(batch),
            Some(stack.clone()),
            events,
        )?
        else {
            return Ok(None);
        };
        let Some(batch) = self.begin_entry_counter_placements_for_batch(
            SimultaneousEntryBatch::Zone(batch),
            Some(stack.clone()),
            events,
        )?
        else {
            return Ok(None);
        };
        let SimultaneousEntryBatch::Zone(batch) = batch else {
            return Err(EngineError::Illegal("zone entry batch changed type"));
        };
        self.commit_zone_entry_batch_ready_with_resume(stack, batch, events)
            .map(Some)
    }

    pub(super) fn commit_zone_entry_batch_ready_with_resume(
        &mut self,
        stack: ParkedStackResolution,
        mut batch: crate::state::PendingZoneEntryBatch,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<CompletedZoneEntryBatch, EngineError> {
        // A looked remainder is frozen too: reject before committing even the first entrant.
        if !self.zone_entry_batch_current(&batch) {
            return Err(EngineError::Illegal("zone entry cohort became stale"));
        }
        if batch.ready.iter().any(|entry| {
            !entry.entry_counter_placements.is_empty() && !entry.entry_counter_replacements_resolved
        }) {
            return Err(EngineError::Illegal(
                "zone-entry counters were not processed by the replacement planner",
            ));
        }
        // Capture typed Program outputs before the entry transaction consumes its completion:
        // resultless searches resume with an empty operation result, while Audacious Reshapers
        // carries the exact prefix count produced before entry and bottoming.
        let typed_resultful_library_search = self.is_typed_resultful_library_search_entry(
            &stack,
            batch.origin,
            batch.completion.as_ref(),
        );
        let selected_search_object_ids = if typed_resultful_library_search {
            match batch.completion.as_ref() {
                Some(crate::state::ZoneEntryCompletion::LibrarySearch(search)) => {
                    search.selected_object_ids.clone()
                }
                _ => Vec::new(),
            }
        } else {
            Vec::new()
        };
        let mut program_operation_result = if typed_resultful_library_search
            || self.is_typed_resultless_library_search_completion(
                &stack,
                batch.origin,
                batch.completion.as_ref(),
            )
            || self.is_typed_resultless_graveyard_return_completion(
                &stack,
                batch.origin,
                batch.completion.as_ref(),
            ) {
            Some(EffectResult::default())
        } else {
            self.typed_reveal_until_artifact_program_result(
                &stack,
                batch.origin,
                batch.completion.as_ref(),
            )?
        };
        self.prune_invalid_zone_entry_auras(&mut batch)?;
        let completion = batch.completion;
        let mut committed = HashSet::new();
        let mut typed_search_result_objects = Vec::new();
        let snapshot = self.snapshot_zone_event();
        let mut triggers = Vec::new();
        if let Some(crate::state::ZoneEntryCompletion::PairedArtifactExchange {
            departure, ..
        }) = &completion
        {
            // Every fallible entry condition is checked against the original battlefield.
            // Replacement and Aura choices have already finished, with neither member moved.
            for entry in &batch.ready {
                self.validate_battlefield_entry_commit(entry)?;
                if entry.unlock_room_door.is_some() {
                    return Err(EngineError::Illegal(
                        "artifact exchange cannot carry a Room unlock",
                    ));
                }
            }
            let source = snapshot
                .source(departure.object_id)
                .ok_or(EngineError::Illegal("exchange departure source missing"))?;
            let was_creature = self
                .characteristics(departure.object_id)
                .is_some_and(|value| value.has_type("Creature"));
            let owner = self.state.objects[&departure.object_id].owner;
            let died = resolution::sacrifice_permanents(
                &mut self.state,
                self.registry,
                &[departure.object_id],
            )?[0];
            triggers.extend(sacrifice_events(
                source,
                was_creature,
                departure.controller_at_event,
                died,
            ));
            events.push(permanent_moved_event(
                &self.state,
                departure.object_id,
                owner,
                if died {
                    rv1::permanent_moved::Destination::Graveyard
                } else {
                    rv1::permanent_moved::Destination::Exile
                },
            ));
            // CR 400.7: a pre-entry Aura choice of the departing incarnation cannot attach
            // to the card it became. It still enters; ordinary post-resolution SBAs follow.
            for entry in &mut batch.ready {
                if entry.attached_to == Some(AttachmentRecipient::Object(departure.object_id)) {
                    entry.attached_to = None;
                }
            }
        }
        for entry in batch.ready {
            let oid = entry.object_id;
            let chosen_x = entry.chosen_x;
            let destination_controller = entry.destination_controller;
            let owner = self.state.objects[&oid].owner;
            let label = events::object_display_name(&self.state, self.registry, oid);
            triggers.extend(
                self.commit_battlefield_entry_state(entry.clone(), entry.attached_to)?
                    .into_iter()
                    .filter(|event| !matches!(event, GameEvent::ZoneChanges(_))),
            );
            triggers.push(GameEvent::EntersBattlefield {
                object_id: oid,
                chosen_x,
                entry_cause: entry.entry_cause.clone(),
            });
            committed.insert(oid);
            if typed_resultful_library_search && selected_search_object_ids.contains(&oid) {
                typed_search_result_objects.push(TriggerObjectRef {
                    object_id: oid,
                    zone_change_generation: self
                        .state
                        .zone_change_generation
                        .get(&oid)
                        .copied()
                        .unwrap_or(0),
                    controller_at_event: destination_controller,
                });
            }
            events.push(permanent_moved_event(
                &self.state,
                oid,
                owner,
                rv1::permanent_moved::Destination::Battlefield,
            ));
            if completion.is_some() {
                events.push(events::ev_log(format!(
                    "P{} puts {label} onto the battlefield.",
                    entry.destination_controller
                )));
            } else {
                events.push(events::ev_log(format!(
                    "{} returns {label} from {} to battlefield.",
                    batch.spell_label,
                    match batch.origin {
                        Zone::Graveyard => "graveyard",
                        Zone::Exile => "exile",
                        _ => "another zone",
                    }
                )));
            }
        }
        if typed_resultful_library_search {
            let Some(result) = program_operation_result.as_mut() else {
                return Err(EngineError::ProgramInvariant(
                    "typed direct search result lost its zone-entry completion",
                ));
            };
            result.produced_objects = typed_search_result_objects;
        }
        self.fire_zone_triggers(snapshot, triggers, events);
        if let Some(crate::state::ZoneEntryCompletion::LibrarySearch(completion)) = completion {
            if completion.shuffle {
                crate::engine::shuffle_player_library_for_current_command(
                    &mut self.state,
                    completion.searcher,
                );
                events.push(events::ev_log(format!(
                    "P{} shuffles their library.",
                    completion.searcher
                )));
            }
            if completion.searched_library {
                self.fire_triggers(
                    &[GameEvent::LibrarySearched {
                        searcher: completion.searcher,
                        library_owner: completion.searcher,
                    }],
                    events,
                );
            }
        } else if let Some(crate::state::ZoneEntryCompletion::LibraryLookRemainder {
            library_owner,
            looked_refs,
            policy,
        }) = completion
        {
            match policy {
                LibraryLookRemainderPolicy::KeepPosition => {}
                LibraryLookRemainderPolicy::RandomBottom => {
                    let idx = self
                        .state
                        .player_idx(library_owner)
                        .ok_or(EngineError::Illegal("looking player missing"))?;
                    // Only known committed entry moves leave the cohort. Skipped entrants stay in rest.
                    let mut remaining: Vec<ObjectId> = looked_refs
                        .into_iter()
                        .filter_map(|(oid, _)| (!committed.contains(&oid)).then_some(oid))
                        .collect();
                    shuffle_object_ids_for_current_command(
                        &self.state,
                        library_owner,
                        &mut remaining,
                    );
                    self.state.players[idx]
                        .library
                        .retain(|oid| !remaining.contains(oid));
                    self.state.players[idx]
                        .library
                        .extend(remaining.iter().copied());
                    events.push(events::ev_log(format!(
                        "P{library_owner} puts {} cards on the bottom of their library in a random order.",
                        remaining.len()
                    )));
                }
            }
        } else if let Some(crate::state::ZoneEntryCompletion::RevealUntilArtifact {
            library_owner,
            found_ref,
            revealed_refs,
            ..
        }) = completion
        {
            self.bottom_revealed_library_refs(
                library_owner,
                &revealed_refs,
                Some(found_ref.0),
                events,
            )?;
        }
        Ok(CompletedZoneEntryBatch {
            stack,
            program_operation_result,
        })
    }

    fn typed_reveal_until_artifact_program_result(
        &self,
        stack: &ParkedStackResolution,
        origin: Zone,
        completion: Option<&crate::state::ZoneEntryCompletion>,
    ) -> Result<Option<EffectResult>, EngineError> {
        let Some(crate::state::ZoneEntryCompletion::RevealUntilArtifact {
            library_owner,
            program_result,
            ..
        }) = completion
        else {
            return Ok(None);
        };
        let Some(program) = stack.program.as_ref() else {
            return Ok(None);
        };
        let effect = self.current_program_perform(program)?;
        if !matches!(effect, SpellEffectKind::RevealUntilArtifact) {
            return Ok(None);
        }
        if origin != Zone::Library
            || program
                .in_flight
                .as_ref()
                .is_none_or(|outputs| outputs.len() != 1)
        {
            return Err(EngineError::ProgramInvariant(
                "Audacious Reshapers entry lost its typed operation boundary",
            ));
        }
        let program_result = program_result
            .as_deref()
            .ok_or(EngineError::ProgramInvariant(
                "Audacious Reshapers entry lost its exact Program result",
            ))?;
        if *library_owner != stack.item.controller
            || !matches!(
                program_result.program_value.as_ref(),
                Some(crate::state::ProgramValue::Count(_))
            )
        {
            return Err(EngineError::ProgramInvariant(
                "Audacious Reshapers entry retained an invalid scalar result",
            ));
        }
        Ok(Some(program_result.clone()))
    }

    /// Randomize the still-current revealed objects to the bottom and renew their object
    /// generations because CR 701.20d makes reordered library cards new objects.
    pub(super) fn bottom_revealed_library_refs(
        &mut self,
        library_owner: PlayerId,
        revealed_refs: &[(ObjectId, u64)],
        excluded: Option<ObjectId>,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<(), EngineError> {
        let idx = self
            .state
            .player_idx(library_owner)
            .ok_or(EngineError::UnknownPlayer(library_owner))?;
        let in_library: HashSet<_> = self.state.players[idx].library.iter().copied().collect();
        let mut bottom: Vec<ObjectId> = revealed_refs
            .iter()
            .filter_map(|(object_id, generation)| {
                if Some(*object_id) == excluded || !in_library.contains(object_id) {
                    return None;
                }
                let is_current = self.state.objects.get(object_id).is_some_and(|object| {
                    object.zone == Zone::Library
                        && object.owner == library_owner
                        && self
                            .state
                            .zone_change_generation
                            .get(object_id)
                            .copied()
                            .unwrap_or(0)
                            == *generation
                });
                is_current.then_some(*object_id)
            })
            .collect();
        if bottom.is_empty() {
            return Ok(());
        }
        shuffle_object_ids_for_current_command(&self.state, library_owner, &mut bottom);
        self.state.players[idx]
            .library
            .retain(|object_id| !bottom.contains(object_id));
        self.state.players[idx]
            .library
            .extend(bottom.iter().copied());
        for object_id in &bottom {
            *self
                .state
                .zone_change_generation
                .entry(*object_id)
                .or_insert(0) += 1;
        }
        events.push(events::ev_log(format!(
            "P{library_owner} puts {} cards on the bottom of their library in a random order.",
            bottom.len()
        )));
        Ok(())
    }

    pub(super) fn commit_observed_zone_move(
        &mut self,
        oid: ObjectId,
        destination: Zone,
        controller: Option<PlayerId>,
        out: &mut Vec<rv1::RuledEvent>,
    ) -> Result<(), EngineError> {
        let snapshot = self.snapshot_zone_event();
        let leave_event = if destination != Zone::Battlefield
            && self
                .state
                .objects
                .get(&oid)
                .is_some_and(|object| object.zone == Zone::Battlefield)
        {
            snapshot
                .sources
                .iter()
                .find(|source| source.object_id == oid)
                .cloned()
                .map(|source| GameEvent::LeavesBattlefield { source })
        } else {
            None
        };
        move_object_to_zone(&mut self.state, self.registry, oid, destination, controller)?;
        self.fire_zone_triggers(snapshot, leave_event.into_iter().collect(), out);
        Ok(())
    }

    /// Commit a prevalidated simultaneous set of non-entry zone changes as one observed event.
    /// The returned receipts describe the actual generations and destinations after the whole
    /// set has committed; callers publish move events only after receiving this complete batch.
    pub(super) fn commit_observed_zone_move_batch(
        &mut self,
        moves: &[(ObjectId, Zone, Option<PlayerId>)],
        out: &mut Vec<rv1::RuledEvent>,
    ) -> Result<Vec<CommittedZoneMoveReceipt>, EngineError> {
        if moves.is_empty() {
            return Err(EngineError::Illegal(
                "an observed zone batch must contain a move",
            ));
        }
        let mut seen = HashSet::new();
        let mut prepared = Vec::with_capacity(moves.len());
        let mut before = Vec::with_capacity(moves.len());
        for &(oid, destination, controller) in moves {
            if !seen.insert(oid) {
                return Err(EngineError::Illegal(
                    "an observed zone batch cannot move one object twice",
                ));
            }
            let object = self
                .state
                .objects
                .get(&oid)
                .ok_or(EngineError::Illegal("no object"))?;
            before.push((
                oid,
                object.owner,
                object.zone,
                self.state
                    .zone_change_generation
                    .get(&oid)
                    .copied()
                    .unwrap_or(0),
            ));
            let movement = super::resolution::prepare_zone_move(
                &self.state,
                self.registry,
                oid,
                destination,
                controller,
            )?
            .ok_or(EngineError::Illegal(
                "an observed zone batch contains an unmovable object",
            ))?;
            prepared.push(movement);
        }

        // All choices have been validated before mutation, and every member gets the same
        // event-time observer snapshot. Commit in the requested order so graveyard append order
        // follows the controller's CR 404.3 choice, then collect triggers once for the whole set.
        let snapshot = self.snapshot_zone_event();
        let leave_events = moves
            .iter()
            .filter_map(|(oid, destination, _)| {
                (*destination != Zone::Battlefield
                    && self
                        .state
                        .objects
                        .get(oid)
                        .is_some_and(|object| object.zone == Zone::Battlefield))
                .then(|| {
                    snapshot
                        .source(*oid)
                        .map(|source| GameEvent::LeavesBattlefield { source })
                })
                .flatten()
            })
            .collect();
        for movement in prepared {
            super::resolution::commit_zone_move(&mut self.state, self.registry, movement)?;
        }
        self.fire_zone_triggers(snapshot, leave_events, out);
        before
            .into_iter()
            .map(
                |(object_id, owner, origin, before_zone_change_generation)| {
                    let object =
                        self.state
                            .objects
                            .get(&object_id)
                            .ok_or(EngineError::ProgramInvariant(
                                "a committed zone move lost its physical object",
                            ))?;
                    let destination_generation = self
                        .state
                        .zone_change_generation
                        .get(&object_id)
                        .copied()
                        .unwrap_or(0);
                    if destination_generation != before_zone_change_generation + 1 {
                        return Err(EngineError::ProgramInvariant(
                            "a committed zone move did not advance exactly one generation",
                        ));
                    }
                    Ok(CommittedZoneMoveReceipt {
                        object_id,
                        owner,
                        origin,
                        destination: object.zone,
                        before_zone_change_generation,
                        destination_generation,
                    })
                },
            )
            .collect()
    }

    pub(super) fn finish_single_zone_event(
        &self,
        snapshot: ZoneEventSnapshot,
        oid: ObjectId,
    ) -> GameEvent {
        let GameEvent::ZoneChanges(mut batch) = self.finish_zone_event(snapshot) else {
            unreachable!()
        };
        batch
            .moves
            .retain(|movement| movement.before.object_id == oid);
        GameEvent::ZoneChanges(batch)
    }

    /// Call before the first mutation of ONE simultaneous instruction. No state is consumed,
    /// so abandoned preflight and rejected commands cannot leave observer bookkeeping behind.
    pub(super) fn snapshot_zone_event(&self) -> ZoneEventSnapshot {
        let mut sources = self.battlefield_sources_apnap();
        // CR 603.10a: departure and sacrifice observers check their existence and
        // trigger conditions immediately before the event, including intervening-if clauses.
        for source in &mut sources {
            source.triggered_abilities.retain(|(_, ability, _)| {
                self.intervening_if_holds(
                    source.object_id,
                    source.controller,
                    ability.intervening_if.as_ref(),
                )
            });
            source.event_conditions_checked = true;
        }
        let mut objects: Vec<_> = self
            .state
            .objects
            .values()
            .filter_map(|object| {
                let fact = match object.zone {
                    Zone::Battlefield => self.event_object_fact(object.id)?,
                    Zone::Graveyard if self.state.is_card_object(object.id) => {
                        let definition = self.registry.get(&object.card_id)?;
                        let faces: Vec<_> =
                            if matches!(definition.layout, Layout::Split | Layout::Room) {
                                definition.faces_iter().collect()
                            } else {
                                vec![definition.primary_face()]
                            };
                        TurnObjectFact {
                            object_id: object.id,
                            zone_change_generation: self
                                .state
                                .zone_change_generation
                                .get(&object.id)
                                .copied()
                                .unwrap_or(0),
                            owner: object.owner,
                            controller: object.owner,
                            is_token: false,
                            types: faces
                                .iter()
                                .flat_map(|face| face.types.iter().cloned())
                                .collect(),
                            all_creature_types: faces.iter().any(|face| {
                                face.characteristic_defining_abilities
                                    .iter()
                                    .any(|ability| {
                                        ability.definition
                                            == CharacteristicDefiningAbility::Changeling
                                    })
                            }),
                            keywords: vec![],
                            power: None,
                        }
                    }
                    Zone::Exile if self.state.is_card_object(object.id) => {
                        let definition = self.registry.get(&object.card_id)?;
                        let faces: Vec<_> = if object.face_down {
                            Vec::new()
                        } else if matches!(definition.layout, Layout::Split | Layout::Room) {
                            definition.faces_iter().collect()
                        } else {
                            vec![definition.primary_face()]
                        };
                        TurnObjectFact {
                            object_id: object.id,
                            zone_change_generation: self
                                .state
                                .zone_change_generation
                                .get(&object.id)
                                .copied()
                                .unwrap_or(0),
                            owner: object.owner,
                            controller: object.owner,
                            is_token: false,
                            types: faces
                                .iter()
                                .flat_map(|face| face.types.iter().cloned())
                                .collect(),
                            all_creature_types: faces.iter().any(|face| {
                                face.characteristic_defining_abilities
                                    .iter()
                                    .any(|ability| {
                                        ability.definition
                                            == CharacteristicDefiningAbility::Changeling
                                    })
                            }),
                            keywords: vec![],
                            power: None,
                        }
                    }
                    _ => return None,
                };
                Some((object.zone, fact))
            })
            .collect();
        objects.sort_by_key(|(_, fact)| fact.object_id);
        let emblems = self
            .state
            .static_emblems
            .iter()
            .filter(|emblem| emblem.triggered_ability.is_some())
            .cloned()
            .collect();
        ZoneEventSnapshot {
            sources,
            objects,
            emblems,
        }
    }

    /// Capture the actual destination immediately after commitment, before a later instruction
    /// or SBA can move the object again. Even a same-zone move has a distinct generation.
    pub(super) fn finish_zone_event(&self, snapshot: ZoneEventSnapshot) -> GameEvent {
        let moves = snapshot
            .objects
            .into_iter()
            .filter_map(|(origin, before)| {
                let object = self.state.objects.get(&before.object_id)?;
                let generation = self
                    .state
                    .zone_change_generation
                    .get(&before.object_id)
                    .copied()
                    .unwrap_or(0);
                (generation != before.zone_change_generation).then_some(ZoneChangeReceipt {
                    origin,
                    destination: object.zone,
                    before,
                    destination_generation: generation,
                })
            })
            .collect();
        GameEvent::ZoneChanges(ZoneEventBatch {
            sources: snapshot.sources,
            moves,
            emblems: snapshot.emblems,
        })
    }

    pub(super) fn fire_zone_triggers(
        &mut self,
        snapshot: ZoneEventSnapshot,
        mut events: Vec<GameEvent>,
        out: &mut Vec<rv1::RuledEvent>,
    ) -> ZoneEventBatch {
        let GameEvent::ZoneChanges(batch) = self.finish_zone_event(snapshot) else {
            unreachable!()
        };
        // One collection retains the existing history/delayed-event handling and APNAP group.
        events.push(GameEvent::ZoneChanges(batch.clone()));
        self.fire_triggers(&events, out);
        batch
    }

    pub(super) fn collect_zone_triggers(
        &self,
        batch: &ZoneEventBatch,
    ) -> Vec<triggers::CollectedTrigger> {
        let mut out = Vec::new();
        for source in &batch.sources {
            let mut seen = HashSet::new();
            for movement in &batch.moves {
                let fact = &movement.before;
                debug_assert!(movement.destination_generation > fact.zone_change_generation);
                let mut matching =
                    self.matching_snapshot_abilities(source, |condition| match condition {
                        TriggerCondition::WheneverPermanentLeavesBattlefield {
                            controller,
                            filter,
                            destination,
                            ..
                        } => {
                            movement.origin == Zone::Battlefield
                                && movement.destination != Zone::Battlefield
                                && self.relative_player_matches(
                                    *controller,
                                    fact.controller,
                                    source.controller,
                                )
                                && destination_matches(destination, movement.destination)
                                && self.event_filter_matches(filter, fact, source)
                        }
                        TriggerCondition::WheneverCardsLeaveGraveyard { owner, filter, .. } => {
                            movement.origin == Zone::Graveyard
                                && !fact.is_token
                                && self.relative_player_matches(
                                    *owner,
                                    fact.owner,
                                    source.controller,
                                )
                                && self.event_filter_matches(filter, fact, source)
                        }
                        _ => false,
                    });
                matching.retain(|trigger| {
                    let cardinality = match trigger.ability.trigger {
                        TriggerCondition::WheneverPermanentLeavesBattlefield {
                            cardinality,
                            ..
                        }
                        | TriggerCondition::WheneverCardsLeaveGraveyard { cardinality, .. } => {
                            cardinality
                        }
                        _ => unreachable!(),
                    };
                    cardinality == ZoneEventCardinality::EachObject
                        || seen.insert(trigger.ability_origin.clone())
                });
                for trigger in &mut matching {
                    let each_object = matches!(
                        trigger.ability.trigger,
                        TriggerCondition::WheneverPermanentLeavesBattlefield {
                            cardinality: ZoneEventCardinality::EachObject,
                            ..
                        } | TriggerCondition::WheneverCardsLeaveGraveyard {
                            cardinality: ZoneEventCardinality::EachObject,
                            ..
                        }
                    );
                    trigger.trigger_context.observed_object =
                        each_object.then_some(TriggerObjectRef {
                            object_id: fact.object_id,
                            zone_change_generation: fact.zone_change_generation,
                            controller_at_event: fact.controller,
                        });
                }
                out.extend(matching);
            }
        }
        // Emblems observe the same committed event, but are not permanents and therefore have
        // no source fact for permanent-trigger multipliers. Preserve both event incarnations.
        for emblem in &batch.emblems {
            let Some(captured) = emblem.triggered_ability.as_ref() else {
                continue;
            };
            let TriggerCondition::WheneverPermanentLeavesBattlefield {
                controller,
                filter,
                destination,
                cardinality: ZoneEventCardinality::EachObject,
            } = &captured.ability.trigger
            else {
                continue;
            };
            let Some(face) = self
                .registry
                .get(&captured.card_id)
                .and_then(|card| card.face(captured.face_index))
            else {
                continue;
            };
            for movement in &batch.moves {
                let fact = &movement.before;
                if movement.origin != Zone::Battlefield
                    || movement.destination == Zone::Battlefield
                    || !self.relative_player_matches(
                        *controller,
                        fact.controller,
                        emblem.controller,
                    )
                    || !destination_matches(destination, movement.destination)
                    || !super::history::permanent_event_fact_matches(
                        &self.state,
                        filter,
                        fact,
                        ConditionContext {
                            controller: emblem.controller,
                            source_object_id: emblem.object_id,
                            source_zone_change: 0,
                            resolving_spell_id: None,
                            stack_item: None,
                            previous_effect_result: None,
                        },
                    )
                {
                    continue;
                }
                let ability_text = captured.ability.fallback_text(&emblem.display_name);
                let presentation = super::presentation::presentation_ref(
                    self.registry,
                    &captured.card_id,
                    &face.face_id,
                    [
                        super::presentation::PresentationPath::Ability(&captured.parent_ability_id),
                        super::presentation::PresentationPath::Ability(
                            &captured.ability.ability_id,
                        ),
                    ],
                    &captured.ability.presentation,
                    ability_text.clone(),
                );
                out.push(triggers::CollectedTrigger {
                    captured_spell: None,
                    source_id: emblem.object_id,
                    card_id: captured.card_id.clone(),
                    source_label: emblem.display_name.clone(),
                    face_index: captured.face_index,
                    source_zone_change: 0,
                    source_face_change: 0,
                    source_fact: None,
                    additional_instances: 0,
                    controller: emblem.controller,
                    ability_index: 0,
                    ability_origin: None,
                    presentation: Some(presentation),
                    ability: captured.ability.clone(),
                    ability_text,
                    trigger_context: TriggerContext {
                        observed_object: Some(TriggerObjectRef {
                            object_id: fact.object_id,
                            zone_change_generation: fact.zone_change_generation,
                            controller_at_event: fact.controller,
                        }),
                        emblem_graveyard_object: Some(TriggerObjectRef {
                            object_id: fact.object_id,
                            zone_change_generation: movement.destination_generation,
                            controller_at_event: fact.controller,
                        }),
                        ..TriggerContext::default()
                    },
                });
            }
        }
        out
    }
}

fn destination_matches(filter: &ZoneEventDestination, zone: Zone) -> bool {
    let zone = match zone {
        Zone::Battlefield => EventZone::Battlefield,
        Zone::Graveyard => EventZone::Graveyard,
        Zone::Hand => EventZone::Hand,
        Zone::Library => EventZone::Library,
        Zone::Exile => EventZone::Exile,
        Zone::Stack => EventZone::Stack,
        Zone::Command => EventZone::Command,
    };
    match filter {
        ZoneEventDestination::Any => true,
        ZoneEventDestination::OneOf(zones) => zones.contains(&zone),
        ZoneEventDestination::Except(zones) => !zones.contains(&zone),
    }
}

#[cfg(test)]
mod timestamp_order_tests {
    use super::*;

    #[test]
    fn observed_zone_batch_is_one_one_or_more_graveyard_departure_event() {
        use tricerules_card_model::primitives::{
            CastTriggerPlayer, ContinuousEffectKind, EffectDuration, PermanentEventFilter,
            PermanentTypeFilter, ZoneEventCardinality,
        };

        let mut engine = GameEngine::new(
            tricerules_cards::registry::global(),
            2026101511,
            &[0, 1],
            20,
            Some(vec![vec!["island".into(); 20], vec!["forest".into(); 20]]),
            true,
        )
        .unwrap();
        let (observer, first, second) = {
            let hand = &engine.state.players[0].hand;
            (hand[0], hand[1], hand[2])
        };
        for (oid, card_id) in [
            (observer, "grizzly_bears"),
            (first, "grizzly_bears"),
            (second, "hill_giant"),
        ] {
            engine.state.objects.get_mut(&oid).unwrap().card_id = card_id.into();
        }
        move_object_to_zone(
            &mut engine.state,
            engine.registry,
            observer,
            Zone::Battlefield,
            Some(0),
        )
        .unwrap();
        for oid in [first, second] {
            move_object_to_zone(
                &mut engine.state,
                engine.registry,
                oid,
                Zone::Graveyard,
                None,
            )
            .unwrap();
        }
        let mut ability = engine
            .registry
            .get("ajanis_pridemate")
            .unwrap()
            .primary_face()
            .triggered_abilities[0]
            .clone();
        ability.trigger = TriggerCondition::WheneverCardsLeaveGraveyard {
            owner: CastTriggerPlayer::Controller,
            filter: PermanentEventFilter {
                permanent_type: Some(PermanentTypeFilter::Creature),
                ..Default::default()
            },
            cardinality: ZoneEventCardinality::OneOrMore,
        };
        engine.state.add_triggered_ability_grant(ContinuousEffect {
            trigger_grant_origin: None,
            source_id: None,
            affected: AffectedScope::Single(observer),
            kind: ContinuousEffectKind::GrantTriggeredAbility(Box::new(ability)),
            condition: None,
            duration: EffectDuration::UntilEndOfTurn,
            timestamp: engine.state.command_index,
        });
        let library_card = *engine.state.players[0].library.front().unwrap();
        engine.state.objects.get_mut(&library_card).unwrap().card_id = "grizzly_bears".into();
        let generations: HashMap<_, _> = [library_card, first, second]
            .into_iter()
            .map(|oid| {
                (
                    oid,
                    engine
                        .state
                        .zone_change_generation
                        .get(&oid)
                        .copied()
                        .unwrap_or(0),
                )
            })
            .collect();
        let mut events = Vec::new();
        let receipts = engine
            .commit_observed_zone_move_batch(
                &[
                    (library_card, Zone::Graveyard, None),
                    (first, Zone::Hand, None),
                    (second, Zone::Command, None),
                ],
                &mut events,
            )
            .unwrap();

        assert_eq!(receipts.len(), 3);
        for (oid, destination) in [
            (library_card, Zone::Graveyard),
            (first, Zone::Hand),
            (second, Zone::Command),
        ] {
            let receipt = receipts
                .iter()
                .find(|receipt| receipt.object_id == oid)
                .expect("one actual receipt for each planned member");
            assert_eq!(receipt.before_zone_change_generation, generations[&oid]);
            assert_eq!(receipt.destination_generation, generations[&oid] + 1);
            assert_eq!(receipt.destination, destination);
        }
        assert_eq!(engine.state.staged_trigger_groups.len(), 1);
        assert_eq!(engine.state.objects[&first].zone, Zone::Hand);
        assert_eq!(engine.state.objects[&second].zone, Zone::Command);
        assert_eq!(engine.state.objects[&library_card].zone, Zone::Graveyard);
    }

    #[test]
    fn paired_artifact_exchange_commits_both_generations_and_preserves_entry_replacement() {
        let mut engine = GameEngine::new(
            tricerules_cards::registry::global(),
            2026100520,
            &[10, 20, 30],
            20,
            None,
            true,
        )
        .unwrap();
        let ids = engine.state.players[1].hand[..2].to_vec();
        for (&oid, card) in ids.iter().zip(["orb_of_dreams", "sol_ring"]) {
            engine.state.objects.get_mut(&oid).unwrap().card_id = card.into();
        }
        move_object_to_zone(
            &mut engine.state,
            engine.registry,
            ids[0],
            Zone::Battlefield,
            Some(20),
        )
        .unwrap();
        move_object_to_zone(
            &mut engine.state,
            engine.registry,
            ids[1],
            Zone::Graveyard,
            None,
        )
        .unwrap();
        let frozen = |oid| TriggerObjectRef {
            object_id: oid,
            zone_change_generation: engine.state.zone_change_generation[&oid],
            controller_at_event: 20,
        };
        let departure = frozen(ids[0]);
        let incoming = frozen(ids[1]);
        let snapshot = engine.snapshot_zone_event();
        let stack = engine.observer_return_item(ids[0], 10);
        let entry = entry_for(&engine, ids[1], 20);
        let mut events = Vec::new();
        assert!(engine
            .begin_zone_entry_batch(
                ParkedStackResolution::new(stack),
                vec![entry],
                Zone::Graveyard,
                "exchange",
                Some(crate::state::ZoneEntryCompletion::PairedArtifactExchange {
                    departure,
                    incoming,
                    player: 20,
                }),
                &mut events
            )
            .unwrap()
            .is_some());
        assert_eq!(engine.state.objects[&ids[0]].zone, Zone::Graveyard);
        assert_eq!(engine.state.objects[&ids[1]].zone, Zone::Battlefield);
        assert_eq!(engine.state.objects[&ids[1]].controller, 20);
        assert!(
            engine.state.objects[&ids[1]].tapped,
            "the outgoing Orb applies before the simultaneous event"
        );
        assert_eq!(
            engine.state.zone_change_generation[&ids[0]],
            departure.zone_change_generation + 1
        );
        assert_eq!(
            engine.state.zone_change_generation[&ids[1]],
            incoming.zone_change_generation + 1
        );
        let GameEvent::ZoneChanges(receipts) = engine.finish_zone_event(snapshot) else {
            unreachable!()
        };
        assert_eq!(receipts.moves.len(), 2);
        assert!(receipts.moves.iter().any(|r| r.before.object_id == ids[0]
            && r.origin == Zone::Battlefield
            && r.destination == Zone::Graveyard));
        assert!(receipts.moves.iter().any(|r| r.before.object_id == ids[1]
            && r.origin == Zone::Graveyard
            && r.destination == Zone::Battlefield));
    }

    fn entry_for(
        engine: &GameEngine,
        object_id: ObjectId,
        controller: PlayerId,
    ) -> BattlefieldEntryEvent {
        BattlefieldEntryEvent {
            entry_reveal_receipts: Vec::new(),
            mana_colors_spent_to_cast: Default::default(),
            prepared: false,
            object_id,
            deciding_player: controller,
            destination_controller: controller,
            battle_protector: None,
            face_index: 0,
            unlock_room_door: None,
            chosen_x: 0,
            cast_by: None,
            cast_cost_receipts: vec![],
            player_life_snapshot: engine.player_life_snapshot(),
            tapped: false,
            set_types: None,
            chosen_basic_land_type: None,
            chosen_opponents: Vec::new(),
            chosen_creature_types: Vec::new(),
            entry_counters: Default::default(),
            entry_counter_placements: Vec::new(),
            entry_counter_replacements_resolved: false,
            entry_modifiers: vec![],
            entry_cause: None,
            attached_to: None,
            pending_copy_candidate: None,
            pending_aura_recipient: None,
            accepted_aura_recipient: None,
            applied_effects: vec![],
        }
    }

    fn order_choice(chosen_object_ids: Vec<ObjectId>) -> rv1::RuledCommand {
        rv1::RuledCommand {
            cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(
                rv1::SubmitResolutionChoice {
                    chosen_object_ids,
                    ..Default::default()
                },
            )),
        }
    }

    #[test]
    fn simultaneous_zone_entries_ask_controller_to_order_same_controller_permanents() {
        let mut engine = GameEngine::new(
            tricerules_cards::registry::global(),
            613_701,
            &[0, 1],
            20,
            Some(vec![vec!["island".into(); 20], vec!["forest".into(); 20]]),
            true,
        )
        .unwrap();
        let ids: Vec<_> = engine.state.players[0]
            .hand
            .iter()
            .copied()
            .take(2)
            .collect();
        for (oid, card_id) in ids.iter().zip(["folio_of_fancies", "twenty-toed_toad"]) {
            engine.state.objects.get_mut(oid).unwrap().card_id = card_id.into();
            move_object_to_zone(
                &mut engine.state,
                engine.registry,
                *oid,
                Zone::Graveyard,
                None,
            )
            .unwrap();
        }
        let entries = ids.iter().map(|&oid| entry_for(&engine, oid, 0)).collect();
        let stack = engine.observer_return_item(ids[0], 0);
        let mut events = Vec::new();
        assert!(
            engine
                .begin_zone_entry_batch(
                    ParkedStackResolution::new(stack),
                    entries,
                    Zone::Graveyard,
                    "return",
                    None,
                    &mut events
                )
                .unwrap()
                .is_none(),
            "the simultaneous cohort must park for a logged timestamp-order choice"
        );
        engine.publish_resolution_prompt_ids(&mut events).unwrap();
        let pending = engine
            .state
            .pending_resolution
            .as_ref()
            .expect("ordered entry choice");
        assert_eq!(pending.deciding_player, 0);
        assert_eq!(pending.presentation.candidates.len(), 2);
        assert!(pending.presentation.ordered);
        assert!(engine.state.players[0].battlefield.is_empty());

        for choice in [vec![], vec![ids[0], ids[0]], vec![ids[0], 999_999]] {
            assert!(engine
                .apply_scenario_command(0, &order_choice(choice))
                .is_err());
            assert!(engine.state.pending_resolution.is_some());
            assert!(engine.state.players[0].battlefield.is_empty());
        }
        assert!(engine
            .apply_scenario_command(1, &order_choice(vec![ids[1], ids[0]]))
            .is_err());
        assert!(engine.state.pending_resolution.is_some());
        engine
            .apply_scenario_command(0, &order_choice(vec![ids[1], ids[0]]))
            .expect("controller chooses reverse entry timestamp order");
        assert!(engine.state.pending_resolution.is_none());
        assert_eq!(engine.maximum_hand_size(0), usize::MAX);
        assert!(
            engine.state.battlefield_entry_timestamps[&ids[1]]
                < engine.state.battlefield_entry_timestamps[&ids[0]]
        );
    }

    #[test]
    fn simultaneous_entry_order_prompts_each_controller_in_apnap_order() {
        let mut engine = GameEngine::new(
            tricerules_cards::registry::global(),
            613_702,
            &[0, 1],
            20,
            Some(vec![vec!["island".into(); 20], vec!["forest".into(); 20]]),
            true,
        )
        .unwrap();
        let p0: Vec<_> = engine.state.players[0]
            .hand
            .iter()
            .copied()
            .take(2)
            .collect();
        let p1: Vec<_> = engine.state.players[1]
            .hand
            .iter()
            .copied()
            .take(2)
            .collect();
        for &oid in p0.iter().chain(p1.iter()) {
            move_object_to_zone(
                &mut engine.state,
                engine.registry,
                oid,
                Zone::Graveyard,
                None,
            )
            .unwrap();
        }
        let entries = p1
            .iter()
            .map(|&oid| entry_for(&engine, oid, 1))
            .chain(p0.iter().map(|&oid| entry_for(&engine, oid, 0)))
            .collect();
        let stack = engine.observer_return_item(p0[0], 0);
        let mut events = Vec::new();
        assert!(engine
            .begin_zone_entry_batch(
                ParkedStackResolution::new(stack),
                entries,
                Zone::Graveyard,
                "return",
                None,
                &mut events
            )
            .unwrap()
            .is_none());
        engine.publish_resolution_prompt_ids(&mut events).unwrap();
        assert_eq!(
            engine
                .state
                .pending_resolution
                .as_ref()
                .unwrap()
                .deciding_player,
            0
        );
        engine
            .apply_scenario_command(0, &order_choice(vec![p0[1], p0[0]]))
            .expect("active controller chooses order first");
        assert_eq!(
            engine
                .state
                .pending_resolution
                .as_ref()
                .unwrap()
                .deciding_player,
            1
        );
        assert!(engine.state.players[0].battlefield.is_empty());
        assert!(engine.state.players[1].battlefield.is_empty());
        engine
            .apply_scenario_command(1, &order_choice(vec![p1[1], p1[0]]))
            .expect("next controller chooses order second");
        let stamps = &engine.state.battlefield_entry_timestamps;
        assert!(stamps[&p0[1]] < stamps[&p0[0]]);
        assert!(stamps[&p0[0]] < stamps[&p1[1]]);
        assert!(stamps[&p1[1]] < stamps[&p1[0]]);
    }

    #[test]
    fn stale_simultaneous_entry_generation_rejects_without_committing_cohort() {
        let mut engine = GameEngine::new(
            tricerules_cards::registry::global(),
            613_703,
            &[0, 1],
            20,
            Some(vec![vec!["island".into(); 20], vec!["forest".into(); 20]]),
            true,
        )
        .unwrap();
        let ids: Vec<_> = engine.state.players[0]
            .hand
            .iter()
            .copied()
            .take(2)
            .collect();
        for &oid in &ids {
            move_object_to_zone(
                &mut engine.state,
                engine.registry,
                oid,
                Zone::Graveyard,
                None,
            )
            .unwrap();
        }
        let entries = ids.iter().map(|&oid| entry_for(&engine, oid, 0)).collect();
        let stack = engine.observer_return_item(ids[0], 0);
        assert!(engine
            .begin_zone_entry_batch(
                ParkedStackResolution::new(stack),
                entries,
                Zone::Graveyard,
                "return",
                None,
                &mut Vec::new()
            )
            .unwrap()
            .is_none());
        move_object_to_zone(
            &mut engine.state,
            engine.registry,
            ids[0],
            Zone::Exile,
            None,
        )
        .unwrap();
        move_object_to_zone(
            &mut engine.state,
            engine.registry,
            ids[0],
            Zone::Graveyard,
            None,
        )
        .unwrap();
        assert!(engine
            .apply_scenario_command(0, &order_choice(ids.clone()))
            .is_err());
        assert!(engine.state.pending_resolution.is_some());
        assert!(engine.state.players[0].battlefield.is_empty());
    }
}
