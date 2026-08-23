//! Shared CR 614/616 replacement ordering and battlefield-entry preprocessing.

use super::characteristics::{apply_face_down_values, creature_matches_scope};
use super::events::{ev_log, finish_with_events};
use super::history::player_life_aggregate_value;
use super::resolution::{
    move_object_to_zone, permanent_moved_event, permanent_moved_event_with_library_position,
};
use super::targeting::{battlefield_objects_matching, object_matches_mass_filter};
use super::*;

fn accumulate_entry_counters(
    counters: &mut BTreeMap<CounterKind, u32>,
    counter: CounterKind,
    count: u32,
) {
    if count == 0 {
        return;
    }
    let total = counters.entry(counter).or_insert(0);
    *total = total.saturating_add(count);
}

/// The event domain currently parked behind the one shared CR 616 choice channel.
#[derive(Debug, Clone)]
pub(crate) enum PendingReplacementEvent {
    Damage(super::damage::PendingDamageBatch),
    BattlefieldEntry(Box<PendingBattlefieldEntry>),
}

pub(super) enum BattlefieldEntryProgress {
    Ready(BattlefieldEntryEvent),
    Parked,
}

impl GameEngine {
    pub(super) fn player_life_snapshot(&self) -> BTreeMap<PlayerId, i32> {
        self.state
            .players
            .iter()
            .map(|player| (player.id, player.life))
            .collect()
    }

    fn entry_condition_holds(
        &self,
        condition: &GameCondition,
        event: &BattlefieldEntryEvent,
    ) -> bool {
        match condition {
            GameCondition::PlayerLifeAggregate {
                players, aggregate, ..
            } => player_life_aggregate_value(
                &self.state,
                *players,
                *aggregate,
                event.destination_controller,
                |player_id| event.player_life_snapshot.get(&player_id).copied(),
            )
            .is_some_and(|value| condition.matches_life_value(value)),
            _ => self.condition_holds(
                condition,
                ConditionContext {
                    controller: event.destination_controller,
                    source_object_id: event.object_id,
                    source_zone_change: self
                        .state
                        .zone_change_generation
                        .get(&event.object_id)
                        .copied()
                        .unwrap_or(0),
                    resolving_spell_id: None,
                },
            ),
        }
    }

    fn battlefield_counter_replacement_affects(
        &self,
        source_id: ObjectId,
        filter: &CreatureScopeFilter,
        event: &BattlefieldEntryEvent,
    ) -> bool {
        let Some(source_controller) = self.controller_of(source_id) else {
            return false;
        };
        let Some(mut characteristics) = self.characteristics_through_layer_5(event.object_id)
        else {
            return false;
        };
        if self
            .state
            .objects
            .get(&event.object_id)
            .is_some_and(|object| object.face_down)
        {
            apply_face_down_values(&mut characteristics);
        }
        characteristics.controller = event.destination_controller;
        creature_matches_scope(
            &self.state,
            self.registry,
            filter,
            source_controller,
            filter.exclude_self.then_some(source_id),
            event.object_id,
            &characteristics,
        )
    }

