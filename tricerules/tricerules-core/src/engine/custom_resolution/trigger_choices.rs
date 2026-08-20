use super::*;

impl GameEngine {
    pub(in crate::engine) fn choose_trigger_target(
        &mut self,
        player: PlayerId,
        targets: &[rv1::TargetRef],
        selected_modes: &[rv1::SelectedSpellMode],
        decline: bool,
    ) -> Result<RuledEventBatch, EngineError> {
        let pending = self
            .state
            .pending_triggers
            .pop_front()
            .ok_or(EngineError::Illegal("no pending trigger awaiting target"))?;

        if pending.controller != player {
            self.state.pending_triggers.push_front(pending);
            return Err(EngineError::Illegal("not your trigger to target"));
        }

        if decline {
            if !pending.may {
                self.state.pending_triggers.push_front(pending);
                return Err(EngineError::Illegal("trigger is not optional"));
            }
            let mut batch = RuledEventBatch::default();
            batch.events.push(ev_log(format!(
                "P{player} declines optional trigger: {}",
                pending.ability_text
            )));
            self.resume_trigger_placement(&mut batch);
            fill_legal(&mut batch, self);
            return Ok(batch);
        }

        // Validate before consuming `pending`: it has already been popped off the queue, so any
        // early return has to put it back. Otherwise a rejected target (clicking the wrong
        // permanent, or answering the wrong trigger when two are queued) silently destroys the
        // trigger while the client is still showing its prompt — and Decline then fails too,
        // because the engine no longer believes anything is pending.
        let card_name = self
            .registry
            .get(&pending.card_id)
            .map(|definition| definition.name.clone())
            .unwrap_or_else(|| pending.card_id.clone());
        let target_source =
            TargetSourceIdentity::captured(pending.source_permanent_id, pending.source_zone_change);
        let mut chosen_modes = Vec::new();
        let mut public_targets = Vec::new();
        let mut chosen_mode_indices = Vec::new();
        let mut chosen_mode_labels = Vec::new();
        let validation = if let Some(modal) = &pending.ability.modal {
            if !targets.is_empty()
                || selected_modes.len() < modal.min_modes as usize
                || selected_modes.len() > modal.max_modes as usize
            {
                Err(EngineError::Illegal("illegal triggered mode selection"))
            } else {
                let mut seen = HashSet::new();
                let mut ordered = selected_modes.iter().collect::<Vec<_>>();
                ordered.sort_by_key(|selection| selection.mode_index);
                let mut result = Ok(());
                for selection in ordered {
                    if !seen.insert(selection.mode_index) {
                        result = Err(EngineError::Illegal("a mode may be selected only once"));
                        break;
                    }
                    let Some(mode) = modal.modes.get(selection.mode_index as usize) else {
                        result = Err(EngineError::Illegal("bad triggered mode index"));
                        break;
                    };
                    if let Err(error) = validate_ability_targets_with_context(
                        self,
                        player,
                        target_source,
                        &mode.effects,
                        mode.targeting.as_ref(),
                        &selection.targets,
                        pending.trigger_context,
                    ) {
                        result = Err(error);
                        break;
                    }
                    public_targets.extend(selection.targets.iter().cloned());
                    chosen_mode_indices.push(selection.mode_index);
                    chosen_mode_labels.push(mode.label.clone());
                    chosen_modes.push(ChosenMode {
                        mode_index: selection.mode_index as usize,
                        targets: selection
                            .targets
                            .iter()
                            .map(|target| capture_stack_target(self, target))
                            .collect(),
                    });
                }
                result
            }
        } else if selected_modes.is_empty() {
            validate_ability_targets_with_context(
                self,
                player,
                target_source,
                &pending.ability.effect,
                pending.ability.targeting.as_ref(),
                targets,
                pending.trigger_context,
            )
            .map(|()| public_targets.extend_from_slice(targets))
        } else {
            Err(EngineError::Illegal("nonmodal trigger cannot select modes"))
        };
        let validated = validation.map(|()| card_name);
        let card_name = match validated {
            Ok(name) => name,
            Err(e) => {
                self.state.pending_triggers.push_front(pending);
                return Err(e);
            }
        };

        // Reserved when the trigger was collected, so the id the client saw in
        // `TriggerOrderRequired` is the id it now sees on the stack.
        let virtual_id = pending.object_id;

        let ability_text = pending.ability_text.clone();
        let card_id = pending.card_id.clone();
        let source_id = pending.source_permanent_id;
        let source_face_index = pending.source_face_index;
        let source_zone_change = pending.source_zone_change;
        let source_face_change = pending.source_face_change;
        let ability_index = pending.ability_index;
        let ability = pending.ability;
        let controller = pending.controller;
        let trigger_context = pending.trigger_context;

        let trefs = public_targets
            .iter()
            .map(|target| target.object_id)
            .collect::<Vec<_>>();
        let tgt_line = format_spell_targets_log(&self.state, self.registry, &trefs);

        self.state.stack.push(StackItem {
            id: virtual_id,
            controller,
            card_id: card_id.clone(),
            targets: public_targets
                .iter()
                .map(|target| capture_stack_target(self, target))
                .collect(),
            ability_text: Some(ability_text.clone()),
            source_permanent_id: Some(source_id),
            source_zone_change,
            source_face_change,
            ability_index: Some(ability_index),
            activated_ability: None,
            triggered_ability: Some(ability),
            is_triggered: true,
            is_copy: false,
            chosen_x: 0,
            face_index: source_face_index,
            chosen_modes,
            resolution_branch_choices: Default::default(),
            trigger_context,
            flashback: false,
        });
        self.state.passes_since_stack_change = 0;

        let mut batch = RuledEventBatch::default();
        batch.events.push(ev_log(format!(
            "P{controller} {card_name} trigger targets{tgt_line}"
        )));
        batch.events.push(rv1::RuledEvent {
            ev: Some(rv1::ruled_event::Ev::StackPushed(rv1::StackPushed {
                object_id: virtual_id,
                description: card_name,
                targets: public_targets.clone(),
                ability_annotation: ability_text,
                card_id: String::new(),
                is_copy: false,
                is_triggered: true,
                copy_source_object_id: 0,
                chosen_mode_indices,
                chosen_mode_labels,
            })),
        });

        // Choosing a target for the original triggered ability can itself cause target-watchers
        // to trigger. This new simultaneous group waits behind the group currently being placed.
        self.fire_triggers(&[GameEvent::TargetsChosen {
            controller,
            source: TargetingSourceKind::Ability,
            targets: trefs,
        }]);

        self.resume_trigger_placement(&mut batch);
        fill_legal(&mut batch, self);
        Ok(batch)
    }

