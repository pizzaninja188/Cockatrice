use super::events::{ev_log, ev_phase, ev_priority_changed, finish_with_events};
use super::legal_actions::fill_legal;
use super::resolution::move_object_to_zone;
use super::*;

/// Sorcery-speed window: your main phase, stack empty, you are the active player (CR 307.5,
/// 601.2; lands CR 305.3).
pub(super) fn sorcery_speed_available(state: &GameState, player: PlayerId) -> bool {
    matches!(state.turn_step, TurnStep::Main1 | TurnStep::Main2)
        && state.stack.is_empty()
        && player == state.active_player_id()
}

pub(super) fn instant_timing_step_allowed(state: &GameState) -> bool {
    matches!(
        state.turn_step,
        TurnStep::Main1
            | TurnStep::Main2
            | TurnStep::Upkeep
            | TurnStep::Draw
            | TurnStep::BeginCombat
            | TurnStep::DeclareAttackers
            | TurnStep::DeclareBlockers
            | TurnStep::CombatDamage
            | TurnStep::EndCombat
            | TurnStep::EndStep
    ) || (state.turn_step == TurnStep::Cleanup && state.cleanup_priority_active)
}

impl GameEngine {
    /// Fail closed on an unexpected stale surviving incarnation without refusing concession.
    /// Independent returns use an end-of-list frame so they still finish before the outer exit.
    pub(super) fn abandon_participating_resolution(
        &mut self,
        mut stack: Option<ParkedStackResolution>,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<(), EngineError> {
        if let Some(stack) = stack.as_mut() {
            stack.resume_effect_index =
                Some(self.build_resolution_effects(&stack.item).0.len() as u32);
        }
        if let Some(batch) = self.state.pending_observer_return_batch.as_mut() {
            batch.resume_stack = stack.clone();
        }
        events.push(ev_log(
            "Interrupted resolution abandoned because a surviving arrival changed incarnation."
                .into(),
        ));
        if self.drain_immediate_observer_actions(stack.clone(), events)? {
            return Ok(());
        }
        if let Some(stack) = stack {
            let completed = self.complete_parked_resolution_with_previous(
                stack.item,
                stack.resume_effect_index,
                stack.previous_result,
                Vec::new(),
            )?;
            events.extend(completed.events);
        }
        Ok(())
    }
    /// Cancel the departed player's unfinished instruction while retaining committed ordering
    /// and independently owed one-shot returns (CR 800.4a, 404.3 and 610.3).
    fn detach_departed_outer_resolution(
        &mut self,
        departed: &[PlayerId],
    ) -> Result<(), EngineError> {
        let is_departed = |stack: &ParkedStackResolution| {
            departed.contains(&stack.item.controller)
                || self
                    .state
                    .objects
                    .get(&stack.item.id)
                    .is_some_and(|object| departed.contains(&object.owner))
        };
        let independent_entry = matches!(self.state.pending_replacement_event.as_ref(),
            Some(super::replacement::PendingReplacementEvent::BattlefieldEntry(entry))
                if matches!(entry.completion, BattlefieldEntryCompletion::ObserverReturn { resume_original_stack: false, .. }));
        let outer = self
            .state
            .pending_resolution
            .as_ref()
            .and_then(|pending| {
                (!independent_entry)
                    .then(|| pending.continuation.stack())
                    .flatten()
            })
            .filter(|stack| is_departed(stack))
            .cloned()
            .or_else(|| {
                self.state
                    .pending_observer_return_batch
                    .as_ref()
                    .and_then(|batch| batch.resume_stack.as_ref())
                    .filter(|stack| is_departed(stack))
                    .cloned()
            });
        let Some(outer) = outer else {
            return Ok(());
        };
        if let Some(batch) = self.state.pending_observer_return_batch.as_mut() {
            batch.resume_stack = None;
        }
        let observer_entry = match self.state.pending_replacement_event.as_mut() {
            Some(super::replacement::PendingReplacementEvent::BattlefieldEntry(entry)) => {
                match &mut entry.completion {
                    BattlefieldEntryCompletion::ObserverReturn {
                        owner,
                        resume_original_stack,
                        ..
                    } => {
                        *resume_original_stack = false;
                        Some((entry.event.object_id, *owner))
                    }
                    _ => None,
                }
            }
            _ => None,
        };
        let synthetic = observer_entry
            .map(|(oid, owner)| ParkedStackResolution::new(self.observer_return_item(oid, owner)));
        let mut preserve = false;
        if let Some(pending) = self.state.pending_resolution.as_mut() {
            match &mut pending.continuation {
                ResolutionContinuation::MassSacrificeGraveyardOrder { stack, .. }
                | ResolutionContinuation::AuraReturn { stack, .. } => {
                    *stack = None;
                    preserve = true;
                }
                ResolutionContinuation::SimultaneousEntryOrder { stack, order } => {
                    if let SimultaneousEntryBatch::Observer(batch) = &mut order.batch {
                        *stack = None;
                        batch.resume_stack = None;
                        preserve = true;
                    }
                }
                continuation => {
                    if let (Some(synthetic), Some(stack)) = (synthetic, continuation.stack_mut()) {
                        *stack = synthetic;
                        preserve = true;
                    }
                }
            }
        }
        if !preserve {
            self.state.pending_resolution = None;
            self.state.pending_replacement_event = None;
        }
        if self
            .state
            .objects
            .get(&outer.item.id)
            .is_some_and(|object| !departed.contains(&object.owner) && object.zone == Zone::Stack)
        {
            move_object_to_zone(
                &mut self.state,
                self.registry,
                outer.item.id,
                Zone::Exile,
                None,
            )?;
        }
        Ok(())
    }
    /// CR 800.4a/e: objects owned by a departing player leave the game, and attacks aimed at
    /// that player stop participating in combat. No zone-change triggers fire for this removal.
    fn remove_departing_player_objects(
        &mut self,
        player: PlayerId,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<(), EngineError> {
        let owned: HashSet<ObjectId> = self
            .state
            .objects
            .iter()
            .filter_map(|(&id, object)| (object.owner == player).then_some(id))
            .collect();
        let mut removed_from_combat = Vec::new();
        if let Some(combat) = self.state.combat.as_mut() {
            let removed_attackers: HashSet<ObjectId> = combat
                .attacking
                .iter()
                .copied()
                .filter(|id| {
                    owned.contains(id)
                        || combat
                            .attack_assignments
                            .get(id)
                            .is_some_and(|assignment| assignment.defending_player == player)
                })
                .collect();
            removed_from_combat.extend(removed_attackers.iter().copied());
            combat
                .attacking
                .retain(|id| !removed_attackers.contains(id));
            combat
                .attack_assignments
                .retain(|id, _| !removed_attackers.contains(id));
            combat
                .blockers
                .retain(|id, _| !removed_attackers.contains(id));
            for blockers in combat.blockers.values_mut() {
                for id in blockers.iter().copied().filter(|id| owned.contains(id)) {
                    removed_from_combat.push(id);
                }
                blockers.retain(|id| !owned.contains(id));
            }
            combat
                .damage_assignments
                .retain(|id, _| !removed_attackers.contains(id));
            combat
                .trample_player_damage
                .retain(|id, _| !removed_attackers.contains(id));
            combat
                .first_strike_attackers
                .retain(|id| !removed_attackers.contains(id));
            combat.first_strike_blockers.retain(|id, blockers| {
                if removed_attackers.contains(id) {
                    return false;
                }
                blockers.retain(|blocker| !owned.contains(blocker));
                true
            });
        }
        removed_from_combat.sort_unstable();
        removed_from_combat.dedup();
        if !removed_from_combat.is_empty() {
            events.push(rv1::RuledEvent {
                ev: Some(rv1::ruled_event::Ev::RemovedFromCombat(
                    rv1::CreaturesRemovedFromCombat {
                        object_ids: removed_from_combat,
                    },
                )),
            });
        }
        for participant in &mut self.state.players {
            participant.library.retain(|id| !owned.contains(id));
            participant.hand.retain(|id| !owned.contains(id));
            participant.battlefield.retain(|id| !owned.contains(id));
            participant.graveyard.retain(|id| !owned.contains(id));
            participant.exile.retain(|id| !owned.contains(id));
        }
        let unowned_stack_cards: Vec<_> = self
            .state
            .stack
            .iter()
            .filter(|item| item.controller == player)
            .filter_map(|item| {
                self.state
                    .objects
                    .get(&item.id)
                    .filter(|object| object.owner != player && object.zone == Zone::Stack)
                    .map(|_| item.id)
            })
            .collect();
        self.state
            .stack
            .retain(|item| !owned.contains(&item.id) && item.controller != player);
        self.state
            .stack_presentations
            .retain(|id, _| !owned.contains(id));
        self.state.objects.retain(|id, _| !owned.contains(id));
        self.state
            .chosen_opponents
            .retain(|record| !owned.contains(&record.key.source_object_id));
        for id in unowned_stack_cards {
            move_object_to_zone(&mut self.state, self.registry, id, Zone::Exile, None)?;
        }
        Ok(())
    }

    pub(super) fn reconcile_departed_players(
        &mut self,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<(), EngineError> {
        if self.state.is_terminal() {
            return Ok(());
        }
        let departed: Vec<_> = self
            .state
            .players
            .iter()
            .filter(|player| player.has_lost)
            .map(|player| player.id)
            .collect();
        if departed.is_empty() {
            return Ok(());
        }
        // Preserve actual owner removals across deletion; entry controllers/deciders may survive.
        let departed_objects: HashSet<_> = self
            .state
            .objects
            .values()
            .filter(|object| departed.contains(&object.owner))
            .map(|object| object.id)
            .collect();
        // Capture the complete cohort before deleting any static-grant provider or ending
        // control effects. A surviving foreign-controlled ability reads this exact source.
        let source_snapshots: Vec<_> = departed_objects
            .iter()
            .filter_map(|&oid| {
                self.state
                    .objects
                    .get(&oid)
                    .filter(|object| object.zone == Zone::Battlefield)?;
                let generation = self
                    .state
                    .zone_change_generation
                    .get(&oid)
                    .copied()
                    .unwrap_or(0);
                self.characteristics(oid)
                    .map(|characteristics| (oid, generation, characteristics))
            })
            .collect();
        for (oid, generation, characteristics) in source_snapshots {
            super::resolution::record_last_known_characteristics(
                &mut self.state,
                oid,
                generation,
                characteristics,
            );
        }
        self.detach_departed_outer_resolution(&departed)?;
        for player in departed {
            self.remove_departing_player_objects(player, events)?;
            // CR 800.4a: an effect granting control to a player who left ends immediately.
            self.state.continuous_effects.retain(|effect| {
                !matches!(effect.kind, ContinuousEffectKind::Layer2Control {
                    controller: ControllerReference::Fixed(controller),
                } if controller == player)
            });
        }
        self.reindex_battlefield_control(events);
        let stranded: Vec<_> = self
            .state
            .objects
            .iter()
            .filter_map(|(&id, object)| {
                (object.zone == Zone::Battlefield
                    && self
                        .state
                        .players
                        .iter()
                        .any(|p| p.id == object.controller && p.has_lost))
                .then_some(id)
            })
            .collect();
        for id in stranded {
            move_object_to_zone(&mut self.state, self.registry, id, Zone::Exile, None)?;
        }
        if self.state.turn_step == TurnStep::DeclareBlockers
            && self
                .state
                .combat
                .as_ref()
                .is_some_and(|combat| !combat.blockers_declared)
        {
            let next_defender = self.next_blocking_player_needing_declaration();
            if let Some(defender) = next_defender {
                self.state.priority_idx = self.state.player_idx(defender).unwrap();
                self.state.passes_since_stack_change = 0;
                events.push(ev_priority_changed(self));
            } else {
                self.state.combat.as_mut().unwrap().blockers_declared = true;
                self.finalize_block_declarations(events)?;
                let active = self.state.active_player_idx;
                self.state.priority_idx = if self.state.players[active].has_lost {
                    self.state
                        .next_in_game_player_idx(active)
                        .ok_or(EngineError::Illegal("no player in game"))?
                } else {
                    active
                };
                self.state.passes_since_stack_change = 0;
                events.push(ev_priority_changed(self));
            }
        }
        if self.state.players[self.state.priority_idx].has_lost {
            if let Some(next) = self.state.next_in_game_player_idx(self.state.priority_idx) {
                self.state.priority_idx = next;
                self.state.passes_since_stack_change = 0;
                if self.state.pending_resolution.is_none() {
                    events.push(ev_priority_changed(self));
                }
            }
        }
        self.refresh_mass_sacrifice_departure(events)?;
        self.refresh_damage_departure(events)?;
        self.refresh_chaos_warp_departure(events)?;
        self.refresh_entry_timestamp_departure(&departed_objects, events)?;
        self.refresh_observer_aura_departure(events)?;
        self.refresh_participating_entry_departure(&departed_objects, events)?;
        self.refresh_entry_opponent_departure(&departed_objects, events)?;
        Ok(())
    }

    /// A combat step owes one priority window after the complete shared settlement boundary.
    pub(super) fn publish_settled_combat_priority(&mut self, events: &mut Vec<rv1::RuledEvent>) {
        if !self.state.combat_damage_priority_pending {
            return;
        }
        events.retain(|event| !matches!(event.ev, Some(rv1::ruled_event::Ev::PriorityChanged(_))));
        if self.state.is_terminal() || self.state.blocking_choice().is_some() {
            return;
        }
        let active = self.state.active_player_idx;
        let priority = if !self.state.players[active].has_lost {
            Some(active)
        } else {
            self.state.next_in_game_player_idx(active)
        };
        if let Some(priority) = priority {
            self.state.priority_idx = priority;
            self.state.passes_since_stack_change = 0;
            self.state.combat_damage_priority_pending = false;
            events.push(ev_priority_changed(self));
        }
    }

    /// Apply deferred draw-from-empty losses once a resolving effect has completed. A player
    /// remains in the game during resolution so mandatory trailing instructions can finish;
    /// the next command boundary then performs the CR 704.5b state-based action (CR 704.4).
    pub(super) fn commit_pending_library_losses(&mut self) {
        if self.state.is_terminal() {
            return;
        }
        for player in &mut self.state.players {
            if player.pending_library_loss {
                player.pending_library_loss = false;
                player.has_lost = true;
            }
        }
    }

    pub(super) fn sweep_life(&mut self) {
        if self.state.is_terminal() {
            return;
        }
        for p in &mut self.state.players {
            if p.life <= 0 {
                p.has_lost = true;
            }
        }
        let still_in: Vec<PlayerId> = self
            .state
            .players
            .iter()
            .filter(|p| p.life > 0 && !p.has_lost)
            .map(|p| p.id)
            .collect();
        self.state.outcome = match still_in.as_slice() {
            [] => Some(crate::state::GameOutcome::Draw),
            [winner] => Some(crate::state::GameOutcome::Winner(*winner)),
            _ => None,
        };
    }

    pub(super) fn concede_batch(
        &mut self,
        player: PlayerId,
    ) -> Result<RuledEventBatch, EngineError> {
        // A concession removes this player; other free-for-all players continue until one remains.
        for p in &mut self.state.players {
            if p.id == player {
                p.has_lost = true;
            }
        }
        let mut batch = RuledEventBatch::default();
        self.state.continuous_effects.retain(|effect| {
            !matches!(
                effect.duration,
                EffectDuration::UntilEndOfNextTurn {
                    player: duration_player,
                    ..
                } if duration_player == player
            )
        });
        batch.events.push(ev_log(format!("P{player} conceded")));
        // Apply the concession's already-ended control durations before it determines a winner.
        self.reindex_battlefield_control(&mut batch.events);
        // Concession takes effect immediately, but a parked instruction/payment must not
        // turn a surviving player's intermediate life value into an early SBA loss.
        let still_in: Vec<_> = self
            .state
            .players
            .iter()
            .filter(|player| !player.has_lost)
            .map(|player| player.id)
            .collect();
        self.state.outcome = match still_in.as_slice() {
            [] => Some(crate::state::GameOutcome::Draw),
            [winner] => Some(crate::state::GameOutcome::Winner(*winner)),
            _ => None,
        };
        if self.state.is_terminal() {
            return Ok(self.finish_terminal_batch(batch));
        }
        if !self.state.is_terminal() {
            self.reconcile_opening_departure(player, &mut batch.events)?;
        }
        self.reconcile_departed_players(&mut batch.events)?;
        self.reindex_battlefield_control(&mut batch.events);
        self.reconcile_draw_departure(&mut batch.events)?;
        if self.state.is_terminal() {
            return Ok(self.finish_terminal_batch(batch));
        }
        self.reconcile_activated_ability_slots();
        batch.events.push(self.ev_zone_view_sync_tracked());
        fill_legal(&mut batch, self);
        Ok(batch)
    }

    pub(super) fn pass_priority(
        &mut self,
        player: PlayerId,
    ) -> Result<RuledEventBatch, EngineError> {
        if self.state.priority_player_id() != player {
            return Err(EngineError::Illegal("not your priority"));
        }
        // `dispatch_command`'s blocking gate already rejects these before they reach here; kept as
        // the local, better-worded refusal for the internal callers that bypass dispatch.
        match self.state.blocking_choice() {
            Some(BlockingChoice::AbilityActivation) => {
                return Err(EngineError::Illegal(
                    "must finish ability activation before passing priority",
                ));
            }
            Some(BlockingChoice::TriggerTarget) => {
                return Err(EngineError::Illegal(
                    "must choose trigger target before passing priority",
                ));
            }
            Some(BlockingChoice::TriggerOrder) => {
                return Err(EngineError::Illegal(
                    "must order simultaneous triggers before passing priority",
                ));
            }
            Some(BlockingChoice::Resolution) => {
                return Err(EngineError::Illegal(
                    "must submit resolution choice before passing priority",
                ));
            }
            None => {}
        }
        if self.state.stack.is_empty()
            && self.state.turn_step == TurnStep::Cleanup
            && self.state.cleanup_discard_player.is_some()
        {
            return Err(EngineError::Illegal("discard to hand size first"));
        }
        let n = self.state.players.iter().filter(|p| !p.has_lost).count() as u32;
        if !self.state.stack.is_empty() {
            return self.pass_priority_on_stack(player, n);
        }
        // empty stack
        self.state.passes_since_stack_change += 1;
        self.state.priority_idx = self
            .state
            .next_in_game_player_idx(self.state.priority_idx)
            .ok_or(EngineError::Illegal("no player in game"))?;
        let ev = vec![rv1::RuledEvent {
            ev: Some(rv1::ruled_event::Ev::PriorityChanged(
                rv1::PriorityChanged {
                    player_id: self.state.priority_player_id(),
                },
            )),
        }];
        if self.state.passes_since_stack_change < n {
            let mut batch = RuledEventBatch {
                payment_preview: None,
                events: ev,
                legal_by_player: Default::default(),
            };
            self.apply_sbas(&mut batch.events)?;
            fill_legal(&mut batch, self);
            return Ok(batch);
        }
        self.state.passes_since_stack_change = 0;
        let mut ev2 = vec![];
        self.adv_on_empty_stack(&mut ev2)
    }

    pub(super) fn pass_priority_on_stack(
        &mut self,
        player: PlayerId,
        n: u32,
    ) -> Result<RuledEventBatch, EngineError> {
        self.state.passes_since_stack_change += 1;
        self.state.priority_idx = self
            .state
            .next_in_game_player_idx(self.state.player_idx(player).unwrap())
            .ok_or(EngineError::Illegal("no player in game"))?;
        let mut ev = vec![rv1::RuledEvent {
            ev: Some(rv1::ruled_event::Ev::PriorityChanged(
                rv1::PriorityChanged {
                    player_id: self.state.priority_player_id(),
                },
            )),
        }];
        if self.state.passes_since_stack_change < n {
            self.apply_sbas(&mut ev)?;
            return Ok(finish_with_events(self, ev));
        }
        self.state.passes_since_stack_change = 0;
        if let Some(i) = self.state.player_idx(self.state.active_player_id()) {
            self.state.priority_idx = i;
        }
        let progress = self.resolve_top_of_stack(&mut ev)?;
        if progress == super::resolution::ResolutionProgress::GameEnded {
            return Ok(finish_with_events(self, ev));
        }
        // A tier-3 custom resolution may have parked mid-resolution awaiting a player choice
        // (CR 608): no player holds priority then — the ResolutionChoiceRequired event already
        // drives the deciding player — so don't advance priority or run SBAs until it completes.
        if self.state.pending_resolution.is_none() {
            ev.push(ev_priority_changed(self));
            self.apply_sbas(&mut ev)?;
        }
        Ok(finish_with_events(self, ev))
    }

    pub(super) fn adv_on_empty_stack(
        &mut self,
        ev: &mut Vec<rv1::RuledEvent>,
    ) -> Result<RuledEventBatch, EngineError> {
        use TurnStep::*;
        let step = self.state.turn_step;
        let ap = self.state.active_player_id();
        match step {
            Untap => {
                self.clear_all_mana_pools();
                self.state.turn_step = Upkeep;
                self.state.combat = None;
                if let Some(i) = self.state.player_idx(ap) {
                    self.state.priority_idx = i;
                }
                self.state.passes_since_stack_change = 0;
                ev.push(ev_phase(self, rv1::PhaseId::Upkeep));
                self.fire_triggers(&[GameEvent::PhaseBegan {
                    phase: rv1::PhaseId::Upkeep,
                    active_player: ap,
                }]);
                self.flush_staged_triggers(ev);
                if self.state.blocking_choice().is_none() {
                    ev.push(ev_priority_changed(self));
                }
            }
            Upkeep => {
                self.clear_all_mana_pools();
                self.state.turn_step = Draw;
                let occurrence = self
                    .state
                    .draw_step_progress
                    .map_or(1, |(_, previous, _)| previous + 1);
                self.state.draw_step_progress = Some((ap, occurrence, 0));
                if let Some(i) = self.state.player_idx(ap) {
                    self.state.priority_idx = i;
                }
                ev.push(ev_phase(self, rv1::PhaseId::Draw));
                // First draw step of the duel: only the starting player skips (CR 103.8). `turn`
                // may stay 1 for the second seat's first turn because we bump `turn` when wrapping
                // to seat 0, not on every active change.
                let skip_opening_draw = self.state.players.len() == 2
                    && self.state.turn == 1
                    && self.state.active_player_idx == self.state.starting_player_idx;
                if !skip_opening_draw {
                    let completion = super::draw::DrawCompletion::FinishDrawStep {
                        active_player: ap,
                        occurrence,
                    };
                    match self.start_draw_transaction(vec![(ap, 1)], completion, "draw step", ev)? {
                        super::draw::DrawProgress::Parked => {
                            return Ok(finish_with_events(self, std::mem::take(ev)))
                        }
                        super::draw::DrawProgress::Complete(_) => {}
                        super::draw::DrawProgress::GameEnded => {
                            return Ok(finish_with_events(self, std::mem::take(ev)));
                        }
                    }
                }
                self.finish_draw_step_action(ap, occurrence, ev)?;
            }
            Draw => {
                self.clear_all_mana_pools();
                self.state.turn_step = Main1;
                self.state.combat = None;
                if let Some(i) = self.state.player_idx(ap) {
                    self.state.priority_idx = i;
                }
                self.state.passes_since_stack_change = 0;
                ev.push(ev_phase(self, rv1::PhaseId::Main1));
                self.perform_precombat_saga_lore_action();
                self.apply_sbas(ev)?;
                self.flush_staged_triggers(ev);
                if self.state.blocking_choice().is_none() {
                    ev.push(ev_priority_changed(self));
                }
            }
            Main1 => {
                self.clear_all_mana_pools();
                self.state.turn_step = BeginCombat;
                if let Some(i) = self.state.player_idx(ap) {
                    self.state.priority_idx = i;
                }
                ev.push(ev_phase(self, rv1::PhaseId::BeginCombat));
                self.fire_triggers(&[GameEvent::PhaseBegan {
                    phase: rv1::PhaseId::BeginCombat,
                    active_player: ap,
                }]);
                self.flush_staged_triggers(ev);
                if self.state.blocking_choice().is_none() {
                    ev.push(ev_priority_changed(self));
                }
            }
            BeginCombat => {
                self.clear_step_mana_pools();
                if !self.active_player_has_eligible_attackers() {
                    // No eligible attackers — skip all declare substeps.
                    self.state.combat = None;
                    self.state.turn_step = EndCombat;
                    if let Some(i) = self.state.player_idx(ap) {
                        self.state.priority_idx = i;
                    }
                    self.state.passes_since_stack_change = 0;
                    ev.push(ev_phase(self, rv1::PhaseId::EndCombat));
                    ev.push(ev_priority_changed(self));
                } else {
                    self.state.turn_step = DeclareAttackers;
                    if let Some(i) = self.state.player_idx(ap) {
                        self.state.priority_idx = i;
                    }
                    self.state.combat = Some(CombatState {
                        attacking: vec![],
                        attack_assignments: HashMap::new(),
                        blockers: HashMap::new(),
                        damage_assignments: HashMap::new(),
                        trample_player_damage: HashMap::new(),
                        damage_assignment_needed: false,
                        attackers_declared: false,
                        blockers_declared_by: Vec::new(),
                        blockers_declared: false,
                        assign_combat_damage_phase: false,
                        first_strike_attackers: Vec::new(),
                        first_strike_blockers: HashMap::new(),
                        first_strike_damage_done: false,
                    });
                    ev.push(ev_phase(self, rv1::PhaseId::DeclareAttackers));
                    ev.push(ev_priority_changed(self));
                }
            }
            DeclareAttackers => {
                self.clear_step_mana_pools();
                self.state.passes_since_stack_change = 0;
                let has_attackers = self
                    .state
                    .combat
                    .as_ref()
                    .is_some_and(|c| !c.attacking.is_empty());
                let next_defender = self.next_blocking_player_needing_declaration();
                if next_defender.is_none() || !has_attackers {
                    // Auto-declare empty blockers; active player gets priority in DeclareBlockers.
                    let blocking_players = self.blocking_player_ids();
                    if let Some(c) = self.state.combat.as_mut() {
                        c.blockers.clear();
                        c.damage_assignments.clear();
                        c.damage_assignment_needed = false;
                        c.assign_combat_damage_phase = false;
                        c.blockers_declared = true;
                        c.blockers_declared_by = blocking_players;
                    }
                    self.state.turn_step = DeclareBlockers;
                    if let Some(i) = self.state.player_idx(ap) {
                        self.state.priority_idx = i;
                    }
                    ev.push(ev_log(
                        "No eligible blockers — auto-declaring empty blockers.".into(),
                    ));
                    ev.push(ev_phase(self, rv1::PhaseId::DeclareBlockers));
                    // Emit BlockersDeclared (empty) AFTER phase_changed so the client's
                    // blockersSubmittedThisStep ends up true (phase_changed resets it to false,
                    // then BlockersDeclared sets it true; order matters).
                    ev.push(RuledEvent {
                        ev: Some(rv1::ruled_event::Ev::BlockersDeclared(
                            rv1::BlockersDeclared {
                                block_pairs: vec![],
                            },
                        )),
                    });
                    ev.push(ev_priority_changed(self));
                } else {
                    self.state.turn_step = DeclareBlockers;
                    // CR 509.1 / 101.4: the first defending player in APNAP order acts first.
                    if let Some(d) = next_defender {
                        if let Some(di) = self.state.player_idx(d) {
                            self.state.priority_idx = di;
                        }
                    }
                    ev.push(ev_phase(self, rv1::PhaseId::DeclareBlockers));
                    ev.push(ev_priority_changed(self));
                }
            }
            DeclareBlockers => {
                // After blockers are declared, players receive priority in declare blockers before
                // moving to damage-order assignment (multi-block) or combat damage.
                let c = self
                    .state
                    .combat
                    .clone()
                    .ok_or(EngineError::Illegal("combat?"))?;
                let multiblock_missing = c.blockers.iter().any(|(atk, blks)| {
                    // Trample with 1+ blockers also requires explicit damage assignment (CR 702.19).
                    let has_trample =
                        self.effective_has_keyword(*atk, tricerules_cards::Keyword::Trample);
                    let needs_assign = blks.len() > 1 || (blks.len() == 1 && has_trample);
                    needs_assign && !c.damage_assignments.contains_key(atk)
                });
                if multiblock_missing {
                    if !c.assign_combat_damage_phase {
                        if let Some(cc) = self.state.combat.as_mut() {
                            cc.assign_combat_damage_phase = true;
                        }
                        self.clear_step_mana_pools();
                        self.state.turn_step = DeclareBlockers;
                        if let Some(i) = self.state.player_idx(ap) {
                            self.state.priority_idx = i;
                        }
                        self.state.passes_since_stack_change = 0;
                        ev.push(ev_log(
                            "Proceeding to combat damage assignment (after declare blockers)."
                                .into(),
                        ));
                        ev.push(ev_phase(self, rv1::PhaseId::AssignCombatDamage));
                        ev.push(ev_priority_changed(self));
                    } else {
                        return Err(EngineError::Illegal(
                            "must assign combat damage before combat damage resolves",
                        ));
                    }
                } else {
                    if c.damage_assignment_needed {
                        return Err(EngineError::Illegal(
                            "must assign combat damage before combat damage resolves",
                        ));
                    }
                    self.resolve_combat_damage_step(ev)?;
                }
            }
            FirstStrikeDamage => {
                // CR 510.4: after first-strike damage and priority, the regular combat damage
                // step deals damage from remaining attackers/blockers (and double-strikers).
                self.resolve_combat_damage_step(ev)?;
            }
            CombatDamage => {
                self.clear_step_mana_pools();
                self.state.turn_step = EndCombat;
                if let Some(i) = self.state.player_idx(ap) {
                    self.state.priority_idx = i;
                }
                self.state.passes_since_stack_change = 0;
                ev.push(ev_phase(self, rv1::PhaseId::EndCombat));
                ev.push(ev_priority_changed(self));
            }
            EndCombat => {
                self.clear_all_mana_pools();
                // CR 511.3 removes creatures from combat when this step ends. Keep attacker
                // membership available to continuous effects through both damage steps and this
                // step (CR 613.5, 702.4b-c).
                self.state.combat = None;
                self.state.turn_step = Main2;
                if let Some(i) = self.state.player_idx(ap) {
                    self.state.priority_idx = i;
                }
                ev.push(ev_phase(self, rv1::PhaseId::Main2));
                self.fire_triggers(&[GameEvent::PhaseBegan {
                    phase: rv1::PhaseId::Main2,
                    active_player: ap,
                }]);
                self.flush_staged_triggers(ev);
                if self.state.blocking_choice().is_none() {
                    ev.push(ev_priority_changed(self));
                }
            }
            Main2 => {
                self.clear_all_mana_pools();
                if let Some(i) = self.state.player_idx(ap) {
                    self.state.priority_idx = i;
                }
                self.state.turn_step = EndStep;
                self.state.passes_since_stack_change = 0;
                ev.push(ev_phase(self, rv1::PhaseId::EndStep));
                self.fire_triggers(&[GameEvent::PhaseBegan {
                    phase: rv1::PhaseId::EndStep,
                    active_player: ap,
                }]);
                self.flush_staged_triggers(ev);
                if self.state.blocking_choice().is_none() {
                    ev.push(ev_priority_changed(self));
                }
            }
            EndStep => {
                self.clear_all_mana_pools();
                self.state.turn_step = Cleanup;
                self.state.passes_since_stack_change = 0;
                // No PhaseChanged: clients keep highlighting end step during engine cleanup (CR 514).
                // Carry the caller's accumulated events forward (consistent with the `Draw` branch);
                // shadowing `ev` with a fresh vec here would silently drop anything already in it.
                self.apply_sbas(ev)?;
                return self.start_cleanup_or_roll_turn(std::mem::take(ev));
            }
            Cleanup if self.state.cleanup_priority_active => {
                self.state.cleanup_priority_active = false;
                return self.start_cleanup_or_roll_turn(std::mem::take(ev));
            }
            _ => {
                self.clear_all_mana_pools();
                if let Some(i) = self.state.player_idx(ap) {
                    self.state.priority_idx = i;
                }
                self.state.passes_since_stack_change = 0;
                ev.push(ev_phase(self, rv1::PhaseId::Main1));
                ev.push(ev_priority_changed(self));
            }
        }
        if !self.state.combat_damage_priority_pending {
            self.apply_sbas(ev)?;
        }
        Ok(finish_with_events(self, std::mem::take(ev)))
    }

    pub(super) fn active_player_cleanup_discard_needed(&self) -> Option<PlayerId> {
        let active_player = self.state.players.get(self.state.active_player_idx)?;
        (active_player.hand.len() > self.maximum_hand_size(active_player.id))
            .then_some(active_player.id)
    }

    pub(super) fn start_cleanup_or_roll_turn(
        &mut self,
        mut ev: Vec<rv1::RuledEvent>,
    ) -> Result<RuledEventBatch, EngineError> {
        if let Some(pid) = self.active_player_cleanup_discard_needed() {
            self.state.cleanup_discard_player = Some(pid);
            if let Some(i) = self.state.player_idx(pid) {
                self.state.priority_idx = i;
            }
            self.state.passes_since_stack_change = 0;
            ev.push(ev_log(format!(
                "P{pid}: discard to hand size ({})",
                self.maximum_hand_size(pid)
            )));
            ev.push(ev_priority_changed(self));
            self.apply_sbas(&mut ev)?;
            return Ok(finish_with_events(self, ev));
        }
        self.state.cleanup_discard_player = None;
        self.finish_cleanup_roll_new_turn(ev)
    }

    pub(super) fn discard_to_hand_size(
        &mut self,
        player: PlayerId,
        d: &rv1::DiscardToHandSize,
    ) -> Result<RuledEventBatch, EngineError> {
        if self.state.turn_step != TurnStep::Cleanup {
            return Err(EngineError::Illegal("discard only during cleanup"));
        }
        if self.state.cleanup_discard_player != Some(player) {
            return Err(EngineError::Illegal("not your cleanup discard"));
        }
        let idx = self
            .state
            .player_idx(player)
            .ok_or(EngineError::UnknownPlayer(player))?;
        let hand_len = self.state.players[idx].hand.len();
        let maximum = self.maximum_hand_size(player);
        if hand_len <= maximum {
            return Err(EngineError::Illegal("hand size not over max"));
        }
        let must_discard = hand_len - maximum;
        let mut positions: Vec<usize> = d.hand_card_indices.iter().map(|&i| i as usize).collect();
        if positions.len() != must_discard {
            return Err(EngineError::Illegal("wrong discard count"));
        }
        positions.sort_unstable();
        positions.dedup();
        if positions.len() != must_discard {
            return Err(EngineError::Illegal("wrong discard count"));
        }
        for &hi in &positions {
            if hi >= hand_len {
                return Err(EngineError::Illegal("bad hand index"));
            }
        }
        let mut oids = Vec::with_capacity(positions.len());
        for &hi in &positions {
            let oid = *self.state.players[idx]
                .hand
                .get(hi)
                .ok_or(EngineError::Illegal("bad hand index"))?;
            oids.push(oid);
        }

        let mut ev = vec![];
        let mut discard_receipts = Vec::with_capacity(oids.len());
        for oid in oids {
            let owner = self
                .state
                .objects
                .get(&oid)
                .map(|o| o.owner)
                .ok_or(EngineError::Illegal("no object"))?;
            // CR 514.1: the turn-based action discards every excess card as one event.
            let (card_name, moved, discard_receipt) =
                self.commit_discard(player, oid, crate::state::DiscardCause::Cleanup, false)?;
            ev.push(ev_log(format!("P{player} discards {card_name} (cleanup)")));
            debug_assert_eq!(owner, player);
            ev.push(moved);
            discard_receipts.push(discard_receipt);
        }
        self.fire_discard_batches(vec![(player, discard_receipts)]);
        self.apply_sbas(&mut ev)?;
        if self.state.players[idx].hand.len() > self.maximum_hand_size(player) {
            ev.push(ev_priority_changed(self));
            return Ok(finish_with_events(self, ev));
        }
        self.state.cleanup_discard_player = None;
        self.finish_cleanup_roll_new_turn(ev)
    }

    /// After cleanup discards (514.1), apply 514.2-style clearing and advance the turn.
    pub(super) fn finish_cleanup_roll_new_turn(
        &mut self,
        mut ev: Vec<rv1::RuledEvent>,
    ) -> Result<RuledEventBatch, EngineError> {
        self.state.cleanup_discard_player = None;
        self.cleanup_until_end_of_turn_effects();
        let ending_turn_instance = self.state.turn_instance;
        self.state
            .active_exile_play_permissions
            .retain(|permission| {
                permission.expires_at_cleanup_turn_instance != Some(ending_turn_instance)
            });
        self.reindex_battlefield_control(&mut ev);
        self.cleanup_marked_damage();
        self.clear_all_mana_pools();
        self.flush_staged_triggers(&mut ev);
        if !self.state.stack.is_empty() || self.state.blocking_choice().is_some() {
            self.state.cleanup_priority_active = true;
            if let Some(index) = self.state.player_idx(self.state.active_player_id()) {
                self.state.priority_idx = index;
            }
            self.state.passes_since_stack_change = 0;
            if self.state.blocking_choice().is_none() {
                ev.push(ev_priority_changed(self));
            }
            return Ok(finish_with_events(self, ev));
        }
        self.state.cleanup_priority_active = false;
        self.state.lands_played_this_turn = 0;
        self.state.activation_uses_this_turn.clear();
        let ending_player = self.state.active_player_id();
        let expired = self
            .state
            .dispatch_event_observers(ObservedGameEvent::TurnEnded {
                active_player: ending_player,
                turn_instance: ending_turn_instance,
            });
        debug_assert!(expired.is_empty(), "turn-end observers only expire");
        self.state.turn_history.finish_turn();
        self.state.active_player_idx = self
            .state
            .next_in_game_player_idx(self.state.active_player_idx)
            .ok_or(EngineError::Illegal("no player in game"))?;
        if self.state.active_player_idx == 0 {
            self.state.turn = self.state.turn.saturating_add(1);
        }
        self.state.turn_instance = self.state.turn_instance.saturating_add(1);
        self.state.trigger_uses_this_turn.clear();
        let ap = self.state.active_player_id();
        // CR 500.4: "until your next turn" effects end as that turn begins, before the untap
        // step or any turn-begin observer sees the new turn. The resolving controller was captured
        // as a concrete player id, so extra turns and multiplayer turn order need no special case.
        self.state
            .continuous_effects
            .retain(|effect| effect.duration != EffectDuration::UntilTurnStart(ap));
        let armed = self
            .state
            .dispatch_event_observers(ObservedGameEvent::TurnBegan {
                active_player: ap,
                turn_instance: self.state.turn_instance,
            });
        debug_assert!(armed.is_empty(), "turn-begin observers only arm");
        self.state.turn_step = TurnStep::Untap;
        ev.push(ev_phase(self, rv1::PhaseId::Untap));

        // CR 502.3: membership, source availability, restrictions and replacement outcomes all
        // read the same boundary. No tapped/counter/skip/sickness mutation precedes preparation.
        let active_cohort = self
            .state
            .objects
            .values()
            .filter(|object| object.zone == Zone::Battlefield)
            .filter(|object| {
                self.characteristics(object.id)
                    .is_some_and(|c| c.controller == ap)
            })
            .map(|object| {
                (
                    object.id,
                    self.state
                        .zone_change_generation
                        .get(&object.id)
                        .copied()
                        .unwrap_or(0),
                )
            })
            .collect::<BTreeSet<_>>();
        let mut cohort = self.other_player_untap_cohort(ap);
        for &(oid, generation) in &active_cohort {
            if !self.state.skip_next_untap.contains(&(oid, generation))
                && !self.doesnt_untap_during_untap_step(oid)
            {
                cohort.insert(oid);
            }
        }
        let plans = cohort
            .into_iter()
            .filter_map(|oid| super::prepare_untap(self, oid))
            .collect::<Vec<_>>();
        for plan in plans {
            super::commit_untap(self, plan);
        }
        for (oid, generation) in active_cohort {
            self.state.skip_next_untap.remove(&(oid, generation));
            if self
                .state
                .zone_change_generation
                .get(&oid)
                .copied()
                .unwrap_or(0)
                == generation
            {
                if let Some(object) = self
                    .state
                    .objects
                    .get_mut(&oid)
                    .filter(|object| object.zone == Zone::Battlefield)
                {
                    object.summoning_sick = false;
                }
            }
        }
        // Servatrice only applies engine untaps during batches that include phase_changed("untap").
        // Emit zone_view in this same batch so battlefield_tapped reaches Cockatrice while
        // batchHasUntapPhase is still true (see Server_Game::applyRuledBatch).
        self.reconcile_activated_ability_slots();
        ev.push(self.ev_zone_view_sync_tracked());
        self.state.turn_step = TurnStep::Upkeep;
        ev.push(ev_phase(self, rv1::PhaseId::Upkeep));
        self.state.combat = None;
        if let Some(i) = self.state.player_idx(ap) {
            self.state.priority_idx = i;
        }
        self.state.passes_since_stack_change = 0;
        self.apply_sbas(&mut ev)?;
        ev.push(ev_log(format!("Turn {}: P{}", self.state.turn, ap)));
        // CR 503.1a: abilities that triggered at the beginning of the upkeep go on the stack
        // *before* the active player gets priority — hence before `ev_priority_changed`.
        //
        // This is the only place a normal turn roll passes through the upkeep: the step machine
        // walks Untap -> Upkeep inline above rather than giving anyone priority in the untap step
        // (CR 502.1, which has no priority). The `Untap` arm of `adv_on_empty_stack` fires the
        // same event for the paths that do stop there, and is unreachable from here.
        self.fire_triggers(&[GameEvent::PhaseBegan {
            phase: rv1::PhaseId::Upkeep,
            active_player: ap,
        }]);
        // The second of the two flush points: this path returns before `dispatch_command`'s tail,
        // so without it the first upkeep's triggers would sit staged until the next command.
        // Priority is withheld while an ordering or target choice is outstanding — CR 603.3b/603.3d
        // both resolve before any player receives priority.
        self.flush_staged_triggers(&mut ev);
        if self.state.blocking_choice().is_none() {
            ev.push(ev_priority_changed(self));
        }
        Ok(finish_with_events(self, ev))
    }

    pub(super) fn primitive_yield_structured(
        &mut self,
        player: PlayerId,
    ) -> Result<RuledEventBatch, EngineError> {
        if !self.state.stack.is_empty() {
            return Err(EngineError::Illegal("stack not empty"));
        }
        use TurnStep::*;
        match self.state.turn_step {
            DeclareAttackers => {
                if player != self.state.active_player_id() {
                    return Err(EngineError::Illegal("not active player"));
                }
                self.set_attackers(&[], player)
            }
            DeclareBlockers => {
                if let Some(c) = &self.state.combat {
                    if c.assign_combat_damage_phase {
                        return Err(EngineError::Illegal(
                            "cannot use structured yield during combat damage assignment",
                        ));
                    }
                    if c.blockers_declared {
                        return Err(EngineError::Illegal("blockers already declared"));
                    }
                }
                if !self.state.is_defending_player(player) {
                    return Err(EngineError::Illegal("not defending player"));
                }
                self.set_blockers(player, &[])
            }
            Untap | Upkeep | Draw | Main1 | BeginCombat | CombatDamage | EndCombat | Main2
            | EndStep => {
                if player != self.state.active_player_id() {
                    return Err(EngineError::Illegal("not active player"));
                }
                let mut ev = vec![];
                self.adv_on_empty_stack(&mut ev)
            }
            _ => Err(EngineError::Illegal(
                "primitive advance not supported in this step",
            )),
        }
    }
}
