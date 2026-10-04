//! CR 603.2c / 603.10a: an explicit rules-event boundary, independent of command and
//! trigger-flush boundaries. Three Tree Scribe counts departures; Mortipede counts batches.
use super::*;
use tricerules_cards::primitives::{EventZone, ZoneEventCardinality, ZoneEventDestination};

#[derive(Clone)]
pub(super) struct ZoneEventSnapshot {
    sources: Vec<TriggerSourceSnapshot>,
    objects: Vec<(Zone, TurnObjectFact)>,
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

#[derive(serde::Serialize, Debug, Clone)]
pub(super) struct ZoneEventBatch {
    pub sources: Vec<TriggerSourceSnapshot>,
    pub moves: Vec<ZoneChangeReceipt>,
}

impl GameEngine {
    pub(super) fn begin_zone_entry_batch(
        &mut self,
        stack: ParkedStackResolution,
        mut entries: Vec<BattlefieldEntryEvent>,
        origin: Zone,
        spell_label: &str,
        completion: Option<crate::state::ZoneEntryCompletion>,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<Option<ParkedStackResolution>, EngineError> {
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
        self.continue_zone_entry_batch(
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
        if let Some(crate::state::ZoneEntryCompletion::DeployRandomBottom {
            library_owner,
            looked_refs,
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

    pub(super) fn continue_zone_entry_batch(
        &mut self,
        stack: ParkedStackResolution,
        mut batch: crate::state::PendingZoneEntryBatch,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<Option<ParkedStackResolution>, EngineError> {
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
            ) {
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
        self.commit_zone_entry_batch_ready(stack, batch, events)
            .map(Some)
    }

    pub(super) fn commit_zone_entry_batch_ready(
        &mut self,
        mut stack: ParkedStackResolution,
        batch: crate::state::PendingZoneEntryBatch,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<ParkedStackResolution, EngineError> {
        // A looked remainder is frozen too: reject before committing even the first entrant.
        if !self.zone_entry_batch_current(&batch) {
            return Err(EngineError::Illegal("zone entry cohort became stale"));
        }
        let mut completion = batch.completion;
        let mut committed = HashSet::new();
        let snapshot = self.snapshot_zone_event();
        let mut triggers = Vec::new();
        for entry in batch.ready {
            let oid = entry.object_id;
            let chosen_x = entry.chosen_x;
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
            });
            committed.insert(oid);
            if let Some(crate::state::ZoneEntryCompletion::LibrarySearch(completion)) =
                completion.as_mut()
            {
                if let Some(result_id) = completion.result_id.take() {
                    stack.item.search_results.insert(
                        result_id,
                        TriggerObjectRef {
                            object_id: oid,
                            zone_change_generation: self
                                .state
                                .zone_change_generation
                                .get(&oid)
                                .copied()
                                .unwrap_or(0),
                            controller_at_event: entry.destination_controller,
                        },
                    );
                }
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
        self.fire_zone_triggers(snapshot, triggers);
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
                self.fire_triggers(&[GameEvent::LibrarySearched {
                    searcher: completion.searcher,
                    library_owner: completion.searcher,
                }]);
            }
        } else if let Some(crate::state::ZoneEntryCompletion::DeployRandomBottom {
            library_owner,
            looked_refs,
        }) = completion
        {
            let idx = self
                .state
                .player_idx(library_owner)
                .ok_or(EngineError::Illegal("looking player missing"))?;
            // Only known committed entry moves leave the cohort. Skipped entrants stay in rest.
            let mut remaining: Vec<ObjectId> = looked_refs
                .into_iter()
                .filter_map(|(oid, _)| (!committed.contains(&oid)).then_some(oid))
                .collect();
            shuffle_object_ids_for_current_command(&self.state, library_owner, &mut remaining);
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
        Ok(stack)
    }

    pub(super) fn commit_observed_zone_move(
        &mut self,
        oid: ObjectId,
        destination: Zone,
        controller: Option<PlayerId>,
    ) -> Result<(), EngineError> {
        let snapshot = self.snapshot_zone_event();
        move_object_to_zone(&mut self.state, self.registry, oid, destination, controller)?;
        self.fire_zone_triggers(snapshot, vec![]);
        Ok(())
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
                    _ => return None,
                };
                Some((object.zone, fact))
            })
            .collect();
        objects.sort_by_key(|(_, fact)| fact.object_id);
        ZoneEventSnapshot { sources, objects }
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
        })
    }

    pub(super) fn fire_zone_triggers(
        &mut self,
        snapshot: ZoneEventSnapshot,
        mut events: Vec<GameEvent>,
    ) {
        let zone = self.finish_zone_event(snapshot);
        // One collection retains the existing history/delayed-event handling and APNAP group.
        events.push(zone);
        self.fire_triggers(&events);
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
            entry_counters: Default::default(),
            entry_modifiers: vec![],
            attached_to: None,
            pending_copy_candidate: None,
            pending_aura_recipient: None,
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
            assert!(engine.apply_command(0, &order_choice(choice)).is_err());
            assert!(engine.state.pending_resolution.is_some());
            assert!(engine.state.players[0].battlefield.is_empty());
        }
        assert!(engine
            .apply_command(1, &order_choice(vec![ids[1], ids[0]]))
            .is_err());
        assert!(engine.state.pending_resolution.is_some());
        engine
            .apply_command(0, &order_choice(vec![ids[1], ids[0]]))
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
            .apply_command(0, &order_choice(vec![p0[1], p0[0]]))
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
            .apply_command(1, &order_choice(vec![p1[1], p1[0]]))
            .expect("next controller chooses order second");
        let stamps = &engine.state.battlefield_entry_timestamps;
        assert!(stamps[&p0[1]] < stamps[&p0[0]]);
        assert!(stamps[&p0[0]] < stamps[&p1[1]]);
        assert!(stamps[&p1[1]] < stamps[&p1[0]]);
    }

    #[test]
    fn stale_simultaneous_entry_generation_rejects_without_committing_cohort() {
        let mut engine = GameEngine::new(
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
        assert!(engine.apply_command(0, &order_choice(ids.clone())).is_err());
        assert!(engine.state.pending_resolution.is_some());
        assert!(engine.state.players[0].battlefield.is_empty());
    }
}
