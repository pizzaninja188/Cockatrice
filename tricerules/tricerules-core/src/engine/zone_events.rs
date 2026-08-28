//! CR 603.2c / 603.10a: an explicit rules-event boundary, independent of command and
//! trigger-flush boundaries. Three Tree Scribe counts departures; Mortipede counts batches.
use super::*;
use tricerules_cards::primitives::{EventZone, ZoneEventCardinality, ZoneEventDestination};

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

pub(super) struct ZoneChangeReceipt {
    pub origin: Zone,
    pub destination: Zone,
    pub before: TurnObjectFact,
    pub destination_generation: u64,
}

pub(super) struct ZoneEventBatch {
    pub sources: Vec<TriggerSourceSnapshot>,
    pub moves: Vec<ZoneChangeReceipt>,
}

impl GameEngine {
    pub(super) fn begin_zone_entry_batch(
        &mut self,
        item: StackItem,
        mut entries: Vec<BattlefieldEntryEvent>,
        spell_label: &str,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<bool, EngineError> {
        entries.sort_by_key(|entry| self.state.apnap_rank(entry.deciding_player));
        let generations = entries
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
        self.continue_zone_entry_batch(
            item,
            crate::state::PendingZoneEntryBatch {
                ready: vec![],
                remaining: entries,
                generations,
                spell_label: spell_label.into(),
            },
            events,
        )
    }

    pub(super) fn zone_entry_batch_current(
        &self,
        batch: &crate::state::PendingZoneEntryBatch,
    ) -> bool {
        batch.generations.iter().all(|(oid, generation)| {
            self.state
                .objects
                .get(oid)
                .is_some_and(|object| object.zone == Zone::Graveyard)
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
        item: StackItem,
        mut batch: crate::state::PendingZoneEntryBatch,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<bool, EngineError> {
        if !self.zone_entry_batch_current(&batch) {
            return Err(EngineError::Illegal("graveyard entry cohort became stale"));
        }
        while !batch.remaining.is_empty() {
            let entry = batch.remaining.remove(0);
            match self.begin_battlefield_entry(
                item.clone(),
                entry,
                BattlefieldEntryCompletion::ZoneEntryBatch(Box::new(batch.clone())),
                events,
            ) {
                replacement::BattlefieldEntryProgress::Parked => return Ok(true),
                replacement::BattlefieldEntryProgress::Ready(entry) => batch.ready.push(entry),
            }
        }
        let snapshot = self.snapshot_zone_event();
        let mut triggers = Vec::new();
        for entry in batch.ready {
            let oid = entry.object_id;
            let owner = self.state.objects[&oid].owner;
            let label = events::object_display_name(&self.state, self.registry, oid);
            triggers.extend(
                self.commit_battlefield_entry_state(entry, None)?
                    .into_iter()
                    .filter(|event| !matches!(event, GameEvent::ZoneChanges(_))),
            );
            triggers.push(GameEvent::EntersBattlefield { object_id: oid });
            events.push(permanent_moved_event(
                &self.state,
                oid,
                owner,
                rv1::permanent_moved::Destination::Battlefield,
            ));
            events.push(events::ev_log(format!(
                "{} returns {label} from graveyard to battlefield.",
                batch.spell_label
            )));
        }
        self.fire_zone_triggers(snapshot, triggers);
        Ok(false)
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
                    Zone::Graveyard if !object.is_token() => {
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
                                    .contains(&CharacteristicDefiningAbility::Changeling)
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
    };
    match filter {
        ZoneEventDestination::Any => true,
        ZoneEventDestination::OneOf(zones) => zones.contains(&zone),
        ZoneEventDestination::Except(zones) => !zones.contains(&zone),
    }
}
