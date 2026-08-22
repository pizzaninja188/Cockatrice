//! Parked resolution and engine-authored choice coordination.
//!
//! This module owns command routing, shared validation, and the common park/resume boundary.
//! Domain modules own the mechanics that apply an accepted choice; they must restore the
//! outstanding [`PendingResolution`] before returning an error so rejected commands are atomic.

use super::events::{
    ev_log, ev_log_hidden_from, ev_log_private, ev_priority_changed, finish_with_events,
    format_spell_targets_log, object_display_name,
};
use super::legal_actions::fill_legal;
use super::resolution::{
    counter_stack_spell, move_object_to_zone, permanent_moved_event,
    permanent_moved_event_with_library_position, sacrifice_permanent,
    seat_resolved_spell_last_in_graveyard,
};
use super::targeting::{
    capture_stack_target, object_matches_mass_filter, validate_ability_targets_with_context,
    validate_spell_targets, TargetSourceIdentity,
};
use super::*;

mod branches;
mod copy_choices;
mod hand_choice;
mod library_order;
mod library_search;
mod manifest_dread;
mod sacrifice_choices;
mod trigger_choices;

impl GameEngine {
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
        let (step, scratch, drawn_players) = {
            let mut ctx = ResolutionCtx::new(
                &mut self.state,
                self.registry,
                events,
                controller,
                0,
                Vec::new(),
            );
            let r = effect.begin(&mut ctx);
            let drawn_players = ctx.take_drawn_players();
            (r, ctx.scratch, drawn_players)
        };
        for drawer in drawn_players {
            self.fire_card_drawn(drawer);
        }
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
        if matches!(
            pending.continuation,
            ResolutionContinuation::AuthoredBranch {
                branch: PendingResolutionBranch {
                    stage: PendingResolutionBranchStage::Selecting,
                    ..
                },
                ..
            }
        ) {
            return self.select_resolution_branch(pending, answer, decision);
        }
        if let Some(payment) = pending.continuation.mana_payment().cloned() {
            return self.finish_resolution_mana_payment(pending, payment, answer, decision, player);
        }
        if decision != rv1::ResolutionChoiceDecision::Unspecified {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal(
                "object resolution choice must leave decision unspecified",
            ));
        }
        let chosen = answer.chosen_object_ids.as_slice();
        let n = chosen.len() as u32;
        if n < pending.presentation.min || n > pending.presentation.max {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal("wrong number of cards chosen"));
        }
        let mut seen = HashSet::new();
        for &oid in chosen {
            if !pending.presentation.candidates.contains(&oid) || !seen.insert(oid) {
                self.state.pending_resolution = Some(pending);
                return Err(EngineError::Illegal("invalid resolution choice"));
            }
        }
        if pending.presentation.unique_names {
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

        match &pending.continuation {
            ResolutionContinuation::CopyTargets { .. } => {
                return self.finish_copy_target_choice(pending, chosen);
            }
            ResolutionContinuation::SearchLibrary { .. } => {
                return self.finish_library_search(pending, chosen);
            }
            ResolutionContinuation::LibraryPartition { .. } => {
                return self.finish_library_partition(pending, chosen);
            }
            ResolutionContinuation::LibraryLook { .. } => {
                return self.finish_look_choose_bottom(pending, chosen);
            }
            ResolutionContinuation::ManifestDread { .. } => {
                return self.finish_manifest_dread(pending, chosen[0]);
            }
            ResolutionContinuation::HandChoice { .. } => {
                return self.finish_hand_choice(pending, chosen);
            }
            ResolutionContinuation::Sacrifice { .. } => {
                return self.finish_sacrifice_chosen(pending, chosen);
            }
            ResolutionContinuation::AuthoredBranch { .. } => {
                return self.finish_resolution_branch_object(pending, chosen);
            }
            ResolutionContinuation::EntryCopySource { .. } => {
                return self.finish_entry_copy_source_choice(pending, chosen);
            }
            ResolutionContinuation::EntryReplacement { .. } => {
                return self.finish_battlefield_entry_replacement_choice(pending, chosen[0]);
            }
            ResolutionContinuation::DamageReplacement { .. } => {
                return self.finish_damage_prevention_choice(pending, chosen[0]);
            }
            ResolutionContinuation::LegendKeep => {
                return self.finish_legend_sba_choice(pending, chosen);
            }
            ResolutionContinuation::Custom { .. } => {}
            ResolutionContinuation::ManaPayment { .. } => unreachable!("handled above"),
        }

        let (key, controller, item, step_no, scratch) = match &pending.continuation {
            ResolutionContinuation::Custom {
                stack,
                key,
                step,
                scratch,
            } => (
                key.clone(),
                stack.item.controller,
                stack.item.clone(),
                *step,
                scratch.clone(),
            ),
            _ => unreachable!("non-custom continuations return above"),
        };
        let effect = match custom::lookup(&key) {
            Some(e) => e,
            None => {
                self.state.pending_resolution = Some(pending);
                return Err(EngineError::MissingCard(key));
            }
        };
        let choice = ResolutionChoice {
            object_ids: chosen.to_vec(),
        };

        let mut ev = vec![];
        let (step, scratch, drawn_players) = {
            let mut ctx = ResolutionCtx::new(
                &mut self.state,
                self.registry,
                &mut ev,
                controller,
                step_no,
                scratch,
            );
            let r = effect.resume(&mut ctx, &choice);
            let drawn_players = ctx.take_drawn_players();
            (r, ctx.scratch, drawn_players)
        };
        for drawer in drawn_players {
            self.fire_card_drawn(drawer);
        }
        self.park_or_finish(item, key, step_no, scratch, step, &mut ev);

        if self.state.pending_resolution.is_none() {
            if let Some(i) = self.state.player_idx(self.state.active_player_id()) {
                self.state.priority_idx = i;
            }
            ev.push(ev_priority_changed(self));
        }
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
                    candidate_selectable: Vec::new(),
                    resolution_branches: Vec::new(),
                    mana_cost: String::new(),
                    generic_mana_cost: 0,
                    payment_currently_legal: false,
                    reveal_audience: 0,
                    revealed_zone_owner_player_id: None,
                },
            )),
        });
        events.push(ev_log(interrupt.prompt.clone()));
        self.state.pending_resolution = Some(PendingResolution {
            deciding_player: interrupt.deciding_player,
            presentation: PendingResolutionPresentation {
                source_object_id: item.id,
                candidates: interrupt.candidates,
                min: interrupt.min,
                max: interrupt.max,
                ordered: interrupt.ordered,
                unique_names: interrupt.unique_names,
                prompt: interrupt.prompt,
                choice_kind: interrupt.choice_kind,
            },
            continuation: ResolutionContinuation::Custom {
                stack: ParkedStackResolution::new(item),
                key: custom_key,
                step: step_no + 1,
                scratch,
            },
        });
    }
}
