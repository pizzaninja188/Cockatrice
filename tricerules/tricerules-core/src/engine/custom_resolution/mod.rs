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
    counter_stack_spell, move_object_to_zone, permanent_moved_event, sacrifice_permanent,
    seat_resolved_spell_last_in_graveyard,
};
use super::targeting::{
    capture_stack_target, object_matches_mass_filter, validate_ability_targets_with_context,
    validate_spell_targets, TargetSourceIdentity,
};
use super::*;

mod branches;
mod copy_choices;
mod discard;
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
        if pending.custom_key == "__manifest_dread" {
            return self.finish_manifest_dread(pending, chosen[0]);
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
}