    fn battlefield_entry_candidates(
        &self,
        event: &BattlefieldEntryEvent,
    ) -> Vec<(EntryReplacementEffectId, ReplacementPriority, String)> {
        let Some(entering) = self.state.objects.get(&event.object_id) else {
            return Vec::new();
        };
        let mut candidates = Vec::new();
        if !entering.face_down {
            if let Some(face) = self.effective_face(event.object_id) {
                for (ability_index, ability) in face.static_abilities.iter().enumerate() {
                    let (priority, label) = match ability {
                        StaticAbilityDef::EntersAsCopy { .. } => (
                            ReplacementPriority::EntryCopy,
                            Some(format!("{} — enters as a copy", face.name)),
                        ),
                        StaticAbilityDef::EntersTapped {
                            affected: EntersTappedAffected::Self_,
                            condition,
                        } if !event.tapped
                            && condition.as_ref().is_none_or(|condition| {
                                self.entry_condition_holds(condition, event)
                            }) =>
                        {
                            (
                                ReplacementPriority::Other,
                                Some(format!("{} — enters tapped", face.name)),
                            )
                        }
                        StaticAbilityDef::EntersWithCounters {
                            affected: EntersWithCountersAffected::Self_,
                            ..
                        } => (
                            ReplacementPriority::Other,
                            Some(format!("{} — enters with counters", face.name)),
                        ),
                        _ => (ReplacementPriority::Other, None),
                    };
                    let Some(label) = label else {
                        continue;
                    };
                    let effect_id = EntryReplacementEffectId::Intrinsic {
                        object_id: event.object_id,
                        copy_revision: entering.copy_revision,
                        ability_index,
                    };
                    if !event.applied_effects.contains(&effect_id) {
                        candidates.push((effect_id, priority, label));
                    }
                }
            }
        }

        let mut battlefield_sources: Vec<_> = self
            .state
            .objects
            .values()
            .filter(|object| object.zone == Zone::Battlefield)
            .collect();
        battlefield_sources.sort_by_key(|object| object.id);
        for source in battlefield_sources {
            if source.face_down {
                continue;
            }
            let Some(face) = self.effective_face(source.id) else {
                continue;
            };
            for (ability_index, ability) in face.static_abilities.iter().enumerate() {
                let label = match ability {
                    StaticAbilityDef::EntersTapped {
                        affected: EntersTappedAffected::Permanents,
                        condition,
                    } if !event.tapped
                        && condition.as_ref().is_none_or(|condition| {
                            self.entry_condition_holds(condition, event)
                        }) =>
                    {
                        Some(format!(
                            "{} (P{}, object {}) — permanents enter tapped",
                            face.name, source.controller, source.id
                        ))
                    }
                    StaticAbilityDef::EntersWithCounters {
                        affected: EntersWithCountersAffected::Creatures(filter),
                        ..
                    } if self.battlefield_counter_replacement_affects(source.id, filter, event) => {
                        Some(format!(
                            "{} (P{}, object {}) — enters with counters",
                            face.name, source.controller, source.id
                        ))
                    }
                    _ => None,
                };
                if let Some(label) = label {
                    let effect_id = EntryReplacementEffectId::Battlefield {
                        source_id: source.id,
                        source_generation: self
                            .state
                            .zone_change_generation
                            .get(&source.id)
                            .copied()
                            .unwrap_or(0),
                        ability_index,
                    };
                    if !event.applied_effects.contains(&effect_id) {
                        candidates.push((effect_id, ReplacementPriority::Other, label));
                    }
                }
            }
        }

        let Some(priority) = candidates.iter().map(|(_, priority, _)| *priority).min() else {
            return candidates;
        };
        candidates
            .into_iter()
            .filter(|(_, candidate_priority, _)| *candidate_priority == priority)
            .collect()
    }

    fn entry_copy_filter(
        &self,
        event: &BattlefieldEntryEvent,
        effect_id: &EntryReplacementEffectId,
    ) -> Option<TargetFilter> {
        let EntryReplacementEffectId::Intrinsic {
            object_id,
            copy_revision,
            ability_index,
        } = effect_id
        else {
            return None;
        };
        if *object_id != event.object_id {
            return None;
        }
        let object = self.state.objects.get(object_id)?;
        if object.copy_revision != *copy_revision {
            return None;
        }
        match self
            .effective_face(*object_id)?
            .static_abilities
            .get(*ability_index)?
        {
            StaticAbilityDef::EntersAsCopy { filter } => Some(filter.clone()),
            _ => None,
        }
    }

    fn copy_source_candidates(
        &self,
        event: &BattlefieldEntryEvent,
        filter: &TargetFilter,
    ) -> Vec<ObjectId> {
        battlefield_objects_matching(self, filter)
            .into_iter()
            .filter(|oid| *oid != event.object_id)
            .collect()
    }

