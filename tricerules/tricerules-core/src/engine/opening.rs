use super::events::{ev_log, ev_phase, ev_priority_changed};
use super::legal_actions::fill_legal;
use super::replacement::{
    append_entry_counter_placement, BattlefieldEntryProgress, PendingReplacementEvent,
};
use super::resolution::{deal_opening_card, move_object_to_zone, permanent_moved_event};
use super::*;
use crate::state::next_unresolved_from;
use crate::state::{
    BattlefieldEntryCompletion, BattlefieldEntryEvent, EntryCounterReplacementResume,
    EntryCounterSource,
};

pub(crate) fn shuffle_player_library(state: &mut GameState, player_idx: usize, mix: u64) {
    let mut rng = rand::rngs::StdRng::seed_from_u64(mix);
    let mut v: Vec<ObjectId> = state.players[player_idx].library.iter().copied().collect();
    v.shuffle(&mut rng);
    state.players[player_idx].library = v.into_iter().collect();
}

/// Shuffle a player's library using the deterministic mix shared by effects resolved during an
/// accepted command. Keeping this derivation in one place preserves replay behavior for searches,
/// typed programs, and battlefield-to-library moves.
pub(crate) fn shuffle_player_library_for_current_command(state: &mut GameState, player: PlayerId) {
    let Some(idx) = state.player_idx(player) else {
        return;
    };
    let mut objects: Vec<ObjectId> = state.players[idx].library.iter().copied().collect();
    shuffle_object_ids_for_current_command(state, player, &mut objects);
    state.players[idx].library = objects.into_iter().collect();
}

/// Randomize only a bounded object cohort using the same replay-stable mix as a library shuffle.
/// This supports instructions such as Brightwood Tracker's random bottom order without shuffling
/// the complete library or creating a shuffle event.
pub(crate) fn shuffle_object_ids_for_current_command(
    state: &GameState,
    player: PlayerId,
    objects: &mut [ObjectId],
) {
    let mix = state
        .seed
        .wrapping_add(state.command_index.wrapping_mul(0x9E37_79B9_7F4A_7C15))
        ^ (player as u64);
    let mut rng = StdRng::seed_from_u64(mix);
    objects.shuffle(&mut rng);
}

fn mulligan_redraw(
    state: &mut GameState,
    registry: &'static CardRegistry,
    player: PlayerId,
) -> Result<(), EngineError> {
    let idx = state
        .player_idx(player)
        .ok_or(EngineError::UnknownPlayer(player))?;
    let hand: Vec<ObjectId> = state.players[idx].hand.drain(..).collect();
    for oid in hand {
        move_object_to_zone(state, registry, oid, Zone::Library, None)?;
    }
    shuffle_player_library(
        state,
        idx,
        state
            .seed
            .wrapping_add(state.command_index)
            .wrapping_add(player as u64),
    );
    for _ in 0..7 {
        deal_opening_card(&mut state.players[idx], &mut state.objects)?;
    }
    Ok(())
}

