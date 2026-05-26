//! Core rules processing (vanilla core ΓÇö simplified combat & mana).

use crate::state::{
    CombatState, GameObject, GameState, ObjectId, OpeningSequence, PlayerId, PlayerState,
    StackItem, TurnStep, Zone,
};
use prost::Message;
use rand::rngs::StdRng;
use rand::seq::SliceRandom;
use rand::SeedableRng;
use std::collections::{HashMap, HashSet};
use thiserror::Error;
use tricerules_cards::primitives::{SpellEffectKind, TargetSpec};
use tricerules_cards::CardRegistry;
use tricerules_proto::ruled::v1 as rv1;
use tricerules_proto::ruled::v1::{
    IpcResponse, LegalActions, RuledCommand, RuledEvent, RuledEventBatch,
};

/// CR 514.1: default maximum hand size (Reliquary Tower–style overrides not modeled yet).
const MAX_HAND_SIZE: usize = 7;

/// Sorcery-speed window: your main phase, stack empty, you are the active player (CR 307.5,
/// 601.2; lands CR 305.3).
fn sorcery_speed_available(state: &GameState, player: PlayerId) -> bool {
    matches!(state.turn_step, TurnStep::Main1 | TurnStep::Main2)
        && state.stack.is_empty()
        && player == state.active_player_id()
}

fn instant_timing_step_allowed(step: TurnStep) -> bool {
    matches!(
        step,
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
    )
}

fn shuffle_player_library(state: &mut GameState, player_idx: usize, mix: u64) {
    let mut rng = StdRng::seed_from_u64(mix);
    let mut v: Vec<ObjectId> = state.players[player_idx].library.iter().copied().collect();
    v.shuffle(&mut rng);
    state.players[player_idx].library = v.into_iter().collect();
}

