use super::*;

impl GameEngine {
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
    pub(super) fn finish_scry(
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
    pub(super) fn finish_look_choose_bottom(
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
}