    fn park_copy_source_choice(
        &mut self,
        item: StackItem,
        event: BattlefieldEntryEvent,
        completion: BattlefieldEntryCompletion,
        effect_id: EntryReplacementEffectId,
        candidates: Vec<ObjectId>,
        events: &mut Vec<rv1::RuledEvent>,
    ) {
        let physical_name = self
            .state
            .objects
            .get(&event.object_id)
            .and_then(|object| self.registry.get(&object.card_id))
            .map(|definition| definition.name.clone())
            .unwrap_or_else(|| "this creature".to_string());
        let prompt = format!(
            "Choose a creature for {physical_name} to copy, or Decline to enter as {physical_name}."
        );
        let candidate_card_ids = candidates
            .iter()
            .map(|oid| {
                self.effective_card_identity(*oid)
                    .map(|(card_id, _)| card_id.to_string())
                    .unwrap_or_default()
            })
            .collect();
        let candidate_names = candidates
            .iter()
            .map(|oid| super::events::object_display_name(&self.state, self.registry, *oid))
            .collect();
        events.push(rv1::RuledEvent {
            ev: Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(
                rv1::ResolutionChoiceRequired {
                    deciding_player_id: event.deciding_player,
                    source_object_id: event.object_id,
                    prompt_text: prompt.clone(),
                    choice_kind: rv1::ChoiceKind::CopySource as i32,
                    candidate_object_ids: candidates.clone(),
                    candidate_card_ids,
                    min: 0,
                    max: 1,
                    ordered: false,
                    candidate_names,
                    candidate_server_card_ids: Vec::new(),
                    candidate_selectable: Vec::new(),
                    resolution_branches: Vec::new(),
                    mana_cost: String::new(),
                    unique_names: false,
                    generic_mana_cost: 0,
                    payment_currently_legal: false,
                    reveal_audience: 0,
                    revealed_zone_owner_player_id: None,
                    candidate_source_zones: Vec::new(),
                },
            )),
        });
        events.push(ev_log(prompt.clone()));
        let deciding_player = event.deciding_player;
        self.state.pending_replacement_event = Some(PendingReplacementEvent::BattlefieldEntry(
            Box::new(PendingBattlefieldEntry {
                event,
                applications: Vec::new(),
                copy_source_effect: Some(effect_id),
                completion,
            }),
        ));
        self.state.pending_resolution = Some(PendingResolution {
            deciding_player,
            presentation: PendingResolutionPresentation {
                source_object_id: item.id,
                candidates,
                min: 0,
                max: 1,
                ordered: false,
                prompt,
                choice_kind: rv1::ChoiceKind::CopySource,
                unique_names: false,
            },
            continuation: ResolutionContinuation::EntryCopySource {
                stack: ParkedStackResolution::new(item),
            },
        });
    }

    fn apply_entry_replacement(
        &self,
        event: &mut BattlefieldEntryEvent,
        effect_id: EntryReplacementEffectId,
    ) {
        match &effect_id {
            EntryReplacementEffectId::Intrinsic {
                object_id,
                copy_revision,
                ability_index,
            } => {
                debug_assert_eq!(*object_id, event.object_id);
                let effective_face = self
                    .state
                    .objects
                    .get(object_id)
                    .filter(|object| object.copy_revision == *copy_revision)
                    .and_then(|_| self.effective_face(*object_id));
                let ability = effective_face
                    .as_deref()
                    .and_then(|face| face.static_abilities.get(*ability_index));
                match ability {
                    Some(StaticAbilityDef::EntersAsCopy { .. }) => {
                        debug_assert!(false, "copy source choice must be completed before apply")
                    }
                    Some(StaticAbilityDef::EntersTapped {
                        affected: EntersTappedAffected::Self_,
                        ..
                    }) => event.tapped = true,
                    Some(StaticAbilityDef::EntersWithCounters {
                        affected: EntersWithCountersAffected::Self_,
                        counter,
                        amount,
                    }) => {
                        let count = self.resolve_amount(
                            amount,
                            AmountContext {
                                controller: event.destination_controller,
                                source_object_id: event.object_id,
                                source_zone_change: self
                                    .state
                                    .zone_change_generation
                                    .get(&event.object_id)
                                    .copied()
                                    .unwrap_or(0),
                                resolving_spell_id: None,
                                chosen_x: event.chosen_x,
                                previous_effect_result: None,
                            },
                        );
                        accumulate_entry_counters(&mut event.entry_counters, *counter, count);
                    }
                    _ => debug_assert!(false, "stale intrinsic entry replacement"),
                }
            }
            EntryReplacementEffectId::Battlefield {
                source_id,
                source_generation,
                ability_index,
            } => {
                let effective_face = self
                    .state
                    .objects
                    .get(source_id)
                    .filter(|source| source.zone == Zone::Battlefield)
                    .filter(|_| {
                        self.state
                            .zone_change_generation
                            .get(source_id)
                            .copied()
                            .unwrap_or(0)
                            == *source_generation
                    })
                    .and_then(|_| self.effective_face(*source_id));
                let ability = effective_face
                    .as_deref()
                    .and_then(|face| face.static_abilities.get(*ability_index));
                match ability {
                    Some(StaticAbilityDef::EntersTapped {
                        affected: EntersTappedAffected::Permanents,
                        ..
                    }) => event.tapped = true,
                    Some(StaticAbilityDef::EntersWithCounters {
                        affected: EntersWithCountersAffected::Creatures(_),
                        counter,
                        amount,
                    }) => {
                        let controller = self
                            .controller_of(*source_id)
                            .unwrap_or(event.destination_controller);
                        let count = self.resolve_amount(
                            amount,
                            AmountContext {
                                controller,
                                source_object_id: *source_id,
                                source_zone_change: *source_generation,
                                resolving_spell_id: None,
                                chosen_x: event.chosen_x,
                                previous_effect_result: None,
                            },
                        );
                        accumulate_entry_counters(&mut event.entry_counters, *counter, count);
                    }
                    _ => debug_assert!(false, "stale battlefield entry replacement"),
                }
            }
        }
        event.applied_effects.push(effect_id);
    }

    pub(super) fn begin_battlefield_entry(
        &mut self,
        item: StackItem,
        event: BattlefieldEntryEvent,
        completion: BattlefieldEntryCompletion,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> BattlefieldEntryProgress {
        self.advance_or_park_battlefield_entry(item, event, completion, events)
    }

    fn advance_or_park_battlefield_entry(
        &mut self,
        item: StackItem,
        mut event: BattlefieldEntryEvent,
        completion: BattlefieldEntryCompletion,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> BattlefieldEntryProgress {
        loop {
            let candidates = self.battlefield_entry_candidates(&event);
            match candidates.as_slice() {
                [] => return BattlefieldEntryProgress::Ready(event),
                [(effect_id, _, _)] => {
                    if let Some(filter) = self.entry_copy_filter(&event, effect_id) {
                        let sources = self.copy_source_candidates(&event, &filter);
                        if sources.is_empty() {
                            event.applied_effects.push(effect_id.clone());
                            continue;
                        }
                        self.park_copy_source_choice(
                            item,
                            event,
                            completion,
                            effect_id.clone(),
                            sources,
                            events,
                        );
                        return BattlefieldEntryProgress::Parked;
                    }
                    self.apply_entry_replacement(&mut event, effect_id.clone());
                }
                _ => {
                    let mut applications = Vec::new();
                    let mut application_ids = Vec::new();
                    let mut candidate_names = Vec::new();
                    for (effect_id, _, label) in candidates {
                        let application_id = self.state.next_replacement_application_id;
                        self.state.next_replacement_application_id =
                            application_id.saturating_add(1);
                        applications.push(EntryReplacementApplication {
                            application_id,
                            effect_id,
                        });
                        application_ids.push(application_id);
                        candidate_names.push(label);
                    }
                    let prompt = format!(
                        "Choose the next replacement effect for {} entering the battlefield.",
                        self.state
                            .objects
                            .get(&event.object_id)
                            .and_then(|object| self.registry.get(&object.card_id))
                            .map(|definition| definition.name.as_str())
                            .unwrap_or("this permanent")
                    );
                    events.push(rv1::RuledEvent {
                        ev: Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(
                            rv1::ResolutionChoiceRequired {
                                deciding_player_id: event.deciding_player,
                                source_object_id: event.object_id,
                                prompt_text: prompt.clone(),
                                choice_kind: rv1::ChoiceKind::ReplacementEffect as i32,
                                candidate_object_ids: application_ids.clone(),
                                candidate_card_ids: vec![String::new(); application_ids.len()],
                                min: 1,
                                max: 1,
                                ordered: false,
                                candidate_names,
                                candidate_server_card_ids: Vec::new(),
                                candidate_selectable: Vec::new(),
                                resolution_branches: Vec::new(),
                                mana_cost: String::new(),
                                unique_names: false,
                                generic_mana_cost: 0,
                                payment_currently_legal: false,
                                reveal_audience: 0,
                                revealed_zone_owner_player_id: None,
                                candidate_source_zones: Vec::new(),
                            },
                        )),
                    });
                    events.push(ev_log(prompt.clone()));
                    let deciding_player = event.deciding_player;
                    self.state.pending_replacement_event =
                        Some(PendingReplacementEvent::BattlefieldEntry(Box::new(
                            PendingBattlefieldEntry {
                                event,
                                applications,
                                copy_source_effect: None,
                                completion,
                            },
                        )));
                    self.state.pending_resolution = Some(PendingResolution {
                        deciding_player,
                        presentation: PendingResolutionPresentation {
                            source_object_id: item.id,
                            candidates: application_ids,
                            min: 1,
                            max: 1,
                            ordered: false,
                            prompt,
                            choice_kind: rv1::ChoiceKind::ReplacementEffect,
                            unique_names: false,
                        },
                        continuation: ResolutionContinuation::EntryReplacement {
                            stack: ParkedStackResolution::new(item),
                        },
                    });
                    return BattlefieldEntryProgress::Parked;
                }
            }
        }
    }

    pub(super) fn commit_battlefield_entry(
        &mut self,
        event: BattlefieldEntryEvent,
        attached_to: Option<AttachmentRecipient>,
    ) -> Result<(), EngineError> {
        let object_id = event.object_id;
        let door_event = self.commit_battlefield_entry_state(event, attached_to)?;
        let mut trigger_events = vec![GameEvent::EntersBattlefield { object_id }];
        trigger_events.extend(door_event);
        self.fire_triggers(&trigger_events);
        Ok(())
    }

    pub(super) fn commit_battlefield_entry_state(
        &mut self,
        event: BattlefieldEntryEvent,
        attached_to: Option<AttachmentRecipient>,
    ) -> Result<Option<GameEvent>, EngineError> {
        let (is_room, enters_as_copy) = self
            .state
            .objects
            .get(&event.object_id)
            .map(|object| {
                let copied_room = object
                    .copiable_values
                    .as_ref()
                    .is_some_and(|values| values.room_faces.is_some());
                let printed_room = self
                    .registry
                    .get(&object.card_id)
                    .is_some_and(|definition| definition.layout == Layout::Room);
                (
                    copied_room || printed_room,
                    object.copiable_values.is_some(),
                )
            })
            .ok_or(EngineError::Illegal("no object"))?;
        move_object_to_zone(
            &mut self.state,
            self.registry,
            event.object_id,
            Zone::Battlefield,
            Some(event.destination_controller),
        )?;
        let timestamp = self.state.command_index;
        if let Some(object) = self.state.objects.get_mut(&event.object_id) {
            object.face_up_index = event.face_index;
            object.tapped = event.tapped;
            object.counters.clear();
            object.counter_timestamps.clear();
            for (counter, count) in event.entry_counters {
                object.add_counters(counter, count, timestamp);
            }
            object.attached_to = attached_to;
        }
        if !is_room {
            return Ok(None);
        }
        self.state
            .room_states
            .insert(event.object_id, RoomState::default());
        if let Some(face_index) = event.unlock_room_door.filter(|_| !enters_as_copy) {
            return self
                .transition_room_door(event.object_id, face_index)
                .map(Some);
        }
        Ok(None)
    }

    pub(super) fn begin_token_entry_batch(
        &mut self,
        item: StackItem,
        mut entries: Vec<TokenBattlefieldEntry>,
        logs: Vec<String>,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<bool, EngineError> {
        // CR 616.1: when one simultaneous event requires choices from multiple players, those
        // players make them in APNAP order. Stable sorting preserves mint order within one seat.
        entries.sort_by_key(|entry| self.state.apnap_rank(entry.event.deciding_player));
        let mut ready = Vec::new();
        while !entries.is_empty() {
            let current = entries.remove(0);
            let completion = BattlefieldEntryCompletion::TokenBatch {
                current_created: current.created.clone(),
                ready: ready.clone(),
                remaining: entries.clone(),
                logs: logs.clone(),
            };
            match self.advance_or_park_battlefield_entry(
                item.clone(),
                current.event,
                completion,
                events,
            ) {
                BattlefieldEntryProgress::Parked => return Ok(true),
                BattlefieldEntryProgress::Ready(event) => ready.push(TokenBattlefieldEntry {
                    event,
                    created: current.created,
                }),
            }
        }
        self.commit_token_entry_batch(ready, logs, events)?;
        Ok(false)
    }

    fn continue_token_entry_batch(
        &mut self,
        item: StackItem,
        current: TokenBattlefieldEntry,
        mut ready: Vec<TokenBattlefieldEntry>,
        mut remaining: Vec<TokenBattlefieldEntry>,
        logs: Vec<String>,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<bool, EngineError> {
        ready.push(current);
        while !remaining.is_empty() {
            let next = remaining.remove(0);
            let completion = BattlefieldEntryCompletion::TokenBatch {
                current_created: next.created.clone(),
                ready: ready.clone(),
                remaining: remaining.clone(),
                logs: logs.clone(),
            };
            match self.advance_or_park_battlefield_entry(
                item.clone(),
                next.event,
                completion,
                events,
            ) {
                BattlefieldEntryProgress::Parked => return Ok(true),
                BattlefieldEntryProgress::Ready(event) => ready.push(TokenBattlefieldEntry {
                    event,
                    created: next.created,
                }),
            }
        }
        self.commit_token_entry_batch(ready, logs, events)?;
        Ok(false)
    }

    fn commit_token_entry_batch(
        &mut self,
        entries: Vec<TokenBattlefieldEntry>,
        logs: Vec<String>,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<(), EngineError> {
        let mut trigger_events = Vec::new();
        for entry in &entries {
            trigger_events.push(GameEvent::EntersBattlefield {
                object_id: entry.event.object_id,
            });
            if let Some(door_event) =
                self.commit_battlefield_entry_state(entry.event.clone(), None)?
            {
                trigger_events.push(door_event);
            }
        }
        for entry in entries {
            events.push(rv1::RuledEvent {
                ev: Some(rv1::ruled_event::Ev::TokenCreated(entry.created)),
            });
        }
        self.fire_triggers(&trigger_events);
        events.extend(logs.into_iter().map(ev_log));
        Ok(())
    }

    fn complete_pending_battlefield_entry(
        &mut self,
        pending: PendingResolution,
        event: BattlefieldEntryEvent,
        completion: BattlefieldEntryCompletion,
        mut events: Vec<rv1::RuledEvent>,
    ) -> Result<RuledEventBatch, EngineError> {
        let stack = pending
            .continuation
            .stack()
            .ok_or(EngineError::Illegal(
                "battlefield-entry continuation missing",
            ))?
            .clone();
        match completion {
            BattlefieldEntryCompletion::LandPlay { player, land_name } => {
                self.commit_battlefield_entry(event, None)?;
                self.state.passes_since_stack_change = 0;
                events.push(ev_log(format!("P{player} played {land_name}")));
                Ok(finish_with_events(self, events))
            }
            BattlefieldEntryCompletion::PermanentSpell { attached_to } => {
                events.push(rv1::RuledEvent {
                    ev: Some(rv1::ruled_event::Ev::StackResolved(rv1::StackResolved {
                        object_id: event.object_id,
                        destination: rv1::StackResolveDestination::Battlefield as i32,
                        owner_player_id: self
                            .state
                            .objects
                            .get(&event.object_id)
                            .map(|object| object.owner),
                    })),
                });
                self.commit_battlefield_entry(event, attached_to)?;
                self.complete_parked_resolution(stack.item, Some(0), events)
            }
            BattlefieldEntryCompletion::ResolutionEffect {
                owner,
                spell_label,
                object_label,
            } => {
                let object_id = event.object_id;
                self.commit_battlefield_entry(event, None)?;
                events.push(ev_log(format!(
                    "{spell_label} returns {object_label} from graveyard to battlefield."
                )));
                events.push(permanent_moved_event(
                    &self.state,
                    object_id,
                    owner,
                    rv1::permanent_moved::Destination::Battlefield,
                ));
                self.complete_parked_resolution(stack.item, stack.resume_effect_index, events)
            }
            BattlefieldEntryCompletion::LibrarySearch {
                owner,
                card_label,
                shuffle,
                resume_effect_index,
            } => {
                let object_id = event.object_id;
                let controller = event.destination_controller;
                self.commit_battlefield_entry(event, None)?;
                events.push(ev_log(format!(
                    "P{controller} puts {card_label} onto the battlefield."
                )));
                events.push(permanent_moved_event(
                    &self.state,
                    object_id,
                    owner,
                    rv1::permanent_moved::Destination::Battlefield,
                ));
                if shuffle {
                    crate::engine::shuffle_player_library_for_current_command(
                        &mut self.state,
                        controller,
                    );
                    events.push(ev_log(format!("P{controller} shuffles their library.")));
                }
                self.complete_parked_resolution(stack.item, resume_effect_index, events)
            }
            BattlefieldEntryCompletion::ManifestDread {
                owner,
                other_object_id,
                chosen_library_position,
            } => {
                let object_id = event.object_id;
                self.commit_battlefield_entry(event, None)?;
                events.push(permanent_moved_event_with_library_position(
                    &self.state,
                    object_id,
                    owner,
                    rv1::permanent_moved::Destination::Battlefield,
                    chosen_library_position,
                ));
                if let Some(other) = other_object_id {
                    move_object_to_zone(
                        &mut self.state,
                        self.registry,
                        other,
                        Zone::Graveyard,
                        None,
                    )?;
                    events.push(permanent_moved_event_with_library_position(
                        &self.state,
                        other,
                        owner,
                        rv1::permanent_moved::Destination::Graveyard,
                        0,
                    ));
                }
                events.push(ev_log(format!("P{owner} manifests dread.")));
                self.complete_parked_resolution(stack.item, stack.resume_effect_index, events)
            }
            BattlefieldEntryCompletion::TokenBatch {
                current_created,
                ready,
                remaining,
                logs,
            } => {
                let current = TokenBattlefieldEntry {
                    event,
                    created: current_created,
                };
                if self.continue_token_entry_batch(
                    stack.item.clone(),
                    current,
                    ready,
                    remaining,
                    logs,
                    &mut events,
                )? {
                    return Ok(finish_with_events(self, events));
                }
                self.complete_parked_resolution(stack.item, stack.resume_effect_index, events)
            }
            BattlefieldEntryCompletion::DevPlacement {
                target,
                ready,
                name,
                verb,
                deferred_events,
                announce_move,
            } => {
                self.complete_dev_battlefield_placement(
                    event,
                    target,
                    ready,
                    &name,
                    &verb,
                    deferred_events,
                    announce_move,
                    &mut events,
                )?;
                Ok(finish_with_events(self, events))
            }
        }
    }

    pub(super) fn finish_entry_copy_source_choice(
        &mut self,
        pending: PendingResolution,
        chosen: &[ObjectId],
    ) -> Result<RuledEventBatch, EngineError> {
        let stack = match &pending.continuation {
            ResolutionContinuation::EntryCopySource { stack } => stack.clone(),
            _ => return Err(EngineError::Illegal("copy-source continuation missing")),
        };
        let Some(pending_event) = self.state.pending_replacement_event.take() else {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal("copy source choice is stale"));
        };
        let mut entry = match pending_event {
            PendingReplacementEvent::BattlefieldEntry(entry) => *entry,
            other => {
                self.state.pending_replacement_event = Some(other);
                self.state.pending_resolution = Some(pending);
                return Err(EngineError::Illegal("copy source choice is stale"));
            }
        };
        let Some(effect_id) = entry.copy_source_effect.take() else {
            self.state.pending_replacement_event =
                Some(PendingReplacementEvent::BattlefieldEntry(Box::new(entry)));
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal("copy source choice is stale"));
        };
        let Some(filter) = self.entry_copy_filter(&entry.event, &effect_id) else {
            entry.copy_source_effect = Some(effect_id);
            self.state.pending_replacement_event =
                Some(PendingReplacementEvent::BattlefieldEntry(Box::new(entry)));
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal("copy replacement is stale"));
        };

        if let Some(&source_id) = chosen.first() {
            if source_id == entry.event.object_id
                || !object_matches_mass_filter(self, source_id, &filter)
            {
                entry.copy_source_effect = Some(effect_id);
                self.state.pending_replacement_event =
                    Some(PendingReplacementEvent::BattlefieldEntry(Box::new(entry)));
                self.state.pending_resolution = Some(pending);
                return Err(EngineError::Illegal("copy source is stale"));
            }
            let Some(values) = self.copiable_values_for(source_id) else {
                entry.copy_source_effect = Some(effect_id);
                self.state.pending_replacement_event =
                    Some(PendingReplacementEvent::BattlefieldEntry(Box::new(entry)));
                self.state.pending_resolution = Some(pending);
                return Err(EngineError::Illegal("copy source is stale"));
            };
            let Some(object) = self.state.objects.get_mut(&entry.event.object_id) else {
                entry.copy_source_effect = Some(effect_id);
                self.state.pending_replacement_event =
                    Some(PendingReplacementEvent::BattlefieldEntry(Box::new(entry)));
                self.state.pending_resolution = Some(pending);
                return Err(EngineError::Illegal("entering copy object is stale"));
            };
            object.must_attack_if_able = values.face.must_attack_if_able;
            object.must_block_if_able = values.face.must_block_if_able;
            object.copiable_values = Some(values);
            object.copy_revision = object.copy_revision.saturating_add(1);
        }
        entry.event.applied_effects.push(effect_id);

        let mut events = Vec::new();
        let event = match self.advance_or_park_battlefield_entry(
            stack.item,
            entry.event,
            entry.completion.clone(),
            &mut events,
        ) {
            BattlefieldEntryProgress::Parked => return Ok(finish_with_events(self, events)),
            BattlefieldEntryProgress::Ready(event) => event,
        };
        self.complete_pending_battlefield_entry(pending, event, entry.completion, events)
    }

    pub(super) fn finish_battlefield_entry_replacement_choice(
        &mut self,
        pending: PendingResolution,
        application_id: u32,
    ) -> Result<RuledEventBatch, EngineError> {
        let stack = match &pending.continuation {
            ResolutionContinuation::EntryReplacement { stack } => stack.clone(),
            _ => {
                return Err(EngineError::Illegal(
                    "entry-replacement continuation missing",
                ))
            }
        };
        let Some(pending_event) = self.state.pending_replacement_event.take() else {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal(
                "battlefield-entry replacement choice is stale",
            ));
        };
        let mut entry = match pending_event {
            PendingReplacementEvent::BattlefieldEntry(entry) => *entry,
            other => {
                self.state.pending_replacement_event = Some(other);
                self.state.pending_resolution = Some(pending);
                return Err(EngineError::Illegal(
                    "battlefield-entry replacement choice is stale",
                ));
            }
        };
        let Some(application) = entry
            .applications
            .iter()
            .find(|application| application.application_id == application_id)
            .cloned()
        else {
            self.state.pending_replacement_event =
                Some(PendingReplacementEvent::BattlefieldEntry(Box::new(entry)));
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal("replacement application is stale"));
        };

        let mut events = Vec::new();
        if let Some(filter) = self.entry_copy_filter(&entry.event, &application.effect_id) {
            let sources = self.copy_source_candidates(&entry.event, &filter);
            if !sources.is_empty() {
                self.park_copy_source_choice(
                    stack.item,
                    entry.event,
                    entry.completion,
                    application.effect_id,
                    sources,
                    &mut events,
                );
                return Ok(finish_with_events(self, events));
            }
            entry.event.applied_effects.push(application.effect_id);
        } else {
            self.apply_entry_replacement(&mut entry.event, application.effect_id);
        }
        let event = match self.advance_or_park_battlefield_entry(
            stack.item,
            entry.event,
            entry.completion.clone(),
            &mut events,
        ) {
            BattlefieldEntryProgress::Parked => return Ok(finish_with_events(self, events)),
            BattlefieldEntryProgress::Ready(event) => event,
        };

        self.complete_pending_battlefield_entry(pending, event, entry.completion, events)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entry_counter_accumulation_saturates_and_omits_zero_entries() {
        let mut counters = BTreeMap::new();
        accumulate_entry_counters(&mut counters, CounterKind::PlusOnePlusOne, 0);
        assert!(counters.is_empty());

        accumulate_entry_counters(&mut counters, CounterKind::PlusOnePlusOne, u32::MAX - 1);
        accumulate_entry_counters(&mut counters, CounterKind::PlusOnePlusOne, 4);
        assert_eq!(counters[&CounterKind::PlusOnePlusOne], u32::MAX);
    }

    #[test]
    fn conditional_entry_uses_the_captured_life_snapshot() {
        let mut engine = GameEngine::new(97_007, &[0, 1], 20, None, true).expect("engine");
        let snapshot = engine.player_life_snapshot();
        engine.state.players[1].life = 1;
        let event = BattlefieldEntryEvent {
            object_id: 999,
            deciding_player: 0,
            destination_controller: 0,
            face_index: 0,
            unlock_room_door: None,
            chosen_x: 0,
            player_life_snapshot: snapshot,
            tapped: false,
            entry_counters: BTreeMap::new(),
            applied_effects: Vec::new(),
        };
        let condition = GameCondition::PlayerLifeAggregate {
            players: RelativePlayerSet::All,
            aggregate: PlayerLifeAggregate::Minimum,
            min: Some(14),
            max: None,
        };

        assert!(engine.entry_condition_holds(&condition, &event));
        assert!(!engine.condition_holds(
            &condition,
            ConditionContext {
                controller: 0,
                source_object_id: 999,
                source_zone_change: 0,
                resolving_spell_id: None,
            }
        ));
    }

    #[test]
    fn a_globe_in_the_same_proposed_batch_is_not_a_battlefield_source() {
        let decks = Some(vec![
            vec![
                "dragonstorm_globe".into(),
                "sparktongue_dragon".into(),
                "mountain".into(),
                "mountain".into(),
                "mountain".into(),
                "mountain".into(),
                "mountain".into(),
            ],
            vec!["forest".into(); 7],
        ]);
        let mut engine = GameEngine::new(97_008, &[0, 1], 20, decks, true).expect("engine");
        let globe = engine
            .state
            .objects
            .values()
            .find(|object| object.card_id == "dragonstorm_globe")
            .expect("Globe")
            .id;
        let dragon = engine
            .state
            .objects
            .values()
            .find(|object| object.card_id == "sparktongue_dragon")
            .expect("Dragon")
            .id;
        engine.state.objects.get_mut(&globe).unwrap().zone = Zone::Stack;
        engine.state.objects.get_mut(&dragon).unwrap().zone = Zone::Stack;
        let event = BattlefieldEntryEvent {
            object_id: dragon,
            deciding_player: 0,
            destination_controller: 0,
            face_index: 0,
            unlock_room_door: None,
            chosen_x: 0,
            player_life_snapshot: engine.player_life_snapshot(),
            tapped: false,
            entry_counters: BTreeMap::new(),
            applied_effects: Vec::new(),
        };

        assert!(engine.battlefield_entry_candidates(&event).is_empty());
        engine.state.objects.get_mut(&globe).unwrap().zone = Zone::Battlefield;
        assert_eq!(engine.battlefield_entry_candidates(&event).len(), 1);
    }
}