fn mulligan_redraw(state: &mut GameState, player: PlayerId) -> Result<(), EngineError> {
    let idx = state
        .player_idx(player)
        .ok_or(EngineError::UnknownPlayer(player))?;
    let hand: Vec<ObjectId> = state.players[idx].hand.drain(..).collect();
    for oid in hand {
        move_object_to_zone(state, oid, Zone::Library)?;
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

#[derive(Debug, Error)]
pub enum EngineError {
    #[error("unknown player {0}")]
    UnknownPlayer(PlayerId),
    #[error("illegal command: {0}")]
    Illegal(&'static str),
    #[error("missing card data {0}")]
    MissingCard(String),
    #[error("player {0} won")]
    GameOver(PlayerId),
}

pub struct GameEngine {
    pub state: GameState,
    registry: CardRegistry,
}

impl GameEngine {
    fn clear_all_mana_pools(&mut self) {
        for p in &mut self.state.players {
            p.mana_pool.clear();
        }
    }

    /// Optional `decks` per player (tricerules id strings); if missing/empty, uses the default M2 test deck.
    ///
    /// When `skip_opening_sequence` is true (scenario tests), opening hands are dealt immediately
    /// like the legacy engine (no choose-first / mulligan prompts).
    pub fn new(
        seed: u64,
        player_ids: &[PlayerId],
        starting_life: i32,
        decks: Option<Vec<Vec<String>>>,
        skip_opening_sequence: bool,
    ) -> Result<Self, EngineError> {
        if player_ids.len() != 2 {
            return Err(EngineError::Illegal("M2: exactly 2 players"));
        }
        let registry =
            CardRegistry::from_embedded().map_err(|_| EngineError::Illegal("bad registry"))?;
        let mut objects = HashMap::new();
        // Player targets in commands use raw `PlayerId` values as `TargetRef.object_id`. Game
        // objects must use disjoint ids so e.g. P1 (id 1) is never confused with object id 1.
        let max_pid: i32 = player_ids.iter().copied().max().unwrap_or(0);
        let mut next_object_id: ObjectId = (max_pid.max(0) as u32).saturating_add(1);
        let mut players = Vec::new();

        for (i, &pid) in player_ids.iter().enumerate() {
            let mut p = PlayerState::new(pid, starting_life);
            let deck_list: Vec<String> = match &decks {
                Some(d) if i < d.len() && !d[i].is_empty() => d[i].clone(),
                _ => default_deck_list(i),
            };
            for card_id in deck_list {
                let def = registry
                    .get(&card_id)
                    .ok_or_else(|| EngineError::MissingCard(card_id.clone()))?;
                let oid = next_object_id;
                next_object_id += 1;
                objects.insert(
                    oid,
                    GameObject {
                        id: oid,
                        owner: pid,
                        card_id: card_id.clone(),
                        zone: Zone::Library,
                        tapped: false,
                        summoning_sick: def.is_creature,
                        power: def.power,
                        toughness: def.toughness,
                        damage: 0,
                        plus_one_plus_one: 0,
                        minus_one_minus_one: 0,
                    },
                );
                p.library.push_back(oid);
            }
            let mut rng = StdRng::seed_from_u64(
                seed.wrapping_add(i as u64)
                    .wrapping_mul(0x9E37_79B9_7F4A_7C15),
            );
            let mut lib: Vec<ObjectId> = p.library.iter().copied().collect();
            lib.shuffle(&mut rng);
            p.library = lib.into_iter().collect();
            if skip_opening_sequence {
                for _ in 0..7 {
                    draw_card(&mut p, &mut objects)?;
                }
            }
            players.push(p);
        }

        let opening = if skip_opening_sequence {
            None
        } else {
            let chooser = player_ids[(seed as usize).wrapping_rem(player_ids.len())];
            Some(OpeningSequence {
                chooser,
                starting_player: None,
                mulligan_actor: None,
                bottom: None,
                mulligans_taken: [0, 0],
                resolved: [false, false],
            })
        };
        let chooser_idx = opening
            .as_ref()
            .and_then(|o| player_ids.iter().position(|&id| id == o.chooser))
            .unwrap_or(0);

        let state = GameState {
            seed,
            players,
            objects,
            stack: Vec::new(),
            priority_idx: if skip_opening_sequence {
                0
            } else {
                chooser_idx
            },
            active_player_idx: if skip_opening_sequence {
                0
            } else {
                chooser_idx
            },
            turn_step: TurnStep::Upkeep,
            turn: 1,
            next_object_id,
            command_index: 0,
            passes_since_stack_change: 0,
            land_dropped_this_turn: false,
            combat: None,
            winner: None,
            cleanup_discard_player: None,
            opening,
            starting_player_idx: 0,
        };
        let mut eng = GameEngine { state, registry };
        let mut e = vec![];
        let _ = eng.apply_sbas(&mut e);
        Ok(eng)
    }

    pub fn new_with_default_decks(
        seed: u64,
        player_ids: &[PlayerId],
        starting_life: i32,
    ) -> Result<Self, EngineError> {
        Self::new(seed, player_ids, starting_life, None, true)
    }

    fn apply_opening_command(
        &mut self,
        player: PlayerId,
        cmd: &RuledCommand,
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
                events.push(ev_phase_labeled(self, "opening_mulligan"));
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
                    mulligan_redraw(&mut self.state, player)?;
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
                        events.push(self.ev_zone_view_sync());
                        Self::opening_set_next_actor_after_mulligan(self, idx, &mut events)?;
                        let mut b = RuledEventBatch {
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
                move_object_to_zone(&mut self.state, oid, Zone::Library)?;
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
        events.push(self.ev_zone_view_sync());
        let mut b = RuledEventBatch {
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
        let other_idx = 1 - mulliganed_idx;
        let next_idx = {
            let op = eng
                .state
                .opening
                .as_mut()
                .ok_or(EngineError::Illegal("opening"))?;
            if op.resolved[other_idx] {
                mulliganed_idx
            } else {
                other_idx
            }
        };
        let pid = eng.state.players[next_idx].id;
        {
            let op = eng.state.opening.as_mut().unwrap();
            op.mulligan_actor = Some(pid);
        }
        eng.state.priority_idx = next_idx;
        events.push(ev_phase_labeled(eng, "opening_mulligan"));
        events.push(ev_priority_changed(eng));
        Ok(())
    }

    fn opening_pick_next_or_finish(
        eng: &mut GameEngine,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<(), EngineError> {
        let done = {
            let op = eng.state.opening.as_ref().unwrap();
            op.resolved[0] && op.resolved[1]
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
            events.push(ev_phase_labeled(eng, "upkeep"));
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
            let order = [start, 1 - start];
            let op = eng.state.opening.as_mut().unwrap();
            for oi in order {
                if !op.resolved[oi] {
                    let pid = eng.state.players[oi].id;
                    op.mulligan_actor = Some(pid);
                    eng.state.priority_idx = oi;
                    events.push(ev_phase_labeled(eng, "opening_mulligan"));
                    events.push(ev_priority_changed(eng));
                    break;
                }
            }
        }
        Ok(())
    }

    pub fn apply_command(
        &mut self,
        player: PlayerId,
        cmd: &RuledCommand,
    ) -> Result<RuledEventBatch, EngineError> {
        if self.state.winner.is_some() {
            return Err(EngineError::Illegal("game over"));
        }
        use rv1::ruled_command::Cmd;
        if matches!(
            cmd.cmd.as_ref(),
            Some(Cmd::PreviewDeclareBlockers(_) | Cmd::PreviewDeclareAttackers(_))
        ) {
            return Err(EngineError::Illegal("preview is not a game command"));
        }
        self.state.command_index += 1;
        if self.state.opening.is_some() {
            return self.apply_opening_command(player, cmd);
        }
        let res = match cmd.cmd.as_ref() {
            None => return Err(EngineError::Illegal("empty command")),
            Some(Cmd::PreviewDeclareBlockers(_) | Cmd::PreviewDeclareAttackers(_)) => {
                unreachable!("preview rejected before command_index bump")
            }
            Some(Cmd::Mulligan(_)) => {
                return Err(EngineError::Illegal("mulligan only during opening"));
            }
            Some(Cmd::ChooseStartingPlayer(_)) | Some(Cmd::PutOpeningHandOnBottom(_)) => {
                return Err(EngineError::Illegal("opening-only command"));
            }
            Some(Cmd::Concede(_)) => return self.concede_batch(player),
            Some(Cmd::DeclareAttackers(a)) => {
                if self.state.turn_step != TurnStep::DeclareAttackers
                    || self.state.active_player_id() != player
                {
                    return Err(EngineError::Illegal("declare attackers not legal"));
                }
                self.set_attackers(&a.creature_ids, player)
            }
            Some(Cmd::DeclareBlockers(b)) => {
                if self.state.turn_step != TurnStep::DeclareBlockers
                    || Some(player) != self.state.defending_player_id_1v1()
                {
                    return Err(EngineError::Illegal("declare blockers not legal"));
                }
                self.set_blockers(&b.block_pairs)
            }
            Some(Cmd::PassPriority(_)) => {
                if self.state.turn_step == TurnStep::DeclareAttackers
                    && self.state.active_player_id() == player
                    && self
                        .state
                        .combat
                        .as_ref()
                        .map(|c| !c.attackers_declared)
                        .unwrap_or(false)
                {
                    self.set_attackers(&[], player)
                } else if self.state.turn_step == TurnStep::DeclareBlockers
                    && Some(player) == self.state.defending_player_id_1v1()
                    && self
                        .state
                        .combat
                        .as_ref()
                        .map(|c| !c.blockers_declared)
                        .unwrap_or(false)
                {
                    self.set_blockers(&[])
                } else {
                    self.pass_priority(player)
                }
            }
            Some(Cmd::PrimitiveYieldStructured(_)) => self.primitive_yield_structured(player),
            Some(Cmd::CastSpell(cs)) => {
                self.cast_spell(player, cs.hand_card_index as usize, &cs.targets)
            }
            Some(Cmd::PlayLand(pl)) => self.play_land(player, pl.hand_card_index as usize),
            Some(Cmd::AddManaToPool(m)) => self.add_mana_to_pool(player, m),
            Some(Cmd::DiscardToHandSize(d)) => self.discard_to_hand_size(player, d),
            Some(Cmd::AssignCombatDamage(acd)) => {
                if self.state.active_player_id() != player {
                    return Err(EngineError::Illegal("not active player"));
                }
                let pairs: Vec<(ObjectId, u32)> = acd
                    .assignments
                    .iter()
                    .map(|p| (p.blocker_id, p.damage))
                    .collect();
                self.assign_combat_damage(acd.attacker_id, &pairs)
            }
        };
        let mut b = res?;
        self.sweep_life();
        let mut d = vec![];
        self.apply_sbas(&mut d)?;
        b.events.extend(d);
        b.events.push(self.ev_zone_view_sync());
        fill_legal(&mut b, self);
        Ok(b)
    }

    fn sweep_life(&mut self) {
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

    /// Returns false if `blocker_id` is not permitted to block `attacker_id` due to
    /// keyword evasion abilities. Checks all active blocking restrictions in order.
    fn can_block(&self, attacker_id: ObjectId, blocker_id: ObjectId) -> bool {
        use tricerules_cards::Keyword;
        let Some(att) = self.state.objects.get(&attacker_id) else {
            return false;
        };
        let Some(blk) = self.state.objects.get(&blocker_id) else {
            return false;
        };

        // CR 702.9b — flying: can only be blocked by creatures with flying or reach.
        if att.has_keyword(&self.registry, Keyword::Flying)
            && !blk.has_keyword(&self.registry, Keyword::Flying)
            && !blk.has_keyword(&self.registry, Keyword::Reach)
        {
            return false;
        }

        // CR 702.13b — intimidate: can only be blocked by artifact creatures and/or
        // creatures that share a color with the intimidate creature.
        if att.has_keyword(&self.registry, Keyword::Intimidate) {
            let blk_def = self.registry.get(&blk.card_id);
            let blk_is_artifact = blk_def.map(|d| d.is_artifact).unwrap_or(false);
            if !blk_is_artifact {
                let att_colors = self
                    .registry
                    .get(&att.card_id)
                    .map(|d| d.colors())
                    .unwrap_or_default();
                let blk_colors = blk_def.map(|d| d.colors()).unwrap_or_default();
                let shares_color = att_colors.iter().any(|c| blk_colors.contains(c));
                if !shares_color {
                    return false;
                }
            }
        }

        true
    }

    fn active_player_has_eligible_attackers(&self) -> bool {
        let ap = self.state.active_player_id();
        let Some(ap_idx) = self.state.player_idx(ap) else {
            return false;
        };
        self.state.players[ap_idx].battlefield.iter().any(|oid| {
            self.state.objects.get(oid).is_some_and(|o| {
                o.zone == Zone::Battlefield
                    && o.owner == ap
                    && o.is_creature(&self.registry)
                    && !o.summoning_sick
                    && !o.tapped
            })
        })
    }

    fn defending_player_has_eligible_blockers(&self) -> bool {
        let Some(dp) = self.state.defending_player_id_1v1() else {
            return false;
        };
        let Some(dp_idx) = self.state.player_idx(dp) else {
            return false;
        };
        let attacking: Vec<ObjectId> = self
            .state
            .combat
            .as_ref()
            .map(|c| c.attacking.clone())
            .unwrap_or_default();
        if attacking.is_empty() {
            return false;
        }
        // CR 302.6: summoning sickness does NOT prevent blocking.
        // A creature is only eligible if it can legally block at least one current attacker
        // (e.g. a ground creature cannot block a flying attacker — CR 702.9b).
        self.state.players[dp_idx].battlefield.iter().any(|oid| {
            self.state.objects.get(oid).is_some_and(|o| {
                o.zone == Zone::Battlefield
                    && o.owner == dp
                    && o.is_creature(&self.registry)
                    && !o.tapped
                    && attacking.iter().any(|&aid| self.can_block(aid, o.id))
            })
        })
    }

    fn set_attackers(
        &mut self,
        ids: &[u32],
        _player: PlayerId,
    ) -> Result<RuledEventBatch, EngineError> {
        if self.state.priority_player_id() != _player {
            return Err(EngineError::Illegal("not your priority"));
        }
        let ap = self.state.active_player_id();
        if ids.is_empty() {
            self.clear_all_mana_pools();
            self.state.combat = None;
            self.state.turn_step = TurnStep::EndCombat;
            if let Some(i) = self.state.player_idx(ap) {
                self.state.priority_idx = i;
            }
            self.state.passes_since_stack_change = 0;
            let mut b2 = RuledEventBatch::default();
            b2.events
                .push(ev_log("No attackers — skipped to end combat".to_string()));
            b2.events.push(ev_phase_labeled(self, "end_combat"));
            b2.events.push(ev_priority_changed(self));
            fill_legal(&mut b2, self);
            return Ok(b2);
        }
        let mut list = Vec::new();
        let mut seen_attackers = HashSet::new();
        for &oid in ids {
            if !seen_attackers.insert(oid) {
                return Err(EngineError::Illegal("duplicate attacker"));
            }
            let o = self
                .state
                .objects
                .get(&oid)
                .ok_or(EngineError::Illegal("attacker id"))?;
            if o.owner != ap || o.zone != Zone::Battlefield {
                return Err(EngineError::Illegal("illegal attacker"));
            }
            if !o.is_creature(&self.registry) {
                return Err(EngineError::Illegal("not creature"));
            }
            if o.summoning_sick {
                return Err(EngineError::Illegal("summoning sick"));
            }
            if o.tapped {
                return Err(EngineError::Illegal("tapped"));
            }
            list.push(oid);
        }
        for &oid in &list {
            // CR 702.20a — Vigilance: attacking doesn't cause this creature to tap.
            let has_vigilance = self
                .state
                .objects
                .get(&oid)
                .map(|o| o.has_keyword(&self.registry, tricerules_cards::Keyword::Vigilance))
                .unwrap_or(false);
            if !has_vigilance {
                if let Some(c) = self.state.objects.get_mut(&oid) {
                    c.tapped = true;
                }
            }
        }
        let attackers_for_event = list.clone();
        if let Some(c) = self.state.combat.as_mut() {
            c.attacking = list;
            c.blockers.clear();
            c.damage_assignments.clear();
            c.damage_assignment_needed = false;
            c.assign_combat_damage_phase = false;
            c.attackers_declared = true;
            c.blockers_declared = false;
        } else {
            self.state.combat = Some(CombatState {
                attacking: list,
                blockers: HashMap::new(),
                damage_assignments: HashMap::new(),
                damage_assignment_needed: false,
                attackers_declared: true,
                blockers_declared: false,
                assign_combat_damage_phase: false,
            });
        }
        self.clear_all_mana_pools();
        // MTG timing: after attackers are declared, the game remains in declare-attackers
        // and the active player receives priority before moving to declare blockers.
        self.state.turn_step = TurnStep::DeclareAttackers;
        if let Some(ai) = self.state.player_idx(ap) {
            self.state.priority_idx = ai;
        }
        self.state.passes_since_stack_change = 0;
        let mut b = RuledEventBatch::default();
        b.events.push(rv1::RuledEvent {
            ev: Some(rv1::ruled_event::Ev::AttackersDeclared(
                rv1::AttackersDeclared {
                    attacking_player_id: ap,
                    attacker_object_ids: attackers_for_event.clone(),
                },
            )),
        });
        let atk_names: Vec<String> = attackers_for_event
            .iter()
            .map(|&oid| object_display_name(&self.state, &self.registry, oid))
            .collect();
        b.events.push(ev_log(format!(
            "P{} attacks with {}",
            ap,
            atk_names.join(", ")
        )));
        b.events.push(ev_priority_changed(self));
        Ok(b)
    }

    fn set_blockers(&mut self, pairs: &[rv1::BlockPair]) -> Result<RuledEventBatch, EngineError> {
        let defending_player = self
            .state
            .defending_player_id_1v1()
            .ok_or(EngineError::Illegal("defender missing"))?;
        // A blocker may appear at most once: CR 509.2 — a creature can only block one attacker.
        let mut seen_blockers = HashSet::new();
        // Build attacker → [blockers] map while validating.
        let mut attacker_to_blockers: HashMap<ObjectId, Vec<ObjectId>> = HashMap::new();
        for p in pairs {
            let in_attack = self
                .state
                .combat
                .as_ref()
                .map(|c| c.attacking.contains(&p.attacker_id))
                .unwrap_or(false);
            if !in_attack {
                return Err(EngineError::Illegal("bad attacker"));
            }
            if !seen_blockers.insert(p.blocker_id) {
                return Err(EngineError::Illegal("blocker assigned more than once"));
            }
            let bobj = self
                .state
                .objects
                .get(&p.blocker_id)
                .ok_or(EngineError::Illegal("blocker?"))?;
            if bobj.zone != Zone::Battlefield {
                return Err(EngineError::Illegal("blocker zone"));
            }
            if bobj.owner != defending_player {
                return Err(EngineError::Illegal("not your blocker"));
            }
            if !bobj.is_creature(&self.registry) {
                return Err(EngineError::Illegal("blocker not creature"));
            }
            if bobj.tapped {
                return Err(EngineError::Illegal("blocker tapped"));
            }
            // Evasion check: flying (CR 702.9b), intimidate (CR 702.13b), etc.
            if !self.can_block(p.attacker_id, p.blocker_id) {
                return Err(EngineError::Illegal(
                    "blocker cannot block this attacker (evasion)",
                ));
            }
            attacker_to_blockers
                .entry(p.attacker_id)
                .or_default()
                .push(p.blocker_id);
        }
        let damage_assignment_needed = attacker_to_blockers.values().any(|v| v.len() > 1);
        if let Some(c) = self.state.combat.as_mut() {
            c.blockers = attacker_to_blockers;
            c.damage_assignments.clear();
            c.damage_assignment_needed = damage_assignment_needed;
            c.assign_combat_damage_phase = false;
            c.blockers_declared = true;
        }
        let block_line = if pairs.is_empty() {
            "declares no blockers".to_string()
        } else {
            pairs
                .iter()
                .map(|p| {
                    let att = object_display_name(&self.state, &self.registry, p.attacker_id);
                    let blk = object_display_name(&self.state, &self.registry, p.blocker_id);
                    format!("{blk} blocks {att}")
                })
                .collect::<Vec<_>>()
                .join("; ")
        };
        let mut b = RuledEventBatch::default();
        let block_pairs_for_event: Vec<rv1::BlockPair> = pairs.to_vec();
        b.events.push(rv1::RuledEvent {
            ev: Some(rv1::ruled_event::Ev::BlockersDeclared(
                rv1::BlockersDeclared {
                    block_pairs: block_pairs_for_event,
                },
            )),
        });
        self.clear_all_mana_pools();
        // MTG timing: blockers are declared in declare-blockers, then players get priority
        // before the game advances into combat-damage where damage is actually dealt.
        self.state.turn_step = TurnStep::DeclareBlockers;
        if let Some(i) = self.state.player_idx(self.state.active_player_id()) {
            self.state.priority_idx = i;
        }
        self.state.passes_since_stack_change = 0;
        b.events
            .push(ev_log(format!("P{} {}", defending_player, block_line)));
        b.events.push(ev_priority_changed(self));
        fill_legal(&mut b, self);
        Ok(b)
    }

    fn assign_combat_damage(
        &mut self,
        attacker_id: ObjectId,
        assignments: &[(ObjectId, u32)],
    ) -> Result<RuledEventBatch, EngineError> {
        let c = self
            .state
            .combat
            .as_ref()
            .ok_or(EngineError::Illegal("not in combat"))?;
        if !c.blockers_declared || !c.damage_assignment_needed || !c.assign_combat_damage_phase {
            return Err(EngineError::Illegal("combat damage assignment not open"));
        }
        let expected_blockers = c
            .blockers
            .get(&attacker_id)
            .ok_or(EngineError::Illegal("attacker not blocked"))?;
        if expected_blockers.len() < 2 {
            return Err(EngineError::Illegal("attacker not multiply-blocked"));
        }
        let mut seen_block = HashSet::new();
        for &(bid, _) in assignments {
            if !seen_block.insert(bid) {
                return Err(EngineError::Illegal("duplicate blocker in assignments"));
            }
        }
        let provided: HashSet<ObjectId> = assignments.iter().map(|(b, _)| *b).collect();
        let expected_set: HashSet<ObjectId> = expected_blockers.iter().copied().collect();
        if provided != expected_set {
            return Err(EngineError::Illegal(
                "assignments must list each blocker exactly once",
            ));
        }
        let att_power = self
            .state
            .objects
            .get(&attacker_id)
            .and_then(|o| o.power)
            .ok_or(EngineError::Illegal("attacker missing"))?;
        let sum: u32 = assignments.iter().map(|(_, d)| d).sum();
        if sum != att_power {
            return Err(EngineError::Illegal(
                "assigned damage must equal attacker power",
            ));
        }

        let mut b = RuledEventBatch::default();
        let c = self.state.combat.as_mut().unwrap();
        c.damage_assignments
            .insert(attacker_id, assignments.to_vec());
        let all_done = c
            .blockers
            .iter()
            .filter(|(_, blks)| blks.len() > 1)
            .all(|(atk, _)| c.damage_assignments.contains_key(atk));
        if all_done {
            c.damage_assignment_needed = false;
        }
        let proto_pairs: Vec<rv1::DamagePair> = assignments
            .iter()
            .map(|&(bid, dmg)| rv1::DamagePair {
                blocker_id: bid,
                damage: dmg,
            })
            .collect();
        b.events.push(rv1::RuledEvent {
            ev: Some(rv1::ruled_event::Ev::CombatDamageAssigned(
                rv1::CombatDamageAssigned {
                    attacker_id,
                    assignments: proto_pairs,
                },
            )),
        });
        let att_name = object_display_name(&self.state, &self.registry, attacker_id);
        b.events
            .push(ev_log(format!("Combat damage assigned for {att_name}.")));

        if !self.state.combat.as_ref().unwrap().damage_assignment_needed {
            let c_now = self
                .state
                .combat
                .clone()
                .ok_or(EngineError::Illegal("combat?"))?;
            self.resolve_combat_damage(&c_now, &mut b.events)?;
            self.state.combat = None;
            self.clear_all_mana_pools();
            self.state.turn_step = TurnStep::CombatDamage;
            if let Some(i) = self.state.player_idx(self.state.active_player_id()) {
                self.state.priority_idx = i;
            }
            self.state.passes_since_stack_change = 0;
            let legend_events = self.apply_legend_sbas()?;
            b.events.extend(legend_events);
            b.events.push(ev_log("Combat damage dealt.".to_string()));
            b.events.push(ev_phase_labeled(self, "combat_damage"));
            b.events.push(ev_priority_changed(self));
        } else {
            b.events.push(ev_priority_changed(self));
        }
        fill_legal(&mut b, self);
        Ok(b)
    }

    fn resolve_combat_damage(
        &mut self,
        c: &CombatState,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<(), EngineError> {
        use tricerules_cards::Keyword;
        let dfd = self.state.defending_player_id_1v1().unwrap();
        let ap = self.state.active_player_id();
        let mut total_life_lost: i32 = 0;
        // (controller_id, amount) pairs — collected during damage assignment, applied after.
        let mut lifelink_gains: Vec<(PlayerId, u32)> = Vec::new();

        for &att in &c.attacking {
            if self.state.objects.get(&att).map(|a| a.zone) != Some(Zone::Battlefield) {
                continue;
            }
            // Capture attacker properties before any mutation.
            let att_power = self
                .state
                .objects
                .get(&att)
                .and_then(|o| o.power)
                .unwrap_or(0);
            let att_has_lifelink = self
                .state
                .objects
                .get(&att)
                .map(|o| o.has_keyword(&self.registry, Keyword::Lifelink))
                .unwrap_or(false);
            let att_owner = self
                .state
                .objects
                .get(&att)
                .map(|o| o.owner)
                .unwrap_or(ap);

            let blockers = c.blockers.get(&att).map(|v| v.as_slice()).unwrap_or(&[]);

            if blockers.is_empty() {
                // Unblocked: deal full power to defending player.
                let p = att_power as i32;
                if let Some(di) = self.state.player_idx(dfd) {
                    self.state.players[di].life -= p;
                    total_life_lost += p;
                }
                // CR 702.15a: attacker with lifelink causes its controller to gain that much life.
                if att_has_lifelink && att_power > 0 {
                    lifelink_gains.push((att_owner, att_power));
                }
            } else if blockers.len() == 1 {
                // Single blocker: exchange full power (unchanged from M2).
                let blk = blockers[0];
                let bpw = self
                    .state
                    .objects
                    .get(&blk)
                    .and_then(|o| o.power)
                    .unwrap_or(0);
                let blk_has_lifelink = self
                    .state
                    .objects
                    .get(&blk)
                    .map(|o| o.has_keyword(&self.registry, Keyword::Lifelink))
                    .unwrap_or(false);
                let blk_owner = self
                    .state
                    .objects
                    .get(&blk)
                    .map(|o| o.owner)
                    .unwrap_or(dfd);
                if let Some(af) = self.state.objects.get_mut(&att) {
                    af.damage += bpw;
                }
                if let Some(bf) = self.state.objects.get_mut(&blk) {
                    bf.damage += att_power;
                }
                // CR 702.15a: attacker with lifelink gains life = damage dealt to blocker.
                if att_has_lifelink && att_power > 0 {
                    lifelink_gains.push((att_owner, att_power));
                }
                // CR 702.15a: blocker with lifelink gains life = damage dealt to attacker.
                if blk_has_lifelink && bpw > 0 {
                    lifelink_gains.push((blk_owner, bpw));
                }
            } else {
                // Multiple blockers: all blockers deal their power to the attacker simultaneously;
                // active player assigns how the attacker's combat damage is divided among blockers.
                // Capture per-blocker properties before any mutation.
                let blocker_info: Vec<(ObjectId, u32, bool, PlayerId)> = blockers
                    .iter()
                    .map(|&blk| {
                        let pw = self
                            .state
                            .objects
                            .get(&blk)
                            .and_then(|o| o.power)
                            .unwrap_or(0);
                        let has_ll = self
                            .state
                            .objects
                            .get(&blk)
                            .map(|o| o.has_keyword(&self.registry, Keyword::Lifelink))
                            .unwrap_or(false);
                        let owner = self
                            .state
                            .objects
                            .get(&blk)
                            .map(|o| o.owner)
                            .unwrap_or(dfd);
                        (blk, pw, has_ll, owner)
                    })
                    .collect();
                let total_blocker_power: u32 = blocker_info.iter().map(|(_, pw, _, _)| pw).sum();
                if let Some(af) = self.state.objects.get_mut(&att) {
                    af.damage += total_blocker_power;
                }
                let pairs = c.damage_assignments.get(&att).ok_or(EngineError::Illegal(
                    "combat damage assignments missing for multiply-blocked attacker",
                ))?;
                for &(blk, dmg) in pairs {
                    if let Some(bf) = self.state.objects.get_mut(&blk) {
                        bf.damage += dmg;
                    }
                }
                // CR 702.15a: attacker with lifelink gains life = total damage dealt to all blockers.
                if att_has_lifelink && att_power > 0 {
                    lifelink_gains.push((att_owner, att_power));
                }
                // CR 702.15a: each blocker with lifelink gains life = damage it dealt to the attacker.
                for (_, blk_pw, blk_has_ll, blk_owner) in blocker_info {
                    if blk_has_ll && blk_pw > 0 {
                        lifelink_gains.push((blk_owner, blk_pw));
                    }
                }
            }
        }
        if total_life_lost > 0 {
            if let Some(di) = self.state.player_idx(dfd) {
                let new_total = self.state.players[di].life;
                events.push(rv1::RuledEvent {
                    ev: Some(rv1::ruled_event::Ev::LifeChanged(rv1::LifeChanged {
                        player_id: dfd,
                        new_total,
                        delta: -total_life_lost,
                    })),
                });
            }
        }
        // Apply lifelink gains and emit LifeChanged events.
        for (pid, amount) in lifelink_gains {
            if let Some(pi) = self.state.player_idx(pid) {
                self.state.players[pi].life += amount as i32;
                let new_total = self.state.players[pi].life;
                events.push(rv1::RuledEvent {
                    ev: Some(rv1::ruled_event::Ev::LifeChanged(rv1::LifeChanged {
                        player_id: pid,
                        new_total,
                        delta: amount as i32,
                    })),
                });
                events.push(ev_log(format!("P{pid} gains {amount} life (lifelink).")));
            }
        }
        Ok(())
    }

    /// Single-step structural advance on an empty stack (Cockatrice "pass turn" in ruled mode).
    /// Active advances main / combat structure; defender may skip blockers during declare blockers.
    fn primitive_yield_structured(
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
                if Some(player) != self.state.defending_player_id_1v1() {
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

    fn concede_batch(&mut self, player: PlayerId) -> Result<RuledEventBatch, EngineError> {
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
        fill_legal(&mut batch, self);
        Ok(batch)
    }

    fn pass_priority(&mut self, player: PlayerId) -> Result<RuledEventBatch, EngineError> {
        if self.state.priority_player_id() != player {
            return Err(EngineError::Illegal("not your priority"));
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

    fn pass_priority_on_stack(
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
        ev.push(ev_priority_changed(self));
        self.apply_sbas(&mut ev)?;
        Ok(finish_with_events(self, ev))
    }

    fn adv_on_empty_stack(
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
                ev.push(ev_phase_labeled(self, "upkeep"));
                ev.push(ev_priority_changed(self));
            }
            Upkeep => {
                self.clear_all_mana_pools();
                self.state.turn_step = Draw;
                if let Some(i) = self.state.player_idx(ap) {
                    self.state.priority_idx = i;
                }
                ev.push(ev_phase_labeled(self, "draw"));
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
                }
                self.state.passes_since_stack_change = 0;
                ev.push(ev_priority_changed(self));
            }
            Draw => {
                self.clear_all_mana_pools();
                self.state.turn_step = Main1;
                self.state.combat = None;
                if let Some(i) = self.state.player_idx(ap) {
                    self.state.priority_idx = i;
                }
                self.state.passes_since_stack_change = 0;
                ev.push(ev_phase_labeled(self, "main1"));
                ev.push(ev_priority_changed(self));
            }
            Main1 => {
                self.clear_all_mana_pools();
                self.state.turn_step = BeginCombat;
                if let Some(i) = self.state.player_idx(ap) {
                    self.state.priority_idx = i;
                }
                ev.push(ev_phase_labeled(self, "begin_combat"));
                ev.push(ev_priority_changed(self));
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
                    ev.push(ev_phase_labeled(self, "end_combat"));
                    ev.push(ev_priority_changed(self));
                } else {
                    self.state.turn_step = DeclareAttackers;
                    if let Some(i) = self.state.player_idx(ap) {
                        self.state.priority_idx = i;
                    }
                    self.state.combat = Some(CombatState {
                        attacking: vec![],
                        blockers: HashMap::new(),
                        damage_assignments: HashMap::new(),
                        damage_assignment_needed: false,
                        attackers_declared: false,
                        blockers_declared: false,
                        assign_combat_damage_phase: false,
                    });
                    ev.push(ev_phase_labeled(self, "declare_attackers"));
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
                    ev.push(ev_phase_labeled(self, "declare_blockers"));
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
                    if let Some(d) = self.state.defending_player_id_1v1() {
                        if let Some(di) = self.state.player_idx(d) {
                            self.state.priority_idx = di;
                        }
                    }
                    ev.push(ev_phase_labeled(self, "declare_blockers"));
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
                let multiblock_missing = c
                    .blockers
                    .iter()
                    .any(|(atk, blks)| blks.len() > 1 && !c.damage_assignments.contains_key(atk));
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
                        ev.push(ev_phase_labeled(self, "assign_combat_damage"));
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
                    self.resolve_combat_damage(&c, ev)?;
                    self.state.combat = None;
                    self.clear_all_mana_pools();
                    self.state.turn_step = CombatDamage;
                    if let Some(i) = self.state.player_idx(ap) {
                        self.state.priority_idx = i;
                    }
                    self.state.passes_since_stack_change = 0;
                    let legend_events = self.apply_legend_sbas()?;
                    ev.extend(legend_events);
                    ev.push(ev_log("Combat damage dealt.".to_string()));
                    ev.push(ev_phase_labeled(self, "combat_damage"));
                    ev.push(ev_priority_changed(self));
                }
            }
            CombatDamage => {
                self.clear_all_mana_pools();
                self.state.turn_step = EndCombat;
                if let Some(i) = self.state.player_idx(ap) {
                    self.state.priority_idx = i;
                }
                self.state.passes_since_stack_change = 0;
                ev.push(ev_phase_labeled(self, "end_combat"));
                ev.push(ev_priority_changed(self));
            }
            EndCombat => {
                self.clear_all_mana_pools();
                self.state.turn_step = Main2;
                if let Some(i) = self.state.player_idx(ap) {
                    self.state.priority_idx = i;
                }
                ev.push(ev_phase_labeled(self, "main2"));
                ev.push(ev_priority_changed(self));
            }
            Main2 => {
                self.clear_all_mana_pools();
                if let Some(i) = self.state.player_idx(ap) {
                    self.state.priority_idx = i;
                }
                self.state.turn_step = EndStep;
                self.state.passes_since_stack_change = 0;
                ev.push(ev_phase_labeled(self, "end_step"));
                ev.push(ev_priority_changed(self));
            }
            EndStep => {
                self.clear_all_mana_pools();
                self.state.turn_step = Cleanup;
                self.state.passes_since_stack_change = 0;
                // No PhaseChanged: clients keep highlighting end step during engine cleanup (CR 514).
                let mut ev = vec![];
                self.apply_sbas(&mut ev)?;
                return self.start_cleanup_or_roll_turn(ev);
            }
            _ => {
                self.clear_all_mana_pools();
                if let Some(i) = self.state.player_idx(ap) {
                    self.state.priority_idx = i;
                }
                self.state.passes_since_stack_change = 0;
                ev.push(ev_phase_labeled(self, "main1"));
                ev.push(ev_priority_changed(self));
            }
        }
        self.apply_sbas(ev)?;
        Ok(finish_with_events(self, std::mem::take(ev)))
    }

    /// Cleanup-step analogue: until-end-of-turn P/T boosts (e.g. Giant Growth) are modeled by
    /// mutating `GameObject::power` / `toughness`; restore printed values from the card registry.
    fn cleanup_until_end_of_turn_creature_pt(&mut self) {
        let ids: Vec<ObjectId> = self
            .state
            .players
            .iter()
            .flat_map(|p| p.battlefield.iter().copied())
            .collect();
        for oid in ids {
            let Some(o) = self.state.objects.get_mut(&oid) else {
                continue;
            };
            if !o.is_creature(&self.registry) {
                continue;
            }
            if let Some(def) = self.registry.get(&o.card_id) {
                o.power = def.power;
                o.toughness = def.toughness;
            }
        }
    }

    /// CR 514.2: damage marked on permanents is removed during cleanup.
    fn cleanup_marked_damage(&mut self) {
        for o in self.state.objects.values_mut() {
            if o.zone == Zone::Battlefield && o.damage != 0 {
                o.damage = 0;
            }
        }
    }

    fn next_cleanup_discard_needed(&self) -> Option<PlayerId> {
        let n = self.state.players.len();
        if n == 0 {
            return None;
        }
        let start = self.state.active_player_idx;
        for k in 0..n {
            let i = (start + k) % n;
            if self.state.players[i].hand.len() > MAX_HAND_SIZE {
                return Some(self.state.players[i].id);
            }
        }
        None
    }

    fn start_cleanup_or_roll_turn(
        &mut self,
        mut ev: Vec<rv1::RuledEvent>,
    ) -> Result<RuledEventBatch, EngineError> {
        if let Some(pid) = self.next_cleanup_discard_needed() {
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

    fn discard_to_hand_size(
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
        let mut positions: Vec<usize> = if !d.hand_card_indices.is_empty() {
            d.hand_card_indices.iter().map(|&i| i as usize).collect()
        } else {
            vec![d.hand_card_index as usize]
        };
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
            let card_name = self
                .registry
                .get(&self.state.objects.get(&oid).unwrap().card_id)
                .map(|d| d.name.clone())
                .unwrap_or_else(|| "card".into());
            move_object_to_zone(&mut self.state, oid, Zone::Graveyard)?;
            ev.push(ev_log(format!("P{player} discards {card_name} (cleanup)")));
            ev.push(permanent_moved_event(
                &self.state,
                oid,
                owner,
                rv1::permanent_moved::Destination::Graveyard,
            ));
        }
        self.apply_sbas(&mut ev)?;
        if self.state.players[idx].hand.len() > MAX_HAND_SIZE {
            ev.push(ev_priority_changed(self));
            return Ok(finish_with_events(self, ev));
        }
        self.state.cleanup_discard_player = None;
        if let Some(pid) = self.next_cleanup_discard_needed() {
            self.state.cleanup_discard_player = Some(pid);
            if let Some(i) = self.state.player_idx(pid) {
                self.state.priority_idx = i;
            }
            ev.push(ev_log(format!(
                "P{pid}: discard to hand size ({MAX_HAND_SIZE})"
            )));
            ev.push(ev_priority_changed(self));
            return Ok(finish_with_events(self, ev));
        }
        self.finish_cleanup_roll_new_turn(ev)
    }

    /// After cleanup discards (514.1), apply 514.2-style clearing and advance the turn.
    fn finish_cleanup_roll_new_turn(
        &mut self,
        mut ev: Vec<rv1::RuledEvent>,
    ) -> Result<RuledEventBatch, EngineError> {
        self.state.cleanup_discard_player = None;
        self.cleanup_until_end_of_turn_creature_pt();
        self.cleanup_marked_damage();
        self.clear_all_mana_pools();
        self.state.land_dropped_this_turn = false;
        let n = self.state.players.len();
        if n >= 1 {
            self.state.active_player_idx = (self.state.active_player_idx + 1) % n;
        }
        if self.state.active_player_idx == 0 {
            self.state.turn = self.state.turn.saturating_add(1);
        }
        let ap = self.state.active_player_id();
        self.state.turn_step = TurnStep::Untap;
        ev.push(ev_phase_labeled(self, "untap"));

        for o in self.state.objects.values_mut() {
            if o.owner == ap {
                o.tapped = false;
            }
        }
        if let Some(idx) = self.state.player_idx(ap) {
            for &oid in &self.state.players[idx].battlefield.clone() {
                if let Some(c) = self.state.objects.get_mut(&oid) {
                    c.summoning_sick = false;
                }
            }
        }
        // Servatrice only applies engine untaps during batches that include phase_changed("untap").
        // Emit zone_view in this same batch so battlefield_tapped reaches Cockatrice while
        // batchHasUntapPhase is still true (see Server_Game::applyRuledBatch).
        ev.push(self.ev_zone_view_sync());
        self.state.turn_step = TurnStep::Upkeep;
        ev.push(ev_phase_labeled(self, "upkeep"));
        self.state.combat = None;
        if let Some(i) = self.state.player_idx(ap) {
            self.state.priority_idx = i;
        }
        self.state.passes_since_stack_change = 0;
        let legend_events = self.apply_legend_sbas()?;
        ev.extend(legend_events);
        self.apply_sbas(&mut ev)?;
        ev.push(ev_log(format!("Turn {}: P{}", self.state.turn, ap)));
        ev.push(ev_priority_changed(self));
        Ok(finish_with_events(self, ev))
    }

    fn resolve_top_of_stack(
        &mut self,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<(), EngineError> {
        let top = self
            .state
            .stack
            .pop()
            .ok_or(EngineError::Illegal("empty stack"))?;
        let controller = top.controller;
        let card_id = top.card_id.clone();
        let targets = top.targets.clone();

        let resolves_to_battlefield = self
            .registry
            .get(&card_id)
            .map(|d| !d.is_instant && !d.is_sorcery)
            .unwrap_or(false);
        let destination = if resolves_to_battlefield {
            rv1::StackResolveDestination::Battlefield as i32
        } else {
            rv1::StackResolveDestination::Graveyard as i32
        };
        events.push(rv1::RuledEvent {
            ev: Some(rv1::ruled_event::Ev::StackResolved(rv1::StackResolved {
                object_id: top.id,
                destination,
            })),
        });
        move_object_to_zone(
            &mut self.state,
            top.id,
            if resolves_to_battlefield {
                Zone::Battlefield
            } else {
                Zone::Graveyard
            },
        )?;

        let effect = self
            .registry
            .get(&card_id)
            .and_then(|c| c.spell_effect.clone())
            .unwrap_or(SpellEffectKind::None);

        let spell_label = self
            .registry
            .get(&card_id)
            .map(|d| d.name.clone())
            .unwrap_or_else(|| "Spell".into());

        let fizzle = spell_has_no_legal_targets_at_resolution(
            &self.state,
            &self.registry,
            &effect,
            &targets,
            controller,
        );
        if fizzle {
            events.push(ev_log(format!("{spell_label} fizzles (no legal targets).")));
            return Ok(());
        }

        match effect {
            SpellEffectKind::DamageTarget { amount, .. } => {
                if let Some(&tid) = targets.first() {
                    if let Some(pi) = self.state.player_idx(tid as i32) {
                        let pid = self.state.players[pi].id;
                        self.state.players[pi].life -= amount as i32;
                        events.push(rv1::RuledEvent {
                            ev: Some(rv1::ruled_event::Ev::LifeChanged(rv1::LifeChanged {
                                player_id: self.state.players[pi].id,
                                new_total: self.state.players[pi].life,
                                delta: -(amount as i32),
                            })),
                        });
                        events.push(ev_log(format!(
                            "{spell_label} deals {amount} damage to P{pid}"
                        )));
                    } else {
                        let tgt = object_display_name(&self.state, &self.registry, tid);
                        if let Some(t) = self.state.objects.get_mut(&tid) {
                            if t.zone == Zone::Battlefield && t.is_creature(&self.registry) {
                                t.damage += amount;
                                events.push(ev_log(format!(
                                    "{spell_label} deals {amount} damage to {tgt}"
                                )));
                            }
                        }
                    }
                }
            }
            SpellEffectKind::Draw { count } => {
                let idx = self.state.player_idx(controller).unwrap();
                for _ in 0..count {
                    draw_card(&mut self.state.players[idx], &mut self.state.objects)?;
                }
                let noun = if count == 1 { "card" } else { "cards" };
                events.push(ev_log(format!(
                    "P{controller} draws {count} {noun} ({spell_label})."
                )));
            }
            SpellEffectKind::PumpTarget { power, toughness } => {
                if let Some(&tid) = targets.first() {
                    let tgt = object_display_name(&self.state, &self.registry, tid);
                    if let Some(t) = self.state.objects.get_mut(&tid) {
                        if t.zone == Zone::Battlefield && t.is_creature(&self.registry) {
                            let p = t.power.unwrap_or(0) as i32 + power;
                            let tt = t.toughness.unwrap_or(0) as i32 + toughness;
                            t.power = Some(p.max(0) as u32);
                            t.toughness = Some(tt.max(0) as u32);
                            events.push(ev_log(format!(
                                "{spell_label} gives +{power}/+{toughness} to {tgt}"
                            )));
                        }
                    }
                }
            }
            SpellEffectKind::DestroyTarget => {
                if let Some(&tid) = targets.first() {
                    let tgt = object_display_name(&self.state, &self.registry, tid);
                    events.push(ev_log(format!("{spell_label} destroys {tgt}")));
                    let owner = self.state.objects.get(&tid).map(|o| o.owner);
                    destroy_permanent(&mut self.state, tid)?;
                    if let Some(owner_id) = owner {
                        events.push(permanent_moved_event(
                            &self.state,
                            tid,
                            owner_id,
                            rv1::permanent_moved::Destination::Graveyard,
                        ));
                    }
                }
            }
            SpellEffectKind::CounterTargetSpell => {
                if let Some(&tid) = targets.first() {
                    if let Some(pos) = self.state.stack.iter().position(|s| s.id == tid) {
                        let st = self.state.stack.remove(pos);
                        let tgt = self
                            .registry
                            .get(&st.card_id)
                            .map(|d| d.name.as_str())
                            .unwrap_or("spell");
                        move_object_to_zone(&mut self.state, st.id, Zone::Graveyard)?;
                        events.push(ev_log(format!("{spell_label} counters {tgt}")));
                    }
                }
            }
            SpellEffectKind::GainLife { amount } => {
                let pi = self.state.player_idx(controller).unwrap();
                self.state.players[pi].life += amount as i32;
                events.push(rv1::RuledEvent {
                    ev: Some(rv1::ruled_event::Ev::LifeChanged(rv1::LifeChanged {
                        player_id: controller,
                        new_total: self.state.players[pi].life,
                        delta: amount as i32,
                    })),
                });
                events.push(ev_log(format!(
                    "P{controller} gains {amount} life ({spell_label})."
                )));
            }
            SpellEffectKind::TargetPlayerGainsLife { amount, .. } => {
                if let Some(&tid) = targets.first() {
                    if let Some(pi) = self.state.player_idx(tid as i32) {
                        let pid = self.state.players[pi].id;
                        self.state.players[pi].life += amount as i32;
                        events.push(rv1::RuledEvent {
                            ev: Some(rv1::ruled_event::Ev::LifeChanged(rv1::LifeChanged {
                                player_id: pid,
                                new_total: self.state.players[pi].life,
                                delta: amount as i32,
                            })),
                        });
                        events.push(ev_log(format!(
                            "P{pid} gains {amount} life ({spell_label})."
                        )));
                    }
                }
            }
            SpellEffectKind::TargetPlayerLosesLife { amount, .. } => {
                if let Some(&tid) = targets.first() {
                    if let Some(pi) = self.state.player_idx(tid as i32) {
                        let pid = self.state.players[pi].id;
                        self.state.players[pi].life -= amount as i32;
                        events.push(rv1::RuledEvent {
                            ev: Some(rv1::ruled_event::Ev::LifeChanged(rv1::LifeChanged {
                                player_id: pid,
                                new_total: self.state.players[pi].life,
                                delta: -(amount as i32),
                            })),
                        });
                        events.push(ev_log(format!(
                            "P{pid} loses {amount} life ({spell_label})."
                        )));
                    }
                }
            }
            SpellEffectKind::EachOpponentLosesLifeYouGainEqual { amount } => {
                let opps: Vec<(usize, PlayerId)> = self
                    .state
                    .players
                    .iter()
                    .enumerate()
                    .filter(|(_, p)| p.id != controller && !p.has_lost)
                    .map(|(i, p)| (i, p.id))
                    .collect();
                let mut total_lost: u32 = 0;
                for (pi, pid) in opps {
                    self.state.players[pi].life -= amount as i32;
                    total_lost += amount;
                    events.push(rv1::RuledEvent {
                        ev: Some(rv1::ruled_event::Ev::LifeChanged(rv1::LifeChanged {
                            player_id: pid,
                            new_total: self.state.players[pi].life,
                            delta: -(amount as i32),
                        })),
                    });
                    events.push(ev_log(format!(
                        "P{pid} loses {amount} life ({spell_label})."
                    )));
                }
                if total_lost > 0 {
                    if let Some(ci) = self.state.player_idx(controller) {
                        self.state.players[ci].life += total_lost as i32;
                        events.push(rv1::RuledEvent {
                            ev: Some(rv1::ruled_event::Ev::LifeChanged(rv1::LifeChanged {
                                player_id: controller,
                                new_total: self.state.players[ci].life,
                                delta: total_lost as i32,
                            })),
                        });
                        events.push(ev_log(format!(
                            "P{controller} gains {total_lost} life ({spell_label})."
                        )));
                    }
                }
            }
            SpellEffectKind::ExileTarget => {
                if let Some(&tid) = targets.first() {
                    let tgt = object_display_name(&self.state, &self.registry, tid);
                    let owner = self.state.objects.get(&tid).map(|o| o.owner);
                    move_object_to_zone(&mut self.state, tid, Zone::Exile)?;
                    events.push(ev_log(format!("{spell_label} exiles {tgt}")));
                    if let Some(owner_id) = owner {
                        events.push(permanent_moved_event(
                            &self.state,
                            tid,
                            owner_id,
                            rv1::permanent_moved::Destination::Exile,
                        ));
                    }
                }
            }
            SpellEffectKind::ExileTargetGainLifeEqualToPower => {
                if let Some(&tid) = targets.first() {
                    let tgt = object_display_name(&self.state, &self.registry, tid);
                    // CR 608: read power at resolution before the object leaves the battlefield.
                    let power = self
                        .state
                        .objects
                        .get(&tid)
                        .and_then(|o| o.power)
                        .unwrap_or(0);
                    let owner = self.state.objects.get(&tid).map(|o| o.owner);
                    let target_controller = owner.unwrap_or(controller);
                    move_object_to_zone(&mut self.state, tid, Zone::Exile)?;
                    events.push(ev_log(format!("{spell_label} exiles {tgt}")));
                    if let Some(owner_id) = owner {
                        events.push(permanent_moved_event(
                            &self.state,
                            tid,
                            owner_id,
                            rv1::permanent_moved::Destination::Exile,
                        ));
                    }
                    if power > 0 {
                        if let Some(pi) = self.state.player_idx(target_controller) {
                            self.state.players[pi].life += power as i32;
                            events.push(rv1::RuledEvent {
                                ev: Some(rv1::ruled_event::Ev::LifeChanged(rv1::LifeChanged {
                                    player_id: target_controller,
                                    new_total: self.state.players[pi].life,
                                    delta: power as i32,
                                })),
                            });
                            events
                                .push(ev_log(format!("P{target_controller} gains {power} life.")));
                        }
                    }
                }
            }
            SpellEffectKind::ReturnTargetCreatureToHand
            | SpellEffectKind::ReturnTargetPermanentToHand => {
                if let Some(&tid) = targets.first() {
                    let tgt = object_display_name(&self.state, &self.registry, tid);
                    let owner = self.state.objects.get(&tid).map(|o| o.owner);
                    move_object_to_zone(&mut self.state, tid, Zone::Hand)?;
                    // Reset transient permanent state when leaving the battlefield.
                    if let Some(o) = self.state.objects.get_mut(&tid) {
                        o.tapped = false;
                        o.summoning_sick = false;
                        o.damage = 0;
                        o.power = None;
                        o.toughness = None;
                    }
                    events.push(ev_log(format!(
                        "{spell_label} returns {tgt} to its owner's hand"
                    )));
                    if let Some(owner_id) = owner {
                        events.push(permanent_moved_event(
                            &self.state,
                            tid,
                            owner_id,
                            rv1::permanent_moved::Destination::Hand,
                        ));
                    }
                }
            }
            SpellEffectKind::MillTargetPlayer { count, .. } => {
                if let Some(&tid) = targets.first() {
                    if let Some(pi) = self.state.player_idx(tid as i32) {
                        let pid = self.state.players[pi].id;
                        let mut milled = 0u32;
                        for _ in 0..count {
                            let Some(oid) = self.state.players[pi].library.pop_front() else {
                                break;
                            };
                            self.state.players[pi].graveyard.push(oid);
                            if let Some(o) = self.state.objects.get_mut(&oid) {
                                o.zone = Zone::Graveyard;
                            }
                            events.push(permanent_moved_event(
                                &self.state,
                                oid,
                                pid,
                                rv1::permanent_moved::Destination::Graveyard,
                            ));
                            milled += 1;
                        }
                        events.push(ev_log(format!(
                            "{spell_label} mills {milled} card(s) from P{pid}"
                        )));
                    }
                }
            }
            SpellEffectKind::None => {}
        }
        events.push(ev_log(format!("{spell_label} resolves.")));
        Ok(())
    }

    fn cast_spell(
        &mut self,
        player: PlayerId,
        hand_idx: usize,
        targets: &[rv1::TargetRef],
    ) -> Result<RuledEventBatch, EngineError> {
        if self.state.priority_player_id() != player {
            return Err(EngineError::Illegal("not your priority"));
        }
        if self.state.turn_step == TurnStep::Cleanup {
            return Err(EngineError::Illegal("no spells during cleanup"));
        }
        let idx = self
            .state
            .player_idx(player)
            .ok_or(EngineError::UnknownPlayer(player))?;
        let oid = *self.state.players[idx]
            .hand
            .get(hand_idx)
            .ok_or(EngineError::Illegal("bad hand index"))?;
        let card_id = self.state.objects.get(&oid).unwrap().card_id.clone();
        let def = self
            .registry
            .get(&card_id)
            .ok_or_else(|| EngineError::MissingCard(card_id.clone()))?;
        if def.is_land {
            return Err(EngineError::Illegal("use play land"));
        }
        let sorcery_ok = sorcery_speed_available(&self.state, player);
        let instant_ok = instant_timing_step_allowed(self.state.turn_step);
        if def.is_sorcery && !sorcery_ok {
            return Err(EngineError::Illegal("sorcery speed only"));
        }
        if def.is_instant && !instant_ok {
            return Err(EngineError::Illegal("instant timing"));
        }
        if !def.is_sorcery && !def.is_instant && !sorcery_ok {
            return Err(EngineError::Illegal("sorcery speed only"));
        }
        // CR 508.1 / 508.2: attackers are declared before any player gets priority in the
        // declare-attackers step. CR 509.1 / 509.3: same for blockers in declare blockers.
        if priority_locked_for_combat_declaration(&self.state) {
            return Err(EngineError::Illegal(
                "cannot cast until attack or block declaration is complete",
            ));
        }
        validate_spell_targets(&self.state, &self.registry, player, &card_id, targets)?;
        pay_mana_simple(&mut self.state, &self.registry, idx, &def.mana_cost)?;

        self.state.players[idx].hand.retain(|&x| x != oid);
        let trefs: Vec<ObjectId> = targets.iter().map(|t| t.object_id).collect();
        let tgt_line = format_spell_targets_log(&self.state, &self.registry, &trefs);

        self.state.stack.push(StackItem {
            id: oid,
            controller: player,
            card_id: card_id.clone(),
            targets: trefs,
        });
        if let Some(o) = self.state.objects.get_mut(&oid) {
            o.zone = Zone::Stack;
        }

        self.state.passes_since_stack_change = 0;
        // MTG priority: after casting a spell, the caster gets priority first.
        self.state.priority_idx = idx;

        let def_name = def.name.clone();
        let mut batch = RuledEventBatch::default();
        batch.events.push(ev_log(format!(
            "P{} casts {}{}",
            player, def.name, tgt_line
        )));
        batch.events.push(rv1::RuledEvent {
            ev: Some(rv1::ruled_event::Ev::StackPushed(rv1::StackPushed {
                object_id: oid,
                description: def_name,
                targets: targets.to_vec(),
            })),
        });
        batch.events.push(ev_priority_changed(self));
        fill_legal(&mut batch, self);
        Ok(batch)
    }

    fn play_land(
        &mut self,
        player: PlayerId,
        hand_idx: usize,
    ) -> Result<RuledEventBatch, EngineError> {
        if self.state.priority_player_id() != player {
            return Err(EngineError::Illegal("not your priority"));
        }
        if self.state.land_dropped_this_turn {
            return Err(EngineError::Illegal("one land per turn"));
        }
        if !sorcery_speed_available(&self.state, player) {
            return Err(EngineError::Illegal("play land only at sorcery speed"));
        }
        let idx = self
            .state
            .player_idx(player)
            .ok_or(EngineError::UnknownPlayer(player))?;
        let oid = *self.state.players[idx]
            .hand
            .get(hand_idx)
            .ok_or(EngineError::Illegal("bad hand index"))?;
        let card_id = self.state.objects.get(&oid).unwrap().card_id.clone();
        let def = self.registry.get(&card_id).unwrap();
        if !def.is_land {
            return Err(EngineError::Illegal("not a land"));
        }
        self.state.land_dropped_this_turn = true;
        self.state.players[idx].hand.retain(|&x| x != oid);
        self.state.players[idx].battlefield.push(oid);
        if let Some(o) = self.state.objects.get_mut(&oid) {
            o.zone = Zone::Battlefield;
        }
        self.state.passes_since_stack_change = 0;
        let mut batch = RuledEventBatch::default();
        batch
            .events
            .push(ev_log(format!("P{} played {}", player, def.name)));
        fill_legal(&mut batch, self);
        Ok(batch)
    }

    fn add_mana_to_pool(
        &mut self,
        player: PlayerId,
        m: &rv1::AddManaToPool,
    ) -> Result<RuledEventBatch, EngineError> {
        let idx = self
            .state
            .player_idx(player)
            .ok_or(EngineError::UnknownPlayer(player))?;
        if idx != self.state.priority_idx {
            return Err(EngineError::Illegal("only priority player can add mana"));
        }
        if priority_locked_for_combat_declaration(&self.state) {
            return Err(EngineError::Illegal(
                "cannot activate mana abilities while declaring attackers or blockers",
            ));
        }
        let clamp = |v: u32, d: i32| -> u32 { (v as i64 + i64::from(d)).clamp(0, 10_000) as u32 };
        let p = &mut self.state.players[idx].mana_pool;
        p.white = clamp(p.white, m.w);
        p.blue = clamp(p.blue, m.u);
        p.black = clamp(p.black, m.b);
        p.red = clamp(p.red, m.r);
        p.green = clamp(p.green, m.g);
        p.colorless = clamp(p.colorless, m.c);
        Ok(RuledEventBatch::default())
    }

    fn apply_sbas(&mut self, out: &mut Vec<rv1::RuledEvent>) -> Result<(), EngineError> {
        let mut to_destroy = Vec::new();
        for (&id, o) in &self.state.objects {
            if o.zone == Zone::Battlefield {
                if let Some(t) = o.toughness {
                    if t == 0 || o.damage >= t {
                        to_destroy.push(id);
                    }
                }
            }
        }
        for id in to_destroy {
            let owner = self.state.objects.get(&id).map(|o| o.owner);
            if destroy_permanent(&mut self.state, id).is_ok() {
                if let Some(owner_id) = owner {
                    out.push(permanent_moved_event(
                        &self.state,
                        id,
                        owner_id,
                        rv1::permanent_moved::Destination::Graveyard,
                    ));
                }
            }
        }
        Ok(())
    }

    fn apply_legend_sbas(&mut self) -> Result<Vec<rv1::RuledEvent>, EngineError> {
        let mut by_name: HashMap<String, Vec<ObjectId>> = HashMap::new();
        let mut out = Vec::new();
        for (&id, o) in &self.state.objects {
            if o.zone != Zone::Battlefield {
                continue;
            }
            if !self
                .registry
                .get(&o.card_id)
                .map(|c| c.is_legendary)
                .unwrap_or(false)
            {
                continue;
            }
            let n = self.registry.get(&o.card_id).unwrap().name.clone();
            by_name.entry(n).or_default().push(id);
        }
        for ids in by_name.values() {
            if ids.len() < 2 {
                continue;
            }
            for &g in ids.iter().skip(1) {
                let owner = self.state.objects.get(&g).map(|o| o.owner);
                if destroy_permanent(&mut self.state, g).is_ok() {
                    if let Some(owner_id) = owner {
                        out.push(permanent_moved_event(
                            &self.state,
                            g,
                            owner_id,
                            rv1::permanent_moved::Destination::Graveyard,
                        ));
                    }
                }
            }
        }
        Ok(out)
    }

    pub fn initial_response_batch(&self) -> RuledEventBatch {
        let mut batch = RuledEventBatch::default();
        batch.events.push(self.ev_zone_view_sync());
        if let Some(op) = &self.state.opening {
            batch
                .events
                .push(ev_phase_labeled(self, "opening_choose_first"));
            batch.events.push(ev_priority_changed(self));
            batch
                .events
                .push(ev_log(format!("P{} chooses who goes first.", op.chooser)));
            fill_legal(&mut batch, self);
            return batch;
        }
        batch.events.push(ev_phase_labeled(self, "upkeep"));
        batch.events.push(ev_priority_changed(self));
        batch.events.push(ev_log(format!(
            "Game started — active P{}, priority P{} (upkeep).",
            self.state.active_player_id(),
            self.state.priority_player_id(),
        )));
        fill_legal(&mut batch, self);
        batch
    }

    pub fn player_command_ipc(&mut self, player: PlayerId, bytes: &[u8]) -> IpcResponse {
        match RuledCommand::decode(bytes) {
            Ok(cmd) => match self.apply_command(player, &cmd) {
                Ok(batch) => IpcResponse {
                    ok: true,
                    error: String::new(),
                    batch: Some(batch),
                },
                Err(EngineError::GameOver(w)) => IpcResponse {
                    ok: true,
                    error: String::new(),
                    batch: Some(self.game_over_batch_winner(w)),
                },
                Err(e) => IpcResponse {
                    ok: false,
                    error: e.to_string(),
                    batch: None,
                },
            },
            Err(e) => IpcResponse {
                ok: false,
                error: format!("decode: {e}"),
                batch: None,
            },
        }
    }

    /// Deck + hand for Cockatrice server to line up with tricerules state.
    fn ev_zone_view_sync(&self) -> RuledEvent {
        let per_player: Vec<rv1::RuledPerPlayerView> = self
            .state
            .players
            .iter()
            .map(|p| rv1::RuledPerPlayerView {
                player_id: p.id,
                hand: p
                    .hand
                    .iter()
                    .map(|&oid| {
                        self.state
                            .objects
                            .get(&oid)
                            .map(|o| o.card_id.clone())
                            .unwrap_or_default()
                    })
                    .collect(),
                hand_object_id: p.hand.clone(),
                lib_ids_csv: p
                    .library
                    .iter()
                    .map(|&oid| {
                        self.state
                            .objects
                            .get(&oid)
                            .map(|o| o.card_id.clone())
                            .unwrap_or_default()
                    })
                    .collect::<Vec<_>>()
                    .join(","),
                battlefield: p
                    .battlefield
                    .iter()
                    .map(|&oid| {
                        self.state
                            .objects
                            .get(&oid)
                            .map(|o| o.card_id.clone())
                            .unwrap_or_default()
                    })
                    .collect(),
                battlefield_tapped: p
                    .battlefield
                    .iter()
                    .map(|&oid| {
                        self.state
                            .objects
                            .get(&oid)
                            .map(|o| o.tapped)
                            .unwrap_or(false)
                    })
                    .collect(),
                battlefield_object_id: p.battlefield.to_vec(),
                battlefield_summoning_sick: p
                    .battlefield
                    .iter()
                    .map(|&oid| {
                        self.state
                            .objects
                            .get(&oid)
                            .map(|o| o.summoning_sick)
                            .unwrap_or(false)
                    })
                    .collect(),
                battlefield_power: p
                    .battlefield
                    .iter()
                    .map(|&oid| {
                        self.state.objects.get(&oid).map_or(0, |o| {
                            if o.is_creature(&self.registry) {
                                o.power.unwrap_or(0)
                            } else {
                                0
                            }
                        })
                    })
                    .collect(),
                battlefield_toughness: p
                    .battlefield
                    .iter()
                    .map(|&oid| {
                        self.state.objects.get(&oid).map_or(0, |o| {
                            if o.is_creature(&self.registry) {
                                o.toughness.unwrap_or(0)
                            } else {
                                0
                            }
                        })
                    })
                    .collect(),
                battlefield_damage: p
                    .battlefield
                    .iter()
                    .map(|&oid| {
                        self.state
                            .objects
                            .get(&oid)
                            .filter(|o| o.is_creature(&self.registry))
                            .map_or(0, |o| o.damage)
                    })
                    .collect(),
                battlefield_is_creature: p
                    .battlefield
                    .iter()
                    .map(|&oid| {
                        self.state
                            .objects
                            .get(&oid)
                            .map(|o| o.is_creature(&self.registry))
                            .unwrap_or(false)
                    })
                    .collect(),
            })
            .collect();
        RuledEvent {
            ev: Some(rv1::ruled_event::Ev::ZoneView(rv1::ZoneViewSync {
                per_player,
            })),
        }
    }

    fn game_over_batch_winner(&self, w: PlayerId) -> RuledEventBatch {
        let mut b = RuledEventBatch::default();
        b.events.push(rv1::RuledEvent {
            ev: Some(rv1::ruled_event::Ev::Log(rv1::LogMessage {
                text: format!("Game over. Winner: {w}"),
            })),
        });
        b
    }
}

fn object_display_name(state: &GameState, registry: &CardRegistry, oid: ObjectId) -> String {
    state
        .objects
        .get(&oid)
        .and_then(|o| registry.get(&o.card_id))
        .map(|d| d.name.clone())
        .unwrap_or_else(|| format!("[object {}]", oid))
}

fn describe_target_for_log(state: &GameState, registry: &CardRegistry, tid: ObjectId) -> String {
    if state.player_idx(tid as i32).is_some() {
        format!("P{tid}")
    } else {
        object_display_name(state, registry, tid)
    }
}

fn format_spell_targets_log(
    state: &GameState,
    registry: &CardRegistry,
    targets: &[ObjectId],
) -> String {
    if targets.is_empty() {
        String::new()
    } else {
        let s: Vec<String> = targets
            .iter()
            .map(|&t| describe_target_for_log(state, registry, t))
            .collect();
        format!(" — {}", s.join(", "))
    }
}

fn default_deck_list(player_index: usize) -> Vec<String> {
    if player_index == 0 {
        let mut d: Vec<String> = std::iter::repeat_n("mountain".into(), 20).collect();
        d.extend(std::iter::repeat_n("lightning_bolt".into(), 20));
        d.extend(std::iter::repeat_n("grizzly_bears".into(), 20));
        d.truncate(60);
        d
    } else {
        let mut d: Vec<String> = std::iter::repeat_n("forest".into(), 20).collect();
        d.extend(std::iter::repeat_n("giant_growth".into(), 20));
        d.extend(std::iter::repeat_n("counterspell".into(), 20));
        d.truncate(60);
        d
    }
}

fn fill_legal(batch: &mut RuledEventBatch, eng: &GameEngine) {
    for p in &eng.state.players {
        let labels = legal_labels(eng, p.id);
        batch.legal_by_player.insert(p.id, LegalActions { labels });
    }
}

/// True while the game is waiting for attack or block declarations before
/// players may take spell/activated actions that require priority (CR 508 / 509).
fn priority_locked_for_combat_declaration(state: &GameState) -> bool {
    match state.turn_step {
        TurnStep::DeclareAttackers => state.combat.as_ref().is_some_and(|c| !c.attackers_declared),
        TurnStep::DeclareBlockers => state.combat.as_ref().is_some_and(|c| !c.blockers_declared),
        _ => false,
    }
}

fn opening_legal_labels(eng: &GameEngine, pid: PlayerId, op: &OpeningSequence) -> Vec<String> {
    if op.starting_player.is_none() {
        if pid == op.chooser {
            return vec![
                "You start (opening pick)".into(),
                "Opponent starts (opening pick)".into(),
            ];
        }
        return vec!["Wait: opponent chooses who goes first (opening)".into()];
    }
    if let Some((bp, _rem)) = op.bottom {
        if pid != bp {
            return vec!["Wait: opponent is bottoming cards (opening)".into()];
        }
        let idx = eng.state.player_idx(bp).unwrap();
        let hand = &eng.state.players[idx].hand;
        let mut out = Vec::new();
        for (i, &oid) in hand.iter().enumerate() {
            let name = eng
                .state
                .objects
                .get(&oid)
                .and_then(|o| eng.registry.get(&o.card_id))
                .map(|d| d.name.as_str())
                .unwrap_or("card");
            out.push(format!("Put {name} on bottom (opening, hand idx {i})"));
        }
        return out;
    }
    if let Some(actor) = op.mulligan_actor {
        if pid != actor {
            return vec!["Wait: opponent mulligan decision (opening)".into()];
        }
        return vec![
            "Keep opening hand (opening)".into(),
            "Mulligan — redraw to 7 (opening)".into(),
        ];
    }
    vec!["Wait (opening)".into()]
}

fn legal_labels(eng: &GameEngine, pid: PlayerId) -> Vec<String> {
    if let Some(op) = &eng.state.opening {
        return opening_legal_labels(eng, pid, op);
    }
    // Assign combat damage sub-phase: active player must assign before anything else.
    if let Some(c) = &eng.state.combat {
        if c.blockers_declared && c.damage_assignment_needed && c.assign_combat_damage_phase {
            if pid == eng.state.active_player_id() {
                let mut out = Vec::new();
                for (&att, blks) in &c.blockers {
                    if blks.len() > 1 && !c.damage_assignments.contains_key(&att) {
                        let name = object_display_name(&eng.state, &eng.registry, att);
                        out.push(format!("Assign combat damage for {name}"));
                    }
                }
                return out;
            } else {
                return vec!["Waiting: opponent assigning combat damage".into()];
            }
        }
    }
    let mut v = vec!["Pass priority".into()];
    if eng.state.priority_player_id() != pid {
        return v;
    }
    if eng.state.turn_step == TurnStep::Cleanup {
        if let Some(cp) = eng.state.cleanup_discard_player {
            if pid != cp {
                return vec!["Waiting (opponent cleanup discard)".into()];
            }
            let idx = eng.state.player_idx(cp).unwrap();
            let hand = &eng.state.players[idx].hand;
            if hand.len() <= MAX_HAND_SIZE {
                return v;
            }
            let mut out = Vec::new();
            for (i, &oid) in hand.iter().enumerate() {
                let name = eng
                    .state
                    .objects
                    .get(&oid)
                    .and_then(|o| eng.registry.get(&o.card_id))
                    .map(|d| d.name.as_str())
                    .unwrap_or("card");
                out.push(format!("Discard {name} (cleanup, hand idx {i})"));
            }
            return out;
        }
    }
    let idx = match eng.state.player_idx(pid) {
        Some(i) => i,
        None => return v,
    };
    let instant_ok = instant_timing_step_allowed(eng.state.turn_step);
    let sorcery_ok = sorcery_speed_available(&eng.state, pid);
    let combat_decl_lock = priority_locked_for_combat_declaration(&eng.state);
    for (i, &oid) in eng.state.players[idx].hand.iter().enumerate() {
        let cid = &eng.state.objects.get(&oid).unwrap().card_id;
        if let Some(def) = eng.registry.get(cid) {
            let name = def.name.as_str();
            if def.is_land {
                if sorcery_ok && !eng.state.land_dropped_this_turn {
                    v.push(format!("Play land {name} (hand idx {i})"));
                }
            } else if !combat_decl_lock {
                let cast_ok = (def.is_instant && instant_ok) || (!def.is_instant && sorcery_ok);
                if cast_ok {
                    v.push(format!("Cast {name} (hand idx {i})"));
                }
            }
        } else if !combat_decl_lock && (instant_ok || sorcery_ok) {
            v.push(format!("Play unknown card (hand idx {i})"));
        }
    }
    v
}

fn ev_phase_labeled(eng: &GameEngine, name: &str) -> RuledEvent {
    RuledEvent {
        ev: Some(rv1::ruled_event::Ev::PhaseChanged(rv1::PhaseChanged {
            phase: name.to_string(),
            active_player_id: eng.state.active_player_id(),
        })),
    }
}

fn ev_priority_changed(eng: &GameEngine) -> RuledEvent {
    RuledEvent {
        ev: Some(rv1::ruled_event::Ev::PriorityChanged(
            rv1::PriorityChanged {
                player_id: eng.state.priority_player_id(),
            },
        )),
    }
}

fn finish_with_events(eng: &GameEngine, events: Vec<RuledEvent>) -> RuledEventBatch {
    let mut b = RuledEventBatch {
        events,
        legal_by_player: Default::default(),
    };
    fill_legal(&mut b, eng);
    b
}

fn ev_log(text: String) -> RuledEvent {
    RuledEvent {
        ev: Some(rv1::ruled_event::Ev::Log(rv1::LogMessage { text })),
    }
}

fn draw_card(
    p: &mut PlayerState,
    objects: &mut HashMap<ObjectId, GameObject>,
) -> Result<(), EngineError> {
    let oid = p
        .library
        .pop_front()
        .ok_or(EngineError::Illegal("library empty"))?;
    p.hand.push(oid);
    if let Some(o) = objects.get_mut(&oid) {
        o.zone = Zone::Hand;
    }
    Ok(())
}

/// Build a `PermanentMoved` event, stamping the tricerules `card_id` from the object so
/// servers can resolve cards that have no engine-oid mapping (e.g. milled library cards).
fn permanent_moved_event(
    state: &GameState,
    oid: ObjectId,
    owner_player_id: PlayerId,
    destination: rv1::permanent_moved::Destination,
) -> rv1::RuledEvent {
    let card_id = state
        .objects
        .get(&oid)
        .map(|o| o.card_id.clone())
        .unwrap_or_default();
    rv1::RuledEvent {
        ev: Some(rv1::ruled_event::Ev::PermanentMoved(rv1::PermanentMoved {
            object_id: oid,
            owner_player_id,
            destination: destination as i32,
            card_id,
        })),
    }
}

fn move_object_to_zone(state: &mut GameState, oid: ObjectId, z: Zone) -> Result<(), EngineError> {
    let owner = state
        .objects
        .get(&oid)
        .map(|o| o.owner)
        .ok_or(EngineError::Illegal("no object"))?;
    let idx = state.player_idx(owner).unwrap();
    let p = &mut state.players[idx];
    p.library.retain(|&x| x != oid);
    p.hand.retain(|&x| x != oid);
    p.battlefield.retain(|&x| x != oid);
    p.graveyard.retain(|&x| x != oid);
    p.exile.retain(|&x| x != oid);
    match z {
        Zone::Graveyard => p.graveyard.push(oid),
        Zone::Hand => p.hand.push(oid),
        Zone::Battlefield => p.battlefield.push(oid),
        Zone::Library => p.library.push_back(oid),
        Zone::Exile => p.exile.push(oid),
        Zone::Stack => {}
    }
    if let Some(o) = state.objects.get_mut(&oid) {
        o.zone = z;
    }
    Ok(())
}

fn destroy_permanent(state: &mut GameState, oid: ObjectId) -> Result<(), EngineError> {
    move_object_to_zone(state, oid, Zone::Graveyard)
}

fn pay_mana_simple(
    state: &mut GameState,
    registry: &CardRegistry,
    player_idx: usize,
    cost: &str,
) -> Result<(), EngineError> {
    // Paying spell costs taps the controller's lands while they have priority — not
    // restricted to the active player (e.g. responding with Counterspell on NAP's turn).
    if player_idx != state.priority_idx {
        return Err(EngineError::Illegal(
            "only priority player can pay mana for spells",
        ));
    }
    let mut need_w = 0u32;
    let mut need_u = 0u32;
    let mut need_b = 0u32;
    let mut need_r = 0u32;
    let mut need_g = 0u32;
    let mut need_c = 0u32;
    for ch in cost.chars() {
        match ch {
            'W' => need_w += 1,
            'B' => need_b += 1,
            'R' => need_r += 1,
            'G' => need_g += 1,
            'U' => need_u += 1,
            '1'..='9' => need_c += ch.to_digit(10).unwrap(),
            _ => {}
        }
    }
    // Floating mana from Cockatrice pool counters (AddManaToPool) pays before auto-tapping.
    {
        let pool = &mut state.players[player_idx].mana_pool;
        let take = |need: &mut u32, avail: &mut u32| {
            let t = (*need).min(*avail);
            *avail -= t;
            *need -= t;
        };
        take(&mut need_w, &mut pool.white);
        take(&mut need_u, &mut pool.blue);
        take(&mut need_b, &mut pool.black);
        take(&mut need_r, &mut pool.red);
        take(&mut need_g, &mut pool.green);

        let mut generic = need_c;
        while generic > 0 {
            if pool.colorless > 0 {
                pool.colorless -= 1;
                generic -= 1;
            } else if pool.white > 0 {
                pool.white -= 1;
                generic -= 1;
            } else if pool.blue > 0 {
                pool.blue -= 1;
                generic -= 1;
            } else if pool.black > 0 {
                pool.black -= 1;
                generic -= 1;
            } else if pool.red > 0 {
                pool.red -= 1;
                generic -= 1;
            } else if pool.green > 0 {
                pool.green -= 1;
                generic -= 1;
            } else {
                break;
            }
        }
        need_c = generic;
    }

    let bf = state.players[player_idx].battlefield.clone();
    for &oid in &bf {
        let o = state.objects.get_mut(&oid).unwrap();
        if o.tapped {
            continue;
        }
        let land_color = basic_land_color_from_object(o, registry);
        if need_w > 0 && land_color == Some('W') {
            o.tapped = true;
            need_w -= 1;
        } else if need_u > 0 && land_color == Some('U') {
            o.tapped = true;
            need_u -= 1;
        } else if need_b > 0 && land_color == Some('B') {
            o.tapped = true;
            need_b -= 1;
        } else if need_r > 0 && land_color == Some('R') {
            o.tapped = true;
            need_r -= 1;
        } else if need_g > 0 && land_color == Some('G') {
            o.tapped = true;
            need_g -= 1;
        }
    }
    let mut need = need_c + need_w + need_u + need_b + need_r + need_g;
    if need == 0 {
        return Ok(());
    }
    let bf = state.players[player_idx].battlefield.clone();
    for &oid in &bf {
        if need == 0 {
            break;
        }
        let o = state.objects.get_mut(&oid).unwrap();
        if o.tapped {
            continue;
        }
        if basic_land_color_from_object(o, registry).is_some() {
            o.tapped = true;
            need -= 1;
        }
    }
    if need > 0 {
        return Err(EngineError::Illegal("cannot pay mana"));
    }
    Ok(())
}

fn basic_land_color_from_object(obj: &GameObject, registry: &CardRegistry) -> Option<char> {
    let def = registry.get(&obj.card_id)?;
    if !def.is_land {
        return None;
    }
    if def.types.iter().any(|t| t == "Plains") {
        return Some('W');
    }
    if def.types.iter().any(|t| t == "Island") {
        return Some('U');
    }
    if def.types.iter().any(|t| t == "Swamp") {
        return Some('B');
    }
    if def.types.iter().any(|t| t == "Mountain") {
        return Some('R');
    }
    if def.types.iter().any(|t| t == "Forest") {
        return Some('G');
    }
    None
}

/// Player or creature permanent on the battlefield (matches cast validation for `bolt`).
fn damage_spell_target_legal(state: &GameState, registry: &CardRegistry, tid: ObjectId) -> bool {
    if state.player_idx(tid as i32).is_some() {
        return true;
    }
    state
        .objects
        .get(&tid)
        .is_some_and(|o| o.zone == Zone::Battlefield && o.is_creature(registry))
}

fn pump_spell_target_legal(state: &GameState, registry: &CardRegistry, tid: ObjectId) -> bool {
    state
        .objects
        .get(&tid)
        .is_some_and(|o| o.zone == Zone::Battlefield && o.is_creature(registry))
}

fn destroy_spell_target_legal(state: &GameState, registry: &CardRegistry, tid: ObjectId) -> bool {
    state
        .objects
        .get(&tid)
        .is_some_and(|o| o.zone == Zone::Battlefield && o.is_creature(registry))
}

/// Target must be an active player (not lost).
fn player_target_legal(state: &GameState, tid: ObjectId) -> bool {
    state
        .player_idx(tid as i32)
        .is_some_and(|pi| !state.players[pi].has_lost)
}

/// Any battlefield permanent (creature, land, etc.) — for broad bounce like Boomerang.
fn any_battlefield_permanent_target_legal(state: &GameState, tid: ObjectId) -> bool {
    state
        .objects
        .get(&tid)
        .is_some_and(|o| o.zone == Zone::Battlefield)
}

/// Legality of a single target against a data-driven [`TargetSpec`].
/// `caster` is needed only to enforce the opponent-only restriction.
fn target_spec_legal(
    state: &GameState,
    registry: &CardRegistry,
    spec: &TargetSpec,
    tid: ObjectId,
    caster: PlayerId,
) -> bool {
    match spec {
        TargetSpec::AnyTarget => damage_spell_target_legal(state, registry, tid),
        TargetSpec::Creature => destroy_spell_target_legal(state, registry, tid),
        TargetSpec::AnyPlayer => player_target_legal(state, tid),
        TargetSpec::OpponentPlayer => player_target_legal(state, tid) && tid as i32 != caster,
    }
}

/// CR 608.2b-style: if every target for the spell is now illegal, none of its effects happen.
fn spell_has_no_legal_targets_at_resolution(
    state: &GameState,
    registry: &CardRegistry,
    effect: &SpellEffectKind,
    targets: &[ObjectId],
    caster: PlayerId,
) -> bool {
    match effect {
        SpellEffectKind::None
        | SpellEffectKind::Draw { .. }
        | SpellEffectKind::GainLife { .. }
        | SpellEffectKind::EachOpponentLosesLifeYouGainEqual { .. } => false,
        SpellEffectKind::DamageTarget { target, .. }
        | SpellEffectKind::TargetPlayerGainsLife { target, .. }
        | SpellEffectKind::TargetPlayerLosesLife { target, .. }
        | SpellEffectKind::MillTargetPlayer { target, .. } => !targets
            .first()
            .is_some_and(|&tid| target_spec_legal(state, registry, target, tid, caster)),
        SpellEffectKind::PumpTarget { .. } => !targets
            .first()
            .is_some_and(|&tid| pump_spell_target_legal(state, registry, tid)),
        SpellEffectKind::DestroyTarget
        | SpellEffectKind::ExileTarget
        | SpellEffectKind::ExileTargetGainLifeEqualToPower
        | SpellEffectKind::ReturnTargetCreatureToHand => !targets
            .first()
            .is_some_and(|&tid| destroy_spell_target_legal(state, registry, tid)),
        SpellEffectKind::ReturnTargetPermanentToHand => !targets
            .first()
            .is_some_and(|&tid| any_battlefield_permanent_target_legal(state, tid)),
        SpellEffectKind::CounterTargetSpell => !targets
            .first()
            .is_some_and(|&tid| state.stack.iter().any(|s| s.id == tid)),
    }
}

fn validate_spell_targets(
    state: &GameState,
    registry: &CardRegistry,
    caster: PlayerId,
    card_id: &str,
    targets: &[rv1::TargetRef],
) -> Result<(), EngineError> {
    let effect = registry
        .get(card_id)
        .and_then(|c| c.spell_effect.clone())
        .unwrap_or(SpellEffectKind::None);

    match effect {
        SpellEffectKind::DestroyTarget => {
            if targets.len() != 1 {
                return Err(EngineError::Illegal(
                    "destroy spells require exactly one target",
                ));
            }
            let target = targets[0].object_id;
            if !destroy_spell_target_legal(state, registry, target) {
                return Err(EngineError::Illegal(
                    "destroy target must be a creature on battlefield",
                ));
            }
        }
        SpellEffectKind::CounterTargetSpell => {
            if targets.len() != 1 {
                return Err(EngineError::Illegal(
                    "counterspell requires exactly one stack target",
                ));
            }
            let target = targets[0].object_id;
            if !state.stack.iter().any(|s| s.id == target) {
                return Err(EngineError::Illegal("counter target must be on stack"));
            }
        }
        SpellEffectKind::DamageTarget { target: spec, .. } => {
            if targets.len() != 1 {
                return Err(EngineError::Illegal(
                    "damage spells require exactly one target",
                ));
            }
            let target = targets[0].object_id;
            if !target_spec_legal(state, registry, &spec, target, caster) {
                return Err(EngineError::Illegal(
                    "damage target must be a battlefield creature or player",
                ));
            }
        }
        SpellEffectKind::PumpTarget { .. } => {
            if targets.len() != 1 {
                return Err(EngineError::Illegal(
                    "pump spells require exactly one target",
                ));
            }
            let target = targets[0].object_id;
            if !pump_spell_target_legal(state, registry, target) {
                return Err(EngineError::Illegal(
                    "pump target must be a creature on the battlefield",
                ));
            }
        }
        SpellEffectKind::ExileTarget
        | SpellEffectKind::ExileTargetGainLifeEqualToPower
        | SpellEffectKind::ReturnTargetCreatureToHand => {
            if targets.len() != 1 {
                return Err(EngineError::Illegal(
                    "this spell requires exactly one creature target",
                ));
            }
            let target = targets[0].object_id;
            if !destroy_spell_target_legal(state, registry, target) {
                return Err(EngineError::Illegal(
                    "target must be a creature on the battlefield",
                ));
            }
        }
        SpellEffectKind::ReturnTargetPermanentToHand => {
            if targets.len() != 1 {
                return Err(EngineError::Illegal(
                    "this spell requires exactly one permanent target",
                ));
            }
            let target = targets[0].object_id;
            if !any_battlefield_permanent_target_legal(state, target) {
                return Err(EngineError::Illegal(
                    "target must be a permanent on the battlefield",
                ));
            }
        }
        SpellEffectKind::TargetPlayerGainsLife { target: spec, .. }
        | SpellEffectKind::TargetPlayerLosesLife { target: spec, .. }
        | SpellEffectKind::MillTargetPlayer { target: spec, .. } => {
            if targets.len() != 1 {
                return Err(EngineError::Illegal(
                    "this spell requires exactly one player target",
                ));
            }
            let target = targets[0].object_id;
            if !player_target_legal(state, target) {
                return Err(EngineError::Illegal(
                    "target must be a player still in the game",
                ));
            }
            if matches!(spec, TargetSpec::OpponentPlayer) && target as i32 == caster {
                return Err(EngineError::Illegal(
                    "target must be an opponent (cannot target yourself)",
                ));
            }
        }
        SpellEffectKind::GainLife { .. }
        | SpellEffectKind::EachOpponentLosesLifeYouGainEqual { .. }
            if !targets.is_empty() =>
        {
            return Err(EngineError::Illegal("this spell takes no targets"));
        }
        _ => {}
    }
    Ok(())
}
