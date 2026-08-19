use super::events::{
    ev_log, ev_log_hidden_from, ev_log_private, ev_priority_changed, finish_with_events,
    format_spell_targets_log, object_display_name,
};
use super::legal_actions::fill_legal;
use super::resolution::{
    counter_stack_spell, move_object_to_zone, permanent_moved_event, sacrifice_permanent,
    seat_resolved_spell_last_in_graveyard,
};
use super::targeting::{
    capture_stack_target, object_matches_mass_filter, validate_ability_targets_with_context,
    validate_spell_targets, TargetSourceIdentity,
};
use super::*;

impl GameEngine {
    pub(super) fn resolution_payment_choice_event(&self) -> Option<rv1::RuledEvent> {
        let pending = self.state.pending_resolution.as_ref()?;
        let payment = pending.mana_payment.as_ref()?;
        Some(rv1::RuledEvent {
            ev: Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(
                rv1::ResolutionChoiceRequired {
                    deciding_player_id: pending.deciding_player,
                    source_object_id: pending.item.id,
                    prompt_text: pending.prompt.clone(),
                    choice_kind: rv1::ChoiceKind::ManaPayment as i32,
                    candidate_object_ids: Vec::new(),
                    candidate_card_ids: Vec::new(),
                    min: 0,
                    max: 0,
                    ordered: false,
                    candidate_names: Vec::new(),
                    candidate_server_card_ids: Vec::new(),
                    candidate_selectable: Vec::new(),
                    resolution_branches: Vec::new(),
                    unique_names: false,
                    generic_mana_cost: payment.generic_mana_cost,
                    payment_currently_legal: if payment.mana_cost.pips.is_empty() {
                        self.can_pay_generic_mana(
                            pending.deciding_player,
                            payment.generic_mana_cost,
                        )
                    } else {
                        self.can_pay_resolution_mana(pending.deciding_player, &payment.mana_cost)
                    },
                    mana_cost: payment.mana_cost.to_string(),
                },
            )),
        })
    }

    pub(super) fn choose_trigger_target(
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
    pub(super) fn submit_trigger_order(
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

    /// Begin a tier-3 custom resolution (CR 608).
    pub(super) fn begin_custom_resolution(
        &mut self,
        item: StackItem,
        custom_key: String,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<(), EngineError> {
        let effect = custom::lookup(&custom_key)
            .ok_or_else(|| EngineError::MissingCard(custom_key.clone()))?;
        let controller = item.controller;
        let (step, scratch) = {
            let mut ctx = ResolutionCtx::new(
                &mut self.state,
                self.registry,
                events,
                controller,
                0,
                Vec::new(),
            );
            let r = effect.begin(&mut ctx);
            (r, ctx.scratch)
        };
        self.park_or_finish(item, custom_key, 0, scratch, step, events);
        Ok(())
    }

    /// Apply a deciding player's answer to the outstanding [`PendingResolution`] (CR 608).
    pub(super) fn submit_resolution_choice(
        &mut self,
        player: PlayerId,
        answer: &rv1::SubmitResolutionChoice,
    ) -> Result<RuledEventBatch, EngineError> {
        let pending = self
            .state
            .pending_resolution
            .take()
            .ok_or(EngineError::Illegal("no resolution awaiting a choice"))?;
        if pending.deciding_player != player {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal("not your resolution choice"));
        }
        let decision = match rv1::ResolutionChoiceDecision::try_from(answer.decision) {
            Ok(decision) => decision,
            Err(_) => {
                self.state.pending_resolution = Some(pending);
                return Err(EngineError::Illegal("unknown resolution choice decision"));
            }
        };
        if pending.custom_key == "__resolution_branch"
            && pending.choice_kind == rv1::ChoiceKind::ResolutionBranch
        {
            return self.select_resolution_branch(pending, answer, decision);
        }
        if let Some(payment) = pending.mana_payment.clone() {
            if !answer.chosen_object_ids.is_empty() {
                self.state.pending_resolution = Some(pending);
                return Err(EngineError::Illegal(
                    "mana payment choice cannot include object ids",
                ));
            }
            let mut events = Vec::new();
            match decision {
                rv1::ResolutionChoiceDecision::PayMana => {
                    let payable = if payment.mana_cost.pips.is_empty() {
                        self.can_pay_generic_mana(player, payment.generic_mana_cost)
                    } else {
                        self.can_pay_resolution_mana(player, &payment.mana_cost)
                    };
                    if !payable {
                        self.state.pending_resolution = Some(pending);
                        return Err(EngineError::Illegal(
                            "resolution mana payment is not affordable",
                        ));
                    }
                    let paid = if payment.mana_cost.pips.is_empty() {
                        self.pay_generic_mana(player, payment.generic_mana_cost)
                    } else {
                        self.pay_resolution_mana(player, &payment.mana_cost)
                    };
                    if let Err(error) = paid {
                        self.state.pending_resolution = Some(pending);
                        return Err(error);
                    }
                    let cost_label = if payment.mana_cost.pips.is_empty() {
                        format!("{{{}}}", payment.generic_mana_cost)
                    } else {
                        payment.mana_cost.to_string()
                    };
                    events.push(ev_log(format!(
                        "P{player} pays {cost_label} during resolution."
                    )));
                    self.state.undoable_mana_abilities.clear();
                }
                rv1::ResolutionChoiceDecision::Decline => {
                    while self.state.undoable_mana_abilities.len() > payment.undo_history_start {
                        events.push(self.rewind_last_undoable_mana_ability(
                            player,
                            payment.undo_history_start,
                        )?);
                    }
                    // Resolution is consequential even though the payment-time entries were
                    // rewound. Older float stays in the pool but is no longer eligible for Undo.
                    self.state.undoable_mana_abilities.clear();
                    if pending.resolution_branch.is_some() {
                        let effect_index = pending
                            .resume_effect_index
                            .and_then(|next| next.checked_sub(1))
                            .ok_or(EngineError::Illegal(
                                "resolution branch continuation missing",
                            ))?;
                        let mut item = pending.item;
                        item.resolution_branch_choices.insert(effect_index, None);
                        return self.complete_parked_resolution(item, Some(effect_index), events);
                    } else {
                        let counter_label = self
                            .registry
                            .get(&pending.item.card_id)
                            .map(|definition| definition.name.clone())
                            .unwrap_or_else(|| pending.item.card_id.clone());
                        counter_stack_spell(
                            self,
                            payment.target_spell_id,
                            &counter_label,
                            &mut events,
                        )?;
                    }
                }
                rv1::ResolutionChoiceDecision::Unspecified => {
                    self.state.pending_resolution = Some(pending);
                    return Err(EngineError::Illegal(
                        "mana payment choice requires pay or decline",
                    ));
                }
                rv1::ResolutionChoiceDecision::SelectBranch => {
                    self.state.pending_resolution = Some(pending);
                    return Err(EngineError::Illegal(
                        "mana payment choice cannot select a branch",
                    ));
                }
            }
            let resume = if pending.resolution_branch.is_some() {
                pending
                    .resume_effect_index
                    .and_then(|next| next.checked_sub(1))
            } else {
                pending.resume_effect_index
            };
            return self.complete_parked_resolution(pending.item, resume, events);
        }
        if decision != rv1::ResolutionChoiceDecision::Unspecified {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal(
                "object resolution choice must leave decision unspecified",
            ));
        }
        let chosen = answer.chosen_object_ids.as_slice();
        let n = chosen.len() as u32;
        if n < pending.min || n > pending.max {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal("wrong number of cards chosen"));
        }
        let mut seen = HashSet::new();
        for &oid in chosen {
            if !pending.candidates.contains(&oid) || !seen.insert(oid) {
                self.state.pending_resolution = Some(pending);
                return Err(EngineError::Illegal("invalid resolution choice"));
            }
        }
        if pending.unique_names {
            let mut name_seen: HashSet<String> = HashSet::new();
            for &oid in chosen {
                let card_id = self
                    .state
                    .objects
                    .get(&oid)
                    .map(|o| o.card_id.clone())
                    .unwrap_or_default();
                let name = self
                    .registry
                    .get(&card_id)
                    .map(|d| d.name.clone())
                    .unwrap_or(card_id);
                if !name_seen.insert(name) {
                    self.state.pending_resolution = Some(pending);
                    return Err(EngineError::Illegal(
                        "chosen cards must have different names",
                    ));
                }
            }
        }

        // CR 707.10c: copy target choice is not a tier-3 CardEffect; handle it directly.
        if pending.custom_key == "__copy_targets" {
            return self.finish_copy_target_choice(pending, chosen);
        }
        // CR 701.18: library search completion (SearchLibrary primitive) — move the chosen card
        // to the declared destination and optionally shuffle.
        if pending.custom_key == "__search_library" {
            return self.finish_library_search(pending, chosen);
        }

        // CR 701.18: scry — step 0 picks the cards going to the bottom, step 1 orders the rest.
        if pending.custom_key == "__scry" {
            return self.finish_scry(pending, chosen);
        }
        if pending.custom_key.starts_with("__look_choose_bottom_") {
            return self.finish_look_choose_bottom(pending, chosen);
        }

        // DiscardCards (caster-chooses): move each chosen card from the target's hand to graveyard.
        if pending.custom_key == "__discard_chosen" {
            return self.finish_discard_chosen(pending, chosen);
        }

        // CR 701.17: sacrifice choice — target player picks which qualifying permanent to lose.
        if pending.custom_key == "__sacrifice_chosen" {
            return self.finish_sacrifice_chosen(pending, chosen);
        }
        if pending.custom_key == "__resolution_branch" {
            return self.finish_resolution_branch_object(pending, chosen);
        }
        if pending.custom_key == "__entry_copy_source" {
            return self.finish_entry_copy_source_choice(pending, chosen);
        }
        if pending.custom_key == "__replacement_effect" {
            return match self.state.pending_replacement_event.as_ref() {
                Some(super::replacement::PendingReplacementEvent::Damage(_)) => {
                    self.finish_damage_prevention_choice(pending, chosen[0])
                }
                Some(super::replacement::PendingReplacementEvent::BattlefieldEntry(_)) => {
                    self.finish_battlefield_entry_replacement_choice(pending, chosen[0])
                }
                None => {
                    self.state.pending_resolution = Some(pending);
                    Err(EngineError::Illegal("replacement choice is stale"))
                }
            };
        }

        // CR 704.5j: legend SBA choice — the chosen object id is the legend to KEEP;
        // all others are sacrificed through the normal die path so LTB/death triggers fire.
        if pending.custom_key == "__legend_sba" {
            return self.finish_legend_sba_choice(pending, chosen);
        }

        let effect = match custom::lookup(&pending.custom_key) {
            Some(e) => e,
            None => {
                let key = pending.custom_key.clone();
                self.state.pending_resolution = Some(pending);
                return Err(EngineError::MissingCard(key));
            }
        };
        let controller = pending.item.controller;
        let item = pending.item;
        let custom_key = pending.custom_key;
        let step_no = pending.step;
        let choice = ResolutionChoice {
            object_ids: chosen.to_vec(),
        };

        let mut ev = vec![];
        let (step, scratch) = {
            let mut ctx = ResolutionCtx::new(
                &mut self.state,
                self.registry,
                &mut ev,
                controller,
                step_no,
                pending.scratch,
            );
            let r = effect.resume(&mut ctx, &choice);
            (r, ctx.scratch)
        };
        self.park_or_finish(item, custom_key, step_no, scratch, step, &mut ev);

        if self.state.pending_resolution.is_none() {
            if let Some(i) = self.state.player_idx(self.state.active_player_id()) {
                self.state.priority_idx = i;
            }
            ev.push(ev_priority_changed(self));
        }
        Ok(finish_with_events(self, ev))
    }

    /// CR 707.10c: check the targets chosen for a copy without mutating anything, so a rejection
    /// can leave the pending choice intact for the player to try again.
    ///
    /// Returns one target vector per chosen mode (empty for a nonmodal copy), so the caller
    /// re-slices nothing and the mode boundaries are computed exactly once.
    fn validated_copy_targets(
        &self,
        pending: &PendingResolution,
        chosen: &[u32],
    ) -> Result<Vec<Vec<ObjectId>>, EngineError> {
        let item = &pending.item;
        let controller = item.controller;
        let target_source = TargetSourceIdentity::for_stack_item(self, item);
        let face = self
            .registry
            .get(&item.card_id)
            .and_then(|definition| definition.face(item.face_index));

        if item.chosen_modes.is_empty() {
            let effects = face.map(|f| f.spell_effect.to_vec()).unwrap_or_default();
            self.validate_copy_target_group(
                controller,
                target_source,
                &effects,
                chosen,
                &item.targets,
            )?;
            return Ok(vec![]);
        }

        let modal = face
            .and_then(|f| f.modal_spell.as_ref())
            .ok_or(EngineError::Illegal(
                "copied modal spell has no mode definition",
            ))?;
        let mut per_mode = Vec::with_capacity(item.chosen_modes.len());
        let mut offset = 0;
        for chosen_mode in &item.chosen_modes {
            let end = offset + chosen_mode.targets.len();
            let mode_targets = chosen
                .get(offset..end)
                .ok_or(EngineError::Illegal("wrong number of copied modal targets"))?;
            let mode = modal
                .modes
                .get(chosen_mode.mode_index)
                .ok_or(EngineError::Illegal("copied modal mode no longer exists"))?;
            self.validate_copy_target_group(
                controller,
                target_source,
                &mode.effects,
                mode_targets,
                &chosen_mode.targets,
            )?;
            per_mode.push(mode_targets.to_vec());
            offset = end;
        }
        if offset != chosen.len() {
            return Err(EngineError::Illegal("wrong number of copied modal targets"));
        }
        Ok(per_mode)
    }

    /// CR 707.10c: a target the copy's controller left *unchanged* stays legal even if it would
    /// not be legal to choose now — which is exactly why [`copy_target_spell`] offers the
    /// original targets as candidates. Only a target that was actually changed has to be legal.
    ///
    /// Count, candidate membership and global uniqueness are already enforced by
    /// [`GameEngine::submit_resolution_choice`], so this validates legality per target.
    fn validate_copy_target_group(
        &self,
        controller: PlayerId,
        target_source: TargetSourceIdentity,
        effects: &[SpellEffectKind],
        chosen: &[ObjectId],
        original: &[StackTarget],
    ) -> Result<(), EngineError> {
        for (index, &object_id) in chosen.iter().enumerate() {
            if original.get(index).map(|target| target.object_id) == Some(object_id) {
                continue;
            }
            let original_target = original
                .get(index)
                .ok_or(EngineError::Illegal("wrong number of copied targets"))?;
            let target_ref = rv1::TargetRef {
                object_id,
                damage_amount: original_target.damage_amount,
                group_index: original_target.group_index,
                kind: original_target.kind,
            };
            validate_spell_targets(
                self,
                controller,
                target_source,
                effects,
                None,
                std::slice::from_ref(&target_ref),
            )?;
        }
        Ok(())
    }

    /// CR 707.10c: the copy's controller has chosen new targets for the copy. Push the copy to
    /// the stack with the chosen targets and hand priority back to the active player.
    fn finish_copy_target_choice(
        &mut self,
        pending: PendingResolution,
        chosen: &[u32],
    ) -> Result<RuledEventBatch, EngineError> {
        let copy_id = pending.item.id;
        let card_id = pending.item.card_id.clone();
        let face_index = pending.item.face_index;
        let controller = pending.item.controller;
        let copy_source_object_id = pending.copy_source_object_id;

        // Validate before consuming `pending`: this choice was already taken out of
        // `state.pending_resolution`, so any early return has to put it back or the copy is lost
        // and the client waits forever on a prompt the engine has forgotten.
        let per_mode_targets = match self.validated_copy_targets(&pending, chosen) {
            Ok(targets) => targets,
            Err(e) => {
                self.state.pending_resolution = Some(pending);
                return Err(e);
            }
        };

        // Everything below this point is infallible.
        let mut copy_item = pending.item;
        for (chosen_mode, mode_targets) in copy_item.chosen_modes.iter_mut().zip(per_mode_targets) {
            for (target, object_id) in chosen_mode.targets.iter_mut().zip(mode_targets) {
                target.object_id = object_id;
            }
        }
        for (target, object_id) in copy_item.targets.iter_mut().zip(chosen) {
            target.object_id = *object_id;
        }

        let copied_name = self
            .registry
            .get(&card_id)
            .and_then(|d| d.face(face_index))
            .map(|f| f.name.to_string())
            .unwrap_or_else(|| card_id.clone());

        let published_targets = copy_item
            .targets
            .iter()
            .map(|target| rv1::TargetRef {
                object_id: target.object_id,
                damage_amount: target.damage_amount,
                group_index: target.group_index,
                kind: target.kind,
            })
            .collect();
        self.state.stack.push(copy_item);
        self.state.passes_since_stack_change = 0;

        self.fire_triggers(&[GameEvent::TargetsChosen {
            controller,
            source: TargetingSourceKind::SpellCopy,
            targets: chosen.to_vec(),
        }]);

        let tgt_log = format_spell_targets_log(&self.state, self.registry, chosen);
        let mut ev = vec![
            rv1::RuledEvent {
                ev: Some(rv1::ruled_event::Ev::StackPushed(rv1::StackPushed {
                    object_id: copy_id,
                    description: copied_name.clone(),
                    targets: published_targets,
                    ability_annotation: "(copy)".to_string(),
                    card_id: card_id.clone(),
                    is_copy: true,
                    is_triggered: false,
                    copy_source_object_id,
                    chosen_mode_indices: vec![],
                    chosen_mode_labels: vec![],
                })),
            },
            ev_log(format!(
                "{copied_name} copy targeting{tgt_log} (P{controller})"
            )),
        ];

        if let Some(i) = self.state.player_idx(self.state.active_player_id()) {
            self.state.priority_idx = i;
        }
        ev.push(ev_priority_changed(self));

        Ok(finish_with_events(self, ev))
    }

    /// CR 701.17: the target player has chosen which permanent to sacrifice.
    fn finish_sacrifice_chosen(
        &mut self,
        pending: PendingResolution,
        chosen: &[u32],
    ) -> Result<RuledEventBatch, EngineError> {
        let oid = chosen[0];
        let card_name = super::events::object_display_name(&self.state, self.registry, oid);
        // Capture last-known information before the zone move clears transient state.
        let owner = self
            .state
            .objects
            .get(&oid)
            .map(|o| o.owner)
            .ok_or(EngineError::Illegal("sacrificed object missing"))?;
        let source = self
            .trigger_source_snapshot(oid)
            .ok_or(EngineError::Illegal("sacrificed object missing"))?;
        let was_creature = self
            .characteristics(oid)
            .is_some_and(|value| value.is_creature());

        sacrifice_permanent(&mut self.state, self.registry, oid)?;

        let mut ev = vec![
            permanent_moved_event(
                &self.state,
                oid,
                owner,
                rv1::permanent_moved::Destination::Graveyard,
            ),
            ev_log(format!(
                "P{} sacrifices {card_name}.",
                pending.deciding_player
            )),
        ];

        self.fire_triggers(&[GameEvent::Dies {
            source,
            was_creature,
        }]);
        let _ = self.apply_sbas(&mut ev);

        self.complete_parked_resolution(pending.item, pending.resume_effect_index, ev)
    }

    /// CR 704.5j: the controller has chosen which legend to keep. Sacrifice all other candidates
    /// via `sacrifice_permanent` so LTB / death triggers fire normally, then re-run SBAs.
    fn finish_legend_sba_choice(
        &mut self,
        pending: PendingResolution,
        chosen: &[u32],
    ) -> Result<RuledEventBatch, EngineError> {
        let keep_id = chosen[0];
        let mut ev = vec![];
        let mut trigger_events = vec![];
        for &oid in &pending.candidates {
            if oid == keep_id {
                continue;
            }
            let owner = self.state.objects.get(&oid).map(|o| o.owner);
            let source = self.trigger_source_snapshot(oid);
            let was_creature = self
                .characteristics(oid)
                .is_some_and(|value| value.is_creature());
            if sacrifice_permanent(&mut self.state, self.registry, oid).is_ok() {
                if let Some(owner_id) = owner {
                    ev.push(permanent_moved_event(
                        &self.state,
                        oid,
                        owner_id,
                        rv1::permanent_moved::Destination::Graveyard,
                    ));
                }
                if let Some(source) = source {
                    trigger_events.push(GameEvent::Dies {
                        source,
                        was_creature,
                    });
                }
            }
        }
        self.fire_triggers(&trigger_events);
        // Re-run SBAs: triggered abilities may have caused further state changes, and
        // multiple legend conflicts are resolved one at a time.
        if self.state.pending_resolution.is_none() {
            if let Some(i) = self.state.player_idx(self.state.active_player_id()) {
                self.state.priority_idx = i;
            }
            self.apply_sbas(&mut ev)?;
            if self.state.pending_resolution.is_none() {
                ev.push(ev_priority_changed(self));
            }
        }
        Ok(finish_with_events(self, ev))
    }

    /// CR 701.18: apply one step of a scry.
    ///
    /// Step 0's `chosen` is the set going to the bottom of the library; the cards left over stay
    /// on top. If two or more stay on top the player still has an ordering decision to make
    /// (CR 701.18a "in any order"), so a second interrupt is parked for it — skipped when 0 or 1
    /// card remains, where the "choice" has exactly one answer. Scry 1 therefore never reaches
    /// step 1.
    ///
    /// **Both steps place cards one at a time, in submitted order, moving away from the middle of
    /// the library.** Step 0 pushes each successive card further down, so its *last* entry is
    /// bottom-most; step 1 pushes each successive card further up, so its *last* entry is the next
    /// card drawn (matching Brainstorm's put-back). The two prompts spell their direction out,
    /// since it is not self-evident from the UI.
    ///
    /// Both steps are pure reorders of the library `VecDeque`: scry looks at cards without moving
    /// them between zones, so nothing here goes through `move_object_to_zone` and no zone-change
    /// trigger fires.
    fn finish_scry(
        &mut self,
        pending: PendingResolution,
        chosen: &[u32],
    ) -> Result<RuledEventBatch, EngineError> {
        let controller = pending.deciding_player;
        let Some(idx) = self.state.player_idx(controller) else {
            return Err(EngineError::Illegal("scrying player missing"));
        };
        let mut ev = vec![];

        if pending.step == 0 {
            // Everything looked at that was not sent to the bottom stays on top, keeping the
            // library order it already had. The bottomed cards go down in submitted order
            // (`push_back` each in turn), so the last one clicked ends up bottom-most.
            let remaining: Vec<ObjectId> = pending
                .scratch
                .iter()
                .copied()
                .filter(|oid| !chosen.contains(oid))
                .collect();

            if !chosen.is_empty() {
                let names = self.object_names(chosen);
                self.state.players[idx]
                    .library
                    .retain(|o| !chosen.contains(o));
                for &oid in chosen {
                    self.state.players[idx].library.push_back(oid);
                }
                let noun = if chosen.len() == 1 { "card" } else { "cards" };
                ev.push(ev_log(format!(
                    "P{controller} puts {} {noun} on the bottom of their library.",
                    chosen.len()
                )));
                ev.push(ev_log_private(
                    format!("P{controller} bottoms {}.", names.join(", ")),
                    controller,
                ));
            } else {
                ev.push(ev_log(format!(
                    "P{controller} keeps every scried card on top."
                )));
            }

            if remaining.len() > 1 {
                return self.park_scry_ordering(pending, remaining, ev);
            }
        } else {
            // Step 1: `chosen` is every remaining card, bottom first. Pull them out and re-seat
            // them in front in submitted order, so the *last* one ends up as the next draw.
            self.state.players[idx]
                .library
                .retain(|o| !chosen.contains(o));
            for &oid in chosen {
                self.state.players[idx].library.push_front(oid);
            }
            ev.push(ev_log(format!(
                "P{controller} orders {} cards on top of their library.",
                chosen.len()
            )));
            ev.push(ev_log_private(
                format!(
                    "P{controller} puts {} back on top, in that order.",
                    self.object_names(chosen).join(", ")
                ),
                controller,
            ));
        }

        self.complete_parked_resolution(pending.item, pending.resume_effect_index, ev)
    }

    /// Park scry's second interrupt: order the cards staying on top (CR 701.18a "in any order").
    /// Same `item` and `resume_effect_index` as step 0, so the spell's tail still resumes after.
    fn park_scry_ordering(
        &mut self,
        pending: PendingResolution,
        remaining: Vec<ObjectId>,
        mut ev: Vec<rv1::RuledEvent>,
    ) -> Result<RuledEventBatch, EngineError> {
        let controller = pending.deciding_player;
        let n = remaining.len() as u32;
        let (candidate_card_ids, candidate_names) =
            super::resolution::candidate_identities(self, &remaining);
        let prompt = format!(
            "Scry: click the {n} cards staying on top in order — the last one you click is the \
             next card you draw."
        );
        ev.push(rv1::RuledEvent {
            ev: Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(
                rv1::ResolutionChoiceRequired {
                    deciding_player_id: controller,
                    source_object_id: pending.item.id,
                    prompt_text: prompt.clone(),
                    choice_kind: rv1::ChoiceKind::LibraryTop as i32,
                    candidate_object_ids: remaining.clone(),
                    candidate_card_ids,
                    candidate_names,
                    min: n,
                    max: n,
                    ordered: true,
                    unique_names: false,
                    candidate_server_card_ids: Vec::new(),
                    candidate_selectable: Vec::new(),
                    resolution_branches: Vec::new(),
                    mana_cost: String::new(),
                    generic_mana_cost: 0,
                    payment_currently_legal: false,
                },
            )),
        });
        ev.push(ev_log_private(prompt.clone(), controller));
        self.state.pending_resolution = Some(PendingResolution {
            step: 1,
            scratch: vec![],
            candidates: remaining,
            min: n,
            max: n,
            ordered: true,
            prompt,
            ..pending
        });
        Ok(finish_with_events(self, ev))
    }

    /// Finish either step of a bounded library look. Step 0 may move one matching card to hand;
    /// random-order cards finish immediately, while chosen-order cards park one more image-based
    /// ordered pick. Step 1 appends the complete submitted permutation to the library bottom.
    fn finish_look_choose_bottom(
        &mut self,
        pending: PendingResolution,
        chosen: &[ObjectId],
    ) -> Result<RuledEventBatch, EngineError> {
        let controller = pending.deciding_player;
        let Some(idx) = self.state.player_idx(controller) else {
            return Err(EngineError::Illegal("looking player missing"));
        };
        let mut ev = Vec::new();

        if pending.custom_key == "__look_choose_bottom_order" {
            self.state.players[idx]
                .library
                .retain(|oid| !chosen.contains(oid));
            for &oid in chosen {
                self.state.players[idx].library.push_back(oid);
            }
            ev.push(ev_log(format!(
                "P{controller} puts {} cards on the bottom of their library.",
                chosen.len()
            )));
            ev.push(ev_log_private(
                format!(
                    "P{controller} bottoms {}.",
                    self.object_names(chosen).join(", ")
                ),
                controller,
            ));
            return self.complete_parked_resolution(pending.item, pending.resume_effect_index, ev);
        }

        let selected = chosen.first().copied();
        let mut remaining: Vec<ObjectId> = pending
            .scratch
            .iter()
            .copied()
            .filter(|oid| Some(*oid) != selected)
            .collect();
        if let Some(oid) = selected {
            let name = object_display_name(&self.state, self.registry, oid);
            let owner = self.state.objects[&oid].owner;
            move_object_to_zone(&mut self.state, self.registry, oid, Zone::Hand, None)?;
            ev.push(ev_log(format!("P{controller} reveals {name}.")));
            ev.push(ev_log(format!(
                "P{controller} puts {name} into their hand."
            )));
            ev.push(permanent_moved_event(
                &self.state,
                oid,
                owner,
                rv1::permanent_moved::Destination::Hand,
            ));
        }

        if pending.custom_key == "__look_choose_bottom_random" {
            shuffle_object_ids_for_current_command(&self.state, controller, &mut remaining);
            self.state.players[idx]
                .library
                .retain(|oid| !remaining.contains(oid));
            for &oid in &remaining {
                self.state.players[idx].library.push_back(oid);
            }
            ev.push(ev_log(format!(
                "P{controller} puts {} cards on the bottom of their library in a random order.",
                remaining.len()
            )));
            ev.push(ev_log_private(
                format!(
                    "P{controller} randomly bottoms {}.",
                    self.object_names(&remaining).join(", ")
                ),
                controller,
            ));
            return self.complete_parked_resolution(pending.item, pending.resume_effect_index, ev);
        }

        if remaining.len() <= 1 {
            self.state.players[idx]
                .library
                .retain(|oid| !remaining.contains(oid));
            for &oid in &remaining {
                self.state.players[idx].library.push_back(oid);
            }
            return self.complete_parked_resolution(pending.item, pending.resume_effect_index, ev);
        }

        let n = remaining.len() as u32;
        let (candidate_card_ids, candidate_names) =
            super::resolution::candidate_identities(self, &remaining);
        let prompt = format!(
            "Click all {n} remaining card images in bottom order. The last image clicked becomes bottom-most."
        );
        ev.push(rv1::RuledEvent {
            ev: Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(
                rv1::ResolutionChoiceRequired {
                    deciding_player_id: controller,
                    source_object_id: pending.item.id,
                    prompt_text: prompt.clone(),
                    choice_kind: rv1::ChoiceKind::LibraryLook as i32,
                    candidate_object_ids: remaining.clone(),
                    candidate_card_ids,
                    candidate_names,
                    min: n,
                    max: n,
                    ordered: true,
                    unique_names: false,
                    candidate_server_card_ids: Vec::new(),
                    resolution_branches: Vec::new(),
                    mana_cost: String::new(),
                    generic_mana_cost: 0,
                    payment_currently_legal: false,
                    candidate_selectable: vec![true; remaining.len()],
                },
            )),
        });
        self.state.pending_resolution = Some(PendingResolution {
            custom_key: "__look_choose_bottom_order".to_string(),
            step: 1,
            scratch: Vec::new(),
            candidates: std::mem::take(&mut remaining),
            min: n,
            max: n,
            ordered: true,
            prompt,
            ..pending
        });
        Ok(finish_with_events(self, ev))
    }

    /// Display names for `oids`, in order (registry lookup, never Oracle).
    fn object_names(&self, oids: &[ObjectId]) -> Vec<String> {
        oids.iter()
            .map(|&oid| object_display_name(&self.state, self.registry, oid))
            .collect()
    }

    pub(super) fn resolution_cost_candidates(
        &self,
        player: PlayerId,
        cost: &ResolutionCost,
    ) -> Vec<ObjectId> {
        let Some(index) = self.state.player_idx(player) else {
            return Vec::new();
        };
        match cost {
            ResolutionCost::None => Vec::new(),
            ResolutionCost::Mana(_) => Vec::new(),
            ResolutionCost::DiscardCard { filter } => self.state.players[index]
                .hand
                .iter()
                .copied()
                .filter(|oid| {
                    resolution::library_card_matches_filter(
                        &self.state,
                        self.registry,
                        *oid,
                        filter.as_ref(),
                    )
                })
                .collect(),
            ResolutionCost::SacrificePermanent { filter } => self.state.players[index]
                .battlefield
                .iter()
                .copied()
                .filter(|oid| object_matches_mass_filter(self, *oid, filter))
                .collect(),
        }
    }

    fn select_resolution_branch(
        &mut self,
        mut pending: PendingResolution,
        answer: &rv1::SubmitResolutionChoice,
        decision: rv1::ResolutionChoiceDecision,
    ) -> Result<RuledEventBatch, EngineError> {
        if !answer.chosen_object_ids.is_empty() {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal(
                "resolution branch selection cannot include object ids",
            ));
        }
        let branch_state = pending
            .resolution_branch
            .clone()
            .ok_or(EngineError::Illegal(
                "resolution branch continuation missing",
            ))?;
        let effect_index = pending
            .resume_effect_index
            .and_then(|next| next.checked_sub(1))
            .ok_or(EngineError::Illegal(
                "resolution branch effect index missing",
            ))?;

        if decision == rv1::ResolutionChoiceDecision::Decline {
            if !branch_state.optional {
                self.state.pending_resolution = Some(pending);
                return Err(EngineError::Illegal("resolution branch is not optional"));
            }
            pending
                .item
                .resolution_branch_choices
                .insert(effect_index, None);
            return self.complete_parked_resolution(
                pending.item,
                Some(effect_index),
                vec![ev_log(format!(
                    "P{} declines the optional resolution choice.",
                    pending.deciding_player
                ))],
            );
        }
        if decision != rv1::ResolutionChoiceDecision::SelectBranch {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal(
                "resolution branch requires a branch selection or decline",
            ));
        }
        let branch_index = answer.selected_branch_index as usize;
        let Some(branch) = branch_state.branches.get(branch_index).cloned() else {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal("bad resolution branch index"));
        };
        let candidates = self.resolution_cost_candidates(pending.deciding_player, &branch.cost);
        if !matches!(branch.cost, ResolutionCost::None | ResolutionCost::Mana(_))
            && candidates.is_empty()
        {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal(
                "that resolution branch no longer has a legal payment",
            ));
        }
        pending
            .item
            .resolution_branch_choices
            .insert(effect_index, Some(branch_index));
        if let Some(state) = pending.resolution_branch.as_mut() {
            state.selected_branch = Some(branch_index);
        }
        let mut ev = vec![ev_log(format!(
            "P{} chooses: {}.",
            pending.deciding_player, branch.label
        ))];
        match branch.cost {
            ResolutionCost::None => {
                return self.complete_parked_resolution(pending.item, Some(effect_index), ev);
            }
            ResolutionCost::Mana(mana_cost) => {
                pending.choice_kind = rv1::ChoiceKind::ManaPayment;
                pending.prompt = format!("Pay {}?", mana_cost);
                pending.mana_payment = Some(PendingManaPayment {
                    target_spell_id: 0,
                    generic_mana_cost: 0,
                    mana_cost,
                    undo_history_start: self.state.undoable_mana_abilities.len(),
                });
                self.state.pending_resolution = Some(pending);
                ev.push(
                    self.resolution_payment_choice_event()
                        .expect("resolution branch payment remains parked"),
                );
            }
            ResolutionCost::DiscardCard { .. } | ResolutionCost::SacrificePermanent { .. } => {
                let is_discard = matches!(branch.cost, ResolutionCost::DiscardCard { .. });
                pending.choice_kind = if is_discard {
                    rv1::ChoiceKind::HandCards
                } else {
                    rv1::ChoiceKind::TargetObjects
                };
                pending.candidates = candidates.clone();
                pending.min = u32::from(!branch_state.optional);
                pending.max = 1;
                pending.prompt = if is_discard {
                    "Choose a card to discard, or decline.".into()
                } else {
                    "Choose a permanent to sacrifice, or decline.".into()
                };
                let candidate_card_ids = candidates
                    .iter()
                    .map(|oid| {
                        self.state
                            .objects
                            .get(oid)
                            .map(|object| object.card_id.clone())
                            .unwrap_or_default()
                    })
                    .collect::<Vec<_>>();
                ev.push(rv1::RuledEvent {
                    ev: Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(
                        rv1::ResolutionChoiceRequired {
                            deciding_player_id: pending.deciding_player,
                            source_object_id: pending.item.id,
                            prompt_text: pending.prompt.clone(),
                            choice_kind: pending.choice_kind as i32,
                            candidate_object_ids: candidates.clone(),
                            candidate_card_ids,
                            min: pending.min,
                            max: 1,
                            ordered: false,
                            candidate_names: self.object_names(&candidates),
                            candidate_server_card_ids: Vec::new(),
                            candidate_selectable: Vec::new(),
                            unique_names: false,
                            generic_mana_cost: 0,
                            payment_currently_legal: false,
                            resolution_branches: Vec::new(),
                            mana_cost: String::new(),
                        },
                    )),
                });
                self.state.pending_resolution = Some(pending);
            }
        }
        Ok(finish_with_events(self, ev))
    }

    fn finish_resolution_branch_object(
        &mut self,
        mut pending: PendingResolution,
        chosen: &[ObjectId],
    ) -> Result<RuledEventBatch, EngineError> {
        let branch_state = pending
            .resolution_branch
            .clone()
            .ok_or(EngineError::Illegal(
                "resolution branch continuation missing",
            ))?;
        let branch_index = branch_state
            .selected_branch
            .ok_or(EngineError::Illegal("resolution branch was not selected"))?;
        let branch = branch_state
            .branches
            .get(branch_index)
            .ok_or(EngineError::Illegal("resolution branch became stale"))?;
        let effect_index = pending
            .resume_effect_index
            .and_then(|next| next.checked_sub(1))
            .ok_or(EngineError::Illegal(
                "resolution branch effect index missing",
            ))?;
        if chosen.is_empty() {
            if !branch_state.optional {
                self.state.pending_resolution = Some(pending);
                return Err(EngineError::Illegal(
                    "resolution branch payment is required",
                ));
            }
            pending
                .item
                .resolution_branch_choices
                .insert(effect_index, None);
            return self.complete_parked_resolution(
                pending.item,
                Some(effect_index),
                vec![ev_log(format!(
                    "P{} declines the optional resolution choice.",
                    pending.deciding_player
                ))],
            );
        }
        let current = self.resolution_cost_candidates(pending.deciding_player, &branch.cost);
        if chosen.len() != 1 || !current.contains(&chosen[0]) {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal("resolution payment choice is stale"));
        }
        let oid = chosen[0];
        let name = object_display_name(&self.state, self.registry, oid);
        let owner = self
            .state
            .objects
            .get(&oid)
            .map(|object| object.owner)
            .ok_or(EngineError::Illegal("resolution payment object missing"))?;
        let mut ev = Vec::new();
        match &branch.cost {
            ResolutionCost::None => {
                self.state.pending_resolution = Some(pending);
                return Err(EngineError::Illegal(
                    "costless branch has no object payment",
                ));
            }
            ResolutionCost::DiscardCard { .. } => {
                resolution::move_object_to_zone(
                    &mut self.state,
                    self.registry,
                    oid,
                    Zone::Graveyard,
                    None,
                )?;
                ev.push(permanent_moved_event(
                    &self.state,
                    oid,
                    owner,
                    rv1::permanent_moved::Destination::Graveyard,
                ));
                ev.push(ev_log(format!(
                    "P{} discards {name}.",
                    pending.deciding_player
                )));
            }
            ResolutionCost::SacrificePermanent { .. } => {
                let source = self.trigger_source_snapshot(oid);
                let was_creature = self
                    .characteristics(oid)
                    .is_some_and(|characteristics| characteristics.is_creature());
                sacrifice_permanent(&mut self.state, self.registry, oid)?;
                ev.push(permanent_moved_event(
                    &self.state,
                    oid,
                    owner,
                    rv1::permanent_moved::Destination::Graveyard,
                ));
                ev.push(ev_log(format!(
                    "P{} sacrifices {name}.",
                    pending.deciding_player
                )));
                if let Some(source) = source {
                    self.fire_triggers(&[GameEvent::Dies {
                        source,
                        was_creature,
                    }]);
                }
            }
            ResolutionCost::Mana(_) => {
                self.state.pending_resolution = Some(pending);
                return Err(EngineError::Illegal("mana branch requires mana payment"));
            }
        }
        self.complete_parked_resolution(pending.item, Some(effect_index), ev)
    }

    /// Close out a parked *primitive* resolution once its choice has been applied.
    ///
    /// CR 608.2: a spell resolves its whole effect list. When the parked effect was not the last
    /// one, `resume_effect_index` says where to pick the list back up — `build_resolution_effects`
    /// re-derives it from the stack item, so nothing had to be stored across the park. Running the
    /// tail is also what emits the closing "resolves." log and seats the spell in the graveyard
    /// (CR 608.2m), which is why the `finish_*` callers do not log that themselves.
    ///
    /// Priority returns to the active player only if the tail did not park again (a second
    /// suspending effect in the same list, e.g. a hypothetical `[Scry, DiscardCards]`).
    pub(super) fn complete_parked_resolution(
        &mut self,
        item: StackItem,
        resume_effect_index: Option<u32>,
        mut ev: Vec<rv1::RuledEvent>,
    ) -> Result<RuledEventBatch, EngineError> {
        if let Some(start) = resume_effect_index {
            let (effects, spell_label) = self.build_resolution_effects(&item);
            self.run_effect_list(&item, &spell_label, effects, start as usize, &mut ev)?;
        }
        if self.state.pending_resolution.is_none() {
            // The original pass-priority call deliberately skipped SBAs while this primitive was
            // parked. Run them only after the resumed effect tail, before granting priority.
            self.apply_sbas(&mut ev)?;
        }
        if self.state.pending_resolution.is_none() {
            if let Some(i) = self.state.player_idx(self.state.active_player_id()) {
                self.state.priority_idx = i;
            }
            ev.push(ev_priority_changed(self));
        }
        Ok(finish_with_events(self, ev))
    }

    pub(super) fn park_or_finish(
        &mut self,
        item: StackItem,
        custom_key: String,
        step_no: u32,
        scratch: Vec<ObjectId>,
        step: ResolutionStep,
        events: &mut Vec<rv1::RuledEvent>,
    ) {
        let interrupt = match step {
            // CR 608.2m: this is the single point where a tier-3 resolution completes, whether it
            // ran straight through in `begin` or came back here from a later `resume`, so it is
            // where the spell takes its place beneath whatever its resolution put in the
            // graveyard — e.g. Gifts Ungiven under the two cards it puts there.
            ResolutionStep::Done => {
                seat_resolved_spell_last_in_graveyard(&mut self.state, item.id);
                return;
            }
            ResolutionStep::NeedsChoice(it) => it,
        };
        let candidate_card_ids: Vec<String> = interrupt
            .candidates
            .iter()
            .map(|o| {
                self.state
                    .objects
                    .get(o)
                    .map(|x| x.card_id.clone())
                    .unwrap_or_default()
            })
            .collect();
        let candidate_names: Vec<String> = candidate_card_ids
            .iter()
            .map(|cid| {
                self.registry
                    .get(cid)
                    .map(|d| d.name.clone())
                    .unwrap_or_else(|| cid.clone())
            })
            .collect();
        events.push(rv1::RuledEvent {
            ev: Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(
                rv1::ResolutionChoiceRequired {
                    deciding_player_id: interrupt.deciding_player,
                    source_object_id: item.id,
                    prompt_text: interrupt.prompt.clone(),
                    choice_kind: interrupt.choice_kind as i32,
                    candidate_object_ids: interrupt.candidates.clone(),
                    candidate_card_ids,
                    candidate_names,
                    min: interrupt.min,
                    max: interrupt.max,
                    ordered: interrupt.ordered,
                    unique_names: interrupt.unique_names,
                    // Populated by the server relay per-player; the engine never fills it.
                    candidate_server_card_ids: Vec::new(),
                    candidate_selectable: Vec::new(),
                    resolution_branches: Vec::new(),
                    mana_cost: String::new(),
                    generic_mana_cost: 0,
                    payment_currently_legal: false,
                },
            )),
        });
        events.push(ev_log(interrupt.prompt.clone()));
        self.state.pending_resolution = Some(PendingResolution {
            item,
            custom_key,
            step: step_no + 1,
            scratch,
            deciding_player: interrupt.deciding_player,
            candidates: interrupt.candidates,
            min: interrupt.min,
            max: interrupt.max,
            ordered: interrupt.ordered,
            unique_names: interrupt.unique_names,
            mana_payment: None,
            resolution_branch: None,
            discard: None,
            prompt: interrupt.prompt,
            choice_kind: interrupt.choice_kind,
            copy_source_object_id: 0,
            search_destination: SearchDestination::Hand,
            search_shuffle: false,
            search_reveal: false,
            // Tier-3 (CR 608): the `CardEffect` owns the whole resolution — `resolve_top_of_stack`
            // hands off before building any primitive list — so there is never a tail to resume,
            // including across the repeated re-parks of a multi-step effect like Gifts Ungiven.
            resume_effect_index: None,
        });
    }

    /// CR 701.18: the controller submitted their library search choice. Move the found card to
    /// the declared destination, optionally reveal it publicly, then optionally shuffle.
    fn finish_library_search(
        &mut self,
        pending: PendingResolution,
        chosen: &[u32],
    ) -> Result<RuledEventBatch, EngineError> {
        let controller = pending.item.controller;

        let mut ev = vec![];

        if chosen.is_empty() {
            ev.push(ev_log(format!("P{controller} finds no card.")));
            if pending.search_shuffle {
                crate::engine::shuffle_player_library_for_current_command(
                    &mut self.state,
                    controller,
                );
                ev.push(ev_log(format!("P{controller} shuffles their library.")));
            }
        } else {
            let oid = chosen[0];
            let card_name = self
                .state
                .objects
                .get(&oid)
                .and_then(|o| self.registry.get(&o.card_id))
                .map(|d| d.name.clone())
                .unwrap_or_else(|| "card".to_string());

            match pending.search_destination {
                SearchDestination::Hand => {
                    // Move to hand first, then shuffle.
                    if let Some(idx) = self.state.player_idx(controller) {
                        self.state.players[idx].library.retain(|&x| x != oid);
                        self.state.players[idx].hand.push(oid);
                    }
                    if let Some(o) = self.state.objects.get_mut(&oid) {
                        o.zone = Zone::Hand;
                    }
                    if pending.search_reveal {
                        ev.push(ev_log(format!("P{controller} reveals {card_name}.")));
                        ev.push(ev_log(format!(
                            "P{controller} puts {card_name} into their hand."
                        )));
                    } else {
                        ev.push(ev_log_private(
                            format!("P{controller} puts {card_name} into their hand."),
                            controller,
                        ));
                        ev.push(ev_log_hidden_from(
                            format!("P{controller} puts a card into their hand."),
                            controller,
                        ));
                    }
                    if pending.search_shuffle {
                        crate::engine::shuffle_player_library_for_current_command(
                            &mut self.state,
                            controller,
                        );
                        ev.push(ev_log(format!("P{controller} shuffles their library.")));
                    }
                }
                SearchDestination::TopOfLibrary => {
                    // Oracle: "then shuffle and put that card on top" — shuffle first, then put on top.
                    if let Some(idx) = self.state.player_idx(controller) {
                        self.state.players[idx].library.retain(|&x| x != oid);
                    }
                    if pending.search_shuffle {
                        crate::engine::shuffle_player_library_for_current_command(
                            &mut self.state,
                            controller,
                        );
                        ev.push(ev_log(format!("P{controller} shuffles their library.")));
                    }
                    if let Some(idx) = self.state.player_idx(controller) {
                        self.state.players[idx].library.push_front(oid);
                    }
                    if let Some(o) = self.state.objects.get_mut(&oid) {
                        o.zone = Zone::Library;
                    }
                    if pending.search_reveal {
                        ev.push(ev_log(format!("P{controller} reveals {card_name}.")));
                        ev.push(ev_log(format!(
                            "P{controller} puts {card_name} on top of their library."
                        )));
                    } else {
                        ev.push(ev_log_private(
                            format!("P{controller} puts {card_name} on top of their library."),
                            controller,
                        ));
                        ev.push(ev_log_hidden_from(
                            format!("P{controller} puts a card on top of their library."),
                            controller,
                        ));
                    }
                }
                SearchDestination::Battlefield { tapped } => {
                    let owner = self
                        .state
                        .objects
                        .get(&oid)
                        .map(|object| object.owner)
                        .ok_or(EngineError::Illegal("searched card is stale"))?;
                    let completion = BattlefieldEntryCompletion::LibrarySearch {
                        owner,
                        card_label: card_name.clone(),
                        shuffle: pending.search_shuffle,
                        resume_effect_index: pending.resume_effect_index,
                    };
                    match self.begin_battlefield_entry(
                        pending.item.clone(),
                        BattlefieldEntryEvent {
                            object_id: oid,
                            deciding_player: controller,
                            destination_controller: controller,
                            face_index: 0,
                            chosen_x: 0,
                            tapped,
                            entry_counters: BTreeMap::new(),
                            applied_effects: Vec::new(),
                        },
                        completion,
                        &mut ev,
                    ) {
                        super::replacement::BattlefieldEntryProgress::Parked => {
                            return Ok(finish_with_events(self, ev));
                        }
                        super::replacement::BattlefieldEntryProgress::Ready(entry) => {
                            self.commit_battlefield_entry(entry, None)?;
                        }
                    }
                    ev.push(ev_log(format!(
                        "P{controller} puts {card_name} onto the battlefield."
                    )));
                    ev.push(permanent_moved_event(
                        &self.state,
                        oid,
                        owner,
                        rv1::permanent_moved::Destination::Battlefield,
                    ));
                    if pending.search_shuffle {
                        crate::engine::shuffle_player_library_for_current_command(
                            &mut self.state,
                            controller,
                        );
                        ev.push(ev_log(format!("P{controller} shuffles their library.")));
                    }
                }
            }
        }

        self.complete_parked_resolution(pending.item, pending.resume_effect_index, ev)
    }

    /// Resolve a caster-chooses DiscardCards interrupt: move chosen cards from the target's hand
    /// to their graveyard, then restore priority to the active player.
    fn finish_discard_chosen(
        &mut self,
        pending: PendingResolution,
        chosen: &[u32],
    ) -> Result<RuledEventBatch, EngineError> {
        let discard = pending
            .discard
            .ok_or(EngineError::Illegal("discard continuation missing"))?;
        let card_name = self
            .registry
            .get(&pending.item.card_id)
            .and_then(|d| d.face(pending.item.face_index))
            .map(|f| f.name.to_string())
            .unwrap_or_else(|| pending.item.card_id.clone());

        let mut ev = vec![];
        for &oid in chosen {
            let owner = self
                .state
                .objects
                .get(&oid)
                .map(|o| o.owner)
                .ok_or(EngineError::Illegal("chosen card object not found"))?;
            if owner != discard.affected_player {
                return Err(EngineError::Illegal(
                    "chosen card is not owned by the affected player",
                ));
            }
            let discard_name = object_display_name(&self.state, self.registry, oid);
            move_object_to_zone(&mut self.state, self.registry, oid, Zone::Graveyard, None)?;
            ev.push(permanent_moved_event(
                &self.state,
                oid,
                owner,
                rv1::permanent_moved::Destination::Graveyard,
            ));
            ev.push(ev_log(format!(
                "P{owner} discards {discard_name} ({card_name})."
            )));
        }
        if discard.draw_after > 0 && (!discard.draw_only_if_discarded || !chosen.is_empty()) {
            resolution::zones::draw_cards_for_player(
                self,
                &mut ev,
                discard.affected_player,
                discard.draw_after,
                &card_name,
            )?;
        }
        self.complete_parked_resolution(pending.item, pending.resume_effect_index, ev)
    }
}
