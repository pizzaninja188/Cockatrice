use super::events::{
    ev_log, ev_log_hidden_from, ev_log_private, ev_priority_changed, finish_with_events,
    format_spell_targets_log, object_display_name,
};
use super::legal_actions::fill_legal;
use super::resolution::{
    permanent_moved_event, sacrifice_permanent, seat_resolved_spell_last_in_graveyard,
};
use super::targeting::{validate_ability_targets, validate_spell_targets};
use super::*;

impl GameEngine {
    pub(super) fn choose_trigger_target(
        &mut self,
        player: PlayerId,
        target_object_id: u32,
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
        let target_ref = &[rv1::TargetRef {
            object_id: target_object_id,
            damage_amount: 0,
        }];
        let validated: Result<String, EngineError> = match self.registry.get(&pending.card_id) {
            None => Err(EngineError::MissingCard(pending.card_id.clone())),
            Some(def) => {
                let name = def.name.clone();
                match def
                    .face(pending.source_face_index)
                    .expect("pending trigger captured a registry-valid face")
                    .triggered_abilities
                    .get(pending.ability_index)
                {
                    Some(ability) => {
                        validate_ability_targets(self, player, &ability.effect, target_ref)
                            .map(|()| name)
                    }
                    None => Ok(name),
                }
            }
        };
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
        let controller = pending.controller;
        let trigger_player = pending.trigger_player;
        let trigger_object = pending.trigger_object;

        let trefs = vec![target_object_id];
        let tgt_line = format_spell_targets_log(&self.state, self.registry, &trefs);

        self.state.stack.push(StackItem {
            id: virtual_id,
            controller,
            card_id: card_id.clone(),
            targets: trefs,
            ability_text: Some(ability_text.clone()),
            source_permanent_id: Some(source_id),
            source_zone_change,
            source_face_change,
            ability_index: Some(ability_index),
            is_triggered: true,
            is_copy: false,
            chosen_x: 0,
            face_index: source_face_index,
            target_damage: vec![],
            chosen_modes: vec![],
            trigger_player,
            trigger_object,
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
                targets: vec![rv1::TargetRef {
                    object_id: target_object_id,
                    damage_amount: 0,
                }],
                ability_annotation: ability_text,
                card_id: String::new(),
                is_copy: false,
                is_triggered: true,
                copy_source_object_id: 0,
                chosen_mode_indices: vec![],
                chosen_mode_labels: vec![],
            })),
        });

        // Choosing a target for the original triggered ability can itself cause target-watchers
        // to trigger. This new simultaneous group waits behind the group currently being placed.
        self.fire_triggers(&[GameEvent::TargetsChosen {
            controller,
            source: TargetingSourceKind::Ability,
            targets: vec![target_object_id],
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
        chosen: &[u32],
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

        // DiscardCards (caster-chooses): move each chosen card from the target's hand to graveyard.
        if pending.custom_key == "__discard_chosen" {
            return self.finish_discard_chosen(pending, chosen);
        }

        // CR 701.17: sacrifice choice — target player picks which qualifying permanent to lose.
        if pending.custom_key == "__sacrifice_chosen" {
            return self.finish_sacrifice_chosen(pending, chosen);
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
        let face = self
            .registry
            .get(&item.card_id)
            .and_then(|definition| definition.face(item.face_index));

        if item.chosen_modes.is_empty() {
            let effects = face.map(|f| f.spell_effect.to_vec()).unwrap_or_default();
            self.validate_copy_target_group(
                controller,
                &effects,
                chosen,
                &item.targets,
                &item.target_damage,
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
                &mode.effects,
                mode_targets,
                &chosen_mode.targets,
                &chosen_mode.target_damage,
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
        effects: &[SpellEffectKind],
        chosen: &[ObjectId],
        original: &[ObjectId],
        target_damage: &[u32],
    ) -> Result<(), EngineError> {
        for (index, &object_id) in chosen.iter().enumerate() {
            if original.get(index) == Some(&object_id) {
                continue;
            }
            let target_ref = rv1::TargetRef {
                object_id,
                damage_amount: target_damage.get(index).copied().unwrap_or(0),
            };
            validate_spell_targets(self, controller, effects, std::slice::from_ref(&target_ref))?;
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
            chosen_mode.targets = mode_targets;
        }
        copy_item.targets = chosen.to_vec();

        let copied_name = self
            .registry
            .get(&card_id)
            .and_then(|d| d.face(face_index))
            .map(|f| f.name.to_string())
            .unwrap_or_else(|| card_id.clone());

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
                    targets: chosen
                        .iter()
                        .map(|&o| rv1::TargetRef {
                            object_id: o,
                            damage_amount: 0,
                        })
                        .collect(),
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
        // Capture card_id and controller before the zone move clears transient state.
        let owner = self
            .state
            .objects
            .get(&oid)
            .map(|o| o.owner)
            .ok_or(EngineError::Illegal("sacrificed object missing"))?;
        let card_id = self
            .state
            .objects
            .get(&oid)
            .map(|o| o.card_id.clone())
            .unwrap_or_default();
        let controller = self
            .state
            .objects
            .get(&oid)
            .map(|o| o.controller)
            .ok_or(EngineError::Illegal("sacrificed object missing"))?;
        let face_index = self
            .state
            .objects
            .get(&oid)
            .map(|o| o.face_up_index)
            .unwrap_or(0);
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
            source: TriggerSourceSnapshot {
                object_id: oid,
                card_id,
                controller,
                face_index,
            },
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
            let controller = self.state.objects.get(&oid).map(|o| o.controller);
            let card_id = self.state.objects.get(&oid).map(|o| o.card_id.clone());
            let face_index = self
                .state
                .objects
                .get(&oid)
                .map(|o| o.face_up_index)
                .unwrap_or(0);
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
                if let (Some(cid), Some(ctrl)) = (card_id, controller) {
                    trigger_events.push(GameEvent::Dies {
                        source: TriggerSourceSnapshot {
                            object_id: oid,
                            card_id: cid,
                            controller: ctrl,
                            face_index,
                        },
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

    /// Display names for `oids`, in order (registry lookup, never Oracle).
    fn object_names(&self, oids: &[ObjectId]) -> Vec<String> {
        oids.iter()
            .map(|&oid| object_display_name(&self.state, self.registry, oid))
            .collect()
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
                let mix = self
                    .state
                    .seed
                    .wrapping_add(self.state.command_index.wrapping_mul(0x9E37_79B9_7F4A_7C15))
                    ^ (controller as u64);
                if let Some(idx) = self.state.player_idx(controller) {
                    crate::engine::shuffle_player_library(&mut self.state, idx, mix);
                }
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
                        let mix = self.state.seed.wrapping_add(
                            self.state.command_index.wrapping_mul(0x9E37_79B9_7F4A_7C15),
                        ) ^ (controller as u64);
                        if let Some(idx) = self.state.player_idx(controller) {
                            crate::engine::shuffle_player_library(&mut self.state, idx, mix);
                        }
                        ev.push(ev_log(format!("P{controller} shuffles their library.")));
                    }
                }
                SearchDestination::TopOfLibrary => {
                    // Oracle: "then shuffle and put that card on top" — shuffle first, then put on top.
                    if let Some(idx) = self.state.player_idx(controller) {
                        self.state.players[idx].library.retain(|&x| x != oid);
                    }
                    if pending.search_shuffle {
                        let mix = self.state.seed.wrapping_add(
                            self.state.command_index.wrapping_mul(0x9E37_79B9_7F4A_7C15),
                        ) ^ (controller as u64);
                        if let Some(idx) = self.state.player_idx(controller) {
                            crate::engine::shuffle_player_library(&mut self.state, idx, mix);
                        }
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
        let card_name = self
            .registry
            .get(&pending.item.card_id)
            .and_then(|d| d.face(pending.item.face_index))
            .map(|f| f.name.to_string())
            .unwrap_or_else(|| pending.item.card_id.clone());

        let mut ev = vec![];
        for &oid in chosen {
            // Owner is the target player (the one whose hand these cards came from).
            let owner = self
                .state
                .objects
                .get(&oid)
                .map(|o| o.owner)
                .ok_or(EngineError::Illegal("chosen card object not found"))?;
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
        self.complete_parked_resolution(pending.item, pending.resume_effect_index, ev)
    }
}