    /// Continue placing staged triggers after one of them stopped the drain, and hand priority back
    /// when nothing is left to decide.
    ///
    /// Answering a trigger's target (or declining it) is the only thing that was blocking
    /// [`Self::flush_staged_triggers`], so it resumes here and either places the rest of the
    /// simultaneous group, parks the next target choice, or raises the next player's ordering
    /// prompt. The three call sites used to hand-roll "emit TriggerNeedsTarget for the new front,
    /// else priority" — that duplicate is gone now that at most one target is ever parked.
    pub(super) fn resume_trigger_placement(&mut self, batch: &mut RuledEventBatch) {
        self.flush_staged_triggers(&mut batch.events);
        if self.state.blocking_choice().is_none() {
            batch.events.push(ev_priority_changed(self));
        }
    }

    /// CR 603.3b: put the named trigger on the stack next.
    ///
    /// One pick, not a permutation. CR 603.3d chooses an ability's targets *as it is put on the
    /// stack*, so placing this one may immediately park a target choice; the remaining candidates
    /// stay in `pending_trigger_order` and the next prompt only goes out once that target is
    /// answered. [`Self::flush_staged_triggers`] owns all of that sequencing — this function just
    /// moves one trigger from the candidate set into the placement path.
    ///
    /// What you pick first is placed first and therefore resolves last (CR 405.5).
    pub(in crate::engine) fn submit_trigger_order(
        &mut self,
        player: PlayerId,
        trigger_object_id: u32,
    ) -> Result<RuledEventBatch, EngineError> {
        let pending = self
            .state
            .pending_trigger_order
            .as_mut()
            .ok_or(EngineError::Illegal("no simultaneous triggers to order"))?;

        // Unlike `choose_trigger_target` this validates *before* mutating, so there is nothing to
        // restore on a rejection — a refused pick leaves the prompt exactly as it was, which it
        // must, since nothing but this command can clear it.
        if pending.deciding_player != player {
            return Err(EngineError::Illegal("not your triggers to order"));
        }
        let Some(index) = pending
            .candidates
            .iter()
            .position(|candidate| candidate.object_id == trigger_object_id)
        else {
            return Err(EngineError::Illegal(
                "that trigger is not one of the ones waiting to be ordered",
            ));
        };

        let staged = pending.candidates.remove(index);
        // The candidate set changed, so the next drain must re-announce what is left.
        pending.prompt_emitted = false;
        let card_name = staged.card_name.clone();

        let mut batch = RuledEventBatch::default();
        batch.events.push(ev_log(format!(
            "P{player} puts {card_name} on the stack next"
        )));
        self.push_trigger(staged, &mut batch.events);
        self.flush_staged_triggers(&mut batch.events);
        if self.state.blocking_choice().is_none() {
            batch.events.push(ev_priority_changed(self));
        }
        fill_legal(&mut batch, self);
        Ok(batch)
    }
}
