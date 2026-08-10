//! Shared CR 614/616 replacement ordering and battlefield-entry preprocessing.

use super::events::{ev_log, finish_with_events};
use super::resolution::{move_object_to_zone, permanent_moved_event};
use super::*;

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
    fn battlefield_entry_candidates(
        &self,
        event: &BattlefieldEntryEvent,
    ) -> Vec<(EntryReplacementEffectId, ReplacementPriority, String)> {
        if event.tapped {
            return Vec::new();
        }

        let Some(entering) = self.state.objects.get(&event.object_id) else {
            return Vec::new();
        };
        let mut candidates = Vec::new();
        if let Some(face) = self
            .registry
            .get(&entering.card_id)
            .and_then(|definition| definition.face(event.face_index))
        {
            for (ability_index, ability) in face.static_abilities.iter().enumerate() {
                if matches!(
                    ability,
                    StaticAbilityDef::EntersTapped {
                        affected: EntersTappedAffected::Self_
                    }
                ) {
                    let effect_id = EntryReplacementEffectId::Intrinsic {
                        object_id: event.object_id,
                        ability_index,
                    };
                    if !event.applied_effects.contains(&effect_id) {
                        candidates.push((
                            effect_id,
                            ReplacementPriority::Other,
                            format!("{} — enters tapped", face.name),
                        ));
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
            let Some(face) = self
                .registry
                .get(&source.card_id)
                .and_then(|definition| definition.face(source.face_up_index))
            else {
                continue;
            };
            for (ability_index, ability) in face.static_abilities.iter().enumerate() {
                if matches!(
                    ability,
                    StaticAbilityDef::EntersTapped {
                        affected: EntersTappedAffected::Permanents
                    }
                ) {
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
                        candidates.push((
                            effect_id,
                            ReplacementPriority::Other,
                            format!(
                                "{} (P{}, object {}) — permanents enter tapped",
                                face.name, source.controller, source.id
                            ),
                        ));
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

    fn apply_entry_replacement(
        event: &mut BattlefieldEntryEvent,
        effect_id: EntryReplacementEffectId,
    ) {
        event.tapped = true;
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
                    Self::apply_entry_replacement(&mut event, effect_id.clone());
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
                                unique_names: false,
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
                                completion,
                            },
                        )));
                    self.state.pending_resolution = Some(PendingResolution {
                        item,
                        custom_key: "__replacement_effect".to_string(),
                        step: 1,
                        scratch: Vec::new(),
                        deciding_player,
                        candidates: application_ids,
                        min: 1,
                        max: 1,
                        ordered: false,
                        prompt,
                        choice_kind: rv1::ChoiceKind::ReplacementEffect,
                        unique_names: false,
                        copy_source_object_id: 0,
                        search_destination: SearchDestination::Hand,
                        search_shuffle: false,
                        search_reveal: false,
                        resume_effect_index: None,
                    });
                    return BattlefieldEntryProgress::Parked;
                }
            }
        }
    }

    pub(super) fn commit_battlefield_entry(
        &mut self,
        event: BattlefieldEntryEvent,
        attached_to: Option<ObjectId>,
    ) -> Result<(), EngineError> {
        let object_id = event.object_id;
        self.commit_battlefield_entry_state(event, attached_to)?;
        self.fire_triggers(&[GameEvent::EntersBattlefield { object_id }]);
        Ok(())
    }

    pub(super) fn commit_battlefield_entry_state(
        &mut self,
        event: BattlefieldEntryEvent,
        attached_to: Option<ObjectId>,
    ) -> Result<(), EngineError> {
        move_object_to_zone(
            &mut self.state,
            self.registry,
            event.object_id,
            Zone::Battlefield,
            Some(event.destination_controller),
        )?;
        if let Some(object) = self.state.objects.get_mut(&event.object_id) {
            object.face_up_index = event.face_index;
            object.tapped = event.tapped;
            object.attached_to = attached_to;
        }
        Ok(())
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
        let trigger_events: Vec<_> = entries
            .iter()
            .map(|entry| GameEvent::EntersBattlefield {
                object_id: entry.event.object_id,
            })
            .collect();
        for entry in &entries {
            self.commit_battlefield_entry_state(entry.event.clone(), None)?;
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

    pub(super) fn finish_battlefield_entry_replacement_choice(
        &mut self,
        pending: PendingResolution,
        application_id: u32,
    ) -> Result<RuledEventBatch, EngineError> {
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

        Self::apply_entry_replacement(&mut entry.event, application.effect_id);
        let mut events = Vec::new();
        let event = match self.advance_or_park_battlefield_entry(
            pending.item.clone(),
            entry.event,
            entry.completion.clone(),
            &mut events,
        ) {
            BattlefieldEntryProgress::Parked => return Ok(finish_with_events(self, events)),
            BattlefieldEntryProgress::Ready(event) => event,
        };

        match entry.completion {
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
                    })),
                });
                self.commit_battlefield_entry(event, attached_to)?;
                self.complete_parked_resolution(pending.item, Some(0), events)
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
                self.complete_parked_resolution(pending.item, pending.resume_effect_index, events)
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
                    pending.item.clone(),
                    current,
                    ready,
                    remaining,
                    logs,
                    &mut events,
                )? {
                    return Ok(finish_with_events(self, events));
                }
                self.complete_parked_resolution(pending.item, pending.resume_effect_index, events)
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
}
