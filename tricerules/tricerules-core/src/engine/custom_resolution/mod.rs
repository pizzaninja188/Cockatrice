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
    counter_stack_object, counter_stack_object_ref, move_object_to_zone, permanent_moved_event,
    permanent_moved_event_with_library_position, put_permanent_in_graveyard, sacrifice_permanent,
    seat_resolved_spell_last_in_graveyard,
};
use super::targeting::{
    capture_stack_target, object_matches_mass_filter, validate_ability_targets_with_context,
    validate_spell_targets, TargetSourceIdentity,
};
use super::*;

mod attacking_tokens;
mod branches;
mod copy_choices;
mod explore;
mod hand_choice;
mod library_order;
mod library_search;
mod manifest_dread;
mod sacrifice_choices;
mod trigger_choices;
mod ward;

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
        let (step, scratch, drawn_players, library_searches) = {
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
            let library_searches = ctx.take_library_searches();
            (r, ctx.scratch, drawn_players, library_searches)
        };
        for drawer in drawn_players {
            self.fire_card_drawn(drawer);
        }
        for (searcher, library_owner) in library_searches {
            self.fire_triggers(&[GameEvent::LibrarySearched {
                searcher,
                library_owner,
            }]);
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
        if let Some(super::replacement::PendingReplacementEvent::BattlefieldEntry(entry)) =
            &self.state.pending_replacement_event
        {
            if let BattlefieldEntryCompletion::ZoneEntryBatch(batch) = &entry.completion {
                if !self.zone_entry_batch_current(batch) {
                    return Err(EngineError::Illegal("graveyard entry cohort became stale"));
                }
            }
        }
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
            ResolutionContinuation::EntryCost { .. }
        ) {
            return self.finish_entry_cost_choice(pending, answer, decision);
        }
        if matches!(
            pending.continuation,
            ResolutionContinuation::SagaReadAhead { .. }
        ) {
            return self.finish_saga_read_ahead_choice(pending, answer, decision);
        }
        if matches!(
            pending.continuation,
            ResolutionContinuation::SearchZoneScope { .. }
        ) {
            return self.finish_search_zone_scope(pending, answer, decision);
        }
        if matches!(
            pending.continuation,
            ResolutionContinuation::OptionalSearch { .. }
        ) {
            return self.finish_optional_search_choice(pending, answer, decision);
        }
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
        if matches!(
            pending.continuation,
            ResolutionContinuation::OwnerLibraryPlacement { .. }
        ) {
            return self.finish_owner_library_placement(pending, answer, decision);
        }
        if let Some(payment) = pending.continuation.mana_payment().cloned() {
            return self.finish_resolution_mana_payment(pending, payment, answer, decision, player);
        }
        if matches!(
            pending.continuation,
            ResolutionContinuation::SiegeCast { .. }
        ) {
            return self.finish_siege_cast_choice(pending, answer, decision);
        }
        if matches!(
            pending.continuation,
            ResolutionContinuation::AttackingTokenDefenders { .. }
        ) {
            return self.finish_attacking_token_defender_choice(pending, answer, decision);
        }
        if matches!(
            pending.continuation,
            ResolutionContinuation::WardPayment {
                ward: PendingWardPayment {
                    stage: PendingWardPaymentStage::Discard { .. },
                    ..
                },
                ..
            }
        ) {
            if decision == rv1::ResolutionChoiceDecision::Decline {
                if !answer.chosen_object_ids.is_empty() {
                    self.state.pending_resolution = Some(pending);
                    return Err(EngineError::Illegal(
                        "declining Ward cannot include a discard",
                    ));
                }
                return self.finish_ward_discard(pending, &[]);
            }
            if decision != rv1::ResolutionChoiceDecision::Unspecified {
                self.state.pending_resolution = Some(pending);
                return Err(EngineError::Illegal(
                    "Ward discard requires a card choice or decline",
                ));
            }
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
            ResolutionContinuation::AuraReturn { .. } => {
                return self.finish_aura_return(pending, chosen[0]);
            }
            ResolutionContinuation::CopyTargets { .. } => {
                return self.finish_copy_target_choice(pending, chosen);
            }
            ResolutionContinuation::SearchLibrary { .. } => {
                return self.finish_library_search(pending, chosen);
            }
            ResolutionContinuation::SearchZoneScope { .. } => {
                unreachable!("search-zone branch handled before object-choice validation")
            }
            ResolutionContinuation::OptionalSearch { .. } => {
                unreachable!("optional-search branch handled before object-choice validation")
            }
            ResolutionContinuation::OwnerLibraryPlacement { .. } => {
                unreachable!("owner placement branch handled before object-choice validation")
            }
            ResolutionContinuation::LibraryPartition { .. } => {
                return self.finish_library_partition(pending, chosen);
            }
            ResolutionContinuation::LibraryLook { .. } => {
                return self.finish_look_choose_bottom(pending, chosen);
            }
            ResolutionContinuation::Explore { .. } => {
                return self.finish_explore_choice(pending, chosen);
            }
            ResolutionContinuation::ManifestDread { .. } => {
                return self.finish_manifest_dread(pending, chosen[0]);
            }
            ResolutionContinuation::HandChoice { .. } => {
                return self.finish_hand_choice(pending, chosen);
            }
            ResolutionContinuation::PlayerSetDiscard { .. } => {
                return self.finish_player_set_discard_choice(pending, chosen);
            }
            ResolutionContinuation::GraveyardChoice { .. } => {
                return self.finish_graveyard_choice(pending, chosen);
            }
            ResolutionContinuation::Sacrifice { .. } => {
                return self.finish_sacrifice_chosen(pending, chosen);
            }
            ResolutionContinuation::AuthoredBranch { .. } => {
                return self.finish_resolution_branch_object(pending, chosen);
            }
            ResolutionContinuation::PermanentChoice { .. } => {
                return self.finish_permanent_choice(pending, chosen);
            }
            ResolutionContinuation::BeholdChoice { .. } => {
                return self.finish_behold_choice(pending, chosen);
            }
            ResolutionContinuation::AmassChoice { .. } => {
                return self.finish_amass_choice(pending, chosen[0]);
            }
            ResolutionContinuation::WardPayment { .. } => {
                return self.finish_ward_discard(pending, chosen);
            }
            ResolutionContinuation::EntryCopySource { .. } => {
                return self.finish_entry_copy_source_choice(pending, chosen);
            }
            ResolutionContinuation::Populate { .. } => {
                return self.finish_populate_choice(pending, chosen[0]);
            }
            ResolutionContinuation::Blight { .. } => {
                return self.finish_blight_choice(pending, chosen[0]);
            }
            ResolutionContinuation::EntryReplacement { .. } => {
                return self.finish_battlefield_entry_replacement_choice(pending, chosen[0]);
            }
            ResolutionContinuation::EntryCost { .. } => {
                unreachable!("entry-cost branch handled before object-choice validation")
            }
            ResolutionContinuation::SagaReadAhead { .. } => {
                unreachable!("read-ahead branch handled before object-choice validation")
            }
            ResolutionContinuation::DamageReplacement { .. } => {
                return self.finish_damage_prevention_choice(pending, chosen[0]);
            }
            ResolutionContinuation::LegendKeep => {
                return self.finish_legend_sba_choice(pending, chosen);
            }
            ResolutionContinuation::BattleProtector { .. } => {
                return self.finish_battle_protector_choice(pending, chosen[0] as PlayerId);
            }
            ResolutionContinuation::SiegeCast { .. } => {
                unreachable!("Siege-cast branch handled before object-choice validation")
            }
            ResolutionContinuation::AttackingTokenDefenders { .. } => {
                unreachable!("attacking-token branch handled before object-choice validation")
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
        let (step, scratch, drawn_players, library_searches) = {
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
            let library_searches = ctx.take_library_searches();
            (r, ctx.scratch, drawn_players, library_searches)
        };
        for drawer in drawn_players {
            self.fire_card_drawn(drawer);
        }
        for (searcher, library_owner) in library_searches {
            self.fire_triggers(&[GameEvent::LibrarySearched {
                searcher,
                library_owner,
            }]);
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

    fn finish_aura_return(
        &mut self,
        pending: PendingResolution,
        chosen: ObjectId,
    ) -> Result<RuledEventBatch, EngineError> {
        let ResolutionContinuation::AuraReturn { stack, exiled } = &pending.continuation else {
            unreachable!("Aura return continuation routed by caller")
        };
        let stack = stack.clone();
        let exiled = *exiled;
        let generation = self
            .state
            .zone_change_generation
            .get(&exiled.object_id)
            .copied()
            .unwrap_or(0);
        let Some(object) = self.state.objects.get(&exiled.object_id) else {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal("returning Aura no longer exists"));
        };
        if object.zone != Zone::Exile || generation != exiled.zone_change_generation {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal("returning Aura choice became stale"));
        }
        let owner = object.owner;
        let Some(filter) = self.effective_face(exiled.object_id).and_then(|face| {
            face.spell_effect.iter().find_map(|effect| match effect {
                SpellEffectKind::AuraAttach { target } => Some(target.clone()),
                _ => None,
            })
        }) else {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal(
                "returning object is no longer an Aura",
            ));
        };
        let recipient = match pending.presentation.choice_kind {
            rv1::ChoiceKind::AuraPermanent => AttachmentRecipient::Object(chosen),
            rv1::ChoiceKind::AuraPlayer => AttachmentRecipient::Player(chosen as PlayerId),
            _ => unreachable!("Aura return uses a typed choice kind"),
        };
        if !super::targeting::attachment_filter_legal(
            self,
            &filter,
            recipient,
            exiled.object_id,
            owner,
        ) {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal("Aura recipient is no longer legal"));
        }

        let label = object_display_name(&self.state, self.registry, exiled.object_id);
        let entry = BattlefieldEntryEvent {
            object_id: exiled.object_id,
            deciding_player: owner,
            destination_controller: owner,
            battle_protector: None,
            face_index: 0,
            unlock_room_door: None,
            chosen_x: 0,
            cast_cost_receipts: Vec::new(),
            player_life_snapshot: self.player_life_snapshot(),
            tapped: false,
            set_types: None,
            entry_counters: BTreeMap::new(),
            applied_effects: Vec::new(),
        };
        let resume_original_stack = stack.is_some();
        let item = stack
            .as_ref()
            .map(|parked| parked.item.clone())
            .unwrap_or_else(|| self.observer_return_item(exiled.object_id, owner));
        let mut events = Vec::new();
        let entry = match self.begin_battlefield_entry(
            item,
            entry,
            BattlefieldEntryCompletion::ObserverReturn {
                owner,
                object_label: label.clone(),
                attached_to: Some(recipient),
                resume_original_stack,
            },
            &mut events,
        ) {
            super::replacement::BattlefieldEntryProgress::Parked => {
                if let (Some(resume), Some(next_pending)) =
                    (stack.as_ref(), self.state.pending_resolution.as_mut())
                {
                    if let Some(parked) = next_pending.continuation.stack_mut() {
                        parked.resume_effect_index = resume.resume_effect_index;
                        parked.previous_result = resume.previous_result.clone();
                    }
                }
                return Ok(finish_with_events(self, events));
            }
            super::replacement::BattlefieldEntryProgress::Ready(entry) => entry,
        };
        self.commit_battlefield_entry_state(entry, Some(recipient))?;
        self.fire_triggers(&[GameEvent::EntersBattlefield {
            object_id: exiled.object_id,
            chosen_x: 0,
        }]);
        events.extend([
            permanent_moved_event(
                &self.state,
                exiled.object_id,
                owner,
                rv1::permanent_moved::Destination::Battlefield,
            ),
            rv1::RuledEvent {
                ev: Some(rv1::ruled_event::Ev::AuraAttached(rv1::AuraAttached {
                    aura_object_id: exiled.object_id,
                    attachment_recipient: Some(attachment_recipient_proto(recipient)),
                })),
            },
            ev_log(format!("{label} returns attached to its chosen recipient.")),
        ]);
        if self.drain_immediate_observer_actions(stack.clone(), &mut events)? {
            return Ok(finish_with_events(self, events));
        }
        if let Some(stack) = stack {
            return self.complete_parked_resolution_with_previous(
                stack.item,
                stack.resume_effect_index,
                stack.previous_result,
                events,
            );
        }
        self.apply_sbas(&mut events)?;
        if let Some(index) = self.state.player_idx(self.state.active_player_id()) {
            self.state.priority_idx = index;
        }
        events.push(ev_priority_changed(self));
        Ok(finish_with_events(self, events))
    }

    fn finish_battle_protector_choice(
        &mut self,
        pending: PendingResolution,
        protector: PlayerId,
    ) -> Result<RuledEventBatch, EngineError> {
        let ResolutionContinuation::BattleProtector { stack } = &pending.continuation else {
            unreachable!("Battle protector continuation routed by caller")
        };
        let _stack = stack;
        let Some(pending_event) = self.state.pending_replacement_event.take() else {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal("Battle protector choice became stale"));
        };
        let mut entry = match pending_event {
            super::replacement::PendingReplacementEvent::BattlefieldEntry(entry) => *entry,
            other => {
                self.state.pending_replacement_event = Some(other);
                self.state.pending_resolution = Some(pending);
                return Err(EngineError::Illegal("Battle protector choice became stale"));
            }
        };
        let battle_id = entry.event.object_id;
        if !self
            .state
            .are_opponents(entry.event.destination_controller, protector)
            || !self
                .characteristics(battle_id)
                .is_some_and(|value| value.has_type("Battle"))
        {
            self.state.pending_replacement_event = Some(
                super::replacement::PendingReplacementEvent::BattlefieldEntry(Box::new(entry)),
            );
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal("Battle protector choice became stale"));
        }
        entry.event.battle_protector = Some(protector);
        let events = vec![ev_log(format!(
            "P{} chooses P{protector} to protect Battle object {battle_id}.",
            pending.deciding_player
        ))];
        self.complete_pending_battlefield_entry(pending, entry.event, entry.completion, events)
    }

    fn finish_siege_cast_choice(
        &mut self,
        pending: PendingResolution,
        answer: &rv1::SubmitResolutionChoice,
        decision: rv1::ResolutionChoiceDecision,
    ) -> Result<RuledEventBatch, EngineError> {
        let ResolutionContinuation::SiegeCast {
            stack,
            exiled,
            face_index,
        } = &pending.continuation
        else {
            unreachable!("Siege cast continuation routed by caller")
        };
        let stack = stack.clone();
        let exiled = *exiled;
        let face_index = *face_index;
        let mut events = Vec::new();
        match decision {
            rv1::ResolutionChoiceDecision::Decline => {
                if answer.cast_spell.is_some() || !answer.chosen_object_ids.is_empty() {
                    self.state.pending_resolution = Some(pending);
                    return Err(EngineError::Illegal(
                        "declining a Siege cast cannot include an announcement",
                    ));
                }
                events.push(ev_log(format!(
                    "P{} declines to cast the defeated Siege.",
                    pending.deciding_player
                )));
            }
            rv1::ResolutionChoiceDecision::CastTransformed => {
                let Some(cast) = answer.cast_spell.as_ref() else {
                    self.state.pending_resolution = Some(pending);
                    return Err(EngineError::Illegal(
                        "accepting a Siege cast requires a spell announcement",
                    ));
                };
                if let Err(error) = self.cast_siege_defeat_offer(
                    pending.deciding_player,
                    cast,
                    exiled,
                    face_index,
                    &mut events,
                ) {
                    self.state.pending_resolution = Some(pending);
                    return Err(error);
                }
            }
            _ => {
                self.state.pending_resolution = Some(pending);
                return Err(EngineError::Illegal(
                    "Siege cast choice requires cast or decline",
                ));
            }
        }
        self.complete_parked_resolution_with_previous(
            stack.item,
            stack.resume_effect_index,
            stack.previous_result,
            events,
        )
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
        ev: Vec<rv1::RuledEvent>,
    ) -> Result<RuledEventBatch, EngineError> {
        self.complete_parked_resolution_with_previous(
            item,
            resume_effect_index,
            EffectResult::default(),
            ev,
        )
    }

    fn finish_permanent_choice(
        &mut self,
        pending: PendingResolution,
        chosen: &[ObjectId],
    ) -> Result<RuledEventBatch, EngineError> {
        let (stack, candidate_generations) = match &pending.continuation {
            ResolutionContinuation::PermanentChoice {
                stack,
                candidate_generations,
            } => (stack.clone(), candidate_generations.clone()),
            _ => unreachable!("permanent-choice continuation"),
        };
        let mut produced_objects = Vec::with_capacity(chosen.len());
        for oid in chosen {
            let Some(expected_generation) = candidate_generations
                .iter()
                .find_map(|(candidate, generation)| (candidate == oid).then_some(*generation))
            else {
                self.state.pending_resolution = Some(pending);
                return Err(EngineError::Illegal("invalid permanent choice"));
            };
            let current_generation = self
                .state
                .zone_change_generation
                .get(oid)
                .copied()
                .unwrap_or(0);
            if current_generation != expected_generation
                || !self
                    .state
                    .objects
                    .get(oid)
                    .is_some_and(|object| object.zone == Zone::Battlefield)
            {
                self.state.pending_resolution = Some(pending);
                return Err(EngineError::Illegal("stale permanent choice"));
            }
            produced_objects.push(TriggerObjectRef {
                object_id: *oid,
                zone_change_generation: expected_generation,
                controller_at_event: self
                    .characteristics(*oid)
                    .map(|characteristics| characteristics.controller)
                    .unwrap_or(pending.deciding_player),
            });
        }
        let names = chosen
            .iter()
            .map(|oid| object_display_name(&self.state, self.registry, *oid))
            .collect::<Vec<_>>();
        let events = vec![ev_log(format!(
            "P{} chooses {}.",
            pending.deciding_player,
            names.join(", ")
        ))];
        self.complete_parked_resolution_with_previous(
            stack.item,
            stack.resume_effect_index,
            EffectResult {
                produced_objects,
                ..Default::default()
            },
            events,
        )
    }

    fn finish_behold_choice(
        &mut self,
        pending: PendingResolution,
        chosen: &[ObjectId],
    ) -> Result<RuledEventBatch, EngineError> {
        let (stack, candidate_generations, hand_candidates, hand_filter, permanent_filter) =
            match &pending.continuation {
                ResolutionContinuation::BeholdChoice {
                    stack,
                    candidate_generations,
                    hand_candidates,
                    hand_filter,
                    permanent_filter,
                } => (
                    stack.clone(),
                    candidate_generations.clone(),
                    hand_candidates.clone(),
                    hand_filter.clone(),
                    permanent_filter.clone(),
                ),
                _ => unreachable!("behold-choice continuation"),
            };
        let effect_index = stack
            .resume_effect_index
            .and_then(|next| next.checked_sub(1))
            .ok_or(EngineError::Illegal("Behold effect index missing"))?;
        let mut item = stack.item;
        let mut events = Vec::new();

        if chosen.is_empty() {
            item.resolution_branch_choices.insert(effect_index, None);
            events.push(ev_log(format!(
                "P{} declines to behold.",
                pending.deciding_player
            )));
        } else {
            let oid = chosen[0];
            let Some(expected_generation) = candidate_generations
                .iter()
                .find_map(|(candidate, generation)| (*candidate == oid).then_some(*generation))
            else {
                self.state.pending_resolution = Some(pending);
                return Err(EngineError::Illegal("invalid Behold choice"));
            };
            let current_generation = self
                .state
                .zone_change_generation
                .get(&oid)
                .copied()
                .unwrap_or(0);
            let is_hand_candidate = hand_candidates.contains(&oid);
            let is_current = current_generation == expected_generation
                && if is_hand_candidate {
                    self.state.objects.get(&oid).is_some_and(|object| {
                        object.owner == pending.deciding_player
                            && object.zone == Zone::Hand
                            && resolution::library_card_matches_filter(
                                &self.state,
                                self.registry,
                                oid,
                                Some(&hand_filter),
                            )
                    })
                } else {
                    let source = TargetSourceIdentity::for_stack_item(self, &item);
                    targeting::permanent_choice_filter_legal(
                        self,
                        &permanent_filter,
                        oid,
                        pending.deciding_player,
                        source,
                        item.trigger_context,
                    )
                };
            if !is_current {
                self.state.pending_resolution = Some(pending);
                return Err(EngineError::Illegal(
                    "Behold choice became stale or no longer matches",
                ));
            }
            let name = object_display_name(&self.state, self.registry, oid);
            if is_hand_candidate {
                let card_id = self.state.objects[&oid].card_id.clone();
                events.push(rv1::RuledEvent {
                    ev: Some(rv1::ruled_event::Ev::CardsRevealed(rv1::CardsRevealed {
                        zone_owner_player_id: pending.deciding_player,
                        source_zone: rv1::ChoiceCandidateSourceZone::Hand as i32,
                        cards: vec![rv1::RevealedCard {
                            object_id: oid,
                            zone_change_generation: current_generation,
                            card_id,
                            card_name: name.clone(),
                        }],
                    })),
                });
                events.push(ev_log(format!(
                    "P{} reveals {name}.",
                    pending.deciding_player
                )));
            } else {
                events.push(ev_log(format!(
                    "P{} beholds {name}.",
                    pending.deciding_player
                )));
            }
            item.resolution_branch_choices.insert(effect_index, Some(0));
        }

        self.complete_parked_resolution_with_previous(
            item,
            Some(effect_index),
            stack.previous_result,
            events,
        )
    }

    pub(super) fn complete_parked_resolution_with_previous(
        &mut self,
        item: StackItem,
        resume_effect_index: Option<u32>,
        previous_result: EffectResult,
        mut ev: Vec<rv1::RuledEvent>,
    ) -> Result<RuledEventBatch, EngineError> {
        if let Some(start) = resume_effect_index {
            let (effects, spell_label) = self.build_resolution_effects(&item);
            self.run_effect_list_with_previous(
                &item,
                &spell_label,
                effects,
                start as usize,
                previous_result,
                &mut ev,
            )?;
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
            // CR 608.2n: this is the single point where a tier-3 resolution completes, whether it
            // ran straight through in `begin` or came back here from a later `resume`, so it is
            // where the spell takes its place beneath whatever its resolution put in the
            // graveyard — e.g. Gifts Ungiven under the two cards it puts there.
            ResolutionStep::Done => {
                super::resolution::finish_deferred_graveyard_entry(&mut self.state, &item);
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
                    candidate_source_zones: Vec::new(),
                    combat_defender_options: Vec::new(),
                    waterbend: false,
                    selection_slots: Vec::new(),
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
