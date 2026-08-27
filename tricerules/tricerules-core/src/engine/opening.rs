use super::events::{ev_log, ev_phase, ev_priority_changed};
use super::legal_actions::fill_legal;
use super::resolution::{draw_card, move_object_to_zone, permanent_moved_event};
use super::*;
use crate::state::next_unresolved_from;

pub(crate) fn shuffle_player_library(state: &mut GameState, player_idx: usize, mix: u64) {
    let mut rng = rand::rngs::StdRng::seed_from_u64(mix);
    let mut v: Vec<ObjectId> = state.players[player_idx].library.iter().copied().collect();
    v.shuffle(&mut rng);
    state.players[player_idx].library = v.into_iter().collect();
}

/// Shuffle a player's library using the deterministic mix shared by effects resolved during an
/// accepted command. Keeping this derivation in one place preserves replay behavior for searches,
/// custom effects, and battlefield-to-library moves.
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
        draw_card(&mut state.players[idx], &mut state.objects)?;
    }
    Ok(())
}

impl GameEngine {
    pub(super) fn apply_opening_command(
        &mut self,
        player: PlayerId,
        cmd: &rv1::RuledCommand,
    ) -> Result<RuledEventBatch, EngineError> {
        use rv1::ruled_command::Cmd;
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
                {
                    let op = self
                        .state
                        .opening
                        .as_mut()
                        .ok_or(EngineError::Illegal("opening"))?;
                    op.starting_player = Some(sp);
                    op.mulligan_actor = Some(sp);
                }
                let sp_idx = self.state.player_idx(sp).unwrap();
                self.state.active_player_idx = sp_idx;
                self.state.priority_idx = sp_idx;
                self.state.starting_player_idx = sp_idx;
                for pi in 0..self.state.players.len() {
                    let p = &mut self.state.players[pi];
                    for _ in 0..7 {
                        draw_card(p, &mut self.state.objects)?;
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
                    if k == 0 {
                        {
                            let op = self.state.opening.as_mut().unwrap();
                            op.resolved[idx] = true;
                            op.mulligan_actor = None;
                        }
                        events.push(ev_log(format!(
                            "P{player} begins the game with 7 cards in hand."
                        )));
                        Self::opening_pick_next_or_finish(self, &mut events)?;
                    } else {
                        {
                            let op = self.state.opening.as_mut().unwrap();
                            op.bottom = Some((player, k));
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
                    mulligan_redraw(&mut self.state, self.registry, player)?;
                    if prev >= MAX_HAND_SIZE as u32 {
                        // Mulliganed to 0 effective cards — auto-keep; go straight to bottom phase.
                        {
                            let op = self.state.opening.as_mut().unwrap();
                            op.bottom = Some((player, prev));
                            op.mulligan_actor = Some(player);
                        }
                        events.push(ev_log(format!(
                            "P{player} mulliganed to 0 — automatically keeping; putting {prev} card(s) on the bottom of their library."
                        )));
                        events.push(ev_priority_changed(self));
                        // Falls through to batch builder below (zone_view_sync added there).
                    } else {
                        events.push(ev_log(format!(
                            "P{player} mulligans to {} cards.",
                            7u32.saturating_sub(prev)
                        )));
                        events.push(self.ev_zone_view_sync_tracked());
                        Self::opening_set_next_actor_after_mulligan(self, idx, &mut events)?;
                        let mut b = RuledEventBatch {
                            spell_payment_preview: None,
                            events,
                            legal_by_player: Default::default(),
                        };
                        self.apply_sbas(&mut b.events)?;
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
                        let total_mulls = op.mulligans_taken[idx];
                        events.push(ev_log(format!(
                            "P{player} puts {total_mulls} card(s) on the bottom of their library and begins the game with {kept} card(s) in their hand."
                        )));
                    }
                    Self::opening_pick_next_or_finish(self, &mut events)?;
                }
            }
            _ => return Err(EngineError::Illegal("illegal command during opening")),
        }
        events.push(self.ev_zone_view_sync_tracked());
        let mut b = RuledEventBatch {
            spell_payment_preview: None,
            events,
            legal_by_player: Default::default(),
        };
        self.apply_sbas(&mut b.events)?;
        fill_legal(&mut b, self);
        Ok(b)
    }

    /// After a mulligan (redraw): alternate to the other player unless they have already kept —
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
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<(), EngineError> {
        let done = {
            let op = eng.state.opening.as_ref().unwrap();
            op.resolved.iter().all(|&r| r)
        };
        if done {
            let sp = {
                let op = eng.state.opening.take().unwrap();
                op.starting_player.ok_or(EngineError::Illegal("opening?"))?
            };
            let sp_idx = eng.state.player_idx(sp).unwrap();
            eng.state.active_player_idx = sp_idx;
            eng.state.priority_idx = sp_idx;
            eng.state.starting_player_idx = sp_idx;
            eng.state.turn_step = TurnStep::Upkeep;
            eng.state.turn = 1;
            events.push(ev_phase(eng, rv1::PhaseId::Upkeep));
            events.push(ev_priority_changed(eng));
            return Ok(());
        }
        {
            let spid = {
                let op = eng.state.opening.as_ref().unwrap();
                op.starting_player
                    .ok_or(EngineError::Illegal("opening not started"))?
            };
            let start = eng.state.player_idx(spid).unwrap();
            let op = eng.state.opening.as_mut().unwrap();
            // Turn order from the starting player (CR 103.4).
            if let Some(oi) = next_unresolved_from(&op.resolved, start) {
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
