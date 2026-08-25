use super::events::{ev_game_over, ev_log, ev_phase, ev_priority_changed, finish_with_events};
use super::legal_actions::fill_legal;
use super::resolution::{draw_card, perform_discard};
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
    pub(super) fn sweep_life(&mut self) {
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
        if still_in.len() == 1 {
            self.state.winner = Some(still_in[0]);
        }
    }

    pub(super) fn concede_batch(
        &mut self,
        player: PlayerId,
    ) -> Result<RuledEventBatch, EngineError> {
        // CR 104.3a ends this two-player game immediately. Drop any parked resolution so hidden
        // choices staged before the concession can neither be published nor committed afterward.
        self.state.pending_resolution = None;
        for p in &mut self.state.players {
            if p.id == player {
                p.has_lost = true;
            }
        }
        for p in &self.state.players {
            if p.id != player {
                self.state.winner = Some(p.id);
                break;
            }
        }
        let mut batch = RuledEventBatch::default();
        batch.events.push(ev_log(format!("P{player} conceded")));
        if let Some(winner) = self.state.winner {
            batch.events.push(ev_game_over(winner));
        }
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
        let n = self.state.players.len() as u32;
        if !self.state.stack.is_empty() {
            return self.pass_priority_on_stack(player, n);
        }
        // empty stack
        self.state.passes_since_stack_change += 1;
        self.state.priority_idx = (self.state.priority_idx + 1) % self.state.players.len();
        let ev = vec![rv1::RuledEvent {
            ev: Some(rv1::ruled_event::Ev::PriorityChanged(
                rv1::PriorityChanged {
                    player_id: self.state.priority_player_id(),
                },
            )),
        }];
        if self.state.passes_since_stack_change < n {
            let mut batch = RuledEventBatch {
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
        self.state.priority_idx =
            (self.state.player_idx(player).unwrap() + 1) % self.state.players.len();
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
        self.resolve_top_of_stack(&mut ev)?;
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
                if let Some(i) = self.state.player_idx(ap) {
                    self.state.priority_idx = i;
                }
                ev.push(ev_phase(self, rv1::PhaseId::Draw));
                // First draw step of the duel: only the starting player skips (CR 103.8). `turn`
                // may stay 1 for the second seat's first turn because we bump `turn` when wrapping
                // to seat 0, not on every active change.
                let skip_opening_draw = self.state.turn == 1
                    && self.state.active_player_idx == self.state.starting_player_idx;
                if skip_opening_draw {
                    // skip draw
                } else if let Some(idx) = self.state.player_idx(ap) {
                    if self.state.players[idx].library.is_empty() {
                        for p in &mut self.state.players {
                            p.has_lost = p.id == ap;
                        }
                        for p in &self.state.players {
                            if p.id != ap {
                                self.state.winner = Some(p.id);
                            }
                        }
                        ev.push(ev_log("Game over: empty library on draw".into()));
                        return Ok(finish_with_events(self, std::mem::take(ev)));
                    }
                    draw_card(&mut self.state.players[idx], &mut self.state.objects)?;
                    self.fire_card_drawn(ap);
                }
                self.state.passes_since_stack_change = 0;
                // CR 504.2: draw-step triggers go on the stack only after the turn-based draw
                // above. Unreachable when the active player just decked out — that path returns.
                self.fire_triggers(&[GameEvent::PhaseBegan {
                    phase: rv1::PhaseId::Draw,
                    active_player: ap,
                }]);
                self.flush_staged_triggers(ev);
                if self.state.blocking_choice().is_none() {
                    ev.push(ev_priority_changed(self));
                }
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
                ev.push(ev_priority_changed(self));
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
                self.clear_all_mana_pools();
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
                self.clear_all_mana_pools();
                self.state.passes_since_stack_change = 0;
                let has_eligible_blockers = self.defending_player_has_eligible_blockers();
                let has_attackers = self
                    .state
                    .combat
                    .as_ref()
                    .is_some_and(|c| !c.attacking.is_empty());
                if !has_eligible_blockers || !has_attackers {
                    // Auto-declare empty blockers; active player gets priority in DeclareBlockers.
                    if let Some(c) = self.state.combat.as_mut() {
                        c.blockers.clear();
                        c.damage_assignments.clear();
                        c.damage_assignment_needed = false;
                        c.assign_combat_damage_phase = false;
                        c.blockers_declared = true;
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
                    if let Some(d) = self.state.defending_player_ids().first().copied() {
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
                        self.clear_all_mana_pools();
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
                self.clear_all_mana_pools();
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
        self.apply_sbas(ev)?;
        Ok(finish_with_events(self, std::mem::take(ev)))
    }

    pub(super) fn active_player_cleanup_discard_needed(&self) -> Option<PlayerId> {
        let active_player = self.state.players.get(self.state.active_player_idx)?;
        (active_player.hand.len() > MAX_HAND_SIZE).then_some(active_player.id)
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
                "P{pid}: discard to hand size ({MAX_HAND_SIZE})"
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
        if hand_len <= MAX_HAND_SIZE {
            return Err(EngineError::Illegal("hand size not over max"));
        }
        let must_discard = hand_len - MAX_HAND_SIZE;
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
        for oid in oids {
            let owner = self
                .state
                .objects
                .get(&oid)
                .map(|o| o.owner)
                .ok_or(EngineError::Illegal("no object"))?;
            let (card_name, moved) = perform_discard(&mut self.state, self.registry, player, oid)?;
            ev.push(ev_log(format!("P{player} discards {card_name} (cleanup)")));
            debug_assert_eq!(owner, player);
            ev.push(moved);
        }
        self.apply_sbas(&mut ev)?;
        if self.state.players[idx].hand.len() > MAX_HAND_SIZE {
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
        self.state.turn_history.finish_turn();
        let n = self.state.players.len();
        if n >= 1 {
            self.state.active_player_idx = (self.state.active_player_idx + 1) % n;
        }
        if self.state.active_player_idx == 0 {
            self.state.turn = self.state.turn.saturating_add(1);
        }
        self.state.turn_instance = self.state.turn_instance.saturating_add(1);
        let ap = self.state.active_player_id();
        self.state.turn_step = TurnStep::Untap;
        ev.push(ev_phase(self, rv1::PhaseId::Untap));

        // CR 502.1 / 502.3: in their untap step the active player untaps the permanents they
        // control and any creature they've controlled since their last turn began loses summoning
        // sickness. Scope to the active player's battlefield, not every object they *own*: untapping
        // is a battlefield-only action (cards in hand/graveyard/library/exile have no tap state),
        // and a permanent is untapped by its controller, not its owner — untapping by owner would
        // wrongly untap a permanent its owner no longer controls and skip one they took control of.
        // The battlefield zone list *is* the control list (see `GameObject::controller`), so this
        // is correct even when a permanent is controlled by someone other than its owner.
        if let Some(idx) = self.state.player_idx(ap) {
            for &oid in &self.state.players[idx].battlefield.clone() {
                let generation = self
                    .state
                    .zone_change_generation
                    .get(&oid)
                    .copied()
                    .unwrap_or(0);
                let skip_untap = self.state.skip_next_untap.remove(&(oid, generation));
                let doesnt_untap = self.doesnt_untap_during_untap_step(oid);
                if !skip_untap && !doesnt_untap {
                    super::attempt_untap(&mut self.state, oid);
                }
                if let Some(c) = self.state.objects.get_mut(&oid) {
                    c.summoning_sick = false;
                }
            }
        }
        // Servatrice only applies engine untaps during batches that include phase_changed("untap").
        // Emit zone_view in this same batch so battlefield_tapped reaches Cockatrice while
        // batchHasUntapPhase is still true (see Server_Game::applyRuledBatch).
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
                self.set_blockers(&[])
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
