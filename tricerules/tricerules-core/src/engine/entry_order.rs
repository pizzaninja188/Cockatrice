//! CR 613.7m: player-chosen timestamp order for simultaneous battlefield entries.

use super::*;

impl GameEngine {
    fn simultaneous_entry_ids(batch: &SimultaneousEntryBatch) -> Vec<(ObjectId, PlayerId)> {
        match batch {
            SimultaneousEntryBatch::Zone(batch) => batch
                .ready
                .iter()
                .map(|entry| (entry.object_id, entry.destination_controller))
                .collect(),
            SimultaneousEntryBatch::Token(batch) => batch
                .ready
                .iter()
                .map(|entry| (entry.event.object_id, entry.event.destination_controller))
                .collect(),
            SimultaneousEntryBatch::Observer(batch) => batch
                .ready
                .iter()
                .map(|entry| (entry.event.object_id, entry.event.destination_controller))
                .collect(),
        }
    }

    fn entry_timestamp_cohort_current(&self, order: &PendingEntryTimestampOrder) -> bool {
        order
            .original_generations
            .iter()
            .all(|(oid, generation, zone)| {
                self.state
                    .objects
                    .get(oid)
                    .is_some_and(|object| object.zone == *zone)
                    && self
                        .state
                        .zone_change_generation
                        .get(oid)
                        .copied()
                        .unwrap_or(0)
                        == *generation
            })
    }

    fn reorder_simultaneous_entry_batch(
        mut batch: SimultaneousEntryBatch,
        order: &[ObjectId],
    ) -> SimultaneousEntryBatch {
        let rank = order
            .iter()
            .enumerate()
            .map(|(position, &oid)| (oid, position))
            .collect::<HashMap<_, _>>();
        match &mut batch {
            SimultaneousEntryBatch::Zone(batch) => {
                batch.ready.sort_by_key(|entry| rank[&entry.object_id]);
            }
            SimultaneousEntryBatch::Token(batch) => {
                batch
                    .ready
                    .sort_by_key(|entry| rank[&entry.event.object_id]);
            }
            SimultaneousEntryBatch::Observer(batch) => {
                batch
                    .ready
                    .sort_by_key(|entry| rank[&entry.event.object_id]);
            }
        }
        batch
    }

    fn advance_entry_timestamp_order(
        &mut self,
        mut order: PendingEntryTimestampOrder,
        stack: Option<ParkedStackResolution>,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<Option<SimultaneousEntryBatch>, EngineError> {
        while let Some((player, candidates)) = order.remaining_groups.pop_front() {
            if candidates.len() == 1 {
                order.chosen_order.push(candidates[0]);
                continue;
            }
            let prompt =
                "Choose your simultaneous battlefield entries from earliest to latest timestamp."
                    .to_string();
            let source_object_id = stack.as_ref().map_or(0, |stack| stack.item.id);
            let names = candidates
                .iter()
                .map(|&oid| events::object_display_name(&self.state, self.registry, oid))
                .collect::<Vec<_>>();
            let card_ids = candidates
                .iter()
                .map(|oid| self.state.objects[oid].card_id.clone())
                .collect::<Vec<_>>();
            let count = candidates.len() as u32;
            let candidate_token_identities = if candidates
                .iter()
                .any(|oid| self.state.objects[oid].is_token())
            {
                candidates
                    .iter()
                    .map(|&oid| {
                        if self.state.objects[&oid].is_token() {
                            self.copiable_values_for(oid)
                                .map(|values| resolution::token_identity(&values))
                                .expect("proposed token retains its copiable identity")
                        } else {
                            rv1::TokenIdentity::default()
                        }
                    })
                    .collect()
            } else {
                Vec::new()
            };
            events.push(rv1::RuledEvent {
                ev: Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(
                    rv1::ResolutionChoiceRequired {
                        deciding_player_id: player,
                        source_object_id,
                        prompt_text: prompt.clone(),
                        choice_kind: rv1::ChoiceKind::SimultaneousEntryOrder as i32,
                        candidate_object_ids: candidates.clone(),
                        candidate_card_ids: card_ids,
                        candidate_names: names,
                        candidate_token_identities,
                        min: count,
                        max: count,
                        ordered: true,
                        ..Default::default()
                    },
                )),
            });
            events.push(events::ev_log_private(prompt.clone(), player));
            self.state.pending_resolution = Some(PendingResolution {
                deciding_player: player,
                presentation: PendingResolutionPresentation {
                    source_object_id,
                    candidates,
                    min: count,
                    max: count,
                    ordered: true,
                    prompt,
                    choice_kind: rv1::ChoiceKind::SimultaneousEntryOrder,
                    unique_names: false,
                },
                continuation: ResolutionContinuation::SimultaneousEntryOrder {
                    stack,
                    order: Box::new(order),
                },
            });
            return Ok(None);
        }
        Ok(Some(Self::reorder_simultaneous_entry_batch(
            order.batch,
            &order.chosen_order,
        )))
    }

