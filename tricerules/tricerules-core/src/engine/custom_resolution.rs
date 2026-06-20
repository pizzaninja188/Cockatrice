use super::events::{ev_log, ev_priority_changed, finish_with_events, format_spell_targets_log};
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
                }],
                ability_annotation: ability_text,
                card_id: String::new(),
                is_copy: false,
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
            prompt: interrupt.prompt,
            choice_kind: interrupt.choice_kind.as_proto(),
        });
    }
}
