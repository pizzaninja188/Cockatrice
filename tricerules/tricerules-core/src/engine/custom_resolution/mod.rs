//! Parked resolution and engine-authored choice coordination.
//!
//! This module owns command routing, shared validation, and the common park/resume boundary.
//! Domain modules own the mechanics that apply an accepted choice; they must restore the
//! outstanding [`PendingResolution`] before returning an error so rejected commands are atomic.

use super::events::{
    ev_log, ev_log_ability, ev_log_hidden_from, ev_log_private, ev_priority_changed,
    finish_with_events, format_spell_targets_log, object_display_name,
};
use super::legal_actions::fill_legal;
use super::resolution::{
    counter_stack_object, counter_stack_object_ref, move_object_to_zone, permanent_moved_event,
    permanent_moved_event_with_library_position, put_permanent_in_graveyard, sacrifice_permanent,
    seat_resolved_spell_last_in_graveyard,
};
use super::targeting::{
    capture_stack_target, compute_spell_targets, validate_ability_targets_with_context,
    validate_spell_targets, TargetSourceIdentity,
};
use super::*;
use crate::state::PendingTargetedPlayerChoiceStage;

mod attacking_tokens;
mod branches;
mod copy_choices;
mod counter_move;
mod explore;
mod hand_choice;
mod join_forces;
mod library_order;
mod library_search;
mod manifest_dread;
mod proliferate;
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
    ) -> Result<super::resolution::ResolutionProgress, EngineError> {
        let effect = custom::lookup(&custom_key)
            .ok_or_else(|| EngineError::MissingCard(custom_key.clone()))?;
        let controller = item.controller;
        let (step, scratch, library_searches) = {
            let mut ctx = ResolutionCtx::new(
                &mut self.state,
                self.registry,
                events,
                controller,
                item.id,
                0,
                Vec::new(),
            );
            let r = effect.begin(&mut ctx);
            let library_searches = ctx.take_library_searches();
            (r, ctx.scratch, library_searches)
        };
        for (searcher, library_owner) in library_searches {
            self.fire_triggers(
                &[GameEvent::LibrarySearched {
                    searcher,
                    library_owner,
                }],
                events,
            );
        }
        self.park_or_finish(item, custom_key, 0, scratch, step, events)
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
            ResolutionContinuation::OptionalTriggeredAbility { .. }
        ) {
            return self.finish_optional_triggered_ability_choice(pending, answer, decision);
        }
        if matches!(
            &pending.continuation,
            ResolutionContinuation::TargetedPlayerPermanentChoice {
                stage: PendingTargetedPlayerChoiceStage::ChoosingDelegate { .. },
                ..
            }
        ) {
            return self.select_targeted_player_choice_delegate(pending, answer, decision);
        }
        if matches!(
            pending.continuation,
            ResolutionContinuation::DrawReplacement { .. }
        ) {
            return self.finish_draw_replacement_choice(pending, answer, decision);
        }
        if matches!(
            pending.continuation,
            ResolutionContinuation::JoinForces { .. }
        ) {
            return self.finish_join_forces_payment(pending, answer, decision, player);
        }
        if matches!(
            pending.continuation,
            ResolutionContinuation::CounterMove { .. }
        ) {
            return self.finish_counter_move_choice(pending, answer, decision);
        }
        if matches!(
            pending.continuation,
            ResolutionContinuation::EntryCost { .. }
        ) {
            return self.finish_entry_cost_choice(pending, answer, decision);
        }
        if matches!(
            pending.continuation,
            ResolutionContinuation::EntryReveal { .. }
        ) {
            return self.finish_entry_reveal_choice(pending, answer, decision);
        }
        if matches!(
            pending.continuation,
            ResolutionContinuation::EntryBasicLandType { .. }
        ) {
            return self.finish_basic_land_type_choice(pending, answer, decision);
        }
        if matches!(
            pending.continuation,
            ResolutionContinuation::EntryChooseOpponent { .. }
        ) {
            return self.finish_entry_opponent_choice(pending, answer, decision);
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
                    stage: PendingResolutionBranchStage::Selecting
                        | PendingResolutionBranchStage::ChoosingDelegate { .. },
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
        if matches!(
            pending.continuation,
            ResolutionContinuation::ChaosWarpCommander { .. }
        ) {
            return self.finish_chaos_warp_commander(pending, answer, decision);
        }
        if let Some(payment) = pending.continuation.mana_payment().cloned() {
            return self.finish_resolution_mana_payment(pending, payment, answer, decision, player);
        }
        if matches!(
            pending.continuation,
            ResolutionContinuation::SpecialCast { .. }
        ) {
            return self.finish_special_cast_choice(pending, answer, decision);
        }
        if matches!(
            pending.continuation,
            ResolutionContinuation::AttackingTokenDefenders { .. }
        ) {
            return self.finish_attacking_token_defender_choice(pending, answer, decision);
        }
        if matches!(
            pending.continuation,
            ResolutionContinuation::Proliferate { .. }
        ) {
            return self.finish_proliferate_choice(pending, answer, decision);
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
        if matches!(
            pending.continuation,
            ResolutionContinuation::PlayerSetDiscard { .. }
        ) && (!answer.chosen_player_ids.is_empty()
            || answer.selected_branch_index != 0
            || answer.cast_spell.is_some()
            || answer.chosen_combat_defender.is_some()
            || answer.payment.is_some()
            || !answer.restricted_mana.is_empty()
            || answer.spell_cast_announcement.is_some())
        {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal(
                "discard choice contains unrelated answer fields",
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
            ResolutionContinuation::OptionalTriggeredAbility { .. } => {
                unreachable!("optional triggered ability choice handled above")
            }
            ResolutionContinuation::SimultaneousEntryOrder { .. } => {
                return self.finish_entry_timestamp_order_choice(pending, chosen);
            }
            ResolutionContinuation::DiscardReplacement { .. } => {
                return self.finish_discard_replacement(pending, chosen)
            }
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
            ResolutionContinuation::ChaosWarpCommander { .. } => {
                unreachable!("commander replacement handled before object-choice validation")
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
            ResolutionContinuation::MassSacrificeGraveyardOrder { .. } => {
                return self.finish_mass_sacrifice_graveyard_order(pending, chosen);
            }
            ResolutionContinuation::AuthoredBranch { .. } => {
                return self.finish_resolution_branch_object(pending, chosen);
            }
            ResolutionContinuation::PermanentChoice { .. } => {
                return self.finish_permanent_choice(pending, chosen);
            }
            ResolutionContinuation::TargetedPlayerPermanentChoice { .. } => {
                return self.finish_targeted_player_permanent_choice(pending, chosen);
            }
            ResolutionContinuation::BeholdChoice { .. } => {
                return self.finish_behold_choice(pending, chosen);
            }
            ResolutionContinuation::Proliferate { .. } => {
                unreachable!("Proliferate branch handled before object-choice validation")
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
            ResolutionContinuation::EntryAuraRecipient { .. } => {
                return self.finish_entry_aura_recipient_choice(pending, chosen[0]);
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
            ResolutionContinuation::EntryCost { .. }
            | ResolutionContinuation::EntryReveal { .. } => {
                unreachable!("entry-cost branch handled before object-choice validation")
            }
            ResolutionContinuation::EntryBasicLandType { .. } => {
                unreachable!("basic-land-type branch handled before object-choice validation")
            }
            ResolutionContinuation::EntryChooseOpponent { .. } => {
                unreachable!("opponent branch handled before object-choice validation")
            }
            ResolutionContinuation::SagaReadAhead { .. } => {
                unreachable!("read-ahead branch handled before object-choice validation")
            }
            ResolutionContinuation::DamageReplacement { .. }
            | ResolutionContinuation::CombatDamageReplacement { .. } => {
                return self.finish_damage_prevention_choice(pending, chosen[0]);
            }
            ResolutionContinuation::ManaAbilityDamageReplacement { .. } => {
                return self.finish_damage_prevention_choice(pending, chosen[0]);
            }
            ResolutionContinuation::LegendKeep => {
                return self.finish_legend_sba_choice(pending, chosen);
            }
            ResolutionContinuation::BattleProtector { .. } => {
                return self.finish_battle_protector_choice(pending, chosen[0] as PlayerId);
            }
            ResolutionContinuation::SpecialCast { .. } => {
                unreachable!("Siege-cast branch handled before object-choice validation")
            }
            ResolutionContinuation::AttackingTokenDefenders { .. } => {
                unreachable!("attacking-token branch handled before object-choice validation")
            }
            ResolutionContinuation::DrawReplacement { .. } => {
                unreachable!("draw replacement handled above")
            }
            ResolutionContinuation::Custom { .. } => {}
            ResolutionContinuation::ManaPayment { .. }
            | ResolutionContinuation::JoinForces { .. }
            | ResolutionContinuation::CounterMove { .. } => unreachable!("handled above"),
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
        let (step, scratch, library_searches) = {
            let mut ctx = ResolutionCtx::new(
                &mut self.state,
                self.registry,
                &mut ev,
                controller,
                item.id,
                step_no,
                scratch,
            );
            let r = effect.resume(&mut ctx, &choice);
            let library_searches = ctx.take_library_searches();
            (r, ctx.scratch, library_searches)
        };
        for (searcher, library_owner) in library_searches {
            self.fire_triggers(
                &[GameEvent::LibrarySearched {
                    searcher,
                    library_owner,
                }],
                &mut ev,
            );
        }
        self.park_or_finish(item, key, step_no, scratch, step, &mut ev)?;

        if self.state.pending_resolution.is_none() {
            if let Some(i) = self.state.player_idx(self.state.active_player_id()) {
                self.state.priority_idx = i;
            }
            ev.push(ev_priority_changed(self));
        }
        Ok(finish_with_events(self, ev))
    }

    fn finish_optional_triggered_ability_choice(
        &mut self,
        pending: PendingResolution,
        answer: &rv1::SubmitResolutionChoice,
        decision: rv1::ResolutionChoiceDecision,
    ) -> Result<RuledEventBatch, EngineError> {
        let invalid_shape = !answer.chosen_object_ids.is_empty()
            || !answer.chosen_player_ids.is_empty()
            || answer.selected_branch_index != 0
            || answer.cast_spell.is_some()
            || answer.chosen_combat_defender.is_some()
            || answer.payment.is_some()
            || !answer.restricted_mana.is_empty()
            || answer.spell_cast_announcement.is_some();
        if invalid_shape {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal(
                "optional triggered ability choice contains unrelated fields",
            ));
        }
        let stack = match &pending.continuation {
            ResolutionContinuation::OptionalTriggeredAbility { stack } => stack.clone(),
            _ => {
                self.state.pending_resolution = Some(pending);
                return Err(EngineError::Illegal(
                    "optional triggered ability continuation missing",
                ));
            }
        };
        let mut events = Vec::new();
        let ability_text = stack.item.ability_text.as_deref().unwrap_or_default();
        let ability_presentation = self
            .state
            .stack_presentations
            .get(&stack.item.id)
            .and_then(|presentation| presentation.primary.clone());
        let resume_index = match decision {
            rv1::ResolutionChoiceDecision::SelectBranch => Some(0),
            rv1::ResolutionChoiceDecision::Decline => {
                events.push(ev_log_ability(
                    format!("P{} declines optional trigger: ", pending.deciding_player),
                    ability_text,
                    ability_presentation,
                    String::new(),
                ));
                Some(self.build_resolution_effects(&stack.item).0.len() as u32)
            }
            _ => {
                self.state.pending_resolution = Some(pending);
                return Err(EngineError::Illegal(
                    "optional triggered ability requires apply or decline",
                ));
            }
        };
        self.complete_parked_resolution_with_previous(
            stack.item,
            resume_index,
            stack.previous_result,
            events,
        )
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
            entry_reveal_receipts: Vec::new(),
            mana_colors_spent_to_cast: Default::default(),
            prepared: false,
            object_id: exiled.object_id,
            deciding_player: owner,
            destination_controller: owner,
            battle_protector: None,
            face_index: 0,
            unlock_room_door: None,
            chosen_x: 0,
            cast_by: None,
            cast_cost_receipts: Vec::new(),
            player_life_snapshot: self.player_life_snapshot(),
            tapped: false,
            set_types: None,
            chosen_basic_land_type: None,
            chosen_opponents: Vec::new(),
            entry_counters: BTreeMap::new(),
            entry_modifiers: Vec::new(),
            attached_to: Some(recipient),
            pending_copy_candidate: None,
            pending_aura_recipient: None,
            accepted_aura_recipient: Some(crate::state::AcceptedAuraEntryRecipient {
                filter,
                entering_zone_generation: generation,
                entering_copy_revision: object.copy_revision,
                recipient_generation: match recipient {
                    AttachmentRecipient::Object(oid) => Some(
                        self.state
                            .zone_change_generation
                            .get(&oid)
                            .copied()
                            .unwrap_or(0),
                    ),
                    AttachmentRecipient::Player(_) => None,
                },
                copy_candidate: None,
            }),
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
            super::replacement::BattlefieldEntryProgress::Ready(entry) => *entry,
            super::replacement::BattlefieldEntryProgress::Skipped(entry) => {
                let item = stack
                    .as_ref()
                    .map(|stack| stack.item.clone())
                    .unwrap_or_else(|| self.observer_return_item(exiled.object_id, owner));
                return self.finish_entry_copy_without_recipient(
                    stack.unwrap_or_else(|| ParkedStackResolution::new(item)),
                    *entry,
                    BattlefieldEntryCompletion::ObserverReturn {
                        owner,
                        object_label: label,
                        attached_to: Some(recipient),
                        resume_original_stack,
                    },
                    events,
                );
            }
        };
        self.state
            .pending_observer_return_batch
            .get_or_insert_with(|| PendingObserverReturnBatch {
                ready: Vec::new(),
                remaining: VecDeque::new(),
                resume_stack: stack.clone(),
            })
            .ready
            .push(ObserverReturnEntry {
                event: entry,
                owner,
                label,
                attached_to: Some(recipient),
            });
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
        if !self.entry_battle_protector_is_live(&entry.event, protector)
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

    fn finish_special_cast_choice(
        &mut self,
        pending: PendingResolution,
        answer: &rv1::SubmitResolutionChoice,
        decision: rv1::ResolutionChoiceDecision,
    ) -> Result<RuledEventBatch, EngineError> {
        if !answer.chosen_object_ids.is_empty()
            || answer.selected_branch_index != 0
            || answer.chosen_combat_defender.is_some()
            || answer.payment.is_some()
            || !answer.restricted_mana.is_empty()
        {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal(
                "special cast has unrelated resolution choice data",
            ));
        }
        let ResolutionContinuation::SpecialCast {
            stack,
            exiled,
            method,
            ..
        } = &pending.continuation
        else {
            unreachable!()
        };
        let stack = stack.clone();
        let exiled = *exiled;
        let method = *method;
        let mut events = Vec::new();
        match decision {
            rv1::ResolutionChoiceDecision::Decline => {
                if answer.cast_spell.is_some()
                    || answer.spell_cast_announcement.is_some()
                    || !answer.chosen_object_ids.is_empty()
                {
                    self.state.pending_resolution = Some(pending);
                    return Err(EngineError::Illegal(
                        "declining a cast cannot include an announcement",
                    ));
                }
                if method == SpellCastMethod::Madness
                    && self
                        .state
                        .objects
                        .get(&exiled.object_id)
                        .is_some_and(|o| o.zone == Zone::Exile)
                    && self
                        .state
                        .zone_change_generation
                        .get(&exiled.object_id)
                        .copied()
                        == Some(exiled.zone_change_generation)
                {
                    let owner = self.state.objects[&exiled.object_id].owner;
                    resolution::move_object_to_zone(
                        &mut self.state,
                        self.registry,
                        exiled.object_id,
                        Zone::Graveyard,
                        None,
                    )?;
                    self.state.discard_reference_successors.insert(
                        (exiled.object_id, exiled.zone_change_generation),
                        self.state.zone_change_generation[&exiled.object_id],
                    );
                    events.push(resolution::permanent_moved_event(
                        &self.state,
                        exiled.object_id,
                        owner,
                        rv1::permanent_moved::Destination::Graveyard,
                    ));
                }
                events.push(ev_log(format!(
                    "P{} declines to cast the offered card.",
                    pending.deciding_player
                )));
            }
            rv1::ResolutionChoiceDecision::CastSpell => {
                if answer.cast_spell.is_some() && answer.spell_cast_announcement.is_some() {
                    self.state.pending_resolution = Some(pending);
                    return Err(EngineError::Illegal(
                        "accepting a cast requires exactly one spell announcement",
                    ));
                }
                if let Some(announcement) = answer.spell_cast_announcement.as_ref() {
                    let player = pending.deciding_player;
                    self.state.pending_resolution = Some(pending);
                    return self.begin_spell_cast(
                        player,
                        &rv1::BeginSpellCast {
                            announcement: Some(announcement.clone()),
                        },
                    );
                }
                let Some(cast) = answer.cast_spell.as_ref() else {
                    self.state.pending_resolution = Some(pending);
                    return Err(EngineError::Illegal(
                        "accepting a cast requires a spell announcement",
                    ));
                };
                let player = pending.deciding_player;
                self.state.pending_resolution = Some(pending);
                match self.cast_spell(player, cast) {
                    Ok(batch) => return Ok(batch),
                    Err(error) => return Err(error),
                }
            }
            _ => {
                self.state.pending_resolution = Some(pending);
                return Err(EngineError::Illegal("cast choice requires cast or decline"));
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
    /// suspending effect in the same list, e.g. a hypothetical `[Scry, ChooseHandCards]`).
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
        let myr_consumer = stack.resume_effect_index.is_some_and(|index| {
            self.build_resolution_effects(&stack.item)
                .0
                .get(index as usize)
                .is_some_and(|effect| {
                    matches!(effect.effect, SpellEffectKind::MyrBattlesphereAttack)
                })
        });
        if myr_consumer
            && !self.myr_attack_cohort_is_legal(pending.deciding_player, &produced_objects)
        {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal("stale Myr attack payment"));
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

    fn select_targeted_player_choice_delegate(
        &mut self,
        mut pending: PendingResolution,
        answer: &rv1::SubmitResolutionChoice,
        decision: rv1::ResolutionChoiceDecision,
    ) -> Result<RuledEventBatch, EngineError> {
        if decision != rv1::ResolutionChoiceDecision::SelectBranch
            || !answer.chosen_object_ids.is_empty()
            || !answer.chosen_player_ids.is_empty()
            || answer.cast_spell.is_some()
            || answer.chosen_combat_defender.is_some()
            || answer.payment.is_some()
            || !answer.restricted_mana.is_empty()
            || answer.spell_cast_announcement.is_some()
        {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal(
                "replacement chooser must select one available player",
            ));
        }
        let (controller, target_player, candidate_slots) = match &pending.continuation {
            ResolutionContinuation::TargetedPlayerPermanentChoice {
                stack,
                target_player,
                stage: PendingTargetedPlayerChoiceStage::ChoosingDelegate { candidates },
                ..
            } => (stack.item.controller, *target_player, candidates.clone()),
            _ => {
                self.state.pending_resolution = Some(pending);
                return Err(EngineError::Illegal(
                    "targeted-player delegate continuation missing",
                ));
            }
        };
        let delegate = candidate_slots
            .get(answer.selected_branch_index as usize)
            .copied()
            .flatten();
        let eligible = resolution::resolution_choice_delegate_candidates(
            &self.state,
            controller,
            target_player,
        );
        let Some(delegate) = delegate.filter(|delegate| eligible.contains(delegate)) else {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal(
                "that replacement chooser is no longer available",
            ));
        };
        let Some(index) = self.state.player_idx(delegate) else {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal(
                "that replacement chooser is no longer available",
            ));
        };
        if self.state.players[index].has_lost {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal(
                "that replacement chooser is no longer available",
            ));
        }
        let old_candidates = match &pending.continuation {
            ResolutionContinuation::TargetedPlayerPermanentChoice {
                candidate_generations,
                ..
            } => candidate_generations.clone(),
            _ => unreachable!("validated targeted-player continuation"),
        };
        let candidates = old_candidates
            .iter()
            .copied()
            .filter(|(object_id, generation)| {
                self.state
                    .zone_change_generation
                    .get(object_id)
                    .copied()
                    .unwrap_or(0)
                    == *generation
                    && self
                        .state
                        .objects
                        .get(object_id)
                        .is_some_and(|object| object.zone == Zone::Battlefield)
            })
            .collect::<Vec<_>>();
        if candidates.is_empty() {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal(
                "no legal permanent remains for the replacement chooser",
            ));
        }
        let candidate_ids = candidates.iter().map(|(object_id, _)| *object_id).collect();
        let ResolutionContinuation::TargetedPlayerPermanentChoice {
            stage,
            candidate_generations,
            ..
        } = &mut pending.continuation
        else {
            unreachable!("validated targeted-player continuation")
        };
        *stage = PendingTargetedPlayerChoiceStage::ChoosingPermanent;
        *candidate_generations = candidates;
        pending.deciding_player = delegate;
        pending.presentation.candidates = candidate_ids;
        pending.presentation.min = 1;
        pending.presentation.max = 1;
        pending.presentation.choice_kind = rv1::ChoiceKind::PermanentObjects;
        pending.presentation.prompt = "Choose a permanent they control.".into();
        self.state.pending_resolution = Some(pending);
        let assigned = ev_log(format!(
            "P{controller} chooses P{delegate} to make P{target_player}'s permanent choice."
        ));
        let event = resolution::targeted_player_permanent_choice_event(self)
            .expect("delegated targeted-player choice remains parked");
        Ok(finish_with_events(self, vec![assigned, event]))
    }

    fn finish_targeted_player_permanent_choice(
        &mut self,
        pending: PendingResolution,
        chosen: &[ObjectId],
    ) -> Result<RuledEventBatch, EngineError> {
        let (stack, target_player, cohort, candidate_generations, stage) =
            match &pending.continuation {
                ResolutionContinuation::TargetedPlayerPermanentChoice {
                    stack,
                    target_player,
                    cohort,
                    candidate_generations,
                    stage,
                    ..
                } => (
                    stack.clone(),
                    *target_player,
                    cohort.clone(),
                    candidate_generations.clone(),
                    stage.clone(),
                ),
                _ => unreachable!("targeted-player permanent-choice continuation"),
            };
        if !matches!(stage, PendingTargetedPlayerChoiceStage::ChoosingPermanent) {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal(
                "replacement chooser must select a player before choosing a permanent",
            ));
        }
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
                    .unwrap_or(target_player),
            });
        }
        let names = chosen
            .iter()
            .map(|oid| object_display_name(&self.state, self.registry, *oid))
            .collect::<Vec<_>>();
        let events = if names.is_empty() {
            Vec::new()
        } else {
            vec![ev_log(format!(
                "P{} chooses {}.",
                pending.deciding_player,
                names.join(", ")
            ))]
        };
        self.complete_parked_resolution_with_previous(
            stack.item,
            stack.resume_effect_index,
            EffectResult {
                produced_objects,
                targeted_player_control_cohort: Some(cohort),
                ..Default::default()
            },
            events,
        )
    }

    fn complete_targeted_player_choice_without_selection(
        &mut self,
        pending: PendingResolution,
        message: &str,
    ) -> Result<RuledEventBatch, EngineError> {
        let (stack, cohort) = match &pending.continuation {
            ResolutionContinuation::TargetedPlayerPermanentChoice { stack, cohort, .. } => {
                (stack.clone(), cohort.clone())
            }
            _ => unreachable!("targeted-player permanent-choice continuation"),
        };
        self.complete_parked_resolution_with_previous(
            stack.item,
            stack.resume_effect_index,
            EffectResult {
                targeted_player_control_cohort: Some(cohort),
                ..Default::default()
            },
            vec![ev_log(message.to_string())],
        )
    }

    /// Refresh Teferi's parked permanent choice after a player leaves. A living target uses
    /// current characteristics and control; a departed target uses its predeparture LKI cohort.
    pub(in crate::engine) fn refresh_targeted_player_permanent_choice_departure(
        &mut self,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<(), EngineError> {
        let Some(mut pending) = self.state.pending_resolution.take() else {
            return Ok(());
        };
        let (
            stack,
            target_player,
            cohort,
            filter,
            constraints,
            old_candidate_generations,
            old_stage,
        ) = match &pending.continuation {
            ResolutionContinuation::TargetedPlayerPermanentChoice {
                stack,
                target_player,
                cohort,
                filter,
                constraints,
                candidate_generations,
                stage,
            } => (
                stack.clone(),
                *target_player,
                cohort.clone(),
                filter.clone(),
                constraints.clone(),
                candidate_generations.clone(),
                stage.clone(),
            ),
            _ => {
                self.state.pending_resolution = Some(pending);
                return Ok(());
            }
        };
        let controller = stack.item.controller;
        if self
            .state
            .player_idx(controller)
            .is_none_or(|index| self.state.players[index].has_lost)
        {
            self.state.pending_resolution = Some(pending);
            return Ok(());
        }
        let target_live = self
            .state
            .player_idx(target_player)
            .is_some_and(|index| !self.state.players[index].has_lost);
        let current_decider_live = self
            .state
            .player_idx(pending.deciding_player)
            .is_some_and(|index| !self.state.players[index].has_lost);
        let was_delegate_prompt = matches!(
            old_stage,
            PendingTargetedPlayerChoiceStage::ChoosingDelegate { .. }
        );
        let candidates = if target_live {
            resolution::current_targeted_player_permanent_candidates(
                self,
                &stack.item,
                target_player,
                &filter,
                &constraints,
            )
            .into_iter()
            .map(|object_id| {
                (
                    object_id,
                    self.state
                        .zone_change_generation
                        .get(&object_id)
                        .copied()
                        .unwrap_or(0),
                )
            })
            .collect::<Vec<_>>()
        } else {
            cohort
                .permanents
                .iter()
                .map(|member| (member.object_id, member.zone_change_generation))
                .filter(|(object_id, generation)| {
                    self.state
                        .zone_change_generation
                        .get(object_id)
                        .copied()
                        .unwrap_or(0)
                        == *generation
                        && self
                            .state
                            .objects
                            .get(object_id)
                            .is_some_and(|object| object.zone == Zone::Battlefield)
                })
                .collect::<Vec<_>>()
        };
        if candidates.len() < pending.presentation.min as usize {
            let completed = self.complete_targeted_player_choice_without_selection(
                pending,
                "No legal permanent remained for the targeted player's choice.",
            )?;
            events.extend(completed.events);
            return Ok(());
        }
        if !was_delegate_prompt && target_live && current_decider_live {
            if candidates != old_candidate_generations {
                let candidate_ids = candidates
                    .iter()
                    .map(|(object_id, _)| *object_id)
                    .collect::<Vec<_>>();
                let ResolutionContinuation::TargetedPlayerPermanentChoice {
                    candidate_generations,
                    ..
                } = &mut pending.continuation
                else {
                    unreachable!("validated targeted-player continuation")
                };
                *candidate_generations = candidates;
                pending.presentation.candidates = candidate_ids;
                self.state.pending_resolution = Some(pending);
                events.push(ev_log(format!(
                    "P{target_player}'s permanent choice options have changed."
                )));
                events.push(
                    resolution::targeted_player_permanent_choice_event(self)
                        .expect("refreshed targeted-player choice remains parked"),
                );
            } else {
                self.state.pending_resolution = Some(pending);
            }
            return Ok(());
        }

        let eligible = resolution::resolution_choice_delegate_candidates(
            &self.state,
            controller,
            target_player,
        );
        let mut candidate_slots = match old_stage {
            PendingTargetedPlayerChoiceStage::ChoosingDelegate { candidates } => candidates,
            PendingTargetedPlayerChoiceStage::ChoosingPermanent => Vec::new(),
        };
        for slot in &mut candidate_slots {
            if slot.is_some_and(|candidate| !eligible.contains(&candidate)) {
                *slot = None;
            }
        }
        for candidate in eligible.iter().copied() {
            if !candidate_slots.contains(&Some(candidate)) {
                candidate_slots.push(Some(candidate));
            }
        }
        if eligible.is_empty() {
            let completed = self.complete_targeted_player_choice_without_selection(
                pending,
                "No other player can make the targeted player's permanent choice.",
            )?;
            events.extend(completed.events);
            return Ok(());
        }

        let retain_delegate_prompt = was_delegate_prompt;
        let delegate = if eligible.len() == 1 && !retain_delegate_prompt {
            eligible.first().copied()
        } else {
            None
        };
        let candidate_ids = candidates
            .iter()
            .map(|(object_id, _)| *object_id)
            .collect::<Vec<_>>();
        let ResolutionContinuation::TargetedPlayerPermanentChoice {
            candidate_generations,
            stage,
            ..
        } = &mut pending.continuation
        else {
            unreachable!("validated targeted-player continuation")
        };
        *candidate_generations = candidates;
        if let Some(delegate) = delegate {
            pending.deciding_player = delegate;
            pending.presentation.candidates = candidate_ids;
            pending.presentation.min = 1;
            pending.presentation.max = 1;
            pending.presentation.choice_kind = rv1::ChoiceKind::PermanentObjects;
            pending.presentation.prompt = "Choose a permanent they control.".into();
            *stage = PendingTargetedPlayerChoiceStage::ChoosingPermanent;
            self.state.pending_resolution = Some(pending);
            events.push(ev_log(format!(
                "P{controller} assigns P{delegate} to make P{target_player}'s permanent choice."
            )));
        } else {
            pending.deciding_player = controller;
            pending.presentation.candidates.clear();
            pending.presentation.min = 1;
            pending.presentation.max = 1;
            pending.presentation.choice_kind = rv1::ChoiceKind::ResolutionBranch;
            pending.presentation.prompt = format!(
                "P{controller}: choose a player to make P{target_player}'s permanent choice."
            );
            *stage = PendingTargetedPlayerChoiceStage::ChoosingDelegate {
                candidates: candidate_slots,
            };
            self.state.pending_resolution = Some(pending);
            events.push(ev_log(format!(
                "P{controller}'s replacement chooser options have changed."
            )));
        }
        events.push(
            resolution::targeted_player_permanent_choice_event(self)
                .expect("reassigned targeted-player choice remains parked"),
        );
        Ok(())
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
                            && card_predicates::zone_card_matches_filter(
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
                events.extend(super::reveals::reveal_cards(
                    &self.state,
                    self.registry,
                    &[oid],
                    item.id,
                    &object_display_name(&self.state, self.registry, item.id),
                ));
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
            let progress = self.run_effect_list_with_previous(
                &item,
                &spell_label,
                effects,
                start as usize,
                previous_result,
                &mut ev,
            )?;
            if progress == super::resolution::ResolutionProgress::GameEnded {
                return Ok(finish_with_events(self, ev));
            }
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
    ) -> Result<super::resolution::ResolutionProgress, EngineError> {
        let interrupt = match step {
            // CR 608.2n: this is the single point where a tier-3 resolution completes, whether it
            // ran straight through in `begin` or came back here from a later `resume`, so it is
            // where the spell takes its place beneath whatever its resolution put in the
            // graveyard — e.g. Gifts Ungiven under the two cards it puts there.
            ResolutionStep::Done => {
                if custom_key == "brainstorm" {
                    self.finish_deferred_stack_exit(
                        &item,
                        super::resolution::DeferredStackExit::Resolved,
                        events,
                    )?;
                }
                super::resolution::finish_deferred_graveyard_entry(&mut self.state, &item);
                seat_resolved_spell_last_in_graveyard(&mut self.state, item.id);
                return Ok(super::resolution::ResolutionProgress::Completed);
            }
            ResolutionStep::NeedsChoice(it) => it,
            ResolutionStep::Draw {
                player,
                count,
                after: custom::PostDrawPhase::BrainstormPutBack,
            } => {
                let completion = super::draw::DrawCompletion::BrainstormPutBack {
                    stack: ParkedStackResolution::new(item),
                    step: step_no,
                    scratch,
                };
                match self.start_draw_transaction(
                    vec![(player, count)],
                    completion,
                    "Brainstorm",
                    events,
                )? {
                    super::draw::DrawProgress::GameEnded => {
                        return Ok(super::resolution::ResolutionProgress::GameEnded);
                    }
                    super::draw::DrawProgress::Parked => {
                        return Ok(super::resolution::ResolutionProgress::Parked);
                    }
                    super::draw::DrawProgress::Complete(done) => {
                        let super::draw::DrawCompletion::BrainstormPutBack {
                            stack,
                            step,
                            scratch,
                        } = done.completion
                        else {
                            unreachable!()
                        };
                        return self.finish_brainstorm_draw(stack.item, step, scratch, events);
                    }
                }
            }
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
                    variable_mana_contribution: false,
                    candidate_token_identities: Vec::new(),
                    candidate_player_ids: Vec::new(),
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
                    candidate_selectable: vec![true; interrupt.candidates.len()],
                    resolution_branches: Vec::new(),
                    mana_cost: String::new(),
                    generic_mana_cost: 0,
                    payment_currently_legal: false,
                    public_reveal: if interrupt.public_reveal {
                        super::reveals::reveal_choice(
                            &self.state,
                            self.registry,
                            &interrupt.candidates,
                            item.id,
                            &object_display_name(&self.state, self.registry, item.id),
                        )
                    } else {
                        None
                    },
                    candidate_source_zones: Vec::new(),
                    combat_defender_options: Vec::new(),
                    waterbend: false,
                    selection_slots: Vec::new(),
                    replacement_options: Vec::new(),
                    selection_alternatives: Vec::new(),
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
        Ok(super::resolution::ResolutionProgress::Parked)
    }

    pub(super) fn finish_brainstorm_draw(
        &mut self,
        item: StackItem,
        step: u32,
        scratch: Vec<ObjectId>,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<super::resolution::ResolutionProgress, EngineError> {
        let controller = item.controller;
        let next = {
            let ctx = ResolutionCtx::new(
                &mut self.state,
                self.registry,
                events,
                controller,
                item.id,
                step,
                scratch.clone(),
            );
            custom::custom_effect_brainstorm::after_draw(&ctx)
        };
        self.park_or_finish(item, "brainstorm".to_string(), step, scratch, next, events)
    }
}