    pub(super) fn begin_entry_timestamp_order(
        &mut self,
        batch: SimultaneousEntryBatch,
        stack: Option<ParkedStackResolution>,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<Option<SimultaneousEntryBatch>, EngineError> {
        let entrants = Self::simultaneous_entry_ids(&batch);
        let original_generations = entrants
            .iter()
            .map(|(oid, _)| {
                let object = &self.state.objects[oid];
                (
                    *oid,
                    self.state
                        .zone_change_generation
                        .get(oid)
                        .copied()
                        .unwrap_or(0),
                    object.zone,
                )
            })
            .collect();
        let mut by_player = BTreeMap::<usize, (PlayerId, Vec<ObjectId>)>::new();
        for (oid, controller) in entrants {
            by_player
                .entry(self.state.apnap_rank(controller))
                .or_insert_with(|| (controller, Vec::new()))
                .1
                .push(oid);
        }
        self.advance_entry_timestamp_order(
            PendingEntryTimestampOrder {
                batch,
                original_generations,
                remaining_groups: by_player.into_values().collect(),
                chosen_order: Vec::new(),
            },
            stack,
            events,
        )
    }

    pub(super) fn finish_entry_timestamp_order_choice(
        &mut self,
        pending: PendingResolution,
        chosen: &[ObjectId],
    ) -> Result<RuledEventBatch, EngineError> {
        let ResolutionContinuation::SimultaneousEntryOrder { stack, mut order } =
            pending.continuation.clone()
        else {
            return Err(EngineError::Illegal("entry-order continuation missing"));
        };
        if !self.entry_timestamp_cohort_current(&order) {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal(
                "simultaneous entry cohort became stale",
            ));
        }
        order.chosen_order.extend_from_slice(chosen);
        let mut events = Vec::new();
        let Some(batch) = self.advance_entry_timestamp_order(*order, stack.clone(), &mut events)?
        else {
            return Ok(events::finish_with_events(self, events));
        };
        match batch {
            SimultaneousEntryBatch::Zone(batch) => {
                let stack = stack.ok_or(EngineError::Illegal("zone entry has no stack"))?;
                self.commit_zone_entry_batch_ready(batch, &mut events)?;
                self.complete_parked_resolution(stack.item, stack.resume_effect_index, events)
            }
            SimultaneousEntryBatch::Token(batch) => {
                let stack = stack.ok_or(EngineError::Illegal("token entry has no stack"))?;
                let created_ids = batch.result_object_ids.clone();
                let amass = batch.options.amass;
                self.commit_token_entry_batch(
                    &stack.item,
                    batch.ready,
                    batch.logs,
                    batch.options.attacking,
                    batch.options.delayed_sacrifice,
                    &mut events,
                )?;
                if let Some(amass) = amass {
                    return self.finish_amass_after_token_entry(stack, amass, events);
                }
                let previous_result = EffectResult {
                    produced_objects: self.token_entry_object_refs(&created_ids),
                    ..EffectResult::default()
                };
                self.complete_parked_resolution_with_previous(
                    stack.item,
                    stack.resume_effect_index,
                    previous_result,
                    events,
                )
            }
            SimultaneousEntryBatch::Observer(batch) => {
                self.commit_observer_return_batch(batch.ready, &mut events)?;
                if let Some(stack) = batch.resume_stack {
                    self.complete_parked_resolution_with_previous(
                        stack.item,
                        stack.resume_effect_index,
                        stack.previous_result,
                        events,
                    )
                } else {
                    self.apply_sbas(&mut events)?;
                    if let Some(index) = self.state.player_idx(self.state.active_player_id()) {
                        self.state.priority_idx = index;
                    }
                    events.push(events::ev_priority_changed(self));
                    Ok(events::finish_with_events(self, events))
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn choice(ids: Vec<ObjectId>) -> rv1::RuledCommand {
        rv1::RuledCommand {
            cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(
                rv1::SubmitResolutionChoice {
                    chosen_object_ids: ids,
                    ..Default::default()
                },
            )),
        }
    }

    #[test]
    fn token_batch_resumes_after_logged_order_and_stamps_chosen_order() {
        let mut engine = GameEngine::new(613_704, &[0, 1], 20, None, true).unwrap();
        let source = engine.state.players[0].hand[0];
        let stack = engine.observer_return_item(source, 0);
        let (entries, logs) = engine
            .prepare_token_entries(
                resolution::TokenCreationRequest {
                    token_id: "soldier_w_1_1",
                    copy: None,
                    count: 2,
                    recipients: vec![0],
                    spell_label: "create soldiers",
                    item: &stack,
                },
                false,
            )
            .unwrap();
        let ids: Vec<_> = entries.iter().map(|entry| entry.event.object_id).collect();
        assert!(engine
            .begin_token_entry_batch(
                stack,
                entries,
                logs,
                TokenEntryBatchOptions::default(),
                &mut Vec::new(),
            )
            .unwrap());
        assert_eq!(
            engine
                .state
                .pending_resolution
                .as_ref()
                .unwrap()
                .deciding_player,
            0
        );
        let ResolutionContinuation::SimultaneousEntryOrder { order, .. } = &engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .continuation
        else {
            panic!("token entry order continuation");
        };
        let SimultaneousEntryBatch::Token(batch) = &order.batch else {
            panic!("token entry batch");
        };
        assert_eq!(
            batch.result_object_ids, ids,
            "result order keeps the original mint order"
        );
        assert!(ids
            .iter()
            .all(|oid| engine.state.objects[oid].zone == Zone::Stack));
        engine
            .apply_command(0, &choice(vec![ids[1], ids[0]]))
            .unwrap();
        assert!(engine.state.pending_resolution.is_none());
        assert!(ids
            .iter()
            .all(|oid| engine.state.objects[oid].zone == Zone::Battlefield));
        assert!(
            engine.state.battlefield_entry_timestamps[&ids[1]]
                < engine.state.battlefield_entry_timestamps[&ids[0]]
        );
        assert_eq!(engine.token_entry_object_refs(&ids)[0].object_id, ids[0]);
    }

    #[test]
    fn observer_returns_preserve_ready_cohort_until_logged_order_commits() {
        let mut engine = GameEngine::new(
            613_705,
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
            move_object_to_zone(&mut engine.state, engine.registry, oid, Zone::Exile, None)
                .unwrap();
            engine.state.pending_immediate_observer_actions.push(
                ImmediateObserverAction::ReturnExiledObject {
                    exiled: TriggerObjectRef {
                        object_id: oid,
                        zone_change_generation: engine
                            .state
                            .zone_change_generation
                            .get(&oid)
                            .copied()
                            .unwrap_or(0),
                        controller_at_event: 0,
                    },
                },
            );
        }
        assert!(engine
            .drain_immediate_observer_actions(None, &mut Vec::new())
            .unwrap());
        assert_eq!(
            engine
                .state
                .pending_resolution
                .as_ref()
                .unwrap()
                .deciding_player,
            0
        );
        assert!(ids
            .iter()
            .all(|oid| engine.state.objects[oid].zone == Zone::Exile));
        engine
            .apply_command(0, &choice(vec![ids[1], ids[0]]))
            .unwrap();
        assert!(engine.state.pending_resolution.is_none());
        assert!(ids
            .iter()
            .all(|oid| engine.state.objects[oid].zone == Zone::Battlefield));
        assert!(
            engine.state.battlefield_entry_timestamps[&ids[1]]
                < engine.state.battlefield_entry_timestamps[&ids[0]]
        );
    }

    #[test]
    fn aura_return_pauses_preserve_the_observer_cohort_and_resume_the_effect_tail() {
        let mut engine = GameEngine::new(
            613_706,
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
            .take(4)
            .collect();
        let (first, aura, last, attachment) = (ids[0], ids[1], ids[2], ids[3]);
        engine.state.objects.get_mut(&aura).unwrap().card_id = "pacifism".into();
        engine.state.objects.get_mut(&attachment).unwrap().card_id = "twenty-toed_toad".into();
        move_object_to_zone(
            &mut engine.state,
            engine.registry,
            attachment,
            Zone::Battlefield,
            Some(0),
        )
        .unwrap();

        for object_id in [first, aura, last] {
            move_object_to_zone(
                &mut engine.state,
                engine.registry,
                object_id,
                Zone::Exile,
                None,
            )
            .unwrap();
            engine.state.pending_immediate_observer_actions.push(
                ImmediateObserverAction::ReturnExiledObject {
                    exiled: TriggerObjectRef {
                        object_id,
                        zone_change_generation: engine
                            .state
                            .zone_change_generation
                            .get(&object_id)
                            .copied()
                            .unwrap_or(0),
                        controller_at_event: 0,
                    },
                },
            );
        }

        let mut item = engine.observer_return_item(first, 0);
        item.card_id = "opt".into();
        item.ability_text = None;
        let mut stack = ParkedStackResolution::new(item);
        stack.resume_effect_index = Some(0);
        assert!(engine
            .drain_immediate_observer_actions(Some(stack), &mut Vec::new())
            .unwrap());
        assert_eq!(
            engine
                .state
                .pending_resolution
                .as_ref()
                .unwrap()
                .presentation
                .choice_kind,
            rv1::ChoiceKind::AuraPermanent
        );
        let parked = engine.state.pending_observer_return_batch.as_ref().unwrap();
        assert_eq!(parked.ready.len(), 1);
        assert_eq!(parked.remaining.len(), 1);
        assert_eq!(engine.state.objects[&first].zone, Zone::Exile);

        engine
            .apply_command(0, &choice(vec![attachment]))
            .expect("choose the creature enchanted by the returning Aura");
        let pending = engine.state.pending_resolution.as_ref().unwrap();
        assert_eq!(
            pending.presentation.choice_kind,
            rv1::ChoiceKind::SimultaneousEntryOrder
        );
        assert_eq!(pending.presentation.candidates.len(), 3);
        assert!([first, aura, last]
            .iter()
            .all(|object_id| engine.state.objects[object_id].zone == Zone::Exile));
        let chosen_order = pending
            .presentation
            .candidates
            .iter()
            .rev()
            .copied()
            .collect();
        engine
            .apply_command(0, &choice(chosen_order))
            .expect("choose timestamps after the Aura return resolves");

        let scry = engine.state.pending_resolution.as_ref().unwrap();
        assert_eq!(scry.presentation.choice_kind, rv1::ChoiceKind::LibraryTop);
        assert!([first, aura, last]
            .iter()
            .all(|object_id| engine.state.objects[object_id].zone == Zone::Battlefield));
        assert_eq!(
            engine.state.objects[&aura].attached_to,
            Some(AttachmentRecipient::Object(attachment))
        );
        let hand_before_draw = engine.state.players[0].hand.len();
        engine
            .apply_command(0, &choice(vec![]))
            .expect("keep Opt's scried card on top and finish its draw");
        assert!(engine.state.pending_resolution.is_none());
        assert_eq!(engine.state.players[0].hand.len(), hand_before_draw + 1);
    }
}