impl GameEngine {
    pub(super) fn reconcile_opening_departure(
        &mut self,
        player: PlayerId,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<(), EngineError> {
        let Some(index) = self.state.player_idx(player) else {
            return Ok(());
        };
        let next = self
            .state
            .next_in_game_player_idx(index)
            .ok_or(EngineError::Illegal("no player in game"))?;
        let next_player = self.state.players[next].id;
        // Retire this pregame transaction before handing the window to another seat.
        // Its entry/counter choices have committed neither the source nor the exile move.
        let abandoned = self
            .pending_opening_hand_entry()
            .and_then(|(event, completion)| {
                matches!(completion,BattlefieldEntryCompletion::OpeningHandAction {actor,source,..}
                if *actor==player && event.object_id==source.object_id)
                .then(|| event.clone())
            });
        if let Some(event) = abandoned {
            self.restore_abandoned_battlefield_entry(&event)?;
            self.state.pending_resolution = None;
            self.state.pending_replacement_event = None;
        }
        let Some(opening) = self.state.opening.as_mut() else {
            return Ok(());
        };
        opening.resolved[index] = true;
        if opening.chooser == player && opening.starting_player.is_none() {
            opening.chooser = next_player;
            self.state.priority_idx = next;
            events.push(ev_priority_changed(self));
            return Ok(());
        }
        if opening.starting_player == Some(player) {
            opening.starting_player = Some(next_player);
            self.state.starting_player_idx = next;
            self.state.active_player_idx = next;
        }
        if let Some(window) = opening.hand_window.as_mut() {
            window.finished[index] = true;
            if window.actor == player {
                Self::opening_advance_hand_window(self, next, events)?;
            }
            return Ok(());
        }
        if opening.mulligan_actor == Some(player)
            || opening
                .bottom
                .is_some_and(|(bottoming, _)| bottoming == player)
        {
            opening.mulligan_actor = None;
            opening.bottom = None;
            Self::opening_pick_next_or_finish(self, index, events)?;
        }
        Ok(())
    }

    pub(super) fn apply_opening_command(
        &mut self,
        player: PlayerId,
        cmd: &rv1::RuledCommand,
    ) -> Result<RuledEventBatch, EngineError> {
        use rv1::ruled_command::Cmd;
        if self.state.pending_resolution.is_some() {
            return Err(EngineError::Illegal(
                "finish the opening entry choice first",
            ));
        }
        let mut events = Vec::new();
        match cmd.cmd.as_ref() {
            Some(Cmd::ChooseStartingPlayer(ch)) => {
                let chooser = {
                    let op = self
                        .state
                        .opening
                        .as_ref()
                        .ok_or(EngineError::Illegal("opening"))?;
                    if op.starting_player.is_some() {
                        return Err(EngineError::Illegal("starting player already chosen"));
                    }
                    if player != op.chooser {
                        return Err(EngineError::Illegal("not your choice"));
                    }
                    op.chooser
                };
                let sp = ch.starting_player_id;
                self.state
                    .player_idx(sp)
                    .ok_or(EngineError::UnknownPlayer(sp))?;
                if self.state.players[self.state.player_idx(sp).unwrap()].has_lost {
                    return Err(EngineError::Illegal("starting player has left the game"));
                }
                {
                    let op = self
                        .state
                        .opening
                        .as_mut()
                        .ok_or(EngineError::Illegal("opening"))?;
                    op.starting_player = Some(sp);
                    op.original_starting_player = Some(sp);
                    op.mulligan_actor = Some(sp);
                }
                let sp_idx = self.state.player_idx(sp).unwrap();
                self.state.active_player_idx = sp_idx;
                self.state.priority_idx = sp_idx;
                self.state.starting_player_idx = sp_idx;
                for pi in 0..self.state.players.len() {
                    let p = &mut self.state.players[pi];
                    if p.has_lost {
                        continue;
                    }
                    for _ in 0..7 {
                        deal_opening_card(p, &mut self.state.objects)?;
                    }
                }
                events.push(ev_log(if chooser == sp {
                    format!("P{chooser} chooses to play first.")
                } else {
                    format!("P{chooser} chooses P{sp} to play first.")
                }));
                events.push(ev_phase(self, rv1::PhaseId::OpeningMulligan));
                events.push(ev_priority_changed(self));
            }
            Some(Cmd::Mulligan(md)) => {
                let actor = {
                    let op = self
                        .state
                        .opening
                        .as_ref()
                        .ok_or(EngineError::Illegal("opening"))?;
                    if op.bottom.is_some() {
                        return Err(EngineError::Illegal("finish bottoming first"));
                    }
                    op.mulligan_actor
                        .ok_or(EngineError::Illegal("no mulligan actor"))?
                };
                if player != actor {
                    return Err(EngineError::Illegal("not your mulligan decision"));
                }
                let idx = self.state.player_idx(player).unwrap();
                if md.keep {
                    let k = {
                        let op = self.state.opening.as_mut().unwrap();
                        op.mulligans_taken[idx]
                    };
                    // Free-for-all's first mulligan does not reduce the kept hand (CR 800.6).
                    let bottom_count = k.saturating_sub(u32::from(self.state.players.len() > 2));
                    if bottom_count == 0 {
                        {
                            let op = self.state.opening.as_mut().unwrap();
                            op.resolved[idx] = true;
                            op.mulligan_actor = None;
                        }
                        events.push(ev_log(format!(
                            "P{player} begins the game with 7 cards in hand."
                        )));
                        Self::opening_pick_next_or_finish(self, idx, &mut events)?;
                    } else {
                        {
                            let op = self.state.opening.as_mut().unwrap();
                            op.bottom = Some((player, bottom_count));
                            op.mulligan_actor = Some(player);
                        }
                        events.push(ev_priority_changed(self));
                    }
                } else {
                    let prev = {
                        let op = self.state.opening.as_mut().unwrap();
                        op.mulligans_taken[idx] += 1;
                        op.mulligans_taken[idx]
                    };
                    let bottom_count = prev.saturating_sub(u32::from(self.state.players.len() > 2));
                    mulligan_redraw(&mut self.state, self.registry, player)?;
                    if bottom_count >= MAX_HAND_SIZE as u32 {
                        // Mulliganed to 0 effective cards — auto-keep; go straight to bottom phase.
                        {
                            let op = self.state.opening.as_mut().unwrap();
                            op.bottom = Some((player, bottom_count));
                            op.mulligan_actor = Some(player);
                        }
                        events.push(ev_log(format!(
                            "P{player} mulliganed to 0 — automatically keeping; putting {bottom_count} card(s) on the bottom of their library."
                        )));
                        events.push(ev_priority_changed(self));
                        // Falls through to batch builder below (zone_view_sync added there).
                    } else {
                        events.push(ev_log(format!(
                            "P{player} mulligans to {} cards.",
                            7u32.saturating_sub(bottom_count)
                        )));
                        events.push(self.ev_zone_view_sync_tracked());
                        Self::opening_set_next_actor_after_mulligan(self, idx, &mut events)?;
                        let mut b = RuledEventBatch {
                            payment_preview: None,
                            resolution_prompt_state: None,
                            replace_stack_presentation: false,
                            events,
                            legal_by_player: Default::default(),
                        };
                        if self.state.opening.is_none() {
                            self.apply_sbas(&mut b.events)?;
                            self.flush_staged_triggers(&mut b.events);
                        }
                        fill_legal(&mut b, self);
                        return Ok(b);
                    }
                }
            }
            Some(Cmd::PutOpeningHandOnBottom(pb)) => {
                let idx = self.state.player_idx(player).unwrap();
                let (owner, rem_before) = {
                    let op = self
                        .state
                        .opening
                        .as_ref()
                        .ok_or(EngineError::Illegal("opening"))?;
                    let (bp, rem) = op
                        .bottom
                        .as_ref()
                        .ok_or(EngineError::Illegal("not bottoming"))?;
                    if *bp != player {
                        return Err(EngineError::Illegal("not your bottom step"));
                    }
                    (player, *rem)
                };
                let hi = pb.hand_card_index as usize;
                let oid = *self.state.players[idx]
                    .hand
                    .get(hi)
                    .ok_or(EngineError::Illegal("bad hand index"))?;
                move_object_to_zone(&mut self.state, self.registry, oid, Zone::Library, None)?;
                let rem_after = rem_before - 1;
                events.push(permanent_moved_event(
                    &self.state,
                    oid,
                    owner,
                    rv1::permanent_moved::Destination::Library,
                ));
                if rem_after > 0 {
                    events.push(ev_log(format!(
                        "P{player} puts a card on the bottom ({rem_after} more to place)."
                    )));
                    {
                        let op = self.state.opening.as_mut().unwrap();
                        op.bottom = Some((player, rem_after));
                    }
                } else {
                    let kept = self.state.players[idx].hand.len();
                    {
                        let op = self.state.opening.as_mut().unwrap();
                        op.bottom = None;
                        op.resolved[idx] = true;
                        op.mulligan_actor = None;
                        let total_mulls = op.mulligans_taken[idx]
                            .saturating_sub(u32::from(self.state.players.len() > 2));
                        events.push(ev_log(format!(
                            "P{player} puts {total_mulls} card(s) on the bottom of their library and begins the game with {kept} card(s) in their hand."
                        )));
                    }
                    Self::opening_pick_next_or_finish(self, idx, &mut events)?;
                }
            }
            Some(Cmd::FinishOpeningHandActions(_)) => {
                let index = self.state.player_idx(player).unwrap();
                let window = self
                    .state
                    .opening
                    .as_mut()
                    .and_then(|op| op.hand_window.as_mut())
                    .ok_or(EngineError::Illegal("not taking opening-hand actions"))?;
                if window.actor != player {
                    return Err(EngineError::Illegal("not your opening-hand actions"));
                }
                window.finished[index] = true;
                Self::opening_advance_hand_window(
                    self,
                    (index + 1) % self.state.players.len(),
                    &mut events,
                )?;
            }
            Some(Cmd::BeginWithGemstoneCaverns(command)) => {
                let reference = |value: &rv1::CostObjectRef| TriggerObjectRef {
                    object_id: value.object_id,
                    zone_change_generation: value.zone_change_generation,
                    controller_at_event: player,
                };
                let source = reference(
                    command
                        .source
                        .as_ref()
                        .ok_or(EngineError::Illegal("missing opening source"))?,
                );
                let exile = command.exile.as_ref().map(reference);
                self.validate_opening_hand_entry(player, source, exile)?;
                let card_id = self.state.objects[&source.object_id].card_id.clone();
                let item = StackItem {
                    source_zone_change: source.zone_change_generation,
                    ..StackItem::entry_context(
                        source.object_id,
                        player,
                        card_id,
                        "opening-hand action",
                    )
                };
                let mut event = BattlefieldEntryEvent::new(
                    source.object_id,
                    player,
                    self.player_life_snapshot(),
                );
                // Presentation of the accepted public proposed entry, not a CR 103.6b reveal
                // instruction. Only this source is announced; the later exile choice stays private.
                if let Some(mut reveal) = super::reveals::reveal_choice(
                    &self.state,
                    self.registry,
                    &[source.object_id],
                    source.object_id,
                    "Gemstone Caverns opening action",
                ) {
                    reveal.reveal_id = format!(
                        "opening:{}:{}:{}",
                        source.object_id, source.zone_change_generation, self.state.command_index
                    );
                    events.push(rv1::RuledEvent {
                        ev: Some(rv1::ruled_event::Ev::CardsRevealed(reveal.clone())),
                    });
                    event.entry_reveal_receipts.push(reveal);
                }
                append_entry_counter_placement(
                    &mut event,
                    player,
                    [(CounterKind::Luck, 1)],
                    EntryCounterSource::OpeningHandAction {
                        source_object_id: source.object_id,
                        source_zone_change_generation: source.zone_change_generation,
                    },
                    false,
                );
                match self.begin_battlefield_entry(
                    item,
                    event,
                    BattlefieldEntryCompletion::OpeningHandAction {
                        actor: player,
                        source,
                        exile,
                    },
                    &mut events,
                )? {
                    BattlefieldEntryProgress::Parked => {}
                    BattlefieldEntryProgress::Ready(event) => {
                        self.commit_opening_hand_entry(player, source, exile, *event, &mut events)?
                    }
                    BattlefieldEntryProgress::Skipped(event) => {
                        self.restore_skipped_battlefield_entry(&event)?;
                        self.continue_opening_hand_actions(player, &mut events)?;
                    }
                }
            }
            _ => return Err(EngineError::Illegal("illegal command during opening")),
        }
        let b = RuledEventBatch {
            payment_preview: None,
            resolution_prompt_state: None,
            replace_stack_presentation: false,
            events,
            legal_by_player: Default::default(),
        };
        self.settle_command_batch(b)
    }

    pub(super) fn pending_opening_hand_entry(
        &self,
    ) -> Option<(&BattlefieldEntryEvent, &BattlefieldEntryCompletion)> {
        if let Some(PendingReplacementEvent::BattlefieldEntry(entry)) =
            &self.state.pending_replacement_event
        {
            if matches!(
                entry.completion,
                BattlefieldEntryCompletion::OpeningHandAction { .. }
            ) {
                return Some((&entry.event, &entry.completion));
            }
        }
        if let Some(ResolutionContinuation::EntryCounterReplacement { resume }) = self
            .state
            .pending_resolution
            .as_ref()
            .map(|pending| &pending.continuation)
        {
            if let EntryCounterReplacementResume::Single {
                event, completion, ..
            } = resume.as_ref()
            {
                if matches!(
                    completion,
                    BattlefieldEntryCompletion::OpeningHandAction { .. }
                ) {
                    return Some((event, completion));
                }
            }
        }
        None
    }

    pub(super) fn validate_pending_opening_hand_entry(&self) -> Result<(), EngineError> {
        let (_, completion) = self
            .pending_opening_hand_entry()
            .ok_or(EngineError::Illegal("no opening entry continuation"))?;
        let BattlefieldEntryCompletion::OpeningHandAction {
            actor,
            source,
            exile,
        } = completion
        else {
            unreachable!()
        };
        self.validate_opening_hand_entry(*actor, *source, *exile)
    }

    fn validate_opening_hand_entry(
        &self,
        actor: PlayerId,
        source: TriggerObjectRef,
        exile: Option<TriggerObjectRef>,
    ) -> Result<(), EngineError> {
        let window = self
            .state
            .opening
            .as_ref()
            .and_then(|opening| opening.hand_window.as_ref())
            .ok_or(EngineError::Illegal("not taking opening-hand actions"))?;
        if window.actor != actor
            || self
                .state
                .player_idx(actor)
                .is_none_or(|index| self.state.players[index].has_lost)
        {
            return Err(EngineError::Illegal("not your opening-hand actions"));
        }
        let current = |reference: TriggerObjectRef| {
            self.opening_hand_cards(actor).iter().any(|card| {
                card.object.as_ref().is_some_and(|object| {
                    object.object_id == reference.object_id
                        && object.zone_change_generation == reference.zone_change_generation
                })
            })
        };
        if !current(source)
            || !self.opening_gemstone_cards(actor).iter().any(|card| {
                card.object
                    .as_ref()
                    .is_some_and(|object| object.object_id == source.object_id)
            })
        {
            return Err(EngineError::Illegal(
                "opening source is stale or ineligible",
            ));
        }
        let has_other = self.opening_hand_cards(actor).iter().any(|card| {
            card.object
                .as_ref()
                .is_some_and(|object| object.object_id != source.object_id)
        });
        if has_other != exile.is_some()
            || exile.is_some_and(|reference| {
                reference.object_id == source.object_id || !current(reference)
            })
        {
            return Err(EngineError::Illegal(
                "choose an exact other hand card to exile if available",
            ));
        }
        Ok(())
    }

    pub(super) fn commit_opening_hand_entry(
        &mut self,
        actor: PlayerId,
        source: TriggerObjectRef,
        exile: Option<TriggerObjectRef>,
        event: BattlefieldEntryEvent,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<(), EngineError> {
        self.validate_opening_hand_entry(actor, source, exile)?;
        if event.object_id != source.object_id || event.destination_controller != actor {
            return Err(EngineError::Illegal("opening entry identity mismatch"));
        }
        self.commit_battlefield_entry(event, None, events)?;
        events.push(permanent_moved_event(
            &self.state,
            source.object_id,
            actor,
            rv1::permanent_moved::Destination::Battlefield,
        ));
        if let Some(reference) = exile {
            move_object_to_zone(
                &mut self.state,
                self.registry,
                reference.object_id,
                Zone::Exile,
                None,
            )?;
            events.push(permanent_moved_event(
                &self.state,
                reference.object_id,
                actor,
                rv1::permanent_moved::Destination::Exile,
            ));
        }
        events.push(ev_log(format!(
            "P{actor} begins the game with Gemstone Caverns on the battlefield."
        )));
        self.continue_opening_hand_actions(actor, events)
    }

    pub(super) fn continue_opening_hand_actions(
        &mut self,
        actor: PlayerId,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<(), EngineError> {
        let index = self
            .state
            .player_idx(actor)
            .ok_or(EngineError::UnknownPlayer(actor))?;
        Self::opening_advance_hand_window(self, index, events)
    }

    pub(super) fn opening_hand_cards(&self, player: PlayerId) -> Vec<rv1::OpeningHandCard> {
        let Some(index) = self.state.player_idx(player) else {
            return vec![];
        };
        if self.state.players[index].has_lost {
            return vec![];
        }
        self.state.players[index]
            .hand
            .iter()
            .enumerate()
            .filter_map(|(slot, &id)| {
                let object = self.state.objects.get(&id)?;
                (object.zone == Zone::Hand && object.owner == player).then(|| {
                    rv1::OpeningHandCard {
                        hand_index: slot as u32,
                        object: Some(self.payment_object_ref(id)),
                    }
                })
            })
            .collect()
    }

    pub(super) fn opening_gemstone_cards(&self, player: PlayerId) -> Vec<rv1::OpeningHandCard> {
        let Some(opening) = &self.state.opening else {
            return vec![];
        };
        if opening.original_starting_player.is_none()
            || opening.original_starting_player == Some(player)
        {
            return vec![];
        }
        self.opening_hand_cards(player)
            .into_iter()
            .filter(|card| {
                card.object
                    .as_ref()
                    .and_then(|reference| self.state.objects.get(&reference.object_id))
                    .and_then(|object| self.registry.get(&object.card_id))
                    .is_some_and(|definition| {
                        definition
                            .primary_face()
                            .static_abilities
                            .iter()
                            .any(|ability| {
                                matches!(
                                    ability.definition,
                                    StaticAbilityDef::GemstoneCavernsOpening
                                )
                            })
                    })
            })
            .collect()
    }

    fn opening_advance_hand_window(
        eng: &mut GameEngine,
        start_index: usize,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<(), EngineError> {
        let count = eng.state.players.len();
        for offset in 0..count {
            let index = (start_index + offset) % count;
            let player = eng.state.players[index].id;
            let finished = eng
                .state
                .opening
                .as_ref()
                .and_then(|op| op.hand_window.as_ref())
                .ok_or(EngineError::Illegal("no opening-hand window"))?
                .finished[index];
            if finished {
                continue;
            }
            if eng.opening_gemstone_cards(player).is_empty() {
                eng.state
                    .opening
                    .as_mut()
                    .unwrap()
                    .hand_window
                    .as_mut()
                    .unwrap()
                    .finished[index] = true;
                continue;
            }
            eng.state
                .opening
                .as_mut()
                .unwrap()
                .hand_window
                .as_mut()
                .unwrap()
                .actor = player;
            eng.state.priority_idx = index;
            // The existing opening phase covers all pre-turn procedures. The structured
            // OpeningActions stage distinguishes these actions from mulligan/bottoming.
            events.push(ev_phase(eng, rv1::PhaseId::OpeningMulligan));
            events.push(ev_priority_changed(eng));
            return Ok(());
        }
        let starting = eng
            .state
            .opening
            .take()
            .unwrap()
            .starting_player
            .ok_or(EngineError::Illegal("opening has no first live player"))?;
        let index = eng.state.player_idx(starting).unwrap();
        eng.state.active_player_idx = index;
        eng.state.priority_idx = index;
        eng.state.starting_player_idx = index;
        eng.state.turn_step = TurnStep::Upkeep;
        eng.state.turn = 1;
        events.push(ev_phase(eng, rv1::PhaseId::Upkeep));
        events.push(ev_priority_changed(eng));
        Ok(())
    }

    /// After a mulligan (redraw): offer the next unresolved player a decision —
    /// then the mulliganing player decides again (CR-style table flow for this fork).
    fn opening_set_next_actor_after_mulligan(
        eng: &mut GameEngine,
        mulliganed_idx: usize,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<(), EngineError> {
        let seats = eng.state.players.len();
        let next_idx = {
            let op = eng
                .state
                .opening
                .as_mut()
                .ok_or(EngineError::Illegal("opening"))?;
            // The next seat that has not kept yet, starting after the one that just mulliganed; if
            // everyone else has kept, that player decides again.
            next_unresolved_from(&op.resolved, (mulliganed_idx + 1) % seats)
                .unwrap_or(mulliganed_idx)
        };
        let pid = eng.state.players[next_idx].id;
        {
            let op = eng.state.opening.as_mut().unwrap();
            op.mulligan_actor = Some(pid);
        }
        eng.state.priority_idx = next_idx;
        events.push(ev_phase(eng, rv1::PhaseId::OpeningMulligan));
        events.push(ev_priority_changed(eng));
        Ok(())
    }

    fn opening_pick_next_or_finish(
        eng: &mut GameEngine,
        previous_idx: usize,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<(), EngineError> {
        let done = {
            let op = eng.state.opening.as_ref().unwrap();
            op.resolved.iter().all(|&r| r)
        };
        if done {
            let original = eng
                .state
                .opening
                .as_ref()
                .unwrap()
                .original_starting_player
                .ok_or(EngineError::Illegal("missing original starting player"))?;
            let index = eng.state.player_idx(original).unwrap();
            eng.state.opening.as_mut().unwrap().hand_window =
                Some(crate::state::OpeningHandWindow {
                    actor: original,
                    finished: vec![false; eng.state.players.len()],
                });
            return Self::opening_advance_hand_window(eng, index, events);
        }
        {
            let op = eng.state.opening.as_mut().unwrap();
            // Continue around the table before returning to a player who mulliganed.
            let next = (previous_idx + 1) % eng.state.players.len();
            if let Some(oi) = next_unresolved_from(&op.resolved, next) {
                let pid = eng.state.players[oi].id;
                op.mulligan_actor = Some(pid);
                eng.state.priority_idx = oi;
                events.push(ev_phase(eng, rv1::PhaseId::OpeningMulligan));
                events.push(ev_priority_changed(eng));
            }
        }
        Ok(())
    }
}
