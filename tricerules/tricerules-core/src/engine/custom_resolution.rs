use super::events::{
    ev_log, ev_log_hidden_from, ev_log_private, ev_priority_changed, finish_with_events,
    format_spell_targets_log, object_display_name,
};
use super::legal_actions::fill_legal;
use super::targeting::validate_effect_targets;
use super::*;

impl GameEngine {
    pub(super) fn choose_trigger_target(
        &mut self,
        player: PlayerId,
        target_object_id: u32,
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

        let def = self
            .registry
            .get(&pending.card_id)
            .ok_or_else(|| EngineError::MissingCard(pending.card_id.clone()))?;

        let effect = def
            .triggered_abilities
            .get(pending.ability_index)
            .map(|a| &a.effect);

        let target_ref = &[rv1::TargetRef {
            object_id: target_object_id,
            damage_amount: 0,
        }];
        if let Some(kind) = effect {
            validate_effect_targets(&self.state, self.registry, player, kind, target_ref)?;
        }

        let virtual_id = self.state.next_object_id;
        self.state.next_object_id += 1;

        let ability_text = pending.ability_text.clone();
        let card_name = def.name.clone();
        let card_id = pending.card_id.clone();
        let source_id = pending.source_permanent_id;
        let ability_index = pending.ability_index;
        let controller = pending.controller;

        let trefs = vec![target_object_id];
        let tgt_line = format_spell_targets_log(&self.state, self.registry, &trefs);

        self.state.stack.push(StackItem {
            id: virtual_id,
            controller,
            card_id: card_id.clone(),
            targets: trefs,
            ability_text: Some(ability_text.clone()),
            source_permanent_id: Some(source_id),
            ability_index: Some(ability_index),
            is_triggered: true,
            is_copy: false,
            chosen_x: 0,
            face_index: 0,
            target_damage: vec![],
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
                copy_source_object_id: 0,
            })),
        });

        if let Some(next) = self.state.pending_triggers.front() {
            let next_name = self
                .registry
                .get(&next.card_id)
                .map(|d| d.name.clone())
                .unwrap_or_else(|| next.card_id.clone());
            batch.events.push(rv1::RuledEvent {
                ev: Some(rv1::ruled_event::Ev::TriggerNeedsTarget(
                    rv1::TriggerNeedsTarget {
                        source_permanent_id: next.source_permanent_id,
                        ability_index: next.ability_index as u32,
                        ability_text: next.ability_text.clone(),
                        controller_player_id: next.controller,
                    },
                )),
            });
            batch.events.push(ev_log(format!(
                "Triggered: {next_name} — choose a target for: {}",
                next.ability_text
            )));
        } else {
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

        // DiscardCards (caster-chooses): move each chosen card from the target's hand to graveyard.
        if pending.custom_key == "__discard_chosen" {
            return self.finish_discard_chosen(pending, chosen);
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

        let mut copy_item = pending.item;
        copy_item.targets = chosen.to_vec();

        let copied_name = self
            .registry
            .get(&card_id)
            .and_then(|d| d.face(face_index))
            .map(|f| f.name.to_string())
            .unwrap_or_else(|| card_id.clone());

        self.state.stack.push(copy_item);
        self.state.passes_since_stack_change = 0;

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
                    copy_source_object_id,
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
            ResolutionStep::Done => return,
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
                    choice_kind: interrupt.choice_kind.as_proto(),
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
            choice_kind: interrupt.choice_kind.as_proto(),
            copy_source_object_id: 0,
            search_destination: SearchDestination::Hand,
            search_shuffle: false,
            search_reveal: false,
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
        let face_index = pending.item.face_index;
        let spell_name = self
            .registry
            .get(&pending.item.card_id)
            .and_then(|d| d.face(face_index))
            .map(|f| f.name.to_string())
            .unwrap_or_else(|| pending.item.card_id.clone());

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

        ev.push(ev_log(format!("{spell_name} resolves.")));

        if let Some(i) = self.state.player_idx(self.state.active_player_id()) {
            self.state.priority_idx = i;
        }
        ev.push(ev_priority_changed(self));

        Ok(finish_with_events(self, ev))
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
            move_object_to_zone(&mut self.state, oid, Zone::Graveyard)?;
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
        ev.push(ev_log(format!("{card_name} resolves.")));

        if let Some(i) = self.state.player_idx(self.state.active_player_id()) {
            self.state.priority_idx = i;
        }
        ev.push(ev_priority_changed(self));

        Ok(finish_with_events(self, ev))
    }
}
